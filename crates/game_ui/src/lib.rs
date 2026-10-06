//! Player-facing UI. Reads simulation state and sends `SimCommand`s; never
//! mutates the simulation directly.

use bevy::prelude::*;

#[cfg(feature = "debug_ui")]
mod debug_panel;
mod labels;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(labels::SettlementLabelsPlugin);
        #[cfg(feature = "debug_ui")]
        app.add_plugins(debug_panel::DebugPanelPlugin);
    }
}
