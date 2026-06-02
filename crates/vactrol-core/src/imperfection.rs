//! Analogue imperfection layer (Phase 3) -- the headline realism feature.
//!
//! Four independently toggleable subsystems, all fed by explicit seedable RNG
//! streams so tests can pin seeds:
//!   1. Per-instance component tolerance ("fingerprinting"), seeded and serialized.
//!   2. Per-block parameter drift (random-walk + pink/1f + low-frequency noise).
//!   3. Sub-Hz thermal wander biasing the vactrol constants.
//!   4. An aggregate output noise floor.
//!
//! Phase 1 provides the seeded shell and a no-op apply so the serialized state
//! (which carries the seed) and the bypass path are settled. Disabled is the
//! Phase 1 default and must be zero-cost.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Clone)]
pub struct Imperfection {
    pub enabled: bool,
    pub seed: u64,
    /// Component-tolerance RNG stream, derived deterministically from `seed`.
    _tolerance_rng: ChaCha8Rng,
}

impl Imperfection {
    /// Build from a stored fingerprint seed. Disabled by default.
    pub fn from_seed(seed: u64) -> Self {
        Self {
            enabled: false,
            seed,
            _tolerance_rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Phase 1 no-op. Phase 3 applies tolerance/drift/thermal to parameters and
    /// adds the noise floor to the output.
    #[inline]
    pub fn apply_output(&mut self, y: f32) -> f32 {
        y
    }
}
