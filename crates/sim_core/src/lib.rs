//! Core simulation crate: the fixed-tick schedule (SIM-03), the seeded RNG,
//! stable spawn IDs, the command path and the state hash (NFR-08).
//!
//! Depends only on `bevy_ecs`, `bevy_app` and `bevy_math` (ADR 0002).

pub mod command;
pub mod components;
pub mod demo;
pub mod hash;
pub mod rng;
pub mod schedule;
pub mod step;
pub mod timing;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;

pub use command::{CommandKind, CommandsProcessed, SimCommand};
pub use components::{Allegiance, DetailLevel, FactionId, Motion, NextSpawnId, Position, SpawnId};
pub use hash::{AppHashExt, StateHash, StateHasher, compute_state_hash};
pub use rng::{SimRng, SimSeed};
pub use schedule::{SIM_TICK_HZ, SimSet, SimTick};
pub use step::{SimStepper, run_sim_tick};
pub use timing::SimTimings;

/// Registers the core resources, the ordered [`SimSet`]s in `FixedUpdate`,
/// and the core components in the state hash.
pub struct SimCorePlugin {
    pub seed: u64,
}

impl Plugin for SimCorePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SimSeed(self.seed))
            .insert_resource(SimRng::from_seed(self.seed))
            .init_resource::<SimTick>()
            .init_resource::<NextSpawnId>()
            .init_resource::<SimTimings>()
            .init_resource::<CommandsProcessed>()
            .add_message::<SimCommand>();

        schedule::configure_sets(app);
        timing::add_markers(app);

        app.add_systems(
            FixedUpdate,
            (
                command::drain_commands.in_set(SimSet::Input),
                schedule::advance_tick.in_set(SimSet::Bookkeeping),
            ),
        );

        app.register_hashed_resource::<SimSeed>()
            .register_hashed_resource::<SimTick>()
            .register_hashed_resource::<SimRng>()
            .register_hashed_resource::<NextSpawnId>()
            .register_hashed_resource::<CommandsProcessed>()
            .register_hashed::<SpawnId>()
            .register_hashed::<Position>()
            .register_hashed::<Motion>()
            .register_hashed::<Allegiance>()
            .register_hashed::<DetailLevel>();
    }
}

/// Spawns `bundle` with the next stable [`SpawnId`]. All simulation entities
/// must be spawned through this so the state hash and stable ordering see them.
pub fn spawn_sim<B: Bundle>(commands: &mut Commands, ids: &mut NextSpawnId, bundle: B) -> Entity {
    let id = ids.allocate();
    commands.spawn((id, bundle)).id()
}
