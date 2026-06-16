# Golden fidelity notes

This file records every known way the Rust port can diverge from the Java
ELK 0.11.0 oracle, and why. The golden corpus (`goldens/`) is curated to
avoid inputs that trigger these, so `cargo test -p elk-cli` is byte-exact;
the notes exist so future cases are chosen with the caveats in mind.

The oracle is the ELK 0.11.0 release jars (`oracle/`), run on JDK 17. The
vendored `elk/` tree is 0.12.0-SNAPSHOT and is a *reading* reference only —
where 0.11.0 and 0.12 differ, the 0.11.0 sources in `tools/elk-sources/`
(downloaded source jars) and the oracle's behavior win.

## 1. Transcendental functions (radial, force, mrtree, disco, splines)

`Math.sin`, `Math.cos`, `Math.log` are **not** guaranteed correctly-rounded
by either the JVM or any libm, and HotSpot's intrinsics differ from Apple
libm / musl by up to 1 ULP on a minority of inputs. `Math.atan2`, `Math.acos`,
`Math.sqrt`, and all of `+ - * /` ARE correctly rounded and match bit-for-bit.

Consequence: algorithms that feed `sin`/`cos`/`log` results into coordinates
(radial node placement, Eades force model's attractive term, mrtree, disco
edge contours, spline control points) can show ~1-ULP coordinate noise on
adversarial inputs. The committed goldens were verified to be unaffected for
their specific inputs. The differential fuzzer (`tools/fuzz_diff.py --tol`)
quantifies the residual: across random radial trees, ~93% are bit-exact and
the rest match within 1 ULP (relative 1e-9); mrtree and disco are bit-exact
100%. A pixel-perfect guarantee for the last few radial ULPs would require
shipping a bit-exact `fdlibm` port; deferred.

## 2. Identity-hash-ordered Java collections

Several ELK code paths iterate a `HashSet`/`HashMap` keyed by object identity
(or by `hashCode()` that falls back to identity), whose order is
JVM-run-dependent — i.e. **Java itself is not reproducible** there. The Rust
port uses deterministic insertion order. Affected, with the analysis that the
order is layout-invariant in practice:

- **disco** `discoGraph` debug property prints `DCGraph@<identityhash>`; the
  hash is unreproducible. The disco golden *expected* files have the `@hash`
  suffix stripped (the only post-processing of any oracle output). All other
  disco output, including the `discoPolys` grid-art string, is byte-exact.
- **spore** Bowyer–Watson triangulation feeds a `HashSet` into a *stable* sort
  in `NaiveMinST`, so set order is layout-visible. This is handled by a
  bit-exact JDK-8 `HashSet`/`HashMap` replica (`elk-alg-common/src/jhash.rs`,
  with Java `Double.hashCode`/`KVector.hashCode`), so spore IS reproducible.
- layered crossing-minimization `graphsWhoseNodeOrderChanged`, BK class-graph
  map, edge-label group map, hyperedge corner tie-break, spline segment sets:
  all proven order-invariant for the result (ties are broken by a
  deterministic secondary key, or the consumed value is order-insensitive).

## 3. `java.util.Random` with seed 0

Several algorithms (random layouter, layered, force) treat `randomSeed == 0`
as "use `new Random()`", which the JVM seeds from the system clock —
unreproducible across Java runs too. The Rust port substitutes a fixed seed.
For any **non-zero** seed the port is bit-exact (`JavaRandom` is a verified
`java.util.Random` LCG replica). Goldens use explicit non-zero seeds.

## 4. Crash-for-crash, not output-for-output

Where Java throws an unchecked exception (NPE on top-level external ports,
`StackOverflowError` on degenerate mrtree, `IllegalArgumentException` for
`edgeRoutingMode=NONE`, network-simplex placer on certain flexible-size
graphs), the Rust port panics / returns `Err` under the *same* conditions
rather than reproducing the exception text. These are unreachable for
well-formed inputs.

## 5. Deliberate engine simplifications

- `ILabelManager` (dynamic label shortening) is never configured here, so the
  CENTER/END label-management processors are no-ops — matching the oracle when
  no manager is set.
- `topdownLayout=true` engine mode is unsupported (errors); the topdownpacking
  *algorithm* is ported and exact in standalone use.
- `underlyingLayoutAlgorithm`/`componentLayoutAlgorithm` (disco) which fetch
  another algorithm from the global service are unsupported; unset (default)
  behaves identically.

## 7. Compound layered: external-port edge cases

Cross-hierarchy edges and external ports (`INCLUDE_CHILDREN`) are ported and
byte-exact for the committed goldens (`layered_xhier_*`, `layered_extport_*`)
and for all flat layered fuzz inputs. Recursive-hierarchy graphs whose nested
nodes have external ports (the `ComponentGroupGraphPlacer` path) are also
byte-exact — see `layered_extport_components` (ELK's `Issue680Test`) and the
`layered_extcomp_*` goldens (N/S, E/W, and mixed-side multi-component cases).
Two narrow compound cases are known to diverge or are deliberately unreached:

- **Merged external port → multiple interior nodes.** When a *single* boundary
  port is the source/target of several edges to different children of the same
  compound node, the children's vertical order can differ from the oracle. Only
  bites at ≥4 children: k=2,3 are byte-exact; for k=4 the oracle orders
  `c3,c1,c2,c4` while the port produces `c3,c4,c1,c2` (oracle appends new
  targets at the tail; the port inserts them after the head). The order is
  **deterministic** — identical across `randomSeed` 1/2/7/42 — so it is *not* a
  `JavaRandom` desync (an exhaustive scan of every stream offset reproduces the
  oracle for no single k). Root cause: in the hierarchical sweep the oracle
  orders that children layer via the `preOrdered` barycenter-fill path
  (`calculateBarycenters` + recursive-midpoint `fillInUnknownBarycenters` — the
  k=8 fingerprint `c3,c1,c6,c7,c2,c8,c4,c5` is its signature), whereas the Rust
  port reaches the `randomize=true` first-layer path (`setFirstLayerOrder`).
  Both `BarycenterHeuristic` ports are byte-faithful in isolation; the two reach
  a *different* `minimizeCrossings(preOrdered, randomize, forward)` branch for
  this single-port-feeds-many layer. Pinpointing the branch needs a Java-side
  trace (a `-javaagent`/instrumented ELK build logging the per-nested-graph
  sweep flags), since the shared hierarchical-sweep path is exercised by all
  `INCLUDE_CHILDREN` goldens and a blind change risks regressing them. Independent
  edges (one port per child) are byte-exact; the layered fuzzer generates flat
  graphs only, so it never hits this.
- **`nodeLabels.placement` echo under `direction=UP`.** A node with an explicit
  `NODE_LABELS_PLACEMENT` (e.g. `[H_CENTER, V_TOP, INSIDE]`) laid out with
  `direction=UP` has the *echoed* placement option flipped to `V_BOTTOM` by the
  oracle's `GraphTransformer` (the two direction passes do not round-trip the
  enum). The Rust port round-trips it back to `V_TOP`. This is **cosmetic**: the
  label and node *geometry* are byte-identical (verified for all four
  directions — ELK's own `Issue682Test` asserts only geometry, which passes).
  Only the echoed config string differs, and only for UP. RIGHT/DOWN/LEFT are
  fully byte-exact (`layered_nodelabel_*` goldens).
- **`restoreDummy` PORT_LABELS branch** (N/S external-port-label margin
  recomputation in the orthogonal router) returns `Err` rather than
  recomputing — unreached by any input with N/S external ports carrying labels
  under a `PORT_LABELS` size constraint.

## 6. Double formatting

The exporter prints doubles via `fmt_java_double`, matching Java
`Double.toString` for integral values (`12.0`) and relying on Rust's
shortest-round-trip `{}` for the rest, which equals Java's algorithm. The
golden comparator (`tools/compare_layouts.py`, and the in-test comparator)
compares parsed numeric values, so representation never causes a false diff.
