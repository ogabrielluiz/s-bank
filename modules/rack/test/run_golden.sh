#!/usr/bin/env bash
# Golden regression test for the C++ DSP.
#
#   ./run_golden.sh            compile + check the DSP output against the committed goldens
#   ./run_golden.sh --bless    regenerate the goldens from the current DSP (do this only
#                              when you have *intentionally* changed the sound)
#
# Goldens (testdata/golden/*.json) are the known-good DSP output, captured from the DSP
# itself; the test fails if the DSP drifts from them. golden_dump.cpp reproduces each
# scenario; golden_check.py compares sample-for-sample.
set -euo pipefail
cd "$(dirname "$0")"
GOLD="$(cd ../../.. && pwd)/testdata/golden"
BIN="${TMPDIR:-/tmp}/sbank_golden_$$"
trap 'rm -f "$BIN"' EXIT
c++ -std=c++11 -O2 -I ../src golden_dump.cpp -o "$BIN"

# "case-arg:golden-file"
CASES=(
  "lpg:pluck_both:pluck_both"
  "lpg:vca_tone:vca_tone"
  "lpg:lowpass_sweep:lowpass_sweep"
  "strike:ping:strike_ping"
  "strike:gated:strike_gated"
  "strike:held:strike_held"
)

if [[ "${1:-}" == "--bless" ]]; then
  for entry in "${CASES[@]}"; do
    arg="${entry%:*}"; file="${entry##*:}"
    "$BIN" "$arg" | python3 -c "import sys,json; print(json.dumps([float(x) for x in sys.stdin.read().split()]))" > "$GOLD/$file.json"
    echo "blessed $file.json"
  done
  exit 0
fi

python3 golden_check.py "$BIN"
