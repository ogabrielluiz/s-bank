//! Oversampling for the in-loop resonance nonlinearity.
//!
//! A 2x stage upsamples with a polyphase halfband FIR, applies the supplied
//! per-sample function at the doubled rate, then decimates with the same halfband.
//! The audio path passes its entire delay-free `solve_step` as that function, so
//! the whole feedback solve (not a memoryless buffer) runs at the finer timestep.
//! 4x is two stages cascaded (the inner stage runs as the outer stage's function).
//! 1x is a passthrough. The factor is selectable per call so the bench harness can
//! compare 1x / 2x / 4x.
//!
//! All state is preallocated at construction; `process` only shifts fixed-length
//! histories and multiplies, so it is allocation-free on the audio thread.
//!
//! The halfband is a Blackman-windowed sinc at cutoff 0.5 (relative to the 2x
//! Nyquist). Length is `HALFBAND_LEN` with the centre tap on an even index, so the
//! even polyphase phase reduces to a pure delay (the defining halfband property)
//! and the odd phase does the interpolation.

use std::f32::consts::PI;

/// Halfband FIR length. Must satisfy `LEN ≡ 1 (mod 4)` so the centre index is
/// even and the even polyphase phase reduces to a pure delay. Longer gives a
/// sharper transition and deeper stopband (better alias rejection) at more cost.
pub const HALFBAND_LEN: usize = 61;

/// Blackman-windowed-sinc halfband taps at cutoff 0.5, normalized for unity DC
/// gain. Shared by the scalar and SIMD oversamplers so they cannot drift.
pub(crate) fn halfband_taps() -> Vec<f32> {
    let l = HALFBAND_LEN;
    let c = (l - 1) / 2; // centre, even by construction (l = 4k+1)
    let mut h = vec![0.0f32; l];
    for (k, hk) in h.iter_mut().enumerate() {
        let x = 0.5 * (k as f32 - c as f32); // cutoff 0.5 (halfband)
        let sinc = if x == 0.0 {
            1.0
        } else {
            (PI * x).sin() / (PI * x)
        };
        let n = l as f32 - 1.0;
        let w =
            0.42 - 0.5 * (2.0 * PI * k as f32 / n).cos() + 0.08 * (4.0 * PI * k as f32 / n).cos();
        *hk = 0.5 * sinc * w;
    }
    let sum: f32 = h.iter().sum();
    for hk in &mut h {
        *hk /= sum;
    }
    h
}

/// Polyphase decomposition of a halfband: `(even, odd)` subfilters, taps `h[2m+p]`.
pub(crate) fn halfband_polyphase(h: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let l = h.len();
    let he = (0..)
        .map(|m| 2 * m)
        .take_while(|&i| i < l)
        .map(|i| h[i])
        .collect();
    let ho = (0..)
        .map(|m| 2 * m + 1)
        .take_while(|&i| i < l)
        .map(|i| h[i])
        .collect();
    (he, ho)
}

/// One 2x oversampling stage with its own halfband state.
///
/// Upsampling uses the halfband's polyphase decomposition (even phase is a pure
/// delay, odd phase interpolates). Decimation runs the full halfband over the
/// interleaved 2x stream and takes one output per base sample, which sidesteps
/// the phase-alignment pitfalls of a polyphase decimator.
#[derive(Debug, Clone)]
struct HalfbandStage {
    /// Full halfband taps (used for decimation), newest-first dot order.
    h: Vec<f32>,
    /// Even polyphase taps (upsample phase 0): a near-pure delay.
    he: Vec<f32>,
    /// Odd polyphase taps (upsample phase 1): the interpolation.
    ho: Vec<f32>,
    /// Base-rate input history, newest at index 0.
    xh: Vec<f32>,
    /// 2x-rate shaped-sample history (decimator input), newest at index 0.
    y2: Vec<f32>,
}

impl HalfbandStage {
    fn new() -> Self {
        let h = halfband_taps();
        let (he, ho) = halfband_polyphase(&h);
        Self {
            xh: vec![0.0; he.len()],
            y2: vec![0.0; h.len()],
            h,
            he,
            ho,
        }
    }

    fn reset(&mut self) {
        self.xh.iter_mut().for_each(|v| *v = 0.0);
        self.y2.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Push one sample into a newest-at-zero history.
    #[inline]
    fn push(hist: &mut [f32], x: f32) {
        let n = hist.len();
        hist.copy_within(0..n - 1, 1);
        hist[0] = x;
    }

    #[inline]
    fn process<F: FnMut(f32) -> f32>(&mut self, x: f32, f: &mut F) -> f32 {
        // Upsample: two phase outputs (gain 2 compensates the zero-stuffing).
        Self::push(&mut self.xh, x);
        let mut up0 = 0.0;
        for (h, &s) in self.he.iter().zip(&self.xh) {
            up0 += h * s;
        }
        let mut up1 = 0.0;
        for (h, &s) in self.ho.iter().zip(&self.xh) {
            up1 += h * s;
        }
        up0 *= 2.0;
        up1 *= 2.0;

        // Apply the nonlinearity at the doubled rate, in time order.
        Self::push(&mut self.y2, f(up0));
        Self::push(&mut self.y2, f(up1));

        // Decimate: one full-halfband evaluation per base sample.
        let mut out = 0.0;
        for (h, &s) in self.h.iter().zip(&self.y2) {
            out += h * s;
        }
        out
    }
}

/// Oversampled waveshaper supporting 1x / 2x / 4x.
#[derive(Debug, Clone)]
pub struct Oversampler {
    stage_a: HalfbandStage,
    stage_b: HalfbandStage,
}

impl Default for Oversampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Oversampler {
    pub fn new() -> Self {
        Self {
            stage_a: HalfbandStage::new(),
            stage_b: HalfbandStage::new(),
        }
    }

    pub fn reset(&mut self) {
        self.stage_a.reset();
        self.stage_b.reset();
    }

    /// Approximate added latency (base-rate samples) for a given factor. The
    /// halfband group delay is `(HALFBAND_LEN - 1) / 2` at the oversampled rate,
    /// applied on the way up and back down.
    pub fn latency_samples(factor: usize) -> f32 {
        match factor {
            1 => 0.0,
            // Up (2x) + down (2x) group delay, expressed at the base rate.
            2 => (HALFBAND_LEN - 1) as f32 / 2.0,
            _ => (HALFBAND_LEN - 1) as f32 / 2.0 * 1.5,
        }
    }

    /// Apply `f` at `factor`x oversampling. `factor` is clamped to {1, 2, 4}.
    #[inline]
    pub fn process<F: FnMut(f32) -> f32>(&mut self, x: f32, factor: usize, mut f: F) -> f32 {
        match factor {
            0 | 1 => f(x),
            2 => self.stage_a.process(x, &mut f),
            _ => {
                // 4x: the inner 2x stage runs as the outer stage's nonlinearity.
                let Self { stage_a, stage_b } = self;
                stage_a.process(x, &mut |s| stage_b.process(s, &mut f))
            }
        }
    }
}
