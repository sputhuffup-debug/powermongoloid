//! Player-facing UI. Reads simulation state and sends `SimCommand`s; never
//! mutates the simulation directly.
//!
//! Plugins: the time bar (GP-11); selection and picking; the
//! villager/soldier inspector and settlement panel (UI-02, UI-03); the
//! captain panel, right-click orders and village menu ([`army`], UI-01);
//! settlement labels; and, in dev builds, the egui debug panel (UI-07).
//! For M4 runs: the new-game and end screens ([`screens`], [`UiScreen`]),
//! the run bar with its offer cards, the event feed (the chronicle's
//! sentences and the military log), and monument tooltips; saves
//! ([`SaveRequest`]: F5, F9, "Resume") and the [`Notice`] line.
//!
//! When `game_app` replaces the sim world (a load or a new run) it bumps
//! [`game_render::WorldEpoch`]; the UI then forgets the old world: the
//! selection, labels, answered offers (`SentOffers`), the open
//! card popup, the feed (it re-reads the loaded feed's last lines without
//! pausing on them), the new-game draft, and which screen is up.

use bevy::prelude::*;
use sim_core::Difficulty;

pub mod army;
mod cards;
#[cfg(feature = "debug_ui")]
mod debug_panel;
mod feed;
mod labels;
mod panels;
mod run_bar;
mod saves;
pub mod screens;
pub mod selection;
mod style;
mod time_bar;
mod tooltip;

pub use saves::{Notice, SaveRequest, SavedRun, SavedRuns};
pub use selection::Selection;

/// Which screen owns the window (M4 runs). Outside a run the game is always
/// `Playing`.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UiScreen {
    /// The island, the HUD and the panels.
    #[default]
    Playing,
    /// Full screen: seed, difficulty, run modifiers and the start boon.
    NewGame,
    /// Full screen: the run's outcome and numbers.
    Ended,
}

/// System sets of the UI (all in `Update`, chained in this order).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSet {
    /// Which screen is up, and the input gate.
    Screens,
    /// Bars, cards, the feed and tooltips.
    Hud,
}

/// Asked for by the new-game and end screens: start a fresh run. The game
/// binary handles it (it builds the island in the background and loads it
/// into the running game; see `game_app`).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewRunRequest {
    /// The new run's seed.
    pub seed: u64,
    /// Its difficulty.
    pub difficulty: Difficulty,
}

/// Run condition: no full-screen UI is up, so hotkeys and world clicks
/// count.
pub fn playing(screen: Option<Res<State<UiScreen>>>) -> bool {
    screen.is_none_or(|s| *s.get() == UiScreen::Playing)
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<UiScreen>()
            .add_message::<NewRunRequest>()
            .configure_sets(Update, (UiSet::Screens, UiSet::Hud).chain())
            .add_plugins((
                labels::SettlementLabelsPlugin,
                selection::SelectionPlugin,
                panels::PanelsPlugin,
                time_bar::TimeBarPlugin,
                army::ArmyUiPlugin,
                screens::ScreensPlugin,
                cards::CardsPlugin,
                run_bar::RunBarPlugin,
                feed::FeedPlugin,
                tooltip::TooltipPlugin,
                saves::SavesUiPlugin,
            ));
        #[cfg(feature = "debug_ui")]
        app.add_plugins(debug_panel::DebugPanelPlugin);
    }
}
