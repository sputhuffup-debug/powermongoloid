//! RND-01: terrain as one mesh per 64 x 64 chunk, built from the height map
//! with vertex colours by terrain type. A chunk is (re)built only when its
//! revision in `WorldMap` changes, spread over frames, nearest first.
//! The M4 overlay ([`Overlay`]) tints vertex colours; a chunk also rebuilds
//! when the overlay discs touching it change.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use sim_world::{Terrain, WorldMap};

use crate::art::ArtTextures;
use crate::bands::{Band, RenderSettings};
use crate::camera::CameraRig;
use crate::overlay::Overlay;
use crate::{RenderSet, epoch_changed};

/// World units per unit of normalised height above sea level.
pub const HEIGHT_SCALE: f32 = 160.0;
/// Chunks built per frame at most, so a full rebuild never stalls a frame.
const CHUNKS_PER_FRAME: usize = 48;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainChunks>()
            .add_systems(Startup, setup_ocean)
            .add_systems(
                Update,
                (clear_chunks, setup_ocean)
                    .chain()
                    .run_if(epoch_changed)
                    .in_set(RenderSet::Epoch),
            )
            .add_systems(
                Update,
                (sync_chunks, swap_chunk_materials)
                    .chain()
                    .in_set(RenderSet::World),
            );
    }
}

/// Tile `(x, y)` maps to world `(x, elevation, -y)`, so map north is
/// screen-up at the default camera yaw.
pub fn tile_to_world(x: f32, y: f32, elevation: f32) -> Vec3 {
    Vec3::new(x, elevation, -y)
}

/// Elevation of a normalised height in world units (sea level is 0).
pub fn elevation(map: &WorldMap, height: f32) -> f32 {
    (height - map.sea_level()).max(0.0) * HEIGHT_SCALE
}

/// The world-space centre of a tile's surface.
pub fn surface_point(map: &WorldMap, tile: IVec2) -> Vec3 {
    let tile = tile.clamp(IVec2::ZERO, IVec2::splat(map.size() as i32 - 1));
    tile_to_world(
        tile.x as f32 + 0.5,
        tile.y as f32 + 0.5,
        elevation(map, map.height(tile)),
    )
}

/// The rendered surface height at a fractional tile position.
pub fn surface_height_at(map: &WorldMap, x: f32, y: f32) -> f32 {
    let t = IVec2::new(x.floor() as i32, y.floor() as i32)
        .clamp(IVec2::ZERO, IVec2::splat(map.size() as i32 - 1));
    elevation(map, map.height(t))
}

pub fn terrain_color(t: Terrain) -> Color {
    match t {
        Terrain::Water => Color::srgb(0.16, 0.36, 0.62),
        Terrain::Beach => Color::srgb(0.86, 0.80, 0.58),
        Terrain::Grass => Color::srgb(0.42, 0.62, 0.30),
        Terrain::Farmland => Color::srgb(0.78, 0.70, 0.30),
        Terrain::Forest => Color::srgb(0.15, 0.38, 0.17),
        Terrain::Hills => Color::srgb(0.55, 0.50, 0.36),
        Terrain::Mountain => Color::srgb(0.60, 0.60, 0.62),
        Terrain::Road => Color::srgb(0.50, 0.33, 0.20),
    }
}

/// Build state of every chunk, plus counters for the debug panel.
#[derive(Resource, Default)]
pub struct TerrainChunks {
    /// Per chunk: the entity, the map revision and the overlay signature
    /// it was built from.
    built: Vec<Option<(Entity, u32, u64)>>,
    /// Per chunk: the overlay signature now, and the overlay generation it
    /// was computed for.
    signatures: Vec<u64>,
    signature_generation: Option<u32>,
    /// Flat material (low-poly band) and textured material (detail band).
    flat: Option<Handle<StandardMaterial>>,
    textured: Option<Handle<StandardMaterial>>,
    /// Which material each chunk currently uses.
    bands: Vec<Option<Band>>,
    /// Chunks currently drawn with stylized textures.
    pub textured_count: usize,
    pub total: usize,
    pub built_count: usize,
    /// Chunk meshes built since startup, including rebuilds.
    pub builds: u64,
}

/// The sea plane under the island.
#[derive(Component)]
struct Ocean;

/// A new world (`WorldEpoch`): forget every chunk and the ocean.
fn clear_chunks(
    mut commands: Commands,
    mut chunks: ResMut<TerrainChunks>,
    ocean: Query<Entity, With<Ocean>>,
) {
    let _span = info_span!("game_render::terrain::clear").entered();
    for (e, ..) in chunks.built.drain(..).flatten() {
        commands.entity(e).despawn();
    }
    chunks.bands.clear();
    chunks.signatures.clear();
    chunks.signature_generation = None;
    chunks.built_count = 0;
    chunks.textured_count = 0;
    for e in &ocean {
        commands.entity(e).despawn();
    }
}

fn setup_ocean(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    map: Option<Res<WorldMap>>,
) {
    let Some(map) = map else { return };
    let size = map.size() as f32;
    // Extends past the map edge so the island sits in open sea.
    commands.spawn((
        Ocean,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(size * 4.0, size * 4.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: terrain_color(Terrain::Water).darker(0.05),
            perceptual_roughness: 0.6,
            ..default()
        })),
        Transform::from_translation(tile_to_world(size / 2.0, size / 2.0, -0.3)),
    ));
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn sync_chunks(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    settings: Res<RenderSettings>,
    art: Option<Res<ArtTextures>>,
    overlay: Res<Overlay>,
    mut chunks: ResMut<TerrainChunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rig: Query<&CameraRig>,
) {
    let _span = info_span!("game_render::terrain::sync_chunks").entered();
    let (Some(map), Some(art)) = (map, art) else {
        return;
    };
    let total = map.chunks_per_side().pow(2) as usize;
    if chunks.built.len() != total {
        chunks.built = vec![None; total];
        chunks.bands = vec![None; total];
        chunks.signatures = vec![0; total];
        chunks.signature_generation = None;
        chunks.total = total;
    }
    if chunks.signature_generation != Some(overlay.generation) {
        let side = map.chunk_size() as f32;
        for c in 0..total {
            chunks.signatures[c] =
                overlay.chunk_signature(map.chunk_origin(c as u32).as_vec2(), side);
        }
        chunks.signature_generation = Some(overlay.generation);
    }
    if chunks.flat.is_none() {
        // Vertex colours carry the terrain colour. The flat base colour
        // matches the detail texture's average brightness, so switching
        // bands changes texture, not tone.
        chunks.flat = Some(materials.add(StandardMaterial {
            base_color: Color::srgb(0.86, 0.86, 0.86),
            perceptual_roughness: 0.95,
            reflectance: 0.08,
            ..default()
        }));
        chunks.textured = Some(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(art.terrain_detail.clone()),
            perceptual_roughness: 0.95,
            reflectance: 0.08,
            ..default()
        }));
    }
    let material = chunks.flat.clone().expect("set above");

    let mut stale: Vec<u32> = (0..total as u32)
        .filter(|&c| {
            chunks.built[c as usize].is_none_or(|(_, rev, sig)| {
                rev != map.chunk_revision(c) || sig != chunks.signatures[c as usize]
            })
        })
        .collect();
    if stale.is_empty() {
        return;
    }
    // Nearest to the camera focus first.
    let focus = rig
        .iter()
        .next()
        .map_or(Vec2::splat(map.size() as f32 / 2.0), |r| r.focus_tile());
    let half = map.chunk_size() as f32 / 2.0;
    stale.sort_by(|&a, &b| {
        let da = (map.chunk_origin(a).as_vec2() + half).distance_squared(focus);
        let db = (map.chunk_origin(b).as_vec2() + half).distance_squared(focus);
        da.total_cmp(&db).then(a.cmp(&b))
    });

    for c in stale.into_iter().take(CHUNKS_PER_FRAME) {
        let mesh = meshes.add(build_chunk_mesh_tinted(
            &map,
            c,
            settings.terrain_texture_tiles,
            Some(&overlay),
        ));
        let origin = map.chunk_origin(c);
        let transform =
            Transform::from_translation(tile_to_world(origin.x as f32, origin.y as f32, 0.0));
        let revision = map.chunk_revision(c);
        let signature = chunks.signatures[c as usize];
        match chunks.built[c as usize] {
            Some((entity, ..)) => {
                commands.entity(entity).insert(Mesh3d(mesh));
            }
            None => {
                let entity = commands
                    .spawn((Mesh3d(mesh), MeshMaterial3d(material.clone()), transform))
                    .id();
                chunks.built_count += 1;
                chunks.built[c as usize] = Some((entity, revision, signature));
                chunks.bands[c as usize] = Some(Band::LowPoly);
            }
        }
        if let Some(slot) = chunks.built[c as usize].as_mut() {
            slot.1 = revision;
            slot.2 = signature;
        }
        chunks.builds += 1;
    }
}

/// Swaps each chunk between the textured (detail) and flat material by its
/// distance from the camera (ADR 0009). A material swap, not a rebuild.
fn swap_chunk_materials(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    settings: Res<RenderSettings>,
    mut chunks: ResMut<TerrainChunks>,
    camera: Query<&GlobalTransform, With<CameraRig>>,
) {
    let (Some(map), Ok(cam)) = (map, camera.single()) else {
        return;
    };
    let (Some(flat), Some(textured)) = (chunks.flat.clone(), chunks.textured.clone()) else {
        return;
    };
    let eye = cam.translation();
    let cs = map.chunk_size() as f32;
    let mut textured_count = 0;
    for c in 0..chunks.built.len() {
        let Some((entity, ..)) = chunks.built[c] else {
            continue;
        };
        let origin = map.chunk_origin(c as u32).as_vec2();
        // Nearest point of the chunk's footprint (world z = -tile y).
        let nx = eye.x.clamp(origin.x, origin.x + cs);
        let nz = eye.z.clamp(-(origin.y + cs), -origin.y);
        let distance = eye.distance(Vec3::new(nx, 0.0, nz));
        let wanted = match settings.select(chunks.bands[c], distance) {
            Band::Detail => Band::Detail,
            _ => Band::LowPoly,
        };
        if wanted == Band::Detail {
            textured_count += 1;
        }
        if chunks.bands[c] != Some(wanted) {
            let m = if wanted == Band::Detail {
                textured.clone()
            } else {
                flat.clone()
            };
            commands.entity(entity).insert(MeshMaterial3d(m));
            chunks.bands[c] = Some(wanted);
        }
    }
    chunks.textured_count = textured_count;
}

/// Height and colour at a tile corner, averaged over the up-to-four tiles
/// that share it.
fn corner(map: &WorldMap, x: i32, y: i32, overlay: Option<&Overlay>) -> (f32, LinearRgba) {
    let mut h = 0.0;
    let mut color = Vec4::ZERO;
    let mut n = 0.0;
    for (dx, dy) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
        let t = IVec2::new(x + dx, y + dy);
        if !map.in_bounds(t) {
            continue;
        }
        let terrain = map.terrain(t);
        let height = map.height(t);
        h += elevation(map, height);
        let mut c = terrain_color(terrain).to_linear().to_vec4();
        if terrain == Terrain::Water {
            // Deeper water is darker.
            let depth = ((map.sea_level() - height) * 6.0).clamp(0.0, 0.6);
            c *= Vec4::new(1.0 - depth, 1.0 - depth, 1.0 - depth * 0.5, 1.0);
        }
        color += c;
        n += 1.0;
    }
    if n == 0.0 {
        return (0.0, terrain_color(Terrain::Water).to_linear());
    }
    let mut c = color / n;
    if let Some(o) = overlay {
        c = o.tint(x as f32, y as f32, c);
    }
    (h / n, LinearRgba::new(c.x, c.y, c.z, 1.0))
}

pub fn build_chunk_mesh(map: &WorldMap, chunk: u32, uv_tiles: f32) -> Mesh {
    build_chunk_mesh_tinted(map, chunk, uv_tiles, None)
}

/// [`build_chunk_mesh`] with vertex colours tinted by an overlay.
pub fn build_chunk_mesh_tinted(
    map: &WorldMap,
    chunk: u32,
    uv_tiles: f32,
    overlay: Option<&Overlay>,
) -> Mesh {
    let cs = map.chunk_size() as i32;
    let origin = map.chunk_origin(chunk);
    let side = (cs + 1) as usize;
    let mut positions = Vec::with_capacity(side * side);
    let mut normals = Vec::with_capacity(side * side);
    let mut colors = Vec::with_capacity(side * side);
    let mut uvs = Vec::with_capacity(side * side);
    let height_at = |x: i32, y: i32| corner(map, x, y, None).0;
    for ly in 0..=cs {
        for lx in 0..=cs {
            let (x, y) = (origin.x + lx, origin.y + ly);
            let (h, color) = corner(map, x, y, overlay);
            positions.push(tile_to_world(lx as f32, ly as f32, h).to_array());
            // Global tile coordinates, so the detail texture tiles
            // seamlessly across chunk borders.
            uvs.push([x as f32 / uv_tiles, y as f32 / uv_tiles]);
            // Central differences over the whole map, so normals match
            // across chunk borders. World z = -y, hence the sign on dz.
            let dx = (height_at(x + 1, y) - height_at(x - 1, y)) * 0.5;
            let dz = -(height_at(x, y + 1) - height_at(x, y - 1)) * 0.5;
            normals.push(Vec3::new(-dx, 1.0, -dz).normalize().to_array());
            colors.push(color.to_f32_array());
        }
    }
    let mut indices = Vec::with_capacity((cs * cs * 6) as usize);
    for ly in 0..cs as u32 {
        for lx in 0..cs as u32 {
            let a = ly * side as u32 + lx;
            let (b, c, d) = (a + 1, a + side as u32 + 1, a + side as u32);
            // Counter-clockwise seen from above (+Y).
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}
