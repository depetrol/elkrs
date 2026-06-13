# elkrs — ELK in native Rust

Goal: rewrite the Eclipse Layout Kernel in Rust with pixel-level output parity
against Java ELK, replicating its test suite.

## Reference

- `elk/` — vendored Java source (0.12.0-SNAPSHOT), the porting source of truth.
- `oracle/` — Java CLI on ELK 0.11.0 jars: `java -jar oracle/target/elk-oracle-1.0.jar graph.json`
  reads ELK JSON, runs `RecursiveGraphLayoutEngine`, prints laid-out JSON.
  Used to generate golden outputs.

## Architecture (Cargo workspace, `crates/`)

| Crate | Mirrors | Status |
|---|---|---|
| `elk-graph` | org.eclipse.elk.graph (model, properties, KVector math, JSON) | done |
| `elk-core` | org.eclipse.elk.core (options, engine, fixed/box/random layouters) | done (topdown layout deferred) |
| `elk-alg-common` | org.eclipse.elk.alg.common (node sizing, polyomino, compaction) | todo |
| `elk-alg-layered` | org.eclipse.elk.alg.layered (62k lines — main effort) | todo |
| `elk-alg-force` | force + stress | todo |
| `elk-alg-mrtree` | mrtree | todo |
| `elk-alg-radial` | radial | todo |
| `elk-alg-rectpacking` | rectpacking | todo |
| `elk-alg-spore` | spore | todo |
| `elk-alg-disco` | disco | todo |
| `elk-alg-topdownpacking` | topdownpacking | todo |
| `elk-alg-vertiflex` | vertiflex | todo |
| `elk-cli` | mirrors oracle CLI for diffing | todo |

## Testing strategy

1. **Golden tests** (`goldens/`): input graphs + oracle output JSON; Rust CLI
   output must match coordinates exactly (tolerance 1e-9, effectively bit-equal
   doubles). Corpus grows with each ported feature.
2. **Ported unit tests**: JUnit tests from `elk/test/**` rewritten as Rust
   `#[test]`s in each crate (~190 files).

## Fidelity rules (learned/important)

- Java ELK relies on deterministic iteration: `LinkedHashMap/LinkedHashSet`
  → use `indexmap`; `ArrayList` → `Vec`. Plain `HashMap` in Java code paths
  must be checked individually for order sensitivity.
- All geometry is `f64`; replicate Java `Math` exactly (`f64` ops are IEEE).
- `Random` (random layouter, greedy switch tie-breaks): replicate
  `java.util.Random` LCG bit-for-bit.
- Double formatting in JSON: Java prints `12.0`; match shape where tests
  compare strings (tests should compare parsed numbers instead).
- Oracle is 0.11.0 jars; vendored source is 0.12.0-SNAPSHOT. Where outputs
  diverge, vendored source wins; note divergences in GOLDEN_NOTES.md.

## Porting order

1. elk-graph: KVector/KVectorChain/ElkMargin/ElkPadding/ElkRectangle, property
   system, graph model, ElkGraphUtil, JSON import/export. + graph tests.
2. elk-core: option metadata (CoreOptions), LayoutMetaDataService,
   RecursiveGraphLayoutEngine, ElkUtil, FixedLayouter, BoxLayouter,
   RandomLayouter, node label/size calculation entry points. + core tests.
3. elk-alg-common: NodeDimensionCalculation/NodeLabelAndSizeCalculator etc.
   (pull in pieces as layered needs them).
4. elk-alg-layered, phase by phase: graph import/transform → cycle breaking →
   layering → crossing minimization → node placement → edge routing →
   post-processing. Golden-test after each intermediate processor using full
   pipelines on small graphs.
5. Remaining algorithms in rough size order: radial, rectpacking, force,
   mrtree, spore, topdownpacking, vertiflex, disco.

## Progress log

- 2026-06-12: repo surveyed; maven + oracle driver built and verified on
  3-node layered graph.
- 2026-06-12: elk-graph + elk-core done. 10 golden cases (box simple/grouped/
  expand/priority/hierarchy, fixed incl. orthogonal junction points, random
  with seed) pixel-identical to oracle. Key Java semantics replicated:
  materializing getProperty for Cloneable defaults (RefCell PropertyMap),
  float casts (1.3f as f64), java.util.Random LCG, PriorityQueue heap order.
- Next: elk-alg-layered. Port order: graph model (LGraph/LNode/LPort/LEdge,
  arena, typed NodeType) -> options (LayeredOptions via generator,
  InternalProperties manual) -> ElkGraphImporter -> ElkLayered driver +
  GraphConfigurator + processor enum (unported processors error with their
  name) -> port processors demanded by default pipeline until basic layered
  golden passes -> expand corpus (ports, labels, hierarchy, edge routing
  variants) porting more processors/phases -> remaining strategies.

## Milestone: 120 goldens, 9 algorithms, 145 unit tests 2026-06-13

All 9 ELK algorithms registered & pixel-exact on 120 golden cases: fixed,
box, random, layered (default + directions + ports incl. inverted/north-south/
external-import + self loops + labels + comments + layer constraints + all
layering strategies [NS/longest-path(+source)/coffman/minwidth/stretch] +
all placers [BK/simple/linear-segments/network-simplex] + node promotion +
orthogonal/polyline/spline routing), force, stress, radial, rectpacking,
mrtree, spore (overlap+compaction), disco, topdownpacking. Wave-1 JUnit
unit tests replicated (145 tests). GOLDEN_NOTES.md records known caveats.

In flight (wave 4 agents): compound/hierarchical layered + external-port
processors + transferrer recursion + component group placers; hyperedge/
hypernode/high-degree/horizontal-compactor (+compaction subpackage);
wrapping (single/multi-edge, unzipper) + partition + model-order
(cycle breakers, layerers, barycenter heuristic).

Then: white-box JUnit processor tests (elk/test layered suite ~96 files),
interactive strategies, broader differential fuzzing, final sweep of
remaining TODO arms.

## Milestone: 63/63 goldens pixel-identical, 9 algorithms 2026-06-12

Algorithms done (pixel-exact vs 0.11.0 oracle): fixed, box, random, layered
(default pipeline + directions + ports incl. FIXED_*/inverted/north-south +
self loops + node labels + layer constraints + LONGEST_PATH + SIMPLE placer),
force, stress, radial, rectpacking, mrtree. `cargo test -p elk-cli` runs the
63-case golden suite. Next wave (agents launched, check completions):
spore+disco, topdownpacking+vertiflex, layered polyline/spline routers,
layered remaining strategies (COFFMAN_GRAHAM/MIN_WIDTH/STRETCH_WIDTH/
LINEAR_SEGMENTS/NETWORK_SIMPLEX placer), layered edge labels
(LABEL_DUMMY_*, END_LABEL_*), layered hierarchical+external ports.
After that: model order/wrapping/compaction strategies, JUnit test
replication (elk/test/**), GOLDEN_NOTES for known ULP/trig caveats.

## Milestone: 19/19 goldens pixel-identical (incl. 6 layered) 2026-06-12

Default layered pipeline fully ported and bit-exact vs oracle. Next fronts:
1. Broaden layered coverage: directions (DOWN/UP/LEFT), explicit ports +
   FIXED_ORDER/POS/RATIO/SIDE, node/port/edge labels, self loops,
   polyline/spline routing, COFFMAN_GRAHAM/MIN_WIDTH/STRETCH_WIDTH layering,
   LINEAR_SEGMENTS/NETWORK_SIMPLEX/SIMPLE placement, node promotion,
   compaction, model order, wrapping, external ports, INCLUDE_CHILDREN.
   Add golden cases per feature; port missing processors as errors surface.
2. Remaining algorithms: mrtree, radial, rectpacking, spore, disco,
   topdownpacking, vertiflex (agents work well; force/stress already done).
3. Replicate elk JUnit test suites (elk/test/**) as Rust tests.

## Current state (layered port, 2026-06-12)

elk-alg-layered structure in place: importer (flat graphs), configurator
(pipeline assembly identical to AlgorithmAssembler), driver, components
processor (SimpleRowGraphPlacer), spacings, transferrer, provider
(registered via elk_cli::create_elk()). Ported processors: edge/layer
constraint reversers+pre/post, long edge splitter/joiner, port side/list,
in-layer constraints, layer size calc, reversed edge restorer, greedy cycle
breaker. PORT FROM tools/elk-sources (0.11.0!), NOT the vendored elk/ repo
(which is 0.12.0-SNAPSHOT) — oracle is 0.11.0 jars.

Background agents were porting (check git status / their output when
resuming): nodespacing -> elk-alg-common (entry points
calculate_label_and_node_sizes, calculate_node_margins, process_node_size,
compute_inside_node_label_padding; wire into processors/
innermost_node_margin_calculator.rs + label_and_node_size_processor.rs and
importer's compute_inside_node_label_padding); network simplex + layerers
-> p2layers; layer sweep stack -> p3order; BK placer -> p4nodes; orthogonal
router -> p5edges. After integration: cargo test goldens with
goldens/cases/layered_*.json (add cases), debug divergences vs oracle
(tools/compare_layouts.py /tmp/oracle.json /tmp/rust.json).

## Deferred/known gaps (revisit before declaring completion)

- Layered: hierarchical (INCLUDE_CHILDREN) import/layout/transfer; external
  ports; LayeredSpacings.withBaseValue; model-order strategies; label
  management; interactive strategies; polyline/spline routing; wrapping;
  compaction; EndLabelSorter is a no-op stub (fine without end labels).

- Topdown layout (engine errors out) and DeprecatedLayoutOptionReplacer.
- IndividualSpacings JSON: importer creates empty holder; values parse OK.
- elk-cli exporter always uses oracle flag set (omit nothing, full keys).
- Random layouter with seed=0: Java is time-seeded; we use seed 1.
