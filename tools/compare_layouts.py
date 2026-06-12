#!/usr/bin/env python3
"""Structurally compare two ELK JSON layouts (oracle vs elkrs).

Objects are compared as unordered maps; arrays in order; numbers exactly
(both sides print shortest-roundtrip doubles). Reports every difference
with a JSON path. Exit code 0 iff identical.
"""

import json
import sys


def compare(a, b, path, diffs):
    if isinstance(a, dict) and isinstance(b, dict):
        for k in a:
            if k not in b:
                diffs.append(f"{path}.{k}: missing in rust output (oracle: {a[k]!r})")
            else:
                compare(a[k], b[k], f"{path}.{k}", diffs)
        for k in b:
            if k not in a:
                diffs.append(f"{path}.{k}: extra in rust output ({b[k]!r})")
    elif isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            diffs.append(f"{path}: array length {len(a)} != {len(b)}")
        for i, (x, y) in enumerate(zip(a, b)):
            compare(x, y, f"{path}[{i}]", diffs)
    elif isinstance(a, bool) or isinstance(b, bool):
        if a != b:
            diffs.append(f"{path}: {a!r} != {b!r}")
    elif isinstance(a, (int, float)) and isinstance(b, (int, float)):
        if float(a) != float(b):
            diffs.append(f"{path}: {a!r} != {b!r}")
    elif a != b:
        diffs.append(f"{path}: {a!r} != {b!r}")


def main():
    with open(sys.argv[1]) as f:
        oracle = json.load(f)
    with open(sys.argv[2]) as f:
        rust = json.load(f)
    diffs = []
    compare(oracle, rust, "$", diffs)
    if diffs:
        for d in diffs[:50]:
            print(d)
        if len(diffs) > 50:
            print(f"... and {len(diffs) - 50} more")
        sys.exit(1)
    print("IDENTICAL")


if __name__ == "__main__":
    main()
