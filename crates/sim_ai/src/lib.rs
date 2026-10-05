//! Rival warlord strategic AI.
//!
//! Empty until M5. Systems go in [`sim_core::SimSet`]s; this crate never
//! depends on rendering, windowing, audio or asset crates.

use bevy_app::{App, Plugin};

pub struct AiPlugin;

impl Plugin for AiPlugin {
    fn build(&self, _app: &mut App) {}
}
