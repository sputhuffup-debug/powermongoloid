//! Captains, followers, orders, combat, morale.
//!
//! Empty until M3. Systems go in [`sim_core::SimSet`]s; this crate never
//! depends on rendering, windowing, audio or asset crates.

use bevy_app::{App, Plugin};

pub struct MilitaryPlugin;

impl Plugin for MilitaryPlugin {
    fn build(&self, _app: &mut App) {}
}
