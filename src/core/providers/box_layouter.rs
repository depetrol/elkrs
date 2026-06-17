
use std::cmp::Ordering;

use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::{KVector, Spacing};

use crate::core::elkutil;
use crate::core::javacompat::JavaPriorityQueue;
use crate::core::options::*;
use crate::core::registry::LayoutProvider;

pub const DEF_ASPECT_RATIO: f64 = 1.3;

#[derive(Default)]
pub struct BoxLayoutProvider;

impl LayoutProvider for BoxLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        let obj_spacing =
            g.node(layout_node).properties.get(&boxl::SPACING_NODE_NODE) as f32;
        let padding = g.node(layout_node).properties.get(&boxl::PADDING);
        let expand_nodes = g.node(layout_node).properties.get(&EXPAND_NODES);
        let interactive = g.node(layout_node).properties.get(&INTERACTIVE);

        match g.node(layout_node).properties.get(&BOX_PACKING_MODE) {
            PackingMode::SIMPLE => {
                place_boxes_simple(g, layout_node, obj_spacing as f64, padding, expand_nodes, interactive);
            }
            _ => {
                place_boxes_grouping(g, layout_node, obj_spacing, padding, expand_nodes);
            }
        }
        Ok(())
    }
}

// ----------------------------------------------------------------- sorting

fn sort_boxes(g: &ElkGraph, parent: NodeId, interactive: bool) -> Vec<NodeId> {
    let mut sorted: Vec<NodeId> = g.node(parent).children.clone();
    sorted.sort_by(|&a, &b| {
        let prio1: i32 = g.node(a).properties.get(&boxl::PRIORITY);
        let prio2: i32 = g.node(b).properties.get(&boxl::PRIORITY);
        if prio1 > prio2 {
            return Ordering::Less;
        } else if prio1 < prio2 {
            return Ordering::Greater;
        }
        if interactive {
            let c = g.node(a).shape.y.total_cmp(&g.node(b).shape.y);
            if c != Ordering::Equal {
                return c;
            }
            let c = g.node(a).shape.x.total_cmp(&g.node(b).shape.x);
            if c != Ordering::Equal {
                return c;
            }
        }
        let size1 = g.node(a).shape.width * g.node(a).shape.height;
        let size2 = g.node(b).shape.width * g.node(b).shape.height;
        size1.total_cmp(&size2)
    });
    sorted
}

// ------------------------------------------------------------- simple mode

fn place_boxes_simple(
    g: &mut ElkGraph,
    parent: NodeId,
    obj_spacing: f64,
    padding: Spacing,
    expand_nodes: bool,
    interactive: bool,
) {
    let sorted_boxes = sort_boxes(g, parent, interactive);
    let min_size = elkutil::effective_min_size_constraint_for(g, parent);
    let mut aspect_ratio: f64 = g.node(parent).properties.get_opt(&boxl::ASPECT_RATIO).unwrap_or(0.0);
    if aspect_ratio <= 0.0 {
        aspect_ratio = DEF_ASPECT_RATIO;
    }
    let parent_size = place_boxes(
        g,
        &sorted_boxes,
        obj_spacing,
        padding,
        min_size.x,
        min_size.y,
        expand_nodes,
        aspect_ratio,
    );
    elkutil::resize_node(g, parent, parent_size.x, parent_size.y, false, true);
}

#[allow(clippy::too_many_arguments)]
fn place_boxes(
    g: &mut ElkGraph,
    sorted_boxes: &[NodeId],
    min_spacing: f64,
    padding: Spacing,
    min_total_width: f64,
    min_total_height: f64,
    expand_nodes: bool,
    aspect_ratio: f64,
) -> KVector {
    let mut max_row_width = 0.0f64;
    let mut total_area = 0.0f64;
    for &node in sorted_boxes {
        elkutil::resize_node_constraints(g, node);
        let s = &g.node(node).shape;
        max_row_width = f64::max(max_row_width, s.width);
        total_area += s.width * s.height;
    }

    let mean = total_area / sorted_boxes.len() as f64;
    let stddev = area_std_dev(g, sorted_boxes, mean);

    total_area += sorted_boxes.len() as f64 * 1.0 * stddev;
    total_area += total_area.sqrt() * (padding.bottom + padding.top);
    total_area += total_area.sqrt() * padding.right;

    max_row_width = f64::max(max_row_width, (total_area * aspect_ratio).sqrt()) + padding.left;

    // place nodes iteratively into rows
    let mut xpos = padding.left;
    let mut ypos = padding.top;
    let mut highest_box = 0.0f64;
    let mut broadest_row = padding.horizontal();
    let mut row_indices: Vec<usize> = vec![0];
    let mut row_heights: Vec<f64> = Vec::new();
    for (index, &node) in sorted_boxes.iter().enumerate() {
        let (width, height) = {
            let s = &g.node(node).shape;
            (s.width, s.height)
        };
        if xpos + width > max_row_width {
            // place box into the next row
            if expand_nodes {
                row_heights.push(highest_box);
                row_indices.push(index);
            }
            xpos = padding.left;
            ypos += highest_box + min_spacing;
            highest_box = 0.0;
            broadest_row = f64::max(broadest_row, padding.horizontal() + width);
        }
        g.node_mut(node).shape.set_location(xpos, ypos);
        broadest_row = f64::max(broadest_row, xpos + width + padding.right);
        highest_box = f64::max(highest_box, height);
        xpos += width + min_spacing;
    }
    broadest_row = f64::max(broadest_row, min_total_width);
    let mut total_height = ypos + highest_box + padding.bottom;
    if total_height < min_total_height {
        highest_box += min_total_height - total_height;
        total_height = min_total_height;
    }

    // expand nodes if required
    if expand_nodes {
        xpos = padding.left;
        row_indices.push(sorted_boxes.len());
        let mut row_index_iter = row_indices.iter();
        let mut next_row_index = *row_index_iter.next().unwrap();
        row_heights.push(highest_box);
        let mut row_height_iter = row_heights.iter();
        let mut row_height = 0.0f64;
        for (index, &node) in sorted_boxes.iter().enumerate() {
            if index == next_row_index {
                xpos = padding.left;
                row_height = *row_height_iter.next().unwrap();
                next_row_index = *row_index_iter.next().unwrap();
            }
            let old_height = g.node(node).shape.height;
            g.node_mut(node).shape.height = row_height;
            let new_height = row_height;
            if index + 1 == next_row_index {
                let new_width = broadest_row - xpos - padding.right;
                let old_width = g.node(node).shape.width;
                g.node_mut(node).shape.width = new_width;
                elkutil::translate_aligned(
                    g,
                    node,
                    KVector::new(new_width, new_height),
                    KVector::new(old_width, old_height),
                );
            }
            xpos += g.node(node).shape.width + min_spacing;
        }
    }

    KVector::new(broadest_row, total_height)
}

fn area_std_dev(g: &ElkGraph, boxes: &[NodeId], mean: f64) -> f64 {
    let mut variance = 0.0;
    for &node in boxes {
        let s = &g.node(node).shape;
        variance += (s.width * s.height - mean).powi(2);
    }
    (variance / (boxes.len() as f64 - 1.0)).sqrt()
}

// ---------------------------------------------------------- grouping modes

/// Arena-backed port of the nested `Group` class.
struct Groups {
    nodes: Vec<Option<NodeId>>,
    children: Vec<Vec<usize>>,
    sizes: Vec<KVector>,
    bottoms: Vec<Vec<usize>>,
    rights: Vec<Vec<usize>>,
}

impl Groups {
    fn new() -> Self {
        Groups {
            nodes: Vec::new(),
            children: Vec::new(),
            sizes: Vec::new(),
            bottoms: Vec::new(),
            rights: Vec::new(),
        }
    }

    fn new_leaf(&mut self, g: &mut ElkGraph, node: NodeId) -> usize {
        g.node_mut(node).shape.set_location(0.0, 0.0);
        self.push(Some(node), Vec::new())
    }

    fn new_group(&mut self, members: Vec<usize>) -> usize {
        self.push(None, members)
    }

    fn push(&mut self, node: Option<NodeId>, children: Vec<usize>) -> usize {
        self.nodes.push(node);
        self.children.push(children);
        self.sizes.push(KVector::default());
        self.bottoms.push(Vec::new());
        self.rights.push(Vec::new());
        self.nodes.len() - 1
    }

    fn width(&self, g: &ElkGraph, idx: usize) -> f64 {
        match self.nodes[idx] {
            Some(n) => g.node(n).shape.width,
            None => self.sizes[idx].x,
        }
    }

    fn height(&self, g: &ElkGraph, idx: usize) -> f64 {
        match self.nodes[idx] {
            Some(n) => g.node(n).shape.height,
            None => self.sizes[idx].y,
        }
    }

    fn area(&self, g: &ElkGraph, idx: usize) -> f64 {
        self.width(g, idx) * self.height(g, idx)
    }

    fn set_width(&mut self, g: &mut ElkGraph, idx: usize, w: f64) {
        match self.nodes[idx] {
            Some(n) => g.node_mut(n).shape.width = w,
            None => {
                let delta = w - self.width(g, idx);
                let rights = self.rights[idx].clone();
                for r in rights {
                    let rw = self.width(g, r);
                    self.set_width(g, r, rw + delta);
                }
            }
        }
    }

    fn set_height(&mut self, g: &mut ElkGraph, idx: usize, h: f64) {
        match self.nodes[idx] {
            Some(n) => g.node_mut(n).shape.height = h,
            None => {
                let delta = h - self.height(g, idx);
                let bottoms = self.bottoms[idx].clone();
                for b in bottoms {
                    let bh = self.height(g, b);
                    self.set_height(g, b, bh + delta);
                }
            }
        }
    }

    fn translate(&mut self, g: &mut ElkGraph, idx: usize, x: f64, y: f64) {
        match self.nodes[idx] {
            Some(n) => {
                let s = &mut g.node_mut(n).shape;
                s.x += x;
                s.y += y;
            }
            None => {
                let children = self.children[idx].clone();
                for c in children {
                    self.translate(g, c, x, y);
                }
            }
        }
    }

    fn translate_inner_nodes(&mut self, g: &mut ElkGraph, idx: usize, x: f64, y: f64) {
        match self.nodes[idx] {
            Some(n) => elkutil::translate(g, n, x, y),
            None => {
                let children = self.children[idx].clone();
                for c in children {
                    self.translate_inner_nodes(g, c, x, y);
                }
            }
        }
    }
}

fn place_boxes_grouping(
    g: &mut ElkGraph,
    parent: NodeId,
    obj_spacing: f32,
    padding: Spacing,
    expand_nodes: bool,
) {
    let mut min_size: KVector = g.node(parent).properties.get(&NODE_SIZE_MINIMUM);
    min_size.x = f64::max(min_size.x - padding.left - padding.right, 0.0);
    min_size.y = f64::max(min_size.y - padding.top - padding.bottom, 0.0);

    let mut aspect_ratio: f64 = g.node(parent).properties.get_opt(&boxl::ASPECT_RATIO).unwrap_or(0.0);
    if aspect_ratio <= 0.0 {
        aspect_ratio = DEF_ASPECT_RATIO;
    }

    let mut groups = Groups::new();
    let children = g.node(parent).children.clone();
    let initial: Vec<usize> = children.iter().map(|&n| groups.new_leaf(g, n)).collect();

    let mode = g.node(parent).properties.get(&BOX_PACKING_MODE);
    let to_be_placed = match mode {
        PackingMode::GROUP_INC => merge_and_place_inc(
            g, &mut groups, initial, obj_spacing as f64, min_size.x, min_size.y, expand_nodes,
        ),
        PackingMode::GROUP_DEC => merge_and_place_dec(
            g, &mut groups, initial, obj_spacing as f64, min_size.x, min_size.y, expand_nodes,
        ),
        _ => merge_and_place_mixed(
            g, &mut groups, initial, obj_spacing as f64, min_size.x, min_size.y, expand_nodes,
        ),
    };

    let final_group = groups.new_group(to_be_placed);
    let parent_size = place_inner_boxes(
        g,
        &mut groups,
        final_group,
        obj_spacing as f64,
        padding,
        min_size.x,
        min_size.y,
        expand_nodes,
        aspect_ratio,
    );
    elkutil::resize_node(g, parent, parent_size.x, parent_size.y, false, true);
}

#[allow(clippy::too_many_arguments)]
fn place_inner_boxes(
    g: &mut ElkGraph,
    groups: &mut Groups,
    group: usize,
    min_spacing: f64,
    padding: Spacing,
    min_total_width: f64,
    min_total_height: f64,
    expand_nodes: bool,
    aspect_ratio: f64,
) -> KVector {
    let members = groups.children[group].clone();

    let mut max_row_width = 0.0f64;
    let mut total_area = 0.0f64;
    for &b in &members {
        if let Some(node) = groups.nodes[b] {
            elkutil::resize_node_constraints(g, node);
        }
        max_row_width = f64::max(max_row_width, groups.width(g, b));
        total_area += groups.width(g, b) * groups.height(g, b);
    }

    let mean = total_area / members.len() as f64;
    let stddev = {
        let mut variance = 0.0;
        for &b in &members {
            variance += (groups.area(g, b) - mean).powi(2);
        }
        (variance / (members.len() as f64 - 1.0)).sqrt()
    };
    let sd_influence = 1.0;
    total_area += members.len() as f64 * sd_influence * stddev;

    max_row_width = f64::max(max_row_width, (total_area * aspect_ratio).sqrt()) + padding.left;

    let mut xpos = padding.left;
    let mut ypos = padding.top;
    let mut highest_box = 0.0f64;
    let mut broadest_row = padding.horizontal();
    let mut row_indices: Vec<usize> = vec![0];
    let mut row_heights: Vec<f64> = Vec::new();
    let mut last: Option<usize> = None;
    let mut bottoms: Vec<usize> = Vec::new();
    for (index, &b) in members.iter().enumerate() {
        let width = groups.width(g, b);
        let height = groups.height(g, b);
        if xpos + width > max_row_width {
            if expand_nodes {
                row_heights.push(highest_box);
                row_indices.push(index);
                groups.rights[group].push(last.unwrap());
                bottoms.clear();
            }
            xpos = padding.left;
            ypos += highest_box + min_spacing;
            highest_box = 0.0;
            broadest_row = f64::max(broadest_row, padding.horizontal() + width);
        }
        bottoms.push(b);
        groups.translate(g, b, xpos, ypos);
        broadest_row = f64::max(broadest_row, xpos + width + padding.right);
        highest_box = f64::max(highest_box, height);
        xpos += width + min_spacing;
        last = Some(b);
    }
    groups.bottoms[group].extend(bottoms.iter().copied());
    groups.rights[group].push(*bottoms.last().unwrap());
    broadest_row = f64::max(broadest_row, min_total_width);
    let mut total_height = ypos + highest_box + padding.bottom;
    if total_height < min_total_height {
        highest_box += min_total_height - total_height;
        total_height = min_total_height;
    }

    if expand_nodes {
        xpos = padding.left;
        row_indices.push(members.len());
        let mut row_index_iter = row_indices.iter();
        let mut next_row_index = *row_index_iter.next().unwrap();
        row_heights.push(highest_box);
        let mut row_height_iter = row_heights.iter();
        let mut row_height = 0.0f64;
        for (index, &b) in members.iter().enumerate() {
            if index == next_row_index {
                xpos = padding.left;
                row_height = *row_height_iter.next().unwrap();
                next_row_index = *row_index_iter.next().unwrap();
            }
            groups.set_height(g, b, row_height);
            if index + 1 == next_row_index {
                let new_width = broadest_row - xpos - padding.right;
                let old_width = groups.width(g, b);
                groups.set_width(g, b, new_width);
                groups.translate_inner_nodes(g, b, (new_width - old_width) / 2.0, 0.0);
            }
            xpos += groups.width(g, b) + min_spacing;
        }
    }

    KVector::new(broadest_row, total_height)
}

fn merge_and_place_dec(
    g: &mut ElkGraph,
    groups: &mut Groups,
    mut group_list: Vec<usize>,
    obj_spacing: f64,
    min_width: f64,
    min_height: f64,
    expand_nodes: bool,
) -> Vec<usize> {
    // sort in decreasing area (stable, like Collections.sort)
    group_list.sort_by(|&a, &b| groups.area(g, b).total_cmp(&groups.area(g, a)));

    let mut box_queue: std::collections::VecDeque<usize> = group_list.into_iter().collect();
    let mut to_be_placed: Vec<usize> = Vec::new();
    let mut maybe_group: Vec<usize> = Vec::new();

    let mut box_to_beat: Option<usize> = None;
    let mut collected_area = 0.0f64;

    while let Some(b) = box_queue.pop_front() {
        let beat_area = box_to_beat.map(|bb| groups.area(g, bb));
        if beat_area.is_none() || beat_area.unwrap() / 2.0 < groups.area(g, b) {
            box_to_beat = Some(b);
            to_be_placed.push(b);
        } else {
            collected_area += groups.area(g, b);
            maybe_group.push(b);

            if maybe_group.len() > 1
                && (collected_area > beat_area.unwrap() / 2.0 || box_queue.is_empty())
            {
                let inner_group = groups.new_group(maybe_group.clone());
                let bb = box_to_beat.unwrap();
                let inner_aspect_ratio = groups.width(g, bb) / groups.height(g, bb);
                let group_size = place_inner_boxes(
                    g,
                    groups,
                    inner_group,
                    obj_spacing,
                    Spacing::default(),
                    min_width,
                    min_height,
                    expand_nodes,
                    inner_aspect_ratio,
                );
                groups.sizes[inner_group] = group_size;

                box_to_beat = Some(inner_group);
                to_be_placed.push(inner_group);

                collected_area = 0.0;
                maybe_group.clear();
            }
        }
    }

    to_be_placed.extend(maybe_group);
    to_be_placed
}

fn merge_and_place_mixed(
    g: &mut ElkGraph,
    groups: &mut Groups,
    group_list: Vec<usize>,
    obj_spacing: f64,
    min_width: f64,
    min_height: f64,
    expand_nodes: bool,
) -> Vec<usize> {
    let n = group_list.len();
    let mut cum_area_array = vec![0.0f64; n];

    // Priority queue ordered by area; keys captured at insertion time.
    let mut pq: JavaPriorityQueue<(f64, usize)> =
        JavaPriorityQueue::new(|a, b| a.0.total_cmp(&b.0));
    for &gi in &group_list {
        pq.add((groups.area(g, gi), gi));
    }

    let mut index = 0usize;
    let mut to_be_placed: Vec<usize> = Vec::new();

    while !pq.is_empty() {
        let &(box_area, box_idx) = pq.peek().unwrap();

        if index > 1 && box_area / 2.0 > cum_area_array[0] {
            let mut an_index = 0usize;
            while an_index < to_be_placed.len() - 1 && box_area / 2.0 > cum_area_array[an_index] {
                an_index += 1;
            }

            let select: Vec<usize> = to_be_placed[0..an_index + 1].to_vec();
            let remain: Vec<usize> = to_be_placed[an_index + 1..].to_vec();
            let inner_group = groups.new_group(select);
            let inner_aspect_ratio = groups.width(g, box_idx) / groups.height(g, box_idx);
            let group_size = place_inner_boxes(
                g,
                groups,
                inner_group,
                obj_spacing,
                Spacing::default(),
                min_width,
                min_height,
                expand_nodes,
                inner_aspect_ratio,
            );
            groups.sizes[inner_group] = group_size;

            pq.add((groups.area(g, inner_group), inner_group));
            for r in remain {
                pq.add((groups.area(g, r), r));
            }
            to_be_placed.clear();
            index = 0;
            cum_area_array.iter_mut().for_each(|v| *v = 0.0);
        } else {
            let (area, idx) = pq.poll().unwrap();
            if index > 0 {
                cum_area_array[index] = cum_area_array[index - 1];
            }
            cum_area_array[index] += area;
            index += 1;
            to_be_placed.push(idx);
        }
    }

    to_be_placed
}

fn merge_and_place_inc(
    g: &mut ElkGraph,
    groups: &mut Groups,
    mut group_list: Vec<usize>,
    obj_spacing: f64,
    min_width: f64,
    min_height: f64,
    expand_nodes: bool,
) -> Vec<usize> {
    group_list.sort_by(|&a, &b| groups.area(g, a).total_cmp(&groups.area(g, b)));

    let mut to_be_placed: Vec<usize> = Vec::new();
    let mut common_area = 0.0f64;

    for gi in group_list {
        if !to_be_placed.is_empty() && groups.area(g, gi) > common_area * 2.0 {
            let merged = groups.new_group(to_be_placed.clone());
            let inner_aspect_ratio = groups.width(g, gi) / groups.height(g, gi);
            let group_size = place_inner_boxes(
                g,
                groups,
                merged,
                obj_spacing,
                Spacing::default(),
                min_width,
                min_height,
                expand_nodes,
                inner_aspect_ratio,
            );
            groups.sizes[merged] = group_size;

            to_be_placed.clear();
            to_be_placed.push(merged);
            to_be_placed.push(gi);
            common_area = groups.area(g, merged) + groups.area(g, gi);
        } else {
            to_be_placed.push(gi);
            common_area += groups.area(g, gi);
        }
    }

    to_be_placed
}
