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
//! - [`monuments`]: landmark models per monument kind with the holder's
//!   pennant (GP-23, M4).
//! - [`overlay`]: terrain tints for ownership, goodwill and loyalty, food
//!   security and monuments, cycled with O (M4); since M4a also the
//!   influence field of both spheres with its front, sphere edges and
//!   invitation rings (UI-12).
//! - [`WorldEpoch`]: bumped when the sim world is replaced in-process, so
//!   caches keyed by sim entities or chunk revisions clear.
//!
//! Render-side state lives in render resources keyed by sim `Entity`, never
//! on sim entities.

pub mod agents;
pub mod art;
pub mod bands;
pub mod buildings;
pub mod camera;
pub mod monuments;
pub mod overlay;
pub mod terrain;

use bevy::prelude::*;

pub use agents::{AgentVisual, AgentVisuals, Look, faction_color, occupation_color};
pub use bands::{Band, RenderSettings};
pub use buildings::BuildingVisuals;
pub use camera::CameraRig;
pub use monuments::MonumentVisuals;
pub use overlay::{Overlay, OverlayMode};
pub use terrain::{TerrainChunks, surface_point, tile_to_world};

/// System sets of the render side (all in `Update`, chained in this order).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderSet {
    /// Clearing caches when the [`WorldEpoch`] changes.
    Epoch,
    /// Overlay input and tint computation, before terrain reads the tint.
    Overlay,
    /// Terrain, buildings, monuments and people.
    World,
}

/// Counts in-process replacements of the sim world (a new run, or a load
/// in M4 step 8). Whoever replaces the world bumps it; render and UI caches
/// keyed by sim entities or chunk revisions (terrain chunks, building and
/// monument models, labels, the selection, the ocean, the opening shot)
/// clear when it changes. Render-side state: never part of the sim.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WorldEpoch(pub u32);

/// Run condition: the world was replaced since the last frame (not on the
/// first frame, when nothing is cached yet).
pub fn epoch_changed(epoch: Res<WorldEpoch>) -> bool {
    epoch.is_changed() && !epoch.is_added()
}

/// Whether world input (camera keys and wheel, overlay keys) is accepted.
/// `game_ui` blocks it while a full-screen screen (new game, end of run) is
/// up.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InputGate {
    /// True while a full-screen UI owns the keyboard and mouse.
    pub blocked: bool,
}

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
            .init_resource::<WorldEpoch>()
            .init_resource::<InputGate>()
            .configure_sets(
                Update,
                (RenderSet::Epoch, RenderSet::Overlay, RenderSet::World).chain(),
            )
            .add_systems(PreStartup, art::generate)
            .add_plugins((
                terrain::TerrainPlugin,
                camera::CameraRigPlugin,
                buildings::BuildingVisualsPlugin,
                agents::AgentVisualsPlugin,
                monuments::MonumentVisualsPlugin,
                overlay::OverlayPlugin,
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
