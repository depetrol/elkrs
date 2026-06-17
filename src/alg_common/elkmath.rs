//! Ports of the parts of `org.eclipse.elk.core.math.ElkMath` and Guava's
//! `DoubleMath` that the spore and disco algorithms rely on.

use crate::graph::math::{ElkRectangle, KVector, KVectorChain};

/// `ElkMath.DOUBLE_EQ_EPSILON`.
pub const DOUBLE_EQ_EPSILON: f64 = 0.00001;

/// Guava `DoubleMath.fuzzyEquals`.
pub fn fuzzy_equals(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance || a == b || (a.is_nan() && b.is_nan())
}

/// Guava `DoubleMath.fuzzyCompare`.
pub fn fuzzy_compare(a: f64, b: f64, tolerance: f64) -> i32 {
    if fuzzy_equals(a, b, tolerance) {
        0
    } else if a < b {
        -1
    } else if a > b {
        1
    } else {
        // Booleans.compare(isNaN(a), isNaN(b))
        (a.is_nan() as i32) - (b.is_nan() as i32)
    }
}

/// Intersection point of segments `p + t*r`
/// and `q + u*s` (0 <= t,u <= 1), or `None`.
pub fn intersects2(p: KVector, r: KVector, q: KVector, s: KVector) -> Option<KVector> {
    let mut pq = q;
    pq.sub(p);
    let pq_x_r = KVector::cross_product(pq, r);
    let r_x_s = KVector::cross_product(r, s);
    let t = KVector::cross_product(pq, s) / r_x_s;
    let u = pq_x_r / r_x_s;
    if r_x_s == 0.0 {
        if pq_x_r == 0.0 {
            // collinear: return point closest to center of s
            let mut center = s;
            center.scale(0.5);
            center.add(q);
            let d1 = p.distance(center);
            let mut p_plus_r = p;
            p_plus_r.add(r);
            let d2 = p_plus_r.distance(center);
            let l = s.length() * 0.5;
            if d1 < d2 && d1 <= l {
                return Some(p);
            }
            if d2 <= l {
                return Some(p_plus_r);
            }
            None
        } else {
            None
        }
    } else if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        let mut res = r;
        res.scale(t);
        res.add(p);
        Some(res)
    } else {
        None
    }
}

/// `KVector.equalsFuzzily(other)` with the default fuzzyness (0.05).
fn equals_fuzzily_default(a: KVector, b: KVector) -> bool {
    a.equals_fuzzily(b, 0.05)
}

fn trace_rays(a1: KVector, a2: KVector, b1: KVector, b2: KVector, v: KVector) -> f64 {
    let mut result = f64::INFINITY;
    let mut endpoint_hit = false;

    let mut a_dir = a2;
    a_dir.sub(a1);
    let mut b_dir = b2;
    b_dir.sub(b1);
    let mut b1_plus_v = b1;
    b1_plus_v.add(v);

    let intersection = intersects2(a1, a_dir, b1_plus_v, b_dir);
    let edge_case = intersection.is_some_and(|i| {
        !(equals_fuzzily_default(i, a1) || equals_fuzzily_default(i, a2))
    });

    if let Some(mut i) = intersects2(a1, a_dir, b1, v) {
        if equals_fuzzily_default(i, a1) == equals_fuzzily_default(i, a2) || edge_case {
            i.sub(b1);
            result = f64::min(result, i.length());
        } else {
            endpoint_hit = true;
        }
    }

    if let Some(mut i) = intersects2(a1, a_dir, b2, v) {
        if endpoint_hit
            || equals_fuzzily_default(i, a1) == equals_fuzzily_default(i, a2)
            || edge_case
        {
            i.sub(b2);
            result = f64::min(result, i.length());
        }
    }

    result
}

/// Direction-dependent
/// distance between two line segments.
pub fn distance_segments(a1: KVector, a2: KVector, b1: KVector, b2: KVector, v: KVector) -> f64 {
    let mut neg_v = v;
    neg_v.negate();
    f64::min(
        trace_rays(a1, a2, b1, b2, v),
        trace_rays(b1, b2, a1, a2, neg_v),
    )
}

pub fn shortest_distance(r1: &ElkRectangle, r2: &ElkRectangle) -> f64 {
    let right_dist = r2.x - (r1.x + r1.width);
    let left_dist = r1.x - (r2.x + r2.width);
    let top_dist = r1.y - (r2.y + r2.height);
    let bottom_dist = r2.y - (r1.y + r1.height);
    let horz_dist = f64::max(left_dist, right_dist);
    let vert_dist = f64::max(top_dist, bottom_dist);
    if (fuzzy_compare(horz_dist, 0.0, DOUBLE_EQ_EPSILON) >= 0)
        ^ (fuzzy_compare(vert_dist, 0.0, DOUBLE_EQ_EPSILON) >= 0)
    {
        // case 1
        return f64::max(vert_dist, horz_dist);
    }
    if fuzzy_compare(horz_dist, 0.0, DOUBLE_EQ_EPSILON) > 0 {
        // case 2
        return (vert_dist * vert_dist + horz_dist * horz_dist).sqrt();
    }
    // case 3
    -(vert_dist * vert_dist + horz_dist * horz_dist).sqrt()
}

pub fn clip_vector(v: &mut KVector, width: f64, height: f64) {
    let wh = width / 2.0;
    let hh = height / 2.0;
    let absx = v.x.abs();
    let absy = v.y.abs();
    let mut xscale = 1.0;
    let mut yscale = 1.0;
    if absx > wh {
        xscale = wh / absx;
    }
    if absy > hh {
        yscale = hh / absy;
    }
    v.scale(f64::min(xscale, yscale));
}

// ------------------------------------------------------------------------
// Factorials, binomials, powers (ports of the `ElkMath` integer math).

/// `ElkMath.FACT_TABLE`: precomputed factorial values.
const FACT_TABLE: [i64; 21] = [
    1,
    1,
    2,
    6,
    24,
    120,
    720,
    5040,
    40320,
    362880,
    3628800,
    39916800,
    479001600,
    6227020800,
    87178291200,
    1307674368000,
    20922789888000,
    355687428096000,
    6402373705728000,
    121645100408832000,
    2432902008176640000,
];

/// Panics if `x < 0` or `x > 20`.
pub fn factl(x: i32) -> i64 {
    if x < 0 || x as usize >= FACT_TABLE.len() {
        panic!("The input must be between 0 and {}", FACT_TABLE.len());
    }
    FACT_TABLE[x as usize]
}

/// Panics if `x < 0`; uses Stirling's approximation for large values.
pub fn factd(x: i32) -> f64 {
    if x < 0 {
        panic!("The input must be positive");
    } else if (x as usize) < FACT_TABLE.len() {
        FACT_TABLE[x as usize] as f64
    } else {
        let xf = x as f64;
        (2.0 * std::f64::consts::PI * xf).sqrt()
            * (powf(x as f32, x) as f64 / powd(std::f64::consts::E, x))
    }
}

/// Panics on negative input or `k > n`.
pub fn binomiall(n: i32, k: i32) -> i64 {
    if n < 0 || k < 0 {
        panic!("k and n must be positive");
    } else if k > n {
        panic!("k must be smaller than n");
    } else if k == 0 || k == n {
        1
    } else if n == 0 {
        0
    } else if (n as usize) < FACT_TABLE.len() {
        factl(n) / (factl(k) * factl(n - k))
    } else {
        binomiall(n - 1, k - 1) + binomiall(n - 1, k)
    }
}

/// Panics on negative input or `k > n`.
pub fn binomiald(n: i32, k: i32) -> f64 {
    if n < 0 || k < 0 {
        panic!("k and n must be positive");
    } else if k > n {
        panic!("k must be smaller than n");
    } else if k == 0 || k == n {
        1.0
    } else if n == 0 {
        0.0
    } else {
        factd(n) / (factd(k) * factd(n - k))
    }
}

pub fn powd(a: f64, b: i32) -> f64 {
    let mut result = 1.0f64;
    let mut base = a;
    let mut exp = b.abs();
    while exp > 0 {
        if exp % 2 == 0 {
            base *= base;
            exp /= 2;
        } else {
            result *= base;
            exp -= 1;
        }
    }
    if b < 0 {
        1.0 / result
    } else {
        result
    }
}

pub fn powf(a: f32, b: i32) -> f32 {
    let mut result = 1.0f32;
    let mut base = a;
    let mut exp = b.abs();
    while exp > 0 {
        if exp % 2 == 0 {
            base *= base;
            exp /= 2;
        } else {
            result *= base;
            exp -= 1;
        }
    }
    if b < 0 {
        1.0 / result
    } else {
        result
    }
}

// ------------------------------------------------------------------------
// Bezier curves (ports of the `ElkMath` Bezier helpers).

pub fn get_point_on_bezier_segment(t: f64, control_points: &[KVector]) -> KVector {
    let n = control_points.len() as i32 - 1;
    let mut px = 0.0;
    let mut py = 0.0;
    for (j, p) in control_points.iter().enumerate() {
        let j = j as i32;
        let factor = binomiald(n, j) * powd(1.0 - t, n - j) * powd(t, j);
        px += p.x * factor;
        py += p.y * factor;
    }
    KVector::new(px, py)
}

/// Points on the
/// curve including the target point but not the source point.
pub fn approximate_bezier_segment(result_size: i32, control_points: &[KVector]) -> Vec<KVector> {
    if result_size <= 0 {
        return Vec::new();
    }
    let dt = 1.0 / result_size as f64;
    let mut t = 0.0;
    let mut result = Vec::with_capacity(result_size as usize);
    for _ in 0..result_size {
        t += dt;
        result.push(get_point_on_bezier_segment(t, control_points));
    }
    result
}

/// The number of
/// approximation points equals the number of control points plus one.
pub fn approximate_bezier_segment_auto(control_points: &[KVector]) -> Vec<KVector> {
    approximate_bezier_segment(control_points.len() as i32 + 1, control_points)
}

/// Interprets the control points
/// as a series of cubic Bezier curves.
pub fn approximate_bezier_spline(control_points: &KVectorChain) -> KVectorChain {
    let ctrl_pt_count = control_points.len();
    let mut spline = KVectorChain::new();
    let pts = &control_points.0;
    let mut i = 0usize;
    let mut current_point = pts[i];
    i += 1;
    spline.add_last(current_point);
    while i < ctrl_pt_count {
        let remaining_points = ctrl_pt_count - i;
        if remaining_points == 1 {
            spline.add_last(pts[i]);
            i += 1;
        } else if remaining_points == 2 {
            // calculate a quadratic bezier curve
            let segment = [current_point, pts[i], pts[i + 1]];
            i += 2;
            spline.0.extend(approximate_bezier_segment_auto(&segment));
        } else {
            // calculate a cubic bezier curve
            let control1 = pts[i];
            let control2 = pts[i + 1];
            let next_point = pts[i + 2];
            i += 3;
            let segment = [current_point, control1, control2, next_point];
            spline.0.extend(approximate_bezier_segment_auto(&segment));
            current_point = next_point;
        }
    }
    spline
}

/// degree of splines equation to find roots (`ElkMath.W_DEGREE`).
const W_DEGREE: usize = 5;
/// cubic Bezier curves (`ElkMath.DEGREE`).
const DEGREE: usize = 3;
/// precomputed "z" for cubics (`ElkMath.CUBIC_Z`).
const CUBIC_Z: [[f64; 4]; 3] = [
    [1.0, 0.6, 0.3, 0.1],
    [0.4, 0.6, 0.6, 0.4],
    [0.1, 0.3, 0.6, 1.0],
];

/// Distance from a cubic spline curve to the point `needle`.
pub fn distance_from_bezier_segment(
    start: KVector,
    c1: KVector,
    c2: KVector,
    end: KVector,
    needle: KVector,
) -> f64 {
    let mut t_candidate = [0.0f64; W_DEGREE]; // possible roots
    let v = [start, c1, c2, end];

    // convert problem to 5th-degree Bezier form
    let w = convert_to_bezier_form(&v, needle);

    // Find all possible roots of 5th-degree equation
    let n_solutions = find_roots(&w, W_DEGREE, &mut t_candidate, 0);

    // Compare distances of P5 to all candidates, and to t=0, and t=1
    let mut min_distance = needle.distance(start);
    let mut t = 0.0;

    for &tc in t_candidate.iter().take(n_solutions) {
        let p = bezier(&v, DEGREE, tc, None, None);
        let distance = needle.distance(p);
        if distance < min_distance {
            min_distance = distance;
            t = tc;
        }
    }

    // Finally, look at distance to end point, where t = 1.0
    let distance = needle.distance(end);
    if distance < min_distance {
        t = 1.0;
    }

    let pn = bezier(&v, DEGREE, t, None, None);
    pn.distance(needle).sqrt()
}

fn convert_to_bezier_form(v: &[KVector; DEGREE + 1], pa: KVector) -> [KVector; W_DEGREE + 1] {
    let mut c = [KVector::default(); DEGREE + 1]; // v(i) - pa
    let mut d = [KVector::default(); DEGREE]; // v(i+1) - v(i)
    let mut cd_table = [[0.0f64; DEGREE + 1]; DEGREE]; // dot product of c, d
    let mut w = [KVector::default(); W_DEGREE + 1]; // ctl pts of 5th-degree curve

    for i in 0..=DEGREE {
        c[i] = KVector::new(v[i].x - pa.x, v[i].y - pa.y);
    }

    let s = DEGREE as f64;
    for i in 0..DEGREE {
        d[i] = KVector::new(s * (v[i + 1].x - v[i].x), s * (v[i + 1].y - v[i].y));
    }

    for (row, d_row) in d.iter().enumerate() {
        for (column, c_col) in c.iter().enumerate() {
            cd_table[row][column] = d_row.x * c_col.x + d_row.y * c_col.y;
        }
    }

    for (i, wi) in w.iter_mut().enumerate() {
        *wi = KVector::new(i as f64 / W_DEGREE as f64, 0.0);
    }

    let n = DEGREE;
    let m = DEGREE - 1;
    for k in 0..=(n + m) {
        let lb = k.saturating_sub(m);
        let ub = k.min(n);
        for i in lb..=ub {
            let j = k - i;
            w[i + j].y += cd_table[j][i] * CUBIC_Z[j][i];
        }
    }

    w
}

/// maximum depth for recursion (`ElkMath.MAXDEPTH`).
const MAXDEPTH: i32 = 64;
/// Flatness (`ElkMath.EPSILON`): `1.0 * Math.pow(2, -MAXDEPTH - 1)`.
const FIND_ROOTS_EPSILON: f64 = 1.0 / ((1u128 << (MAXDEPTH + 1)) as f64);

/// All roots of a 5th-degree equation in
/// Bernstein-Bezier form within [0, 1]. Returns the number of roots found.
fn find_roots(w: &[KVector; W_DEGREE + 1], degree: usize, t: &mut [f64], depth: i32) -> usize {
    match crossing_count(w, degree) {
        0 => return 0, // No solutions here
        1 => {
            // Unique solution; stop recursion when the tree is deep enough
            if depth >= MAXDEPTH {
                t[0] = (w[0].x + w[W_DEGREE].x) / 2.0;
                return 1;
            }
            if control_polygon_flat_enough(w, degree) {
                t[0] = compute_x_intercept(w, degree);
                return 1;
            }
        }
        _ => {} // nothing
    }

    // Otherwise, solve recursively after subdividing control polygon
    let mut left = [KVector::default(); W_DEGREE + 1];
    let mut right = [KVector::default(); W_DEGREE + 1];
    let mut left_t = [0.0f64; W_DEGREE + 1];
    let mut right_t = [0.0f64; W_DEGREE + 1];

    bezier(w, degree, 0.5, Some(&mut left), Some(&mut right));
    let left_count = find_roots(&left, degree, &mut left_t, depth + 1);
    let right_count = find_roots(&right, degree, &mut right_t, depth + 1);

    t[..left_count].copy_from_slice(&left_t[..left_count]);
    t[left_count..left_count + right_count].copy_from_slice(&right_t[..right_count]);

    left_count + right_count
}

fn control_polygon_flat_enough(v: &[KVector; W_DEGREE + 1], degree: usize) -> bool {
    // Derive the implicit equation for line connecting first and last
    // control points
    let a = v[0].y - v[degree].y;
    let b = v[degree].x - v[0].x;
    let c = v[0].x * v[degree].y - v[degree].x * v[0].y;

    let ab_squared = a * a + b * b;
    let mut distance = [0.0f64; W_DEGREE + 1];

    for (i, di) in distance.iter_mut().enumerate().take(degree).skip(1) {
        *di = a * v[i].x + b * v[i].y + c;
        if *di > 0.0 {
            *di = (*di * *di) / ab_squared;
        }
        if *di < 0.0 {
            *di = -((*di * *di) / ab_squared);
        }
    }

    let mut max_distance_above = 0.0f64;
    let mut max_distance_below = 0.0f64;
    for &di in distance.iter().take(degree).skip(1) {
        if di < 0.0 {
            max_distance_below = max_distance_below.min(di);
        }
        if di > 0.0 {
            max_distance_above = max_distance_above.max(di);
        }
    }

    // Implicit equation for zero line
    let a1 = 0.0;
    let b1 = 1.0;
    let c1 = 0.0;

    // Implicit equation for "above" line
    let mut a2 = a;
    let mut b2 = b;
    let mut c2 = c + max_distance_above;

    let mut det = a1 * b2 - a2 * b1;
    let mut d_inv = 1.0 / det;

    let intercept1 = (b1 * c2 - b2 * c1) * d_inv;

    // Implicit equation for "below" line
    a2 = a;
    b2 = b;
    c2 = c + max_distance_below;

    det = a1 * b2 - a2 * b1;
    d_inv = 1.0 / det;

    let intercept2 = (b1 * c2 - b2 * c1) * d_inv;

    // Compute intercepts of bounding box
    let left_intercept = intercept1.min(intercept2);
    let right_intercept = intercept1.max(intercept2);

    let error = (right_intercept - left_intercept) / 2.0;

    error < FIND_ROOTS_EPSILON
}

fn compute_x_intercept(v: &[KVector; W_DEGREE + 1], degree: usize) -> f64 {
    let xnm = v[degree].x - v[0].x;
    let ynm = v[degree].y - v[0].y;
    let xmk = v[0].x;
    let ymk = v[0].y;

    let det_inv = -1.0 / ynm;

    (xnm * ymk - ynm * xmk) * det_inv
}

fn crossing_count(v: &[KVector; W_DEGREE + 1], degree: usize) -> usize {
    let mut n_crossings = 0;
    let mut old_sign = if v[0].y < 0.0 { -1 } else { 1 };
    for vi in v.iter().take(degree + 1).skip(1) {
        let sign = if vi.y < 0.0 { -1 } else { 1 };
        if sign != old_sign {
            n_crossings += 1;
        }
        old_sign = sign;
    }
    n_crossings
}

/// Computes a point on the curve and optionally the
/// left/right control polygons of the subdivision at `t`.
fn bezier(
    c: &[KVector],
    degree: usize,
    t: f64,
    left: Option<&mut [KVector]>,
    right: Option<&mut [KVector]>,
) -> KVector {
    let mut p = [[KVector::default(); W_DEGREE + 1]; W_DEGREE + 1];

    p[0][..=degree].copy_from_slice(&c[..=degree]);

    for i in 1..=degree {
        for j in 0..=(degree - i) {
            p[i][j] = KVector::new(
                (1.0 - t) * p[i - 1][j].x + t * p[i - 1][j + 1].x,
                (1.0 - t) * p[i - 1][j].y + t * p[i - 1][j + 1].y,
            );
        }
    }

    if let Some(left) = left {
        for (j, l) in left.iter_mut().enumerate().take(degree + 1) {
            *l = p[j][0];
        }
    }

    if let Some(right) = right {
        for (j, r) in right.iter_mut().enumerate().take(degree + 1) {
            *r = p[degree - j][j];
        }
    }

    p[degree][0]
}

// ------------------------------------------------------------------------
// Min / max / average families (ports of the `ElkMath` varargs helpers).

pub fn maxi(values: &[i32]) -> i32 {
    values.iter().copied().fold(i32::MIN, i32::max)
}

pub fn mini(values: &[i32]) -> i32 {
    values.iter().copied().fold(i32::MAX, i32::min)
}

pub fn averagel(values: &[i64]) -> i64 {
    values.iter().sum::<i64>() / values.len() as i64
}

pub fn maxf(values: &[f32]) -> f32 {
    values.iter().copied().fold(-f32::MAX, |m, v| if v > m { v } else { m })
}

pub fn minf(values: &[f32]) -> f32 {
    values.iter().copied().fold(f32::MAX, |m, v| if v < m { v } else { m })
}

pub fn averagef(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len() as f32
}

pub fn maxd(values: &[f64]) -> f64 {
    values.iter().copied().fold(-f64::MAX, |m, v| if v > m { v } else { m })
}

pub fn mind(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::MAX, |m, v| if v < m { v } else { m })
}

pub fn averaged(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

pub fn rect_contains_point(rect: &ElkRectangle, p: KVector) -> bool {
    let min_x = rect.x;
    let max_x = rect.x + rect.width;
    let min_y = rect.y;
    let max_y = rect.y + rect.height;
    (p.x > min_x && p.x < max_x) && (p.y > min_y && p.y < max_y)
}

pub fn rect_contains_line(rect: &ElkRectangle, p1: KVector, p2: KVector) -> bool {
    rect_contains_point(rect, p1) && rect_contains_point(rect, p2)
}

pub fn rect_contains_path(rect: &ElkRectangle, path: &KVectorChain) -> bool {
    if path.len() < 2 {
        return false;
    }
    let pts = &path.0;
    let first = pts[0];
    let mut p1 = first;
    for &p2 in &pts[1..] {
        if !rect_contains_line(rect, p1, p2) {
            return false;
        }
        p1 = p2;
    }
    if !rect_contains_line(rect, p1, first) {
        return false;
    }
    true
}

pub fn segments_intersect(l11: KVector, l12: KVector, l21: KVector, l22: KVector) -> bool {
    let mut v0 = l12;
    v0.sub(l11);
    let mut v1 = l22;
    v1.sub(l21);
    let (x00, y00) = (l11.x, l11.y);
    let (x10, y10) = (l21.x, l21.y);
    let (x01, y01) = (v0.x, v0.y);
    let (x11, y11) = (v1.x, v1.y);

    let d = x11 * y01 - x01 * y11;
    if fuzzy_equals(0.0, d, DOUBLE_EQ_EPSILON) {
        return false;
    }
    let s = (1.0 / d) * ((x00 - x10) * y01 - (y00 - y10) * x01);
    let t = (1.0 / d) * -(-(x00 - x10) * y11 + (y00 - y10) * x11);

    fuzzy_compare(0.0, s, DOUBLE_EQ_EPSILON) < 0
        && fuzzy_compare(s, 1.0, DOUBLE_EQ_EPSILON) < 0
        && fuzzy_compare(0.0, t, DOUBLE_EQ_EPSILON) < 0
        && fuzzy_compare(t, 1.0, DOUBLE_EQ_EPSILON) < 0
}

fn rect_intersects_line(rect: &ElkRectangle, p1: KVector, p2: KVector) -> bool {
    if rect_contains_line(rect, p1, p2) {
        return false;
    }
    segments_intersect(rect.position(), rect.top_right(), p1, p2)
        || segments_intersect(rect.top_right(), rect.bottom_right(), p1, p2)
        || segments_intersect(rect.bottom_right(), rect.bottom_left(), p1, p2)
        || segments_intersect(rect.bottom_left(), rect.position(), p1, p2)
}

pub fn rect_intersects_path(rect: &ElkRectangle, path: &KVectorChain) -> bool {
    if path.len() < 2 {
        return false;
    }
    let pts = &path.0;
    let first = pts[0];
    let mut p1 = first;
    for &p2 in &pts[1..] {
        if rect_intersects_line(rect, p1, p2) {
            return true;
        }
        p1 = p2;
    }
    rect_intersects_line(rect, p1, first)
}
