# References and model validation

The DSP was validated against the primary sources (see the session research
notes). Summary of what was confirmed, corrected, and what remains approximate.

## Primary sources

- **Julian Parker & Stefano D'Angelo, "A Digital Model of the Buchla Lowpass-Gate,"
  Proc. DAFx-13, Maynooth, 2013.** The canonical model.
  - PDF: http://research.spa.aalto.fi/publications/papers/dafx13-lpg/ ,
    https://www.dafx.de/paper-archive/details.php?id=uqLQLFI9j52bBmv10UKKMg
  - The authors' own gen~ reference code survives in Cycling '74's
    `RNBOSynthBuildingBlocks` (`patchers/sbb.env.lpg.rnbopat`) and as the
    SuperCollider `LPG` UGen (`madskjeldgaard/portedplugins`, `plugins/lpg.cpp`).
    The `audio_path.rs` state-space update and the `vactrol.rs` power law were
    ported/checked against that code.
- **VTL5C3 / VTL5C3/2 datasheet (PerkinElmer / Excelitas; Xvive clone).**
  Current-dependent ON resistance (~10 kΩ@1 mA, ~1 kΩ@10 mA, ~500 Ω@40 mA),
  10 MΩ dark, 2.5 ms turn-on, 35 ms turn-off, ~75 dB range.

## What the model gets right (confirmed against the authors' code)

- **Audio path is a 3-capacitor continuous-time state-space filter** (states for
  C1=1 nF, C2=220 pF, C3=4.7 nF), discretised topology-preservingly and solved as
  a closed-form delay-free loop (the `Dx`/`Do`/`Dmas` Schur-complement factors).
  This matches the authors' code structure, including the coefficient set
  `a1=1/(C1 Rf)`, `a2=-(1/Rf+1/R3)/C1`, `b1=b3=1/(Rf C2)`, `b2=-2/(Rf C2)`,
  `b4=C3/C2`, `d1=a`, `d2=-1`.
- **DC gain `R3/(R3 + 2·Rf)`** (the two series vactrol resistances): reproduced
  exactly by the solve.
- **Resonance** is the feedback gain `a`, clamped to the exact stability limit
  `amax = (2 C1 R3 + (C2+C3)(R3+Rf))/(C3 R3)`, recomputed per sample. The `tanh`
  is linearised about the previous output (no Newton iteration). Confirmed.
- **C3 switched out in VCA mode** (amplitude response), in for Lowpass/Both.
- **Vactrol power law `Rf = B + A/If^1.4`** with **A = 3.4645912**,
  **B = 1136.2129956** (`If` in amperes): these are the authors' exact published
  constants. The "5x at 1 mA vs datasheet" worry is not a bug: the monomial fit
  only holds over the 5-40 mA range the 292 actually drives; below ~3 mA both the
  fit and real parts (the datasheet's own "consult factory" caveat) misbehave.
- **Control circuit (CV -> LED current)** is ported verbatim from the authors'
  `lpg.cpp` (`LpgControlCircuit::process`): the bias stage, the cubic Lambert-W
  approximation `w = k0 + k1 x + k2 x^2 + k3 x^3` (k0=146.8, k1=0.49202,
  k2=4.1667e-4, k3=7.3915e-9) in the central branch, the saturating branches, and
  the piecewise `If` clamp to `[10.1 uA, 40 mA]`. All resistor/op-amp constants
  (R3, R5, R6, R7, R8, R9, alpha, beta, G, n, VT, Vs, ...) match the source. The
  Rust port is checked against the reference values in a unit test (`control_path`).

## Where the model diverges (documented approximations, not bugs)

- **R3 and the mode mapping.** In the reference code `R3` is the resonance/offset
  control resistor (swept ~50 kΩ-1.05 MΩ). With a single `resonance` knob this
  port uses fixed nominal `R3` per mode (`R3_FILTER = 1 MΩ`, `R3_VCA = 100 kΩ`)
  and drives `a` from `resonance`. The Both vs Lowpass musical distinction in the
  original is subtle and not fully reproduced.
- **Control circuit front-panel controls.** The reference exposes `offset` and
  `scale` knobs (they set the bias divider R1/R2 and R6). This port fixes them
  (offset ~ 0 so the gate fully darkens at CV = 0, scale = 1); exposing them as
  parameters is straightforward future work. The CV-to-Vb input scaling is 1:1
  (a control voltage in volts), so the gate opens over roughly CV 7-11 V.
- **Vactrol time constants.** The reference uses a specific state-dependent
  attack/decay smoother; this port uses a comparable asymmetric, state-dependent
  one-pole (attack ~5 ms, decay ~120 ms, between the VTL5C3 single-part 2.5/35 ms
  and the /2's 12/250 ms). Qualitatively faithful, not identical.

## Further reading

- Najnudel, Hélie, Roze et al., "Power-balanced dynamic modeling of vactrols,"
  HAL hal-04452215 / DAFx-23 (rigorous energy-balanced vactrol dynamics).
- Iverson & Smith, "Mathematical Modeling of Photoconductor Transient Response,"
  IEEE Trans. Electron Devices, 1987 (the physics of the slow CdS decay).

## Provenance caveat

The accompanying-code equations are effectively primary (the authors' own), but
the paper PDF could not be read verbatim during validation, so literal equation
numbers (e.g. "Eq. 11/12") and the printed Table 1 are probable but not
letter-confirmed. The constants and structure above are confirmed against the
reference code.
