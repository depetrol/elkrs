//! Temporary stub (created to unblock a concurrent build; the owning agent's
//! real implementation will overwrite this file).
use crate::graph::{LGraphArena, LGraphId};
pub fn process(_a: &mut LGraphArena, _graph: LGraphId) -> Result<(), String> {
    Err("TODO: HORIZONTAL_COMPACTOR not ported yet".to_string())
}
