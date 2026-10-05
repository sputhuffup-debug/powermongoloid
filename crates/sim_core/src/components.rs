use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};

use crate::hash::{StateHash, StateHasher};

/// Stable identity for every simulation entity, allocated in spawn order.
///
/// `Entity` bits are not stable across runs once entities are despawned and
/// reused, so anything order-dependent sorts by `SpawnId` instead (NFR-08).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpawnId(pub u64);

impl StateHash for SpawnId {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0);
    }
}

/// Allocator for [`SpawnId`]s.
#[derive(Resource, Debug, Default)]
pub struct NextSpawnId(pub u64);

impl NextSpawnId {
    pub fn allocate(&mut self) -> SpawnId {
        let id = SpawnId(self.0);
        self.0 += 1;
        id
    }
}

impl StateHash for NextSpawnId {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FactionId(pub u16);

/// Tile coordinates. The sub-tile part of the position lives in [`Motion`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position(pub IVec2);

impl StateHash for Position {
    fn hash_state(&self, h: &mut StateHasher) {
        h.ivec2(self.0);
    }
}

/// Sub-tile offset (in tiles, within -0.5..0.5) and velocity (tiles per tick).
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Motion {
    pub offset: Vec2,
    pub velocity: Vec2,
}

impl StateHash for Motion {
    fn hash_state(&self, h: &mut StateHasher) {
        h.vec2(self.offset);
        h.vec2(self.velocity);
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allegiance(pub FactionId);

impl StateHash for Allegiance {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u16(self.0.0);
    }
}

/// Whether an entity runs the full per-agent simulation or is dormant.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetailLevel {
    pub full: bool,
}

impl StateHash for DetailLevel {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bool(self.full);
    }
}
