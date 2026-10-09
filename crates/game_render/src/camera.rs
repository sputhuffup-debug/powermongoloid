//! RND-02: perspective orbit camera. Pan with WASD/arrows, screen edges or
//! right/middle-drag; zoom with the wheel; Q/E tap to rotate 90 degrees, hold
//! to rotate freely; N / Shift+N jump to the next / previous settlement.
//! Every motion eases toward its target.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use sim_core::SpawnId;
use sim_world::{Settlement, WorldMap};

use crate::terrain::{surface_height_at, tile_to_world};

pub const MIN_DISTANCE: f32 = 20.0;
pub const MAX_DISTANCE: f32 = 3200.0;
/// Pitch (radians above the horizon) when fully zoomed in / out.
const PITCH_NEAR: f32 = 0.45;
const PITCH_FAR: f32 = 1.25;
/// How quickly values approach their targets (per second).
const DAMPING: f32 = 10.0;
/// A Q/E press shorter than this is a 90-degree step; longer rotates freely.
const TAP_SECONDS: f32 = 0.25;
const FREE_ROTATE_SPEED: f32 = 1.6;
const EDGE_PIXELS: f32 = 10.0;

pub struct CameraRigPlugin;

impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera).add_systems(
            Update,
            (
                (rig_input, jump_to_settlement).run_if(|g: Res<crate::InputGate>| !g.blocked),
                apply_rig,
            )
                .chain()
                .in_set(crate::RenderSet::World),
        );
    }
}

/// Orbit state. `focus` is on the map plane in tile coordinates.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct CameraRig {
    pub focus: Vec2,
    pub yaw: f32,
    pub distance: f32,
    pub target_focus: Vec2,
    pub target_yaw: f32,
    pub target_distance: f32,
    /// Smoothed terrain height under the focus.
    pub focus_height: f32,
    /// Seconds Q/E has been held (sign: + for Q, - for E).
    rotate_held: f32,
}

impl CameraRig {
    pub fn new(focus: Vec2, distance: f32) -> Self {
        Self {
            focus,
            yaw: 0.0,
            distance,
            target_focus: focus,
            target_yaw: 0.0,
            target_distance: distance,
            focus_height: 0.0,
            rotate_held: 0.0,
        }
    }

    pub fn focus_tile(&self) -> Vec2 {
        self.focus
    }

    pub fn pitch(&self) -> f32 {
        pitch_for(self.distance)
    }

    /// Advances the eased values toward their targets.
    pub fn step(&mut self, dt: f32) {
        self.focus = damp_vec2(self.focus, self.target_focus, dt);
        self.yaw = damp(self.yaw, self.target_yaw, dt);
        self.distance = damp(self.distance, self.target_distance, dt);
    }

    /// The camera transform for the current state.
    pub fn transform(&self) -> Transform {
        let focus = tile_to_world(self.focus.x, self.focus.y, self.focus_height);
        let pitch = self.pitch();
        let offset = Vec3::new(
            self.yaw.sin() * pitch.cos(),
            pitch.sin(),
            self.yaw.cos() * pitch.cos(),
        ) * self.distance;
        Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y)
    }
}

/// Exponential easing: frame-rate independent, never overshoots.
pub fn damp(current: f32, target: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-DAMPING * dt).exp())
}

fn damp_vec2(current: Vec2, target: Vec2, dt: f32) -> Vec2 {
    Vec2::new(damp(current.x, target.x, dt), damp(current.y, target.y, dt))
}

/// Pitch rises from near-horizontal close up to near top-down far out.
pub fn pitch_for(distance: f32) -> f32 {
    let t = ((distance / MIN_DISTANCE).ln() / (MAX_DISTANCE / MIN_DISTANCE).ln()).clamp(0.0, 1.0);
    PITCH_NEAR + (PITCH_FAR - PITCH_NEAR) * t
}

/// Map-plane movement (in tiles) for a screen-space direction (x = right,
/// y = up) at the given yaw. Speed scales with zoom.
pub fn pan_delta(dir: Vec2, yaw: f32, distance: f32, dt: f32) -> Vec2 {
    if dir == Vec2::ZERO {
        return Vec2::ZERO;
    }
    let dir = dir.normalize();
    // Screen-up is away from the camera: world (-sin yaw, -cos yaw) in x/z,
    // which is (+y tiles) when yaw = 0 because world z = -tile y.
    let forward = Vec2::new(-yaw.sin(), yaw.cos());
    let right = Vec2::new(yaw.cos(), yaw.sin());
    (right * dir.x + forward * dir.y) * distance * 0.9 * dt
}

/// Zooms the target distance by wheel `lines` (positive = in).
pub fn zoom(target: f32, lines: f32) -> f32 {
    (target * 0.9f32.powf(lines)).clamp(MIN_DISTANCE, MAX_DISTANCE)
}

/// The next 90-degree yaw from `target` in direction `dir` (+1 or -1),
/// snapping to the grid first if free rotation left it between steps.
pub fn snap_yaw(target: f32, dir: f32) -> f32 {
    let steps = target / FRAC_PI_2;
    let snapped = if dir > 0.0 {
        steps.floor() + 1.0
    } else {
        steps.ceil() - 1.0
    };
    snapped * FRAC_PI_2
}

/// Keeps the focus on the map.
pub fn clamp_focus(focus: Vec2, size: f32) -> Vec2 {
    focus.clamp(Vec2::ZERO, Vec2::splat(size))
}

fn spawn_camera(mut commands: Commands, map: Option<Res<WorldMap>>) {
    let size = map.as_ref().map_or(256.0, |m| m.size() as f32);
    // Start on an overview of the whole island.
    let rig = CameraRig::new(Vec2::splat(size / 2.0), size * 1.15);
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            near: 0.5,
            far: 20_000.0,
            ..default()
        }),
        rig.transform(),
        rig,
    ));
}

#[allow(clippy::too_many_arguments)]
fn rig_input(
    // Real time: the camera moves while the game is paused.
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Query<&Window, With<PrimaryWindow>>,
    map: Option<Res<WorldMap>>,
    mut rigs: Query<&mut CameraRig>,
) {
    let dt = time.delta_secs();
    let Ok(mut rig) = rigs.single_mut() else {
        return;
    };

    // Keyboard and screen-edge panning.
    let mut dir = Vec2::ZERO;
    for (keys_for, d) in [
        ([KeyCode::KeyW, KeyCode::ArrowUp], Vec2::Y),
        ([KeyCode::KeyS, KeyCode::ArrowDown], Vec2::NEG_Y),
        ([KeyCode::KeyA, KeyCode::ArrowLeft], Vec2::NEG_X),
        ([KeyCode::KeyD, KeyCode::ArrowRight], Vec2::X),
    ] {
        if keys.any_pressed(keys_for) {
            dir += d;
        }
    }
    if let Ok(window) = window.single()
        && window.focused
        && let Some(cursor) = window.cursor_position()
    {
        let (w, h) = (window.width(), window.height());
        if cursor.x <= EDGE_PIXELS {
            dir.x -= 1.0;
        } else if cursor.x >= w - EDGE_PIXELS {
            dir.x += 1.0;
        }
        // Window y runs downwards.
        if cursor.y <= EDGE_PIXELS {
            dir.y += 1.0;
        } else if cursor.y >= h - EDGE_PIXELS {
            dir.y -= 1.0;
        }
    }
    let boost = if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        3.0
    } else {
        1.0
    };
    let (yaw, distance) = (rig.yaw, rig.distance);
    rig.target_focus += pan_delta(dir, yaw, distance, dt) * boost;

    // Drag panning: the ground follows the cursor.
    if mouse.any_pressed([MouseButton::Right, MouseButton::Middle]) && motion.delta != Vec2::ZERO {
        let per_pixel = distance * 0.0012;
        let screen = Vec2::new(-motion.delta.x, motion.delta.y) * per_pixel;
        let forward = Vec2::new(-yaw.sin(), yaw.cos());
        let right = Vec2::new(yaw.cos(), yaw.sin());
        let delta = right * screen.x + forward * screen.y;
        rig.target_focus += delta;
        rig.focus += delta;
    }

    // Wheel zoom.
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if lines != 0.0 {
        rig.target_distance = zoom(rig.target_distance, lines);
    }

    // Q/E: tap for a 90-degree step, hold to rotate freely.
    for (key, sign) in [(KeyCode::KeyQ, 1.0f32), (KeyCode::KeyE, -1.0f32)] {
        if keys.just_pressed(key) {
            rig.rotate_held = 0.0;
        }
        if keys.pressed(key) {
            rig.rotate_held += dt;
            if rig.rotate_held >= TAP_SECONDS {
                rig.target_yaw += sign * FREE_ROTATE_SPEED * dt;
            }
        }
        if keys.just_released(key) && rig.rotate_held < TAP_SECONDS {
            rig.target_yaw = snap_yaw(rig.target_yaw, sign);
        }
    }
    // Keep angles bounded without a visible jump.
    if rig.target_yaw.abs() > 4.0 * PI {
        let wrap = (rig.target_yaw / (2.0 * PI)).trunc() * 2.0 * PI;
        rig.target_yaw -= wrap;
        rig.yaw -= wrap;
    }

    if let Some(map) = map {
        let size = map.size() as f32;
        rig.target_focus = clamp_focus(rig.target_focus, size);
        let ground = surface_height_at(&map, rig.focus.x, rig.focus.y);
        rig.focus_height = damp(rig.focus_height, ground, dt);
    }
}

/// Distance the camera settles at when jumping to a settlement.
const SETTLEMENT_VIEW_DISTANCE: f32 = 110.0;

/// N / Shift+N: cycle the camera through settlements in stable ID order.
fn jump_to_settlement(
    keys: Res<ButtonInput<KeyCode>>,
    settlements: Query<(&SpawnId, &Settlement, Has<sim_economy::SettlementEconomy>)>,
    mut rigs: Query<&mut CameraRig>,
    mut index: Local<Option<usize>>,
) {
    if !keys.just_pressed(KeyCode::KeyN) {
        return;
    }
    let Ok(mut rig) = rigs.single_mut() else {
        return;
    };
    // Populated settlements first, then the rest, each in stable ID order.
    let mut sorted: Vec<(bool, SpawnId, IVec2)> = settlements
        .iter()
        .map(|(id, s, populated)| (!populated, *id, s.center))
        .collect();
    if sorted.is_empty() {
        return;
    }
    sorted.sort_unstable_by_key(|(unpopulated, id, _)| (*unpopulated, *id));
    let list: Vec<(SpawnId, IVec2)> = sorted.into_iter().map(|(_, id, c)| (id, c)).collect();
    let back = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let n = list.len();
    let next = match (*index, back) {
        (None, false) => 0,
        (None, true) => n - 1,
        (Some(i), false) => (i + 1) % n,
        (Some(i), true) => (i + n - 1) % n,
    };
    *index = Some(next);
    rig.target_focus = list[next].1.as_vec2() + 0.5;
    rig.target_distance = SETTLEMENT_VIEW_DISTANCE;
}

fn apply_rig(time: Res<Time<Real>>, mut q: Query<(&mut CameraRig, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut rig, mut transform) in &mut q {
        rig.step(dt);
        *transform = rig.transform();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn pan_follows_yaw() {
        // Yaw 0: screen-up is +y tiles (north), screen-right is +x.
        let up = pan_delta(Vec2::Y, 0.0, 100.0, 1.0);
        assert!(close(up.x, 0.0) && up.y > 0.0);
        let right = pan_delta(Vec2::X, 0.0, 100.0, 1.0);
        assert!(right.x > 0.0 && close(right.y, 0.0));
        // After a 90-degree turn, screen-up points along the old screen-left.
        let turned = pan_delta(Vec2::Y, FRAC_PI_2, 100.0, 1.0);
        assert!(turned.x < 0.0 && close(turned.y, 0.0));
    }

    #[test]
    fn pan_speed_scales_with_zoom() {
        let near = pan_delta(Vec2::X, 0.0, 10.0, 1.0).length();
        let far = pan_delta(Vec2::X, 0.0, 1000.0, 1.0).length();
        assert!(close(far / near, 100.0));
        assert_eq!(pan_delta(Vec2::ZERO, 0.0, 10.0, 1.0), Vec2::ZERO);
    }

    #[test]
    fn zoom_is_clamped() {
        assert!(zoom(100.0, 1.0) < 100.0);
        assert!(zoom(100.0, -1.0) > 100.0);
        assert_eq!(zoom(MIN_DISTANCE, 50.0), MIN_DISTANCE);
        assert_eq!(zoom(MAX_DISTANCE, -50.0), MAX_DISTANCE);
    }

    #[test]
    fn rotation_snaps_to_quarter_turns() {
        assert!(close(snap_yaw(0.0, 1.0), FRAC_PI_2));
        assert!(close(snap_yaw(0.0, -1.0), -FRAC_PI_2));
        // From a free-rotated angle, the next step lands on the grid.
        assert!(close(snap_yaw(0.3, 1.0), FRAC_PI_2));
        assert!(close(snap_yaw(0.3, -1.0), 0.0));
        assert!(close(snap_yaw(FRAC_PI_2, 1.0), PI));
    }

    #[test]
    fn easing_is_smooth_and_converges() {
        let mut rig = CameraRig::new(Vec2::ZERO, 100.0);
        rig.target_focus = Vec2::new(100.0, 0.0);
        rig.target_distance = 50.0;
        let mut last = 0.0;
        for _ in 0..120 {
            rig.step(1.0 / 60.0);
            assert!(rig.focus.x >= last && rig.focus.x <= 100.0, "no overshoot");
            last = rig.focus.x;
        }
        assert!((rig.focus.x - 100.0).abs() < 0.01);
        assert!((rig.distance - 50.0).abs() < 0.01);
        // A single frame moves only part of the way: no jumps.
        let mut rig = CameraRig::new(Vec2::ZERO, 100.0);
        rig.target_focus = Vec2::new(100.0, 0.0);
        rig.step(1.0 / 60.0);
        assert!(rig.focus.x > 0.0 && rig.focus.x < 20.0);
    }

    #[test]
    fn pitch_rises_with_distance_and_camera_looks_at_focus() {
        assert!(pitch_for(MIN_DISTANCE) < pitch_for(MAX_DISTANCE));
        let rig = CameraRig::new(Vec2::new(10.0, 20.0), 200.0);
        let t = rig.transform();
        let focus = tile_to_world(10.0, 20.0, 0.0);
        assert!((t.translation.distance(focus) - 200.0).abs() < 0.01);
        let looking = t.forward().as_vec3();
        assert!(looking.dot((focus - t.translation).normalize()) > 0.999);
    }

    #[test]
    fn focus_stays_on_the_map() {
        assert_eq!(
            clamp_focus(Vec2::new(-5.0, 3000.0), 2048.0),
            Vec2::new(0.0, 2048.0)
        );
    }
}
