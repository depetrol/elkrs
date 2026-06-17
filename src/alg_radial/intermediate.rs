
use crate::graph::graph::{ElkGraph, NodeId, ShapeId};
use crate::graph::math::{ElkMargin, ElkPadding, KVector};

use crate::alg_radial::options::{
    self, CHILD_AREA_HEIGHT, CHILD_AREA_WIDTH, MARGINS, NODE_SIZE_FIXED_GRAPH_SIZE, PADDING,
};

/// `Double.MIN_VALUE` (smallest positive double, not the most negative).
const JAVA_DOUBLE_MIN_VALUE: f64 = 4.9406564584124654e-324;

/// Calculate the size of the graph and
/// shift nodes into positive coordinates if necessary.
pub fn calculate_graph_size(g: &mut ElkGraph, graph: NodeId, root: NodeId) {
    // calculate the offset from border spacing and node distribution
    let mut min_x_pos = f64::MAX;
    let mut min_y_pos = f64::MAX;
    let mut max_x_pos = JAVA_DOUBLE_MIN_VALUE;
    let mut max_y_pos = JAVA_DOUBLE_MIN_VALUE;

    for &node in &g.node(graph).children {
        let n = g.node(node);
        let margins: ElkMargin = n.properties.get(&MARGINS);
        min_x_pos = min_x_pos.min(n.shape.x - margins.left);
        min_y_pos = min_y_pos.min(n.shape.y - margins.top);
        max_x_pos = max_x_pos.max(n.shape.x + n.shape.width + margins.right);
        max_y_pos = max_y_pos.max(n.shape.y + n.shape.height + margins.bottom);
    }

    let padding: ElkPadding = g.node(graph).properties.get(&PADDING);
    let mut offset = KVector::new(min_x_pos - padding.left, min_y_pos - padding.top);

    let mut width = max_x_pos - min_x_pos + padding.horizontal();
    let mut height = max_y_pos - min_y_pos + padding.vertical();

    if g.node(graph).properties.get(&options::CENTER_ON_ROOT) {
        let root_node = g.node(root);
        let root_margins: ElkMargin = root_node.properties.get(&MARGINS);
        // calculate the current midpoint of the root, taking into account the
        // defined margins and the already calculated offset necessary to
        // shift the graph into the positive quadrant of the coordinate system
        let root_x = root_node.shape.x + root_node.shape.width / 2.0
            + (root_margins.left + root_margins.right) / 2.0
            - offset.x;
        let root_y = root_node.shape.y + root_node.shape.height / 2.0
            + (root_margins.top + root_margins.bottom) / 2.0
            - offset.y;

        let dx = width - root_x;
        let dy = height - root_y;

        if dx < width / 2.0 {
            // need to add additional space on the left
            let additional_x = dx - root_x;
            width += additional_x;
            offset.x -= additional_x;
        } else {
            // add additional space on the right
            let additional_x = root_x - dx;
            width += additional_x;
        }

        if dy < height / 2.0 {
            // need to add additional space on the top
            let additional_y = dy - root_y;
            height += additional_y;
            offset.y -= additional_y;
        } else {
            // add additional space on the bottom
            let additional_y = root_y - dy;
            height += additional_y;
        }
    }

    // process the nodes
    let children = g.node(graph).children.clone();
    for node in children {
        // set the node position
        let shape = &mut g.node_mut(node).shape;
        shape.x -= offset.x;
        shape.y -= offset.y;
    }

    // set up the graph
    if !g.node(graph).properties.get(&NODE_SIZE_FIXED_GRAPH_SIZE) {
        let shape = &mut g.node_mut(graph).shape;
        shape.width = width;
        shape.height = height;
    }

    // store child area info
    let props = &g.node(graph).properties;
    props.set(&CHILD_AREA_WIDTH, width - padding.horizontal());
    props.set(&CHILD_AREA_HEIGHT, height - padding.vertical());
}

/// Store the angle of each edge
/// leaving the root on the connected target so subsequent (top-down) child
/// layouts can align to it.
pub fn edge_angle_calculator(g: &mut ElkGraph, _graph: NodeId, root: NodeId) {
    for edge in g.node(root).outgoing_edges.clone() {
        let section = g.section(g.edge(edge).sections[0]);
        let start = KVector::new(section.start_x, section.start_y);
        let end = KVector::new(section.end_x, section.end_y);

        let edge_vector = KVector::diff(end, start);
        let angle = edge_vector.y.atan2(edge_vector.x);

        match g.edge(edge).targets[0] {
            ShapeId::Node(n) => {
                g.node(n).properties.set(&options::ROTATION_TARGET_ANGLE, angle);
            }
            ShapeId::Port(p) => {
                g.port(p).properties.set(&options::ROTATION_TARGET_ANGLE, angle);
            }
        }
    }
}
