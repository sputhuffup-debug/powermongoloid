//! The full simulation as one plugin group, shared by the headless binary,
//! integration tests, benchmarks and `game_app`.

use bevy_app::{App, PluginGroup, PluginGroupBuilder};
use sim_core::demo::DemoWanderersPlugin;
use sim_core::{SimCorePlugin, SimStepper, compute_state_hash};

/// Default number of M0 placeholder wanderers.
pub const DEFAULT_DEMO_AGENTS: u32 = 256;

/// Every `sim_*` plugin, in dependency order.
pub struct SimPlugins {
    pub seed: u64,
    pub demo_agents: u32,
}

impl PluginGroup for SimPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SimCorePlugin { seed: self.seed })
            .add(sim_world::WorldPlugin)
            .add(sim_agents::AgentsPlugin)
            .add(sim_economy::EconomyPlugin)
            .add(sim_military::MilitaryPlugin)
            .add(sim_knowledge::KnowledgePlugin)
            .add(sim_lod::LodPlugin)
            .add(sim_ai::AiPlugin)
            .add(DemoWanderersPlugin {
                count: self.demo_agents,
            })
    }
}

/// Builds a windowless simulation app, ready to [`SimStepper::step`].
pub fn headless_app(seed: u64, demo_agents: u32) -> App {
    let mut app = App::new();
    app.add_plugins(SimPlugins { seed, demo_agents })
        .prepare_sim();
    app
}

/// Runs `ticks` ticks from `seed` and returns the final state hash.
pub fn run_and_hash(seed: u64, ticks: u64, demo_agents: u32) -> u64 {
    let mut app = headless_app(seed, demo_agents);
    app.step(ticks);
    compute_state_hash(app.world_mut())
}
