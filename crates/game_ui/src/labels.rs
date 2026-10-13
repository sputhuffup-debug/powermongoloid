//! Settlement name labels: UI text projected from each settlement's position,
//! hidden when off screen or when the camera is far out.

use bevy::prelude::*;
use game_render::{CameraRig, surface_point};
use sim_core::NameTable;
use sim_world::{Settlement, WorldMap};

/// Labels hide beyond this camera distance (world units).
const MAX_LABEL_DISTANCE: f32 = 1800.0;
/// Height above the ground the label is anchored at.
const LABEL_LIFT: f32 = 4.0;

pub struct SettlementLabelsPlugin;

impl Plugin for SettlementLabelsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Labelled>().add_systems(
            Update,
            (
                clear_labels.run_if(game_render::epoch_changed),
                spawn_labels,
                badge_labels,
                place_labels,
                click_labels,
            )
                .chain()
                .in_set(crate::UiSet::Hud),
        );
    }
}

#[derive(Component)]
struct SettlementLabel {
    settlement: Entity,
    anchor: Vec3,
    /// The settlement's name; the badge lines go under it.
    name: String,
}

/// Settlements that already have a label. Kept on the UI side: the UI never
/// writes to simulation entities.
#[derive(Resource, Default)]
struct Labelled(bevy::platform::collections::HashSet<Entity>);

/// A new world (`WorldEpoch`): drop every label.
fn clear_labels(
    mut commands: Commands,
    mut done: ResMut<Labelled>,
    labels: Query<Entity, With<SettlementLabel>>,
) {
    done.0.clear();
    for e in &labels {
        commands.entity(e).despawn();
    }
}

fn spawn_labels(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    names: Option<Res<NameTable>>,
    settlements: Query<(Entity, &Settlement)>,
    mut done: ResMut<Labelled>,
) {
    let (Some(map), Some(names)) = (map, names) else {
        return;
    };
    for (entity, settlement) in &settlements {
        if !done.0.insert(entity) {
            continue;
        }
        commands.spawn((
            SettlementLabel {
                settlement: entity,
                anchor: surface_point(&map, settlement.center) + Vec3::Y * LABEL_LIFT,
                name: names.get(settlement.name).to_string(),
            },
            Text::new(names.get(settlement.name)),
            // Clicking a label selects the settlement (UI-02).
            Button,
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.97, 0.88)),
            TextShadow {
                offset: Vec2::splat(1.5),
                color: Color::srgba(0.0, 0.0, 0.0, 0.85),
            },
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Visibility::Hidden,
        ));
    }
}

fn click_labels(
    labels: Query<(&Interaction, &SettlementLabel), Changed<Interaction>>,
    mut selection: ResMut<crate::selection::Selection>,
) {
    for (interaction, label) in &labels {
        if *interaction == Interaction::Pressed {
            *selection = crate::selection::Selection {
                villager: None,
                settlement: Some(label.settlement),
                army: selection.army,
            };
        }
    }
}

fn place_labels(
    camera: Query<(&Camera, &GlobalTransform), With<CameraRig>>,
    mut labels: Query<(&SettlementLabel, &ComputedNode, &mut Node, &mut Visibility)>,
    settlements: Query<(), With<Settlement>>,
) {
    let Ok((camera, cam_transform)) = camera.single() else {
        return;
    };
    let cam_pos = cam_transform.translation();
    for (label, computed, mut node, mut visibility) in &mut labels {
        let far = cam_pos.distance(label.anchor) > MAX_LABEL_DISTANCE;
        let gone = settlements.get(label.settlement).is_err();
        match camera.world_to_viewport(cam_transform, label.anchor) {
            Ok(screen) if !far && !gone => {
                // Centre the text over the anchor.
                let size = computed.size() * computed.inverse_scale_factor();
                node.left = Val::Px(screen.x - size.x / 2.0);
                node.top = Val::Px(screen.y - size.y);
                *visibility = Visibility::Visible;
            }
            _ => *visibility = Visibility::Hidden,
        }
    }
}

/// GP-34 (M4a): under the name, a line for each edict dial off its default,
/// with a countdown while the first wave runs. Read from the panel's view,
/// which refreshes twice a second.
fn badge_labels(
    view: Res<crate::panels::GovernView>,
    mut labels: Query<(&SettlementLabel, &mut Text)>,
) {
    for (label, mut text) in &mut labels {
        let mut s = label.name.clone();
        if let Some(lines) = view.badges.get(&label.settlement) {
            for l in lines {
                s += "\n";
                s += l;
            }
        }
        if text.0 != s {
            text.0 = s;
        }
    }
}
