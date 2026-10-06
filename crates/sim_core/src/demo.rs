//! M0 placeholder: wandering entities so the state hash covers moving,
//! RNG-driven state before real systems exist. Delete once M2 villagers land.

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use rand::RngExt;

use crate::hash::{AppHashExt, StateHash, StateHasher};
use crate::rng::rng_key;
use crate::{Motion, NextSpawnId, Position, SimSeed, SimSet, SimTick, SpawnId, spawn_sim};

/// A placeholder wanderer that drifts around its home tile.
#[derive(Component, Debug, Clone, Copy)]
pub struct Wanderer {
    pub home: IVec2,
}

impl StateHash for Wanderer {
    fn hash_state(&self, h: &mut StateHasher) {
        h.ivec2(self.home);
    }
}

/// Tiles the wanderers are spread around (settlement centres once a world
/// exists; the origin otherwise). Filled in by `sim_world`.
#[derive(Resource, Debug, Default, Clone)]
pub struct DemoAnchors(pub Vec<IVec2>);

impl StateHash for DemoAnchors {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0.len() as u64);
        for a in &self.0 {
            h.ivec2(*a);
        }
    }
}

/// How far wanderers stray from home before drifting back, in tiles.
const LEASH: f32 = 6.0;

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
            .init_resource::<DemoAnchors>()
            .register_hashed::<Wanderer>()
            .register_hashed_resource::<DemoCount>()
            .register_hashed_resource::<DemoAnchors>()
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
    anchors: Res<DemoAnchors>,
    seed: Res<SimSeed>,
) {
    let mut rng = seed.stream(rng_key("sim_core::demo::spawn"), 0);
    for i in 0..count.0 as usize {
        let home = match anchors.0.len() {
            0 => IVec2::ZERO,
            n => anchors.0[i % n],
        };
        let tile = home + IVec2::new(rng.random_range(-4..=4), rng.random_range(-4..=4));
        spawn_sim(
            &mut commands,
            &mut ids,
            (Wanderer { home }, Position(tile), Motion::default()),
        );
    }
}

fn wander(
    mut q: Query<(&SpawnId, &Wanderer, &mut Position, &mut Motion)>,
    seed: Res<SimSeed>,
    tick: Res<SimTick>,
) {
    let _span = tracing::info_span!("sim_core::demo::wander").entered();
    let mut rng = seed.stream(rng_key("sim_core::demo::wander"), tick.0);
    // Draw order must not depend on query order: sort by stable SpawnId.
    let mut agents: Vec<_> = q.iter_mut().collect();
    agents.sort_unstable_by_key(|(id, ..)| **id);
    for (_, wanderer, mut pos, mut motion) in agents {
        let jitter = Vec2::new(rng.random_range(-0.02..0.02), rng.random_range(-0.02..0.02));
        motion.velocity = (motion.velocity + jitter).clamp_length_max(0.1);
        // Drift back toward home so wanderers stay near their settlement.
        let from_home = (pos.0 - wanderer.home).as_vec2() + motion.offset;
        if from_home.length() > LEASH {
            motion.velocity -= from_home.normalize() * 0.01;
        }
        let offset = motion.offset + motion.velocity;
        let step = offset.round();
        pos.0 += step.as_ivec2();
        motion.offset = offset - step;
    }
}
