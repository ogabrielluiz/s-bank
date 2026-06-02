# Vactrol LPG vertical slice: design notes

This crate is the portable DSP core of a virtual-analog Buchla-292-style vactrol
low-pass gate. It deliberately has **no VCV Rack SDK dependency** so the entire
test and benchmark pipeline runs headless. A future thin C++ adapter
(`vcv-adapter/`) links the staticlib over the C ABI in `src/ffi.rs`.

## Architecture (per sample)

```
CV in --> control_path --> If (LED current) --> vactrol --> Rf (ohms)
                                                              |
audio in -----------------------------------------> audio_path (TPT 2-pole) --> out
```

- **control_path**: smooths CV (one-pole) then maps it to LED current with a
  smooth saturating curve fit. Upgrade path: the Lambert-W / piecewise log-amp
  from Parker & D'Angelo.
- **vactrol**: target resistance from the datasheet power law `Rf = A/If^1.4 + B`,
  followed by an asymmetric, state-dependent one-pole (fast attack, slow decay).
- **audio_path**: topology-preserving (TPT/ZDF) 2-pole state-variable filter, one
  linear solve per sample. `Rf` sets the cutoff (Both/Lowpass) and, via the
  potential divider `Rα/(Rα+2·Rf)`, the DC gain. Modes: Both (couples brightness
  and amplitude), VCA (amplitude gate), Lowpass (Sallen-Key filter).

## Why TPT and not a direct-form transfer function

The direct-form bilinear transfer function collapses the three capacitor states
to two and **diverges under fast modulation**. The TPT structure preserves the
states and is stable under any rate of modulation. `tests/stability.rs` is the
guard for this property.

## Status

- **Phase 0**: workspace scaffolding. Done.
- **Phase 1**: DSP core + vactrol model + tests. Done.
- **Phase 2**: first-order ADAA + polyphase halfband oversampling (1x/2x/4x) +
  spectral/aliasing tests. Done.
- **Phase 3**: imperfection layer (per-instance tolerance, drift, thermal, noise
  floor), seedable and serializable. Done.
- **Phase 4**: golden-file management (`reference` module, `bless`, tolerance
  comparison) + smoke/correctness/spectral tests. Done.
- **Phase 5**: benchmark suite (per-config, voices, worst-case vs typical). Done.
- **Phase 6**: tiered CI design documented in `docs/CI.md` (YAML ready to add;
  see the note there about the `workflows` permission).
- **Phase 7**: VCV adapter. Placeholder only (`vcv-adapter/`).

## Antialiasing notes

The buffer `tanh` sits at the audio-path output (memoryless, outside the
resonance feedback), so first-order ADAA is exact here, not an approximation.
Oversampling targets that same memoryless stage: the linear SVF runs at the base
rate (it generates no aliasing). When `drive == 0` the nonlinear stage is bypassed
so the linear path has no oversampling latency. Measured aliasing for the
recommended 2x+ADAA config is ~ -42 dB rel fundamental on a 9 kHz full-scale tone
at drive 5 (`tests/spectral.rs`), with 4x improving on that.

## References

- Parker & D'Angelo, "A Digital Model of the Buchla Lowpass-Gate", DAFx-13.
- Zavalishin, "The Art of VA Filter Design".
- Bilbao, Esqueda, Parker, Välimäki, ADAA, IEEE SPL 2017.

## Licensing

Clean-room implementation from the papers; core is `MIT OR Apache-2.0`. Reference
only BSD/MIT code (chowdsp_wdf, Jatin's ADAA) for technique. If the project later
ships inside Cardinal or as a non-exception VCV plugin it becomes GPLv3; that
choice is deferred and does not affect the clean-room core.
