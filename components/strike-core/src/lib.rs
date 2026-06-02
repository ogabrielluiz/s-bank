//! S-Bank **Strike** — a clean, zero-bleed, envelope-driven low-pass gate.
//!
//! An independent design in the spirit of the well-known "natural"-style dual LPG
//! that deliberately avoids vactrols: the gate fully closes (no bleed), there is no
//! resonance, and the character lives in a carefully shaped envelope generator. The
//! sibling [`vactrol-core`] instrument covers the dirty/resonant vactrol world.
//!
//! Signature behaviours: dual-mode decay (click ↔ multi-second ring), no-reset memory
//! accumulation on rapid HITs, frequency-dependent decay (higher pitch rings shorter),
//! CTRL pre-EG / OPEN post-EG floor, and a clean ping (feed DC at the input → the raw
//! envelope appears at the output). Improvements over the reference: a continuous
//! MATERIAL morph and an optional, seedable analogue-imperfection layer.
//!
//! Signal flow per sample:
//! ```text
//! HIT ─▶ Envelope (shaped, memory, freq-dep decay) ─┐
//! CTRL (pre, clamp) ────────────────────────────────┤─▶ control 0..1 ─▶ Gate (cutoff+VCA) ─▶ ×level ─▶ out
//! OPEN (post floor) ─────────────────────────────────┘                         ▲
//! DECAY (+ CV) ─▶ decay time                              MATERIAL ────────────┘ (attack/ceiling/level)
//! IN (audio; feed DC for ping/EG-out) ─────────────────────────────▶ Gate
//! ```

pub mod envelope;
pub mod gate;
pub mod imperfection;
pub mod material;
pub mod pitch;

pub use imperfection::{Imperfection, ImperfectionConfig, DEFAULT_SEED};

use envelope::Envelope;
use gate::Gate;
use material::Material;
use pitch::PitchTracker;

/// Reference pitch (Hz) at which the frequency-dependent decay factor is 1.0.
const PITCH_REF_HZ: f32 = 220.0;

/// User-facing parameters (knob positions, `0..=1` where noted). CV inputs (CTRL,
/// DECAY) and the HIT trigger are passed per-sample to [`Strike::process_sample`].
#[derive(Debug, Clone, Copy)]
pub struct StrikeParams {
    /// Post-EG floor: the gate never closes below this. `0` = full close.
    pub open: f32,
    /// Decay-time control / pivot for DECAY CV. `0` = percussive, `1` = long ring.
    pub decay: f32,
    /// MATERIAL morph `0..1`: hard (fast/bright/loud) → soft (slow/dull/quieter).
    pub material: f32,
}

impl Default for StrikeParams {
    fn default() -> Self {
        Self {
            open: 0.0,
            decay: 0.4,
            material: 0.0,
        }
    }
}

/// One Strike voice.
#[derive(Debug, Clone)]
pub struct Strike {
    sample_rate: f32,
    params: StrikeParams,
    envelope: Envelope,
    gate: Gate,
    pitch: PitchTracker,
    imperfection: Imperfection,
    last_control: f32,
}

impl Strike {
    pub fn new(sample_rate: f32) -> Self {
        Self::with_seed(sample_rate, DEFAULT_SEED)
    }

    pub fn with_seed(sample_rate: f32, seed: u64) -> Self {
        Self {
            sample_rate,
            params: StrikeParams::default(),
            envelope: Envelope::new(sample_rate),
            gate: Gate::new(sample_rate),
            pitch: PitchTracker::new(sample_rate),
            imperfection: Imperfection::from_seed(seed),
            last_control: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.envelope.set_sample_rate(sample_rate);
        self.gate.set_sample_rate(sample_rate);
        self.pitch.set_sample_rate(sample_rate);
    }

    pub fn set_params(&mut self, params: StrikeParams) {
        self.params = params;
    }

    pub fn params(&self) -> &StrikeParams {
        &self.params
    }

    pub fn set_imperfection(&mut self, config: ImperfectionConfig) {
        self.imperfection.config = config;
        self.imperfection.reset();
    }

    pub fn imperfection_config(&self) -> &ImperfectionConfig {
        &self.imperfection.config
    }

    pub fn seed(&self) -> u64 {
        self.imperfection.seed
    }

    /// Last gate-opening control value `0..=1` — useful for an "openness" LED.
    pub fn last_control(&self) -> f32 {
        self.last_control
    }

    pub fn reset(&mut self) {
        self.envelope.reset();
        self.gate.reset();
        self.pitch.reset();
        self.imperfection.reset();
        self.last_control = 0.0;
    }

    /// Frequency factor for the decay: `<1` shortens (high pitch), `>1` lengthens.
    #[inline]
    fn pitch_factor(&self, est_hz: f32) -> f32 {
        (PITCH_REF_HZ / est_hz.max(20.0)).sqrt().clamp(0.25, 2.0)
    }

    /// Process one sample.
    ///
    /// * `audio_in` – the audio to gate. For a clean **ping / EG-out**, feed a constant
    ///   DC value here (what the module does when its IN jack is unpatched): the output
    ///   becomes the raw envelope shape.
    /// * `ctrl01` – pre-EG opening in `0..1` (CTRL, normalled/attenuverted by the host).
    /// * `decay_mod` – additive DECAY modulation in `0..1` units (DECAY CV·attenuverter).
    /// * `hit_v` – HIT trigger (volts); a rising edge past the threshold fires.
    #[inline]
    pub fn process_sample(&mut self, audio_in: f32, ctrl01: f32, decay_mod: f32, hit_v: f32) -> f32 {
        self.imperfection.update();

        let mat = Material::from01(self.params.material);
        let decay_eff = (self.params.decay + decay_mod).clamp(0.0, 1.0);
        let (decay_eff, cutoff_ceiling, level) =
            self.imperfection.apply(decay_eff, mat.cutoff_ceiling, mat.level);

        let est_hz = self.pitch.process(audio_in);
        let pf = self.pitch_factor(est_hz);

        let control = self.envelope.process(
            decay_eff,
            pf,
            mat.attack_tau,
            ctrl01,
            self.params.open,
            hit_v,
        );
        self.last_control = control;

        let y = self.gate.process(audio_in, control, cutoff_ceiling) * level;
        self.imperfection.apply_output(y)
    }

    /// Process a block in place over the shortest of the slices.
    #[inline]
    pub fn process_block(
        &mut self,
        audio_in: &[f32],
        ctrl01: &[f32],
        decay_mod: &[f32],
        hit_v: &[f32],
        out: &mut [f32],
    ) {
        for i in 0..out.len() {
            out[i] = self.process_sample(audio_in[i], ctrl01[i], decay_mod[i], hit_v[i]);
        }
    }
}
