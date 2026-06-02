//! Audio path: a topology-preserving nodal model of the 292-style vactrol ladder.
//!
//! This is a circuit model, not a generic biquad. The 292 audio path is a passive
//! ladder driven by the vactrol resistance `Rf`:
//!
//! ```text
//!   Vin --Rf-- n1 --Rf-- n2(=Vout)
//!              |          |
//!              C1*        C2     Ra
//!              |          |       |
//!             Vfb        gnd     gnd
//!   (* C3 bridges n1-n2 in Lowpass mode only)
//! ```
//!
//! Each capacitor is replaced by its trapezoidal companion model (a conductance
//! `2C/T` in parallel with a history current source), and the two node voltages
//! are found by a 2x2 modified-nodal-analysis solve every sample. Consequences:
//!
//! * The DC divider `Rα / (Rα + 2·Rf)` (Eq. 12) falls out of the solve exactly,
//!   in all three modes, instead of being multiplied on afterwards.
//! * There are three independent capacitor states (C1, C2, C3), which is the
//!   paper's whole point: the transfer-function form keeps only two and diverges
//!   under fast modulation. The companion-model `G` matrix is passive, so the
//!   solve is unconditionally stable at any modulation rate, no cutoff clamp.
//! * Every component value (C1, C2, C3, Rα, Rf) appears in the coefficients.
//!
//! Resonance uses the Sallen-Key mechanism: the C1 return is a buffered, gained
//! copy of the output `Vfb = K · f(Vout)`, with `K` raising Q toward
//! self-oscillation. The buffer nonlinearity `f` (tanh) therefore sits *inside*
//! the feedback loop. It is evaluated on the previous (oversampled) output, so the
//! `G` matrix stays passive and the per-sample solve stays linear and well-posed;
//! the explicit one-sample feedback is the modelling approximation, and
//! oversampling is the antialiasing backstop. At DC the C1 branch is open, so
//! resonance does not disturb the Eq. 12 divider.
//!
//! Mode selects `Rα` (5 MΩ in Both/Lowpass, 5 kΩ in VCA) and whether the C3 branch
//! is engaged (Lowpass). The scalar `f32` solve is branch-light, so a
//! lane-parametric SIMD variant across voices is a structural drop-in.

use crate::nonlinear::{self, TanhAdaa};
use crate::oversample::Oversampler;
use crate::params::{Components, Mode, Params};

/// Resonance span: `K = 1 + resonance·K_SPAN`. Tuned so `resonance = 1` reaches
/// bounded self-oscillation (the `amax` boundary).
const K_SPAN: f32 = 2.2;

/// The companion-model ladder: capacitor histories, node memory, and the in-loop
/// buffer nonlinearity. One `solve_step` advances a single (possibly oversampled)
/// sample at the configured `dt`.
#[derive(Debug, Clone)]
struct Ladder {
    dt: f32,
    // Previous capacitor currents (trapezoidal history).
    i1: f32,
    i2: f32,
    i3: f32,
    // Previous node voltages and feedback voltage.
    vn1: f32,
    vn2: f32,
    vfb: f32,
    // In-loop buffer nonlinearity with optional ADAA.
    adaa: TanhAdaa,
    // Per-call operating point (set by `AudioPath::process`).
    rf: f32,
    r_alpha: f32,
    c1: f32,
    c2: f32,
    c3: f32, // 0 unless Lowpass
    k: f32,
    drive: f32,
    use_adaa: bool,
}

impl Ladder {
    fn new() -> Self {
        Self {
            dt: 1.0 / 48_000.0,
            i1: 0.0,
            i2: 0.0,
            i3: 0.0,
            vn1: 0.0,
            vn2: 0.0,
            vfb: 0.0,
            adaa: TanhAdaa::default(),
            rf: 1.0e6,
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
        self.i1 = 0.0;
        self.i2 = 0.0;
        self.i3 = 0.0;
        self.vn1 = 0.0;
        self.vn2 = 0.0;
        self.vfb = 0.0;
        self.adaa.reset();
    }

    /// Advance one sample: assemble the 2x2 MNA system and solve for the node
    /// voltages, then update capacitor histories. Returns `Vout = Vn2`.
    #[inline]
    fn solve_step(&mut self, vin: f32) -> f32 {
        let gf = 1.0 / self.rf;
        let ga = 1.0 / self.r_alpha;
        let gc1 = 2.0 * self.c1 / self.dt;
        let gc2 = 2.0 * self.c2 / self.dt;
        let gc3 = 2.0 * self.c3 / self.dt; // c3 == 0 disables the branch

        // Resonance buffer: explicit (uses the previous output), nonlinearity in
        // the loop. K scales the Sallen-Key feedback toward self-oscillation.
        let buf = if self.use_adaa {
            self.adaa.process(self.vn2, self.drive)
        } else {
            nonlinear::saturate(self.vn2, self.drive)
        };
        let vfb = self.k * buf;

        // Capacitor history current sources: Ics = Gc·v[n-1] + i[n-1].
        let ics1 = gc1 * (self.vn1 - self.vfb) + self.i1;
        let ics2 = gc2 * self.vn2 + self.i2;
        let ics3 = gc3 * (self.vn1 - self.vn2) + self.i3;

        // MNA stamps for nodes n1, n2 (symmetric: g21 == g12).
        let g11 = 2.0 * gf + gc1 + gc3;
        let g22 = gf + ga + gc2 + gc3;
        let g12 = -gf - gc3;
        let b1 = gf * vin + gc1 * vfb + ics1 + ics3;
        let b2 = ics2 - ics3;

        let det = g11 * g22 - g12 * g12;
        let vn1 = (b1 * g22 - g12 * b2) / det;
        let vn2 = (g11 * b2 - g12 * b1) / det;

        // Update capacitor histories: i[n] = Gc·v[n] - Ics.
        self.i1 = gc1 * (vn1 - vfb) - ics1;
        self.i2 = gc2 * vn2 - ics2;
        self.i3 = gc3 * (vn1 - vn2) - ics3;
        self.vn1 = vn1;
        self.vn2 = vn2;
        self.vfb = vfb;

        vn2
    }
}

#[derive(Debug, Clone)]
pub struct AudioPath {
    sample_rate: f32,
    ladder: Ladder,
    oversampler: Oversampler,
}

impl AudioPath {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ladder: Ladder::new(),
            oversampler: Oversampler::new(),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn reset(&mut self) {
        self.ladder.reset();
        self.oversampler.reset();
    }

    /// Process one sample. `rf` is the live vactrol resistance (ohms).
    #[inline]
    pub fn process(&mut self, x: f32, rf: f32, params: &Params, comp: &Components) -> f32 {
        let lowpass = matches!(params.mode, Mode::Lowpass);
        let l = &mut self.ladder;
        l.rf = rf.max(1.0);
        l.r_alpha = match params.mode {
            Mode::Vca => comp.r_alpha_vca,
            _ => comp.r_alpha_both,
        };
        l.c1 = comp.c1;
        l.c2 = comp.c2;
        l.c3 = if lowpass { comp.c3 } else { 0.0 };
        l.k = 1.0 + params.resonance.clamp(0.0, 1.0) * K_SPAN;
        l.drive = params.drive;
        l.use_adaa = params.adaa;

        let base_dt = 1.0 / self.sample_rate;
        let factor = params.oversample_factor();
        if factor == 1 {
            l.dt = base_dt;
            l.solve_step(x)
        } else {
            // Oversample the whole nonlinear-feedback solve: the halfband runs the
            // ladder `factor` times per output sample at the finer timestep.
            l.dt = base_dt / factor as f32;
            self.oversampler.process(x, factor, |xs| l.solve_step(xs))
        }
    }
}
