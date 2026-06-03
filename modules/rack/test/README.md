# C++ DSP tests & parity

The shipping plugin's DSP is **native C++** (`../src/dsp/SBankDSP.hpp`). The Rust
crates in `components/` are kept as the **reference implementation and golden source**;
these tests prove the C++ port matches them.

## Run

```sh
./run_parity.sh          # compile parity.cpp, check C++ output vs the Rust goldens
./run_parity.sh --bless  # regenerate the Strike goldens from the Rust reference
c++ -std=c++11 -I ../src dsp_smoke.cpp -o /tmp/smoke && /tmp/smoke   # finite/sane smoke
```

## What parity checks

`parity.cpp` reproduces the Rust golden scenarios with the C++ port; `parity_check.py`
compares sample-for-sample against `testdata/golden/`:

- **Vactrol**: `pluck_both`, `vca_tone`, `lowpass_sweep` (from `cargo run -p vactrol-harness -- bless`).
- **Strike**: `ping`, `gated`, `held` (from the `strike-core` `parity_dump` example).

The deterministic signal paths match the Rust reference to **< 0.002 % of peak** (the
residual is just `sinf`/`tanhf`/`expf` ULP differences between the two std libs).

## Known intentional divergence: the imperfection layer

Parity covers the deterministic DSP (the default sound). The optional **analogue
imperfection** layer is deliberately *not* bit-identical to Rust:

- **Vactrol**: the C++ `VactrolLpg` omits the imperfection layer entirely — the vactrol
  module never exposed an imperfection control, so there is zero user-facing effect.
- **Strike**: the C++ `StrikeImperfection` uses SplitMix64/xorshift where the Rust core
  used ChaCha8 for the per-instance tolerance. It is finite, bounded, and deterministic
  from its seed (verified), so it is a valid analogue-dirt feature — it just produces a
  *different* random fingerprint than Rust. For random dirt with no golden, bit-parity
  is neither required nor meaningful.
