//! Villager rendering (RND-03, RND-06). Each villager gets a render entity
//! that never touches the sim entity. Its look depends on camera distance:
//! a textured figure close up, a flat low-poly figure further out, then a
//! camera-facing silhouette, then a dot. One shared mesh and one material
//! per (band, occupation) means Bevy draws each group as instanced batches.
//!
//! Positions are interpolated between the last two ticks: a `FixedFirst`
//! system snapshots positions before each tick, and `Update` blends with
//! `Time<Fixed>::overstep_fraction`.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use sim_agents::Villager;
use sim_core::{Motion, Position};
use sim_economy::Occupation;
use sim_world::{WorldMap, world_pos};

use crate::art::ArtTextures;
use crate::bands::{Band, RenderSettings};
use crate::camera::CameraRig;
use crate::terrain::{surface_height_at, tile_to_world};

pub struct AgentVisualsPlugin;

impl Plugin for AgentVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PreviousPositions>()
            .init_resource::<AgentVisuals>()
            .add_systems(FixedFirst, snapshot_positions)
            .add_systems(Update, (sync_visuals, place_visuals).chain());
    }
}

/// Occupation colours, readable at a distance.
pub fn occupation_color(o: Occupation) -> Color {
    match o {
        Occupation::Farmer => Color::srgb(0.85, 0.72, 0.25),
        Occupation::Fisher => Color::srgb(0.25, 0.55, 0.85),
        Occupation::Shepherd => Color::srgb(0.92, 0.92, 0.88),
        Occupation::Woodcutter => Color::srgb(0.35, 0.6, 0.25),
        Occupation::Miner => Color::srgb(0.45, 0.42, 0.45),
        Occupation::Smith => Color::srgb(0.75, 0.3, 0.2),
        Occupation::Baker => Color::srgb(0.95, 0.75, 0.55),
        Occupation::Carter => Color::srgb(0.6, 0.4, 0.25),
        Occupation::Idle => Color::srgb(0.7, 0.55, 0.75),
    }
}

/// Where each villager was before the latest tick.
#[derive(Resource, Default)]
pub struct PreviousPositions(HashMap<Entity, Vec2>);

/// Render-side state: sim entity -> (render entity, band, occupation).
#[derive(Resource, Default)]
pub struct AgentVisuals {
    pub by_sim: HashMap<Entity, (Entity, Band, Occupation)>,
    /// Count per band, for the debug panel.
    pub per_band: [usize; 4],
}

/// Marks a villager's render entity.
#[derive(Component)]
pub struct AgentVisual {
    pub sim: Entity,
}

#[derive(Resource)]
struct AgentAssets {
    figure: Handle<Mesh>,
    billboard: Handle<Mesh>,
    dot: Handle<Mesh>,
    /// [band][occupation]
    materials: Vec<Vec<Handle<StandardMaterial>>>,
}

impl AgentAssets {
    fn mesh(&self, band: Band) -> Handle<Mesh> {
        match band {
            Band::Detail | Band::LowPoly => self.figure.clone(),
            Band::Billboard => self.billboard.clone(),
            Band::Dot => self.dot.clone(),
        }
    }

    fn material(&self, band: Band, occ: Occupation) -> Handle<StandardMaterial> {
        self.materials[band.index()][occ as usize].clone()
    }
}

fn snapshot_positions(
    mut prev: ResMut<PreviousPositions>,
    q: Query<(Entity, &Position, &Motion), With<Villager>>,
) {
    for (e, pos, motion) in &q {
        prev.0.insert(e, world_pos(pos, Some(motion)));
    }
}

/// A body and head, about 1.3 tiles tall, standing on y = 0.
fn figure_mesh() -> Mesh {
    let mut body = Mesh::from(Capsule3d::new(0.22, 0.6)).translated_by(Vec3::Y * 0.52);
    let head = Sphere::new(0.17)
        .mesh()
        .ico(1)
        .expect("valid ico")
        .translated_by(Vec3::Y * 1.1);
    body.merge(&head).expect("same attributes");
    body
}

fn make_assets(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    art: &ArtTextures,
) -> AgentAssets {
    let mut by_band = Vec::new();
    for band in Band::ALL {
        let mut per_occ = Vec::new();
        for occ in Occupation::ALL {
            let color = occupation_color(occ);
            per_occ.push(materials.add(match band {
                Band::Detail => StandardMaterial {
                    base_color: color,
                    base_color_texture: Some(art.cloth.clone()),
                    perceptual_roughness: 0.9,
                    ..default()
                },
                Band::LowPoly => StandardMaterial {
                    base_color: color,
                    perceptual_roughness: 0.9,
                    ..default()
                },
                Band::Billboard => StandardMaterial {
                    base_color: color,
                    base_color_texture: Some(art.silhouette.clone()),
                    alpha_mode: AlphaMode::Mask(0.5),
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                },
                Band::Dot => StandardMaterial {
                    base_color: color,
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                },
            }));
        }
        by_band.push(per_occ);
    }
    AgentAssets {
        figure: meshes.add(figure_mesh()),
        billboard: meshes.add(Rectangle::new(0.7, 1.4)),
        dot: meshes.add(Circle::new(0.5)),
        materials: by_band,
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn sync_visuals(
    mut commands: Commands,
    art: Option<Res<ArtTextures>>,
    assets: Option<Res<AgentAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visuals: ResMut<AgentVisuals>,
    mut prev: ResMut<PreviousPositions>,
    villagers: Query<(Entity, &Occupation), With<Villager>>,
) {
    let Some(art) = art else { return };
    let assets = match assets {
        Some(a) => a.into_inner(),
        None => {
            commands.insert_resource(make_assets(&mut meshes, &mut materials, &art));
            return;
        }
    };
    for (e, occ) in &villagers {
        match visuals.by_sim.get_mut(&e) {
            None => {
                let band = Band::LowPoly;
                let r = commands
                    .spawn((
                        AgentVisual { sim: e },
                        Mesh3d(assets.mesh(band)),
                        MeshMaterial3d(assets.material(band, *occ)),
                        Transform::default(),
                        Visibility::Hidden,
                    ))
                    .id();
                visuals.by_sim.insert(e, (r, band, *occ));
            }
            Some((r, band, old)) if *old != *occ => {
                commands
                    .entity(*r)
                    .insert(MeshMaterial3d(assets.material(*band, *occ)));
                *old = *occ;
            }
            _ => {}
        }
    }
    // Villagers that no longer exist lose their visuals.
    let gone: Vec<Entity> = visuals
        .by_sim
        .keys()
        .copied()
        .filter(|e| villagers.get(*e).is_err())
        .collect();
    for e in gone {
        if let Some((r, ..)) = visuals.by_sim.remove(&e) {
            commands.entity(r).despawn();
        }
        prev.0.remove(&e);
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn place_visuals(
    mut commands: Commands,
    fixed: Res<Time<Fixed>>,
    settings: Res<RenderSettings>,
    map: Option<Res<WorldMap>>,
    assets: Option<Res<AgentAssets>>,
    prev: Res<PreviousPositions>,
    mut visuals: ResMut<AgentVisuals>,
    camera: Query<&GlobalTransform, With<CameraRig>>,
    sims: Query<(&Position, &Motion)>,
    mut renders: Query<(&AgentVisual, &mut Transform, &mut Visibility)>,
) {
    let (Some(map), Some(assets), Ok(cam)) = (map, assets, camera.single()) else {
        return;
    };
    let eye = cam.translation();
    let alpha = fixed.overstep_fraction().clamp(0.0, 1.0);
    let mut per_band = [0usize; 4];
    let visuals = &mut *visuals;
    for (visual, mut transform, mut visibility) in &mut renders {
        let Ok((pos, motion)) = sims.get(visual.sim) else {
            continue;
        };
        let Some((render, band, occ)) = visuals.by_sim.get_mut(&visual.sim) else {
            continue;
        };
        // RND-06: blend from the previous tick's position to this one.
        let now = world_pos(pos, Some(motion));
        let before = prev.0.get(&visual.sim).copied().unwrap_or(now);
        let p = before.lerp(now, alpha);
        let ground = surface_height_at(&map, p.x, p.y);
        let base = tile_to_world(p.x, p.y, ground);
        let distance = eye.distance(base);
        let wanted = settings.select(Some(*band), distance);
        if wanted != *band {
            commands.entity(*render).insert((
                Mesh3d(assets.mesh(wanted)),
                MeshMaterial3d(assets.material(wanted, *occ)),
            ));
            *band = wanted;
        }
        per_band[band.index()] += 1;
        *visibility = Visibility::Inherited;
        *transform = match *band {
            Band::Detail | Band::LowPoly => {
                // Face the way they're walking.
                let v = motion.velocity;
                let rot = if v.length_squared() > 1e-6 {
                    Quat::from_rotation_y((-v.x).atan2(v.y) + std::f32::consts::PI)
                } else {
                    transform.rotation
                };
                Transform::from_translation(base).with_rotation(rot)
            }
            Band::Billboard => {
                let c = base + Vec3::Y * 0.7;
                Transform::from_translation(c).looking_at(c + (c - eye).with_y(0.0), Vec3::Y)
            }
            Band::Dot => {
                // Constant on-screen size: scale with distance.
                let c = base + Vec3::Y * 0.5;
                Transform::from_translation(c)
                    .looking_at(c + (c - eye), Vec3::Y)
                    .with_scale(Vec3::splat((distance * 0.004).max(1.0)))
            }
        };
    }
    visuals.per_band = per_band;
}
