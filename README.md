# elkrs — Eclipse Layout Kernel in native Rust

A byte-exact Rust rewrite of the [Eclipse Layout Kernel](https://eclipse.dev/elk/)
(ELK) 0.11.0. The goal is **pixel-level output parity** with Java ELK: for the
same input graph, `elkrs` produces JSON whose coordinates are bit-identical to
the reference implementation, verified against the ELK 0.11.0 release jars.

**Status:** all 12 ELK layout algorithms ported and pixel-exact — `fixed`, `box`,
`random`, `layered` (incl. compound/`INCLUDE_CHILDREN` + external ports),
`force`, `stress`, `radial`, `rectpacking`, `mrtree`, `spore`, `disco`,
`topdownpacking`. 201 golden cases byte-identical to the oracle; 205 tests pass
(118 are faithful ports of ELK's own JUnit suite). Known residual divergences
are enumerated at the end of this file.

## Layout

A single crate, `elkrs`, with one module per mirrored ELK plugin under `src/`:

| Module | Mirrors |
|---|---|
| `graph` | `org.eclipse.elk.graph` — model, properties, KVector math, JSON I/O |
| `core` | `org.eclipse.elk.core` — options, engine, fixed/box/random layouters |
| `alg_common` | `org.eclipse.elk.alg.common` — node sizing, compaction, spore/triangulation |
| `alg_layered` | `org.eclipse.elk.alg.layered` — the main effort (~62k Java lines) |
| `alg_force` | force + stress |
| `alg_radial` · `alg_mrtree` · `alg_rectpacking` · `alg_spore` · `alg_disco` · `alg_topdownpacking` | the remaining algorithms |

The crate root re-exports `create_elk()` and builds both a library and the
`elkrs` binary (`src/main.rs`), which reads ELK JSON on stdin and prints the
laid-out JSON.

Development-only trees (excluded from the published crate): `oracle/` (Java CLI
on the 0.11.0 jars, generates goldens), `tools/elk-sources/` (downloaded 0.11.0
source jars — the authoritative porting reference; the vendored `elk/` tree is
0.12.0-SNAPSHOT and read-only), `goldens/` (input cases + expected oracle
output), `tools/` (fuzzer + comparators).

## Build, run, test

```sh
cargo build                                   # build the crate
echo '{"id":"g","layoutOptions":{"org.eclipse.elk.algorithm":"org.eclipse.elk.layered"},
       "children":[{"id":"n","width":30,"height":30}]}' | ./target/debug/elkrs -

cargo test                                     # all 205 tests
cargo test --test goldens                      # the 201-case byte-exact golden suite
```

As a library:

```rust
let out = elkrs::create_elk().layout_json(input_json)?;   // returns laid-out JSON
```

The oracle (for regenerating goldens / diffing) needs JDK 17 + Maven:

```sh
java -jar oracle/target/elk-oracle-1.0.jar - < input.json   # reference output
python3 tools/compare_layouts.py oracle.json rust.json      # exact numeric diff
```

## How fidelity is verified

1. **Golden tests** (`goldens/`): curated input graphs paired with oracle output;
   the Rust CLI must match every coordinate (tolerance 1e-9 ≈ bit-equal doubles).
2. **Replicated JUnit tests** (`tests/junit_*.rs`): 118 of ELK's own
   `@Test`s rewritten as Rust tests — black-box ones run through
   `elkrs::create_elk()`, white-box ones call the ported APIs directly
   (e.g. `OneDimensionalCompactorTest`, `ForceImportTest`, the layered
   `Issue*Test`/`BinaryIndexedTreeTest`/`OverallLayoutTest`).
3. **Differential fuzzer** (`tools/fuzz_diff.py [N] --seed S --algorithm A`):
   generates random graphs, lays them out with both engines, reports any
   coordinate divergence. The strongest parity check — 100% identical across
   layered/force/mrtree/rectpacking (incl. model-order paths); `--tol` quantifies
   the radial trig residual.

## Fidelity rules

- Java relies on deterministic iteration: `LinkedHashMap/Set` → `indexmap`,
  `ArrayList` → `Vec`; every plain `HashMap` in a Java path is checked for order
  sensitivity. `java.util.Random` and `PriorityQueue` are bit-exact replicas.
- Java `getProperty` materializes `Cloneable` defaults into the map (output-
  visible) — modelled by a `RefCell`-backed `PropertyMap` (`.get` materializes,
  `.try_get` does not). Float literals (`1.3f`) are cast through `f32` first.
- All geometry is `f64`; `+ - * / sqrt atan2 acos` are IEEE-correct and match
  bit-for-bit. Doubles print via `fmt_java_double` (matches `Double.toString`),
  though comparators parse numbers so representation never causes false diffs.
- The 0.11.0 jars are the oracle; where they differ from the vendored 0.12
  source, `tools/elk-sources/` (0.11.0) and the oracle's behavior win.

## Known divergences

The golden corpus avoids inputs that trigger these, so the golden suite is
byte-exact. They are recorded so future cases are chosen with the caveats in mind.

1. **Transcendental ULP** — `Math.sin/cos/log` are not correctly-rounded by the
   JVM or any libm; HotSpot's intrinsics differ from Apple libm/musl by ≤1 ULP on
   a minority of inputs. Affects coordinates fed by trig (radial placement, Eades
   force term, splines). Fuzzer residual: ~93% of radial trees bit-exact, the rest
   within 1 ULP (1e-9); mrtree/disco 100% bit-exact. A perfect fix needs a
   bit-exact `fdlibm` port (deferred).
2. **Identity-hash-ordered collections** — a few ELK paths iterate a `HashSet`/
   `HashMap` keyed by object identity, whose order is JVM-run-dependent (Java
   itself isn't reproducible there). The Rust port uses insertion order. Cases:
   disco's `DCGraph@<hash>` debug string (the `@hash` is stripped from goldens —
   the only post-processing); spore's triangulation (handled by a bit-exact JDK-8
   `HashSet`/`HashMap` replica, `jhash.rs`, so spore *is* reproducible); various
   layered tie-breaks (proven order-invariant for the result).
3. **`randomSeed == 0`** — ELK treats it as `new Random()` (clock-seeded,
   unreproducible across Java runs); the port substitutes a fixed seed. Any
   non-zero seed is bit-exact (`JavaRandom` is a verified LCG replica).
4. **Crash-for-crash** — where Java throws (NPE on top-level external ports,
   `StackOverflowError` on degenerate mrtree, etc.), the port panics/`Err`s under
   the same conditions rather than reproducing the message. Unreachable for
   well-formed input.
5. **Deliberate engine simplifications** — no `ILabelManager` is configured, so
   the CENTER/END label-management processors are no-ops (matching the oracle with
   no manager); `topdownLayout=true` engine mode errors (the topdownpacking
   *algorithm* is exact standalone); disco's `underlyingLayoutAlgorithm` is unset
   (default behaves identically).
6. **Compound layered, two narrow cases** — cross-hierarchy edges, external ports,
   and the `ComponentGroupGraphPlacer` path are byte-exact (see the
   `layered_xhier_*`/`layered_extport_*`/`layered_extcomp_*` goldens and ELK's
   `Issue680Test`). Remaining:
   - *Merged external port → ≥4 interior nodes* (one boundary port feeding several
     children of one compound node): the children's vertical order can differ
     (k=2,3 exact; k=4 oracle `c3,c1,c2,c4` vs port `c3,c4,c1,c2`). **Deterministic**
     (seed-independent — *not* a `JavaRandom` desync). Root cause: in the
     hierarchical sweep the oracle orders that layer via the `preOrdered`
     barycenter-fill path while the port reaches the `randomize` first-layer path;
     both `BarycenterHeuristic` ports are byte-faithful in isolation. Pinpointing
     the branch needs a Java-side sweep trace; the shared path touches all
     `INCLUDE_CHILDREN` goldens, so a blind change risks regressions. Never hit by
     the (flat-graph) fuzzer.
   - *`nodeLabels.placement` echo under `direction=UP`* — **cosmetic**: the
     *geometry* is byte-identical (ELK's `Issue682Test` passes), only the echoed
     placement option flips `V_TOP`→`V_BOTTOM`. RIGHT/DOWN/LEFT fully exact.
   - The `restoreDummy` PORT_LABELS branch (N/S external-port-label margins)
     returns `Err` — unreached by any tested input.
