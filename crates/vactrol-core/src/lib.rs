//! Portable virtual-analog vactrol low-pass gate DSP core.
//!
//! Signal flow per sample:
//!   CV in -> [control_path] LED current -> [vactrol] resistance Rf
//!         -> [audio_path] TPT 2-pole 292 cell (Both / VCA / Lowpass) -> out
//!
//! The core has no dependency on the VCV Rack SDK, so the whole test and
//! benchmark pipeline runs headless. A thin C++ adapter links the staticlib over
//! the [`ffi`] C ABI.
//!
//! ## SIMD readiness
//! The scalar `f32` path is the reference. `process_sample` is branch-light and
//! works one frame at a time over plain arithmetic, so a lane-parametric SIMD
//! variant (Rack's `float_4`, a `__m128` wrapper of four `f32`) is a drop-in: the
//! same expressions vectorize across four polyphony voices. SIMD lands in a later
//! phase; the structure here is what makes that cheap.

pub mod audio_path;
pub mod control_path;
pub mod ffi;
pub mod imperfection;
pub mod nonlinear;
pub mod oversample;
pub mod params;
pub mod vactrol;

pub use params::{Components, Mode, Params, SerializedState};

use audio_path::AudioPath;
use control_path::ControlPath;
use vactrol::Vactrol;

/// One voice of the vactrol low-pass gate.
#[derive(Debug, Clone)]
pub struct Lpg {
    sample_rate: f32,
    params: Params,
    comp: Components,
    control: ControlPath,
    vactrol: Vactrol,
    audio: AudioPath,
    last_rf: f32,
}

impl Lpg {
    /// Build a voice at the given sample rate with default parameters/components.
    pub fn new(sample_rate: f32) -> Self {
        let comp = Components::default();
        Self {
            sample_rate,
            params: Params::default(),
            comp,
            control: ControlPath::new(sample_rate),
            vactrol: Vactrol::new(sample_rate),
            audio: AudioPath::new(sample_rate),
            last_rf: comp.r_off,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.control.set_sample_rate(sample_rate);
        self.vactrol.set_sample_rate(sample_rate);
        self.audio.set_sample_rate(sample_rate);
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn set_params(&mut self, params: Params) {
        self.params = params;
    }

    pub fn params(&self) -> &Params {
        &self.params
    }

    pub fn components(&self) -> &Components {
        &self.comp
    }

    /// Last vactrol resistance (ohms). Useful for tests and metering.
    pub fn last_rf(&self) -> f32 {
        self.last_rf
    }

    /// Clear all filter/envelope state back to silence and a closed gate.
    pub fn reset(&mut self) {
        self.control.reset();
        self.vactrol.reset(&self.comp);
        self.audio.reset();
        self.last_rf = self.comp.r_off;
    }

    /// Process one sample. `audio_in` is the audio signal, `cv_in` the control
    /// voltage. Allocation-free.
    #[inline]
    pub fn process_sample(&mut self, audio_in: f32, cv_in: f32) -> f32 {
        let current = self
            .control
            .process(cv_in, self.params.cv_offset, &self.comp);
        let rf = self.vactrol.process(current, &self.comp);
        self.last_rf = rf;
        self.audio.process(
            audio_in,
            rf,
            self.params.mode,
            self.params.resonance,
            self.params.drive,
            &self.comp,
        )
    }

    /// Process a block. `audio_in`, `cv_in`, and `out` should share a length;
    /// processing runs over the shortest. Allocation-free.
    #[inline]
    pub fn process_block(&mut self, audio_in: &[f32], cv_in: &[f32], out: &mut [f32]) {
        for ((&a, &c), o) in audio_in.iter().zip(cv_in).zip(out.iter_mut()) {
            *o = self.process_sample(a, c);
        }
    }
}
