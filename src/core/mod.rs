//! Rust port of `org.eclipse.elk.core`: layout option metadata, the
//! recursive layout engine, basic layout providers, and ELK JSON I/O.

pub mod adapters;
pub mod data;
pub mod elkutil;
pub mod engine;
pub mod enum_helpers;
pub mod javacompat;
pub mod json;
pub mod options;
pub mod options_gen;
pub mod providers;
pub mod registry;
pub mod util;

use data::LayoutMetaDataRegistry;
use registry::AlgorithmRegistry;

/// Convenience bundle of option metadata + algorithm registries with
/// everything from elk-core registered.
pub struct Elk {
    pub options: LayoutMetaDataRegistry,
    pub algorithms: AlgorithmRegistry,
}

impl Default for Elk {
    fn default() -> Self {
        Self::new()
    }
}

impl Elk {
    pub fn new() -> Self {
        let mut options = LayoutMetaDataRegistry::default();
        options_gen::register_core_options(&mut options);
        let mut algorithms = AlgorithmRegistry::default();
        providers::register_core_algorithms(&mut algorithms);
        Elk { options, algorithms }
    }

    /// Parse ELK JSON, lay out, and export laid-out JSON (oracle-compatible).
    pub fn layout_json(&self, input: &str) -> Result<serde_json::Value, String> {
        let value: serde_json::Value =
            serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
        let mut importer = json::JsonImporter::new(&self.options);
        let mut graph = importer.import_graph(&value)?;
        let engine = engine::RecursiveGraphLayoutEngine::new(&self.algorithms);
        engine.layout(&mut graph)?;
        let mut exporter = json::JsonExporter::new(&self.options);
        Ok(exporter.export(&graph))
    }
}
