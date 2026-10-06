//! Player-facing UI. Reads simulation state and sends `SimCommand`s; never
//! mutates the simulation directly.
//!
//! Plugins: the time bar (GP-11); selection and picking; the
//! villager/soldier inspector and settlement panel (UI-02, UI-03); the
//! captain panel, right-click orders and village menu ([`army`], UI-01);
//! settlement labels; and, in dev builds, the egui debug panel (UI-07).

use bevy::prelude::*;

pub mod army;
#[cfg(feature = "debug_ui")]
mod debug_panel;
mod labels;
mod panels;
pub mod selection;
mod style;
mod time_bar;

pub use selection::Selection;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            labels::SettlementLabelsPlugin,
            selection::SelectionPlugin,
            panels::PanelsPlugin,
            time_bar::TimeBarPlugin,
            army::ArmyUiPlugin,
        ));
        #[cfg(feature = "debug_ui")]
        app.add_plugins(debug_panel::DebugPanelPlugin);
    }
}
