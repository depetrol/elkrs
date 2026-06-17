//! elkrs — a native-Rust, byte-exact reimplementation of the Eclipse Layout
//! Kernel (ELK) graph layout algorithms.

pub mod graph;
pub mod core;
pub mod alg_common;
pub mod alg_layered;
pub mod alg_force;
pub mod alg_radial;
pub mod alg_rectpacking;
pub mod alg_mrtree;
pub mod alg_spore;
pub mod alg_disco;
pub mod alg_topdownpacking;

/// Assembles a fully-registered ELK instance (all ported algorithms).
pub fn create_elk() -> crate::core::Elk {
    let mut elk = crate::core::Elk::new();
    crate::alg_layered::provider::register(&mut elk.options, &mut elk.algorithms);
    crate::alg_force::register(&mut elk.options, &mut elk.algorithms);
    crate::alg_radial::register(&mut elk.options, &mut elk.algorithms);
    crate::alg_rectpacking::register(&mut elk.options, &mut elk.algorithms);
    crate::alg_mrtree::register(&mut elk.options, &mut elk.algorithms);
    crate::alg_topdownpacking::register(&mut elk.options, &mut elk.algorithms);
    crate::alg_spore::register(&mut elk.options, &mut elk.algorithms);
    crate::alg_disco::register(&mut elk.options, &mut elk.algorithms);
    elk
}
