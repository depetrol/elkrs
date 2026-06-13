//! Assembles a fully-registered ELK instance (all ported algorithms).

pub fn create_elk() -> elk_core::Elk {
    let mut elk = elk_core::Elk::new();
    elk_alg_layered::provider::register(&mut elk.options, &mut elk.algorithms);
    elk_alg_force::register(&mut elk.options, &mut elk.algorithms);
    elk_alg_radial::register(&mut elk.options, &mut elk.algorithms);
    elk_alg_rectpacking::register(&mut elk.options, &mut elk.algorithms);
    elk_alg_mrtree::register(&mut elk.options, &mut elk.algorithms);
    elk_alg_topdownpacking::register(&mut elk.options, &mut elk.algorithms);
    elk
}
