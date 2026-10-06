//! Clicking the world (UI-02, UI-03): a ray from the cursor to the terrain,
//! then the nearest villager there (via the sim's `SpatialGrid`), else the
//! nearest settlement. Esc clears the selection. Selection is UI state; it
//! never touches the simulation.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_render::CameraRig;
use game_render::terrain::surface_height_at;
use sim_world::{Settlement, SpatialGrid, WorldMap};

pub struct SelectionPlugin;

impl Plugin for SelectionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selection>()
            .add_systems(Update, (pick, highlight).chain());
    }
}

#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct Selection {
    pub villager: Option<Entity>,
    pub settlement: Option<Entity>,
}

/// Where a ray from the camera first meets the terrain, in tile
/// coordinates. Marches in steps that shrink near the ground.
pub fn ground_hit(map: &WorldMap, ray: Ray3d) -> Option<Vec2> {
    let dir = *ray.direction;
    if dir.y >= 0.0 {
        return None;
    }
    let mut t = 0.0;
    let mut prev = ray.origin;
    for _ in 0..20_000 {
        let p = ray.origin + dir * t;
        let ground = surface_height_at(map, p.x, -p.z);
        if p.y <= ground {
            // Refine between the last point above ground and this one.
            let (mut a, mut b) = (prev, p);
            for _ in 0..12 {
                let m = (a + b) / 2.0;
                if m.y <= surface_height_at(map, m.x, -m.z) {
                    b = m;
                } else {
                    a = m;
                }
            }
            return Some(Vec2::new(b.x, -b.z));
        }
        prev = p;
        let height_above = (p.y - ground).max(0.0);
        t += (height_above * 0.5).clamp(0.25, 20.0);
        if t > 50_000.0 {
            break;
        }
    }
    None
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn pick(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    map: Option<Res<WorldMap>>,
    grid: Option<Res<SpatialGrid>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<CameraRig>>,
    ui: Query<&Interaction>,
    settlements: Query<(Entity, &Settlement)>,
    mut selection: ResMut<Selection>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        *selection = Selection::default();
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    // Clicks on UI panels and buttons aren't world clicks.
    if ui.iter().any(|i| *i != Interaction::None) {
        debug!("click on UI");
        return;
    }
    let (Some(map), Ok(window), Ok((camera, cam_gt))) = (map, window.single(), camera.single())
    else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(cam_gt, cursor) else {
        return;
    };
    let Some(hit) = ground_hit(&map, ray) else {
        return;
    };
    let distance = cam_gt.translation().distance(Vec3::new(hit.x, 0.0, -hit.y));
    // Generous at a distance, where villagers are a few pixels wide.
    let radius = (distance * 0.015).clamp(1.8, 25.0);
    debug!(
        "click at {cursor}: ground {hit}, radius {radius}, grid {} entities, nearest {:?}",
        grid.as_ref().map_or(0, |g| g.len()),
        grid.as_ref()
            .and_then(|g| g.within(hit, 40.0).first().map(|(_, d)| *d))
    );
    if let Some((villager, _)) = grid
        .as_ref()
        .and_then(|g| g.within(hit, radius).into_iter().next())
    {
        info!("selected villager {villager} near tile {hit}");
        *selection = Selection {
            villager: Some(villager),
            settlement: None,
        };
        return;
    }
    let nearest = settlements
        .iter()
        .map(|(e, s)| (e, s.center.as_vec2().distance(hit)))
        .filter(|(_, d)| *d < 20.0)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    *selection = Selection {
        villager: None,
        settlement: nearest.map(|(e, _)| e),
    };
}

fn highlight(
    selection: Res<Selection>,
    map: Option<Res<WorldMap>>,
    visuals: Option<Res<game_render::AgentVisuals>>,
    transforms: Query<&GlobalTransform>,
    settlements: Query<&Settlement>,
    mut gizmos: Gizmos,
) {
    let Some(map) = map else { return };
    if let (Some(v), Some(visuals)) = (selection.villager, visuals)
        && let Some((render, ..)) = visuals.by_sim.get(&v)
        && let Ok(gt) = transforms.get(*render)
    {
        let p = gt.translation();
        gizmos.circle(
            Isometry3d::new(
                p + Vec3::Y * 0.05,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            0.8,
            Color::srgb(1.0, 0.9, 0.3),
        );
    }
    if let Some(s) = selection.settlement
        && let Ok(settlement) = settlements.get(s)
    {
        let p = game_render::surface_point(&map, settlement.center);
        gizmos.circle(
            Isometry3d::new(
                p + Vec3::Y * 0.3,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            14.0,
            Color::srgb(1.0, 0.9, 0.3),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_flat_ground_where_expected() {
        let map = sim_world::generate_world(
            1,
            &sim_world::WorldGenSettings::default_data().with_size(128),
            1,
        )
        .map;
        // Straight down onto a tile far out at sea (height clamps to 0).
        let ray = Ray3d::new(Vec3::new(5.5, 50.0, -7.5), Dir3::NEG_Y);
        let hit = ground_hit(&map, ray).expect("hits");
        assert!((hit - Vec2::new(5.5, 7.5)).length() < 0.05, "{hit}");
        // Pointing up never hits.
        assert!(ground_hit(&map, Ray3d::new(Vec3::Y, Dir3::Y)).is_none());
    }
}
