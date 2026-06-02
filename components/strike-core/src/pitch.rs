//! Cheap input pitch/brightness estimator for the frequency-dependent decay.
//!
//! The reference rings longer for low-pitched material and shorter for high-pitched
//! material (struck-instrument behaviour). We estimate the fundamental from the
//! positive-going zero-crossing period of the audio input, gated by amplitude and
//! heavily smoothed — robust and audio-rate cheap, not a precise pitch tracker.

/// Tracks an estimated frequency (Hz) of a signal from its zero-crossing rate.
#[derive(Debug, Clone)]
pub struct PitchTracker {
    sr: f32,
    prev: f32,
    samples_since_cross: f32,
    /// Smoothed envelope (for the amplitude gate).
    env: f32,
    /// Smoothed frequency estimate (Hz).
    est_hz: f32,
}

impl PitchTracker {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sr: sample_rate,
            prev: 0.0,
            samples_since_cross: 0.0,
            env: 0.0,
            est_hz: 110.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sr = sample_rate;
    }

    pub fn reset(&mut self) {
        self.prev = 0.0;
        self.samples_since_cross = 0.0;
        self.env = 0.0;
        self.est_hz = 110.0;
    }

    /// Current smoothed estimate (Hz).
    #[inline]
    pub fn est_hz(&self) -> f32 {
        self.est_hz
    }

    /// Feed one input sample; returns the updated estimate (Hz).
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        // Amplitude follower for the gate (fast attack, slow release).
        let a = x.abs();
        self.env += if a > self.env { 0.05 } else { 0.0008 } * (a - self.env);

        self.samples_since_cross += 1.0;
        // Positive-going zero crossing with a small hysteresis, only when there is
        // enough signal to be a real fundamental (not noise/silence).
        let gate = 0.02;
        if self.env > gate && self.prev <= 0.0 && x > 0.0 {
            let period = self.samples_since_cross.max(1.0);
            let hz = (self.sr / period).clamp(20.0, 12_000.0);
            // Smooth toward the measured pitch.
            self.est_hz += 0.10 * (hz - self.est_hz);
            self.samples_since_cross = 0.0;
        }
        self.prev = x;
        self.est_hz
    }
}
