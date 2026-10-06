//! Promotion and demotion between aggregate and full detail.
//!
//! Empty until M5. Systems go in [`sim_core::SimSet`]s; this crate never
//! depends on rendering, windowing, audio or asset crates.

use bevy_app::{App, Plugin};

/// Placeholder plugin for simulation LOD (M5).
pub struct LodPlugin;

impl Plugin for LodPlugin {
    fn build(&self, _app: &mut App) {}
}
