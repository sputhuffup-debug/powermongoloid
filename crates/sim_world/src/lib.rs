//! Terrain, worldgen, regions, spatial grid, flow fields.
//!
//! Empty until M1. Systems go in [`sim_core::SimSet`]s; this crate never
//! depends on rendering, windowing, audio or asset crates.

use bevy_app::{App, Plugin};

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, _app: &mut App) {}
}
