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
- **audio_path**: a topology-preserving **nodal circuit model** of the 292-style
  passive vactrol ladder (`Vin -Rf- n1 -Rf- n2`, shunt caps C1/C2, terminating Rα,
  optional C3 bridge in Lowpass mode). Each capacitor is a trapezoidal companion
  model (conductance `2C/T` plus a history current source); the two node voltages
  come from a 2x2 modified-nodal-analysis solve per sample. The DC divider
  `Rα/(Rα+2·Rf)` (Eq. 12) falls out of the solve exactly in all three modes.
  Resonance is the Sallen-Key mechanism: the C1 return is a buffered, gained copy
  of the output (`Vfb = K·f(Vout)`), so the buffer nonlinearity sits inside the
  feedback loop; `K = 1 + 2·C2/C1` is the self-oscillation threshold. Modes select
  Rα (5 MΩ Both/Lowpass, 5 kΩ VCA) and C3 engagement.

## Why a companion-model solve and not a direct-form transfer function

The direct-form bilinear transfer function collapses the three capacitor states
to two and **diverges under fast modulation**. The companion-model MNA keeps all
three states (one per cap), and its conductance matrix is passive, so the
per-sample solve is unconditionally stable at any modulation rate with no cutoff
clamp. `tests/stability.rs` guards this; the DC-divider identity is verified
against Eq. 12 directly.

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

The buffer `tanh` sits **inside** the resonance feedback loop (the Sallen-Key C1
return). It is evaluated on the previous (oversampled) output, so the MNA matrix
stays passive and the per-sample solve stays linear and well-posed; the explicit
one-sample feedback is the modelling approximation. Antialiasing comes from
oversampling the whole feedback solve (the halfband runs the ladder `factor` times
per output sample at the finer timestep), with first-order ADAA on the buffer as
an additional reduction. Measured aliasing for the recommended 2x+ADAA config is
~ -61 dB rel fundamental on a 9 kHz full-scale tone at drive 5 (`tests/spectral.rs`).

This is a deliberate trade: the explicit (delayed) feedback keeps the solve cheap
and robust. Replacing it with instantaneous linearisation of the tanh around the
previous operating point (the paper's Taylor approach) would tighten the loop at
the cost of a per-sample relinearisation; that is the documented upgrade path.

## References

- Parker & D'Angelo, "A Digital Model of the Buchla Lowpass-Gate", DAFx-13.
- Zavalishin, "The Art of VA Filter Design".
- Bilbao, Esqueda, Parker, Välimäki, ADAA, IEEE SPL 2017.

## Licensing

Clean-room implementation from the papers; core is `MIT OR Apache-2.0`. Reference
only BSD/MIT code (chowdsp_wdf, Jatin's ADAA) for technique. If the project later
ships inside Cardinal or as a non-exception VCV plugin it becomes GPLv3; that
choice is deferred and does not affect the clean-room core.
