//! Armies: captains, followers, orders, crowd movement, combat, morale and
//! upkeep (GP-01..04, SIM-13, SIM-30..32). Design: ADR 0010 and ADR 0011.
//!
//! # Tick order ([`MilitarySet`])
//! 1. **Orders** (Input): [`orders`] applies validated commands, then each
//!    captain works its first order and deals with settlements (the serial
//!    logistics step).
//! 2. **Index** (Knowledge): `index::build_index` builds the
//!    [`ArmyIndex`] and [`FighterGrid`]; [`threat`] sets each settlement's
//!    threat.
//! 3. **Decide**: [`soldiers`] choose to follow, fight or rout, and aim
//!    blows.
//! 4. **Move**: captains walk the goal field; followers keep loose formation
//!    and descend the rally field when far away.
//! 5. **Combat** (Act): [`combat`] resolves blows serially by SpawnId and
//!    removes the fallen.
//! 6. **Upkeep** (Needs): [`upkeep`] covers eating, morale, starvation and
//!    desertion.
//!
//! Also here: army data ([`Army`], `army.ron`), [`spawn::spawn_army`] and
//! the playtest setup, the scripted [`raiders`] (in `SimInputSet::Ai`), and
//! runs ([`progression`], `progression.ron`, GP-21/27): the start draws,
//! [`setup_run`], offers and the champion.
//!
//! Systems go in [`sim_core::SimSet`]s. This crate never depends on
//! rendering, windowing, audio or asset crates.

pub mod combat;
pub mod components;
pub mod data;
pub mod index;
pub mod orders;
pub mod progression;
pub mod raiders;
pub mod soldiers;
pub mod spawn;
pub mod threat;
pub mod upkeep;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use sim_agents::AgentSet;
use sim_core::{AppHashExt, Calendar, SimInputSet, SimSet};
use sim_economy::{Economy, EconomySet};

pub use combat::Casualties;
pub use components::{
    ArmyStats, Captain, Equipment, FactionStats, Factions, Follower, MilitaryLog, Order, Raider,
    Soldier, describe,
};
pub use data::Army;
pub use index::{ArmyIndex, ArmyInfo, FighterGrid, Side};
pub use orders::{CommandInbox, arm_followers, enlist, find_settlement, provoke};
pub use progression::{
    Champion, Progression, RunProgress, RunStart, RunWorld, StartDraws, StartTerrain, Traits,
    champion_of, draw_start, setup_run,
};
pub use soldiers::slot_offset;
pub use spawn::{ArmySpec, Skirmish, nearest_passable, setup_battle, setup_skirmish, spawn_army};

/// Military systems, in tick order within their `SimSet`s.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MilitarySet {
    /// Commands applied, orders progressed (in `SimInputSet::Consume`).
    Orders,
    /// Army index, fighter grid, settlement threat (Knowledge).
    Index,
    /// Soldiers and militia choose (Decide).
    Decide,
    /// Captains, then soldiers, move (Move).
    Move,
    /// Blows land, the fallen are removed (Act).
    Combat,
    /// Hunger, morale, desertion (Needs).
    Upkeep,
}

/// Captains, followers, orders, combat and morale.
pub struct MilitaryPlugin;

impl Plugin for MilitaryPlugin {
    fn build(&self, app: &mut App) {
        let calendar = app
            .world()
            .get_resource::<Calendar>()
            .cloned()
            .unwrap_or_else(Calendar::default_data);
        let econ = app
            .world()
            .get_resource::<Economy>()
            .cloned()
            .unwrap_or_else(Economy::default_data);
        app.insert_resource(Army::default_data(&calendar, &econ))
            .insert_resource(Progression::default_data())
            .init_resource::<RunProgress>()
            .init_resource::<Factions>()
            .init_resource::<ArmyStats>()
            .init_resource::<MilitaryLog>()
            .init_resource::<CommandInbox>()
            .init_resource::<ArmyIndex>()
            .init_resource::<FighterGrid>()
            .init_resource::<Casualties>()
            .init_resource::<upkeep::Deserters>()
            .configure_sets(
                FixedUpdate,
                (
                    MilitarySet::Orders.in_set(SimInputSet::Consume),
                    MilitarySet::Index.in_set(SimSet::Knowledge),
                    MilitarySet::Decide
                        .in_set(SimSet::Decide)
                        .after(AgentSet::Decide),
                    MilitarySet::Move.in_set(SimSet::Move).after(AgentSet::Move),
                    MilitarySet::Combat
                        .in_set(SimSet::Act)
                        .after(EconomySet::Resolve),
                    MilitarySet::Upkeep
                        .in_set(SimSet::Needs)
                        .after(AgentSet::Needs),
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    raiders::think.in_set(SimInputSet::Ai),
                    (
                        orders::collect_commands,
                        orders::apply_commands.run_if(orders::has_commands),
                        orders::run_orders.run_if(any_with_component::<Captain>),
                    )
                        .chain()
                        .in_set(MilitarySet::Orders),
                    (index::build_index, threat::update_threats)
                        .chain()
                        .in_set(MilitarySet::Index),
                    soldiers::decide.in_set(MilitarySet::Decide),
                    (soldiers::move_captains, soldiers::move_soldiers)
                        .chain()
                        .in_set(MilitarySet::Move),
                    upkeep::eat
                        .in_set(EconomySet::Post)
                        .after(AgentSet::Perform),
                    (
                        combat::resolve_strikes,
                        combat::find_fallen,
                        combat::casualties.run_if(combat::anyone_fell),
                    )
                        .chain()
                        .in_set(MilitarySet::Combat),
                    (
                        upkeep::soldier_upkeep,
                        upkeep::civilian_morale,
                        upkeep::tally,
                        upkeep::apply_desertions.run_if(upkeep::anyone_deserting),
                    )
                        .chain()
                        .in_set(MilitarySet::Upkeep),
                ),
            )
            .register_hashed_resource::<Army>()
            .register_hashed_resource::<Factions>()
            .register_hashed_resource::<ArmyStats>()
            .register_hashed::<Captain>()
            .register_hashed::<Follower>()
            .register_hashed::<Equipment>()
            .register_hashed::<Soldier>()
            .register_hashed::<Raider>()
            .register_hashed_resource::<Progression>()
            .register_hashed_resource::<RunProgress>()
            .register_hashed::<Champion>()
            .register_hashed::<Traits>()
            .exempt_from_hash::<MilitaryLog>("UI history; its effects are in the hashed state")
            .exempt_from_hash::<CommandInbox>("always empty between ticks")
            .exempt_from_hash::<ArmyIndex>("rebuilt from hashed state every tick")
            .exempt_from_hash::<FighterGrid>("rebuilt from hashed state every tick")
            .exempt_from_hash::<Casualties>("this tick's fallen; derived from hashed state")
            .exempt_from_hash::<upkeep::Deserters>(
                "this tick's deserters; derived from hashed state",
            );
    }
}
