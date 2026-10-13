//! Villagers: population, needs, occupations and the utility AI (SIM-10..13).
//!
//! # Main pieces
//! - [`Villager`], [`Needs`], [`Activity`], [`Travel`], [`Employment`].
//! - The AI ([`ai`]): every [`AgentSet::Decide`] tick, a tenth of villagers
//!   score their options (eat, fetch, sleep, work or haul, go home, idle,
//!   hide, flee, fight) and record why in [`DecisionTrace`] (ADR 0006).
//! - Actions (`act::perform`, in `EconomySet::Post`): carrying out the
//!   chosen action. Goods move only through `Transfers`.
//! - Walking ([`movement`]): parallel path planning and movement.
//! - [`FoodSecurity`], plus the combat-side components that every person
//!   carries ([`CombatStats`], [`Strike`], [`Threat`] on settlements,
//!   [`Enlisted`] for soldiers).
//! - [`population::populate`]: creates households when the world is built:
//!   one region in the developed world, every settlement at its drawn size
//!   on the run island (GP-21).
//! - Standing and loyalty (GP-05, GP-06, SIM-24; ADR 0014): each populated
//!   settlement's [`Owner`] and [`Goodwill`], [`set_owner`] to hand it over
//!   ([`standing`]), and each villager's [`Loyalty`], drifted daily and
//!   summed into [`SettlementLoyalty`] ([`loyalty`], [`AgentSet::Standing`]).
//!   Deeds move goodwill through the [`GoodwillLedger`], applied with its
//!   spread to nearby settlements first in [`AgentSet::Standing`] (GP-22).
//! - Lordship (M4a, ADR 0020), run only: a settlement's itemised
//!   [`LoyaltyTerms`] (the bias of its loyalty target) and dated
//!   [`DeedLog`], an [`Agent`] living in a rival town, the [`edicts`]
//!   ([`Edicts`], [`Rationed`], [`Mustered`], [`Assignment`]) and a
//!   settlement's [`Specialty`]. Inserted by sim_military's `setup_run`
//!   (or populate's run branch); absent elsewhere.
//!
//! Tuning lives in `villagers.ron` ([`Villagers`]). Never depends on
//! rendering, windowing, audio or asset crates.

pub mod act;
pub mod ai;
pub mod components;
pub mod data;
pub mod edicts;
pub mod food;
pub mod jobs;
pub mod loyalty;
pub mod movement;
pub mod population;
pub mod standing;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use sim_core::{AppHashExt, AppSaveExt, Calendar, SimSet, on_day};
use sim_economy::EconomySet;

pub use act::eat_from;
pub use components::{
    ActionKind, Activity, CombatStats, DecisionTrace, Employment, Enlisted, Factors, Needs,
    NextDecision, ScoredOption, Stage, Stance, Strike, Threat, Travel, TravelState, Villager,
};
pub use data::Villagers;
pub use edicts::{Assignment, DialState, Edicts, Mustered, Rationed};
pub use food::FoodSecurity;
pub use loyalty::{Loyalty, LoyaltyTerm, LoyaltyTerms, SettlementLoyalty};
pub use population::civilian_stats;
pub use population::{PopulationReport, Specialty};
pub use standing::{
    Agent, Deed, DeedEntry, DeedLog, Goodwill, GoodwillChange, GoodwillLedger, Owner, OwnerChange,
    set_owner,
};

/// Villager systems, for ordering other crates' systems around them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentSet {
    Decide,
    Move,
    Perform,
    Needs,
    /// Standing and loyalty (SimSet::Needs, after `Needs`):
    /// `apply_goodwill` when deeds are waiting (GP-22), then the daily
    /// `drift_loyalty` and `aggregate_loyalty` (GP-06, SIM-24).
    Standing,
}

/// Villagers. `population` overrides the total in `villagers.ron`.
#[derive(Default)]
pub struct AgentsPlugin {
    /// Overrides `villagers.ron`'s total in the developed world. Ignored on
    /// the run island, where every settlement has a drawn size (GP-21).
    pub population: Option<u32>,
}

impl Plugin for AgentsPlugin {
    fn build(&self, app: &mut App) {
        let calendar = app
            .world()
            .get_resource::<Calendar>()
            .cloned()
            .unwrap_or_else(Calendar::default_data);
        let mut villagers = Villagers::default_data(&calendar);
        if let Some(n) = self.population {
            villagers = villagers.with_population(n);
        }
        app.insert_resource(villagers)
            .init_resource::<PopulationReport>()
            .init_resource::<GoodwillLedger>()
            .add_systems(
                FixedUpdate,
                (
                    ai::decide.in_set(SimSet::Decide).in_set(AgentSet::Decide),
                    (movement::plan_paths, movement::move_agents)
                        .chain()
                        .in_set(SimSet::Move)
                        .in_set(AgentSet::Move),
                    act::perform
                        .in_set(EconomySet::Post)
                        .in_set(AgentSet::Perform),
                    (act::update_needs, food::update_food_security)
                        .in_set(SimSet::Needs)
                        .in_set(AgentSet::Needs),
                    // Goodwill first, so a day's drift sees today's deeds.
                    (
                        standing::apply_goodwill.run_if(standing::ledger_nonempty),
                        loyalty::drift_loyalty.run_if(on_day),
                        loyalty::aggregate_loyalty.run_if(on_day),
                    )
                        .chain()
                        .in_set(AgentSet::Standing),
                ),
            )
            .configure_sets(
                FixedUpdate,
                AgentSet::Standing
                    .in_set(SimSet::Needs)
                    .after(AgentSet::Needs),
            )
            .register_hashed_resource::<Villagers>()
            .register_hashed::<Villager>()
            .register_hashed::<Employment>()
            .register_hashed::<Needs>()
            .register_hashed::<Activity>()
            .register_hashed::<Travel>()
            .register_hashed::<NextDecision>()
            .register_hashed::<FoodSecurity>()
            .register_hashed::<Enlisted>()
            .register_hashed::<CombatStats>()
            .register_hashed::<Threat>()
            .register_hashed::<Strike>()
            .register_hashed::<Owner>()
            .register_hashed::<Goodwill>()
            .register_hashed::<Loyalty>()
            .register_hashed::<SettlementLoyalty>()
            .register_hashed_resource::<GoodwillLedger>()
            // M4a (ADR 0020): Lordship's per-town and per-person state,
            // present only in a run.
            .register_hashed::<LoyaltyTerms>()
            .register_hashed::<DeedLog>()
            .register_hashed::<Agent>()
            .register_hashed::<Edicts>()
            .register_hashed::<Rationed>()
            .register_hashed::<Mustered>()
            .register_hashed::<Assignment>()
            .register_hashed::<Specialty>()
            .exempt_from_hash::<DecisionTrace>(
                "explains the last decision; derived, never read by the sim",
            )
            .exempt_from_hash::<PopulationReport>("setup diagnostics")
            // GP-12: every hashed type is saved, and DecisionTrace's
            // presence (the deciding systems query it).
            .register_saved_resource_with::<Villagers>(data::save_villagers, data::load_villagers)
            .register_saved::<Villager>()
            .register_saved::<Employment>()
            .register_saved::<Needs>()
            .register_saved::<Activity>()
            .register_saved::<Travel>()
            .register_saved::<NextDecision>()
            .register_saved::<FoodSecurity>()
            .register_saved::<Enlisted>()
            .register_saved::<CombatStats>()
            .register_saved::<Threat>()
            .register_saved::<Strike>()
            .register_saved::<Owner>()
            .register_saved::<Goodwill>()
            .register_saved::<Loyalty>()
            .register_saved::<SettlementLoyalty>()
            .register_saved_resource::<GoodwillLedger>()
            .register_saved::<LoyaltyTerms>()
            .register_saved::<DeedLog>()
            .register_saved::<Agent>()
            .register_saved::<Edicts>()
            .register_saved::<Rationed>()
            .register_saved::<Mustered>()
            .register_saved::<Assignment>()
            .register_saved::<Specialty>()
            .register_saved::<DecisionTrace>();
    }

    fn finish(&self, app: &mut App) {
        // Needs the world (WorldPlugin::finish runs first) and the economy.
        if app.world().contains_resource::<sim_world::WorldMap>() {
            population::populate(app.world_mut());
        }
    }
}
