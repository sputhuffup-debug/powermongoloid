//! Rendering. Runs in `Update`, reads simulation state, never writes it.
//!
//! M1: chunked terrain meshes (RND-01), the orbit camera rig (RND-02) and
//! settlement visuals; the M0 placeholder wanderers are drawn as gizmos.

pub mod camera;
pub mod settlements;
pub mod terrain;

use bevy::prelude::*;
use sim_core::demo::Wanderer;
use sim_core::{Motion, Position};
use sim_world::WorldMap;

pub use camera::CameraRig;
pub use terrain::{TerrainChunks, surface_point, tile_to_world};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.53, 0.70, 0.86)))
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 400.0,
                ..default()
            })
            .add_plugins((
                terrain::TerrainPlugin,
                camera::CameraRigPlugin,
                settlements::SettlementVisualsPlugin,
            ))
            .add_systems(Startup, setup_light)
            .add_systems(Update, draw_wanderers);
    }
}

fn setup_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            // Shadows over a 2048-tile island need cascades tuned per zoom;
            // off until the art pass (RND-05).
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 1.0, 0.0).looking_at(Vec3::new(-0.6, 0.0, -0.4), Vec3::Y),
    ));
}

fn draw_wanderers(
    q: Query<(&Position, &Motion), With<Wanderer>>,
    map: Option<Res<WorldMap>>,
    mut gizmos: Gizmos,
) {
    let Some(map) = map else { return };
    for (pos, motion) in &q {
        let p = pos.0.as_vec2() + motion.offset + 0.5;
        let ground = terrain::surface_height_at(&map, p.x, p.y);
        gizmos.sphere(
            tile_to_world(p.x, p.y, ground + 0.4),
            0.35,
            Color::srgb(0.85, 0.25, 0.20),
        );
    }
}
