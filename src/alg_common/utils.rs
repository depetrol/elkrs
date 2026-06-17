
use crate::graph::math::{ElkRectangle, KVector};

/// The factor by which the line connecting the two
/// rectangle centers has to be stretched so that the rectangles just touch.
pub fn overlap(r1: &ElkRectangle, r2: &ElkRectangle) -> f64 {
    let horizontal_overlap = f64::min(
        (r1.x - (r2.x + r2.width)).abs(),
        (r1.x + r1.width - r2.x).abs(),
    );
    let vertical_overlap = f64::min(
        (r1.y - (r2.y + r2.height)).abs(),
        (r1.y + r1.height - r2.y).abs(),
    );
    let horizontal_center_distance =
        ((r1.x + r1.width / 2.0) - (r2.x + r2.width / 2.0)).abs();
    if horizontal_center_distance > r1.width / 2.0 + r2.width / 2.0 {
        return 1.0;
    }
    let vertical_center_distance =
        ((r1.y + r1.height / 2.0) - (r2.y + r2.height / 2.0)).abs();
    if vertical_center_distance > r1.height / 2.0 + r2.height / 2.0 {
        return 1.0;
    }
    if horizontal_center_distance == 0.0 && vertical_center_distance == 0.0 {
        return 0.0;
    }
    if horizontal_center_distance == 0.0 {
        return vertical_overlap / vertical_center_distance + 1.0;
    }
    if vertical_center_distance == 0.0 {
        return horizontal_overlap / horizontal_center_distance + 1.0;
    }
    f64::min(
        horizontal_overlap / horizontal_center_distance,
        vertical_overlap / vertical_center_distance,
    ) + 1.0
}

/// The four edges of a rectangle.
pub fn get_rect_edges(r: &ElkRectangle) -> [(KVector, KVector); 4] {
    [
        (r.position(), r.top_right()),
        (r.position(), r.bottom_left()),
        (r.bottom_right(), r.top_right()),
        (r.bottom_right(), r.bottom_left()),
    ]
}
