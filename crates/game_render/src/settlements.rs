//! Settlements as small clusters of houses on the terrain. House placement
//! comes from a hash of the settlement's SpawnId, so it is stable but needs
//! no RNG (rendering never touches the simulation's RNG).

use bevy::prelude::*;
use sim_core::SpawnId;
use sim_world::{Settlement, Terrain, WorldMap};

use crate::terrain::surface_point;

pub struct SettlementVisualsPlugin;

impl Plugin for SettlementVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_houses);
    }
}

#[derive(Resource)]
struct HouseAssets {
    walls: Handle<Mesh>,
    roof: Handle<Mesh>,
    wall_material: Handle<StandardMaterial>,
    roof_material: Handle<StandardMaterial>,
}

/// Marks the root of a settlement's houses.
#[derive(Component)]
pub struct SettlementVisual;

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Settlements whose houses exist. Kept on the render side: rendering never
/// writes to simulation entities.
#[derive(Default)]
struct Built(bevy::platform::collections::HashSet<Entity>);

fn spawn_houses(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    assets: Option<Res<HouseAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settlements: Query<(Entity, &SpawnId, &Settlement)>,
    mut built: Local<Built>,
) {
    let Some(map) = map else { return };
    let new: Vec<_> = settlements
        .iter()
        .filter(|(e, ..)| !built.0.contains(e))
        .collect();
    if new.is_empty() {
        return;
    }
    let assets = match assets {
        Some(a) => HouseAssets {
            walls: a.walls.clone(),
            roof: a.roof.clone(),
            wall_material: a.wall_material.clone(),
            roof_material: a.roof_material.clone(),
        },
        None => {
            let a = HouseAssets {
                walls: meshes.add(Cuboid::new(1.4, 1.1, 1.8)),
                roof: meshes.add(Cuboid::new(1.6, 0.5, 2.0)),
                wall_material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.88, 0.82, 0.68),
                    perceptual_roughness: 0.9,
                    ..default()
                }),
                roof_material: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.22, 0.16),
                    perceptual_roughness: 0.9,
                    ..default()
                }),
            };
            commands.insert_resource(HouseAssets {
                walls: a.walls.clone(),
                roof: a.roof.clone(),
                wall_material: a.wall_material.clone(),
                roof_material: a.roof_material.clone(),
            });
            a
        }
    };

    for (entity, id, settlement) in new {
        built.0.insert(entity);
        let mut h = splitmix(id.0);
        let count = 7 + (h % 6) as usize;
        let root = commands
            .spawn((
                SettlementVisual,
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        for _ in 0..count {
            h = splitmix(h);
            let dx = (h % 11) as i32 - 5;
            let dy = ((h >> 8) % 11) as i32 - 5;
            let tile = settlement.center + IVec2::new(dx, dy);
            if !map.in_bounds(tile)
                || matches!(map.terrain(tile), Terrain::Water | Terrain::Road)
                || tile == settlement.center
            {
                continue;
            }
            let base = surface_point(&map, tile);
            let yaw = ((h >> 16) % 4) as f32 * std::f32::consts::FRAC_PI_2
                + ((h >> 20) % 100) as f32 * 0.002;
            let rot = Quat::from_rotation_y(yaw);
            commands.entity(root).with_children(|p| {
                p.spawn((
                    Mesh3d(assets.walls.clone()),
                    MeshMaterial3d(assets.wall_material.clone()),
                    Transform::from_translation(base + Vec3::Y * 0.55).with_rotation(rot),
                ));
                p.spawn((
                    Mesh3d(assets.roof.clone()),
                    MeshMaterial3d(assets.roof_material.clone()),
                    Transform::from_translation(base + Vec3::Y * 1.35).with_rotation(rot),
                ));
            });
        }
    }
}
