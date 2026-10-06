//! Rendering. Runs in `Update`, reads simulation state, never writes it
//! (ADR 0002).
//!
//! # Main pieces
//! - [`terrain`]: chunked terrain meshes rebuilt when a chunk's revision
//!   changes (RND-01).
//! - [`camera`]: the orbit camera rig (RND-02).
//! - [`agents`]: people in detail bands, interpolated between ticks (RND-03,
//!   RND-06, ADR 0008). Villagers are coloured by occupation, soldiers by
//!   faction; captains carry banners.
//! - [`buildings`]: houses and workplaces in the same bands.
//! - [`art`]: procedurally generated textures (ADR 0009).
//! - [`bands`]: detail-band selection with hysteresis (`render.ron`).
//!
//! Render-side state lives in render resources keyed by sim `Entity`, never
//! on sim entities.

pub mod agents;
pub mod art;
pub mod bands;
pub mod buildings;
pub mod camera;
pub mod terrain;

use bevy::prelude::*;

pub use agents::{AgentVisual, AgentVisuals, Look, faction_color, occupation_color};
pub use bands::{Band, RenderSettings};
pub use buildings::BuildingVisuals;
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
            .init_resource::<RenderSettings>()
            .add_systems(PreStartup, art::generate)
            .add_plugins((
                terrain::TerrainPlugin,
                camera::CameraRigPlugin,
                buildings::BuildingVisualsPlugin,
                agents::AgentVisualsPlugin,
            ))
            .add_systems(Startup, setup_light);
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
