//! Audio path: the Parker & D'Angelo Buchla 292 model, ported from the authors'
//! own reference implementation (the gen~ patch accompanying the DAFx-13 paper,
//! also surviving as the SuperCollider `LPG` UGen).
//!
//! It is a **three-capacitor continuous-time state-space filter** discretised with
//! a topology-preserving (trapezoidal) scheme and solved as a delay-free loop in
//! closed form each sample. The three states correspond to C1 (1 nF), C2 (220 pF)
//! and C3 (4.7 nF):
//!
//! ```text
//!   a1 = 1/(C1·Rf)              b1 =  1/(Rf·C2)
//!   a2 = -(1/Rf + 1/R3)/C1      b2 = -2/(Rf·C2)
//!   b3 =  1/(Rf·C2)             b4 =  C3/C2
//!   d1 =  a (resonance gain)    d2 = -1
//! ```
//!
//! `Rf` is the vactrol resistance (sets cutoff and, with the two series `Rf`, the
//! `R3/(R3 + 2·Rf)` output divider that closes the gate). `R3` is the output-node
//! resistor; resonance is the feedback gain `a`, clamped to the exact stability
//! limit `amax = (2·C1·R3 + (C2+C3)(R3+Rf)) / (C3·R3)` (recomputed every sample as
//! Rf modulates). The `tanh` resonance nonlinearity is handled by instantaneous
//! linearisation about the previous output `xo`, so the delay-free loop is solved
//! algebraically (the `Dx`, `Do`, `Dmas` factors are the closed-form / Schur
//! complement of the instantaneous system) with no Newton iteration. This is the
//! topology-preserving version the paper shows is stable under any rate of
//! modulation, unlike the direct-form bilinear transfer function.
//!
//! Modes: C3 is switched out (`= 0`) in VCA mode (amplitude/divider response only)
//! and in for Lowpass/Both. `drive` scales the resonance nonlinearity's hardness
//! (`drive = 1` is the authors' plain `tanh`); `resonance = 0` makes the cell
//! linear (the nonlinear terms carry the factor `a`). The whole solve is
//! oversampled when requested; that is the antialiasing path for the in-loop
//! nonlinearity. The scalar solve is branch-light, mirrored lane-for-lane by the
//! SIMD path in `simd.rs`.

use crate::oversample::Oversampler;
use crate::params::{Components, Mode, Params};

/// Output-node resistor `R3`. VCA uses a smaller value for a stronger amplitude
/// gate; Both/Lowpass use a larger value (mostly filter, gentle gating).
pub(crate) const R3_VCA: f32 = 1.0e5;
pub(crate) const R3_FILTER: f32 = 1.0e6;

/// The 292 cell: three capacitor states plus the previous output (for the
/// resonance linearisation). One `solve_step` advances a single (possibly
/// oversampled) sample at the configured timestep.
#[derive(Debug, Clone)]
struct Cell292 {
    /// Trapezoidal prewarp factor `f = T/2` at the (possibly oversampled) rate.
    f: f32,
    // Capacitor states.
    sx: f32,
    so: f32,
    sd: f32,
    /// Previous output (resonance linearisation point).
    xo: f32,
    // Per-call operating point.
    rf: f32,
    r3: f32,
    c1: f32,
    c2: f32,
    c3: f32, // 0 in VCA mode
    resonance: f32,
    drive: f32,
}

impl Cell292 {
    fn new() -> Self {
        Self {
            f: 0.5 / 48_000.0,
            sx: 0.0,
            so: 0.0,
            sd: 0.0,
            xo: 0.0,
            rf: 1.0e6,
            r3: R3_FILTER,
            c1: 1.0e-9,
            c2: 220.0e-12,
            c3: 4.7e-9,
            resonance: 0.0,
            drive: 1.0,
        }
    }

    fn reset(&mut self) {
        self.sx = 0.0;
        self.so = 0.0;
        self.sd = 0.0;
        self.xo = 0.0;
    }

    #[inline]
    fn solve_step(&mut self, x: f32) -> f32 {
        let (rf, r3, c1, c2, c3, f) = (self.rf, self.r3, self.c1, self.c2, self.c3, self.f);

        let a1 = 1.0 / (c1 * rf);
        let a2 = -(1.0 / rf + 1.0 / r3) / c1;
        let b1 = 1.0 / (rf * c2);
        let b2 = -2.0 / (rf * c2);
        let b3 = 1.0 / (rf * c2);
        let b4 = c3 / c2;
        let d2 = -1.0;

        // Resonance feedback gain, clamped to the exact stability limit amax.
        let d1 = if c3 > 0.0 {
            let amax = (2.0 * c1 * r3 + (c2 + c3) * (r3 + rf)) / (c3 * r3);
            self.resonance.clamp(0.0, 1.0) * amax
        } else {
            0.0
        };

        // Resonance nonlinearity g(v) = tanh(drive·v)/drive, linearised about xo.
        // `gx` plays the role of tanh(xo); `s2` of its slope 1 - tanh^2.
        let (gx, s2) = if self.drive > 0.0 {
            let t = (self.drive * self.xo).tanh();
            (t / self.drive, 1.0 - t * t)
        } else {
            (self.xo, 1.0)
        };

        let dx = 1.0 / (1.0 - b2 * f);
        let do_ = 1.0 / (1.0 - a2 * f);
        let dmas =
            1.0 / (1.0 - dx * (f * f * b3 * do_ * a1 + b4 * f * d1 * s2 * do_ * a1 + b4 * d2));

        let nl = d1 * (gx - self.xo * s2);

        let yx = (self.sx
            + f * b1 * x
            + f * b3 * do_ * self.so
            + f * b4 * (self.sd + (1.0 / f) * nl)
            + b4 * d1 * s2 * do_ * self.so)
            * dx
            * dmas;
        let yo = (self.so + f * a1 * yx) * do_;
        let yd = (self.sd + (1.0 / f) * nl) + (1.0 / f) * (d1 * s2 * yo + d2 * yx);

        self.sx += 2.0 * f * (b1 * x + b2 * yx + b3 * yo + b4 * yd);
        self.so += 2.0 * f * (a1 * yx + a2 * yo);
        self.sd = if c3 > 0.0 {
            -(self.sd + (2.0 / f) * nl) - (2.0 / f) * (d1 * s2 * yo + d2 * yx)
        } else {
            0.0
        };
        self.xo = yo;

        yo
    }
}

#[derive(Debug, Clone)]
pub struct AudioPath {
    sample_rate: f32,
    cell: Cell292,
    oversampler: Oversampler,
}

impl AudioPath {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            cell: Cell292::new(),
            oversampler: Oversampler::new(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn reset(&mut self) {
        self.cell.reset();
        self.oversampler.reset();
    }

    /// Process one sample. `rf` is the live vactrol resistance (ohms).
    #[inline]
    pub fn process(&mut self, x: f32, rf: f32, params: &Params, comp: &Components) -> f32 {
        let c = &mut self.cell;
        c.rf = rf.max(1.0);
        c.c1 = comp.c1;
        c.c2 = comp.c2;
        match params.mode {
            Mode::Vca => {
                c.c3 = 0.0;
                c.r3 = R3_VCA;
            }
            _ => {
                c.c3 = comp.c3;
                c.r3 = R3_FILTER;
            }
        }
        c.resonance = params.resonance;
        c.drive = params.drive;

        let factor = params.oversample_factor();
        c.f = 0.5 / (self.sample_rate * factor as f32);
        if factor == 1 {
            c.solve_step(x)
        } else {
            self.oversampler.process(x, factor, |xs| c.solve_step(xs))
        }
    }
}
