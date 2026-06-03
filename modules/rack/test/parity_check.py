#!/usr/bin/env python3
"""Compare the C++ port's output to the Rust golden buffers (sample-for-sample).

Usage: parity_check.py <parity_binary>
Passes if every case's max abs diff is < 0.1% of the case peak — i.e. the C++ DSP is
numerically faithful to the Rust reference (residual = std-lib transcendental ULP).
"""
import json
import math
import subprocess
import sys
from pathlib import Path

GOLD = Path(__file__).resolve().parents[3] / "testdata" / "golden"
TOL = 1e-3  # fraction of peak

# (case arg, golden file)
CASES = [
    ("lpg:pluck_both", "pluck_both.json"),
    ("lpg:vca_tone", "vca_tone.json"),
    ("lpg:lowpass_sweep", "lowpass_sweep.json"),
    ("strike:ping", "strike_ping.json"),
    ("strike:gated", "strike_gated.json"),
    ("strike:held", "strike_held.json"),
]


def main() -> int:
    binary = sys.argv[1]
    ok = True
    for case, gfile in CASES:
        gold = json.load(open(GOLD / gfile))
        out = subprocess.run([binary, case], capture_output=True, text=True).stdout.split()
        cpp = [float(x) for x in out]
        if len(cpp) != len(gold):
            print(f"FAIL {case}: length {len(cpp)} != golden {len(gold)}")
            ok = False
            continue
        peak = max(1e-9, max(abs(v) for v in gold))
        maxd = max(abs(g - c) for g, c in zip(gold, cpp))
        rel = maxd / peak
        passed = rel < TOL
        ok = ok and passed
        print(f"{'PASS' if passed else 'FAIL'} {case:22} peak {peak:.4f}  maxdiff {maxd:.2e} ({rel*100:.4f}% of peak)")
    print("C++<->Rust DSP parity:", "PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
