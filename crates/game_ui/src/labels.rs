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
        app.add_systems(Update, (spawn_labels, place_labels).chain());
    }
}

#[derive(Component)]
struct SettlementLabel {
    settlement: Entity,
    anchor: Vec3,
}

/// Settlements that already have a label. Kept on the UI side: the UI never
/// writes to simulation entities.
#[derive(Default)]
struct Labelled(bevy::platform::collections::HashSet<Entity>);

fn spawn_labels(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    names: Option<Res<NameTable>>,
    settlements: Query<(Entity, &Settlement)>,
    mut done: Local<Labelled>,
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
            },
            Text::new(names.get(settlement.name)),
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
