//! Phase 5: edge routing (`org.eclipse.elk.alg.layered.p5edges`).

pub mod orthogonal;

use elk_core::options::EdgeRouting;
use elk_graph::properties::EnumSet;

use elk_core::javacompat::JavaRandom;

use crate::graph::{LGraphArena, LGraphId};
use crate::internal_properties as iprops;
use crate::options_gen as lopts;
use crate::options_gen::GraphProperties;
use crate::phases::{IntermediateProcessorStrategy as Ips, LayeredPhases, ProcessorConfiguration};

/// Java `EdgeRouterFactory.factoryFor`: maps the edge routing option to the
/// router actually used (UNDEFINED and ORTHOGONAL map to orthogonal).
pub fn effective_routing(routing: EdgeRouting) -> EdgeRouting {
    match routing {
        EdgeRouting::POLYLINE => EdgeRouting::POLYLINE,
        EdgeRouting::SPLINES => EdgeRouting::SPLINES,
        _ => EdgeRouting::ORTHOGONAL,
    }
}

pub fn processor_configuration(
    routing: EdgeRouting,
    a: &LGraphArena,
    graph: LGraphId,
    config: &mut ProcessorConfiguration,
) -> Result<(), String> {
    match effective_routing(routing) {
        EdgeRouting::ORTHOGONAL => {
            let graph_properties: EnumSet<GraphProperties> =
                a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);

            if graph_properties.contains(GraphProperties::HYPEREDGES) {
                config.add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::HYPEREDGE_DUMMY_MERGER);
                config.add_before(LayeredPhases::P3_NODE_ORDERING, Ips::INVERTED_PORT_PROCESSOR);
            }
            if graph_properties.contains(GraphProperties::NON_FREE_PORTS)
                || a.graph(graph).properties.get(&lopts::FEEDBACK_EDGES)
            {
                config.add_before(LayeredPhases::P3_NODE_ORDERING, Ips::INVERTED_PORT_PROCESSOR);
                if graph_properties.contains(GraphProperties::NORTH_SOUTH_PORTS) {
                    config
                        .add_before(
                            LayeredPhases::P3_NODE_ORDERING,
                            Ips::NORTH_SOUTH_PORT_PREPROCESSOR,
                        )
                        .add_after(
                            LayeredPhases::P5_EDGE_ROUTING,
                            Ips::NORTH_SOUTH_PORT_POSTPROCESSOR,
                        );
                }
            }
            if graph_properties.contains(GraphProperties::EXTERNAL_PORTS) {
                config
                    .add_before(
                        LayeredPhases::P3_NODE_ORDERING,
                        Ips::HIERARCHICAL_PORT_CONSTRAINT_PROCESSOR,
                    )
                    .add_before(
                        LayeredPhases::P4_NODE_PLACEMENT,
                        Ips::HIERARCHICAL_PORT_DUMMY_SIZE_PROCESSOR,
                    )
                    .add_after(
                        LayeredPhases::P5_EDGE_ROUTING,
                        Ips::HIERARCHICAL_PORT_ORTHOGONAL_EDGE_ROUTER,
                    );
            }
            if graph_properties.contains(GraphProperties::SELF_LOOPS) {
                config
                    .add_before(LayeredPhases::P1_CYCLE_BREAKING, Ips::SELF_LOOP_PREPROCESSOR)
                    .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::SELF_LOOP_POSTPROCESSOR)
                    .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::SELF_LOOP_PORT_RESTORER)
                    .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::SELF_LOOP_ROUTER);
            }
            if graph_properties.contains(GraphProperties::HYPERNODES) {
                config.add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::HYPERNODE_PROCESSOR);
            }
            if graph_properties.contains(GraphProperties::CENTER_LABELS) {
                config
                    .add_before(LayeredPhases::P2_LAYERING, Ips::LABEL_DUMMY_INSERTER)
                    .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::LABEL_DUMMY_SWITCHER)
                    .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::LABEL_SIDE_SELECTOR)
                    .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::LABEL_DUMMY_REMOVER);
            }
            if graph_properties.contains(GraphProperties::END_LABELS) {
                config
                    .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::LABEL_SIDE_SELECTOR)
                    .add_before(LayeredPhases::P4_NODE_PLACEMENT, Ips::END_LABEL_PREPROCESSOR)
                    .add_after(LayeredPhases::P5_EDGE_ROUTING, Ips::END_LABEL_POSTPROCESSOR);
            }
            Ok(())
        }
        other => Err(format!("TODO: edge routing {other:?} is not ported yet")),
    }
}

pub fn process(
    routing: EdgeRouting,
    a: &mut LGraphArena,
    graph: LGraphId,
    _random: &mut JavaRandom,
) -> Result<(), String> {
    match effective_routing(routing) {
        EdgeRouting::ORTHOGONAL => orthogonal::process(a, graph),
        other => Err(format!("TODO: edge routing {other:?} is not ported yet")),
    }
}
