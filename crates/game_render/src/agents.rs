//! People rendering (RND-03, RND-06): villagers, militia and soldiers. Each
//! person gets a render entity that never touches the sim entity. Its look
//! depends on camera distance: a textured figure close up, a flat low-poly
//! figure further out, then a camera-facing silhouette, then a dot. One
//! shared mesh and one material per (band, look) means Bevy draws each
//! group as instanced batches. Villagers are coloured by occupation;
//! soldiers by faction (captains brighter, routed men greyed); militia dark
//! red. Captains carry a banner, readable from far away.
//!
//! Interpolation: a `FixedFirst` system snapshots positions before each
//! tick, and `Update` blends with `Time<Fixed>::overstep_fraction`.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use sim_agents::{ActionKind, Activity, CombatStats, Stance};
use sim_core::{Allegiance, Motion, Position};
use sim_economy::Occupation;
use sim_military::{Army, Captain};
use sim_world::{Mobile, WorldMap, world_pos};

use crate::art::ArtTextures;
use crate::bands::{Band, RenderSettings};
use crate::camera::CameraRig;
use crate::terrain::{surface_height_at, tile_to_world};

pub struct AgentVisualsPlugin;

impl Plugin for AgentVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PreviousPositions>()
            .init_resource::<AgentVisuals>()
            .init_resource::<Banners>()
            .add_systems(FixedFirst, snapshot_positions)
            .add_systems(Update, (sync_visuals, place_visuals, place_banners).chain());
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

/// How a person is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Villager(Occupation),
    Militia,
    Soldier(u16),
    Captain(u16),
    Routed(u16),
}

impl Look {
    fn index(self) -> usize {
        let n = Occupation::ALL.len();
        match self {
            Look::Villager(o) => o as usize,
            Look::Militia => n,
            Look::Soldier(f) => n + 1 + 3 * f as usize,
            Look::Captain(f) => n + 2 + 3 * f as usize,
            Look::Routed(f) => n + 3 + 3 * f as usize,
        }
    }

    fn count(factions: usize) -> usize {
        Occupation::ALL.len() + 1 + 3 * factions
    }

    /// From the person's sim state.
    pub fn of(
        occupation: Option<&Occupation>,
        captain: bool,
        allegiance: &Allegiance,
        activity: Option<&Activity>,
        stats: &CombatStats,
    ) -> Look {
        let f = allegiance.0.0;
        match stats.stance {
            Stance::Militia { .. } => Look::Militia,
            Stance::Soldier if activity.is_some_and(|a| a.kind == ActionKind::Rout) => {
                Look::Routed(f)
            }
            Stance::Soldier if captain => Look::Captain(f),
            Stance::Soldier => Look::Soldier(f),
            Stance::Civilian => Look::Villager(occupation.copied().unwrap_or(Occupation::Idle)),
        }
    }
}

/// A faction's colour from army.ron.
pub fn faction_color(army: &Army, f: u16) -> Color {
    let (r, g, b) = army
        .data
        .factions
        .get(f as usize)
        .map_or((0.6, 0.6, 0.6), |d| d.color);
    Color::srgb(r, g, b)
}

fn look_color(look: Look, army: &Army) -> Color {
    match look {
        Look::Villager(o) => occupation_color(o),
        Look::Militia => Color::srgb(0.55, 0.18, 0.12),
        Look::Soldier(f) => faction_color(army, f),
        Look::Captain(f) => faction_color(army, f).mix(&Color::WHITE, 0.35),
        Look::Routed(f) => faction_color(army, f).mix(&Color::srgb(0.55, 0.55, 0.55), 0.7),
    }
}

/// Where each person was before the latest tick.
#[derive(Resource, Default)]
pub struct PreviousPositions(HashMap<Entity, Vec2>);

/// Render-side state: sim entity -> (render entity, band, look).
#[derive(Resource, Default)]
pub struct AgentVisuals {
    pub by_sim: HashMap<Entity, (Entity, Band, Look)>,
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
    /// [band][look]
    materials: Vec<Vec<Handle<StandardMaterial>>>,
    banner: Handle<Mesh>,
    /// Per faction.
    banner_materials: Vec<Handle<StandardMaterial>>,
}

impl AgentAssets {
    fn mesh(&self, band: Band) -> Handle<Mesh> {
        match band {
            Band::Detail | Band::LowPoly => self.figure.clone(),
            Band::Billboard => self.billboard.clone(),
            Band::Dot => self.dot.clone(),
        }
    }

    fn material(&self, band: Band, look: Look) -> Handle<StandardMaterial> {
        let per_band = &self.materials[band.index()];
        per_band
            .get(look.index())
            .unwrap_or(&per_band[Occupation::Idle as usize])
            .clone()
    }
}

fn snapshot_positions(
    mut prev: ResMut<PreviousPositions>,
    q: Query<(Entity, &Position, &Motion), With<Mobile>>,
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

/// A pole with a flag, about 3 tiles tall.
fn banner_mesh() -> Mesh {
    let mut pole = Mesh::from(Cuboid::new(0.08, 3.0, 0.08)).translated_by(Vec3::Y * 1.5);
    let flag = Mesh::from(Cuboid::new(1.0, 0.6, 0.05)).translated_by(Vec3::new(0.5, 2.65, 0.0));
    pole.merge(&flag).expect("same attributes");
    pole
}

fn make_assets(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    art: &ArtTextures,
    army: &Army,
) -> AgentAssets {
    let factions = army.data.factions.len();
    let mut looks: Vec<Look> = Occupation::ALL.iter().map(|o| Look::Villager(*o)).collect();
    looks.push(Look::Militia);
    for f in 0..factions as u16 {
        looks.extend([Look::Soldier(f), Look::Captain(f), Look::Routed(f)]);
    }
    debug_assert_eq!(looks.len(), Look::count(factions));
    let mut by_band = Vec::new();
    for band in Band::ALL {
        let mut per_occ = Vec::new();
        for look in &looks {
            let color = look_color(*look, army);
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
        banner: meshes.add(banner_mesh()),
        banner_materials: (0..factions as u16)
            .map(|f| {
                materials.add(StandardMaterial {
                    base_color: faction_color(army, f),
                    unlit: true,
                    ..default()
                })
            })
            .collect(),
    }
}

/// Captain -> banner render entity.
#[derive(Resource, Default)]
pub struct Banners(HashMap<Entity, Entity>);

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn sync_visuals(
    mut commands: Commands,
    art: Option<Res<ArtTextures>>,
    assets: Option<Res<AgentAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut visuals: ResMut<AgentVisuals>,
    mut prev: ResMut<PreviousPositions>,
    army: Option<Res<Army>>,
    people: Query<PersonQuery, With<Mobile>>,
) {
    let (Some(art), Some(army)) = (art, army) else {
        return;
    };
    let assets = match assets {
        Some(a) => a.into_inner(),
        None => {
            commands.insert_resource(make_assets(&mut meshes, &mut materials, &art, &army));
            return;
        }
    };
    for (e, occ, captain, allegiance, activity, stats) in &people {
        let look = Look::of(occ, captain, allegiance, activity, stats);
        match visuals.by_sim.get_mut(&e) {
            None => {
                let band = Band::LowPoly;
                let r = commands
                    .spawn((
                        AgentVisual { sim: e },
                        Mesh3d(assets.mesh(band)),
                        MeshMaterial3d(assets.material(band, look)),
                        Transform::default(),
                        Visibility::Hidden,
                    ))
                    .id();
                visuals.by_sim.insert(e, (r, band, look));
            }
            Some((r, band, old)) if *old != look => {
                commands
                    .entity(*r)
                    .insert(MeshMaterial3d(assets.material(*band, look)));
                *old = look;
            }
            _ => {}
        }
    }
    // People who no longer exist lose their visuals.
    let gone: Vec<Entity> = visuals
        .by_sim
        .keys()
        .copied()
        .filter(|e| people.get(*e).is_err())
        .collect();
    for e in gone {
        if let Some((r, ..)) = visuals.by_sim.remove(&e) {
            commands.entity(r).despawn();
        }
        prev.0.remove(&e);
    }
}

type PersonQuery = (
    Entity,
    Option<&'static Occupation>,
    Has<Captain>,
    &'static Allegiance,
    Option<&'static Activity>,
    &'static CombatStats,
);

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn place_visuals(
    mut commands: Commands,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    settings: Res<RenderSettings>,
    map: Option<Res<WorldMap>>,
    assets: Option<Res<AgentAssets>>,
    prev: Res<PreviousPositions>,
    mut visuals: ResMut<AgentVisuals>,
    camera: Query<&GlobalTransform, With<CameraRig>>,
    sims: Query<(&Position, &Motion, Option<&Activity>)>,
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
        let Ok((pos, motion, activity)) = sims.get(visual.sim) else {
            continue;
        };
        let Some((render, band, look)) = visuals.by_sim.get_mut(&visual.sim) else {
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
                MeshMaterial3d(assets.material(wanted, *look)),
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
                // Swing feedback: fighters lunge back and forth.
                let rot = if activity.is_some_and(|a| a.kind == ActionKind::Fight) {
                    let phase = (visual.sim.index_u32() % 17) as f32;
                    rot * Quat::from_rotation_x((time.elapsed_secs() * 9.0 + phase).sin() * 0.35)
                } else {
                    rot
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

/// Keeps a banner over every captain, scaled up with distance so armies
/// can be found from far away.
#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn place_banners(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    assets: Option<Res<AgentAssets>>,
    fixed: Res<Time<Fixed>>,
    prev: Res<PreviousPositions>,
    mut banners: ResMut<Banners>,
    camera: Query<&GlobalTransform, With<CameraRig>>,
    captains: Query<(Entity, &Captain, &Position, &Motion)>,
    mut transforms: Query<&mut Transform>,
) {
    let (Some(map), Some(assets), Ok(cam)) = (map, assets, camera.single()) else {
        return;
    };
    let eye = cam.translation();
    let alpha = fixed.overstep_fraction().clamp(0.0, 1.0);
    for (e, captain, pos, motion) in &captains {
        let banner = *banners.0.entry(e).or_insert_with(|| {
            let m = assets
                .banner_materials
                .get(captain.faction.0 as usize)
                .or(assets.banner_materials.first())
                .cloned()
                .unwrap_or_default();
            commands
                .spawn((
                    Mesh3d(assets.banner.clone()),
                    MeshMaterial3d(m),
                    Transform::default(),
                ))
                .id()
        });
        let now = world_pos(pos, Some(motion));
        let p = prev.0.get(&e).copied().unwrap_or(now).lerp(now, alpha);
        let base = tile_to_world(p.x, p.y, surface_height_at(&map, p.x, p.y));
        let scale = (eye.distance(base) * 0.008).max(1.0);
        if let Ok(mut t) = transforms.get_mut(banner) {
            *t = Transform::from_translation(base)
                .with_rotation(Quat::from_rotation_y(
                    cam.rotation().to_euler(EulerRot::YXZ).0,
                ))
                .with_scale(Vec3::splat(scale));
        }
    }
    banners.0.retain(|captain, banner| {
        let alive = captains.get(*captain).is_ok();
        if !alive {
            commands.entity(*banner).despawn();
        }
        alive
    });
}
