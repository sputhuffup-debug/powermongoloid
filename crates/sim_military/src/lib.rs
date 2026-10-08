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
//! 7. **Progress** (Needs, hourly): [`progression::progress`] drives
//!    settlement-level steps in a fixed order, such as [`conquest::revolts`],
//!    the daily [`wild::regrow`] and [`drifters::draw`], [`requests::step`]
//!    and [`invitations::step`].
//!
//! Settlements change hands through [`conquest::transfer`]: an `Attack`
//! that breaks the militia and holds the centre, or a revolt (GP-05,
//! GP-06). A `Garrison` order holds an owned settlement, which feeds its
//! owner's armies from its spare food.
//!
//! A landless band lives off the land with a `Forage` order ([`wild`],
//! SIM-38) and hands food to a village with `Give`. Deeds at a settlement
//! (fair trade, seizure, plunder, provocation, killing its militia,
//! answering its request) post goodwill changes to
//! `sim_agents::GoodwillLedger`, with deltas from progression.ron
//! `goodwill_deeds` (GP-22). In a run, settlements post [`requests`] the
//! sim checks off, liked settlements send [`drifters`] and offer lordship
//! ([`invitations`]).
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
pub mod conquest;
pub mod data;
pub mod drifters;
pub mod index;
pub mod invitations;
pub mod orders;
pub mod progression;
pub mod raiders;
pub mod requests;
pub mod soldiers;
pub mod spawn;
pub mod threat;
pub mod upkeep;
pub mod wild;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use sim_agents::AgentSet;
use sim_core::{AppHashExt, Calendar, SimInputSet, SimSet, on_hour};
use sim_economy::{Economy, EconomySet};

pub use combat::Casualties;
pub use components::{
    ArmyStats, Captain, Equipment, FactionStats, Factions, Follower, MilitaryLog, Order, Raider,
    Soldier, describe,
};
pub use conquest::{militia_or_foes_near, owner_of, transfer, transfer_seeded};
pub use data::Army;
pub use index::{ArmyIndex, ArmyInfo, FighterGrid, Side};
pub use orders::{CommandInbox, arm_followers, enlist, enlist_into, find_settlement, provoke};
pub use progression::{
    Champion, DriftersData, ForageData, GoodwillDeeds, InvitationData, Progression, RequestData,
    RunProgress, RunStart, RunWorld, StartDraws, StartTerrain, Traits, champion_of, draw_start,
    setup_run,
};
pub use requests::{Request, RequestBoard, RequestKind, Reward};
pub use soldiers::slot_offset;
pub use spawn::{ArmySpec, Skirmish, nearest_passable, setup_battle, setup_skirmish, spawn_army};
pub use wild::WildFood;

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
    /// The hourly progress driver: revolts and, later in M4, the run's
    /// settlement-level steps (Needs, after `Upkeep` and
    /// `AgentSet::Standing`).
    Progress,
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
            .init_resource::<WildFood>()
            .init_resource::<RequestBoard>()
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
                    // After loyalty too: desertions remove `Enlisted`, which
                    // decides who drifts and who counts (GP-06).
                    MilitarySet::Upkeep
                        .in_set(SimSet::Needs)
                        .after(AgentSet::Needs)
                        .after(AgentSet::Standing),
                    // Revolts read the day's loyalty summary (GP-06).
                    MilitarySet::Progress
                        .in_set(SimSet::Needs)
                        .after(MilitarySet::Upkeep)
                        .after(AgentSet::Standing),
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
                    progression::progress
                        .run_if(on_hour)
                        .in_set(MilitarySet::Progress),
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
            .register_hashed_resource::<WildFood>()
            .register_hashed_resource::<RequestBoard>()
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
