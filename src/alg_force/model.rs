
use crate::core::javacompat::JavaRandom;
use crate::graph::math::KVector;

use crate::alg_force::graph::{wiggle, FArena, FGraph, FParticleId};
use crate::alg_force::options;

/// factor by which nodes influence the displacement bound.
const DISP_BOUND_FACTOR: f64 = 16.0;

/// Subclass hooks of `AbstractForceModel`.
pub trait ForceModel {
    /// The subclass part of `initialize` (the shared part lives in [`layout`]).
    fn initialize(&mut self, arena: &FArena, graph: &FGraph);

    fn more_iterations(&self, count: i32) -> bool;

    fn calc_displacement(
        &mut self,
        arena: &mut FArena,
        graph: &FGraph,
        random: &mut JavaRandom,
        forcer: FParticleId,
        forcee: FParticleId,
    ) -> KVector;

    /// The subclass part of `iterationDone`.
    fn iteration_done(&mut self);
}

pub fn layout(
    model: &mut dyn ForceModel,
    arena: &mut FArena,
    graph: &mut FGraph,
    random: &mut JavaRandom,
) {
    // ---- AbstractForceModel.initialize ----
    // calculate the adjacency matrix for the graph
    graph.calc_adjacency(arena);

    // calculate an upper bound for particle displacement
    let disp_bound = f64::max(
        graph.nodes.len() as f64 * DISP_BOUND_FACTOR + graph.edges.len() as f64,
        DISP_BOUND_FACTOR * DISP_BOUND_FACTOR,
    );

    // if interactive mode is off, randomize the layout
    if !graph.properties.get(&options::INTERACTIVE) {
        let pos_scale = graph.nodes.len() as f64;
        for &node in &graph.nodes {
            let pos = &mut arena.node_mut(node).position;
            pos.x = random.next_double() * pos_scale;
            pos.y = random.next_double() * pos_scale;
        }
    }

    // create bend points for node repulsion
    for i in 0..graph.edges.len() {
        let edge = graph.edges[i];
        let count: i32 = arena.edge(edge).properties.get(&options::REPULSIVE_POWER);
        if count > 0 {
            for _ in 0..count {
                let bp = arena.create_bendpoint(edge);
                graph.bendpoints.push(bp);
            }
            arena.distribute_bendpoints(edge);
        }
    }

    // subclass initialization
    model.initialize(arena, graph);

    // ---- AbstractForceModel.layout ----
    let mut iterations: i32 = 0;
    while model.more_iterations(iterations) {
        iteration_done(model, arena, graph);

        // calculate attractive and repulsive forces
        let particles = graph.particles();
        for &v in &particles {
            for &u in &particles {
                if u != v {
                    let displacement = model.calc_displacement(arena, graph, random, u, v);
                    arena.displacement_mut(v).add(displacement);
                }
            }
        }

        // apply calculated displacement
        for &v in &particles {
            let mut d = arena.displacement(v);
            d.bound(-disp_bound, -disp_bound, disp_bound, disp_bound);
            arena.position_mut(v).add(d);
            arena.displacement_mut(v).reset();
        }
        iterations += 1;
    }
}

fn iteration_done(model: &mut dyn ForceModel, arena: &mut FArena, graph: &FGraph) {
    for &edge in &graph.edges {
        // adjust label positions
        let labels = arena.edge(edge).labels.clone();
        for label in labels {
            arena.refresh_label_position(label);
        }

        // adjust bend point positions
        arena.distribute_bendpoints(edge);
    }
    model.iteration_done();
}

pub fn avoid_same_position(
    arena: &mut FArena,
    random: &mut JavaRandom,
    u: FParticleId,
    v: FParticleId,
) {
    loop {
        let pu = arena.position(u);
        let pv = arena.position(v);
        if pu.x - pv.x != 0.0 || pu.y - pv.y != 0.0 {
            break;
        }
        if let (FParticleId::Bend(bu), FParticleId::Bend(bv)) = (u, v) {
            // Wiggle orthogonal to edge direction
            let u_edge = arena.bendpoint(bu).edge;
            let mut u_vector = arena.edge_target_point(u_edge);
            u_vector.sub(arena.edge_source_point(u_edge));
            let length = 2.0;
            let orthogonal_u = KVector::new(
                u_vector.x / u_vector.length() * length,
                -u_vector.y / u_vector.length() * length,
            );
            arena.position_mut(u).add(orthogonal_u);

            let v_edge = arena.bendpoint(bv).edge;
            let mut v_vector = arena.edge_target_point(v_edge);
            v_vector.sub(arena.edge_source_point(v_edge));
            // The two freshly allocated KVector objects are compared by
            // reference (`uVector == vVector`), which is always false.
            let length = 2.0;
            let orthogonal_v = KVector::new(
                (v_vector.x / v_vector.length()) * length,
                -(v_vector.y / v_vector.length()) * length,
            );
            arena.position_mut(u).add(orthogonal_v);
        } else {
            wiggle(arena.position_mut(u), random, 1.0);
            wiggle(arena.position_mut(v), random, 1.0);
        }
    }
}

// ------------------------------------------------------------------ Eades

pub struct EadesModel {
    /// the maximal number of iterations after which the model stops.
    max_iterations: i32,
    /// the spring length that determines the optimal distance of connected nodes.
    spring_length: f64,
    /// additional factor for repulsive forces.
    repulsion_factor: f64,
}

/// factor used for force calculations when the distance of two particles is zero.
const ZERO_FACTOR: f64 = 100.0;

impl Default for EadesModel {
    fn default() -> Self {
        EadesModel {
            max_iterations: 300,    // ForceOptions.ITERATIONS default
            spring_length: 80.0,    // ForceOptions.SPACING_NODE_NODE default
            repulsion_factor: 5.0,  // ForceOptions.REPULSION default
        }
    }
}

impl ForceModel for EadesModel {
    fn initialize(&mut self, _arena: &FArena, graph: &FGraph) {
        self.max_iterations = graph.properties.get(&options::ITERATIONS);
        self.spring_length = graph.properties.get(&options::SPACING_NODE_NODE);
        self.repulsion_factor = graph.properties.get(&options::REPULSION);
    }

    fn more_iterations(&self, count: i32) -> bool {
        count < self.max_iterations
    }

    fn calc_displacement(
        &mut self,
        arena: &mut FArena,
        graph: &FGraph,
        random: &mut JavaRandom,
        forcer: FParticleId,
        forcee: FParticleId,
    ) -> KVector {
        avoid_same_position(arena, random, forcer, forcee);

        // compute distance (z in the original algorithm)
        let mut displacement = arena.position(forcee);
        displacement.sub(arena.position(forcer));
        let length = displacement.length();
        let d = f64::max(0.0, length - arena.radius(forcer) - arena.radius(forcee));

        // calculate attractive or repulsive force, depending of adjacency
        let connection = graph.connection(arena, forcer, forcee);
        let force = if connection > 0 {
            -eades_attractive(d, self.spring_length) * connection as f64
        } else {
            let priority: i32 = arena.properties(forcer).get(&options::PRIORITY);
            eades_repulsive(d, self.repulsion_factor) * priority as f64
        };

        // scale distance vector to the amount of repulsive forces
        displacement.scale(force / length);

        displacement
    }

    fn iteration_done(&mut self) {}
}

fn eades_repulsive(d: f64, r: f64) -> f64 {
    if d > 0.0 {
        r / (d * d)
    } else {
        r * ZERO_FACTOR
    }
}

pub fn eades_attractive(d: f64, s: f64) -> f64 {
    if d > 0.0 {
        (d / s).ln()
    } else {
        -ZERO_FACTOR
    }
}

// ---------------------------------------------------- Fruchterman-Reingold

pub struct FruchtermanReingoldModel {
    temperature: f64,
    threshold: f64,
    k: f64,
}

/// factor that determines the C constant used for calculation of K.
const SPACING_FACTOR: f64 = 0.01;

impl Default for FruchtermanReingoldModel {
    fn default() -> Self {
        FruchtermanReingoldModel {
            temperature: 0.001, // ForceOptions.TEMPERATURE default
            threshold: 0.0,
            k: 0.0,
        }
    }
}

impl ForceModel for FruchtermanReingoldModel {
    fn initialize(&mut self, arena: &FArena, graph: &FGraph) {
        self.temperature = graph.properties.get(&options::TEMPERATURE);
        let iterations: i32 = graph.properties.get(&options::ITERATIONS);
        self.threshold = self.temperature / iterations as f64;

        // calculate an appropriate value for K
        let n = graph.nodes.len();
        let mut total_width = 0.0f64;
        let mut total_height = 0.0f64;
        for &v in &graph.nodes {
            total_width += arena.node(v).size.x;
            total_height += arena.node(v).size.y;
        }
        let area = total_width * total_height;
        let c: f64 = graph.properties.get(&options::SPACING_NODE_NODE) * SPACING_FACTOR;
        self.k = (area / (2 * n as i32) as f64).sqrt() * c;
    }

    fn more_iterations(&self, _count: i32) -> bool {
        self.temperature > 0.0
    }

    fn calc_displacement(
        &mut self,
        arena: &mut FArena,
        graph: &FGraph,
        random: &mut JavaRandom,
        forcer: FParticleId,
        forcee: FParticleId,
    ) -> KVector {
        avoid_same_position(arena, random, forcer, forcee);

        // compute distance (z in the original algorithm)
        let mut displacement = arena.position(forcee);
        displacement.sub(arena.position(forcer));
        let length = displacement.length();
        let d = f64::max(0.0, length - arena.radius(forcer) - arena.radius(forcee));

        // calculate repulsive force, independent of adjacency
        let priority: i32 = arena.properties(forcer).get(&options::PRIORITY);
        let mut force = fr_repulsive(d, self.k) * priority as f64;

        // calculate attractive force, depending of adjacency
        let connection = graph.connection(arena, forcer, forcee);
        if connection > 0 {
            force -= fr_attractive(d, self.k) * connection as f64;
        }

        // scale distance vector to the amount of repulsive forces
        displacement.scale(force * self.temperature / length);

        displacement
    }

    fn iteration_done(&mut self) {
        self.temperature -= self.threshold;
    }
}

fn fr_repulsive(d: f64, k: f64) -> f64 {
    if d > 0.0 {
        k * k / d
    } else {
        k * k * ZERO_FACTOR
    }
}

pub fn fr_attractive(d: f64, k: f64) -> f64 {
    d * d / k
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::math::KVector;

    /// Two 10x10 nodes connected by a single edge; ids 0 and 1.
    fn two_node_graph() -> (FArena, FGraph) {
        let mut arena = FArena::default();
        let n0 = arena.create_node(String::new());
        arena.node_mut(n0).id = 0;
        arena.node_mut(n0).size = KVector::new(10.0, 10.0);
        let n1 = arena.create_node(String::new());
        arena.node_mut(n1).id = 1;
        arena.node_mut(n1).size = KVector::new(10.0, 10.0);
        let e = arena.create_edge(n0, n1);
        let mut graph = FGraph::default();
        graph.nodes = vec![n0, n1];
        graph.edges = vec![e];
        (arena, graph)
    }

    /// Hand-traced Eades run, one iteration, seed 1.
    ///
    /// Trace (all IEEE doubles; java.util.Random(1) nextDouble sequence):
    ///   posScale = nodes = 2
    ///   r1..r4 = nextDouble(): n0 = (r1*2, r2*2) = (1.4617563814065817,
    ///   0.8201616229844033), n1 = (r3*2, r4*2) = (0.41542968261943414,
    ///   0.6654341119190224).
    ///   dispBound = max(2*16 + 1, 256) = 256.
    ///   radius(10x10) = sqrt(200)/2 ≈ 7.071 for both nodes, so
    ///   d = max(0, |n0-n1| - 2r) = 0 for every pair: attractive(0, 80)
    ///   returns -ZERO_FACTOR = -100 and connection = 1, hence
    ///   force = -(-100)*1 = 100 for both directions.
    ///   v=n0,u=n1: disp = (n0-n1) * (100/|n0-n1|)  (magnitude 100 < 256)
    ///   v=n1,u=n0: disp = (n1-n0) * (100/|n1-n0|)
    ///   final n0 = (100.38598953240691, 15.448767007180436)
    ///   final n1 = (-98.50880346838089, -13.96317127227701)
    /// (values verified with an independent IEEE-754 simulation;
    /// no transcendental functions are involved since d == 0).
    #[test]
    fn eades_first_iteration_exact() {
        let (mut arena, mut graph) = two_node_graph();
        graph.properties.set(&options::ITERATIONS, 1);
        let mut random = JavaRandom::new(1);
        let mut model = EadesModel::default();
        layout(&mut model, &mut arena, &mut graph, &mut random);

        let n0 = arena.node(graph.nodes[0]).position;
        let n1 = arena.node(graph.nodes[1]).position;
        assert_eq!(n0.x, 100.38598953240691);
        assert_eq!(n0.y, 15.448767007180436);
        assert_eq!(n1.x, -98.50880346838089);
        assert_eq!(n1.y, -13.96317127227701);
    }

    /// Hand-traced Fruchterman-Reingold run, seed 1, TEMPERATURE = 1.0,
    /// ITERATIONS = 2 (threshold = 0.5).
    ///
    /// Trace:
    ///   randomized positions as in the Eades test (same seed):
    ///   n0 = (1.4617563814065817, 0.8201616229844033),
    ///   n1 = (0.41542968261943414, 0.6654341119190224).
    ///   k = sqrt((20*20)/(2*2)) * (80*0.01) = 8.
    ///   Iteration 1: iterationDone runs first, so temperature = 1.0-0.5 = 0.5.
    ///   d = 0 for the pair (radii dominate) => repulsive = k*k*100 = 6400,
    ///   attractive(0, 8) = 0, connection = 1 => force = 6400.
    ///   displacement = (n0-n1) * (6400*0.5/|n0-n1|): magnitude 3200, so both
    ///   components clamp at the displacement bound ±256:
    ///   n0 += (256, 256), n1 += (-256, -256).
    ///   Iteration 2: temperature = 0.5-0.5 = 0 => zero forces, no movement;
    ///   loop exits since temperature is no longer > 0.
    ///   final n0 = (257.4617563814066, 256.8201616229844)
    ///   final n1 = (-255.58457031738055, -255.334565888081)
    #[test]
    fn fruchterman_reingold_first_iteration_exact() {
        let (mut arena, mut graph) = two_node_graph();
        graph.properties.set(&options::TEMPERATURE, 1.0);
        graph.properties.set(&options::ITERATIONS, 2);
        let mut random = JavaRandom::new(1);
        let mut model = FruchtermanReingoldModel::default();
        layout(&mut model, &mut arena, &mut graph, &mut random);

        assert_eq!(model.k, 8.0);
        let n0 = arena.node(graph.nodes[0]).position;
        let n1 = arena.node(graph.nodes[1]).position;
        assert_eq!(n0.x, 257.4617563814066);
        assert_eq!(n0.y, 256.8201616229844);
        assert_eq!(n1.x, -255.58457031738055);
        assert_eq!(n1.y, -255.334565888081);
    }
}
