use bevy_ecs::prelude::*;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::hash::{StateHash, StateHasher, fnv1a_64};

/// The world seed (SIM-02). Also the root of every derived RNG stream.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimSeed(pub u64);

impl SimSeed {
    /// A per-system RNG stream for one tick, derived from `(seed, key, tick)`.
    ///
    /// Systems that run in parallel use this instead of borrowing [`SimRng`],
    /// so they never contend for it and never depend on run order. `key` must
    /// be a constant unique to the calling system (see `rng_key!`).
    pub fn stream(&self, key: u64, tick: u64) -> ChaCha8Rng {
        let mut seed = [0u8; 32];
        seed[0..8].copy_from_slice(&self.0.to_le_bytes());
        seed[8..16].copy_from_slice(&key.to_le_bytes());
        seed[16..24].copy_from_slice(&tick.to_le_bytes());
        seed[24..32].copy_from_slice(&0x5741_524C_4F52_4453u64.to_le_bytes());
        ChaCha8Rng::from_seed(seed)
    }
}

impl StateHash for SimSeed {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0);
    }
}

/// The single shared RNG for serial systems. Never use `rand::rng()` or
/// `thread_rng` in simulation code (enforced by clippy.toml).
#[derive(Resource, Debug, Clone)]
pub struct SimRng(pub ChaCha8Rng);

impl SimRng {
    /// The shared RNG seeded from the world seed.
    pub fn from_seed(seed: u64) -> Self {
        SimRng(ChaCha8Rng::seed_from_u64(seed))
    }
}

impl StateHash for SimRng {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(&self.0.get_seed());
        h.u128(self.0.get_word_pos());
    }
}

crate::save_newtype!(SimSeed);

impl crate::save::SaveState for SimRng {
    fn save(&self, w: &mut crate::save::SaveWriter) {
        w.raw(&self.0.get_seed());
        w.u64(self.0.get_stream());
        w.u128(self.0.get_word_pos());
    }
    fn load(r: &mut crate::save::SaveReader) -> Result<Self, crate::save::SaveError> {
        let seed: [u8; 32] = r.raw(32)?.try_into().expect("32 bytes");
        let mut rng = ChaCha8Rng::from_seed(seed);
        rng.set_stream(r.u64()?);
        rng.set_word_pos(r.u128()?);
        Ok(SimRng(rng))
    }
}

/// Builds a stable RNG stream key from a string, e.g. `rng_key("sim_agents::decide")`.
pub const fn rng_key(name: &str) -> u64 {
    fnv1a_64(name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn streams_are_reproducible() {
        let seed = SimSeed(42);
        let a: Vec<u64> = (0..4).map(|_| seed.stream(7, 100).next_u64()).collect();
        assert!(a.windows(2).all(|w| w[0] == w[1]));
    }

    #[test]
    fn streams_are_separated_by_key_tick_and_seed() {
        let base = SimSeed(42).stream(1, 1).next_u64();
        assert_ne!(base, SimSeed(42).stream(2, 1).next_u64());
        assert_ne!(base, SimSeed(42).stream(1, 2).next_u64());
        assert_ne!(base, SimSeed(43).stream(1, 1).next_u64());
    }

    #[test]
    fn rng_key_is_const_and_distinct() {
        const A: u64 = rng_key("a");
        assert_eq!(A, rng_key("a"));
        assert_ne!(A, rng_key("b"));
    }
}
