//! `cargo run -p game_app [-- --seed <u64>]`

use bevy::prelude::*;
use sim_core::SIM_TICK_HZ;
use sim_headless::{DEFAULT_DEMO_AGENTS, SimPlugins};

fn seed_from_args() -> u64 {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == "--seed")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(42)
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Warlord Sim".into(),
                ..default()
            }),
            ..default()
        }))
        // SIM-03: the simulation advances only in FixedUpdate, at 10 Hz.
        .insert_resource(Time::<Fixed>::from_hz(SIM_TICK_HZ))
        .add_plugins(SimPlugins {
            seed: seed_from_args(),
            demo_agents: DEFAULT_DEMO_AGENTS,
        })
        .add_plugins((game_render::RenderPlugin, game_ui::UiPlugin))
        .run();
}
