//!
//! The object graph (CNode <-> CGroup <-> CGraph with bidirectional
//! references) is modelled here with index-based arenas held inside [`CGraph`].

use std::cmp::Ordering;

use crate::core::options::Direction;
use crate::graph::math::{ElkRectangle, KVector};

pub mod longest_path;
pub mod one_dimensional_compactor;
pub mod quadratic;
pub mod scanline;

pub use longest_path::longest_path_compact;
pub use one_dimensional_compactor::OneDimensionalCompactor;
pub use quadratic::quadratic_constraints;
pub use scanline::scanline_constraints;

/// Index of a [`CNode`] within a [`CGraph`].
pub type CNodeId = usize;
/// Index of a [`CGroup`] within a [`CGraph`].
pub type CGroupId = usize;

/// Tolerance-affected double comparisons.
pub mod compare_fuzzy {
    pub const TOLERANCE: f64 = 0.0001;

    /// Guava `DoubleMath.fuzzyEquals`.
    pub fn fuzzy_equals(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance || a == b || (a.is_nan() && b.is_nan())
    }

    /// Guava `DoubleMath.fuzzyCompare`.
    pub fn fuzzy_compare(a: f64, b: f64, tolerance: f64) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        if fuzzy_equals(a, b, tolerance) {
            Ordering::Equal
        } else if a < b {
            Ordering::Less
        } else if a > b {
            Ordering::Greater
        } else {
            a.is_nan().cmp(&b.is_nan())
        }
    }

    pub fn eq(d1: f64, d2: f64) -> bool {
        fuzzy_equals(d1, d2, TOLERANCE)
    }
    pub fn gt(d1: f64, d2: f64) -> bool {
        fuzzy_compare(d1, d2, TOLERANCE).is_gt()
    }
    pub fn lt(d1: f64, d2: f64) -> bool {
        fuzzy_compare(d1, d2, TOLERANCE).is_lt()
    }
    pub fn ge(d1: f64, d2: f64) -> bool {
        fuzzy_compare(d1, d2, TOLERANCE).is_ge()
    }
    pub fn le(d1: f64, d2: f64) -> bool {
        fuzzy_compare(d1, d2, TOLERANCE).is_le()
    }
}

/// Opaque origin of a [`CNode`], set by the caller.
/// The compaction core does not interpret these; the layered transformer
/// distinguishes nodes from vertical segments via the variants.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CNodeOrigin {
    #[default]
    None,
    /// An `LNode` (index into the layered graph arena).
    LNode(u32),
    /// A `VerticalSegment` (index into the layered transformer's segment vec).
    VerticalSegment(u32),
}

/// A 4-tuple used as a 'compaction lock'.
#[derive(Clone, Copy, Default, Debug)]
pub struct Quadruplet {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
}

impl Quadruplet {
    pub fn new() -> Self {
        Quadruplet::default()
    }

    /// Sets all four flags at once.
    pub fn set_all(&mut self, l: bool, r: bool, u: bool, d: bool) {
        self.left = l;
        self.right = r;
        self.up = u;
        self.down = d;
    }

    pub fn apply_or(&mut self, other: &Quadruplet) {
        self.left |= other.left;
        self.right |= other.right;
        self.up |= other.up;
        self.down |= other.down;
    }

    pub fn set_dir(&mut self, value: bool, direction: Direction) {
        match direction {
            Direction::LEFT => self.left = value,
            Direction::RIGHT => self.right = value,
            Direction::UP => self.up = value,
            Direction::DOWN => self.down = value,
            Direction::UNDEFINED => {}
        }
    }

    pub fn get(&self, direction: Direction) -> bool {
        match direction {
            Direction::LEFT => self.left,
            Direction::RIGHT => self.right,
            Direction::UP => self.up,
            Direction::DOWN => self.down,
            Direction::UNDEFINED => false,
        }
    }
}

/// Representation of a node/box in the constraint graph.
#[derive(Clone, Debug)]
pub struct CNode {
    pub id: i32,
    pub origin: CNodeOrigin,
    pub type_: Option<String>,
    pub cgroup: Option<CGroupId>,
    pub cgroup_offset: KVector,
    pub hitbox_pre_compaction: ElkRectangle,
    pub hitbox: ElkRectangle,
    pub constraints: Vec<CNodeId>,
    pub start_pos: f64,
}

impl Default for CNode {
    fn default() -> Self {
        CNode {
            id: 0,
            origin: CNodeOrigin::None,
            type_: None,
            cgroup: None,
            cgroup_offset: KVector::default(),
            hitbox_pre_compaction: ElkRectangle::default(),
            hitbox: ElkRectangle::default(),
            constraints: Vec::new(),
            start_pos: f64::NEG_INFINITY,
        }
    }
}

/// A group of [`CNode`]s whose relative distances are preserved.
#[derive(Clone, Debug)]
pub struct CGroup {
    pub id: i32,
    pub master: Option<CNodeId>,
    /// Insertion order is kept in a `Vec`.
    pub cnodes: Vec<CNodeId>,
    pub start_pos: f64,
    /// Insertion-ordered `Vec` (membership-checked).
    pub incoming_constraints: Vec<CNodeId>,
    pub out_degree: i32,
    pub out_degree_real: i32,
    pub reference: Option<CNodeId>,
    pub delta: f64,
    pub delta_normalized: f64,
}

impl Default for CGroup {
    fn default() -> Self {
        CGroup {
            id: 0,
            master: None,
            cnodes: Vec::new(),
            start_pos: f64::NEG_INFINITY,
            incoming_constraints: Vec::new(),
            out_degree: 0,
            out_degree_real: 0,
            reference: None,
            delta: 0.0,
            delta_normalized: 0.0,
        }
    }
}

/// Representation of a constraint graph.
#[derive(Clone, Debug, Default)]
pub struct CGraph {
    pub cnodes: Vec<CNode>,
    pub cgroups: Vec<CGroup>,
    supported_directions: Vec<Direction>,
    pub predefined_horizontal_constraints: Vec<(CNodeId, CNodeId)>,
    pub predefined_vertical_constraints: Vec<(CNodeId, CNodeId)>,
}

impl CGraph {
    pub fn new(supported_directions: Vec<Direction>) -> Self {
        CGraph { supported_directions, ..Default::default() }
    }

    pub fn supports(&self, direction: Direction) -> bool {
        self.supported_directions.contains(&direction)
    }

    // ---- node/group creation ----

    /// Creates a new [`CNode`] and appends it; returns its id.
    pub fn add_cnode(&mut self, node: CNode) -> CNodeId {
        let id = self.cnodes.len();
        self.cnodes.push(node);
        id
    }

    /// Wraps the given nodes in a fresh group. Returns the group id.
    pub fn add_cgroup_with(&mut self, nodes: &[CNodeId], master: Option<CNodeId>) -> CGroupId {
        let gid = self.cgroups.len();
        self.cgroups.push(CGroup { master, ..Default::default() });
        for &n in nodes {
            self.group_add_cnode(gid, n);
        }
        gid
    }

    /// Adds a node to the given group.
    pub fn group_add_cnode(&mut self, group: CGroupId, node: CNodeId) {
        if self.cnodes[node].cgroup.is_some() {
            panic!("CNode belongs to another CGroup.");
        }
        self.cgroups[group].cnodes.push(node);
        self.cnodes[node].cgroup = Some(group);
        if self.cgroups[group].reference.is_none() {
            self.cgroups[group].reference = Some(node);
        }
    }
}

/// Adds `value` to `vec` only if it isn't present already.
pub(crate) fn set_add(vec: &mut Vec<CNodeId>, value: CNodeId) {
    if !vec.contains(&value) {
        vec.push(value);
    }
}

/// A function evaluating whether a node may move in the passed direction.
pub type LockFun<'a> = Box<dyn Fn(&CGraph, CNodeId, Direction) -> bool + 'a>;

/// Reports the spacings between pairs of [`CNode`]s.
pub trait SpacingsHandler {
    fn horizontal_spacing(&self, cgraph: &CGraph, n1: CNodeId, n2: CNodeId) -> f64;
    fn vertical_spacing(&self, cgraph: &CGraph, n1: CNodeId, n2: CNodeId) -> f64;
}

/// Default handler returning no spacing.
pub struct DefaultSpacingsHandler;
impl SpacingsHandler for DefaultSpacingsHandler {
    fn horizontal_spacing(&self, _: &CGraph, _: CNodeId, _: CNodeId) -> f64 {
        0.0
    }
    fn vertical_spacing(&self, _: &CGraph, _: CNodeId, _: CNodeId) -> f64 {
        0.0
    }
}

/// Comparator used by the scanline; exposed for the layered subclass.
pub(crate) fn double_compare(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}
