//! Rust port of
//! `org.eclipse.elk.alg.common.compaction.oned.OneDimensionalCompactorTest`.
//! White-box exercise of the constraint-graph compactor: nodes, groups,
//! per-pair spacing handlers, and every compaction direction.

use elkrs::alg_common::compaction::one_dimensional_compactor::{
    OneDimensionalCompactor, QuadraticConstraintCalculation, ScanlineConstraintCalculator,
};
use elkrs::alg_common::compaction::{CGraph, CNode, CNodeId, SpacingsHandler};
use elkrs::core::options::Direction;
use elkrs::graph::math::ElkRectangle;

const EPS: f64 = 0.0001;
const SPACING: f64 = 5.0;

/// Spacing handler whose value for a pair is the first match (in list order)
/// against either node id — mirroring the test's "order realizes a max"
/// inline handlers. Falls back to the given default.
struct MaxHandler {
    horizontal: Vec<(CNodeId, f64)>,
    vertical: Vec<(CNodeId, f64)>,
    h_default: f64,
    v_default: f64,
}

impl MaxHandler {
    fn pick(list: &[(CNodeId, f64)], default: f64, n1: CNodeId, n2: CNodeId) -> f64 {
        for &(id, s) in list {
            if n1 == id || n2 == id {
                return s;
            }
        }
        default
    }
    /// Uniform `SPACING` in both axes (the test's `TEST_SPACING_HANDLER`).
    fn uniform() -> Box<dyn SpacingsHandler> {
        Box::new(MaxHandler {
            horizontal: vec![],
            vertical: vec![],
            h_default: SPACING,
            v_default: SPACING,
        })
    }
}

impl SpacingsHandler for MaxHandler {
    fn horizontal_spacing(&self, _: &CGraph, n1: CNodeId, n2: CNodeId) -> f64 {
        Self::pick(&self.horizontal, self.h_default, n1, n2)
    }
    fn vertical_spacing(&self, _: &CGraph, n1: CNodeId, n2: CNodeId) -> f64 {
        Self::pick(&self.vertical, self.v_default, n1, n2)
    }
}

fn cgraph() -> CGraph {
    CGraph::new(vec![
        Direction::LEFT,
        Direction::RIGHT,
        Direction::UP,
        Direction::DOWN,
    ])
}

fn node(g: &mut CGraph, x: f64, y: f64, w: f64, h: f64) -> CNodeId {
    g.add_cnode(CNode {
        hitbox: ElkRectangle::new(x, y, w, h),
        ..Default::default()
    })
}

/// Build a quadratic-constraint compactor (the test's `compacter`), optionally
/// swapping in scanline constraints / a spacing handler, run the given
/// direction sequence, finish, and hand the graph back.
fn run(
    graph: CGraph,
    scanline: bool,
    handler: Option<Box<dyn SpacingsHandler>>,
    dirs: &[Direction],
) -> CGraph {
    let mut c = OneDimensionalCompactor::new(graph);
    if scanline {
        c.set_constraint_algorithm(Box::new(ScanlineConstraintCalculator));
    } else {
        c.set_constraint_algorithm(Box::new(QuadraticConstraintCalculation));
    }
    if let Some(h) = handler {
        c.set_spacings_handler(h);
    }
    for &d in dirs {
        c.change_direction(d);
        c.compact();
    }
    c.finish();
    c.cgraph
}

fn x(g: &CGraph, id: CNodeId) -> f64 {
    g.cnodes[id].hitbox.x
}
fn y(g: &CGraph, id: CNodeId) -> f64 {
    g.cnodes[id].hitbox.y
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= EPS, "{a} != {b}");
}

#[test]
fn test_left_compaction() {
    let mut g = cgraph();
    let left = node(&mut g, 0., 0., 20., 20.);
    let right = node(&mut g, 30., 0., 20., 20.);
    let g = run(g, false, None, &[Direction::LEFT]);
    close(x(&g, left), 0.);
    close(x(&g, right), 20.);
}

#[test]
fn test_left_compaction_equal_y_coordinate() {
    let mut g = cgraph();
    let top = node(&mut g, 0., 0., 20., 20.);
    let bot = node(&mut g, 30., 20., 20., 20.);
    let g = run(g, true, None, &[Direction::LEFT]);
    close(x(&g, top), 0.);
    close(x(&g, bot), 0.);
}

#[test]
fn test_left_compaction_spacing_aware() {
    let mut g = cgraph();
    let left = node(&mut g, 0., 0., 20., 20.);
    let right = node(&mut g, 30., 20. + SPACING - 1., 20., 20.);
    let g = run(g, false, Some(MaxHandler::uniform()), &[Direction::LEFT]);
    close(x(&g, left), 0.);
    close(x(&g, right), 25.);
}

#[test]
fn test_left_compaction_spacing_aware2() {
    let mut g = cgraph();
    let left = node(&mut g, 0., 0., 20., 20.);
    let right = node(&mut g, 30., 20. + SPACING + 1., 20., 20.);
    let g = run(g, false, Some(MaxHandler::uniform()), &[Direction::LEFT]);
    close(x(&g, left), 0.);
    close(x(&g, right), 0.);
}

#[test]
fn test_right_compaction() {
    let mut g = cgraph();
    let left = node(&mut g, 0., 0., 20., 20.);
    let right = node(&mut g, 30., 0., 20., 20.);
    let g = run(g, false, None, &[Direction::RIGHT]);
    close(x(&g, left), 10.);
    close(x(&g, right), 30.);
}

#[test]
fn test_up_compaction() {
    let mut g = cgraph();
    let upper = node(&mut g, 0., 0., 20., 20.);
    let lower = node(&mut g, 0., 30., 20., 20.);
    let g = run(g, false, None, &[Direction::UP]);
    close(y(&g, upper), 0.);
    close(y(&g, lower), 20.);
}

#[test]
fn test_down_compaction() {
    let mut g = cgraph();
    let upper = node(&mut g, 0., 0., 20., 20.);
    let lower = node(&mut g, 0., 30., 20., 20.);
    let g = run(g, false, None, &[Direction::DOWN]);
    close(y(&g, upper), 10.);
    close(y(&g, lower), 30.);
}

#[test]
fn test_left_group_compaction() {
    let mut g = cgraph();
    let left = node(&mut g, 0., 0., 20., 20.);
    let upper_right = node(&mut g, 40., 5., 20., 20.);
    let lower_right = node(&mut g, 30., 25., 20., 20.);
    g.add_cgroup_with(&[upper_right, lower_right], None);
    let g = run(g, false, None, &[Direction::LEFT]);
    close(x(&g, left), 0.);
    close(x(&g, upper_right), 20.);
    close(x(&g, lower_right), 10.);
}

#[test]
fn test_right_group_compaction() {
    let mut g = cgraph();
    let left = node(&mut g, 0., 5., 20., 20.);
    let upper_right = node(&mut g, 40., 0., 20., 20.);
    let lower_right = node(&mut g, 10., 25., 20., 20.);
    g.add_cgroup_with(&[left, lower_right], None);
    let g = run(g, false, None, &[Direction::RIGHT]);
    close(x(&g, left), 20.);
    close(x(&g, upper_right), 40.);
    close(x(&g, lower_right), 30.);
}

#[test]
fn test_up_group_compaction() {
    let mut g = cgraph();
    let upper_left = node(&mut g, 0., 0., 20., 20.);
    let lower_left = node(&mut g, 5., 40., 20., 20.);
    let right = node(&mut g, 25., 30., 20., 20.);
    g.add_cgroup_with(&[lower_left, right], None);
    let g = run(g, false, None, &[Direction::UP]);
    close(y(&g, upper_left), 0.);
    close(y(&g, lower_left), 20.);
    close(y(&g, right), 10.);
}

#[test]
fn test_down_group_compaction() {
    let mut g = cgraph();
    let upper_left = node(&mut g, 0., 0., 20., 20.);
    let lower_left = node(&mut g, 5., 40., 10., 20.);
    let right = node(&mut g, 25., 10., 20., 20.);
    g.add_cgroup_with(&[upper_left, right], None);
    let g = run(g, false, None, &[Direction::DOWN]);
    close(y(&g, upper_left), 20.);
    close(y(&g, lower_left), 40.);
    close(y(&g, right), 30.);
}

#[test]
fn test_no_spacing_applied_within_groups() {
    let mut g = cgraph();
    let one = node(&mut g, 0., 0., 20., 20.);
    let two = node(&mut g, 20., 10., 20., 20.);
    let three = node(&mut g, 40., 20., 20., 20.);
    g.add_cgroup_with(&[one, two, three], None);
    let four = node(&mut g, 22., 80., 20., 20.);
    let five = node(&mut g, 42., 90., 20., 20.);
    let six = node(&mut g, 62., 100., 20., 20.);
    g.add_cgroup_with(&[four, five, six], None);
    let g = run(
        g,
        false,
        Some(MaxHandler::uniform()),
        &[Direction::LEFT, Direction::RIGHT, Direction::UP, Direction::DOWN],
    );
    close(x(&g, one), 0.);
    close(x(&g, two), 20.);
    close(x(&g, three), 40.);
    close(x(&g, four), 0.);
    close(x(&g, five), 20.);
    close(x(&g, six), 40.);
    close(y(&g, one), 0.);
    close(y(&g, two), 10.);
    close(y(&g, three), 20.);
    close(y(&g, four), 35.);
    close(y(&g, five), 45.);
    close(y(&g, six), 55.);
}

#[test]
fn test_subsequent_directions_compaction() {
    let dirs = [Direction::LEFT, Direction::RIGHT, Direction::UP, Direction::DOWN];
    let mut g = cgraph();
    let one = node(&mut g, 0., 0., 20., 20.);
    let two = node(&mut g, 25., 0., 20., 20.);
    let three = node(&mut g, 0., 25., 20., 20.);
    let four = node(&mut g, 25., 25., 20., 20.);
    for &d1 in &dirs {
        for &d2 in &dirs {
            for &d3 in &dirs {
                for &d4 in &dirs {
                    g = run(g, false, Some(MaxHandler::uniform()), &[d1, d2, d3, d4]);
                    close(x(&g, one), 0.);
                    close(y(&g, one), 0.);
                    close(x(&g, two), 25.);
                    close(y(&g, two), 0.);
                    close(x(&g, three), 0.);
                    close(y(&g, three), 25.);
                    close(x(&g, four), 25.);
                    close(y(&g, four), 25.);
                }
            }
        }
    }
}

#[test]
fn test_horizontal_spacings() {
    let mut g = cgraph();
    let one = node(&mut g, 0., 0., 20., 20.);
    let two = node(&mut g, 50., 0., 20., 20.);
    let three = node(&mut g, 150., 0., 20., 20.);
    let handler = || -> Box<dyn SpacingsHandler> {
        Box::new(MaxHandler {
            horizontal: vec![(three, 10.), (two, 7.), (one, 5.)],
            vertical: vec![],
            h_default: SPACING,
            v_default: SPACING,
        })
    };
    for dir in [Direction::LEFT, Direction::RIGHT, Direction::LEFT] {
        g = run(g, false, Some(handler()), &[dir]);
        close(x(&g, one), 0.);
        close(x(&g, two), 27.);
        close(x(&g, three), 57.);
    }
}

#[test]
fn test_vertical_spacing_during_horizontal_compaction() {
    let mut g = cgraph();
    let one = node(&mut g, 150., 11., 20., 20.);
    let two = node(&mut g, 0., 40., 20., 20.);
    let three = node(&mut g, 150., 76., 20., 20.);
    let handler: Box<dyn SpacingsHandler> = Box::new(MaxHandler {
        horizontal: vec![],
        vertical: vec![(three, 15.), (one, 10.), (two, 5.)],
        h_default: 0.,
        v_default: 0.,
    });
    let g = run(g, false, Some(handler), &[Direction::LEFT]);
    close(x(&g, one), 20.);
    close(x(&g, two), 0.);
    close(x(&g, three), 0.);
}

#[test]
fn test_vertical_spacings() {
    let mut g = cgraph();
    let one = node(&mut g, 0., 0., 20., 20.);
    let two = node(&mut g, 0., 50., 20., 20.);
    let three = node(&mut g, 0., 150., 20., 20.);
    let handler = || -> Box<dyn SpacingsHandler> {
        Box::new(MaxHandler {
            horizontal: vec![],
            vertical: vec![(three, 10.), (two, 7.), (one, 5.)],
            h_default: 0.,
            v_default: 0.,
        })
    };
    for dir in [Direction::UP, Direction::DOWN, Direction::UP] {
        g = run(g, false, Some(handler()), &[dir]);
        close(y(&g, one), 0.);
        close(y(&g, two), 27.);
        close(y(&g, three), 57.);
    }
}

#[test]
fn test_horizontal_spacing_during_vertical_compaction() {
    let mut g = cgraph();
    let one = node(&mut g, 16., 150., 20., 20.);
    let two = node(&mut g, 40., 0., 20., 20.);
    let three = node(&mut g, 76., 150., 20., 20.);
    let handler: Box<dyn SpacingsHandler> = Box::new(MaxHandler {
        horizontal: vec![(three, 15.), (two, 10.), (one, 5.)],
        vertical: vec![],
        h_default: 0.,
        v_default: 0.,
    });
    let g = run(g, false, Some(handler), &[Direction::UP]);
    close(y(&g, one), 20.);
    close(y(&g, two), 0.);
    close(y(&g, three), 0.);
}
