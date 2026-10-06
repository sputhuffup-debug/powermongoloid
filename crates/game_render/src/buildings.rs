//! Buildings drawn from the sim's `Building` entities: low-poly models per
//! kind, switching between stylized-textured and flat materials by camera
//! distance (ADR 0009). Rendering never writes to sim entities; which
//! buildings have models is tracked here.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use sim_economy::{Building, BuildingKind, WorkplaceKind};
use sim_world::WorldMap;

use crate::art::ArtTextures;
use crate::bands::{Band, RenderSettings};
use crate::camera::CameraRig;
use crate::terrain::surface_point;

pub struct BuildingVisualsPlugin;

impl Plugin for BuildingVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuildingVisuals>()
            .add_systems(Update, (spawn_buildings, swap_building_materials).chain());
    }
}

/// What a part is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Finish {
    Plaster,
    Roof,
    Wood,
    Stone,
}

impl Finish {
    const ALL: [Finish; 4] = [Finish::Plaster, Finish::Roof, Finish::Wood, Finish::Stone];

    fn color(self) -> Color {
        match self {
            Finish::Plaster => Color::srgb(0.88, 0.82, 0.68),
            Finish::Roof => Color::srgb(0.58, 0.25, 0.18),
            Finish::Wood => Color::srgb(0.48, 0.33, 0.2),
            Finish::Stone => Color::srgb(0.42, 0.42, 0.44),
        }
    }
}

#[derive(Component)]
struct Part(Finish);

#[derive(Resource, Default)]
pub struct BuildingVisuals {
    /// sim building -> (model root, band).
    by_sim: HashMap<Entity, (Entity, Band)>,
    cube: Option<Handle<Mesh>>,
    /// (flat, textured) per material.
    materials: HashMap<Finish, (Handle<StandardMaterial>, Handle<StandardMaterial>)>,
    pub textured: usize,
    pub total: usize,
}

/// Parts of each kind of building: (material, centre, size), in tiles.
fn parts(kind: BuildingKind) -> Vec<(Finish, Vec3, Vec3)> {
    use Finish::*;
    let house = |w: f32, d: f32, h: f32| {
        vec![
            (Plaster, Vec3::new(0.0, h / 2.0, 0.0), Vec3::new(w, h, d)),
            (
                Roof,
                Vec3::new(0.0, h + 0.25, 0.0),
                Vec3::new(w + 0.2, 0.5, d + 0.2),
            ),
        ]
    };
    match kind {
        // Houses stand every other tile; lanes run between them.
        BuildingKind::House => house(1.3, 1.5, 1.2),
        BuildingKind::Workplace(k) => match k {
            WorkplaceKind::Storehouse => {
                let mut p = house(3.0, 3.6, 2.0);
                p.push((Wood, Vec3::new(0.0, 0.6, 1.9), Vec3::new(1.2, 1.2, 0.2)));
                p
            }
            WorkplaceKind::Bakery => {
                let mut p = house(1.8, 2.0, 1.4);
                p.push((Stone, Vec3::new(0.6, 2.0, 0.5), Vec3::new(0.35, 1.0, 0.35)));
                p
            }
            WorkplaceKind::Smithy => {
                let mut p = house(2.0, 2.2, 1.3);
                p.push((
                    Stone,
                    Vec3::new(-0.7, 2.1, -0.6),
                    Vec3::new(0.45, 1.3, 0.45),
                ));
                p
            }
            WorkplaceKind::Farm => vec![
                (Wood, Vec3::new(0.0, 0.5, 0.0), Vec3::new(1.2, 1.0, 1.2)),
                (Roof, Vec3::new(0.0, 1.1, 0.0), Vec3::new(1.4, 0.3, 1.4)),
            ],
            WorkplaceKind::FishingSpot => vec![
                (Wood, Vec3::new(0.0, 0.45, 0.0), Vec3::new(1.0, 0.9, 1.0)),
                (Wood, Vec3::new(0.0, 0.1, 1.0), Vec3::new(0.5, 0.15, 1.6)),
            ],
            WorkplaceKind::Pasture => vec![
                (Wood, Vec3::new(-1.0, 0.35, 0.0), Vec3::new(0.12, 0.7, 2.0)),
                (Wood, Vec3::new(1.0, 0.35, 0.0), Vec3::new(0.12, 0.7, 2.0)),
                (Wood, Vec3::new(0.0, 0.5, 1.0), Vec3::new(2.0, 0.12, 0.12)),
            ],
            WorkplaceKind::WoodcutterCamp => vec![
                (Wood, Vec3::new(0.0, 0.3, 0.0), Vec3::new(1.4, 0.6, 0.8)),
                (Wood, Vec3::new(0.2, 0.75, 0.0), Vec3::new(1.0, 0.3, 0.6)),
            ],
            WorkplaceKind::Mine => vec![
                (Stone, Vec3::new(0.0, 0.7, 0.0), Vec3::new(1.6, 1.4, 1.2)),
                (Wood, Vec3::new(0.0, 0.5, 0.65), Vec3::new(0.8, 1.0, 0.1)),
            ],
        },
    }
}

fn spawn_buildings(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    art: Option<Res<ArtTextures>>,
    mut visuals: ResMut<BuildingVisuals>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    buildings: Query<(Entity, &Building)>,
) {
    let (Some(map), Some(art)) = (map, art) else {
        return;
    };
    if visuals.cube.is_none() {
        visuals.cube = Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0)));
        for m in Finish::ALL {
            let texture = match m {
                Finish::Plaster => art.plaster.clone(),
                Finish::Roof => art.shingles.clone(),
                Finish::Wood => art.planks.clone(),
                Finish::Stone => art.stone.clone(),
            };
            let flat = materials.add(StandardMaterial {
                base_color: m.color(),
                perceptual_roughness: 0.9,
                ..default()
            });
            let textured = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(texture),
                perceptual_roughness: 0.9,
                ..default()
            });
            visuals.materials.insert(m, (flat, textured));
        }
    }
    let cube = visuals.cube.clone().expect("set above");
    for (e, b) in &buildings {
        if visuals.by_sim.contains_key(&e) {
            continue;
        }
        let base = surface_point(&map, b.tile);
        // A stable orientation per tile, so villages don't look gridded.
        let yaw =
            ((b.tile.x * 31 + b.tile.y * 17).rem_euclid(4)) as f32 * std::f32::consts::FRAC_PI_2;
        let root = commands
            .spawn((
                Transform::from_translation(base).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
            ))
            .id();
        for (m, centre, size) in parts(b.kind) {
            let flat = visuals.materials[&m].0.clone();
            commands.entity(root).with_children(|p| {
                p.spawn((
                    Part(m),
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(flat),
                    Transform::from_translation(centre).with_scale(size),
                ));
            });
        }
        visuals.by_sim.insert(e, (root, Band::LowPoly));
    }
    visuals.total = visuals.by_sim.len();
}

fn swap_building_materials(
    mut commands: Commands,
    settings: Res<RenderSettings>,
    mut visuals: ResMut<BuildingVisuals>,
    camera: Query<&GlobalTransform, With<CameraRig>>,
    roots: Query<(&GlobalTransform, &Children)>,
    parts: Query<&Part>,
) {
    let Ok(cam) = camera.single() else { return };
    let eye = cam.translation();
    let visuals = &mut *visuals;
    let mut textured = 0;
    for (root, band) in visuals.by_sim.values_mut() {
        let Ok((gt, children)) = roots.get(*root) else {
            continue;
        };
        let wanted = match settings.select(Some(*band), eye.distance(gt.translation())) {
            Band::Detail => Band::Detail,
            _ => Band::LowPoly,
        };
        if wanted == Band::Detail {
            textured += 1;
        }
        if wanted == *band {
            continue;
        }
        for child in children.iter() {
            if let Ok(Part(m)) = parts.get(child) {
                let (flat, tex) = &visuals.materials[m];
                let mat = if wanted == Band::Detail {
                    tex.clone()
                } else {
                    flat.clone()
                };
                commands.entity(child).insert(MeshMaterial3d(mat));
            }
        }
        *band = wanted;
    }
    visuals.textured = textured;
}
