//! Core simulation crate: the fixed-tick schedule (SIM-03), the calendar,
//! seeded randomness, stable spawn IDs, the command path and replays, and the
//! state hash (NFR-08).
//!
//! # Main pieces
//! - [`SimSet`]: the nine ordered phases of a tick in `FixedUpdate`. Every sim
//!   system belongs to exactly one. [`SimTick`] counts ticks.
//! - [`SimCommand`]: the only way input reaches the sim. [`SimInputSet::Inject`]
//!   replays scripted commands and records the player's; consumers run in
//!   [`SimInputSet::Consume`].
//! - [`SpawnId`] and [`spawn_sim`]: stable ids that every order-dependent
//!   step sorts by.
//! - [`SimRng`] and [`SimSeed::stream`]: the only sources of randomness.
//! - [`compute_state_hash`] and [`AppHashExt`]: every sim component and
//!   resource is registered in the hash or exempted with a reason.
//! - [`Replay`] and [`CommandScript`]: recorded sessions.
//! - [`Calendar`]: day length, phases and seasons (`calendar.ron`).
//!
//! Depends only on `bevy_ecs`, `bevy_app` and `bevy_math` (ADR 0002). See
//! `docs/dev/ARCHITECTURE.md` and `docs/dev/DETERMINISM.md`.

pub mod calendar;
pub mod command;
/// Core components: SpawnId, position, motion, allegiance, detail level.
pub mod components;
pub mod hash;
pub mod names;
pub mod replay;
/// Seeded randomness: the shared `SimRng` and per-system streams.
pub mod rng;
/// The ordered tick phases (`SimSet`) and the tick counter.
pub mod schedule;
/// Stepping the simulation outside the game loop (headless, tests).
pub mod step;
pub mod timing;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;

pub use calendar::{Calendar, ClockReading, Phase};
pub use command::{
    CommandKind, CommandLog, CommandOrigin, CommandScript, CommandsProcessed, LoggedCommand,
    OrderSpec, Posture, SimCommand, SimInputSet,
};
pub use components::{Allegiance, DetailLevel, FactionId, Motion, NextSpawnId, Position, SpawnId};
pub use hash::{AppHashExt, StateHash, StateHasher, compute_state_hash, unhashed_state};
pub use names::{NameId, NameTable};
pub use replay::{Checkpoint, REPLAY_FORMAT, Replay};
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
            .insert_resource(Calendar::default_data())
            .insert_resource(SimRng::from_seed(self.seed))
            .init_resource::<SimTick>()
            .init_resource::<NextSpawnId>()
            .init_resource::<SimTimings>()
            .init_resource::<CommandsProcessed>()
            .init_resource::<CommandLog>()
            .init_resource::<CommandScript>()
            .init_resource::<NameTable>()
            .add_message::<SimCommand>();

        schedule::configure_sets(app);
        timing::add_markers(app);

        app.configure_sets(
            FixedUpdate,
            (SimInputSet::Inject, SimInputSet::Consume)
                .chain()
                .in_set(SimSet::Input),
        )
        .add_systems(
            FixedUpdate,
            (
                (command::inject_scripted, command::record_commands)
                    .chain()
                    .in_set(SimInputSet::Inject),
                command::drain_commands.in_set(SimInputSet::Consume),
                schedule::advance_tick.in_set(SimSet::Bookkeeping),
            ),
        );

        app.register_hashed_resource::<SimSeed>()
            .register_hashed_resource::<Calendar>()
            .register_hashed_resource::<SimTick>()
            .register_hashed_resource::<SimRng>()
            .register_hashed_resource::<NextSpawnId>()
            .register_hashed_resource::<CommandsProcessed>()
            .register_hashed_resource::<NameTable>()
            .register_hashed::<SpawnId>()
            .register_hashed::<Position>()
            .register_hashed::<Motion>()
            .register_hashed::<Allegiance>()
            .register_hashed::<DetailLevel>()
            .exempt_from_hash::<SimTimings>("wall-clock diagnostics for the debug panel")
            .exempt_from_hash::<CommandLog>("history; its effects are in the hashed state")
            .exempt_from_hash::<CommandScript>("pending replay input, not yet applied");
    }
}

/// Spawns `bundle` with the next stable [`SpawnId`]. All simulation entities
/// must be spawned through this so the state hash and stable ordering see them.
pub fn spawn_sim<B: Bundle>(commands: &mut Commands, ids: &mut NextSpawnId, bundle: B) -> Entity {
    let id = ids.allocate();
    commands.spawn((id, bundle)).id()
}
