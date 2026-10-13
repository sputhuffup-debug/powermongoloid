//! Settlement name labels: UI text projected from each settlement's position,
//! hidden when off screen or when the camera is far out.

use bevy::prelude::*;
use game_render::{CameraRig, surface_point};
use sim_agents::Owner;
use sim_core::{FactionId, NameTable, SpawnId};
use sim_military::Tenure;
use sim_world::{Settlement, WorldMap};

use crate::panels::take::{self, TakeView};

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
                mark_labels,
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

/// A label's first line: the town's name and, in a run, its tenure mark
/// (GP-32, M4a step 5) and whether a submitted town is watched.
fn label_text(name: &str, mark: Option<String>) -> String {
    match mark {
        Some(m) => format!("{name} {m}"),
        None => name.to_string(),
    }
}

/// Writes each label's text (one system, so the two parts never overwrite
/// each other). The first line is the name with the run town's tenure mark:
/// sword, broken sword, bent knee, ribbon or eye as ASCII tags, and
/// "watched" on a submitted town of yours paying tribute (GP-32, M4a step
/// 5, from the [`TakeView`]). Under it, a line for each edict dial off its
/// default, with a countdown while the first wave runs (GP-34, M4a step 4,
/// from the [`GovernView`](crate::panels::GovernView)). Both views refresh
/// twice a second; this only reads.
fn mark_labels(
    take: Res<TakeView>,
    govern: Res<crate::panels::GovernView>,
    fallout: Res<crate::fallout::FalloutView>,
    towns: Query<(&SpawnId, Option<&Tenure>, Option<&Owner>)>,
    mut labels: Query<(&SettlementLabel, &mut Text)>,
) {
    let _span = info_span!("game_ui::labels::mark_labels").entered();
    for (label, mut text) in &mut labels {
        let mark = towns
            .get(label.settlement)
            .ok()
            .and_then(|(id, tenure, owner)| {
                let owner = owner.map_or(FactionId::INDEPENDENT, |o| o.faction);
                let watched = take.tribute(id.0).map(|t| t.watched_now);
                tenure.and_then(|t| take::tenure_mark(owner, t.how, watched))
            });
        // GP-33 (M4a step 6): a town the rival has vowed to retake.
        let vow = towns
            .get(label.settlement)
            .ok()
            .and_then(|(id, ..)| fallout.vow_mark(id.0));
        let mark = match (mark, vow) {
            (Some(m), Some(v)) => Some(format!("{m} {v}")),
            (None, Some(v)) => Some(v.to_string()),
            (m, None) => m,
        };
        let mut new = label_text(&label.name, mark);
        if let Some(lines) = govern.badges.get(&label.settlement) {
            for l in lines {
                new += "\n";
                new += l;
            }
        }
        if text.0 != new {
            text.0 = new;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_the_name_then_the_mark() {
        assert_eq!(label_text("Gorsefield", None), "Gorsefield");
        assert_eq!(
            label_text("Gorsefield", Some("[bent knee, watched]".into())),
            "Gorsefield [bent knee, watched]"
        );
    }
}
