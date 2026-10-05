//! Villagers, needs, utility AI, occupations.
//!
//! Empty until M2. Systems go in [`sim_core::SimSet`]s; this crate never
//! depends on rendering, windowing, audio or asset crates.

use bevy_app::{App, Plugin};

pub struct AgentsPlugin;

impl Plugin for AgentsPlugin {
    fn build(&self, _app: &mut App) {}
}
