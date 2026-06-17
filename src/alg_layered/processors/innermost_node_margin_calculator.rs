
use crate::alg_layered::graph::{LGraphArena, LGraphId};
use crate::alg_layered::lgraph_adapters::LGraphAdapter;

pub fn process(a: &mut LGraphArena, graph: LGraphId) -> Result<(), String> {
    let mut adapter = LGraphAdapter::new(a, graph, false, false, |_, _| true);
    crate::alg_common::nodespacing::calculate_node_margins(&mut adapter, true);
    Ok(())
}
