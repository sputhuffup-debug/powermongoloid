//! Monument placement (GP-23): `per_region` monuments in each region, on
//! sites that suit their kind, away from settlement centres and from each
//! other. Paints no terrain.

use std::collections::BTreeSet;

use bevy_math::IVec2;
use rand::RngExt;
use sim_core::SimSeed;
use sim_core::rng::rng_key;

use super::fields::distance_to;
use crate::map::{NO_REGION, RegionId, Terrain};
use crate::monuments::{MonumentKind, MonumentSite};
use crate::settings::{MonumentKindRule, SiteRule, WorldGenSettings};

/// Spacing factors tried in turn when no kind fits a slot at full spacing.
const SPACING_RELAX: [f32; 3] = [1.0, 0.5, 0.25];

pub(crate) struct MonumentInputs<'a> {
    pub size: usize,
    /// Final terrain: roads and farmland already painted.
    pub terrain: &'a [Terrain],
    pub tile_region: &'a [RegionId],
    pub water_dist: &'a [u8],
    pub centers: &'a [IVec2],
    /// Settlement names, never reused for a monument.
    pub settlement_names: &'a [String],
}

/// Distance fields a site rule may need, computed only when some rule
/// uses them.
struct Distances {
    road: Option<Vec<u8>>,
    mountain: Option<Vec<u8>>,
}

/// Open ground: the terrain a monument may stand on unless its rule says
/// hills or forest.
fn open(t: Terrain) -> bool {
    matches!(
        t,
        Terrain::Grass | Terrain::Forest | Terrain::Beach | Terrain::Hills
    )
}

/// Whether tile `i` suits `rule`. The base filter (no Water, Road, Mountain
/// or Farmland, in a region, away from settlements) is applied beforehand.
fn allows(rule: SiteRule, inp: &MonumentInputs, d: &Distances, i: usize) -> bool {
    let t = inp.terrain[i];
    let road = |i: usize| d.road.as_ref().map_or(u8::MAX, |r| r[i]);
    match rule {
        SiteRule::NearWater { min, max } => open(t) && (min..=max).contains(&inp.water_dist[i]),
        SiteRule::Hills => t == Terrain::Hills,
        SiteRule::HillsNearMountain { within } => {
            t == Terrain::Hills && d.mountain.as_ref().is_some_and(|m| m[i] <= within)
        }
        SiteRule::NearRoad { min, max } => open(t) && (min..=max).contains(&road(i)),
        SiteRule::ForestEdgeOrRoad => {
            let size = inp.size;
            let (x, y) = (i % size, i / size);
            let forest_edge = t == Terrain::Forest
                && [
                    (x > 0).then(|| i - 1),
                    (x + 1 < size).then(|| i + 1),
                    (y > 0).then(|| i - size),
                    (y + 1 < size).then(|| i + size),
                ]
                .into_iter()
                .flatten()
                .any(|j| {
                    matches!(
                        inp.terrain[j],
                        Terrain::Grass | Terrain::Beach | Terrain::Hills | Terrain::Farmland
                    )
                });
            forest_edge || (open(t) && road(i) == 1)
        }
        SiteRule::GrassOrHills => matches!(t, Terrain::Grass | Terrain::Hills),
    }
}

/// Places monuments region by region (D9 streams `sim_world::gen::monuments
/// ^ region` and `sim_world::gen::monument_names`, tick 0). Returns them in
/// placement order: by region, then by slot.
///
/// Per region, `per_region` kinds are drawn by weight. Each slot then draws
/// an anchor tile among the region's eligible tiles and takes the tile
/// nearest to it that suits the kind and keeps `min_spacing` from every
/// monument already placed (ties to the lowest tile index). If the kind
/// fits nowhere, the kinds after it in the list are tried in turn
/// (wrapping), then the spacing is relaxed to a half and a quarter; a slot
/// nothing fits stays empty.
pub(crate) fn place(
    seed: SimSeed,
    inp: &MonumentInputs,
    s: &WorldGenSettings,
) -> Vec<MonumentSite> {
    let m = &s.monuments;
    if m.per_region == 0 || m.kinds.is_empty() || inp.centers.is_empty() {
        return Vec::new();
    }
    let size = inp.size;
    let n = inp.terrain.len();
    let needs = |f: fn(&SiteRule) -> bool| m.kinds.iter().any(|k| f(&k.site));
    let d = Distances {
        road: needs(|r| matches!(r, SiteRule::NearRoad { .. } | SiteRule::ForestEdgeOrRoad))
            .then(|| distance_to(inp.terrain, size, |t| t == Terrain::Road)),
        mountain: needs(|r| matches!(r, SiteRule::HillsNearMountain { .. }))
            .then(|| distance_to(inp.terrain, size, |t| t == Terrain::Mountain)),
    };

    // Tiles too close to a settlement centre.
    let mut near_settlement = vec![false; n];
    let r = m.min_settlement_distance.max(0.0);
    let ri = r.ceil() as i32;
    for c in inp.centers {
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let p = *c + IVec2::new(dx, dy);
                if p.x >= 0
                    && p.y >= 0
                    && (p.x as usize) < size
                    && (p.y as usize) < size
                    && ((dx * dx + dy * dy) as f32) < r * r
                {
                    near_settlement[p.y as usize * size + p.x as usize] = true;
                }
            }
        }
    }

    // Eligible tiles per region, in index order.
    let regions = inp
        .tile_region
        .iter()
        .filter(|&&r| r != NO_REGION)
        .max()
        .map_or(0, |&r| r as usize + 1);
    let mut eligible: Vec<Vec<usize>> = vec![Vec::new(); regions];
    let tiles = inp
        .tile_region
        .iter()
        .zip(inp.terrain)
        .zip(&near_settlement);
    for (i, ((&reg, &t), &near)) in tiles.enumerate() {
        if reg != NO_REGION
            && !near
            && !matches!(
                t,
                Terrain::Water | Terrain::Road | Terrain::Mountain | Terrain::Farmland
            )
        {
            eligible[reg as usize].push(i);
        }
    }

    let pos = |i: usize| IVec2::new((i % size) as i32, (i / size) as i32);
    let total_weight: f32 = m.kinds.iter().map(|k| k.weight).sum();
    let mut placed: Vec<(MonumentKind, usize, RegionId)> = Vec::new();
    for (reg, tiles) in eligible.iter().enumerate() {
        if tiles.is_empty() {
            continue;
        }
        let mut rng = seed.stream(rng_key("sim_world::gen::monuments") ^ reg as u64, 0);
        let drawn: Vec<usize> = (0..m.per_region)
            .map(|_| pick_weighted(&m.kinds, total_weight, rng.random_range(0.0..1.0)))
            .collect();
        for k in drawn {
            let anchor = pos(tiles[rng.random_range(0..tiles.len())]);
            let found = SPACING_RELAX.iter().find_map(|&f| {
                let spacing = m.min_spacing * f;
                (0..m.kinds.len())
                    .map(|o| (k + o) % m.kinds.len())
                    .find_map(|kk| {
                        nearest(tiles, anchor, size, |i| {
                            allows(m.kinds[kk].site, inp, &d, i)
                                && placed.iter().all(|&(_, j, _)| {
                                    pos(j).as_vec2().distance(pos(i).as_vec2()) >= spacing
                                })
                        })
                        .map(|i| (m.kinds[kk].kind, i))
                    })
            });
            if let Some((kind, i)) = found {
                placed.push((kind, i, reg as RegionId));
            }
        }
    }

    let names = monument_names(seed, s, inp.settlement_names, placed.len());
    placed
        .into_iter()
        .zip(names)
        .map(|((kind, i, region), place)| MonumentSite {
            kind,
            center: pos(i),
            region,
            name: format!("{place} {}", kind.title()),
        })
        .collect()
}

/// The rule index `u` (0..1) falls on, by cumulative weight.
fn pick_weighted(kinds: &[MonumentKindRule], total: f32, u: f32) -> usize {
    let mut x = u * total;
    for (i, k) in kinds.iter().enumerate() {
        if x < k.weight {
            return i;
        }
        x -= k.weight;
    }
    kinds.len() - 1
}

/// The tile in `tiles` nearest to `anchor` that passes `ok`; ties go to the
/// lowest index. Integer distances, so the choice is exact.
fn nearest(
    tiles: &[usize],
    anchor: IVec2,
    size: usize,
    mut ok: impl FnMut(usize) -> bool,
) -> Option<usize> {
    let mut best: Option<(i64, usize)> = None;
    for &i in tiles {
        let p = IVec2::new((i % size) as i32, (i / size) as i32) - anchor;
        let d2 = p.x as i64 * p.x as i64 + p.y as i64 * p.y as i64;
        // `tiles` is in index order, so a strictly smaller distance wins ties.
        if best.is_none_or(|(b, _)| d2 < b) && ok(i) {
            best = Some((d2, i));
        }
    }
    best.map(|(_, i)| i)
}

/// `n` place names for monuments from the settlement syllables, in a seeded
/// order, never one a settlement already has.
fn monument_names(seed: SimSeed, s: &WorldGenSettings, taken: &[String], n: usize) -> Vec<String> {
    let taken: BTreeSet<&str> = taken.iter().map(String::as_str).collect();
    let mut seen = BTreeSet::new();
    let mut all: Vec<String> = Vec::new();
    for p in &s.names.prefixes {
        for x in &s.names.suffixes {
            let name = format!("{p}{x}");
            if !taken.contains(name.as_str()) && seen.insert(name.clone()) {
                all.push(name);
            }
        }
    }
    let mut rng = seed.stream(rng_key("sim_world::gen::monument_names"), 0);
    for i in (1..all.len()).rev() {
        let j = rng.random_range(0..=i);
        all.swap(i, j);
    }
    all.truncate(n);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_pick_covers_every_rule() {
        let rule = |kind, weight| MonumentKindRule {
            kind,
            weight,
            site: SiteRule::Hills,
        };
        let kinds = [
            rule(MonumentKind::StandingStones, 1.0),
            rule(MonumentKind::Watchtower, 3.0),
        ];
        assert_eq!(pick_weighted(&kinds, 4.0, 0.0), 0);
        assert_eq!(pick_weighted(&kinds, 4.0, 0.24), 0);
        assert_eq!(pick_weighted(&kinds, 4.0, 0.26), 1);
        assert_eq!(pick_weighted(&kinds, 4.0, 0.999), 1);
    }

    #[test]
    fn nearest_breaks_ties_on_the_lowest_index() {
        // A 5x5 map; tiles (1,2) and (3,2) are both 1 from the anchor (2,2).
        let tiles = [11, 13, 0];
        assert_eq!(nearest(&tiles, IVec2::new(2, 2), 5, |_| true), Some(11));
        assert_eq!(nearest(&tiles, IVec2::new(2, 2), 5, |i| i != 11), Some(13));
        assert_eq!(nearest(&tiles, IVec2::new(2, 2), 5, |_| false), None);
    }

    #[test]
    fn site_rules_read_the_terrain() {
        // One row: water, beach, grass, road, grass, forest, hills, mountain.
        let terrain = vec![
            Terrain::Water,
            Terrain::Beach,
            Terrain::Grass,
            Terrain::Road,
            Terrain::Grass,
            Terrain::Forest,
            Terrain::Hills,
            Terrain::Mountain,
        ];
        let size = terrain.len();
        // A single row is a size x 1 map; pad it to size x size with water.
        let mut full = terrain.clone();
        full.resize(size * size, Terrain::Water);
        let water: Vec<u8> = super::super::fields::water_distance(&full, size);
        let inp = MonumentInputs {
            size,
            terrain: &full,
            tile_region: &vec![0; size * size],
            water_dist: &water,
            centers: &[],
            settlement_names: &[],
        };
        let d = Distances {
            road: Some(distance_to(&full, size, |t| t == Terrain::Road)),
            mountain: Some(distance_to(&full, size, |t| t == Terrain::Mountain)),
        };
        let ok = |rule, i| allows(rule, &inp, &d, i);
        assert!(ok(SiteRule::NearWater { min: 1, max: 2 }, 1));
        assert!(!ok(SiteRule::NearWater { min: 2, max: 6 }, 1));
        assert!(ok(SiteRule::NearRoad { min: 1, max: 1 }, 2));
        assert!(ok(SiteRule::NearRoad { min: 1, max: 1 }, 4));
        assert!(!ok(SiteRule::NearRoad { min: 2, max: 3 }, 4));
        assert!(ok(SiteRule::Hills, 6));
        assert!(ok(SiteRule::HillsNearMountain { within: 1 }, 6));
        assert!(!ok(SiteRule::HillsNearMountain { within: 1 }, 2));
        assert!(ok(SiteRule::ForestEdgeOrRoad, 5));
        assert!(ok(SiteRule::ForestEdgeOrRoad, 4));
        assert!(ok(SiteRule::GrassOrHills, 2));
        assert!(!ok(SiteRule::GrassOrHills, 5));
    }
}
