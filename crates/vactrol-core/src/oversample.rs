//! Oversampling (Phase 2).
//!
//! The target is a 2x polyphase halfband oversampler (FIR halfband, with an IIR
//! option documented) wrapped around the nonlinear/feedback stage, with the
//! factor (1x / 2x / 4x) configurable for the benchmark harness. Phase 1 provides
//! the type and a passthrough so the signal flow and FFI surface are settled.

#[derive(Debug, Clone)]
pub struct Oversampler {
    /// Oversampling factor (1, 2, or 4). 1 == passthrough.
    pub factor: usize,
}

impl Oversampler {
    pub fn new(factor: usize) -> Self {
        Self {
            factor: factor.max(1),
        }
    }

    /// Phase 1 passthrough. Phase 2 upsamples, runs `f` per oversampled frame,
    /// and downsamples through the halfband.
    #[inline]
    pub fn process<F: FnMut(f32) -> f32>(&mut self, x: f32, mut f: F) -> f32 {
        f(x)
    }
}
