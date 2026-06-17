//!
//! Elements and components live in arenas on the [`DCGraph`]; object
//! identity maps to indices.

use crate::alg_common::elkmath;
use crate::graph::math::{ElkRectangle, KVector, KVectorChain};
use crate::graph::properties::PropertyMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DCDirection {
    North,
    East,
    South,
    West,
}

impl DCDirection {
    pub fn is_horizontal(self) -> bool {
        matches!(self, DCDirection::East | DCDirection::West)
    }
}

/// A semi-infinite strip attached to a `DCElement`.
#[derive(Clone, Debug)]
pub struct DCExtension {
    pub direction: DCDirection,
    /// Top-left corner of the extension, relative to the element's bounds.
    pub offset: KVector,
    pub width: f64,
}

impl DCExtension {
    /// The `DCExtension` constructor (the caller adds the result to
    /// the element's extension list).
    pub fn new(
        parent_bounds: &ElkRectangle,
        direction: DCDirection,
        middle_pos: KVector,
        width: f64,
    ) -> Self {
        let mut offset = KVector::new(-parent_bounds.x, -parent_bounds.y);
        offset.add(middle_pos);
        let half_width = width / 2.0;
        if direction.is_horizontal() {
            offset.sub_xy(0.0, half_width);
        } else {
            offset.sub_xy(half_width, 0.0);
        }
        DCExtension { direction, offset, width }
    }
}

/// A polygon (plus extensions).
pub struct DCElement {
    /// Closed polygonal path of this element's shape.
    pub shape: KVectorChain,
    /// Bounding box of `shape`.
    pub bounds: ElkRectangle,
    /// Short hierarchical edges to/from the parent node.
    pub extensions: Vec<DCExtension>,
    /// Index of the owning `DCComponent` (set by `DCComponent::add_element`).
    pub component: usize,
}

impl DCElement {
    pub fn new(poly_path: KVectorChain) -> Self {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for v in poly_path.iter() {
            min_x = f64::min(min_x, v.x);
            min_y = f64::min(min_y, v.y);
            max_x = f64::max(max_x, v.x);
            max_y = f64::max(max_y, v.y);
        }
        DCElement {
            shape: poly_path,
            bounds: ElkRectangle {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            },
            extensions: Vec::new(),
            component: usize::MAX,
        }
    }

    pub fn intersects(&self, rect: &ElkRectangle) -> bool {
        elkmath::rect_intersects_path(rect, &self.shape)
            || elkmath::rect_contains_path(rect, &self.shape)
    }
}

/// A connected component of the `DCGraph`.
pub struct DCComponent {
    /// Offset from the original position, set by the compactor.
    pub offset: KVector,
    /// Indices of the `DCElement`s belonging to this component.
    pub elements: Vec<usize>,
    /// For debugging purposes only.
    pub id: i32,
}

impl DCComponent {
    fn new() -> Self {
        DCComponent { offset: KVector::default(), elements: Vec::new(), id: -1 }
    }

    fn compute(&self, elements: &[DCElement]) -> (KVector, KVector) {
        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for &e in &self.elements {
            let elem = &elements[e];
            let elem_bounds = &elem.bounds;
            min_x = f64::min(min_x, elem_bounds.x);
            max_x = f64::max(max_x, elem_bounds.x + elem_bounds.width);
            min_y = f64::min(min_y, elem_bounds.y);
            max_y = f64::max(max_y, elem_bounds.y + elem_bounds.height);

            for ext in &elem.extensions {
                if ext.direction.is_horizontal() {
                    let min_pos = elem_bounds.y + ext.offset.y;
                    let max_pos = min_pos + ext.width;
                    min_y = f64::min(min_y, min_pos);
                    max_y = f64::max(max_y, max_pos);
                } else {
                    let min_pos = elem_bounds.x + ext.offset.x;
                    let max_pos = min_pos + ext.width;
                    min_x = f64::min(min_x, min_pos);
                    max_x = f64::max(max_x, max_pos);
                }
            }
        }

        let bounds = KVector::new(max_x - min_x, max_y - min_y);
        let min_corner = KVector::new(min_x + self.offset.x, min_y + self.offset.y);
        (bounds, min_corner)
    }

    pub fn dimensions_of_bounding_rectangle(&self, elements: &[DCElement]) -> KVector {
        self.compute(elements).0
    }

    pub fn min_corner(&self, elements: &[DCElement]) -> KVector {
        self.compute(elements).1
    }

    pub fn intersects(&self, rect: &ElkRectangle, elements: &[DCElement]) -> bool {
        for &e in &self.elements {
            if elements[e].intersects(rect) {
                return true;
            }
        }
        false
    }
}

pub struct DCGraph {
    /// Arena of all elements.
    pub elements: Vec<DCElement>,
    /// The connected components (insertion order).
    pub components: Vec<DCComponent>,
    /// Width and height of this graph (after compaction).
    pub dimensions: KVector,
    /// Properties copied from the original graph's parent node.
    pub properties: PropertyMap,
}

impl DCGraph {
    /// The `DCGraph` constructor: each inner list of element indices
    /// becomes one component.
    pub fn new(elements: Vec<DCElement>, components: Vec<Vec<usize>>) -> Self {
        let mut graph = DCGraph {
            elements,
            components: Vec::new(),
            dimensions: KVector::default(),
            properties: PropertyMap::new(),
        };
        for elems in components {
            let comp_idx = graph.components.len();
            let mut component = DCComponent::new();
            for e in elems {
                component.elements.push(e);
                graph.elements[e].component = comp_idx;
            }
            graph.components.push(component);
        }
        graph
    }
}
