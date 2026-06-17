//! The reachable subset of `NubSpline`: the
//! clamped uniform constructor, conversion to bezier form, and extraction of
//! the bezier control points. The derivation / inversion / extremum machinery
//! is only used by `NubsSelfLoop`, which is itself unreachable in ELK 0.11.0
//! (the new self loop machinery uses `SplineSelfLoopRouter`).

use crate::graph::math::KVector;

const EPSILON: f64 = 0.000001;

#[derive(Clone, Debug)]
struct PolarCP {
    cp: KVector,
    polar_coordinate: Vec<f64>,
}

impl PolarCP {
    fn new(control_point: KVector, polar_coordinate: &[f64]) -> PolarCP {
        PolarCP { cp: control_point, polar_coordinate: polar_coordinate.to_vec() }
    }

    /// Calculates the new PolarCP from the two given ones during knot insertion.
    fn combine(first_cp: &PolarCP, second_cp: &PolarCP, new_knot: f64) -> PolarCP {
        let first_factor = first_cp.polar_coordinate[0];
        let second_factor = *second_cp.polar_coordinate.last().unwrap();

        // vectorC = ((b-c) * vectorA + (c-a) * vectorB) / (b-a)
        let mut a_scaled = first_cp.cp;
        a_scaled.scale(second_factor - new_knot);
        let mut b_scaled = second_cp.cp;
        b_scaled.scale(new_knot - first_factor);
        let mut total = a_scaled;
        total.add(b_scaled);
        total.scale(1.0 / (second_factor - first_factor));

        let mut polar_coordinate = Vec::new();
        // Specifies if the newKnot still needs to be added.
        let mut needs_to_be_added = true;
        // From the firstCP, we take the W (skip the first element).
        for &next_knot in first_cp.polar_coordinate.iter().skip(1) {
            if needs_to_be_added && next_knot - new_knot > EPSILON {
                polar_coordinate.push(new_knot);
                needs_to_be_added = false;
            }
            polar_coordinate.push(next_knot);
        }
        if needs_to_be_added {
            polar_coordinate.push(new_knot);
        }

        PolarCP { cp: total, polar_coordinate }
    }
}

pub struct NubSpline {
    knot_vector: Vec<f64>,
    control_points: Vec<PolarCP>,
    dim_nubs: usize,
    is_clamped: bool,
    is_bezier: bool,
    #[allow(dead_code)]
    min_knot: f64,
    max_knot: f64,
}

impl NubSpline {
    /// Clamped constructor (`clamped == true`). Note that the constructor
    /// mutates the passed list (padding it at the front); this is replicated on
    /// the owned `Vec`.
    pub fn new_clamped(dimension: usize, mut k_vectors: Vec<KVector>) -> NubSpline {
        assert!(dimension >= 1, "The dimension must be at least 1!");

        // fill the list of control-points to be at least equal to dimension + 1
        let mut i = k_vectors.len() as i32 - 1;
        while i < dimension as i32 {
            let first = k_vectors[0];
            k_vectors.insert(0, first);
            i += 1;
        }

        assert!(
            k_vectors.len() >= dimension + 1,
            "At (least dimension + 1) control points are necessary!"
        );

        let mut spline = NubSpline {
            knot_vector: Vec::new(),
            control_points: Vec::new(),
            dim_nubs: dimension,
            is_clamped: true,
            is_bezier: false,
            min_knot: 0.0,
            max_knot: 0.0,
        };

        // create the knot vector
        spline.create_uniform_knot_vector(true, k_vectors.len() + spline.dim_nubs - 1);

        let mut polar_coordinate: Vec<f64> = Vec::new();
        let mut knot_iter = spline.knot_vector.iter();

        // the first (dimNUBS - 1) elements of the knotVector for the "sliding
        // window" that determines the polarCoordinates of the PolarCP.
        for _ in 0..(spline.dim_nubs - 1) {
            polar_coordinate.push(*knot_iter.next().unwrap());
        }

        // Create the PolarCPs: a sliding window over the knot vector
        // determines the polar coordinates of the PolarCPs.
        let mut control_points = Vec::new();
        for k_vector in &k_vectors {
            polar_coordinate.push(*knot_iter.next().unwrap());
            control_points.push(PolarCP::new(*k_vector, &polar_coordinate));
            polar_coordinate.remove(0);
        }
        spline.control_points = control_points;

        spline
    }

    /// Clamped variant.
    fn create_uniform_knot_vector(&mut self, clamped: bool, size: usize) {
        assert!(
            size >= 2 * self.dim_nubs,
            "The knot vector must have at least two time the dimension elements."
        );
        let my_size: f64;

        if clamped {
            self.min_knot = 0.0;
            self.max_knot = 1.0;
            for _ in 0..self.dim_nubs {
                self.knot_vector.push(0.0);
            }
            my_size = (size as i32 + 1 - 2 * self.dim_nubs as i32) as f64;
        } else {
            my_size = (size + 1) as f64;
            let ddim = self.dim_nubs as f64;
            self.min_knot = ddim / (my_size + 1.0);
            self.max_knot = (my_size - ddim) / my_size;
        }

        let fraction = my_size;
        let mut i = 1;
        while (i as f64) < my_size {
            self.knot_vector.push(i as f64 / fraction);
            i += 1;
        }

        if self.is_clamped {
            for _ in 0..self.dim_nubs {
                self.knot_vector.push(1.0);
            }
        }
    }

    fn get_multiplicity(&self, knot_to_check: f64) -> usize {
        let mut count = 0;
        for &current_knot in &self.knot_vector {
            let diff = current_knot - knot_to_check;
            if diff > EPSILON {
                return count;
            } else if diff > -EPSILON {
                count += 1;
            }
        }
        count
    }

    /// The two cursors emulate `ListIterator`
    /// positions ("between" indices) over `control_points` and
    /// `knot_vector` respectively.
    fn insert_knot_at_current_position(
        &mut self,
        insertions: usize,
        knot_to_insert: f64,
        iter_cp: &mut usize,
        iter_knot: &mut usize,
    ) {
        let multiplicity = self.get_multiplicity(knot_to_insert);
        for i in 0..insertions {
            // Insert the new knot to the knotVector.
            self.knot_vector.insert(*iter_knot, knot_to_insert);
            *iter_knot += 1;

            // We will first construct the new CPs and then add them.
            let mut new_cps: Vec<PolarCP> = Vec::new();
            // The first CP we need for the calculation.
            let mut second_cp_idx = *iter_cp; // iterCP.next()
            *iter_cp += 1;

            for _j in (multiplicity + i)..self.dim_nubs {
                // The second CP we need for the calculation.
                let first_cp_idx = second_cp_idx;
                second_cp_idx = *iter_cp; // iterCP.next()
                *iter_cp += 1;

                // Calculate the new CP.
                new_cps.push(PolarCP::combine(
                    &self.control_points[first_cp_idx],
                    &self.control_points[second_cp_idx],
                    knot_to_insert,
                ));
            }

            // move to the insertion position, and on the way delete all CPs we
            // have used for two calculations as they don't belong to the new
            // list of CPs.
            for j in (multiplicity + i)..self.dim_nubs {
                *iter_cp -= 1; // iterCP.previous()
                if j > multiplicity + i {
                    // iterCP.remove(): removes the element just returned by
                    // previous(); the cursor stays at the same index.
                    self.control_points.remove(*iter_cp);
                }
            }

            // now we can add the new CPs
            for cp in new_cps {
                self.control_points.insert(*iter_cp, cp);
                *iter_cp += 1;
            }

            // Move back to the position in front of the first new CP, if there
            // will be more insertions
            if i < insertions - 1 {
                for _j in (multiplicity + i)..self.dim_nubs {
                    *iter_cp -= 1; // iterCP.previous()
                }
            }
        }
    }

    /// Converts this NubSpline to a bezier spline. All inner
    /// knots of the knotVector get the multiplicity of dimNUBS.
    pub fn to_bezier(&mut self) {
        let mut iter_knot: usize = 0;
        let mut iter_cp: usize = 0;

        // Clamped knotVectors have (dim) leading and trailing knots that are
        // already repeated. We skip them for performance.
        if self.is_clamped {
            iter_knot += self.dim_nubs;
        } else {
            for _ in 0..(self.dim_nubs - 1) {
                // iterKnot.next() followed by iterKnot.remove()
                self.knot_vector.remove(iter_knot);
            }
        }

        let mut current_knot = self.knot_vector[iter_knot];
        iter_knot += 1;
        // Iterate over all knots whose multiplicity we possibly have to increase.
        while self.max_knot - current_knot > EPSILON {
            let knot_to_count = current_knot;
            let mut occurrence: usize = 0;

            // Count occurrences of knotToCount.
            while (current_knot - knot_to_count).abs() < EPSILON {
                occurrence += 1;
                current_knot = self.knot_vector[iter_knot];
                iter_knot += 1;
                iter_cp += 1; // iterCP.next()
            }

            // insert new knots, if multiplicity is not as expected (dimNUBS)
            if occurrence < self.dim_nubs {
                iter_knot -= 1; // iterKnot.previous()
                self.insert_knot_at_current_position(
                    self.dim_nubs - occurrence,
                    knot_to_count,
                    &mut iter_cp,
                    &mut iter_knot,
                );
                iter_knot += 1; // iterKnot.next() (value discarded)
            }

            // Proceed to next elements.
            iter_cp -= 1; // iterCP.previous()
        }

        if !self.is_clamped {
            for _ in 0..(self.dim_nubs - 1) {
                self.knot_vector.remove(iter_knot);
            }
        }
        self.is_clamped = true;
        self.is_bezier = true;
    }

    /// All bezier control points without the source and target vectors.
    pub fn get_bezier_cp(&mut self) -> Vec<KVector> {
        if !self.is_bezier {
            self.to_bezier();
        }
        let mut ret_val: Vec<KVector> = self
            .control_points
            .iter()
            .skip(1) // withSourceVector == false
            .map(|p| p.cp)
            .collect();
        ret_val.pop(); // withTargetVector == false
        ret_val
    }
}
