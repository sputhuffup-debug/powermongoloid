//! Deterministic state hash over every registered simulation resource and
//! component (NFR-08). See ADR 0003.

use std::collections::HashMap;

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_ecs::world::EntityRef;
use bevy_math::{IVec2, Vec2};

use crate::components::SpawnId;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64-bit over a byte slice. Stable across runs and builds.
pub const fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

/// Implemented by every simulation component and resource that is part of
/// the world state. Floats hash by bit pattern; entity references must go
/// through [`StateHasher::entity`] so they hash as stable [`SpawnId`]s.
pub trait StateHash {
    fn hash_state(&self, h: &mut StateHasher);
}

/// FNV-1a accumulator with helpers for simulation value types.
pub struct StateHasher<'a> {
    state: u64,
    spawn_ids: &'a HashMap<Entity, SpawnId>,
}

impl<'a> StateHasher<'a> {
    pub fn new(spawn_ids: &'a HashMap<Entity, SpawnId>) -> Self {
        Self {
            state: FNV_OFFSET,
            spawn_ids,
        }
    }

    pub fn finish(&self) -> u64 {
        self.state
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    pub fn bool(&mut self, v: bool) {
        self.bytes(&[v as u8]);
    }
    pub fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }
    pub fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn u128(&mut self, v: u128) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub fn ivec2(&mut self, v: IVec2) {
        self.i32(v.x);
        self.i32(v.y);
    }
    pub fn vec2(&mut self, v: Vec2) {
        self.f32(v.x);
        self.f32(v.y);
    }

    /// Hashes an entity reference as its [`SpawnId`]. Entities without one
    /// (not simulation entities, or despawned) hash as a distinct marker.
    pub fn entity(&mut self, e: Entity) {
        match self.spawn_ids.get(&e) {
            Some(id) => {
                self.u8(1);
                self.u64(id.0);
            }
            None => self.u8(0),
        }
    }
}

type ResourceHashFn = fn(&World, &mut StateHasher);
type ComponentHashFn = fn(&EntityRef, &mut StateHasher);

/// Every resource and component type that is part of the simulation state.
/// Each `sim_*` plugin registers its own types via [`AppHashExt`].
#[derive(Resource, Default)]
pub struct HashRegistry {
    resources: Vec<(&'static str, ResourceHashFn)>,
    components: Vec<(&'static str, ComponentHashFn)>,
}

impl HashRegistry {
    pub fn resource_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.resources.iter().map(|(n, _)| *n)
    }
    pub fn component_names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.components.iter().map(|(n, _)| *n)
    }
}

pub trait AppHashExt {
    /// Includes component `T` in the state hash.
    fn register_hashed<T: Component + StateHash>(&mut self) -> &mut Self;
    /// Includes resource `T` in the state hash.
    fn register_hashed_resource<T: Resource + StateHash>(&mut self) -> &mut Self;
}

impl AppHashExt for App {
    fn register_hashed<T: Component + StateHash>(&mut self) -> &mut Self {
        let name = std::any::type_name::<T>();
        let mut reg = self.world_mut().get_resource_or_init::<HashRegistry>();
        if !reg.components.iter().any(|(n, _)| *n == name) {
            reg.components.push((name, |e, h| match e.get::<T>() {
                Some(c) => {
                    h.u8(1);
                    c.hash_state(h);
                }
                None => h.u8(0),
            }));
        }
        self
    }

    fn register_hashed_resource<T: Resource + StateHash>(&mut self) -> &mut Self {
        let name = std::any::type_name::<T>();
        let mut reg = self.world_mut().get_resource_or_init::<HashRegistry>();
        if !reg.resources.iter().any(|(n, _)| *n == name) {
            reg.resources
                .push((name, |w, h| match w.get_resource::<T>() {
                    Some(r) => {
                        h.u8(1);
                        r.hash_state(h);
                    }
                    None => h.u8(0),
                }));
        }
        self
    }
}

/// Hashes all registered resources, then every entity with a [`SpawnId`] in
/// ascending SpawnId order, through every registered component.
pub fn compute_state_hash(world: &mut World) -> u64 {
    let mut entities: Vec<(SpawnId, Entity)> = world
        .query::<(Entity, &SpawnId)>()
        .iter(world)
        .map(|(e, id)| (*id, e))
        .collect();
    entities.sort_unstable();
    let spawn_ids: HashMap<Entity, SpawnId> = entities.iter().map(|&(id, e)| (e, id)).collect();

    let world: &World = world;
    let registry = world.get_resource::<HashRegistry>();
    let mut h = StateHasher::new(&spawn_ids);
    let Some(registry) = registry else {
        return h.finish();
    };

    for (name, f) in &registry.resources {
        h.bytes(name.as_bytes());
        f(world, &mut h);
    }
    h.u64(entities.len() as u64);
    for &(id, e) in &entities {
        h.u64(id.0);
        let entity = world.entity(e);
        for (_, f) in &registry.components {
            f(&entity, &mut h);
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NextSpawnId, Position};

    #[test]
    fn fnv_matches_reference_vectors() {
        assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a_64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    fn app_with(positions: &[(u64, IVec2)]) -> App {
        let mut app = App::new();
        app.register_hashed::<SpawnId>()
            .register_hashed::<Position>();
        for &(id, p) in positions {
            app.world_mut().spawn((SpawnId(id), Position(p)));
        }
        app.init_resource::<NextSpawnId>();
        app
    }

    #[test]
    fn hash_ignores_spawn_order_but_not_values() {
        let a = compute_state_hash(app_with(&[(0, IVec2::ZERO), (1, IVec2::ONE)]).world_mut());
        let b = compute_state_hash(app_with(&[(1, IVec2::ONE), (0, IVec2::ZERO)]).world_mut());
        let c = compute_state_hash(app_with(&[(0, IVec2::ZERO), (1, IVec2::X)]).world_mut());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn registration_is_idempotent() {
        let mut app = App::new();
        app.register_hashed::<Position>()
            .register_hashed::<Position>();
        assert_eq!(
            app.world()
                .resource::<HashRegistry>()
                .component_names()
                .count(),
            1
        );
    }
}
