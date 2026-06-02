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
- **audio_path**: the Parker & D'Angelo **3-capacitor state-space 292 model**,
  ported from the authors' own reference code (see `docs/REFERENCES.md`). States
  for C1/C2/C3; coefficients `a1=1/(C1 Rf)`, `a2=-(1/Rf+1/R3)/C1`,
  `b1=b3=1/(Rf C2)`, `b2=-2/(Rf C2)`, `b4=C3/C2`, `d1=a`, `d2=-1`. The delay-free
  loop is solved in closed form (the `Dx`/`Do`/`Dmas` Schur-complement factors);
  the `tanh` resonance nonlinearity is linearised about the previous output. The
  DC divider `R3/(R3+2·Rf)` falls out of the solve exactly. Resonance is the
  feedback gain `a` clamped to the exact `amax`; C3 is switched out in VCA mode.

## Why the topology-preserving state-space and not a direct-form transfer function

The direct-form bilinear transfer function collapses the three capacitor states
to two and **diverges under fast modulation**. The state-space form keeps all
three states (one per cap) and is solved as a stable delay-free loop, so it stays
finite at any modulation rate with no cutoff clamp. `tests/stability.rs` guards
this; the DC-divider identity was verified numerically against `R3/(R3+2·Rf)`.

> Earlier revisions used a generic SVF, then a clean-room passive-ladder MNA
> model. Both were replaced once the authors' actual reference code was located
> (see `docs/REFERENCES.md`): the real 292 is the state-space cell above, with no
> Sallen-Key stage.

## Status

- **Phase 0**: workspace scaffolding. Done.
- **Phase 1**: DSP core + vactrol model + tests. Done.
- **Phase 2**: polyphase halfband oversampling (1x/2x/4x) of the full delay-free
  solve + spectral/aliasing tests. Done. (An earlier memoryless output-buffer ADAA
  stage was removed once the audio path became the in-loop state-space solve; see
  the antialiasing notes.)
- **Phase 3**: imperfection layer (per-instance tolerance, drift, thermal, noise
  floor), seedable and serializable. Done.
- **Phase 4**: golden-file management (`reference` module, `bless`, tolerance
  comparison) + smoke/correctness/spectral tests. Done.
- **Phase 5**: benchmark suite (per-config, voices, worst-case vs typical). Done.
- **SIMD voice block** (`simd.rs`, `LpgX4`): four voices on `wide::f32x4`, a
  line-for-line mirror of the scalar DSP (verified to match within 1e-3 in
  `tests/simd.rs`). 16 voices cost ~0.73 ms vs ~2.4 ms scalar in the bench, a
  ~3.3x throughput gain. Imperfection is applied per lane: each of the four voices
  carries its own `Imperfection` instance (seed derived from one base seed), so the
  polyphony voices are each a slightly different physical channel. Lane `i` mirrors
  a scalar `Lpg` with the same derived seed (`tests/simd.rs`); when imperfection is
  disabled the block runs the original shared/splat fast path.
- **Phase 6**: tiered CI live in `.github/workflows/` (smoke/pr/nightly/release);
  rationale in `docs/CI.md`. Done.
- **Phase 7**: VCV adapter builds and links against the Rack v2 SDK on
  macOS/arm64 (`vcv-adapter/`, produces `plugin.dylib` over the C ABI). Done; a
  scripted in-Rack runtime test and other platforms remain open.

## Antialiasing notes

The `tanh` resonance nonlinearity sits **inside** the delay-free loop. It is
handled by first-order (Taylor) instantaneous linearisation about the previous
output `xo` each sample: `g(v) ≈ g(xo) + g'(xo)·(v − xo)`, where the constant part
`d1·(gx − xo·s2)` is injected as a source and the slope `s2 = 1 − tanh²` is folded
into the closed-form solve (the `dmas` Schur factor in `audio_path.rs`). So the
per-sample solve stays linear and well-posed with no Newton iteration — this is
the paper's Taylor approach, not an explicit/delayed feedback.

Antialiasing comes from **oversampling the whole delay-free solve**: the halfband
runs the ladder `factor` times per output sample at the finer timestep (the
`Oversampler` wraps the entire `solve_step`, not a memoryless buffer). Measured
aliasing for the recommended 2x config is ~ -61 dB rel fundamental on a 9 kHz
full-scale tone at drive 5 (`tests/spectral.rs`).

> History: earlier revisions placed a memoryless `tanh` buffer at the path output
> with first-order ADAA (`src/nonlinear.rs`) and an explicit one-sample resonance
> feedback. When the audio path was rewritten to the authors' state-space cell
> with in-loop instantaneous linearisation, that ADAA buffer (and its inert `adaa`
> flag) became dead code and was removed; oversampling the solve is the
> antialiasing path.

## References

- Parker & D'Angelo, "A Digital Model of the Buchla Lowpass-Gate", DAFx-13.
- Zavalishin, "The Art of VA Filter Design".
- Bilbao, Esqueda, Parker, Välimäki, ADAA, IEEE SPL 2017.

## Licensing

Clean-room implementation from the papers; core is `MIT OR Apache-2.0`. Reference
only BSD/MIT code (chowdsp_wdf, Jatin's ADAA) for technique. If the project later
ships inside Cardinal or as a non-exception VCV plugin it becomes GPLv3; that
choice is deferred and does not affect the clean-room core.
