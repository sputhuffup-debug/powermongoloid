//! Villagers: households, needs, occupations and utility AI (SIM-10,
//! SIM-11, SIM-12).
//!
//! Never depends on rendering, windowing, audio or asset crates.

pub mod act;
pub mod ai;
pub mod components;
pub mod data;
pub mod food;
pub mod movement;
pub mod population;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use sim_core::{AppHashExt, Calendar, SimSet};
use sim_economy::EconomySet;

pub use act::eat_from;
pub use components::{
    ActionKind, Activity, CombatStats, DecisionTrace, Employment, Enlisted, Factors, Needs,
    NextDecision, ScoredOption, Stage, Stance, Strike, Threat, Travel, TravelState, Villager,
};
pub use data::Villagers;
pub use food::FoodSecurity;
pub use population::PopulationReport;
pub use population::civilian_stats;

/// Villager systems, for ordering other crates' systems around them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentSet {
    Decide,
    Move,
    Perform,
    Needs,
}

/// Villagers. `population` overrides the total in `villagers.ron`.
#[derive(Default)]
pub struct AgentsPlugin {
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
                ),
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
            .exempt_from_hash::<DecisionTrace>(
                "explains the last decision; derived, never read by the sim",
            )
            .exempt_from_hash::<PopulationReport>("setup diagnostics");
    }

    fn finish(&self, app: &mut App) {
        // Needs the world (WorldPlugin::finish runs first) and the economy.
        if app.world().contains_resource::<sim_world::WorldMap>() {
            population::populate(app.world_mut());
        }
    }
}
