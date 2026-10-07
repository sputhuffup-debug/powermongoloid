//! GP-21, GP-27: a run's start draws, a pure function of the seed, the
//! progression data and the generated world (no App, no population), so the
//! same seed always gives the same start and tests can draw starts for many
//! seeds cheaply.
//!
//! Streams (all at tick 0, never shared with any other draw):
//! `sim_military::progression::start` (start spot), `::band`, `::modifiers`,
//! `::traits ^ 0` (the champion's first trait; the champion has no SpawnId
//! yet) and `::boons ^ 0` (the start offer is always offer 0).

use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sim_core::rng::rng_key;
use sim_core::{Position, SimSeed, SpawnId};
use sim_world::{
    GeneratedWorld, Monument, NO_REGION, RegionId, Settlement, SettlementSize, Terrain,
    TravelCosts, WorldMap,
};

use super::{BoonWhen, Progression, StartData};

/// The kind of place a band wakes up in (GP-21). Append only.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(u8)]
pub enum StartTerrain {
    /// By the sea: a beach, or within two tiles of water.
    Coast,
    /// Where forest meets open grass.
    ForestEdge,
    /// In the hills.
    Hills,
    /// None of the above (only when no spot of the drawn kind fits).
    #[default]
    Open,
}

impl StartTerrain {
    /// Every kind, in discriminant order.
    pub const ALL: [StartTerrain; 4] = [
        StartTerrain::Coast,
        StartTerrain::ForestEdge,
        StartTerrain::Hills,
        StartTerrain::Open,
    ];

    /// Lower-case display name.
    pub fn name(self) -> &'static str {
        match self {
            StartTerrain::Coast => "coast",
            StartTerrain::ForestEdge => "forest edge",
            StartTerrain::Hills => "hills",
            StartTerrain::Open => "open ground",
        }
    }

    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A settlement or monument as the start draws see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiteInfo {
    /// Its SpawnId. In a generated world, settlements are numbered first,
    /// then monuments, in their order in [`GeneratedWorld`], exactly as
    /// `install_world` numbers them.
    pub id: u64,
    /// Centre tile.
    pub center: IVec2,
    /// Its region.
    pub region: RegionId,
    /// Drawn population (settlements with a size); 0 otherwise.
    pub people: u32,
}

/// What the start draws read: the map, walking costs, and settlements and
/// monuments in SpawnId order.
#[derive(Debug, Clone)]
pub struct RunWorld<'a> {
    /// The terrain and regions.
    pub map: &'a WorldMap,
    /// Villagers' walking costs (the band must be able to reach the seat).
    pub costs: &'a TravelCosts,
    /// Settlements, by SpawnId.
    pub settlements: Vec<SiteInfo>,
    /// Monuments, by SpawnId.
    pub monuments: Vec<SiteInfo>,
}

impl<'a> RunWorld<'a> {
    /// From a generated world, before it is installed (world-level tests).
    pub fn from_generated(g: &'a GeneratedWorld, costs: &'a TravelCosts) -> Self {
        let n = g.settlements.len() as u64;
        Self {
            map: &g.map,
            costs,
            settlements: g
                .settlements
                .iter()
                .enumerate()
                .map(|(i, s)| SiteInfo {
                    id: i as u64,
                    center: s.center,
                    region: s.region,
                    people: s.size.map_or(0, |z| z.people),
                })
                .collect(),
            monuments: g
                .monuments
                .iter()
                .enumerate()
                .map(|(j, m)| SiteInfo {
                    id: n + j as u64,
                    center: m.center,
                    region: m.region,
                    people: 0,
                })
                .collect(),
        }
    }

    /// From an installed world. `None` if there is no map.
    pub fn from_world(world: &'a World, costs: &'a TravelCosts) -> Option<Self> {
        let map = world.get_resource::<WorldMap>()?;
        let mut settlements: Vec<SiteInfo> = world
            .try_query::<(Entity, &SpawnId, &Settlement)>()
            .map(|mut q| {
                q.iter(world)
                    .map(|(e, id, s)| SiteInfo {
                        id: id.0,
                        center: s.center,
                        region: s.region,
                        people: world.get::<SettlementSize>(e).map_or(0, |z| z.people),
                    })
                    .collect()
            })
            .unwrap_or_default();
        settlements.sort_unstable_by_key(|s| s.id);
        let mut monuments: Vec<SiteInfo> = world
            .try_query::<(&SpawnId, &Monument, &Position)>()
            .map(|mut q| {
                q.iter(world)
                    .map(|(id, m, p)| SiteInfo {
                        id: id.0,
                        center: p.0,
                        region: m.region,
                        people: 0,
                    })
                    .collect()
            })
            .unwrap_or_default();
        monuments.sort_unstable_by_key(|m| m.id);
        Some(Self {
            map,
            costs,
            settlements,
            monuments,
        })
    }
}

/// Everything a run draws at its start (GP-21, GP-27).
#[derive(Debug, Clone, PartialEq)]
pub struct StartDraws {
    /// The rival's seat: the most populous settlement (SpawnId).
    pub seat: u64,
    /// The seat's centre.
    pub seat_center: IVec2,
    /// The seat's region.
    pub rival_region: RegionId,
    /// The region the band starts in: the one whose settlements lie
    /// farthest from the seat.
    pub start_region: RegionId,
    /// The start spot.
    pub start: IVec2,
    /// Its kind.
    pub terrain: StartTerrain,
    /// Monuments within `start.monument_radius` of the start (SpawnIds).
    pub nearby_monuments: Vec<u64>,
    /// The band template (index into `bands`).
    pub band: u16,
    /// The champion's first trait (trait id).
    pub champion_trait: u16,
    /// Run modifier cards, in draw order.
    pub modifiers: Vec<u16>,
    /// The three start boons on offer (boon cards), distinct.
    pub boon_choices: [u16; 3],
}

/// Picks an index with probability proportional to its weight. `None` if
/// no weight is positive.
pub(crate) fn pick_weighted<R: rand::Rng + ?Sized>(rng: &mut R, weights: &[f32]) -> Option<usize> {
    let total: f32 = weights.iter().filter(|w| **w > 0.0).sum();
    if total.is_nan() || total <= 0.0 {
        return None;
    }
    let mut x = rng.random_range(0.0..total);
    let mut last = None;
    for (i, &w) in weights.iter().enumerate() {
        if w <= 0.0 {
            continue;
        }
        if x < w {
            return Some(i);
        }
        x -= w;
        last = Some(i);
    }
    last
}

/// Draws up to `n` distinct cards by weight, without replacement. A card
/// with a group removes every other card of that group from the draw.
pub(crate) fn draw_cards<R: rand::Rng + ?Sized>(
    rng: &mut R,
    cards: &[(u16, f32, Option<&str>)],
    n: usize,
) -> Vec<u16> {
    let mut pool: Vec<(u16, f32, Option<&str>)> = cards.to_vec();
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let weights: Vec<f32> = pool.iter().map(|c| c.1).collect();
        let Some(i) = pick_weighted(rng, &weights) else {
            break;
        };
        let (card, _, group) = pool.remove(i);
        if let Some(g) = group {
            pool.retain(|c| c.2 != Some(g));
        }
        out.push(card);
    }
    out
}

/// Tiles reachable on foot from `from` (8-connected, like `find_path`).
fn reachable(map: &WorldMap, costs: &TravelCosts, from: IVec2) -> Vec<bool> {
    let size = map.size() as usize;
    let mut seen = vec![false; size * size];
    let passable = |p: IVec2| costs.per_terrain[map.terrain(p) as usize].is_some();
    if !map.in_bounds(from) || !passable(from) {
        return seen;
    }
    seen[map.index(from)] = true;
    let mut queue = VecDeque::from([from]);
    while let Some(p) = queue.pop_front() {
        for (dx, dy) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ] {
            let q = p + IVec2::new(dx, dy);
            if map.in_bounds(q) && !seen[map.index(q)] && passable(q) {
                seen[map.index(q)] = true;
                queue.push_back(q);
            }
        }
    }
    seen
}

/// Grows a row-major mask by `r` tiles in every direction (a square).
fn dilate(mask: &[bool], size: usize, r: usize) -> Vec<bool> {
    let window = |line: &mut dyn FnMut(usize) -> bool, out: &mut dyn FnMut(usize, bool)| {
        let mut prefix = vec![0u32; size + 1];
        for i in 0..size {
            prefix[i + 1] = prefix[i] + line(i) as u32;
        }
        for i in 0..size {
            let (lo, hi) = (i.saturating_sub(r), (i + r).min(size - 1));
            out(i, prefix[hi + 1] > prefix[lo]);
        }
    };
    let mut rows = vec![false; mask.len()];
    for y in 0..size {
        window(&mut |x| mask[y * size + x], &mut |x, v| {
            rows[y * size + x] = v
        });
    }
    let mut out = vec![false; mask.len()];
    for x in 0..size {
        window(&mut |y| rows[y * size + x], &mut |y, v| {
            out[y * size + x] = v
        });
    }
    out
}

/// A tile the band could start on, and how well it fits.
#[derive(Debug, Clone, Copy)]
struct Spot {
    tile: IVec2,
    index: usize,
    /// `StartTerrain` bits.
    kinds: u8,
    /// Monuments within the radius.
    monuments: u32,
    /// On the far side of the start region from the seat.
    far: bool,
    /// Near the start region's edge.
    edge: bool,
}

impl Spot {
    /// Lower is better: monuments in range first (the acceptance criterion),
    /// then away from the rival, then the drawn kind, then the region's edge.
    fn rank(&self, s: &StartData, kind: StartTerrain) -> (u8, bool, bool, bool) {
        let m = self.monuments;
        let monuments = if (s.min_monuments..=s.max_monuments).contains(&m) {
            0
        } else if m > s.max_monuments {
            1
        } else if m > 0 {
            2
        } else {
            3
        };
        (
            monuments,
            !self.far,
            self.kinds & kind.bit() == 0,
            !self.edge,
        )
    }

    fn terrain(&self, drawn: StartTerrain) -> StartTerrain {
        if self.kinds & drawn.bit() != 0 {
            return drawn;
        }
        StartTerrain::ALL
            .into_iter()
            .find(|k| self.kinds & k.bit() != 0)
            .unwrap_or(StartTerrain::Open)
    }
}

/// Region-independent tile masks for [`spots`], computed once per draw.
struct Masks {
    /// Reachable on foot from the seat.
    reach: Vec<bool>,
    /// Within two tiles of water.
    near_water: Vec<bool>,
}

impl Masks {
    fn new(w: &RunWorld, seat: IVec2) -> Self {
        let map = w.map;
        let size = map.size() as usize;
        let water: Vec<bool> = map
            .terrain_slice()
            .iter()
            .map(|t| *t == Terrain::Water)
            .collect();
        Self {
            reach: reachable(map, w.costs, seat),
            near_water: dilate(&water, size, 2),
        }
    }
}

/// Every tile of `region` the band could start on: walkable open ground
/// (not road, farmland or mountain), reachable from the seat, and at least
/// `min_settlement_distance` from every settlement centre.
fn spots(
    w: &RunWorld,
    s: &StartData,
    masks: &Masks,
    region: RegionId,
    seat: IVec2,
    start_c: Vec2,
) -> Vec<Spot> {
    let map = w.map;
    let size = map.size() as usize;
    let (reach, near_water) = (&masks.reach, &masks.near_water);
    let edge = {
        let mask: Vec<bool> = (0..size * size)
            .map(|i| {
                let p = map.pos(i);
                map.region(p) == region && map.is_region_edge(p)
            })
            .collect();
        dilate(&mask, size, s.edge_distance as usize)
    };
    let terrain_at = |p: IVec2| map.in_bounds(p).then(|| map.terrain(p));
    let neighbour = |p: IVec2, f: &dyn Fn(Terrain) -> bool| {
        [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y]
            .into_iter()
            .any(|d| terrain_at(p + d).is_some_and(f))
    };
    let toward = seat.as_vec2() - start_c;
    let min_d2 = s.min_settlement_distance * s.min_settlement_distance;
    let r2 = s.monument_radius * s.monument_radius;
    let mut out = Vec::new();
    for (index, &t) in map.terrain_slice().iter().enumerate() {
        if !matches!(
            t,
            Terrain::Grass | Terrain::Beach | Terrain::Forest | Terrain::Hills
        ) || w.costs.per_terrain[t as usize].is_none()
            || !reach[index]
        {
            continue;
        }
        let tile = map.pos(index);
        if map.region(tile) != region {
            continue;
        }
        let p = tile.as_vec2();
        if w.settlements
            .iter()
            .any(|st| st.center.as_vec2().distance_squared(p) < min_d2)
        {
            continue;
        }
        let mut kinds = 0;
        if t == Terrain::Beach || near_water[index] {
            kinds |= StartTerrain::Coast.bit();
        }
        let open = |n: Terrain| {
            matches!(
                n,
                Terrain::Grass | Terrain::Beach | Terrain::Hills | Terrain::Farmland
            )
        };
        if (t == Terrain::Forest && neighbour(tile, &open))
            || (t == Terrain::Grass && neighbour(tile, &|n| n == Terrain::Forest))
        {
            kinds |= StartTerrain::ForestEdge.bit();
        }
        if t == Terrain::Hills {
            kinds |= StartTerrain::Hills.bit();
        }
        let monuments = w
            .monuments
            .iter()
            .filter(|m| m.center.as_vec2().distance_squared(p) <= r2)
            .count() as u32;
        out.push(Spot {
            tile,
            index,
            kinds,
            monuments,
            far: toward.dot(p - start_c) < 0.0,
            edge: edge[index],
        });
    }
    out
}

/// GP-21, GP-27: draws a run's start from `seed` (see the module docs for
/// the streams). `None` if the world has no settlements or no start tile.
///
/// 1. The rival's seat is the most populous settlement (ties: lowest id);
///    its region is the rival's.
/// 2. The other regions are ranked by how far their settlements' centroid
///    lies from the seat (farthest first, ties: lowest id).
/// 3. A start kind is drawn by `terrain_weights`. A region's possible start
///    tiles are ranked by class: `min..=max` monuments within
///    `monument_radius`, then on the far side of the region from the seat,
///    then of the drawn kind, then near the region's edge. The start region
///    is the farthest region whose best class has `min..=max` monuments
///    (else the farthest region). Within its best class, the tile farthest
///    from the seat plus a seeded `jitter` wins (ties: lowest tile index).
/// 4. The band template, the champion's trait, the run modifiers (by
///    weight, no two of one `group`) and three distinct Start boons (by
///    weight) are drawn, each from its own stream.
pub fn draw_start(seed: SimSeed, data: &Progression, w: &RunWorld) -> Option<StartDraws> {
    let _span = tracing::info_span!("sim_military::draw_start").entered();
    let d = &data.data;
    let s = &d.start;
    let seat = *w
        .settlements
        .iter()
        .max_by(|a, b| a.people.cmp(&b.people).then(b.id.cmp(&a.id)))?;
    let seat_c = seat.center.as_vec2();

    // Region centroids, by RegionId.
    let regions = w
        .settlements
        .iter()
        .filter(|st| st.region != NO_REGION)
        .map(|st| st.region as usize + 1)
        .max()
        .unwrap_or(0);
    let mut sums = vec![(Vec2::ZERO, 0u32); regions];
    for st in w.settlements.iter().filter(|st| st.region != NO_REGION) {
        sums[st.region as usize].0 += st.center.as_vec2();
        sums[st.region as usize].1 += 1;
    }
    let centroid = |r: usize| (sums[r].1 > 0).then(|| sums[r].0 / sums[r].1 as f32);
    // Other regions, farthest from the seat first (ties: lowest id); the
    // seat's own region only when there is no other.
    let mut order: Vec<(f32, RegionId, Vec2)> = (0..regions)
        .filter(|&r| r != seat.region as usize)
        .filter_map(|r| centroid(r).map(|c| (c.distance(seat_c), r as RegionId, c)))
        .collect();
    order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    if order.is_empty() {
        let c = (seat.region != NO_REGION)
            .then(|| centroid(seat.region as usize))
            .flatten()
            .unwrap_or(seat_c);
        order.push((0.0, seat.region, c));
    }

    // The start spot: in the farthest region with a spot that has
    // `min..=max` monuments nearby, else in the farthest region.
    let mut rng = seed.stream(rng_key("sim_military::progression::start"), 0);
    let weights: Vec<f32> = s.terrain_weights.iter().map(|(_, w)| *w).collect();
    let kind = s.terrain_weights[pick_weighted(&mut rng, &weights)?].0;
    let masks = Masks::new(w, seat.center);
    let mut fallback = None;
    let mut found = None;
    for &(_, region, c) in &order {
        let spots = spots(w, s, &masks, region, seat.center, c);
        let Some(best) = spots.iter().map(|sp| sp.rank(s, kind)).min() else {
            continue;
        };
        if best.0 == 0 {
            found = Some((region, best, spots));
            break;
        }
        if fallback.is_none() {
            fallback = Some((region, best, spots));
        }
    }
    let (start_region, best, spots) = found.or(fallback)?;
    let mut scored: Vec<(f32, usize, Spot)> = spots
        .into_iter()
        .filter(|sp| sp.rank(s, kind) == best)
        .map(|sp| {
            let jitter = s.jitter * rng.random::<f32>();
            (sp.tile.as_vec2().distance(seat_c) + jitter, sp.index, sp)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let spot = scored.first()?.2;
    let r2 = s.monument_radius * s.monument_radius;
    let nearby_monuments = w
        .monuments
        .iter()
        .filter(|m| m.center.as_vec2().distance_squared(spot.tile.as_vec2()) <= r2)
        .map(|m| m.id)
        .collect();

    // The band, the champion's trait, run modifiers, start boons.
    let mut rng = seed.stream(rng_key("sim_military::progression::band"), 0);
    let band = rng.random_range(0..d.bands.len()) as u16;
    let mut rng = seed.stream(rng_key("sim_military::progression::traits"), 0);
    let champion_trait = d.champion_traits[rng.random_range(0..d.champion_traits.len())];
    let mut rng = seed.stream(rng_key("sim_military::progression::modifiers"), 0);
    let (lo, hi) = d.modifiers_per_run;
    let count = rng.random_range(lo..=hi) as usize;
    let cards: Vec<(u16, f32, Option<&str>)> = d
        .modifiers
        .iter()
        .map(|m| (m.id, m.weight, m.group.as_deref()))
        .collect();
    let modifiers = draw_cards(&mut rng, &cards, count);
    let mut rng = seed.stream(rng_key("sim_military::progression::boons"), 0);
    let cards: Vec<(u16, f32, Option<&str>)> = d
        .boons
        .iter()
        .filter(|b| b.when == BoonWhen::Start)
        .map(|b| (b.id, b.weight, None))
        .collect();
    let boons = draw_cards(&mut rng, &cards, 3);
    let boon_choices: [u16; 3] = boons.try_into().ok()?;

    Some(StartDraws {
        seat: seat.id,
        seat_center: seat.center,
        rival_region: seat.region,
        start_region,
        start: spot.tile,
        terrain: spot.terrain(kind),
        nearby_monuments,
        band,
        champion_trait,
        modifiers,
        boon_choices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_world::{WorldGenSettings, WorldPreset, generate_world, worldgen::default_threads};

    #[test]
    fn weighted_draws_skip_zero_weights_and_groups() {
        let mut rng = SimSeed(1).stream(rng_key("test"), 0);
        for _ in 0..200 {
            let i = pick_weighted(&mut rng, &[0.0, 1.0, 0.0, 2.0]).expect("some");
            assert!(i == 1 || i == 3);
        }
        assert_eq!(pick_weighted(&mut rng, &[0.0, 0.0]), None);
        let cards = [
            (0, 1.0, Some("harvest")),
            (1, 1.0, Some("harvest")),
            (2, 1.0, None),
        ];
        for _ in 0..50 {
            let drawn = draw_cards(&mut rng, &cards, 3);
            assert_eq!(drawn.len(), 2, "a group gives at most one card");
            assert!(drawn.contains(&2));
        }
    }

    #[test]
    fn dilate_grows_a_square() {
        let size = 7;
        let mut mask = vec![false; size * size];
        mask[3 * size + 3] = true;
        let out = dilate(&mask, size, 2);
        let count = out.iter().filter(|b| **b).count();
        assert_eq!(count, 25);
        assert!(out[size + 1] && !out[0] && !out[3 * size + 6]);
    }

    /// GP-21 on the shipped run preset: the start is seeded, away from the
    /// rival, near 2-3 monuments, and offers three distinct start boons.
    #[test]
    fn a_run_start_is_seeded_and_fits() {
        let data = Progression::default_data();
        let costs =
            sim_agents::Villagers::default_data(&sim_core::Calendar::default_data()).travel_costs;
        let settings = WorldGenSettings::preset(WorldPreset::Run);
        let g = generate_world(42, &settings, default_threads());
        let w = RunWorld::from_generated(&g, &costs);
        let a = draw_start(SimSeed(42), &data, &w).expect("a start");
        let b = draw_start(SimSeed(42), &data, &w).expect("a start");
        assert_eq!(a, b, "same seed, same start");
        let seat = w.settlements.iter().find(|s| s.id == a.seat).unwrap();
        assert!(w.settlements.iter().all(|s| s.people <= seat.people));
        assert_ne!(a.start_region, a.rival_region);
        assert_eq!(g.map.region(a.start), a.start_region);
        let m = a.nearby_monuments.len() as u32;
        assert!(
            (data.data.start.min_monuments..=data.data.start.max_monuments).contains(&m),
            "{m} monuments near the start"
        );
        let mut boons = a.boon_choices.to_vec();
        boons.sort_unstable();
        boons.dedup();
        assert_eq!(boons.len(), 3);
        for c in a.boon_choices {
            assert_eq!(data.boon(c).unwrap().when, BoonWhen::Start);
        }
        let (lo, hi) = data.data.modifiers_per_run;
        assert!((lo as usize..=hi as usize).contains(&a.modifiers.len()));
        let path = sim_world::find_path(
            g.map.terrain_slice(),
            g.map.height_slice(),
            g.map.size() as usize,
            a.start,
            a.seat_center,
            &costs,
            None,
        );
        assert!(path.is_some(), "the band can walk to the seat");
    }
}

#[cfg(test)]
mod survey {
    use super::*;
    use sim_world::{WorldGenSettings, WorldPreset, generate_world, worldgen::default_threads};

    #[test]
    #[ignore]
    fn survey_twenty_seeds() {
        let data = Progression::default_data();
        let costs =
            sim_agents::Villagers::default_data(&sim_core::Calendar::default_data()).travel_costs;
        let settings = WorldGenSettings::preset(WorldPreset::Run);
        let seeds: Vec<u64> = std::env::var("SURVEY_SEEDS")
            .ok()
            .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
            .unwrap_or_else(|| (1..=20).collect());
        for seed in seeds {
            let g = generate_world(seed, &settings, default_threads());
            let w = RunWorld::from_generated(&g, &costs);
            let t = std::time::Instant::now();
            let a = draw_start(SimSeed(seed), &data, &w).expect("a start");
            let spots = {
                let s = &data.data.start;
                let seat = w.settlements.iter().find(|x| x.id == a.seat).unwrap();
                let mut sums = (Vec2::ZERO, 0);
                for st in w.settlements.iter().filter(|x| x.region == a.start_region) {
                    sums.0 += st.center.as_vec2();
                    sums.1 += 1;
                }
                let c = sums.0 / sums.1 as f32;
                spots(
                    &w,
                    s,
                    &Masks::new(&w, seat.center),
                    a.start_region,
                    seat.center,
                    c,
                )
            };
            let best_classes: std::collections::BTreeMap<(u8, bool, bool, bool), usize> = {
                let mut m = std::collections::BTreeMap::new();
                for sp in &spots {
                    *m.entry(sp.rank(&data.data.start, a.terrain)).or_insert(0) += 1;
                }
                m
            };
            let dist = (a.start - a.seat_center).as_vec2().length();
            println!(
                "seed {seed:>2}: {:?} start {:?} r{} rival r{} dist {:.0} mons {:?} terrain {} band {} trait {} mods {:?} boons {:?} spots {} first classes {:?} ({:.0} ms)",
                a.seat,
                a.start,
                a.start_region,
                a.rival_region,
                dist,
                a.nearby_monuments,
                a.terrain.name(),
                a.band,
                a.champion_trait,
                a.modifiers,
                a.boon_choices,
                spots.len(),
                best_classes.iter().take(6).collect::<Vec<_>>(),
                t.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}
