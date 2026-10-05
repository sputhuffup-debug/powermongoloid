#!/usr/bin/env python3
"""Flag criterion benchmarks that regressed against the saved baseline.

Reads target/criterion/**/change/estimates.json, written when a run is
compared with a baseline (`cargo bench -- --save-baseline nightly`). A
benchmark regresses when the lower bound of its 95% confidence interval for
the change in mean time is above the threshold, so ordinary runner noise
does not trip it. SEED.md "Testing requirements": flag regressions over 10%.

Usage: check_bench_regression.py [criterion_dir] [--threshold 0.10]
Exit status 1 if any benchmark regressed.
"""

import json
import os
import sys
from pathlib import Path


def main() -> int:
    args = sys.argv[1:]
    threshold = 0.10
    if "--threshold" in args:
        i = args.index("--threshold")
        threshold = float(args[i + 1])
        del args[i : i + 2]
    root = Path(args[0] if args else "target/criterion")

    rows = []
    for est in sorted(root.glob("**/change/estimates.json")):
        bench = est.parent.parent.relative_to(root).as_posix()
        mean = json.loads(est.read_text())["mean"]
        rows.append(
            (
                bench,
                mean["point_estimate"],
                mean["confidence_interval"]["lower_bound"],
                mean["confidence_interval"]["upper_bound"],
            )
        )

    if not rows:
        print(f"No baseline comparisons under {root}; nothing to check (first run?).")
        return 0

    lines = [
        f"| Benchmark | Change in mean | 95% CI | Status |",
        f"| --- | --- | --- | --- |",
    ]
    regressed = []
    for bench, point, lo, hi in rows:
        bad = lo > threshold
        if bad:
            regressed.append(bench)
            print(f"::error title=Benchmark regression::{bench} is {point:+.1%} slower (95% CI {lo:+.1%} to {hi:+.1%})")
        lines.append(f"| `{bench}` | {point:+.1%} | {lo:+.1%} to {hi:+.1%} | {'REGRESSED' if bad else 'ok'} |")

    report = "\n".join(lines)
    print(report)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a") as f:
            f.write(f"## Benchmarks vs previous nightly (threshold +{threshold:.0%})\n\n{report}\n")

    if regressed:
        print(f"\n{len(regressed)} benchmark(s) regressed by more than {threshold:.0%}.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
