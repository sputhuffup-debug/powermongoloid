//! Core simulation crate: the fixed-tick schedule (SIM-03), the calendar,
//! seeded randomness, stable spawn IDs, the command path and replays, and the
//! state hash (NFR-08).
//!
//! # Main pieces
//! - [`SimSet`]: the nine ordered phases of a tick in `FixedUpdate`. Every sim
//!   system belongs to exactly one. [`SimTick`] counts ticks.
//! - [`SimCommand`]: the only way input reaches the sim. [`SimInputSet::Inject`]
//!   replays scripted commands and records the player's, the sim's AI writes
//!   its commands in [`SimInputSet::Ai`], and consumers run in
//!   [`SimInputSet::Consume`].
//! - [`SpawnId`] and [`spawn_sim`]: stable ids that every order-dependent
//!   step sorts by.
//! - [`SimRng`] and [`SimSeed::stream`]: the only sources of randomness.
//! - [`compute_state_hash`] and [`AppHashExt`]: every sim component and
//!   resource is registered in the hash or exempted with a reason.
//! - [`Replay`] and [`CommandScript`]: recorded sessions.
//! - [`Calendar`]: day length, phases and seasons (`calendar.ron`), and game
//!   hours ([`on_hour`], [`on_day`]), and the first tick ([`first_tick`]).
//! - [`RunState`] and [`RunConfig`] (GP-21): a run's settings and outcome.
//!   Once a run ends, [`run_in_progress`] gates every [`SimSet`] off and the
//!   clock stops. [`Offers`] queues the player's pending choices (GP-27).
//! - [`SimEvent`]: typed events from any sim crate, written inside ticks
//!   (setup writes none), recorded each tick by
//!   [`events::record_events`] into the [`EventFeed`] and the [`Chronicle`]
//!   (UI-05, UI-09).
//! - [`FactionModifiers`]: per-faction [`Stat`] bonuses, read by every crate.
//! - [`save_world`] and [`load_world`] (GP-12): every hashed type is also
//!   registered with [`AppSaveExt`] and written as [`SaveState`].
//!
//! Depends only on `bevy_ecs`, `bevy_app` and `bevy_math` (ADR 0002). See
//! `docs/dev/ARCHITECTURE.md` and `docs/dev/DETERMINISM.md`.

pub mod calendar;
pub mod command;
/// Core components: SpawnId, position, motion, allegiance, detail level.
pub mod components;
pub mod events;
pub mod hash;
pub mod modifiers;
pub mod names;
pub mod replay;
/// Seeded randomness: the shared `SimRng` and per-system streams.
pub mod rng;
pub mod run;
pub mod save;
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
pub use events::{
    BattleReport, Cause, Chronicle, ChronicleEntry, EventFeed, FeedEntry, Promotion, SimEvent,
    SimEventKind,
};
pub use hash::{AppHashExt, StateHash, StateHasher, compute_state_hash, unhashed_state};
pub use modifiers::{FactionModifiers, ModifierEntry, ModifierSource, Stat};
pub use names::{NameId, NameTable};
pub use replay::{Checkpoint, REPLAY_FORMAT, Replay};
pub use rng::{SimRng, SimSeed};
pub use run::{
    Difficulty, Offer, OfferKind, Offers, RunConfig, RunOutcome, RunState, in_run, post_offer,
    run_in_progress,
};
pub use save::{
    AppSaveExt, SAVE_FORMAT, SAVE_MAGIC, SaveError, SaveHeader, SaveReader, SaveRegistry,
    SaveState, SaveWriter, load_world, read_header, save_world,
};
pub use schedule::{BookkeepingSet, SIM_TICK_HZ, SimSet, SimTick, first_tick, on_day, on_hour};
pub use step::{SimStepper, run_sim_tick};
pub use timing::SimTimings;

/// Registers the core resources, the ordered [`SimSet`]s in `FixedUpdate`,
/// and the core components in the state hash.
pub struct SimCorePlugin {
    /// The world seed (SIM-02).
    pub seed: u64,
    /// `Some` to play a run (GP-21); `None` for the developed world and the
    /// M2/M3 scenarios.
    pub run: Option<RunConfig>,
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
            .insert_resource(RunState {
                config: self.run,
                ..RunState::default()
            })
            .init_resource::<Offers>()
            .init_resource::<Chronicle>()
            .init_resource::<EventFeed>()
            .init_resource::<FactionModifiers>()
            .add_message::<SimCommand>()
            .add_message::<SimEvent>();

        schedule::configure_sets(app);
        timing::add_markers(app);

        app.configure_sets(
            FixedUpdate,
            (SimInputSet::Inject, SimInputSet::Ai, SimInputSet::Consume)
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
                (
                    events::record_events,
                    run::settle_outcome.run_if(run::outcome_pending),
                )
                    .chain()
                    .in_set(BookkeepingSet::Record),
                schedule::advance_tick.in_set(BookkeepingSet::Advance),
            ),
        );

        app.register_hashed_resource::<SimSeed>()
            .register_hashed_resource::<Calendar>()
            .register_hashed_resource::<SimTick>()
            .register_hashed_resource::<SimRng>()
            .register_hashed_resource::<NextSpawnId>()
            .register_hashed_resource::<CommandsProcessed>()
            .register_hashed_resource::<NameTable>()
            .register_hashed_resource::<RunState>()
            .register_hashed_resource::<Offers>()
            .register_hashed_resource::<Chronicle>()
            .register_hashed_resource::<FactionModifiers>()
            .register_hashed::<SpawnId>()
            .register_hashed::<Position>()
            .register_hashed::<Motion>()
            .register_hashed::<Allegiance>()
            .register_hashed::<DetailLevel>()
            .exempt_from_hash::<SimTimings>("wall-clock diagnostics for the debug panel")
            .exempt_from_hash::<CommandLog>("history; its effects are in the hashed state")
            .exempt_from_hash::<CommandScript>("pending replay input, not yet applied")
            .exempt_from_hash::<EventFeed>("UI feed, derived from SimEvents");

        // GP-12: every hashed type is saved too; the feed is saved so the
        // UI's cursor carries on after a load.
        app.register_saved_resource::<SimSeed>()
            .register_saved_resource::<Calendar>()
            .register_saved_resource::<SimTick>()
            .register_saved_resource::<SimRng>()
            .register_saved_resource::<NextSpawnId>()
            .register_saved_resource::<CommandsProcessed>()
            .register_saved_resource::<NameTable>()
            .register_saved_resource::<RunState>()
            .register_saved_resource::<Offers>()
            .register_saved_resource::<Chronicle>()
            .register_saved_resource::<FactionModifiers>()
            .register_saved_resource::<EventFeed>()
            .register_saved::<SpawnId>()
            .register_saved::<Position>()
            .register_saved::<Motion>()
            .register_saved::<Allegiance>()
            .register_saved::<DetailLevel>();
    }
}

/// Spawns `bundle` with the next stable [`SpawnId`]. All simulation entities
/// must be spawned through this so the state hash and stable ordering see them.
pub fn spawn_sim<B: Bundle>(commands: &mut Commands, ids: &mut NextSpawnId, bundle: B) -> Entity {
    let id = ids.allocate();
    commands.spawn((id, bundle)).id()
}
