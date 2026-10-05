//! Player-facing UI. Reads simulation state and sends `SimCommand`s; never
//! mutates the simulation directly.

use bevy::prelude::*;

#[cfg(feature = "debug_ui")]
mod debug_panel;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(feature = "debug_ui")]
        _app.add_plugins(debug_panel::DebugPanelPlugin);
    }
}
