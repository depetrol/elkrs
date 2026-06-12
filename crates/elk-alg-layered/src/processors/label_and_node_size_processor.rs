//! Port of `LabelAndNodeSizeProcessor`.

use elk_graph::properties::EnumSet;

use crate::graph::{LGraphArena, LGraphId, NodeType};
use crate::internal_properties as iprops;
use crate::lgraph_adapters::LGraphAdapter;
use crate::options_gen::GraphProperties;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    // Java: NodeDimensionCalculation.calculateLabelAndNodeSizes(
    //           LGraphAdapters.adapt(graph, true, true, NORMAL-filter))
    {
        let mut adapter = LGraphAdapter::new(a, graph, true, true, |arena, n| {
            arena.node(n).node_type == NodeType::NORMAL
        });
        elk_alg_common::nodespacing::calculate_label_and_node_sizes(&mut adapter, |_, _| true);
    }

    let graph_properties: EnumSet<GraphProperties> =
        a.graph(graph).properties.get(&iprops::GRAPH_PROPERTIES);
    if graph_properties.contains(GraphProperties::EXTERNAL_PORTS) {
        return Err(
            "TODO: external port dummy label placement is not ported yet".to_string(),
        );
    }
    Ok(())
}
