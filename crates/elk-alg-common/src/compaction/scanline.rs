
use std::cmp::Ordering;

use super::compare_fuzzy;
use super::double_compare;
use super::one_dimensional_compactor::OneDimensionalCompactor;
use super::CNodeId;

/// Entry point used by the base scanline algorithm: a single sweep over all
/// nodes.
pub fn scanline_constraints(compactor: &mut OneDimensionalCompactor) {
    sweep(compactor, |_, _| true);
}

/// A timestamp in the sense of the scanline algorithm.
struct Timestamp {
    low: bool,
    node: CNodeId,
}

/// Executes a single sweep of the scanline, using CNodes that fulfill
/// `filter_fun`. Exposed so the layered edge-aware subclass can re-run sweeps.
pub fn sweep<F>(compactor: &mut OneDimensionalCompactor, filter_fun: F)
where
    F: Fn(&OneDimensionalCompactor, CNodeId) -> bool,
{
    // add all nodes twice (lower and upper border)
    let mut points: Vec<Timestamp> = Vec::new();
    for n in 0..compactor.cgraph.cnodes.len() {
        if filter_fun(compactor, n) {
            points.push(Timestamp { node: n, low: true });
            points.push(Timestamp { node: n, low: false });
        }
    }

    // reset internal state: assign ids
    let mut index = 0i32;
    for n in &mut compactor.cgraph.cnodes {
        n.id = index;
        index += 1;
    }
    let mut cand: Vec<i32> = vec![-1; compactor.cgraph.cnodes.len()];
    let mut intervals = IntervalSet::new();

    // sort the points (stable, matching Collections.sort)
    sort_points(compactor, &mut points);

    // execute the scanline
    for p in &points {
        if p.low {
            insert(compactor, &mut intervals, &mut cand, p.node);
        } else {
            delete(compactor, &mut intervals, &mut cand, p.node);
        }
    }
}

/// Comparator for timestamps: by the chosen y-coordinate; if equal, sort
/// "high" (border = y+height) before "low" (border = y).
fn sort_points(compactor: &OneDimensionalCompactor, points: &mut [Timestamp]) {
    // Collections.sort is stable; Rust's sort_by is stable.
    points.sort_by(|p1, p2| {
        let hb1 = compactor.cgraph.cnodes[p1.node].hitbox;
        let hb2 = compactor.cgraph.cnodes[p2.node].hitbox;
        let y1 = if p1.low { hb1.y } else { hb1.y + hb1.height };
        let y2 = if p2.low { hb2.y } else { hb2.y + hb2.height };
        let cmp = double_compare(y1, y2);
        if cmp == Ordering::Equal {
            if !p1.low && p2.low {
                return Ordering::Less;
            } else if !p2.low && p1.low {
                return Ordering::Greater;
            }
        }
        cmp
    });
}

fn insert(
    compactor: &mut OneDimensionalCompactor,
    intervals: &mut IntervalSet,
    cand: &mut [i32],
    node: CNodeId,
) {
    let success = intervals.add(compactor, node);
    if !success {
        panic!("Invalid hitboxes for scanline constraint calculation.");
    }

    // (Overlap here is non-fatal; omit.)

    let node_id = compactor.cgraph.cnodes[node].id as usize;
    cand[node_id] = match intervals.lower(node) {
        Some(l) => l as i32,
        None => -1,
    };

    if let Some(right) = intervals.higher(node) {
        let right_id = compactor.cgraph.cnodes[right].id as usize;
        cand[right_id] = node as i32;
    }
}

fn delete(
    compactor: &mut OneDimensionalCompactor,
    intervals: &mut IntervalSet,
    cand: &mut [i32],
    node: CNodeId,
) {
    let node_id = compactor.cgraph.cnodes[node].id as usize;

    if let Some(left) = intervals.lower(node) {
        if cand[node_id] == left as i32 {
            // different groups?
            let lg = compactor.cgraph.cnodes[left].cgroup;
            let ng = compactor.cgraph.cnodes[node].cgroup;
            if lg.is_some() && lg != ng {
                compactor.cgraph.cnodes[left].constraints.push(node);
            }
        }
    }

    if let Some(right) = intervals.higher(node) {
        let right_id = compactor.cgraph.cnodes[right].id as usize;
        if cand[right_id] == node as i32 {
            let rg = compactor.cgraph.cnodes[right].cgroup;
            let ng = compactor.cgraph.cnodes[node].cgroup;
            if rg.is_some() && rg != ng {
                compactor.cgraph.cnodes[node].constraints.push(right);
            }
        }
    }

    intervals.remove(compactor, node);
}

/// A sorted set of CNodes, ordered by `hitbox.x + hitbox.width/2`, replicating
/// the relevant `TreeSet` operations (add/remove + lower/higher/floor/ceiling).
struct IntervalSet {
    /// CNodeIds kept sorted by the interval key.
    items: Vec<CNodeId>,
}

impl IntervalSet {
    fn new() -> Self {
        IntervalSet { items: Vec::new() }
    }

    fn key(compactor: &OneDimensionalCompactor, node: CNodeId) -> f64 {
        let hb = compactor.cgraph.cnodes[node].hitbox;
        hb.x + hb.width / 2.0
    }

    #[allow(dead_code)]
    fn cmp(compactor: &OneDimensionalCompactor, a: CNodeId, b: CNodeId) -> Ordering {
        double_compare(Self::key(compactor, a), Self::key(compactor, b))
    }

    /// Returns the index where `node`'s key would be located, and whether an
    /// element comparing equal already exists.
    fn locate(&self, compactor: &OneDimensionalCompactor, node: CNodeId) -> (usize, bool) {
        let key = Self::key(compactor, node);
        // binary search by key
        let mut lo = 0usize;
        let mut hi = self.items.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let mk = Self::key(compactor, self.items[mid]);
            match double_compare(mk, key) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return (mid, true),
            }
        }
        (lo, false)
    }

    /// Adds an element; returns false if an equal element already exists.
    fn add(&mut self, compactor: &OneDimensionalCompactor, node: CNodeId) -> bool {
        let (idx, exists) = self.locate(compactor, node);
        if exists {
            return false;
        }
        self.items.insert(idx, node);
        true
    }

    fn remove(&mut self, compactor: &OneDimensionalCompactor, node: CNodeId) {
        let (idx, exists) = self.locate(compactor, node);
        if exists {
            // the located index has an equal key; ensure it is the node
            if self.items[idx] == node {
                self.items.remove(idx);
            } else {
                // fall back to identity search
                if let Some(pos) = self.items.iter().position(|&n| n == node) {
                    self.items.remove(pos);
                }
            }
        } else if let Some(pos) = self.items.iter().position(|&n| n == node) {
            self.items.remove(pos);
        }
    }

    /// Greatest element strictly less than `node`.
    fn lower(&self, node: CNodeId) -> Option<CNodeId> {
        let pos = self.items.iter().position(|&n| n == node)?;
        if pos == 0 {
            None
        } else {
            Some(self.items[pos - 1])
        }
    }

    /// Least element strictly greater than `node`.
    fn higher(&self, node: CNodeId) -> Option<CNodeId> {
        let pos = self.items.iter().position(|&n| n == node)?;
        if pos + 1 < self.items.len() {
            Some(self.items[pos + 1])
        } else {
            None
        }
    }
}

/// `overlap` predicate — kept for completeness / potential debugging.
#[allow(dead_code)]
fn overlap(compactor: &OneDimensionalCompactor, n1: Option<CNodeId>, n2: Option<CNodeId>) -> bool {
    match (n1, n2) {
        (Some(a), Some(b)) if a != b => {
            let h1 = compactor.cgraph.cnodes[a].hitbox;
            let h2 = compactor.cgraph.cnodes[b].hitbox;
            compare_fuzzy::le(h1.x, h2.x + h2.width) && compare_fuzzy::le(h2.x, h1.x + h1.width)
        }
        _ => false,
    }
}
