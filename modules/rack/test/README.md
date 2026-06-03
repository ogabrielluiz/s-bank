# C++ DSP tests

The shipping plugin's DSP is **native C++** (`../src/dsp/SBankDSP.hpp`). These tests
lock its sound in place: a **golden regression** that fails if the output drifts from
the committed reference buffers, plus a finite/sane smoke check.

## Run

```sh
./run_golden.sh          # compile golden_dump.cpp, check DSP output vs the committed goldens
./run_golden.sh --bless  # regenerate the goldens from the current DSP (only after an
                         # intentional sound change)
c++ -std=c++11 -I ../src dsp_smoke.cpp -o /tmp/smoke && /tmp/smoke   # finite/sane smoke
```

## What the golden regression checks

`golden_dump.cpp` renders fixed scenarios with the DSP; `golden_check.py` compares the
output sample-for-sample against `testdata/golden/`:

- **Vactrol**: `pluck_both`, `vca_tone`, `lowpass_sweep`
- **Strike**: `ping`, `gated`, `held`

The goldens were captured from the DSP itself (`./run_golden.sh --bless`). The check
tolerance is **0.1 % of peak**, which absorbs cross-platform libm differences in
`sinf`/`tanhf`/`expf` while still catching any real change to the algorithm.

## Out of scope: the imperfection layer

The golden regression covers the deterministic DSP — the default sound. The optional
**analogue imperfection** layer (per-instance random tolerance / drift) is *not* golden-
tested, by design:

- **Vactrol**: `VactrolLpg` omits the imperfection layer entirely — the module never
  exposed an imperfection control, so there is no user-facing effect.
- **Strike**: `StrikeImperfection` is seeded random dirt — finite, bounded, and
  deterministic from its seed (verified). Because it is intentionally random and has no
  single "correct" buffer, a sample-for-sample golden is neither meaningful nor required.
