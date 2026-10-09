//! Monument tooltips (GP-23): hovering a monument shows its name, kind,
//! bonus and holder. The ray-cast to the ground runs only when the cursor
//! moves (it can take thousands of steps).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_render::CameraRig;
use sim_core::{FactionId, NameTable, Position};
use sim_military::{Army, Holder, Progression};
use sim_world::{Monument, WorldMap};

use crate::cards::stat_text;
use crate::selection::ground_hit;
use crate::style;
use crate::{UiSet, playing};

pub struct TooltipPlugin;

impl Plugin for TooltipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hovered>()
            .add_systems(Startup, spawn_tooltip)
            .add_systems(Update, (hover, show_tooltip).chain().in_set(UiSet::Hud));
    }
}

/// A monument within this many tiles of the cursor's ground point counts
/// as hovered.
const HOVER_TILES: f32 = 9.0;

/// The monument under the cursor (UI-side only).
#[derive(Resource, Default)]
struct Hovered(Option<Entity>);

#[derive(Component)]
struct Tooltip;

#[derive(Component)]
struct TooltipText;

/// The hovered monument and where the cursor is.
#[derive(Default)]
struct Hover {
    cursor: Option<Vec2>,
    /// The camera's position at the last ray-cast.
    eye: Vec3,
    monument: Option<Entity>,
}

fn spawn_tooltip(mut commands: Commands) {
    commands
        .spawn((
            Tooltip,
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(style::PANEL),
            GlobalZIndex(12),
            Visibility::Hidden,
        ))
        .with_children(|t| {
            t.spawn((
                TooltipText,
                Text::new(""),
                style::font(13.0),
                TextColor(style::TEXT),
            ));
        });
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn hover(
    map: Option<Res<WorldMap>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<CameraRig>>,
    monuments: Query<(Entity, &Position), With<Monument>>,
    ui: Query<&Interaction>,
    screen_ok: Option<Res<State<crate::UiScreen>>>,
    mut state: Local<Hover>,
    mut hovered: ResMut<Hovered>,
    mut shown: Query<(&mut Node, &mut Visibility), With<Tooltip>>,
) {
    let _span = info_span!("game_ui::tooltip::hover").entered();
    let Ok((mut node, mut vis)) = shown.single_mut() else {
        return;
    };
    let cursor = window.single().ok().and_then(|w| w.cursor_position());
    let over_ui = ui.iter().any(|i| *i != Interaction::None);
    if cursor.is_none() || over_ui || !playing(screen_ok) {
        state.monument = None;
        state.cursor = cursor;
        hovered.0 = None;
        vis.set_if_neq(Visibility::Hidden);
        return;
    }
    let eye = camera
        .single()
        .map_or(Vec3::ZERO, |(_, gt)| gt.translation());
    if cursor != state.cursor || eye.distance(state.eye) > 0.5 {
        state.cursor = cursor;
        state.eye = eye;
        state.monument = None;
        if let (Some(map), Ok((cam, gt)), Some(c)) = (map, camera.single(), cursor)
            && let Some(hit) = cam
                .viewport_to_world(gt, c)
                .ok()
                .and_then(|ray| ground_hit(&map, ray))
        {
            state.monument = monuments
                .iter()
                .map(|(e, p)| (e, (p.0.as_vec2() + 0.5).distance(hit)))
                .filter(|(_, d)| *d <= HOVER_TILES)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(e, _)| e);
        }
    }
    hovered.0 = state.monument;
    match state.monument.zip(cursor) {
        Some((_, c)) => {
            // Left of the cursor near the right edge, so it stays on screen.
            let width = window.single().map_or(f32::MAX, |w| w.width());
            node.left = Val::Px(if c.x > width - 360.0 {
                c.x - 340.0
            } else {
                c.x + 16.0
            });
            node.top = Val::Px(c.y + 16.0);
            vis.set_if_neq(Visibility::Inherited);
        }
        None => {
            vis.set_if_neq(Visibility::Hidden);
        }
    }
}

fn show_tooltip(
    names: Option<Res<NameTable>>,
    army: Option<Res<Army>>,
    progression: Option<Res<Progression>>,
    hovered: Res<Hovered>,
    monuments: Query<(&Monument, Option<&Holder>)>,
    mut text: Query<&mut Text, With<TooltipText>>,
) {
    let _span = info_span!("game_ui::tooltip::show").entered();
    let (Some(names), Some(army), Some(progression), Ok(mut text)) =
        (names, army, progression, text.single_mut())
    else {
        return;
    };
    let Some(Ok((m, holder))) = hovered.0.map(|e| monuments.get(e)) else {
        return;
    };
    let bonus = progression
        .data
        .monuments
        .bonus(m.kind)
        .map_or_else(String::new, |b| {
            format!(
                "\n{} each (up to {})",
                stat_text(b.stat, b.bonus),
                stat_text(b.stat, b.cap)
            )
        });
    let held = match holder.map(|h| h.faction) {
        None | Some(FactionId::INDEPENDENT) => "nobody".to_string(),
        Some(f) => army.faction_name(f).to_string(),
    };
    text.0 = format!(
        "{} ({}, region {}){bonus}\nHeld by: {held}",
        names.get(m.name),
        m.kind.name(),
        m.region
    );
}
