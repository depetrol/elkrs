
use crate::core::options::Direction;

use super::{
    set_add, CGraph, CGroupId, CNodeId, DefaultSpacingsHandler, LockFun, SpacingsHandler,
};

/// A constraint calculation algorithm.
/// Implemented for trait objects so subclasses (e.g. the edge-aware scanline)
/// can hold their own state.
pub trait ConstraintCalculationAlgorithm {
    fn calculate_constraints(&mut self, compactor: &mut OneDimensionalCompactor);
}

/// A compaction algorithm.
pub trait CompactionAlgorithm {
    fn compact(&mut self, compactor: &mut OneDimensionalCompactor);
}

/// Built-in longest-path compaction.
pub struct LongestPathCompaction;
impl CompactionAlgorithm for LongestPathCompaction {
    fn compact(&mut self, compactor: &mut OneDimensionalCompactor) {
        super::longest_path::longest_path_compact(compactor);
    }
}

/// Built-in quadratic constraint calculation.
pub struct QuadraticConstraintCalculation;
impl ConstraintCalculationAlgorithm for QuadraticConstraintCalculation {
    fn calculate_constraints(&mut self, compactor: &mut OneDimensionalCompactor) {
        super::quadratic::quadratic_constraints(compactor);
    }
}

/// Built-in scanline constraint calculation.
pub struct ScanlineConstraintCalculator;
impl ConstraintCalculationAlgorithm for ScanlineConstraintCalculator {
    fn calculate_constraints(&mut self, compactor: &mut OneDimensionalCompactor) {
        super::scanline::scanline_constraints(compactor);
    }
}

/// Implements the compaction of a [`CGraph`].
pub struct OneDimensionalCompactor {
    pub cgraph: CGraph,
    pub lock_fun: Option<LockFun<'static>>,
    pub spacings_handler: Box<dyn SpacingsHandler>,
    pub direction: Direction,
    finished: bool,
    constraint_algorithm: Box<dyn ConstraintCalculationAlgorithm>,
    compaction_algorithm: Box<dyn CompactionAlgorithm>,
}

impl OneDimensionalCompactor {
    /// Constructor.
    pub fn new(cgraph: CGraph) -> Self {
        let mut odc = OneDimensionalCompactor {
            cgraph,
            lock_fun: None,
            spacings_handler: Box::new(DefaultSpacingsHandler),
            direction: Direction::UNDEFINED,
            finished: false,
            constraint_algorithm: Box::new(ScanlineConstraintCalculator),
            compaction_algorithm: Box::new(LongestPathCompaction),
        };

        // deduce group offsets for any pre-specified groups
        odc.calculate_group_offsets();

        // wrap any plain CNodes into a CGroup; remember pre-compaction hitboxes
        let n = odc.cgraph.cnodes.len();
        for i in 0..n {
            if odc.cgraph.cnodes[i].cgroup.is_none() {
                odc.cgraph.add_cgroup_with(&[i], None);
            }
            odc.cgraph.cnodes[i].hitbox_pre_compaction = odc.cgraph.cnodes[i].hitbox;
        }

        odc
    }

    pub fn set_spacings_handler(&mut self, handler: Box<dyn SpacingsHandler>) -> &mut Self {
        self.spacings_handler = handler;
        self
    }

    pub fn set_compaction_algorithm(
        &mut self,
        compactor: Box<dyn CompactionAlgorithm>,
    ) -> &mut Self {
        self.compaction_algorithm = compactor;
        self
    }

    pub fn set_constraint_algorithm(
        &mut self,
        algo: Box<dyn ConstraintCalculationAlgorithm>,
    ) -> &mut Self {
        self.constraint_algorithm = algo;
        self
    }

    pub fn set_lock_function(&mut self, fun: Option<LockFun<'static>>) -> &mut Self {
        self.lock_fun = fun;
        self
    }

    /// Compacts the graph in the specified direction.
    pub fn compact(&mut self) -> &mut Self {
        if self.finished {
            panic!("The OneDimensionalCompactor instance has been finished already.");
        }
        if self.direction == Direction::UNDEFINED {
            self.change_direction(Direction::LEFT);
        }
        // reset initial outDegree value for groups
        for g in &mut self.cgraph.cgroups {
            g.out_degree = g.out_degree_real;
        }
        // reset nodes' positions
        for n in &mut self.cgraph.cnodes {
            n.start_pos = f64::NEG_INFINITY;
        }

        // perform the actual compaction (take the algorithm out to avoid a
        // double mutable borrow, then put it back)
        let mut algo = std::mem::replace(
            &mut self.compaction_algorithm,
            Box::new(LongestPathCompaction),
        );
        algo.compact(self);
        self.compaction_algorithm = algo;

        self
    }

    /// Runs the preamble of [`compact`](Self::compact) — defaulting the
    /// direction (which calculates the constraints) and resetting the per-group
    /// `out_degree`/per-node `start_pos` — but does *not* run a compaction
    /// algorithm. Used by callers that need to drive a compaction algorithm
    /// externally (e.g. the layered network-simplex compaction, which requires
    /// access to the surrounding LGraph).
    pub fn prepare_external_compaction(&mut self) -> &mut Self {
        if self.finished {
            panic!("The OneDimensionalCompactor instance has been finished already.");
        }
        if self.direction == Direction::UNDEFINED {
            self.change_direction(Direction::LEFT);
        }
        for g in &mut self.cgraph.cgroups {
            g.out_degree = g.out_degree_real;
        }
        for n in &mut self.cgraph.cnodes {
            n.start_pos = f64::NEG_INFINITY;
        }
        self
    }

    /// Indicate that the compaction is finished. The direction is changed back
    /// to LEFT.
    pub fn finish(&mut self) -> &mut Self {
        self.change_direction(Direction::LEFT);
        self.finished = true;
        self
    }

    /// Changes the direction for compaction by transforming the hitboxes.
    pub fn change_direction(&mut self, dir: Direction) -> &mut Self {
        if self.finished {
            panic!("The OneDimensionalCompactor instance has been finished already.");
        }
        if !self.cgraph.supports(dir) {
            panic!("The direction {dir:?} is not supported by the CGraph instance.");
        }
        if dir == self.direction {
            return self;
        }

        let old_direction = self.direction;
        self.direction = dir;

        use Direction::*;
        match old_direction {
            UNDEFINED => match dir {
                LEFT => self.calculate_constraints(),
                RIGHT => {
                    self.mirror_hitboxes();
                    self.calculate_constraints();
                }
                UP => {
                    self.transpose_hitboxes();
                    self.calculate_constraints();
                }
                DOWN => {
                    self.transpose_hitboxes();
                    self.mirror_hitboxes();
                    self.calculate_constraints();
                }
                _ => {}
            },
            LEFT => match dir {
                RIGHT => {
                    self.mirror_hitboxes();
                    self.reverse_constraints();
                }
                UP => {
                    self.transpose_hitboxes();
                    self.calculate_constraints();
                }
                DOWN => {
                    self.transpose_hitboxes();
                    self.mirror_hitboxes();
                    self.calculate_constraints();
                }
                _ => {}
            },
            RIGHT => match dir {
                LEFT => {
                    self.mirror_hitboxes();
                    self.reverse_constraints();
                }
                UP => {
                    self.mirror_hitboxes();
                    self.transpose_hitboxes();
                    self.calculate_constraints();
                }
                DOWN => {
                    self.mirror_hitboxes();
                    self.transpose_hitboxes();
                    self.mirror_hitboxes();
                    self.calculate_constraints();
                }
                _ => {}
            },
            UP => match dir {
                LEFT => {
                    self.transpose_hitboxes();
                    self.calculate_constraints();
                }
                RIGHT => {
                    self.transpose_hitboxes();
                    self.mirror_hitboxes();
                    self.calculate_constraints();
                }
                DOWN => {
                    self.mirror_hitboxes();
                    self.reverse_constraints();
                }
                _ => {}
            },
            DOWN => match dir {
                LEFT => {
                    self.mirror_hitboxes();
                    self.transpose_hitboxes();
                    self.calculate_constraints();
                }
                RIGHT => {
                    self.mirror_hitboxes();
                    self.transpose_hitboxes();
                    self.mirror_hitboxes();
                    self.calculate_constraints();
                }
                UP => {
                    self.mirror_hitboxes();
                    self.reverse_constraints();
                }
                _ => {}
            },
        }

        self
    }

    /// Whether the given node is locked in the given direction.
    pub fn is_locked_node(&self, node: CNodeId, dir: Direction) -> bool {
        if let Some(f) = &self.lock_fun {
            return f(&self.cgraph, node, dir);
        }
        false
    }

    /// Whether the given group is locked in the given direction.
    pub fn is_locked_group(&self, group: CGroupId, dir: Direction) -> bool {
        for &n in &self.cgraph.cgroups[group].cnodes {
            if self.is_locked_node(n, dir) {
                return true;
            }
        }
        false
    }

    pub fn force_constraints_recalculation(&mut self) -> &mut Self {
        self.calculate_constraints();
        self
    }

    /// Calculates the offsets of all groups.
    pub fn calculate_group_offsets(&mut self) -> &mut Self {
        let group_count = self.cgraph.cgroups.len();
        for g in 0..group_count {
            self.cgraph.cgroups[g].reference = None;

            // find left-most element
            let cnodes = self.cgraph.cgroups[g].cnodes.clone();
            for &n in &cnodes {
                self.cgraph.cnodes[n].cgroup_offset.reset();
                let reference = self.cgraph.cgroups[g].reference;
                let take = match reference {
                    None => true,
                    Some(r) => self.cgraph.cnodes[n].hitbox.x < self.cgraph.cnodes[r].hitbox.x,
                };
                if take {
                    self.cgraph.cgroups[g].reference = Some(n);
                }
            }

            // calculate offsets
            let reference = self.cgraph.cgroups[g].reference.unwrap();
            let ref_hb = self.cgraph.cnodes[reference].hitbox;
            for &n in &cnodes {
                self.cgraph.cnodes[n].cgroup_offset.x = self.cgraph.cnodes[n].hitbox.x - ref_hb.x;
                self.cgraph.cnodes[n].cgroup_offset.y = self.cgraph.cnodes[n].hitbox.y - ref_hb.y;
            }
        }
        self
    }

    // ----------------------------- private --------------------------------

    fn mirror_hitboxes(&mut self) {
        for n in &mut self.cgraph.cnodes {
            n.hitbox.x = -n.hitbox.x - n.hitbox.width;
        }
        self.calculate_group_offsets();
    }

    fn transpose_hitboxes(&mut self) {
        for n in &mut self.cgraph.cnodes {
            std::mem::swap(&mut n.hitbox.x, &mut n.hitbox.y);
            std::mem::swap(&mut n.hitbox.width, &mut n.hitbox.height);
            std::mem::swap(&mut n.cgroup_offset.x, &mut n.cgroup_offset.y);
        }
        self.calculate_group_offsets();
    }

    pub(crate) fn calculate_constraints(&mut self) {
        // resetting constraints
        for n in &mut self.cgraph.cnodes {
            n.constraints.clear();
        }

        // apply any precalculated constraints
        let cstrs = if self.direction.is_horizontal() {
            self.cgraph.predefined_horizontal_constraints.clone()
        } else {
            self.cgraph.predefined_vertical_constraints.clone()
        };
        for (first, second) in cstrs {
            if self.direction == Direction::LEFT || self.direction == Direction::UP {
                self.cgraph.cnodes[first].constraints.push(second);
            } else {
                self.cgraph.cnodes[second].constraints.push(first);
            }
        }

        // run the specified constraint calculation algorithm
        let mut algo = std::mem::replace(
            &mut self.constraint_algorithm,
            Box::new(ScanlineConstraintCalculator),
        );
        algo.calculate_constraints(self);
        self.constraint_algorithm = algo;

        // update the "external" constraints of the groups
        self.calculate_constraints_for_cgroups();
    }

    fn calculate_constraints_for_cgroups(&mut self) {
        for g in &mut self.cgraph.cgroups {
            g.out_degree = 0;
            g.out_degree_real = 0;
            g.incoming_constraints.clear();
        }

        let group_count = self.cgraph.cgroups.len();
        for group in 0..group_count {
            let cnodes = self.cgraph.cgroups[group].cnodes.clone();
            for cnode in cnodes {
                let constraints = self.cgraph.cnodes[cnode].constraints.clone();
                for inc in constraints {
                    let inc_group = self.cgraph.cnodes[inc].cgroup.unwrap();
                    if inc_group != group {
                        set_add(&mut self.cgraph.cgroups[group].incoming_constraints, inc);
                        self.cgraph.cgroups[inc_group].out_degree += 1;
                        self.cgraph.cgroups[inc_group].out_degree_real += 1;
                    }
                }
            }
        }
    }

    fn reverse_constraints(&mut self) {
        let n = self.cgraph.cnodes.len();
        let mut inc_map: Vec<Vec<CNodeId>> = vec![Vec::new(); n];

        for cnode in 0..n {
            self.cgraph.cnodes[cnode].start_pos = f64::NEG_INFINITY;
            let constraints = self.cgraph.cnodes[cnode].constraints.clone();
            for inc in constraints {
                inc_map[inc].push(cnode);
            }
        }

        for cnode in 0..n {
            self.cgraph.cnodes[cnode].constraints = std::mem::take(&mut inc_map[cnode]);
        }

        self.calculate_constraints_for_cgroups();
    }
}
