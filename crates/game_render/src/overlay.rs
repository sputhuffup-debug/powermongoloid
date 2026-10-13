//! M4 overlays: the terrain around each settlement (or monument) is tinted
//! by ownership, goodwill or loyalty toward the player, food security, or
//! monument holders. O cycles the modes. The tint is a list of discs; it is
//! recomputed twice a second, and its `generation` (which the terrain
//! folds into each chunk's staleness check) moves only when a quantised
//! colour changes, so terrain chunks rebuild rarely.
//!
//! M4a adds [`OverlayMode::Influence`] (UI-12, GP-31): instead of discs, a
//! coarse grid of the player's and the rival's influence shares tints the
//! terrain, with the front where they meet, the dashed sphere edges and
//! rings that fill toward an invitation ([`influence`]). It is computed
//! render-side from each town's `Influence` every two seconds while shown
//! and never writes the sim.

use bevy::prelude::*;
use sim_agents::{FoodSecurity, Goodwill, Owner, SettlementLoyalty};
use sim_core::{FactionId, Position, SpawnId};
use sim_economy::SettlementEconomy;
use sim_military::{Army, Holder, Influence, Progression, RunProgress};
use sim_world::{Monument, Settlement, WorldMap};

use crate::terrain::{surface_height_at, surface_point, tile_to_world};
use crate::{InputGate, RenderSet, faction_color};

pub mod influence;

use influence::{Camp, InfluenceView, Palette, Ring, RingState, TownView};

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Overlay>().add_systems(
            Update,
            (
                overlay_keys,
                compute_overlay,
                monument_rings,
                influence_marks,
            )
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
    /// Influence (GP-31, UI-12): your colour to the rival's, the front
    /// where they meet, the sphere edges and rings that fill toward an
    /// invitation. Appended to the cycle in M4a.
    Influence,
}

impl OverlayMode {
    /// Every mode, in the order O cycles through them.
    pub const ALL: [OverlayMode; 6] = [
        OverlayMode::Off,
        OverlayMode::Ownership,
        OverlayMode::Standing,
        OverlayMode::Food,
        OverlayMode::Monuments,
        OverlayMode::Influence,
    ];

    /// Shown in the run bar.
    pub fn name(self) -> &'static str {
        match self {
            OverlayMode::Off => "off",
            OverlayMode::Ownership => "ownership",
            OverlayMode::Standing => "goodwill / loyalty",
            OverlayMode::Food => "food security",
            OverlayMode::Monuments => "monuments",
            OverlayMode::Influence => "influence",
        }
    }

    /// What the mode's marks mean, for the run bar (the colours of the
    /// sides themselves come from army.ron, so they aren't named here).
    pub fn legend(self) -> Option<&'static str> {
        match self {
            OverlayMode::Influence => Some("amber front, dashed sphere edge"),
            _ => None,
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
    /// Changes whenever the tint (`discs`, or the influence grid) changes.
    pub generation: u32,
    discs: Vec<Disc>,
    /// The influence field, dashes and rings (only in
    /// [`OverlayMode::Influence`]).
    view: Option<InfluenceView>,
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

    /// The influence field drawn now, in [`OverlayMode::Influence`] once
    /// computed.
    pub fn influence(&self) -> Option<&InfluenceView> {
        self.view.as_ref()
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
        if let Some(view) = &self.view {
            view.hash_chunk(origin, side, &mut h);
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
        let color = match best {
            // Soft edge over the outer fifth.
            Some((r, d)) => {
                let a = STRENGTH * ((1.0 - r) / 0.2).clamp(0.0, 1.0);
                color * (1.0 - a) + d.linear() * a
            }
            None => color,
        };
        match &self.view {
            Some(view) => view.tint(x, y, color),
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

/// Seconds between recomputes of the influence field (UI-12).
const INFLUENCE_PERIOD: f32 = 2.0;

/// Seconds between recomputes of the other modes.
const PERIOD: f32 = 0.5;

/// What [`compute_overlay`] reads of a town for the influence field.
type TownQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SpawnId,
        &'static Settlement,
        &'static SettlementEconomy,
        &'static Owner,
        &'static Influence,
        &'static Goodwill,
    ),
>;

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters.
fn compute_overlay(
    time: Res<Time<Real>>,
    army: Option<Res<Army>>,
    map: Option<Res<WorldMap>>,
    progression: Option<Res<Progression>>,
    progress: Option<Res<RunProgress>>,
    mut overlay: ResMut<Overlay>,
    settlements: Query<(
        &Settlement,
        &SettlementEconomy,
        Option<&Owner>,
        Option<&Goodwill>,
        Option<&SettlementLoyalty>,
        Option<&FoodSecurity>,
    )>,
    towns: TownQuery<'_, '_>,
    monuments: Query<(&Position, &Holder), With<Monument>>,
) {
    let _span = info_span!("game_render::overlay::compute").entered();
    overlay.cooldown -= time.delta_secs();
    if overlay.cooldown > 0.0 {
        return;
    }
    overlay.cooldown = if overlay.mode == OverlayMode::Influence {
        INFLUENCE_PERIOD
    } else {
        PERIOD
    };
    let Some(army) = army else { return };
    let mut discs = Vec::new();
    let mut view = None;
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
        OverlayMode::Influence => {
            if let (Some(map), Some(progression)) = (map, progression) {
                let seat = progress.and_then(|p| p.rival_seat);
                let list: Vec<TownView> = towns
                    .iter()
                    .map(|(id, s, se, owner, influence, goodwill)| TownView {
                        id: id.0,
                        center: s.center,
                        people: se.population,
                        owner: owner.faction,
                        seat: Some(id.0) == seat,
                        goodwill: goodwill.get(FactionId::PLAYER),
                        influence: *influence,
                    })
                    .collect();
                // Nothing to show with influence off or outside a run.
                if progression.data.influence.enabled && !list.is_empty() {
                    view = Some(InfluenceView::build(
                        &progression.data.influence,
                        progression.data.invitations.goodwill,
                        &list,
                        map.size(),
                        influence_palette(&army),
                    ));
                }
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
    // The generation moves only when what terrain draws does: the discs or
    // the influence grid. Dashes and rings are gizmos and may change freely.
    let same_field = match (&overlay.view, &view) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same_tint(b),
        _ => false,
    };
    let changed = discs != overlay.discs || !same_field;
    if view != overlay.view {
        overlay.view = view;
    }
    if changed {
        overlay.discs = discs;
        overlay.generation = overlay.generation.wrapping_add(1);
    }
}

/// The influence overlay's colours: the player's and the rival's faction
/// colours and a front colour that neither can be mistaken for (the
/// rival's own colour in army.ron is purple, so the front is amber).
fn influence_palette(army: &Army) -> Palette {
    let linear = |c: Color| c.to_linear().to_vec4();
    Palette {
        player: linear(faction_color(army, FactionId::PLAYER.0)),
        rival: linear(faction_color(army, FactionId::RIVAL.0)),
        front: linear(Color::srgb(0.98, 0.78, 0.18)),
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

/// Points on a ring, `n` of them, from angle 0 round to `fill` of the way.
fn ring_points(ring: &Ring, n: usize) -> Vec<Vec2> {
    let steps = ((ring.fill * n as f32).ceil() as usize).min(n);
    (0..=steps)
        .map(|k| {
            let angle = (k as f32 / n as f32).min(ring.fill) * std::f32::consts::TAU;
            ring.center + Vec2::from_angle(angle) * ring.radius
        })
        .collect()
}

/// How far above the terrain the influence marks float, in world units.
const LIFT: f32 = 0.8;

/// Segments in a full ring.
const RING_SEGMENTS: usize = 48;

/// The influence overlay's dashed sphere edges and rings, drawn with
/// gizmos along the terrain from the view [`compute_overlay`] keeps. Each
/// frame it only reads that view; it never touches the sim.
fn influence_marks(overlay: Res<Overlay>, map: Option<Res<WorldMap>>, mut gizmos: Gizmos) {
    let _span = info_span!("game_render::overlay::influence_marks").entered();
    if overlay.mode != OverlayMode::Influence {
        return;
    }
    let (Some(view), Some(map)) = (overlay.influence(), map) else {
        return;
    };
    let palette = view.palette();
    let lift = |p: Vec2| tile_to_world(p.x, p.y, surface_height_at(&map, p.x, p.y) + LIFT);
    let bright = |v: Vec4| Color::linear_rgba(v.x, v.y, v.z, 1.0).mix(&Color::WHITE, 0.35);
    for dash in &view.dashes {
        let color = match dash.camp {
            Camp::Player => bright(palette.player),
            Camp::Rival => bright(palette.rival),
        };
        gizmos.line(lift(dash.from), lift(dash.to), color);
    }
    for ring in &view.rings {
        // The whole ring, faint: the invitation's distance.
        let track = Ring { fill: 1.0, ..*ring };
        gizmos.linestrip(
            ring_points(&track, RING_SEGMENTS).into_iter().map(lift),
            Color::srgba(1.0, 1.0, 1.0, 0.25),
        );
        let color = match ring.state {
            RingState::Warming => {
                Color::linear_rgba(palette.player.x, palette.player.y, palette.player.z, 0.8)
            }
            RingState::InSphere => bright(palette.player),
            RingState::Envoys => Color::srgb(1.0, 0.85, 0.3),
        };
        if ring.fill > 0.0 {
            gizmos.linestrip(
                ring_points(ring, RING_SEGMENTS).into_iter().map(lift),
                color,
            );
        }
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

    /// UI-12: the influence overlay is appended to the O cycle, after the
    /// monuments, and the cycle closes on Off.
    #[test]
    fn influence_ends_the_cycle() {
        assert_eq!(OverlayMode::Monuments.next(), OverlayMode::Influence);
        assert_eq!(OverlayMode::Influence.next(), OverlayMode::Off);
        assert_eq!(OverlayMode::ALL.last(), Some(&OverlayMode::Influence));
        assert_eq!(OverlayMode::Influence.name(), "influence");
        assert!(OverlayMode::Influence.legend().is_some());
        assert!(OverlayMode::Off.legend().is_none());
        // Every mode has a distinct name for the run bar.
        let mut names: Vec<_> = OverlayMode::ALL.iter().map(|m| m.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), OverlayMode::ALL.len());
    }

    /// An app with the resources `compute_overlay` reads and the towns
    /// `(id, tile, owner, people, influence, goodwill)`.
    fn app_with(towns: &[(u64, IVec2, FactionId, u32, Influence, f32)]) -> App {
        let cal = sim_core::Calendar::default_data();
        let econ = sim_economy::Economy::default_data();
        let mut app = App::new();
        app.init_resource::<Time<Real>>()
            .init_resource::<Overlay>()
            .insert_resource(Army::default_data(&cal, &econ))
            .insert_resource(Progression::default_data())
            .insert_resource(WorldMap::flat(128, 32, sim_world::Terrain::Grass))
            .add_systems(Update, compute_overlay);
        for (id, at, owner, people, influence, goodwill) in towns {
            let mut g = Goodwill::default();
            g.set(FactionId::PLAYER, *goodwill);
            app.world_mut().spawn((
                SpawnId(*id),
                Settlement {
                    name: sim_core::NameId(0),
                    region: 0,
                    center: *at,
                },
                SettlementEconomy {
                    storehouse: Entity::PLACEHOLDER,
                    homes: Vec::new(),
                    workplaces: Vec::new(),
                    population: *people,
                },
                Owner {
                    faction: *owner,
                    since: 0,
                },
                *influence,
                g,
            ));
        }
        app
    }

    /// Runs the overlay system once, now.
    fn recompute(app: &mut App) {
        app.world_mut().resource_mut::<Overlay>().cooldown = 0.0;
        app.update();
    }

    fn held_influence(player: f32, rival: f32, power: f32) -> Influence {
        Influence {
            power,
            player,
            rival,
            envoy: false,
        }
    }

    /// UI-12: in the influence mode the field is computed render-side from
    /// each town's `Influence`, only every two seconds, and terrain is told
    /// to rebuild (the generation moves) only when the tint changes: a ring
    /// filling does not, a stronger town does; leaving the mode clears it.
    #[test]
    fn the_influence_field_is_computed_from_the_towns() {
        let p = FactionId::PLAYER;
        let mut app = app_with(&[
            (
                1,
                IVec2::new(30, 60),
                p,
                300,
                held_influence(0.7, 0.0, 1.3),
                0.8,
            ),
            (
                2,
                IVec2::new(90, 60),
                FactionId::INDEPENDENT,
                120,
                held_influence(0.3, 0.0, 0.0),
                0.1,
            ),
        ]);
        // A short reach, so the far corner of this small map is bare.
        app.world_mut()
            .resource_mut::<Progression>()
            .data
            .influence
            .reach = 40.0;
        recompute(&mut app);
        assert!(
            app.world().resource::<Overlay>().influence().is_none(),
            "not in the mode"
        );
        let g0 = app.world().resource::<Overlay>().generation;

        app.world_mut().resource_mut::<Overlay>().mode = OverlayMode::Influence;
        recompute(&mut app);
        let overlay = app.world().resource::<Overlay>();
        let view = overlay.influence().expect("a field");
        let g1 = overlay.generation;
        assert_ne!(g1, g0, "terrain is told");
        let base = Vec4::new(0.3, 0.3, 0.3, 1.0);
        let here = overlay.tint(31.0, 61.0, base);
        assert!(here.z > here.x, "your colour at your town: {here:?}");
        assert_eq!(overlay.tint(120.0, 120.0, base), base, "bare far away");
        // The warmed town gets a ring: 0.1 of 0.5 filled.
        assert_eq!(view.rings.len(), 1, "{:?}", view.rings);
        assert!((view.rings[0].fill - 0.2).abs() < 0.04, "{:?}", view.rings);

        // Two seconds are two seconds: no recompute before the cooldown.
        let cooldown = app.world().resource::<Overlay>().cooldown;
        assert_eq!(cooldown, INFLUENCE_PERIOD);
        let mut q = app.world_mut().query::<&mut Goodwill>();
        for mut g in q.iter_mut(app.world_mut()) {
            g.set(FactionId::PLAYER, 0.45);
        }
        app.update();
        assert!(
            (app.world().resource::<Overlay>().influence().unwrap().rings[0].fill - 0.2).abs()
                < 0.04,
            "not yet recomputed"
        );

        // Goodwill moves the ring, not the tint.
        recompute(&mut app);
        let overlay = app.world().resource::<Overlay>();
        assert_eq!(overlay.generation, g1, "terrain is not rebuilt for a ring");
        assert!(overlay.influence().unwrap().rings[0].fill > 0.8);

        // A stronger town moves the tint.
        let mut q = app.world_mut().query::<(&SpawnId, &mut Influence)>();
        for (id, mut i) in q.iter_mut(app.world_mut()) {
            if id.0 == 1 {
                i.power = 2.6;
            }
        }
        recompute(&mut app);
        assert_ne!(app.world().resource::<Overlay>().generation, g1);

        // Leaving the mode drops the field.
        let g2 = app.world().resource::<Overlay>().generation;
        app.world_mut().resource_mut::<Overlay>().mode = OverlayMode::Ownership;
        recompute(&mut app);
        let overlay = app.world().resource::<Overlay>();
        assert!(overlay.influence().is_none());
        assert_ne!(overlay.generation, g2);
        assert_eq!(overlay.discs().len(), 2);
    }

    /// Outside a run (no `Influence` anywhere) or with influence switched
    /// off, the mode shows nothing and never moves the generation.
    #[test]
    fn the_influence_mode_shows_nothing_without_influence() {
        let mut app = app_with(&[]);
        app.world_mut().resource_mut::<Overlay>().mode = OverlayMode::Influence;
        recompute(&mut app);
        let overlay = app.world().resource::<Overlay>();
        assert!(overlay.influence().is_none());
        assert_eq!(overlay.generation, 0);

        let mut app = app_with(&[(
            1,
            IVec2::new(30, 60),
            FactionId::PLAYER,
            300,
            held_influence(0.7, 0.0, 1.3),
            0.0,
        )]);
        app.world_mut()
            .resource_mut::<Progression>()
            .data
            .influence
            .enabled = false;
        app.world_mut().resource_mut::<Overlay>().mode = OverlayMode::Influence;
        recompute(&mut app);
        assert!(app.world().resource::<Overlay>().influence().is_none());
        assert_eq!(app.world().resource::<Overlay>().generation, 0);
    }

    /// A ring is drawn to its fill: the first point is at angle 0, the last
    /// where the fill ends, every point at the radius.
    #[test]
    fn rings_are_drawn_to_their_fill() {
        let ring = Ring {
            center: Vec2::new(10.0, 20.0),
            radius: 5.0,
            fill: 0.25,
            state: RingState::Warming,
        };
        let pts = ring_points(&ring, 48);
        assert_eq!(pts.len(), 13);
        assert!(pts[0].distance(Vec2::new(15.0, 20.0)) < 1e-4);
        assert!(
            pts[12].distance(Vec2::new(10.0, 25.0)) < 1e-4,
            "a quarter turn"
        );
        assert!(
            pts.iter()
                .all(|p| (p.distance(ring.center) - 5.0).abs() < 1e-4)
        );
        let full = ring_points(&Ring { fill: 1.0, ..ring }, 48);
        assert_eq!(full.len(), 49);
        assert!(full[48].distance(full[0]) < 1e-3, "closed");
        // An off-grid fill ends exactly at its angle.
        let odd = ring_points(&Ring { fill: 0.3, ..ring }, 48);
        let end = odd.last().unwrap();
        let angle = (end.y - 20.0).atan2(end.x - 10.0);
        assert!((angle - 0.3 * std::f32::consts::TAU).abs() < 1e-3);
    }
}
