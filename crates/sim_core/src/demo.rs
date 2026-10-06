//! M0 placeholder: wandering entities so the state hash covers moving,
//! RNG-driven state before real systems exist. Delete once M2 villagers land.

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use rand::RngExt;

use crate::hash::{AppHashExt, StateHash, StateHasher};
use crate::rng::rng_key;
use crate::{Motion, NextSpawnId, Position, SimSeed, SimSet, SimTick, SpawnId, spawn_sim};

/// Marks a placeholder wanderer.
#[derive(Component, Debug, Clone, Copy)]
pub struct Wanderer;

impl StateHash for Wanderer {
    fn hash_state(&self, _h: &mut StateHasher) {}
}

pub struct DemoWanderersPlugin {
    pub count: u32,
}

#[derive(Resource)]
struct DemoCount(u32);

impl StateHash for DemoCount {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.0);
    }
}

impl Plugin for DemoWanderersPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DemoCount(self.count))
            .register_hashed::<Wanderer>()
            .register_hashed_resource::<DemoCount>()
            .add_systems(
                FixedUpdate,
                (
                    spawn_wanderers
                        .in_set(SimSet::Input)
                        .run_if(resource_equals(SimTick(0))),
                    wander.in_set(SimSet::Move),
                ),
            );
    }
}

fn spawn_wanderers(
    mut commands: Commands,
    mut ids: ResMut<NextSpawnId>,
    count: Res<DemoCount>,
    seed: Res<SimSeed>,
) {
    let mut rng = seed.stream(rng_key("sim_core::demo::spawn"), 0);
    for _ in 0..count.0 {
        let tile = IVec2::new(rng.random_range(-16..=16), rng.random_range(-16..=16));
        spawn_sim(
            &mut commands,
            &mut ids,
            (Wanderer, Position(tile), Motion::default()),
        );
    }
}

fn wander(
    mut q: Query<(&SpawnId, &mut Position, &mut Motion), With<Wanderer>>,
    seed: Res<SimSeed>,
    tick: Res<SimTick>,
) {
    let _span = tracing::info_span!("sim_core::demo::wander").entered();
    let mut rng = seed.stream(rng_key("sim_core::demo::wander"), tick.0);
    // Draw order must not depend on query order: sort by stable SpawnId.
    let mut agents: Vec<_> = q.iter_mut().collect();
    agents.sort_unstable_by_key(|(id, ..)| **id);
    for (_, mut pos, mut motion) in agents {
        let jitter = Vec2::new(rng.random_range(-0.02..0.02), rng.random_range(-0.02..0.02));
        motion.velocity = (motion.velocity + jitter).clamp_length_max(0.1);
        // Drift back toward the origin so wanderers stay on screen.
        let world_pos = pos.0.as_vec2() + motion.offset;
        if world_pos.length() > 20.0 {
            motion.velocity -= world_pos.normalize() * 0.01;
        }
        let offset = motion.offset + motion.velocity;
        let step = offset.round();
        pos.0 += step.as_ivec2();
        motion.offset = offset - step;
    }
}
