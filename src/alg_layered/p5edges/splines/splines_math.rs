//! The reachable subset of `SplinesMath` plus the
//! pieces of `ElkMath` used by the spline routing
//! code (bezier approximation, rectangle/line intersection tests).

use crate::core::options::PortSide;
use crate::graph::math::{ElkRectangle, KVector};

/// Differences below this are treated as zero.
const EPSILON: f64 = 0.00000001;

pub const HALF_PI: f64 = std::f64::consts::PI / 2.0;
/// Computed as `HALF_PI + HALF_PI + HALF_PI`.
pub const THREE_HALF_PI: f64 = HALF_PI + HALF_PI + HALF_PI;

/// Converts a `PortSide` to the
/// direction from a node's center to the given side in radians.
pub fn port_side_to_direction(side: PortSide) -> f64 {
    match side {
        PortSide::NORTH => THREE_HALF_PI,
        PortSide::EAST => 0.0,
        PortSide::SOUTH => HALF_PI,
        PortSide::WEST => std::f64::consts::PI,
        _ => 0.0,
    }
}

pub fn is_between(value: f64, boundary0: f64, boundary1: f64) -> bool {
    if (boundary0 - value).abs() < EPSILON || (boundary1 - value).abs() < EPSILON {
        return true;
    }
    if (boundary0 - value) > EPSILON {
        (value - boundary1) > EPSILON
    } else {
        (boundary1 - value) > EPSILON
    }
}

// ---------------------------------------------------------------------------
// ElkMath subset

/// table of precomputed factorial values.
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

/// Float variant, used by `factd` for large inputs.
fn powf(a: f32, b: i32) -> f32 {
    let mut result: f32 = 1.0;
    let mut base = a;
    let mut exp = if b >= 0 { b } else { -b };
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

fn powd(a: f64, b: i32) -> f64 {
    let mut result: f64 = 1.0;
    let mut base = a;
    let mut exp = if b >= 0 { b } else { -b };
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

fn factd(x: i32) -> f64 {
    assert!(x >= 0, "The input must be positive");
    if (x as usize) < FACT_TABLE.len() {
        FACT_TABLE[x as usize] as f64
    } else {
        (2.0 * std::f64::consts::PI * x as f64).sqrt()
            * (powf(x as f32, x) as f64 / powd(std::f64::consts::E, x))
    }
}

fn binomiald(n: i32, k: i32) -> f64 {
    assert!(n >= 0 && k >= 0, "k and n must be positive");
    assert!(k <= n, "k must be smaller than n");
    if k == 0 || k == n {
        1.0
    } else if n == 0 {
        0.0
    } else {
        factd(n) / (factd(k) * factd(n - k))
    }
}

fn get_point_on_bezier_segment(t: f64, control_points: &[KVector]) -> KVector {
    let n = control_points.len() as i32 - 1;
    let mut px = 0.0;
    let mut py = 0.0;
    for j in 0..=n {
        let p = control_points[j as usize];
        let factor = binomiald(n, j) * powd(1.0 - t, n - j) * powd(t, j);
        px += p.x * factor;
        py += p.y * factor;
    }
    KVector::new(px, py)
}

pub fn approximate_bezier_segment(result_size: i32, control_points: &[KVector]) -> Vec<KVector> {
    if result_size <= 0 {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(result_size as usize);
    let dt = 1.0 / result_size as f64;
    let mut t = 0.0;
    for _ in 0..result_size {
        t += dt;
        result.push(get_point_on_bezier_segment(t, control_points));
    }
    result
}

const DOUBLE_EQ_EPSILON: f64 = 0.00001;

/// Guava `DoubleMath.fuzzyEquals`.
fn fuzzy_equals(a: f64, b: f64, tolerance: f64) -> bool {
    f64::copysign(a - b, 1.0) <= tolerance || a == b || (a.is_nan() && b.is_nan())
}

/// Guava `DoubleMath.fuzzyCompare`.
fn fuzzy_compare(a: f64, b: f64, tolerance: f64) -> i32 {
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

/// Line-line intersection test.
fn lines_intersect(l11: KVector, l12: KVector, l21: KVector, l22: KVector) -> bool {
    let u0 = l11;
    let v0 = KVector::new(l12.x - l11.x, l12.y - l11.y);
    let u1 = l21;
    let v1 = KVector::new(l22.x - l21.x, l22.y - l21.y);
    let (x00, y00) = (u0.x, u0.y);
    let (x10, y10) = (u1.x, u1.y);
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

fn rect_contains_point(rect: &ElkRectangle, p: KVector) -> bool {
    let min_x = rect.x;
    let max_x = rect.x + rect.width;
    let min_y = rect.y;
    let max_y = rect.y + rect.height;
    (p.x > min_x && p.x < max_x) && (p.y > min_y && p.y < max_y)
}

pub fn rect_contains_line(rect: &ElkRectangle, p1: KVector, p2: KVector) -> bool {
    rect_contains_point(rect, p1) && rect_contains_point(rect, p2)
}

pub fn rect_intersects_line(rect: &ElkRectangle, p1: KVector, p2: KVector) -> bool {
    // simple cases first: fully contained
    if rect_contains_line(rect, p1, p2) {
        return false;
    }
    let top_left = rect.position();
    let top_right = rect.top_right();
    let bottom_right = rect.bottom_right();
    let bottom_left = rect.bottom_left();
    lines_intersect(top_left, top_right, p1, p2)
        || lines_intersect(top_right, bottom_right, p1, p2)
        || lines_intersect(bottom_right, bottom_left, p1, p2)
        || lines_intersect(bottom_left, top_left, p1, p2)
}
