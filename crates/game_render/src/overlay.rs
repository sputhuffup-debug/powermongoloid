//! M4 overlays: the terrain around each settlement (or monument) is tinted
//! by ownership, goodwill or loyalty toward the player, food security, or
//! monument holders. O cycles the modes. The tint is a list of discs; it is
//! recomputed twice a second, and its `generation` (which the terrain
//! folds into each chunk's staleness check) moves only when a quantised
//! colour changes, so terrain chunks rebuild rarely.

use bevy::prelude::*;
use sim_agents::{FoodSecurity, Goodwill, Owner, SettlementLoyalty};
use sim_core::{FactionId, Position};
use sim_economy::SettlementEconomy;
use sim_military::{Army, Holder, Progression};
use sim_world::{Monument, Settlement, WorldMap};

use crate::terrain::surface_point;
use crate::{InputGate, RenderSet, faction_color};

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Overlay>().add_systems(
            Update,
            (overlay_keys, compute_overlay, monument_rings)
                .chain()
                .in_set(RenderSet::Overlay),
        );
    }
}

/// What the terrain tint shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayMode {
    /// No tint.
    #[default]
    Off,
    /// Each settlement in its owner's colour.
    Ownership,
    /// Loyalty in your settlements, goodwill toward you elsewhere: red
    /// (hostile) to green (devoted).
    Standing,
    /// Days of food: red (none) to green (two days or more).
    Food,
    /// Monuments in their holder's colour, with rings at their hold radius.
    Monuments,
}

impl OverlayMode {
    /// Every mode, in the order O cycles through them.
    pub const ALL: [OverlayMode; 5] = [
        OverlayMode::Off,
        OverlayMode::Ownership,
        OverlayMode::Standing,
        OverlayMode::Food,
        OverlayMode::Monuments,
    ];

    /// Shown in the run bar.
    pub fn name(self) -> &'static str {
        match self {
            OverlayMode::Off => "off",
            OverlayMode::Ownership => "ownership",
            OverlayMode::Standing => "goodwill / loyalty",
            OverlayMode::Food => "food security",
            OverlayMode::Monuments => "monuments",
        }
    }

    /// The next mode (O).
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// A tinted disc of terrain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Disc {
    /// Centre, in tiles.
    pub center: Vec2,
    /// Radius, in tiles.
    pub radius: f32,
    /// Quantised colour (8 levels a channel), so tiny changes never rebuild.
    pub color: [u8; 3],
}

impl Disc {
    fn linear(&self) -> Vec4 {
        let c = |v: u8| v as f32 / 7.0;
        Color::srgb(c(self.color[0]), c(self.color[1]), c(self.color[2]))
            .to_linear()
            .to_vec4()
    }
}

/// The current overlay.
#[derive(Resource, Debug, Default)]
pub struct Overlay {
    /// What it shows.
    pub mode: OverlayMode,
    /// Changes whenever `discs` changes.
    pub generation: u32,
    discs: Vec<Disc>,
    /// Seconds until the next recompute.
    cooldown: f32,
}

/// How strongly the tint covers the terrain at a disc's centre.
const STRENGTH: f32 = 0.55;

impl Overlay {
    /// The discs drawn now.
    pub fn discs(&self) -> &[Disc] {
        &self.discs
    }

    /// A signature of the discs touching a chunk's footprint (origin and
    /// side in tiles): equal signatures draw identically.
    pub fn chunk_signature(&self, origin: Vec2, side: f32) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for d in &self.discs {
            let nearest = d.center.clamp(origin, origin + side);
            if nearest.distance(d.center) > d.radius {
                continue;
            }
            for v in [
                d.center.x.to_bits(),
                d.center.y.to_bits(),
                d.radius.to_bits(),
                u32::from_le_bytes([d.color[0], d.color[1], d.color[2], 1]),
            ] {
                h = (h ^ v as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
        h
    }

    /// Tints a linear terrain colour at tile corner `(x, y)`.
    pub fn tint(&self, x: f32, y: f32, color: Vec4) -> Vec4 {
        let p = Vec2::new(x, y);
        let mut best: Option<(f32, &Disc)> = None;
        for d in &self.discs {
            let r = p.distance(d.center) / d.radius;
            if r < 1.0 && best.is_none_or(|(b, _)| r < b) {
                best = Some((r, d));
            }
        }
        match best {
            // Soft edge over the outer fifth.
            Some((r, d)) => {
                let a = STRENGTH * ((1.0 - r) / 0.2).clamp(0.0, 1.0);
                color * (1.0 - a) + d.linear() * a
            }
            None => color,
        }
    }
}

fn quantise(c: Color) -> [u8; 3] {
    let s = c.to_srgba();
    let q = |v: f32| (v.clamp(0.0, 1.0) * 7.0).round() as u8;
    [q(s.red), q(s.green), q(s.blue)]
}

/// Red (0) through yellow to green (1).
pub fn red_to_green(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::srgb((2.0 - 2.0 * t).min(1.0), (2.0 * t).min(1.0), 0.15)
}

fn overlay_keys(
    keys: Res<ButtonInput<KeyCode>>,
    gate: Res<InputGate>,
    mut overlay: ResMut<Overlay>,
) {
    let _span = info_span!("game_render::overlay::keys").entered();
    if !gate.blocked && keys.just_pressed(KeyCode::KeyO) {
        overlay.mode = overlay.mode.next();
        overlay.cooldown = 0.0;
        info!("overlay: {}", overlay.mode.name());
    }
}

/// Settlements' disc radius: wider for bigger places.
fn settlement_radius(people: u32) -> f32 {
    (6.0 + (people as f32).sqrt() * 0.9).clamp(8.0, 26.0)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters.
fn compute_overlay(
    time: Res<Time<Real>>,
    army: Option<Res<Army>>,
    mut overlay: ResMut<Overlay>,
    settlements: Query<(
        &Settlement,
        &SettlementEconomy,
        Option<&Owner>,
        Option<&Goodwill>,
        Option<&SettlementLoyalty>,
        Option<&FoodSecurity>,
    )>,
    monuments: Query<(&Position, &Holder), With<Monument>>,
) {
    let _span = info_span!("game_render::overlay::compute").entered();
    overlay.cooldown -= time.delta_secs();
    if overlay.cooldown > 0.0 {
        return;
    }
    overlay.cooldown = 0.5;
    let Some(army) = army else { return };
    let mut discs = Vec::new();
    match overlay.mode {
        OverlayMode::Off => {}
        OverlayMode::Monuments => {
            for (pos, holder) in &monuments {
                discs.push(Disc {
                    center: pos.0.as_vec2() + 0.5,
                    radius: 9.0,
                    color: quantise(faction_color(&army, holder.faction.0)),
                });
            }
        }
        mode => {
            for (s, se, owner, goodwill, loyalty, food) in &settlements {
                let owner = owner.map_or(FactionId::INDEPENDENT, |o| o.faction);
                let color = match mode {
                    OverlayMode::Ownership => faction_color(&army, owner.0),
                    OverlayMode::Standing => {
                        if owner == FactionId::PLAYER {
                            red_to_green(loyalty.map_or(0.5, |l| l.mean))
                        } else {
                            let g = goodwill.map_or(0.0, |g| g.get(FactionId::PLAYER));
                            red_to_green((g + 1.0) / 2.0)
                        }
                    }
                    _ => red_to_green(food.map_or(0.0, |f| f.days / 2.0)),
                };
                discs.push(Disc {
                    center: s.center.as_vec2() + 0.5,
                    radius: settlement_radius(se.population),
                    color: quantise(color),
                });
            }
        }
    }
    // Query order is not stable; sort so equal sets compare equal.
    discs.sort_by(|a, b| {
        (a.center.x, a.center.y)
            .partial_cmp(&(b.center.x, b.center.y))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if discs != overlay.discs {
        overlay.discs = discs;
        overlay.generation = overlay.generation.wrapping_add(1);
    }
}

/// Monument overlay: a ring at each monument's hold radius, in its
/// holder's colour.
fn monument_rings(
    overlay: Res<Overlay>,
    map: Option<Res<WorldMap>>,
    army: Option<Res<Army>>,
    progression: Option<Res<Progression>>,
    monuments: Query<(&Position, &Holder), With<Monument>>,
    mut gizmos: Gizmos,
) {
    let _span = info_span!("game_render::overlay::rings").entered();
    if overlay.mode != OverlayMode::Monuments {
        return;
    }
    let (Some(map), Some(army), Some(progression)) = (map, army, progression) else {
        return;
    };
    let radius = progression.data.monuments.hold_radius;
    for (pos, holder) in &monuments {
        let p = surface_point(&map, pos.0);
        gizmos.circle(
            Isometry3d::new(
                p + Vec3::Y * 0.6,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            radius,
            faction_color(&army, holder.faction.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_cycle_back_to_off() {
        let mut m = OverlayMode::Off;
        for _ in 0..OverlayMode::ALL.len() {
            m = m.next();
        }
        assert_eq!(m, OverlayMode::Off);
    }

    #[test]
    fn tint_covers_only_the_disc_and_signatures_track_it() {
        let mut o = Overlay::default();
        let base = Vec4::new(0.2, 0.2, 0.2, 1.0);
        assert_eq!(o.tint(5.0, 5.0, base), base);
        let empty = o.chunk_signature(Vec2::ZERO, 64.0);
        o.discs.push(Disc {
            center: Vec2::new(10.0, 10.0),
            radius: 8.0,
            color: [7, 0, 0],
        });
        assert_ne!(o.tint(10.0, 10.0, base), base);
        assert_eq!(o.tint(30.0, 30.0, base), base);
        assert_ne!(o.chunk_signature(Vec2::ZERO, 64.0), empty);
        // A chunk far away is untouched.
        assert_eq!(
            o.chunk_signature(Vec2::splat(640.0), 64.0),
            Overlay::default().chunk_signature(Vec2::splat(640.0), 64.0)
        );
    }

    #[test]
    fn quantising_ignores_small_changes() {
        assert_eq!(quantise(red_to_green(0.50)), quantise(red_to_green(0.505)));
        assert_ne!(quantise(red_to_green(0.0)), quantise(red_to_green(1.0)));
    }
}
