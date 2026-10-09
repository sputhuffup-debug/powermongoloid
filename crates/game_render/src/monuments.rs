//! GP-23: monuments as landmarks. Each `Monument` gets a low-poly model of
//! cuboid parts per kind (like the buildings) and a pennant in its holder's
//! faction colour (army.ron), lowered while nobody holds it. Models are
//! tracked here, keyed by sim entity; nothing is written to the sim.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use sim_core::{FactionId, Position};
use sim_military::{Army, Holder};
use sim_world::{Monument, MonumentKind, WorldMap};

use crate::buildings::Finish;
use crate::terrain::surface_point;
use crate::{RenderSet, epoch_changed, faction_color};

pub struct MonumentVisualsPlugin;

impl Plugin for MonumentVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MonumentVisuals>()
            .add_systems(
                Update,
                clear_monuments
                    .run_if(epoch_changed)
                    .in_set(RenderSet::Epoch),
            )
            .add_systems(
                Update,
                (spawn_monuments, update_pennants)
                    .chain()
                    .in_set(RenderSet::World),
            );
    }
}

/// One monument's model.
struct Model {
    root: Entity,
    pennant: Entity,
    holder: Option<FactionId>,
}

/// Monument models by sim entity, and their shared meshes and materials.
#[derive(Resource, Default)]
pub struct MonumentVisuals {
    by_sim: HashMap<Entity, Model>,
    cube: Option<Handle<Mesh>>,
    finishes: HashMap<Finish, Handle<StandardMaterial>>,
    /// Pennant material per faction.
    pennants: HashMap<FactionId, Handle<StandardMaterial>>,
    /// Models spawned.
    pub total: usize,
}

/// Parts of each kind: (material, centre, size), in tiles. Landmarks are a
/// few times a house's size so they read from afar.
pub fn parts(kind: MonumentKind) -> Vec<(Finish, Vec3, Vec3)> {
    use Finish::*;
    match kind {
        MonumentKind::StandingStones => (0..7)
            .map(|i| {
                let a = i as f32 / 7.0 * std::f32::consts::TAU;
                let h = 2.4 + (i % 3) as f32 * 0.5;
                (
                    Stone,
                    Vec3::new(a.cos() * 3.2, h / 2.0, a.sin() * 3.2),
                    Vec3::new(0.7, h, 0.5),
                )
            })
            .collect(),
        MonumentKind::RuinedFort => vec![
            (Stone, Vec3::new(0.0, 1.0, -3.0), Vec3::new(6.5, 2.0, 0.8)),
            (Stone, Vec3::new(-3.0, 1.4, 0.0), Vec3::new(0.8, 2.8, 6.0)),
            (Stone, Vec3::new(3.0, 0.7, 0.8), Vec3::new(0.8, 1.4, 4.4)),
            (Stone, Vec3::new(-3.0, 2.2, -3.0), Vec3::new(1.6, 4.4, 1.6)),
            (Stone, Vec3::new(1.2, 0.4, 3.0), Vec3::new(3.0, 0.8, 0.8)),
        ],
        MonumentKind::HolyWell => vec![
            (Stone, Vec3::new(0.0, 0.5, 0.0), Vec3::new(2.4, 1.0, 2.4)),
            (Wood, Vec3::new(-1.0, 1.6, 0.0), Vec3::new(0.2, 2.2, 0.2)),
            (Wood, Vec3::new(1.0, 1.6, 0.0), Vec3::new(0.2, 2.2, 0.2)),
            (Roof, Vec3::new(0.0, 2.8, 0.0), Vec3::new(2.8, 0.3, 1.8)),
        ],
        MonumentKind::RoadMarker => vec![
            (Stone, Vec3::new(0.0, 0.3, 0.0), Vec3::new(1.6, 0.6, 1.6)),
            (Stone, Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.8, 3.2, 0.8)),
        ],
        MonumentKind::AncientForge => vec![
            (Stone, Vec3::new(0.0, 1.0, 0.0), Vec3::new(4.0, 2.0, 3.0)),
            (Stone, Vec3::new(1.2, 3.0, 0.8), Vec3::new(0.9, 2.2, 0.9)),
            (Wood, Vec3::new(-2.6, 0.5, 0.0), Vec3::new(1.0, 1.0, 1.0)),
            (Roof, Vec3::new(0.0, 2.2, 0.0), Vec3::new(4.4, 0.4, 3.4)),
        ],
        MonumentKind::MarketCross => vec![
            (Stone, Vec3::new(0.0, 0.25, 0.0), Vec3::new(3.0, 0.5, 3.0)),
            (Stone, Vec3::new(0.0, 0.75, 0.0), Vec3::new(2.0, 0.5, 2.0)),
            (Stone, Vec3::new(0.0, 2.6, 0.0), Vec3::new(0.5, 3.2, 0.5)),
            (Stone, Vec3::new(0.0, 3.4, 0.0), Vec3::new(1.6, 0.4, 0.4)),
        ],
        MonumentKind::Wayshrine => vec![
            (Stone, Vec3::new(0.0, 0.3, 0.0), Vec3::new(1.4, 0.6, 1.4)),
            (Plaster, Vec3::new(0.0, 1.3, 0.0), Vec3::new(1.0, 1.6, 1.0)),
            (Roof, Vec3::new(0.0, 2.3, 0.0), Vec3::new(1.5, 0.4, 1.5)),
        ],
        MonumentKind::Watchtower => vec![
            (Wood, Vec3::new(-0.9, 3.0, -0.9), Vec3::new(0.3, 6.0, 0.3)),
            (Wood, Vec3::new(0.9, 3.0, -0.9), Vec3::new(0.3, 6.0, 0.3)),
            (Wood, Vec3::new(-0.9, 3.0, 0.9), Vec3::new(0.3, 6.0, 0.3)),
            (Wood, Vec3::new(0.9, 3.0, 0.9), Vec3::new(0.3, 6.0, 0.3)),
            (Wood, Vec3::new(0.0, 6.0, 0.0), Vec3::new(2.6, 0.3, 2.6)),
            (Roof, Vec3::new(0.0, 7.2, 0.0), Vec3::new(2.2, 0.8, 2.2)),
        ],
    }
}

/// Top of the pennant pole, in tiles above the ground.
const POLE: f32 = 7.5;

fn clear_monuments(mut commands: Commands, mut visuals: ResMut<MonumentVisuals>) {
    let _span = info_span!("game_render::monuments::clear").entered();
    for (_, m) in visuals.by_sim.drain() {
        commands.entity(m.root).despawn();
    }
    visuals.total = 0;
}

fn spawn_monuments(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    mut visuals: ResMut<MonumentVisuals>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    monuments: Query<(Entity, &Monument, &Position)>,
) {
    let _span = info_span!("game_render::monuments::spawn").entered();
    let Some(map) = map else { return };
    if visuals.cube.is_none() {
        visuals.cube = Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0)));
        for f in Finish::ALL {
            let m = materials.add(StandardMaterial {
                base_color: f.color(),
                perceptual_roughness: 0.9,
                ..default()
            });
            visuals.finishes.insert(f, m);
        }
    }
    let cube = visuals.cube.clone().expect("set above");
    for (e, monument, pos) in &monuments {
        if visuals.by_sim.contains_key(&e) {
            continue;
        }
        let base = surface_point(&map, pos.0);
        let yaw = ((pos.0.x * 13 + pos.0.y * 7).rem_euclid(8)) as f32 * std::f32::consts::FRAC_PI_4;
        let root = commands
            .spawn((
                Transform::from_translation(base).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
            ))
            .id();
        let mut kids = Vec::new();
        for (f, centre, size) in parts(monument.kind) {
            kids.push(
                commands
                    .spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(visuals.finishes[&f].clone()),
                        Transform::from_translation(centre).with_scale(size),
                    ))
                    .id(),
            );
        }
        // The pole stands beside the model; the pennant hangs from it.
        let pole_at = Vec3::new(3.6, POLE / 2.0, 3.6);
        kids.push(
            commands
                .spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(visuals.finishes[&Finish::Wood].clone()),
                    Transform::from_translation(pole_at).with_scale(Vec3::new(0.18, POLE, 0.18)),
                ))
                .id(),
        );
        let pennant = commands
            .spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(visuals.finishes[&Finish::Plaster].clone()),
                Transform::from_translation(Vec3::new(4.5, POLE - 0.8, 3.6))
                    .with_scale(Vec3::new(1.8, 1.2, 0.08)),
                Visibility::Hidden,
            ))
            .id();
        kids.push(pennant);
        commands.entity(root).add_children(&kids);
        visuals.by_sim.insert(
            e,
            Model {
                root,
                pennant,
                holder: None,
            },
        );
    }
    visuals.total = visuals.by_sim.len();
}

/// Raises the pennant in the holder's colour, lowers it for nobody.
fn update_pennants(
    mut commands: Commands,
    army: Option<Res<Army>>,
    mut visuals: ResMut<MonumentVisuals>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    holders: Query<(Entity, &Holder)>,
) {
    let _span = info_span!("game_render::monuments::pennants").entered();
    let Some(army) = army else { return };
    let visuals = &mut *visuals;
    for (e, holder) in &holders {
        let Some(model) = visuals.by_sim.get_mut(&e) else {
            continue;
        };
        if model.holder == Some(holder.faction) {
            continue;
        }
        model.holder = Some(holder.faction);
        if holder.faction == FactionId::INDEPENDENT {
            commands.entity(model.pennant).insert(Visibility::Hidden);
            continue;
        }
        let material = visuals
            .pennants
            .entry(holder.faction)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: faction_color(&army, holder.faction.0),
                    unlit: true,
                    ..default()
                })
            })
            .clone();
        commands
            .entity(model.pennant)
            .insert((MeshMaterial3d(material), Visibility::Inherited));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_model_above_ground() {
        for kind in MonumentKind::ALL {
            let p = parts(kind);
            assert!(!p.is_empty(), "{kind:?}");
            for (_, centre, size) in p {
                assert!(centre.y - size.y / 2.0 >= -0.01, "{kind:?} sinks");
                assert!(size.min_element() > 0.0);
            }
        }
    }
}
