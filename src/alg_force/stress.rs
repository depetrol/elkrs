
use crate::core::registry::LayoutProvider;
use crate::graph::graph::{ElkGraph, NodeId};
use crate::graph::math::KVector;

use crate::alg_force::graph::{FArena, FEdgeId, FGraph, FNodeId};
use crate::alg_force::options::{self, Dimension};
use crate::alg_force::provider::{execute_node_micro_layout, force_layout};
use crate::alg_force::{components, importer};

#[derive(Default)]
pub struct StressLayoutProvider;

impl LayoutProvider for StressLayoutProvider {
    fn layout(&mut self, g: &mut ElkGraph, layout_node: NodeId) -> Result<(), String> {
        // calculate initial coordinates
        if !g.node(layout_node).properties.get(&options::INTERACTIVE) {
            force_layout(g, layout_node)?;
        } else if !g
            .node(layout_node)
            .properties
            .get(&options::OMIT_NODE_MICRO_LAYOUT)
        {
            // If requested, compute nodes's dimensions, place node labels,
            // ports, port labels, etc. (handled by the force provider in the
            // non-interactive case above).
            execute_node_micro_layout(g, layout_node);
        }

        // transform the input graph
        let (mut arena, fgraph) = importer::import_graph(g, layout_node)?;

        // split the input graph into components
        let components = components::split(&mut arena, fgraph);

        // perform the actual layout
        let mut stress_majorization = StressMajorization::default();
        for sub_graph in &components {
            if sub_graph.nodes.len() <= 1 {
                continue;
            }
            stress_majorization.initialize(&arena, sub_graph);
            stress_majorization.execute(&mut arena, sub_graph);

            // Note that contrary to force itself, labels are not considered
            // during stress layout. Hence, all we can do here is to place the
            // labels at reasonable positions after layout has finished.
            for &label in &sub_graph.labels {
                arena.refresh_label_position(label);
            }
        }

        // pack the components back into one graph
        let fgraph = components::recombine(&mut arena, components);

        // apply the layout results to the original graph
        importer::apply_layout(&arena, &fgraph, g, layout_node);

        Ok(())
    }
}

#[derive(Default)]
pub struct StressMajorization {
    /// All pairs shortest path matrix, indexed by `FNode.id`.
    apsp: Vec<Vec<f64>>,
    /// Weights for each pair of nodes.
    w: Vec<Vec<f64>>,
    /// Common desired edge length, can be overridden by individual edges.
    desired_edge_length: f64,
    /// Dimensions to consider during layout.
    dim: Dimension,
    /// Epsilon for terminating the stress minimizing process.
    epsilon: f64,
    /// Maximum number of iterations (overrides the epsilon).
    iteration_limit: i32,
    /// Edges connected to each node, by `FNode.id`.
    connected_edges: Vec<Vec<FEdgeId>>,
}

impl StressMajorization {
    pub fn initialize(&mut self, arena: &FArena, graph: &FGraph) {
        if graph.nodes.len() <= 1 {
            return;
        }

        self.dim = graph.properties.get(&options::DIMENSION);
        self.iteration_limit = graph.properties.get(&options::ITERATION_LIMIT);
        self.epsilon = graph.properties.get(&options::EPSILON);
        self.desired_edge_length = graph.properties.get(&options::DESIRED_EDGE_LENGTH);

        let n = graph.nodes.len();
        self.connected_edges = vec![Vec::new(); n];
        for &edge in &graph.edges {
            let e = arena.edge(edge);
            self.connected_edges[arena.node(e.source).id as usize].push(edge);
            self.connected_edges[arena.node(e.target).id as usize].push(edge);
        }

        // all pairs shortest path
        self.apsp = vec![vec![0.0; n]; n];
        for &source in &graph.nodes {
            let sid = arena.node(source).id as usize;
            let mut dist = std::mem::take(&mut self.apsp[sid]);
            self.dijkstra(arena, graph, source, &mut dist);
            self.apsp[sid] = dist;
        }

        // init weight matrix (the diagonal becomes 1/0 = +inf;
        // it is never read)
        self.w = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                let dij = self.apsp[i][j];
                let wij = 1.0 / (dij * dij);
                self.w[i][j] = wij;
            }
        }
    }

    pub fn execute(&mut self, arena: &mut FArena, graph: &FGraph) {
        if graph.nodes.len() <= 1 {
            return;
        }

        let mut count: i32 = 0;
        let mut prev_stress = self.compute_stress(arena, graph);
        let mut cur_stress = f64::INFINITY;

        loop {
            if count > 0 {
                prev_stress = cur_stress;
            }

            for &u in &graph.nodes {
                // note that we do not use 'NO_LAYOUT' here, since that option
                // results in the node already being excluded by the layout engine
                if arena.node(u).properties.get(&options::FIXED) {
                    continue;
                }

                let new_pos = self.compute_new_position(arena, graph, u);
                let pos = &mut arena.node_mut(u).position;
                pos.reset();
                pos.add(new_pos);
            }

            cur_stress = self.compute_stress(arena, graph);

            let done = self.done(count, prev_stress, cur_stress);
            count += 1;
            if done {
                break;
            }
        }
    }

    fn dijkstra(&self, arena: &FArena, graph: &FGraph, source: FNodeId, dist: &mut [f64]) {
        let mut queue = JavaPriorityQueue::default();
        let mut mark = vec![false; graph.nodes.len()];

        // init
        let sid = arena.node(source).id as usize;
        dist[sid] = 0.0;
        for &node in &graph.nodes {
            let id = arena.node(node).id as usize;
            if id != sid {
                dist[id] = 2147483647.0; // Integer.MAX_VALUE
            }
            queue.add((node, id), dist);
        }

        // find shortest paths
        while let Some((u, uid)) = queue.poll(dist) {
            mark[uid] = true;

            for &e in &self.connected_edges[uid] {
                let v = get_other(arena, e, u);
                let vid = arena.node(v).id as usize;
                if mark[vid] {
                    continue;
                }
                // get e's desired length
                let el = arena
                    .edge(e)
                    .properties
                    .try_get(&options::DESIRED_EDGE_LENGTH)
                    .unwrap_or(self.desired_edge_length);
                let d = dist[uid] + el;
                if d < dist[vid] {
                    dist[vid] = d;
                    queue.remove(v, dist);
                    queue.add((v, vid), dist);
                }
            }
        }
    }

    fn done(&self, count: i32, prev_stress: f64, cur_stress: f64) -> bool {
        prev_stress == 0.0
            || ((prev_stress - cur_stress) / prev_stress) < self.epsilon
            || count >= self.iteration_limit
    }

    fn compute_stress(&self, arena: &FArena, graph: &FGraph) -> f64 {
        let mut stress = 0.0;
        let nodes = &graph.nodes;
        for i in 0..nodes.len() {
            let u = arena.node(nodes[i]);
            for j in (i + 1)..nodes.len() {
                let v = arena.node(nodes[j]);
                let euc_dist = u.position.distance(v.position);
                let euc_displacement = euc_dist - self.apsp[u.id as usize][v.id as usize];
                stress += self.w[u.id as usize][v.id as usize]
                    * euc_displacement
                    * euc_displacement;
            }
        }
        stress
    }

    fn compute_new_position(&self, arena: &FArena, graph: &FGraph, u: FNodeId) -> KVector {
        let mut weight_sum = 0.0;
        let mut x_disp = 0.0;
        let mut y_disp = 0.0;

        // we need at least two nodes here, otherwise we would divide by zero below
        debug_assert!(graph.nodes.len() > 1);

        let un = arena.node(u);
        let uid = un.id as usize;
        for &v in &graph.nodes {
            if u == v {
                continue;
            }
            let vn = arena.node(v);
            let vid = vn.id as usize;

            let wij = self.w[uid][vid];
            weight_sum += wij;

            let euc_dist = un.position.distance(vn.position);

            if euc_dist > 0.0 && self.dim != Dimension::Y {
                x_disp += wij
                    * (vn.position.x
                        + self.apsp[uid][vid] * (un.position.x - vn.position.x) / euc_dist);
            }

            if euc_dist > 0.0 && self.dim != Dimension::X {
                y_disp += wij
                    * (vn.position.y
                        + self.apsp[uid][vid] * (un.position.y - vn.position.y) / euc_dist);
            }
        }

        match self.dim {
            Dimension::X => KVector::new(x_disp / weight_sum, un.position.y),
            Dimension::Y => KVector::new(un.position.x, y_disp / weight_sum),
            Dimension::XY => KVector::new(x_disp / weight_sum, y_disp / weight_sum),
        }
    }
}

fn get_other(arena: &FArena, edge: FEdgeId, one: FNodeId) -> FNodeId {
    let e = arena.edge(edge);
    if e.source == one {
        e.target
    } else if e.target == one {
        e.source
    } else {
        panic!("Node 'one' must be either source or target of edge 'edge'.");
    }
}

/// `java.util.PriorityQueue` with the comparator
/// `(n1, n2) -> Double.compare(dist[n1.id], dist[n2.id])`, with exact
/// sift/removeAt semantics (the comparator reads the live `dist` array,
/// so it is passed into every operation). Entries are `(node, id)` pairs.
#[derive(Default)]
struct JavaPriorityQueue {
    heap: Vec<(FNodeId, usize)>,
}

impl JavaPriorityQueue {
    fn cmp(dist: &[f64], a: (FNodeId, usize), b: (FNodeId, usize)) -> std::cmp::Ordering {
        // Double.compare
        dist[a.1].total_cmp(&dist[b.1])
    }

    fn add(&mut self, x: (FNodeId, usize), dist: &[f64]) {
        self.heap.push(x);
        let k = self.heap.len() - 1;
        self.sift_up(k, x, dist);
    }

    fn poll(&mut self, dist: &[f64]) -> Option<(FNodeId, usize)> {
        if self.heap.is_empty() {
            return None;
        }
        let result = self.heap[0];
        let x = self.heap.pop().unwrap();
        if !self.heap.is_empty() {
            self.sift_down(0, x, dist);
        }
        Some(result)
    }

    /// `remove(Object)`: linear `indexOf` (object identity), then `removeAt`.
    fn remove(&mut self, node: FNodeId, dist: &[f64]) {
        if let Some(i) = self.heap.iter().position(|&(n, _)| n == node) {
            self.remove_at(i, dist);
        }
    }

    fn remove_at(&mut self, i: usize, dist: &[f64]) {
        let s = self.heap.len() - 1;
        if s == i {
            // removed last element
            self.heap.pop();
        } else {
            let moved = self.heap[s];
            self.heap.pop();
            self.sift_down(i, moved, dist);
            if self.heap[i] == moved {
                self.sift_up(i, moved, dist);
            }
        }
    }

    fn sift_up(&mut self, mut k: usize, x: (FNodeId, usize), dist: &[f64]) {
        while k > 0 {
            let parent = (k - 1) >> 1;
            let e = self.heap[parent];
            if Self::cmp(dist, x, e) != std::cmp::Ordering::Less {
                break;
            }
            self.heap[k] = e;
            k = parent;
        }
        self.heap[k] = x;
    }

    fn sift_down(&mut self, mut k: usize, x: (FNodeId, usize), dist: &[f64]) {
        let n = self.heap.len();
        let half = n >> 1;
        while k < half {
            let mut child = 2 * k + 1;
            let mut c = self.heap[child];
            let right = child + 1;
            if right < n && Self::cmp(dist, c, self.heap[right]) == std::cmp::Ordering::Greater {
                child = right;
                c = self.heap[child];
            }
            if Self::cmp(dist, x, c) != std::cmp::Ordering::Greater {
                break;
            }
            self.heap[k] = c;
            k = child;
        }
        self.heap[k] = x;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stress majorization on a 3-node path with default options
    /// (desiredEdgeLength = 100, epsilon = 10e-4, iterationLimit = MAX).
    ///
    /// Trace: nodes start at (0,0), (10,0), (20,0) with edges 0-1, 1-2.
    /// Dijkstra yields apsp = [[0,100,200],[100,0,100],[200,100,0]] exactly,
    /// w[i][j] = 1/apsp²  (1e-4 and 2.5e-5). All y-displacements stay exactly
    /// 0. The majorization loop converges after 16 iterations to
    ///   n0 = -139.99999999999994, n1 = -39.999999999999964,
    ///   n2 = 60.000000000000036
    /// (values verified with an independent IEEE-754 simulation; only +,*,/
    /// and sqrt are involved, which are exactly rounded).
    #[test]
    fn stress_majorization_converges_exactly() {
        let mut arena = FArena::default();
        let mut graph = FGraph::default();
        let coords = [0.0, 10.0, 20.0];
        let mut nodes = Vec::new();
        for (i, &x) in coords.iter().enumerate() {
            let n = arena.create_node(String::new());
            arena.node_mut(n).id = i as i32;
            arena.node_mut(n).position = KVector::new(x, 0.0);
            arena.node_mut(n).size = KVector::new(10.0, 10.0);
            nodes.push(n);
            graph.nodes.push(n);
        }
        graph.edges.push(arena.create_edge(nodes[0], nodes[1]));
        graph.edges.push(arena.create_edge(nodes[1], nodes[2]));

        let mut sm = StressMajorization::default();
        sm.initialize(&arena, &graph);
        assert_eq!(sm.apsp[0], vec![0.0, 100.0, 200.0]);
        assert_eq!(sm.apsp[1], vec![100.0, 0.0, 100.0]);
        assert_eq!(sm.apsp[2], vec![200.0, 100.0, 0.0]);

        sm.execute(&mut arena, &graph);

        let p0 = arena.node(nodes[0]).position;
        let p1 = arena.node(nodes[1]).position;
        let p2 = arena.node(nodes[2]).position;
        assert_eq!(p0.x, -139.99999999999994);
        assert_eq!(p1.x, -39.999999999999964);
        assert_eq!(p2.x, 60.000000000000036);
        assert_eq!(p0.y, 0.0);
        assert_eq!(p1.y, 0.0);
        assert_eq!(p2.y, 0.0);

        // converged to (almost exactly) the desired edge lengths
        assert!((p0.distance(p1) - 100.0).abs() < 1e-9);
        assert!((p1.distance(p2) - 100.0).abs() < 1e-9);
    }
}
