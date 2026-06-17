//! The intermediate layout
//! processors (RootProcessor, FanProcessor, LevelProcessor,
//! NeighborsProcessor, LevelHeightProcessor, DirectionProcessor,
//! NodePositionProcessor, CompactionProcessor, LevelCoordinatesProcessor,
//! GraphBoundsProcessor, Untreeifyer).

use indexmap::IndexMap;

use crate::core::options_gen::Direction;
use crate::graph::math::KVector;

use crate::alg_mrtree::graph::{add_child, TArena, TGraph, TNodeId};
use crate::alg_mrtree::options;
use crate::alg_mrtree::options::EdgeRoutingMode;
use crate::alg_mrtree::tree_util;

// ------------------------------------------------------------- RootProcessor

/// Connects all roots of a given graph to a
/// super root which then is the new root of the graph.
pub fn root_processor(arena: &mut TArena, graph: &mut TGraph) {
    let mut roots: Vec<TNodeId> = Vec::new();

    for &node in &graph.nodes {
        if arena.node(node).incoming.is_empty() {
            arena.node_mut(node).root = true;
            roots.push(node);
        }
    }

    match roots.len() {
        0 => {
            let root = arena.create_node(0, "DUMMY_ROOT".to_string());
            arena.node_mut(root).root = true;
            arena.node_mut(root).dummy = true;
            graph.nodes.push(root);
        }
        1 => {
            // perfect, we already have only one root
        }
        _ => {
            let super_root = arena.create_node(0, "SUPER_ROOT".to_string());
            for &troot in &roots {
                add_child(arena, graph, super_root, troot);
                arena.node_mut(troot).root = false;
            }
            arena.node_mut(super_root).root = true;
            arena.node_mut(super_root).dummy = true;
            graph.nodes.push(super_root);
        }
    }
}

// -------------------------------------------------------------- FanProcessor

fn format_right(value: i32, len: i32) -> String {
    let mut s = value.to_string();
    while (s.len() as i32) < len {
        s.insert(0, '0');
    }
    s
}

/// Computes the maximal fan out and the
/// number of descendants for each node.
pub fn fan_processor(arena: &mut TArena, graph: &TGraph) {
    let mut glo_fan_map: IndexMap<String, i32> = IndexMap::new();
    let mut glo_desc_map: IndexMap<String, i32> = IndexMap::new();

    // find the root of the component
    let root = graph.nodes.iter().copied().find(|&n| arena.node(n).root);
    let root = match root {
        Some(r) => r,
        None => return, // a root always exists in practice
    };

    calculate_fan(arena, vec![root], &mut glo_fan_map, &mut glo_desc_map);

    // set the fan and descendants for all nodes
    for &tnode in &graph.nodes {
        let key = arena.node(tnode).id_string.clone();
        let fan = glo_fan_map.get(&key).copied().unwrap_or(0);
        arena.node_mut(tnode).fan = fan;
        let desc = 1 + glo_desc_map.get(&key).copied().unwrap_or(0);
        arena.node_mut(tnode).descendants = desc;
    }
}

fn calculate_fan(
    arena: &mut TArena,
    current_level: Vec<TNodeId>,
    glo_fan_map: &mut IndexMap<String, i32>,
    glo_desc_map: &mut IndexMap<String, i32>,
) {
    if current_level.is_empty() {
        return;
    }

    // the children of the current level are the next level
    let mut next_level: Vec<TNodeId> = Vec::new();

    let mut id: Option<String> = None;
    // The previous provisional id is compared by *reference*; within a
    // level the provisional ids of siblings are the same String object and
    // different parents always produce distinct id strings, so comparing by
    // content is equivalent.
    let mut p_id: Option<String> = None;

    // the size by which the stringId will be extended for this level
    let digits = ((current_level.len() as f64).log10().floor() + 1.0) as i32;

    // set the final stringId for all nodes in this level and the provisional
    // stringId for their children
    let mut index = 0;
    for &tnode in &current_level {
        let provisional = arena.node(tnode).id_string.clone();
        if p_id.as_deref() != Some(provisional.as_str()) {
            p_id = Some(provisional);
            index = 0;
        }
        // pId is always non-null here (the property default is "")
        let new_id = format!("{}{}", p_id.as_deref().unwrap(), format_right(index, digits));
        index += 1;
        arena.node_mut(tnode).id_string = new_id.clone();
        for tchild in arena.children(tnode) {
            next_level.push(tchild);
            // the provisional stringId is the id of the parent
            arena.node_mut(tchild).id_string = new_id.clone();
        }
        id = Some(new_id);
    }

    // holds the occurrences of descendants in this level
    let mut loc_fan_map: IndexMap<String, i32> = IndexMap::new();
    let id = id.unwrap();
    let prefix_len = id.len() as i32 - digits;
    for i in 0..prefix_len {
        for &tnode in &current_level {
            let key = arena.node(tnode).id_string[..(i + 1) as usize].to_string();
            *loc_fan_map.entry(key).or_insert(0) += 1;
        }
    }

    // update the global maps with the values from locFanMap
    for (key, value) in &loc_fan_map {
        let glo_value = glo_desc_map.get(key).copied().unwrap_or(0);
        glo_desc_map.insert(key.clone(), value + glo_value);
        let glo_value = glo_fan_map.get(key).copied();
        if glo_value.is_none() || glo_value.unwrap() < *value {
            glo_fan_map.insert(key.clone(), *value);
        }
    }

    // calculate the occurrences in the deeper levels
    calculate_fan(arena, next_level, glo_fan_map, glo_desc_map);
}

// ------------------------------------------------------------ LevelProcessor

/// Computes the treeLevel property for each
/// node. The level map is keyed by the `id` field, so a SUPER_ROOT
/// (which shares id 0 with a real node) overwrites/receives that node's
/// level, exactly like the original.
pub fn level_processor(arena: &mut TArena, graph: &TGraph) {
    let mut glo_level_map: IndexMap<i32, i32> = IndexMap::new();

    let roots: Vec<TNodeId> =
        graph.nodes.iter().copied().filter(|&n| arena.node(n).root).collect();

    set_level(arena, &roots, 0, &mut glo_level_map);

    for &tnode in &graph.nodes {
        let level = glo_level_map.get(&arena.node(tnode).id).copied().unwrap_or(0);
        arena.set_tree_level(tnode, level);
    }
}

fn set_level(
    arena: &TArena,
    current_level: &[TNodeId],
    level: i32,
    glo_level_map: &mut IndexMap<i32, i32>,
) {
    if !current_level.is_empty() {
        let mut next_level: Vec<TNodeId> = Vec::new();
        for &tnode in current_level {
            glo_level_map.insert(arena.node(tnode).id, level);
            for tchild in arena.children(tnode) {
                next_level.push(tchild);
            }
        }
        set_level(arena, &next_level, level + 1, glo_level_map);
    }
}

// -------------------------------------------------------- NeighborsProcessor

/// Determines the neighbors and
/// siblings for all nodes in the graph.
pub fn neighbors_processor(arena: &mut TArena, graph: &TGraph) {
    // find the root of the component
    let root = graph.nodes.iter().copied().find(|&n| arena.node(n).root);

    if let Some(root) = root {
        let level = arena.children(root);
        set_neighbors(arena, level);
    }
}

fn set_neighbors(arena: &mut TArena, current_level: Vec<TNodeId>) {
    if current_level.is_empty() {
        return;
    }
    let mut next_level: Vec<TNodeId> = Vec::new();
    let mut l_n: Option<TNodeId> = None;

    for &c_n in &current_level {
        // append the children of the current node to the next level
        next_level.extend(arena.children(c_n));
        if let Some(l_n) = l_n {
            arena.node_mut(l_n).right_neighbor = Some(c_n);
            arena.node_mut(c_n).left_neighbor = Some(l_n);
            if arena.parent(c_n) == arena.parent(l_n) {
                arena.node_mut(l_n).right_sibling = Some(c_n);
                arena.node_mut(c_n).left_sibling = Some(l_n);
            }
        }
        l_n = Some(c_n);
    }

    set_neighbors(arena, next_level);
}

// ------------------------------------------------------ LevelHeightProcessor

/// Sets each level's height to the
/// height of the tallest node of the level.
pub fn level_height_processor(arena: &mut TArena, graph: &TGraph) {
    let root = graph.nodes.iter().copied().find(|&n| arena.node(n).root);
    if let Some(root) = root {
        let layout_direction: Direction = graph.properties.get(&options::DIRECTION);
        set_level_heights(arena, vec![root], layout_direction);
    }
}

fn set_level_heights(arena: &mut TArena, current_level: Vec<TNodeId>, layout_direction: Direction) {
    if current_level.is_empty() {
        return;
    }
    let mut next_level: Vec<TNodeId> = Vec::new();
    let mut height = 0.0f64;
    if layout_direction.is_horizontal() {
        for &c_n in &current_level {
            next_level.extend(arena.children(c_n));
            if height < arena.node(c_n).size.x {
                height = arena.node(c_n).size.x;
            }
        }
    } else {
        for &c_n in &current_level {
            next_level.extend(arena.children(c_n));
            if height < arena.node(c_n).size.y {
                height = arena.node(c_n).size.y;
            }
        }
    }
    for &c_n in &current_level {
        arena.node_mut(c_n).level_height = height;
    }

    set_level_heights(arena, next_level, layout_direction);
}

// -------------------------------------------------------- DirectionProcessor

/// Swaps the integer coordinates
/// according to the layout direction.
pub fn direction_processor(arena: &mut TArena, graph: &TGraph) {
    let d: Direction = graph.properties.get(&options::DIRECTION);

    if d != Direction::DOWN {
        for &n in &graph.nodes {
            let mut x = arena.node(n).xcoor;
            let mut y = arena.node(n).ycoor;

            match d {
                Direction::UP => {
                    y = -y;
                }
                Direction::RIGHT => {
                    std::mem::swap(&mut x, &mut y);
                }
                Direction::LEFT => {
                    let tmp2 = x;
                    x = -y;
                    y = tmp2;
                }
                _ => {}
            }

            arena.node_mut(n).xcoor = x;
            arena.node_mut(n).ycoor = y;
        }
    }
}

// ----------------------------------------------------- NodePositionProcessor

/// Sets the final coordinates for
/// each node from XCOOR/YCOOR, then shifts every node to its top-left corner.
pub fn node_position_processor(arena: &mut TArena, graph: &TGraph) {
    // find the root of the component
    let mut root: Option<TNodeId> = None;
    for &tnode in &graph.nodes {
        if arena.node(tnode).root {
            root = Some(tnode);
            let n = arena.node_mut(tnode);
            n.pos.x = n.xcoor as f64;
            n.pos.y = n.ycoor as f64;
            break;
        }
    }
    let root = root.expect("NodePositionProcessor: no root");

    // start with the root and level down by bfs
    let mut next_level = arena.children(root);
    while !next_level.is_empty() {
        let mut new_level: Vec<TNodeId> = Vec::new();
        for &tnode in &next_level {
            new_level.extend(arena.children(tnode));
            let n = arena.node_mut(tnode);
            n.pos.x = n.xcoor as f64;
            n.pos.y = n.ycoor as f64;
        }
        next_level = new_level;
    }

    // move node positions to their middle to achieve the same spacing
    // between all nodes
    for &n in &graph.nodes {
        let node = arena.node_mut(n);
        let half = KVector::new(node.size.x / 2.0, node.size.y / 2.0);
        node.pos.sub(half);
    }
}

// ------------------------------------------------- LevelCoordinatesProcessor

/// Computes the start and end
/// coordinates for each level's nodes.
pub fn level_coordinates_processor(arena: &mut TArena, graph: &TGraph) {
    let mut levels: Vec<(f64, f64)> = Vec::new();
    let horizontal = graph.properties.get(&options::DIRECTION).is_horizontal();

    // set up levels
    for &n in &graph.nodes {
        while arena.tree_level(n) > levels.len() as i32 - 1 {
            levels.push((f64::MAX, -f64::MAX));
        }

        let cur_level = arena.tree_level(n) as usize;
        let node = arena.node(n);
        if horizontal {
            if node.pos.x < levels[cur_level].0 {
                levels[cur_level].0 = node.pos.x;
            }
            if node.pos.x + node.size.x > levels[cur_level].1 {
                levels[cur_level].1 = node.pos.x + node.size.x;
            }
        } else {
            if node.pos.y < levels[cur_level].0 {
                levels[cur_level].0 = node.pos.y;
            }
            if node.pos.y + node.size.y > levels[cur_level].1 {
                levels[cur_level].1 = node.pos.y + node.size.y;
            }
        }
    }

    // set node properties
    for &n in &graph.nodes {
        let cur_level = arena.tree_level(n) as usize;
        arena.node_mut(n).level_min = levels[cur_level].0;
        arena.node_mut(n).level_max = levels[cur_level].1;
    }
}

// ------------------------------------------------------ GraphBoundsProcessor

/// Sets the graph's x/y max/min.
pub fn graph_bounds_processor(arena: &TArena, graph: &mut TGraph) {
    graph.graph_xmin = graph
        .nodes
        .iter()
        .map(|&n| arena.node(n).pos.x)
        .fold(f64::INFINITY, f64::min);
    graph.graph_ymin = graph
        .nodes
        .iter()
        .map(|&n| arena.node(n).pos.y)
        .fold(f64::INFINITY, f64::min);
    graph.graph_xmax = graph
        .nodes
        .iter()
        .map(|&n| arena.node(n).pos.x + arena.node(n).size.x)
        .fold(f64::NEG_INFINITY, f64::max);
    graph.graph_ymax = graph
        .nodes
        .iter()
        .map(|&n| arena.node(n).pos.y + arena.node(n).size.y)
        .fold(f64::NEG_INFINITY, f64::max);
}

// ---------------------------------------------------------------- Untreeifyer

/// Reinserts the edges that were removed
/// during treeification.
pub fn untreeifyer(arena: &mut TArena, graph: &TGraph) {
    for &tedge in &graph.removable_edges {
        let (source, target) = {
            let e = arena.edge(tedge);
            (e.source, e.target)
        };
        arena.node_mut(source).outgoing.push(tedge);
        arena.node_mut(target).incoming.push(tedge);
    }
}

// -------------------------------------------------------- CompactionProcessor

/// A `TreeSet<TNode>` with a comparator over the projection of the node
/// position onto the direction vector: elements whose keys compare equal via
/// `Double.compare` are treated as the *same* element (add is rejected,
/// remove removes the resident element).
struct NodeTreeSet {
    /// sorted ascending by key, unique keys
    items: Vec<(f64, TNodeId)>,
}

impl NodeTreeSet {
    fn new() -> Self {
        NodeTreeSet { items: Vec::new() }
    }

    fn add(&mut self, key: f64, n: TNodeId) {
        match self.items.binary_search_by(|(k, _)| k.total_cmp(&key)) {
            Ok(_) => {} // equal element already present: rejected
            Err(pos) => self.items.insert(pos, (key, n)),
        }
    }

    fn remove(&mut self, key: f64) {
        if let Ok(pos) = self.items.binary_search_by(|(k, _)| k.total_cmp(&key)) {
            self.items.remove(pos);
        }
    }

    /// Index of the first element with `k >= key` (start of `tailSet`).
    fn tail_start(&self, key: f64) -> usize {
        self.items.partition_point(|(k, _)| k.total_cmp(&key) == std::cmp::Ordering::Less)
    }

    /// `headSet(key).size()`.
    fn head_size(&self, key: f64) -> usize {
        self.tail_start(key)
    }

    /// `headSet(key).last()`.
    fn head_last(&self, key: f64) -> Option<TNodeId> {
        let i = self.tail_start(key);
        if i > 0 {
            Some(self.items[i - 1].1)
        } else {
            None
        }
    }

    /// `tailSet(key).size()`.
    fn tail_size(&self, key: f64) -> usize {
        self.items.len() - self.tail_start(key)
    }

    /// The second element of
    /// `tailSet(key)`.
    fn right_element(&self, key: f64) -> TNodeId {
        let i = self.tail_start(key);
        self.items[i + 1].1
    }
}

/// One dimensional compaction.
pub fn compaction_processor(arena: &mut TArena, graph: &mut TGraph) {
    if !graph.properties.get(&options::COMPACTION) {
        return; // leave if option is not set
    }

    let dir: Direction = graph.properties.get(&options::DIRECTION);
    let node_node_spacing: f64 = graph.properties.get(&options::SPACING_NODE_NODE);

    let levels = set_up_levels(arena, graph, dir);

    compute_node_constraints(arena, graph, node_node_spacing / 2.0 / 2.0);

    // Simple one dimensional compaction \w level preservation.
    // The graph's node list is sorted in place, affecting all later
    // processors' iteration order.
    let dir_vec = tree_util::get_direction_vector(dir);
    let mut nodes = std::mem::take(&mut graph.nodes);
    nodes.sort_by(|&x, &y| {
        let kx = dir_vec.dot_product(arena.node(x).pos);
        let ky = dir_vec.dot_product(arena.node(y).pos);
        kx.total_cmp(&ky)
    });
    graph.nodes = nodes;

    let avoid_overlap =
        graph.properties.get(&options::EDGE_ROUTING_MODE) == EdgeRoutingMode::AVOID_OVERLAP;

    for i in 0..graph.nodes.len() {
        let n = graph.nodes[i];
        // root nodes aren't compactable
        if arena.node(n).root {
            continue;
        }
        let d = get_lowest_dependent_node(arena, n, dir);
        let p = tree_util::get_lowest_parent(arena, n, graph);
        let mut new_pos = 0.0f64;
        let mut new_pos_size = 0.0f64;
        if let Some(d) = d {
            // n has a dependent node
            let pos = arena.node(d).pos;
            let p = p.expect("compaction: dependent node without parent");
            match dir {
                Direction::LEFT => {
                    new_pos = pos.x - node_node_spacing - arena.node(n).size.x;
                    if arena.node(p).pos.x - node_node_spacing - arena.node(n).size.x < new_pos {
                        new_pos = arena.node(p).pos.x - node_node_spacing - arena.node(n).size.x;
                    }
                    new_pos_size = new_pos + arena.node(n).size.x;
                }
                Direction::RIGHT => {
                    new_pos = pos.x + arena.node(d).size.x + node_node_spacing;
                    if arena.node(p).pos.x + node_node_spacing > new_pos {
                        new_pos = arena.node(p).pos.x + arena.node(p).size.x + node_node_spacing;
                    }
                    new_pos_size = new_pos + arena.node(n).size.x;
                }
                Direction::UP => {
                    new_pos = pos.y - node_node_spacing - arena.node(n).size.y;
                    if arena.node(p).pos.y - node_node_spacing - arena.node(n).size.y < new_pos {
                        new_pos = arena.node(p).pos.y - node_node_spacing - arena.node(n).size.y;
                    }
                    new_pos_size = new_pos + arena.node(n).size.y;
                }
                Direction::DOWN => {
                    new_pos = pos.y + arena.node(d).size.y + node_node_spacing;
                    if arena.node(p).pos.y + node_node_spacing > new_pos {
                        new_pos = arena.node(p).pos.y + arena.node(p).size.y + node_node_spacing;
                    }
                    new_pos_size = new_pos + arena.node(n).size.y;
                }
                _ => {}
            }
        } else if let Some(p) = p {
            // n does not have a dependent node but a parent
            match dir {
                Direction::LEFT => {
                    new_pos = arena.node(p).pos.x - node_node_spacing - arena.node(n).size.x;
                    new_pos_size = new_pos + arena.node(n).size.x;
                }
                Direction::RIGHT => {
                    new_pos = arena.node(p).pos.x + arena.node(p).size.x + node_node_spacing;
                    new_pos_size = new_pos + arena.node(n).size.x;
                }
                Direction::UP => {
                    new_pos = arena.node(p).pos.y - node_node_spacing - arena.node(n).size.y;
                    new_pos_size = new_pos + arena.node(n).size.y;
                }
                Direction::DOWN => {
                    new_pos = arena.node(p).pos.y + arena.node(p).size.y + node_node_spacing;
                    new_pos_size = new_pos + arena.node(n).size.y;
                }
                _ => {}
            }
        }

        if avoid_overlap {
            let mut level: Option<usize> = levels
                .iter()
                .position(|&(first, second)| first <= new_pos && second >= new_pos_size);
            if level.is_some() {
                // the node ended up within a level
                if dir.is_horizontal() {
                    arena.node_mut(n).pos.x = new_pos;
                } else {
                    arena.node_mut(n).pos.y = new_pos;
                }
            } else {
                if dir == Direction::LEFT || dir == Direction::UP {
                    // skip the first level as it only contains the SUPER_ROOT
                    level = levels
                        .iter()
                        .enumerate()
                        .skip(1)
                        .find(|&(_, &(first, _))| first <= new_pos)
                        .map(|(i, _)| i);
                } else {
                    level = levels
                        .iter()
                        .enumerate()
                        .skip(1)
                        .find(|&(_, &(first, _))| first >= new_pos)
                        .map(|(i, _)| i);
                }

                // force n into the found level
                if let Some(level) = level {
                    if dir.is_horizontal() {
                        arena.node_mut(n).pos.x = levels[level].0;
                    } else {
                        arena.node_mut(n).pos.y = levels[level].0;
                    }
                }
            }

            // update tree level of node; levels.indexOf(level) is used,
            // which finds the first level pair *equal* to the found one
            if let Some(level) = level {
                let target = levels[level];
                let new_index = levels
                    .iter()
                    .position(|&(a, b)| {
                        a.to_bits() == target.0.to_bits() && b.to_bits() == target.1.to_bits()
                    })
                    .unwrap() as i32;
                if new_index > 0 && new_index != arena.tree_level(n) {
                    arena.node_mut(n).compact_level_ascension = true;
                    arena.set_tree_level(n, new_index);
                }
            }
        } else {
            // in case of aggressive compaction just set the parameters
            if dir.is_horizontal() {
                arena.node_mut(n).pos.x = new_pos;
            } else {
                arena.node_mut(n).pos.y = new_pos;
            }
        }
    }
}

fn set_up_levels(arena: &TArena, graph: &TGraph, dir: Direction) -> Vec<(f64, f64)> {
    let mut levels: Vec<(f64, f64)> = Vec::new();

    for &n in &graph.nodes {
        // adapt levels size to the current level
        while arena.tree_level(n) > levels.len() as i32 - 1 {
            levels.push((f64::MAX, -f64::MAX));
        }

        // update level bounds
        let cur_level = arena.tree_level(n) as usize;
        let node = arena.node(n);
        if dir.is_horizontal() {
            if node.pos.x < levels[cur_level].0 {
                levels[cur_level].0 = node.pos.x;
            }
            if node.pos.x + node.size.x > levels[cur_level].1 {
                levels[cur_level].1 = node.pos.x + node.size.x;
            }
        } else {
            if node.pos.y < levels[cur_level].0 {
                levels[cur_level].0 = node.pos.y;
            }
            if node.pos.y + node.size.y > levels[cur_level].1 {
                levels[cur_level].1 = node.pos.y + node.size.y;
            }
        }
    }

    levels
}

fn compute_node_constraints(arena: &mut TArena, graph: &TGraph, node_node_spacing: f64) {
    let d: Direction = graph.properties.get(&options::DIRECTION);
    let right = if d.is_horizontal() { Direction::DOWN } else { Direction::RIGHT };
    let right_vec = tree_util::get_direction_vector(right);
    let d_vec = tree_util::get_direction_vector(d);

    // get a filtered list of all relevant nodes
    let actual_nodes: Vec<TNodeId> = graph
        .nodes
        .iter()
        .copied()
        .filter(|&x| !arena.node(x).label.contains("SUPER_ROOT"))
        .collect();

    // build the point list: upper left node edges, then lower right ones
    let mut points: Vec<(TNodeId, KVector, bool)> = actual_nodes
        .iter()
        .map(|&x| {
            let mut v = arena.node(x).pos;
            v.sub_xy(node_node_spacing, node_node_spacing);
            (x, v, true)
        })
        .collect();
    points.extend(actual_nodes.iter().map(|&x| {
        let node = arena.node(x);
        let mut v = node.pos;
        v.add_xy(node.size.x + node_node_spacing, node.size.y + node_node_spacing);
        (x, v, false)
    }));
    points.sort_by(|x, y| {
        right_vec.dot_product(x.1).total_cmp(&right_vec.dot_product(y.1))
    });

    // set sorted by the node position's projection onto the direction vector
    let mut s = NodeTreeSet::new();
    let key = |arena: &TArena, n: TNodeId| d_vec.dot_product(arena.node(n).pos);

    let mut cand: IndexMap<TNodeId, TNodeId> = IndexMap::new();

    // scanline
    for &(r, _, third) in &points {
        let rk = key(arena, r);
        if third {
            s.add(rk, r);
            if s.head_size(rk) > 0 {
                cand.insert(r, s.head_last(rk).unwrap());
            }
            if s.tail_size(rk) > 1 {
                cand.insert(s.right_element(rk), r);
            }
        } else {
            // we need to check if right and left even exist
            if s.head_size(rk) > 0 {
                let left_node = s.head_last(rk).unwrap();
                if cand.get(&r) == Some(&left_node) {
                    arena.node_mut(r).compact_constraints.push(left_node);
                }
            }
            if s.tail_size(rk) > 1 {
                let right_node = s.right_element(rk);
                if cand.get(&right_node) == Some(&r) {
                    arena.node_mut(right_node).compact_constraints.push(r);
                }
            }
            s.remove(rk);
        }
    }
}

fn get_lowest_dependent_node(arena: &TArena, n: TNodeId, d: Direction) -> Option<TNodeId> {
    let cons = &arena.node(n).compact_constraints;
    if cons.is_empty() {
        return None;
    } else if cons.len() == 1 {
        return Some(cons[0]);
    }

    // Stream.min/max keep the *first* extremal element on ties
    // (reduce keeps `a` when compare(a, b) <= 0 resp. >= 0).
    fn stream_min(items: &[TNodeId], key: impl Fn(TNodeId) -> f64) -> Option<TNodeId> {
        items.iter().copied().reduce(|a, b| if key(a).total_cmp(&key(b)).is_le() { a } else { b })
    }
    fn stream_max(items: &[TNodeId], key: impl Fn(TNodeId) -> f64) -> Option<TNodeId> {
        items.iter().copied().reduce(|a, b| if key(a).total_cmp(&key(b)).is_ge() { a } else { b })
    }

    match d {
        Direction::LEFT => stream_min(cons, |x| arena.node(x).pos.x),
        Direction::RIGHT => stream_max(cons, |x| arena.node(x).pos.x + arena.node(x).size.x),
        Direction::UP => stream_min(cons, |x| arena.node(x).pos.y),
        Direction::DOWN => stream_max(cons, |x| arena.node(x).pos.y + arena.node(x).size.y),
        _ => None,
    }
}
