//! Goods, inventories, recipes, stockpiles, trade.
//!
//! Empty until M2. Systems go in [`sim_core::SimSet`]s; this crate never
//! depends on rendering, windowing, audio or asset crates.

use bevy_app::{App, Plugin};

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, _app: &mut App) {}
}
