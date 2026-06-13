//! Ports of the parts of `org.eclipse.elk.core.math.ElkMath` and Guava's
//! `DoubleMath` that the spore and disco algorithms rely on.

use elk_graph::math::{ElkRectangle, KVector, KVectorChain};

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

/// Port of `ElkMath.intersects2`: intersection point of segments `p + t*r`
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

/// Port of `ElkMath.traceRays`.
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

/// Port of `ElkMath.distance(a1, a2, b1, b2, v)`: direction-dependent
/// distance between two line segments.
pub fn distance_segments(a1: KVector, a2: KVector, b1: KVector, b2: KVector, v: KVector) -> f64 {
    let mut neg_v = v;
    neg_v.negate();
    f64::min(
        trace_rays(a1, a2, b1, b2, v),
        trace_rays(b1, b2, a1, a2, neg_v),
    )
}

/// Port of `ElkMath.shortestDistance(ElkRectangle, ElkRectangle)`.
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

/// Port of `ElkMath.clipVector`.
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

/// Port of `ElkMath.contains(ElkRectangle, KVector)` (strict interior).
fn rect_contains_point(rect: &ElkRectangle, p: KVector) -> bool {
    let min_x = rect.x;
    let max_x = rect.x + rect.width;
    let min_y = rect.y;
    let max_y = rect.y + rect.height;
    (p.x > min_x && p.x < max_x) && (p.y > min_y && p.y < max_y)
}

/// Port of `ElkMath.contains(ElkRectangle, KVector, KVector)`.
fn rect_contains_line(rect: &ElkRectangle, p1: KVector, p2: KVector) -> bool {
    rect_contains_point(rect, p1) && rect_contains_point(rect, p2)
}

/// Port of `ElkMath.contains(ElkRectangle, KVectorChain)` (closed path).
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

/// Port of `ElkMath.intersects(KVector, KVector, KVector, KVector)`
/// (fuzzy segment intersection; touching is not intersecting).
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

/// Port of `ElkMath.intersects(ElkRectangle, KVector, KVector)`.
fn rect_intersects_line(rect: &ElkRectangle, p1: KVector, p2: KVector) -> bool {
    if rect_contains_line(rect, p1, p2) {
        return false;
    }
    segments_intersect(rect.position(), rect.top_right(), p1, p2)
        || segments_intersect(rect.top_right(), rect.bottom_right(), p1, p2)
        || segments_intersect(rect.bottom_right(), rect.bottom_left(), p1, p2)
        || segments_intersect(rect.bottom_left(), rect.position(), p1, p2)
}

/// Port of `ElkMath.intersects(ElkRectangle, KVectorChain)` (closed path).
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
