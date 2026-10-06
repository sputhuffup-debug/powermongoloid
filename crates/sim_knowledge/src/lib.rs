//! Faction knowledge, messengers, line of sight.
//!
//! Empty until M5. Systems go in [`sim_core::SimSet`]s; this crate never
//! depends on rendering, windowing, audio or asset crates.

use bevy_app::{App, Plugin};

/// Placeholder plugin for faction knowledge and messengers (M5).
pub struct KnowledgePlugin;

impl Plugin for KnowledgePlugin {
    fn build(&self, _app: &mut App) {}
}
