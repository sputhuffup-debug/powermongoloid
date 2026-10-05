//! Rendering. Runs in `Update`, reads simulation state, never writes it.
//!
//! M0: a camera, a light, a ground plane, and the placeholder wanderers drawn
//! as gizmos. Terrain chunks and the camera rig arrive in M1 (RND-01, RND-02).

use bevy::prelude::*;
use sim_core::demo::Wanderer;
use sim_core::{Motion, Position};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.53, 0.70, 0.86)))
            .add_systems(Startup, setup_scene)
            .add_systems(Update, draw_wanderers);
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(30.0, 32.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(48.0, 48.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.36, 0.55, 0.30))),
    ));
}

/// Sim tile (x, y) maps to world (x, 0, y).
fn draw_wanderers(q: Query<(&Position, &Motion), With<Wanderer>>, mut gizmos: Gizmos) {
    for (pos, motion) in &q {
        let p = pos.0.as_vec2() + motion.offset;
        gizmos.sphere(Vec3::new(p.x, 0.3, p.y), 0.3, Color::srgb(0.85, 0.25, 0.20));
    }
}
