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

## 6. Double formatting

The exporter prints doubles via `fmt_java_double`, matching Java
`Double.toString` for integral values (`12.0`) and relying on Rust's
shortest-round-trip `{}` for the rest, which equals Java's algorithm. The
golden comparator (`tools/compare_layouts.py`, and the in-test comparator)
compares parsed numeric values, so representation never causes a false diff.
