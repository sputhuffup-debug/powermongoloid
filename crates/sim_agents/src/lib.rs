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

pub use components::{
    ActionKind, Activity, DecisionTrace, Employment, Needs, NextDecision, ScoredOption, Stage,
    Travel, TravelState, Villager,
};
pub use data::Villagers;
pub use food::FoodSecurity;
pub use population::PopulationReport;

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
                    ai::decide.in_set(SimSet::Decide),
                    (movement::plan_paths, movement::move_agents)
                        .chain()
                        .in_set(SimSet::Move),
                    act::perform.in_set(EconomySet::Post),
                    (act::update_needs, food::update_food_security).in_set(SimSet::Needs),
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
