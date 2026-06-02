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

use wide::{f32x4, CmpGt, CmpLt};

use crate::control_path::{CTRL_TAU_S, I_MAX_A, V_SCALE};
use crate::nonlinear::ADAA_EPS;
use crate::oversample::{halfband_polyphase, halfband_taps};
use crate::params::{Components, Mode, Params};
use crate::vactrol::I_FLOOR_A;

#[inline]
fn splat(v: f32) -> f32x4 {
    f32x4::splat(v)
}

/// Numerically stable `tanh` on four lanes (argument clamped to avoid `exp`
/// overflow; `tanh` saturates well before then).
#[inline]
fn tanh4(x: f32x4) -> f32x4 {
    let z = x.max(splat(-20.0)).min(splat(20.0));
    let e = (z * splat(2.0)).exp();
    (e - splat(1.0)) / (e + splat(1.0))
}

/// `lncosh(z) = |z| + ln(1 + exp(-2|z|)) - ln 2` on four lanes.
#[inline]
fn lncosh4(z: f32x4) -> f32x4 {
    let a = z.abs();
    a + (splat(1.0) + (splat(-2.0) * a).exp()).ln() - splat(core::f32::consts::LN_2)
}

/// `tanh` saturator, unity-gain normalized. `drive` is shared across lanes.
#[inline]
fn saturate4(x: f32x4, drive: f32) -> f32x4 {
    if drive > 0.0 {
        tanh4(x * splat(drive)) / splat(drive)
    } else {
        x
    }
}

/// Per-lane first-order ADAA state for the buffer `tanh`.
#[derive(Debug, Clone, Default)]
struct TanhAdaaX4 {
    x1: f32x4,
    f1: f32x4,
}

impl TanhAdaaX4 {
    fn reset(&mut self) {
        self.x1 = splat(0.0);
        self.f1 = splat(0.0);
    }

    #[inline]
    fn process(&mut self, x: f32x4, drive: f32) -> f32x4 {
        if drive <= 0.0 {
            self.x1 = x;
            self.f1 = splat(0.0);
            return x;
        }
        let d = drive;
        let f1x = lncosh4(x * splat(d)) / splat(d * d);
        let diff = x - self.x1;
        let y_div = (f1x - self.f1) / diff;
        let xbar = splat(0.5) * (x + self.x1);
        let y_fb = saturate4(xbar, d);
        // Use the divided difference where |diff| is large enough, else fallback.
        let big = diff.abs().cmp_gt(splat(ADAA_EPS));
        let y = big.blend(y_div, y_fb);
        self.x1 = x;
        self.f1 = f1x;
        y
    }
}

/// SIMD control path: smooth CV, then map to LED current (amps).
#[derive(Debug, Clone)]
struct ControlPathX4 {
    cv_state: f32x4,
    smooth: f32,
}

impl ControlPathX4 {
    fn new(sample_rate: f32) -> Self {
        Self {
            cv_state: splat(0.0),
            smooth: (-1.0 / (CTRL_TAU_S * sample_rate)).exp(),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.smooth = (-1.0 / (CTRL_TAU_S * sample_rate)).exp();
    }

    fn reset(&mut self) {
        self.cv_state = splat(0.0);
    }

    #[inline]
    fn process(&mut self, cv: f32x4, offset: f32) -> f32x4 {
        let target = cv + splat(offset);
        self.cv_state = target + (self.cv_state - target) * splat(self.smooth);
        let vv = self.cv_state.max(splat(0.0));
        splat(I_MAX_A) * (splat(1.0) - (-vv / splat(V_SCALE)).exp())
    }
}

/// SIMD vactrol: datasheet power law plus the asymmetric, state-dependent one-pole.
#[derive(Debug, Clone)]
struct VactrolX4 {
    sample_rate: f32,
    rf: f32x4,
}

impl VactrolX4 {
    fn new(sample_rate: f32, comp: &Components) -> Self {
        Self {
            sample_rate,
            rf: splat(comp.r_off),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    fn reset(&mut self, comp: &Components) {
        self.rf = splat(comp.r_off);
    }

    #[inline]
    fn process(&mut self, if_current: f32x4, comp: &Components) -> f32x4 {
        let i_eff = if_current.max(splat(I_FLOOR_A));
        let target = (splat(comp.rf_law_a) / i_eff.powf(1.4) + splat(comp.rf_law_b))
            .max(splat(comp.r_on_min))
            .min(splat(comp.r_off));

        let opening = target.cmp_lt(self.rf);
        let mut tau = opening.blend(splat(comp.tau_attack_s), splat(comp.tau_decay_s));

        let span = (comp.r_off / comp.r_on_min).ln();
        let openness = ((splat(comp.r_off) / self.rf).ln() / splat(span))
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

/// SIMD ladder: the 2x2 MNA companion-model solve on four lanes.
#[derive(Debug, Clone)]
struct LadderX4 {
    dt: f32,
    i1: f32x4,
    i2: f32x4,
    i3: f32x4,
    vn1: f32x4,
    vn2: f32x4,
    vfb: f32x4,
    adaa: TanhAdaaX4,
    rf: f32x4,
    r_alpha: f32,
    c1: f32,
    c2: f32,
    c3: f32,
    k: f32,
    drive: f32,
    use_adaa: bool,
}

impl LadderX4 {
    fn new() -> Self {
        Self {
            dt: 1.0 / 48_000.0,
            i1: splat(0.0),
            i2: splat(0.0),
            i3: splat(0.0),
            vn1: splat(0.0),
            vn2: splat(0.0),
            vfb: splat(0.0),
            adaa: TanhAdaaX4::default(),
            rf: splat(1.0e6),
            r_alpha: 5.0e6,
            c1: 1.0e-9,
            c2: 220.0e-12,
            c3: 0.0,
            k: 1.0,
            drive: 0.0,
            use_adaa: false,
        }
    }

    fn reset(&mut self) {
        self.i1 = splat(0.0);
        self.i2 = splat(0.0);
        self.i3 = splat(0.0);
        self.vn1 = splat(0.0);
        self.vn2 = splat(0.0);
        self.vfb = splat(0.0);
        self.adaa.reset();
    }

    #[inline]
    fn solve_step(&mut self, vin: f32x4) -> f32x4 {
        let gf = splat(1.0) / self.rf;
        let ga = splat(1.0 / self.r_alpha);
        let gc1 = splat(2.0 * self.c1 / self.dt);
        let gc2 = splat(2.0 * self.c2 / self.dt);
        let gc3 = splat(2.0 * self.c3 / self.dt);

        let buf = if self.use_adaa {
            self.adaa.process(self.vn2, self.drive)
        } else {
            saturate4(self.vn2, self.drive)
        };
        let vfb = splat(self.k) * buf;

        let ics1 = gc1 * (self.vn1 - self.vfb) + self.i1;
        let ics2 = gc2 * self.vn2 + self.i2;
        let ics3 = gc3 * (self.vn1 - self.vn2) + self.i3;

        let g11 = splat(2.0) * gf + gc1 + gc3;
        let g22 = gf + ga + gc2 + gc3;
        let g12 = -gf - gc3;
        let b1 = gf * vin + gc1 * vfb + ics1 + ics3;
        let b2 = ics2 - ics3;

        let det = g11 * g22 - g12 * g12;
        let vn1 = (b1 * g22 - g12 * b2) / det;
        let vn2 = (g11 * b2 - g12 * b1) / det;

        self.i1 = gc1 * (vn1 - vfb) - ics1;
        self.i2 = gc2 * vn2 - ics2;
        self.i3 = gc3 * (vn1 - vn2) - ics3;
        self.vn1 = vn1;
        self.vn2 = vn2;
        self.vfb = vfb;

        vn2
    }
}

/// Four vactrol low-pass-gate voices processed together on `f32x4` lanes.
#[derive(Debug, Clone)]
pub struct LpgX4 {
    sample_rate: f32,
    params: Params,
    comp: Components,
    control: ControlPathX4,
    vactrol: VactrolX4,
    ladder: LadderX4,
    oversampler: OversamplerX4,
    last_rf: f32x4,
}

impl LpgX4 {
    pub fn new(sample_rate: f32) -> Self {
        let comp = Components::default();
        Self {
            sample_rate,
            params: Params::default(),
            comp,
            control: ControlPathX4::new(sample_rate),
            vactrol: VactrolX4::new(sample_rate, &comp),
            ladder: LadderX4::new(),
            oversampler: OversamplerX4::new(),
            last_rf: splat(comp.r_off),
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

    /// Last per-lane vactrol resistance (ohms).
    pub fn last_rf(&self) -> [f32; 4] {
        self.last_rf.to_array()
    }

    pub fn reset(&mut self) {
        self.control.reset();
        self.vactrol.reset(&self.comp);
        self.ladder.reset();
        self.oversampler.reset();
        self.last_rf = splat(self.comp.r_off);
    }

    /// Process one sample for all four voices. `audio_in`/`cv_in` are per-lane.
    #[inline]
    pub fn process(&mut self, audio_in: f32x4, cv_in: f32x4) -> f32x4 {
        let current = self.control.process(cv_in, self.params.cv_offset);
        let rf = self.vactrol.process(current, &self.comp);
        self.last_rf = rf;

        let lowpass = matches!(self.params.mode, Mode::Lowpass);
        let l = &mut self.ladder;
        l.rf = rf.max(splat(1.0));
        l.r_alpha = match self.params.mode {
            Mode::Vca => self.comp.r_alpha_vca,
            _ => self.comp.r_alpha_both,
        };
        l.c1 = self.comp.c1;
        l.c2 = self.comp.c2;
        l.c3 = if lowpass { self.comp.c3 } else { 0.0 };
        l.k = 1.0 + self.params.resonance.clamp(0.0, 1.0) * crate::audio_path::K_SPAN;
        l.drive = self.params.drive;
        l.use_adaa = self.params.adaa;

        let base_dt = 1.0 / self.sample_rate;
        let factor = self.params.oversample_factor();
        if factor == 1 {
            l.dt = base_dt;
            l.solve_step(audio_in)
        } else {
            l.dt = base_dt / factor as f32;
            self.oversampler
                .process(audio_in, factor, |xs| l.solve_step(xs))
        }
    }

    /// Convenience: process a block of per-lane samples in place.
    #[inline]
    pub fn process_block(&mut self, audio_in: &[f32x4], cv_in: &[f32x4], out: &mut [f32x4]) {
        for ((&a, &c), o) in audio_in.iter().zip(cv_in).zip(out.iter_mut()) {
            *o = self.process(a, c);
        }
    }
}
