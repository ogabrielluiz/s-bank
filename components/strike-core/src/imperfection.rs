//! Optional analogue-imperfection layer (our improvement) — lets the clean engine be
//! "dirtied" on demand. Off by default → bit-identical, deterministic, zero RNG.
//!
//! When enabled it applies: a fixed per-instance component tolerance (decay / cutoff /
//! level offsets drawn once from the seed), a slow random-walk drift on the decay, and
//! a pink output noise floor. Seedable and reproducible.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Default fingerprint seed (distinct from the vactrol core's).
pub const DEFAULT_SEED: u64 = 0x5354_5249_4B45_0000; // "STRIKE\0\0"

/// Pink (1/f) noise via Paul Kellet's economy filter. Output ≈ unit-scaled.
#[derive(Debug, Clone, Default)]
struct PinkNoise {
    b: [f32; 7],
}

impl PinkNoise {
    fn reset(&mut self) {
        self.b = [0.0; 7];
    }
    #[inline]
    fn next(&mut self, white: f32) -> f32 {
        let b = &mut self.b;
        b[0] = 0.99886 * b[0] + white * 0.0555179;
        b[1] = 0.99332 * b[1] + white * 0.0750759;
        b[2] = 0.96900 * b[2] + white * 0.153852;
        b[3] = 0.86650 * b[3] + white * 0.3104856;
        b[4] = 0.55000 * b[4] + white * 0.5329522;
        b[5] = -0.7616 * b[5] - white * 0.016898;
        let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362;
        b[6] = white * 0.115926;
        pink * 0.11
    }
}

/// Tiny xorshift for the per-sample hot path (no heavy RNG on the audio thread).
#[derive(Debug, Clone)]
struct XorShift {
    state: u64,
}
impl XorShift {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15 | 1,
        }
    }
    #[inline]
    fn next_f32(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        let u = (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32;
        (u as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ImperfectionConfig {
    pub enabled: bool,
    pub noise_amp: f32,
    pub drift_amount: f32,
}
impl Default for ImperfectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            noise_amp: 1.0e-4,
            drift_amount: 1.0,
        }
    }
}

const DRIFT_BLOCK: u32 = 64;

#[derive(Debug, Clone)]
pub struct Imperfection {
    pub seed: u64,
    pub config: ImperfectionConfig,
    // Fixed per-instance tolerances (multiplicative).
    tol_decay: f32,
    tol_cutoff: f32,
    tol_level: f32,
    // Hot-path state.
    noise_rng: XorShift,
    drift_rng: XorShift,
    pink_floor: PinkNoise,
    counter: u32,
    drift_decay: f32,
}

impl Imperfection {
    pub fn new(seed: u64, config: ImperfectionConfig) -> Self {
        // Draw the fingerprint once from a reproducible ChaCha stream.
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let dev = |rng: &mut ChaCha8Rng, pct: f32| 1.0 + (rng.gen::<f32>() * 2.0 - 1.0) * pct;
        Self {
            seed,
            config,
            tol_decay: dev(&mut rng, 0.10),
            tol_cutoff: dev(&mut rng, 0.06),
            tol_level: dev(&mut rng, 0.03),
            noise_rng: XorShift::new(seed ^ 0x2222_2222_2222_2222),
            drift_rng: XorShift::new(seed ^ 0x1111_1111_1111_1111),
            pink_floor: PinkNoise::default(),
            counter: 0,
            drift_decay: 0.0,
        }
    }

    pub fn from_seed(seed: u64) -> Self {
        Self::new(seed, ImperfectionConfig::default())
    }

    pub fn reset(&mut self) {
        self.noise_rng = XorShift::new(self.seed ^ 0x2222_2222_2222_2222);
        self.drift_rng = XorShift::new(self.seed ^ 0x1111_1111_1111_1111);
        self.pink_floor.reset();
        self.counter = 0;
        self.drift_decay = 0.0;
    }

    /// Advance per-block drift state. Cheap; most samples just increment.
    #[inline]
    pub fn update(&mut self) {
        if !self.config.enabled {
            return;
        }
        self.counter += 1;
        if self.counter < DRIFT_BLOCK {
            return;
        }
        self.counter = 0;
        let amt = self.config.drift_amount;
        self.drift_decay =
            (self.drift_decay * 0.99 + self.drift_rng.next_f32() * 0.0015 * amt).clamp(-0.04, 0.04);
    }

    /// Perturb the (decay, cutoff_ceiling, level) triple. Identity when disabled.
    #[inline]
    pub fn apply(&self, decay01: f32, cutoff: f32, level: f32) -> (f32, f32, f32) {
        if !self.config.enabled {
            return (decay01, cutoff, level);
        }
        (
            (decay01 * self.tol_decay + self.drift_decay).clamp(0.0, 1.0),
            cutoff * self.tol_cutoff,
            level * self.tol_level,
        )
    }

    /// Add the pink noise floor to the output. Identity when disabled.
    #[inline]
    pub fn apply_output(&mut self, y: f32) -> f32 {
        if !self.config.enabled || !self.config.noise_amp.is_finite() {
            return y;
        }
        let p = self.pink_floor.next(self.noise_rng.next_f32());
        y + p * self.config.noise_amp
    }
}
