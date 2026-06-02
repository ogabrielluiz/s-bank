//! Four-voice SIMD path: the same DSP as the scalar core, run on `wide::f32x4`.
//!
//! This is the lane-parametric polyphony path. One [`LpgX4`] processes four voices
//! at once: parameters (mode, resonance, drive, oversampling) are shared across the
//! four lanes (the normal VCV polyphony case: one module, four channels), while the
//! audio input, CV input, and all internal state are per-lane. The arithmetic is a
//! line-for-line mirror of the scalar modules, so results match within the
//! tolerance of `wide`'s transcendental approximations (verified in
//! `tests/simd.rs`). It maps directly onto Rack's `float_4` (a `__m128` of four
//! `f32`).
//!
//! The imperfection layer is intentionally not applied in this path yet; the SIMD
//! path targets the deterministic DSP core (the performance-critical part).

use wide::{f32x4, CmpGe, CmpLe, CmpLt};

use crate::audio_path::{R3_FILTER, R3_VCA};
use crate::control_path::{ControlCoeffs, CTRL_TAU_S};
use crate::imperfection::{Imperfection, ImperfectionConfig, DEFAULT_SEED};
use crate::oversample::{halfband_polyphase, halfband_taps};
use crate::params::{Components, Mode, Params};
use crate::vactrol::I_FLOOR_A;

#[inline]
fn splat(v: f32) -> f32x4 {
    f32x4::splat(v)
}

/// Per-lane salts that decorrelate the four voices' fingerprint seeds from one
/// base seed. Lane 0 uses the base seed unchanged, so a four-voice instance's
/// first voice matches a scalar `Lpg` built with the same seed.
const LANE_SEED_SALT: [u64; 4] = [
    0x0000_0000_0000_0000,
    0x9E37_79B9_7F4A_7C15,
    0xC2B2_AE3D_27D4_EB4F,
    0x1656_67B1_9E37_79F9,
];

/// Derive lane `i`'s fingerprint seed from a base seed.
#[inline]
fn lane_seed(base: u64, i: usize) -> u64 {
    base ^ LANE_SEED_SALT[i]
}

/// The tolerance-perturbed component values, packed per lane for the `f32x4`
/// solve. Only the fields the imperfection layer can perturb live here; the rest
/// of the operating point (`r3`, `drive`, the trapezoidal step) stays scalar and
/// shared. When imperfection is disabled, every lane holds the nominal value, so
/// this is just a splat of the base components and the math is unchanged.
#[derive(Debug, Clone, Copy)]
struct ComponentsX4 {
    c1: f32x4,
    c2: f32x4,
    c3: f32x4,
    rf_law_a: f32x4,
    rf_law_b: f32x4,
    r_on_min: f32x4,
    r_off: f32x4,
    tau_attack_s: f32x4,
    tau_decay_s: f32x4,
}

impl ComponentsX4 {
    /// All four lanes share one set of nominal components.
    fn splat(c: &Components) -> Self {
        Self::from_lanes(&[*c, *c, *c, *c])
    }

    /// Gather four per-lane `Components` into lane-packed vectors.
    fn from_lanes(c: &[Components; 4]) -> Self {
        let gather = |f: fn(&Components) -> f32| f32x4::from([f(&c[0]), f(&c[1]), f(&c[2]), f(&c[3])]);
        Self {
            c1: gather(|c| c.c1),
            c2: gather(|c| c.c2),
            c3: gather(|c| c.c3),
            rf_law_a: gather(|c| c.rf_law_a),
            rf_law_b: gather(|c| c.rf_law_b),
            r_on_min: gather(|c| c.r_on_min),
            r_off: gather(|c| c.r_off),
            tau_attack_s: gather(|c| c.tau_attack_s),
            tau_decay_s: gather(|c| c.tau_decay_s),
        }
    }
}

/// Numerically stable `tanh` on four lanes (argument clamped to avoid `exp`
/// overflow; `tanh` saturates well before then).
#[inline]
fn tanh4(x: f32x4) -> f32x4 {
    let z = x.max(splat(-20.0)).min(splat(20.0));
    let e = (z * splat(2.0)).exp();
    (e - splat(1.0)) / (e + splat(1.0))
}

/// The authors' control circuit on four lanes (mirror of `ControlCoeffs::current`),
/// piecewise branches realised as masked `blend`s.
#[inline]
fn control_current_x4(c: &ControlCoeffs, vb: f32x4) -> f32x4 {
    let vb = vb.max(splat(-10.0)).min(splat(50.0));
    let ia = vb * splat(c.inv_r5) + splat(c.bias);

    // V3: middle (cubic) branch plus the two saturating branches, selected by Ia.
    let x = ia * splat(c.x_coeff);
    let w = splat(c.k0) + x * (splat(c.k1) + x * (splat(c.k2) + x * splat(c.k3)));
    let v3_mid = splat(c.v3_w) * w + splat(c.v3_ia) * ia;
    let v3_low = splat(c.v3_ia) * ia;
    let v3_sat = splat(c.v3_sat_const) - ia * splat(c.r6r7);
    let v3 = ia.cmp_ge(splat(c.bound1)).blend(v3_sat, v3_mid);
    let v3 = ia.cmp_le(splat(-c.bound1)).blend(v3_low, v3);

    // If: four branches by ascending Ia thresholds.
    let ifbound1 = splat(c.alpha) * (splat(c.ifmin) - splat(c.beta) * v3);
    let if_mid = splat(c.beta) * v3 + ia * splat(c.inv_alpha);
    let if_b3 = splat(c.ifb3_slope) * ia + splat(c.ifb3_const);
    let r = ia.cmp_le(splat(c.ifbound3)).blend(if_b3, splat(c.ifmax));
    let r = ia.cmp_le(splat(c.ifbound2)).blend(if_mid, r);
    let r = ia.cmp_le(ifbound1).blend(splat(c.ifmin), r);
    r.max(splat(c.ifmin)).min(splat(c.ifmax))
}

/// SIMD control path: smooth CV, then map to LED current (amps).
#[derive(Debug, Clone)]
struct ControlPathX4 {
    cv_state: f32x4,
    smooth: f32,
    coeffs: ControlCoeffs,
}

impl ControlPathX4 {
    fn new(sample_rate: f32) -> Self {
        Self {
            cv_state: splat(0.0),
            smooth: (-1.0 / (CTRL_TAU_S * sample_rate)).exp(),
            coeffs: ControlCoeffs::new(),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.smooth = (-1.0 / (CTRL_TAU_S * sample_rate)).exp();
    }

    fn reset(&mut self) {
        self.cv_state = splat(0.0);
    }

    #[inline]
    fn process(&mut self, cv: f32x4, offset: f32x4) -> f32x4 {
        let target = cv + offset;
        self.cv_state = target + (self.cv_state - target) * splat(self.smooth);
        control_current_x4(&self.coeffs, self.cv_state)
    }
}

/// SIMD vactrol: datasheet power law plus the asymmetric, state-dependent one-pole.
#[derive(Debug, Clone)]
struct VactrolX4 {
    sample_rate: f32,
    rf: f32x4,
}

impl VactrolX4 {
    fn new(sample_rate: f32, comp: &ComponentsX4) -> Self {
        Self {
            sample_rate,
            rf: comp.r_off,
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    fn reset(&mut self, comp: &ComponentsX4) {
        self.rf = comp.r_off;
    }

    #[inline]
    fn process(&mut self, if_current: f32x4, comp: &ComponentsX4) -> f32x4 {
        let i_eff = if_current.max(splat(I_FLOOR_A));
        let target = (comp.rf_law_a / i_eff.powf(1.4) + comp.rf_law_b)
            .max(comp.r_on_min)
            .min(comp.r_off);

        let opening = target.cmp_lt(self.rf);
        let mut tau = opening.blend(comp.tau_attack_s, comp.tau_decay_s);

        let span = (comp.r_off / comp.r_on_min).ln();
        let openness = ((comp.r_off / self.rf).ln() / span)
            .max(splat(0.0))
            .min(splat(1.0));
        tau *= splat(0.5) + splat(0.5) * (splat(1.0) - openness);

        let alpha = (splat(-1.0) / (tau * splat(self.sample_rate))).exp();
        self.rf = target + (self.rf - target) * alpha;
        self.rf
    }
}

/// SIMD halfband stage (mirrors the scalar one with `f32x4` histories).
#[derive(Debug, Clone)]
struct HalfbandStageX4 {
    h: Vec<f32x4>,
    he: Vec<f32x4>,
    ho: Vec<f32x4>,
    xh: Vec<f32x4>,
    y2: Vec<f32x4>,
}

impl HalfbandStageX4 {
    fn new() -> Self {
        let taps = halfband_taps();
        let (he, ho) = halfband_polyphase(&taps);
        let to_vec = |v: &[f32]| v.iter().map(|&c| splat(c)).collect::<Vec<_>>();
        Self {
            h: to_vec(&taps),
            xh: vec![splat(0.0); he.len()],
            y2: vec![splat(0.0); taps.len()],
            he: to_vec(&he),
            ho: to_vec(&ho),
        }
    }

    fn reset(&mut self) {
        self.xh.iter_mut().for_each(|v| *v = splat(0.0));
        self.y2.iter_mut().for_each(|v| *v = splat(0.0));
    }

    #[inline]
    fn push(hist: &mut [f32x4], x: f32x4) {
        let n = hist.len();
        hist.copy_within(0..n - 1, 1);
        hist[0] = x;
    }

    #[inline]
    fn process<F: FnMut(f32x4) -> f32x4>(&mut self, x: f32x4, f: &mut F) -> f32x4 {
        Self::push(&mut self.xh, x);
        let mut up0 = splat(0.0);
        for (h, &s) in self.he.iter().zip(&self.xh) {
            up0 += *h * s;
        }
        let mut up1 = splat(0.0);
        for (h, &s) in self.ho.iter().zip(&self.xh) {
            up1 += *h * s;
        }
        up0 *= splat(2.0);
        up1 *= splat(2.0);

        Self::push(&mut self.y2, f(up0));
        Self::push(&mut self.y2, f(up1));

        let mut out = splat(0.0);
        for (h, &s) in self.h.iter().zip(&self.y2) {
            out += *h * s;
        }
        out
    }
}

#[derive(Debug, Clone)]
struct OversamplerX4 {
    stage_a: HalfbandStageX4,
    stage_b: HalfbandStageX4,
}

impl OversamplerX4 {
    fn new() -> Self {
        Self {
            stage_a: HalfbandStageX4::new(),
            stage_b: HalfbandStageX4::new(),
        }
    }

    fn reset(&mut self) {
        self.stage_a.reset();
        self.stage_b.reset();
    }

    #[inline]
    fn process<F: FnMut(f32x4) -> f32x4>(&mut self, x: f32x4, factor: usize, mut f: F) -> f32x4 {
        match factor {
            0 | 1 => f(x),
            2 => self.stage_a.process(x, &mut f),
            _ => {
                let Self { stage_a, stage_b } = self;
                stage_a.process(x, &mut |s| stage_b.process(s, &mut f))
            }
        }
    }
}

/// SIMD 292 cell: the 3-state Schur-complement solve on four lanes (mirror of the
/// scalar `Cell292`). `rf`, the capacitor values, and resonance are per-lane (the
/// imperfection layer perturbs them per voice); `r3`/`drive`/the trapezoidal step
/// are shared. `c3_active` is the mode switch (C3 out in VCA), shared across lanes.
#[derive(Debug, Clone)]
struct Cell292X4 {
    f: f32,
    sx: f32x4,
    so: f32x4,
    sd: f32x4,
    xo: f32x4,
    rf: f32x4,
    r3: f32,
    c1: f32x4,
    c2: f32x4,
    c3: f32x4,
    c3_active: bool,
    resonance: f32x4,
    drive: f32,
}

impl Cell292X4 {
    fn new() -> Self {
        Self {
            f: 0.5 / 48_000.0,
            sx: splat(0.0),
            so: splat(0.0),
            sd: splat(0.0),
            xo: splat(0.0),
            rf: splat(1.0e6),
            r3: R3_FILTER,
            c1: splat(1.0e-9),
            c2: splat(220.0e-12),
            c3: splat(4.7e-9),
            c3_active: true,
            resonance: splat(0.0),
            drive: 1.0,
        }
    }

    fn reset(&mut self) {
        self.sx = splat(0.0);
        self.so = splat(0.0);
        self.sd = splat(0.0);
        self.xo = splat(0.0);
    }

    #[inline]
    fn solve_step(&mut self, x: f32x4) -> f32x4 {
        let (rf, c1, c2, c3, f, r3) = (self.rf, self.c1, self.c2, self.c3, self.f, self.r3);
        let r3v = splat(r3);

        let a1 = splat(1.0) / (c1 * rf);
        let a2 = -(splat(1.0) / rf + splat(1.0 / r3)) / c1;
        let b1 = splat(1.0) / (rf * c2);
        let b2 = splat(-2.0) / (rf * c2);
        let b3 = b1;
        let b4 = c3 / c2;
        let d2 = splat(-1.0);

        let d1 = if self.c3_active {
            let amax = (splat(2.0) * c1 * r3v + (c2 + c3) * (r3v + rf)) / (c3 * r3v);
            self.resonance.max(splat(0.0)).min(splat(1.0)) * amax
        } else {
            splat(0.0)
        };

        let (gx, s2) = if self.drive > 0.0 {
            let t = tanh4(self.xo * splat(self.drive));
            (t / splat(self.drive), splat(1.0) - t * t)
        } else {
            (self.xo, splat(1.0))
        };

        let fv = splat(f);
        let inv_f = splat(1.0 / f);
        let dx = splat(1.0) / (splat(1.0) - b2 * fv);
        let do_ = splat(1.0) / (splat(1.0) - a2 * fv);
        let dmas = splat(1.0)
            / (splat(1.0)
                - dx * (fv * fv * b3 * do_ * a1 + b4 * fv * d1 * s2 * do_ * a1 + b4 * d2));

        let nl = d1 * (gx - self.xo * s2);

        let yx = (self.sx
            + fv * b1 * x
            + fv * b3 * do_ * self.so
            + fv * b4 * (self.sd + inv_f * nl)
            + b4 * d1 * s2 * do_ * self.so)
            * dx
            * dmas;
        let yo = (self.so + fv * a1 * yx) * do_;
        let yd = (self.sd + inv_f * nl) + inv_f * (d1 * s2 * yo + d2 * yx);

        self.sx += splat(2.0) * fv * (b1 * x + b2 * yx + b3 * yo + b4 * yd);
        self.so += splat(2.0) * fv * (a1 * yx + a2 * yo);
        self.sd = if self.c3_active {
            -(self.sd + splat(2.0 / f) * nl) - splat(2.0 / f) * (d1 * s2 * yo + d2 * yx)
        } else {
            splat(0.0)
        };
        self.xo = yo;

        yo
    }
}

/// Four vactrol low-pass-gate voices processed together on `f32x4` lanes.
///
/// Each lane carries its own [`Imperfection`] instance (its own fingerprint seed,
/// tolerance, drift, thermal wander and noise floor), so four polyphony voices are
/// each a slightly different physical channel — the realistic analogue behaviour.
/// The four seeds are derived from one base seed via [`lane_seed`], so the whole
/// block is reproducible and serializable from a single seed, and lane 0 matches a
/// scalar [`Lpg`](crate::Lpg) built with that base seed. The layer reuses the exact
/// scalar `Imperfection` code, so each lane mirrors its scalar counterpart.
///
/// When imperfection is disabled (the default) the per-lane work is skipped
/// entirely and the block runs the original shared/splat fast path.
#[derive(Debug, Clone)]
pub struct LpgX4 {
    sample_rate: f32,
    params: Params,
    base_seed: u64,
    /// Nominal component values (before per-instance tolerance).
    base_comp: Components,
    /// Effective per-lane component values (tolerance applied when enabled).
    compx4: ComponentsX4,
    config: ImperfectionConfig,
    imperfection: [Imperfection; 4],
    control: ControlPathX4,
    vactrol: VactrolX4,
    cell: Cell292X4,
    oversampler: OversamplerX4,
    last_rf: f32x4,
}

impl LpgX4 {
    /// Build four voices with the default fingerprint seed (imperfection disabled).
    pub fn new(sample_rate: f32) -> Self {
        Self::with_seed(sample_rate, DEFAULT_SEED)
    }

    /// Build four voices whose per-lane seeds derive from `base_seed`.
    pub fn with_seed(sample_rate: f32, base_seed: u64) -> Self {
        let base_comp = Components::default();
        let config = ImperfectionConfig::default();
        let imperfection =
            std::array::from_fn(|i| Imperfection::new(lane_seed(base_seed, i), config));
        // Disabled config => every lane reports nominal components => a plain splat.
        let compx4 = ComponentsX4::splat(&base_comp);
        Self {
            sample_rate,
            params: Params::default(),
            base_seed,
            base_comp,
            compx4,
            config,
            imperfection,
            control: ControlPathX4::new(sample_rate),
            vactrol: VactrolX4::new(sample_rate, &compx4),
            cell: Cell292X4::new(),
            oversampler: OversamplerX4::new(),
            last_rf: compx4.r_off,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.control.set_sample_rate(sample_rate);
        self.vactrol.set_sample_rate(sample_rate);
    }

    pub fn set_params(&mut self, params: Params) {
        self.params = params;
    }

    pub fn params(&self) -> &Params {
        &self.params
    }

    /// The base fingerprint seed; lane `i` uses [`lane_seed`]`(seed, i)`.
    pub fn seed(&self) -> u64 {
        self.base_seed
    }

    /// The four per-lane fingerprint seeds derived from the base seed. Building a
    /// scalar [`Lpg`](crate::Lpg) with `lane_seeds()[i]` and the same imperfection
    /// config reproduces lane `i` exactly.
    pub fn lane_seeds(&self) -> [u64; 4] {
        std::array::from_fn(|i| lane_seed(self.base_seed, i))
    }

    /// Configure the imperfection layer for all four lanes. Re-derives each lane's
    /// component tolerance from its seed and resets the layers' transient state.
    /// Mirrors [`Lpg::set_imperfection`](crate::Lpg::set_imperfection) per lane.
    pub fn set_imperfection(&mut self, config: ImperfectionConfig) {
        self.config = config;
        for imp in &mut self.imperfection {
            imp.config = config;
            imp.reset();
        }
        let lanes: [Components; 4] =
            std::array::from_fn(|i| self.imperfection[i].tolerance_components(&self.base_comp));
        self.compx4 = ComponentsX4::from_lanes(&lanes);
        self.last_rf = self.compx4.r_off;
    }

    /// The current imperfection configuration (shared across lanes).
    pub fn imperfection_config(&self) -> &ImperfectionConfig {
        &self.config
    }

    /// Last per-lane vactrol resistance (ohms).
    pub fn last_rf(&self) -> [f32; 4] {
        self.last_rf.to_array()
    }

    pub fn reset(&mut self) {
        self.control.reset();
        self.vactrol.reset(&self.compx4);
        self.cell.reset();
        self.oversampler.reset();
        for imp in &mut self.imperfection {
            imp.reset();
        }
        self.last_rf = self.compx4.r_off;
    }

    /// Set the cell operating point and run the (optionally oversampled) solve.
    /// `resonance` is per-lane (drift perturbs it); the rest is shared.
    #[inline]
    fn solve(&mut self, audio_in: f32x4, rf: f32x4, resonance: f32x4) -> f32x4 {
        let c = &mut self.cell;
        c.rf = rf.max(splat(1.0));
        c.c1 = self.compx4.c1;
        c.c2 = self.compx4.c2;
        match self.params.mode {
            Mode::Vca => {
                c.c3 = splat(0.0);
                c.c3_active = false;
                c.r3 = R3_VCA;
            }
            _ => {
                c.c3 = self.compx4.c3;
                c.c3_active = true;
                c.r3 = R3_FILTER;
            }
        }
        c.resonance = resonance;
        c.drive = self.params.drive;

        let factor = self.params.oversample_factor();
        c.f = 0.5 / (self.sample_rate * factor as f32);
        if factor == 1 {
            c.solve_step(audio_in)
        } else {
            self.oversampler
                .process(audio_in, factor, |xs| c.solve_step(xs))
        }
    }

    /// Process one sample for all four voices. `audio_in`/`cv_in` are per-lane.
    #[inline]
    pub fn process(&mut self, audio_in: f32x4, cv_in: f32x4) -> f32x4 {
        if !self.config.enabled {
            // Shared fast path: every lane identical, no per-lane gather.
            let current = self.control.process(cv_in, splat(self.params.cv_offset));
            let rf = self.vactrol.process(current, &self.compx4);
            self.last_rf = rf;
            return self.solve(audio_in, rf, splat(self.params.resonance));
        }

        // Per-lane path: each voice advances its own imperfection layer.
        let mut res = [0.0f32; 4];
        let mut off = [0.0f32; 4];
        for (i, imp) in self.imperfection.iter_mut().enumerate() {
            imp.update(self.sample_rate);
            let p = imp.apply_params(&self.params);
            res[i] = p.resonance;
            off[i] = p.cv_offset;
        }
        let current = self.control.process(cv_in, f32x4::from(off));
        let rf = self.vactrol.process(current, &self.compx4);
        self.last_rf = rf;
        let y = self.solve(audio_in, rf, f32x4::from(res));

        let mut ya = y.to_array();
        for (i, imp) in self.imperfection.iter_mut().enumerate() {
            ya[i] = imp.apply_output(ya[i]);
        }
        f32x4::from(ya)
    }

    /// Convenience: process a block of per-lane samples in place.
    #[inline]
    pub fn process_block(&mut self, audio_in: &[f32x4], cv_in: &[f32x4], out: &mut [f32x4]) {
        for ((&a, &c), o) in audio_in.iter().zip(cv_in).zip(out.iter_mut()) {
            *o = self.process(a, c);
        }
    }
}
