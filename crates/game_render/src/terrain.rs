//! RND-01: terrain as one mesh per 64 x 64 chunk, built from the height map
//! with vertex colours by terrain type. A chunk is (re)built only when its
//! revision in `WorldMap` changes, spread over frames, nearest first.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use sim_world::{Terrain, WorldMap};

use crate::camera::CameraRig;

/// World units per unit of normalised height above sea level.
pub const HEIGHT_SCALE: f32 = 160.0;
/// Chunks built per frame at most, so a full rebuild never stalls a frame.
const CHUNKS_PER_FRAME: usize = 48;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainChunks>()
            .add_systems(Startup, setup_ocean)
            .add_systems(Update, sync_chunks);
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
    /// Per chunk: the entity and the map revision it was built from.
    built: Vec<Option<(Entity, u32)>>,
    material: Option<Handle<StandardMaterial>>,
    pub total: usize,
    pub built_count: usize,
    /// Chunk meshes built since startup, including rebuilds.
    pub builds: u64,
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
        Mesh3d(meshes.add(Plane3d::default().mesh().size(size * 4.0, size * 4.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: terrain_color(Terrain::Water).darker(0.05),
            perceptual_roughness: 0.6,
            ..default()
        })),
        Transform::from_translation(tile_to_world(size / 2.0, size / 2.0, -0.3)),
    ));
}

fn sync_chunks(
    mut commands: Commands,
    map: Option<Res<WorldMap>>,
    mut chunks: ResMut<TerrainChunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    rig: Query<&CameraRig>,
) {
    let Some(map) = map else { return };
    let total = map.chunks_per_side().pow(2) as usize;
    if chunks.built.len() != total {
        chunks.built = vec![None; total];
        chunks.total = total;
    }
    let material = chunks
        .material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                // Vertex colours carry the terrain colour.
                base_color: Color::WHITE,
                perceptual_roughness: 0.95,
                reflectance: 0.08,
                ..default()
            })
        })
        .clone();

    let mut stale: Vec<u32> = (0..total as u32)
        .filter(|&c| chunks.built[c as usize].is_none_or(|(_, rev)| rev != map.chunk_revision(c)))
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
        let mesh = meshes.add(build_chunk_mesh(&map, c));
        let origin = map.chunk_origin(c);
        let transform =
            Transform::from_translation(tile_to_world(origin.x as f32, origin.y as f32, 0.0));
        let revision = map.chunk_revision(c);
        match chunks.built[c as usize] {
            Some((entity, _)) => {
                commands.entity(entity).insert(Mesh3d(mesh));
            }
            None => {
                let entity = commands
                    .spawn((Mesh3d(mesh), MeshMaterial3d(material.clone()), transform))
                    .id();
                chunks.built_count += 1;
                chunks.built[c as usize] = Some((entity, revision));
            }
        }
        if let Some(slot) = chunks.built[c as usize].as_mut() {
            slot.1 = revision;
        }
        chunks.builds += 1;
    }
}

/// Height and colour at a tile corner, averaged over the up-to-four tiles
/// that share it.
fn corner(map: &WorldMap, x: i32, y: i32) -> (f32, LinearRgba) {
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
    let c = color / n;
    (h / n, LinearRgba::new(c.x, c.y, c.z, 1.0))
}

pub fn build_chunk_mesh(map: &WorldMap, chunk: u32) -> Mesh {
    let cs = map.chunk_size() as i32;
    let origin = map.chunk_origin(chunk);
    let side = (cs + 1) as usize;
    let mut positions = Vec::with_capacity(side * side);
    let mut normals = Vec::with_capacity(side * side);
    let mut colors = Vec::with_capacity(side * side);
    let height_at = |x: i32, y: i32| corner(map, x, y).0;
    for ly in 0..=cs {
        for lx in 0..=cs {
            let (x, y) = (origin.x + lx, origin.y + ly);
            let (h, color) = corner(map, x, y);
            positions.push(tile_to_world(lx as f32, ly as f32, h).to_array());
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
    .with_inserted_indices(Indices::U32(indices))
}
