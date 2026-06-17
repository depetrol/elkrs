
use crate::alg_layered::graph::{LGraphArena, LNodeId};
use crate::alg_layered::internal_properties as iprops;
use crate::alg_layered::options_gen as lopts;
use crate::alg_layered::options_gen::LayerConstraint;

/// Helper to compute constraint-aware (group) model order. Reset between uses.
#[derive(Default)]
pub struct GroupModelOrderCalculator {
    first_separate_nodes: i32,
    last_separate_nodes: i32,
}

impl GroupModelOrderCalculator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn compute_constraint_model_order(
        &mut self,
        a: &LGraphArena,
        node: LNodeId,
        offset: i32,
    ) -> i32 {
        let mut model_order = self.constraint_base(a, node, offset);
        if a.node(node).properties.has(&iprops::MODEL_ORDER) {
            model_order =
                model_order.wrapping_add(a.node(node).properties.get(&iprops::MODEL_ORDER));
        }
        model_order
    }

    pub fn compute_constraint_group_model_order(
        &mut self,
        a: &LGraphArena,
        node: LNodeId,
        offset: i32,
        small_offset: i32,
    ) -> i32 {
        let mut model_order = self.constraint_base(a, node, offset);
        if a.node(node).properties.has(&iprops::MODEL_ORDER) {
            let group_id = a
                .node(node)
                .properties
                .get(&lopts::CONSIDER_MODEL_ORDER_GROUP_MODEL_ORDER_CYCLE_BREAKING_ID);
            model_order = model_order
                .wrapping_add(group_id.wrapping_mul(small_offset))
                .wrapping_add(a.node(node).properties.get(&iprops::MODEL_ORDER));
        }
        model_order
    }

    fn constraint_base(&mut self, a: &LGraphArena, node: LNodeId, offset: i32) -> i32 {
        match a.node(node).properties.get::<LayerConstraint>(&lopts::LAYERING_LAYER_CONSTRAINT) {
            LayerConstraint::FIRST_SEPARATE => {
                let v = 2i32.wrapping_mul(-offset).wrapping_add(self.first_separate_nodes);
                self.first_separate_nodes += 1;
                v
            }
            LayerConstraint::FIRST => -offset,
            LayerConstraint::LAST => offset,
            LayerConstraint::LAST_SEPARATE => {
                let v = 2i32.wrapping_mul(offset).wrapping_add(self.last_separate_nodes);
                self.last_separate_nodes += 1;
                v
            }
            LayerConstraint::NONE => 0,
        }
    }

    pub fn reset_internal_counters(&mut self) {
        self.first_separate_nodes = 0;
        self.last_separate_nodes = 0;
    }
}
