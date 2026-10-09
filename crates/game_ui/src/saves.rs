//! The UI side of saves (GP-12): what the player asks of the save slots
//! ([`SaveRequest`], handled by `game_app` between ticks), the saved runs
//! `game_app` found on disk ([`SavedRuns`], the new-game screen's
//! "Resume"), and a one-line [`Notice`] at the top of the screen for saves,
//! loads and islands being built.

use std::path::PathBuf;

use bevy::prelude::*;
use sim_core::SaveHeader;

use crate::UiSet;
use crate::style;

pub struct SavesUiPlugin;

impl Plugin for SavesUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SaveRequest>()
            .init_resource::<SavedRuns>()
            .init_resource::<Notice>()
            .add_systems(Startup, spawn_notice)
            .add_systems(Update, (quick_keys, show_notice).in_set(UiSet::Hud));
    }
}

/// Asked of `game_app`, which does it between ticks (GP-12).
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub enum SaveRequest {
    /// Save this run to its slot (F5).
    Save,
    /// Load this run's slot (F9).
    Load,
    /// Resume the saved run in this file (the new-game screen).
    Resume(PathBuf),
}

/// A saved run on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedRun {
    /// The slot file.
    pub path: PathBuf,
    /// Its header: seed, difficulty and tick.
    pub header: SaveHeader,
}

/// Saved runs, newest first. `game_app` fills it at startup and after
/// every save, load and new run.
#[derive(Resource, Debug, Clone, Default)]
pub struct SavedRuns(pub Vec<SavedRun>);

/// A one-line notice at the top of the screen ("Saved day 3", "Building
/// the island ...").
#[derive(Resource, Debug, Clone, Default)]
pub struct Notice {
    /// What it says.
    pub text: String,
    /// Real seconds since startup at which it hides.
    pub until: f32,
}

impl Notice {
    /// Shows `text` for `secs` real seconds from `now`.
    pub fn show(&mut self, text: impl Into<String>, now: f32, secs: f32) {
        self.text = text.into();
        self.until = now + secs;
    }
}

#[derive(Component)]
struct NoticeText;

/// F5 saves and F9 loads this run's slot (GP-12; kept for development,
/// M4_PLAN). Works on every screen; `game_app` refuses what makes no sense
/// (saving a finished run).
fn quick_keys(keys: Res<ButtonInput<KeyCode>>, mut out: MessageWriter<SaveRequest>) {
    let _span = info_span!("game_ui::saves::quick_keys").entered();
    if keys.just_pressed(KeyCode::F5) {
        out.write(SaveRequest::Save);
    }
    if keys.just_pressed(KeyCode::F9) {
        out.write(SaveRequest::Load);
    }
}

fn spawn_notice(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(130.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            GlobalZIndex(30),
        ))
        .with_children(|p| {
            p.spawn((
                NoticeText,
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                    ..default()
                },
                Text::new(""),
                style::font(15.0),
                TextColor(style::GOLD),
                BackgroundColor(style::PANEL),
                Visibility::Hidden,
            ));
        });
}

fn show_notice(
    notice: Res<Notice>,
    time: Res<Time<Real>>,
    mut text: Query<(&mut Text, &mut Visibility), With<NoticeText>>,
) {
    let _span = info_span!("game_ui::saves::show_notice").entered();
    let Ok((mut t, mut vis)) = text.single_mut() else {
        return;
    };
    let up = !notice.text.is_empty() && time.elapsed_secs() < notice.until;
    if notice.is_changed() && t.0 != notice.text {
        t.0.clone_from(&notice.text);
    }
    vis.set_if_neq(if up {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
}
