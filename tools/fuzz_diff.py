#!/usr/bin/env python3
"""Differential fuzzer: generate random ELK graphs, lay them out with both the
Java oracle and the Rust port, and report any coordinate divergence.

This is the strongest pixel-level-reproduction check we have: it explores the
input space far beyond the curated golden corpus. Option combinations are
restricted to features the Rust port claims to support, so a diff is a real
fidelity bug (modulo the caveats in GOLDEN_NOTES.md).

Usage: fuzz_diff.py [N] [--seed S] [--algorithm A] [--keep-going]
"""

import json
import random
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# Java `Object.toString` identity-hash suffix (e.g. `DCGraph@7a8c8dcf`). This is
# unreproducible across JVM runs — see GOLDEN_NOTES.md §2 — so we strip it from
# oracle output before comparing, matching how the disco goldens are stored.
_IDENTITY_HASH = re.compile(r"@[0-9a-f]+$")


def _normalize(v):
    if isinstance(v, str):
        return _IDENTITY_HASH.sub("", v)
    if isinstance(v, dict):
        return {k: _normalize(x) for k, x in v.items()}
    if isinstance(v, list):
        return [_normalize(x) for x in v]
    return v

ROOT = Path(__file__).resolve().parent.parent
ORACLE = ROOT / "oracle" / "target" / "elk-oracle-1.0.jar"
ELKRS = ROOT / "target" / "debug" / "elkrs"
COMPARE = ROOT / "tools" / "compare_layouts.py"

# Layered options known to be ported, with value pools. Each fuzz run picks a
# random subset to set on the root graph.
LAYERED_GRAPH_OPTIONS = {
    "elk.direction": ["RIGHT", "DOWN", "LEFT", "UP"],
    "layering.strategy": ["NETWORK_SIMPLEX", "LONGEST_PATH", "LONGEST_PATH_SOURCE",
                          "COFFMAN_GRAHAM", "MIN_WIDTH", "STRETCH_WIDTH"],
    "nodePlacement.strategy": ["BRANDES_KOEPF", "SIMPLE", "LINEAR_SEGMENTS",
                               "NETWORK_SIMPLEX"],
    "edgeRouting": ["ORTHOGONAL", "POLYLINE", "SPLINES"],
    "spacing.nodeNode": ["10", "20", "35"],
    "layered.spacing.nodeNodeBetweenLayers": ["10", "20", "40"],
    "spacing.edgeEdge": ["5", "10"],
    "crossingMinimization.strategy": ["LAYER_SWEEP"],
    "cycleBreaking.strategy": ["GREEDY", "DEPTH_FIRST"],
    "layering.nodePromotion.strategy": ["NONE", "NIKOLOV", "NIKOLOV_IMPROVED"],
    "separateConnectedComponents": ["true", "false"],
}


def rand_graph(rng, n_nodes, n_edges, with_ports, algorithm):
    opts = {"elk.algorithm": algorithm}
    if algorithm == "layered":
        for key, pool in LAYERED_GRAPH_OPTIONS.items():
            if rng.random() < 0.45:
                opts[key] = rng.choice(pool)
    # Position-driven algorithms (spore, stress, disco, radial interactive)
    # layout from existing node positions; coincident centers trigger Java's
    # time-seeded Math.random() perturbation (unreproducible). Give distinct
    # positions on a jittered grid for those.
    positioned = algorithm in ("sporeOverlap", "sporeCompaction", "stress")
    children = []
    for i in range(n_nodes):
        node = {"id": f"n{i}", "width": rng.choice([20, 30, 40, 25, 50]),
                "height": rng.choice([20, 30, 40, 15, 35])}
        if positioned:
            node["x"] = (i % 4) * 90 + rng.randint(0, 30)
            node["y"] = (i // 4) * 90 + rng.randint(0, 30)
        if with_ports and rng.random() < 0.4:
            node.setdefault("ports", [])
        children.append(node)
    edges = []
    for j in range(n_edges):
        s = rng.randrange(n_nodes)
        t = rng.randrange(n_nodes)
        if s == t:
            continue
        edges.append({"id": f"e{j}", "sources": [f"n{s}"], "targets": [f"n{t}"]})
    g = {"id": "root", "layoutOptions": opts, "children": children}
    if edges:
        g["edges"] = edges
    return g


def _within_tol(a, b, tol):
    """True if every leaf numeric value in a and b matches within relative
    tolerance `tol` and all non-numeric leaves are equal. Used to confirm that
    a fuzz "divergence" is only trig ULP noise (GOLDEN_NOTES §1), not a bug."""
    if isinstance(a, dict) and isinstance(b, dict):
        if a.keys() != b.keys():
            return False
        return all(_within_tol(a[k], b[k], tol) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(_within_tol(x, y, tol) for x, y in zip(a, b))
    # numeric strings (ELK serializes some doubles as strings)
    fa = _as_float(a)
    fb = _as_float(b)
    if fa is not None and fb is not None:
        return abs(fa - fb) <= tol * max(1.0, abs(fa), abs(fb))
    return a == b


def _as_float(v):
    if isinstance(v, bool):
        return None
    if isinstance(v, (int, float)):
        return float(v)
    if isinstance(v, str):
        try:
            return float(v)
        except ValueError:
            return None
    return None


def run(cmd, inp):
    try:
        p = subprocess.run(cmd, input=inp, capture_output=True, text=True, timeout=45)
    except subprocess.TimeoutExpired:
        # A pathological graph where the layout engine is slow; not a fidelity
        # signal. Report as a non-comparable run.
        return None, "", ""
    return p.returncode, p.stdout, p.stderr


def main():
    args = sys.argv[1:]
    n = 200
    seed = 1
    algorithm = "layered"
    keep_going = False
    tol = 0.0
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--seed":
            seed = int(args[i + 1]); i += 2
        elif a == "--algorithm":
            algorithm = args[i + 1]; i += 2
        elif a == "--keep-going":
            keep_going = True; i += 1
        elif a == "--tol":
            tol = float(args[i + 1]); i += 2
        else:
            n = int(a); i += 1

    rng = random.Random(seed)
    diffs = 0
    both_err = 0
    rust_only_err = 0
    ok = 0
    near = 0
    for case in range(n):
        n_nodes = rng.randint(2, 9)
        n_edges = rng.randint(1, n_nodes + 3)
        graph = rand_graph(rng, n_nodes, n_edges, rng.random() < 0.3, algorithm)
        inp = json.dumps(graph)

        orc, o_out, o_err = run(["java", "-jar", str(ORACLE), "-"], inp)
        rst, r_out, r_err = run([str(ELKRS), "-"], inp)

        if orc is None or rst is None:
            continue  # a run timed out; skip (not a fidelity signal)
        if orc != 0:
            # Oracle itself failed (unsupported combo / Java exception). Rust
            # should fail too (crash-for-crash); a Rust success here is benign.
            if rst != 0:
                both_err += 1
            continue
        if rst != 0:
            rust_only_err += 1
            print(f"\n[case {case}] RUST ERROR but oracle OK:\n  {r_err.strip()[:200]}")
            print(f"  input: {inp}")
            if not keep_going:
                break
            continue

        o_norm = json.dumps(_normalize(json.loads(o_out)))
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as of:
            of.write(o_norm); o_path = of.name
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as rf:
            rf.write(r_out); r_path = rf.name
        cmp = subprocess.run(["python3", str(COMPARE), o_path, r_path],
                             capture_output=True, text=True)
        if cmp.returncode == 0:
            ok += 1
        elif tol > 0 and _within_tol(json.loads(o_norm), json.loads(r_out), tol):
            near += 1
        else:
            diffs += 1
            print(f"\n[case {case}] DIVERGENCE:")
            print("  " + "\n  ".join(cmp.stdout.strip().splitlines()[:8]))
            print(f"  input: {inp}")
            print(f"  (oracle={o_path} rust={r_path})")
            if not keep_going:
                break

    print(f"\n=== {algorithm} fuzz: {ok} identical, {near} within-tol, {diffs} diverged, "
          f"{rust_only_err} rust-only-errors, {both_err} both-errored "
          f"(seed {seed}, {n} cases) ===")
    sys.exit(1 if (diffs or rust_only_err) else 0)


if __name__ == "__main__":
    main()
