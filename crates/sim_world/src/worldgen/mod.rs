//! World generation (SIM-01, SIM-02): a pure function of the seed, the
//! settings and nothing else. The thread count only affects speed.
//!
//! Stages: noise, classify, settlements, roads, farmland, regions and
//! names, then (M4, run preset only) settlement sizes and monuments.

mod fields;
mod monuments;
mod roads;
mod settle;
mod sizes;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use bevy_math::IVec2;
use rand::RngExt;
use sim_core::SimSeed;
use sim_core::rng::rng_key;

use crate::SettlementSize;
use crate::map::{NO_REGION, RegionId, Terrain, WorldMap};
use crate::monuments::MonumentSite;
use crate::settings::WorldGenSettings;

pub use settle::{MAX_SITE_SLOPE, suitable as suitable_site};
pub use sizes::draw_sizes;

/// A settlement chosen by world generation.
#[derive(Debug, Clone, PartialEq)]
pub struct SettlementSite {
    pub name: String,
    pub center: IVec2,
    pub region: RegionId,
    /// Its drawn size (GP-21); `None` when the preset draws no sizes.
    pub size: Option<SettlementSize>,
}

/// A road between two settlements (indices into `GeneratedWorld::settlements`).
#[derive(Debug, Clone, PartialEq)]
pub struct RoadLink {
    pub a: usize,
    pub b: usize,
    /// Tiles from `a`'s centre to `b`'s centre, each 8-adjacent to the next.
    pub path: Vec<IVec2>,
}

/// A generated world, ready to install: map, settlement sites, monument
/// sites, roads, stage timings.
#[derive(Debug, Clone)]
pub struct GeneratedWorld {
    pub map: WorldMap,
    pub settlements: Vec<SettlementSite>,
    /// Monuments (GP-23), by region then slot; empty when the preset places none.
    pub monuments: Vec<MonumentSite>,
    pub roads: Vec<RoadLink>,
    /// Wall-clock time per stage, for diagnostics only.
    pub timings: Vec<(&'static str, Duration)>,
}

/// Threads to use by default: all available cores.
pub fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Generates the world for `seed` (SIM-01, SIM-02). The result doesn't depend on `threads`.
pub fn generate_world(seed: u64, s: &WorldGenSettings, threads: usize) -> GeneratedWorld {
    let _span = tracing::info_span!("sim_world::generate_world", seed, size = s.size).entered();
    let seed = SimSeed(seed);
    let size = s.size as usize;
    let mut timings = Vec::new();
    let mut clock = Instant::now();
    let mut lap = |name: &'static str, timings: &mut Vec<(&'static str, Duration)>| {
        timings.push((name, clock.elapsed()));
        clock = Instant::now();
    };

    let height = fields::height_field(seed, s, threads);
    let moisture = fields::moisture_field(seed, s, threads);
    lap("noise", &mut timings);

    let fields::Classified {
        mut terrain,
        sea_level,
    } = fields::classify(&height, &moisture, s);
    let slope = fields::slope_field(&height, size, threads);
    let mut fertility = fields::fertility(&height, &moisture, &slope, &terrain, sea_level);
    let water_dist = fields::water_distance(&terrain, size);
    let (landmass, main) = fields::landmasses(&terrain, size);
    lap("classify", &mut timings);

    let site_idx = settle::place(
        &settle::SiteInputs {
            size,
            terrain: &terrain,
            fertility: &fertility,
            slope: &slope,
            water_dist: &water_dist,
            landmass: &landmass,
            main_landmass: main,
        },
        s,
    );
    let centers: Vec<IVec2> = site_idx
        .iter()
        .map(|&i| IVec2::new((i % size) as i32, (i / size) as i32))
        .collect();
    lap("settlements", &mut timings);

    // Village greens: clear the core of each settlement.
    let st = &s.settlements;
    for_each_in_radius(&centers, st.core_radius, size, |i, _| {
        if matches!(terrain[i], Terrain::Forest | Terrain::Beach) {
            terrain[i] = Terrain::Grass;
        }
    });

    let mut roads = Vec::new();
    for (a, b) in roads::links(&centers, s) {
        let margin = Some(s.scaled(s.roads.search_margin_tiles as f32) as i32);
        let path = roads::route(
            &terrain, &height, size, centers[a], centers[b], &s.roads, margin,
        )
        .or_else(|| {
            roads::route(
                &terrain, &height, size, centers[a], centers[b], &s.roads, None,
            )
        });
        if let Some(path) = path {
            for p in &path {
                terrain[p.y as usize * size + p.x as usize] = Terrain::Road;
            }
            roads.push(RoadLink { a, b, path });
        }
    }
    lap("roads", &mut timings);

    // Farmland on fertile grass around each settlement, outside the green.
    // The fertility needed rises towards the edge, so fields follow the soil
    // instead of forming perfect discs.
    let r2 = (st.farmland_radius * st.farmland_radius).max(1) as f32;
    for_each_in_radius(&centers, st.farmland_radius, size, |i, d2| {
        let needed = st.farmland_min_fertility as f32 + st.farmland_falloff as f32 * d2 as f32 / r2;
        if d2 > st.core_radius * st.core_radius
            && terrain[i] == Terrain::Grass
            && fertility[i] as f32 >= needed
        {
            terrain[i] = Terrain::Farmland;
        }
    });
    // Road tiles have no fertility.
    for (f, t) in fertility.iter_mut().zip(&terrain) {
        if *t == Terrain::Road {
            *f = 0;
        }
    }

    let (site_region, region) = regions(&centers, &terrain, size, s.regions);
    let names = settlement_names(seed, s, centers.len());
    lap("regions+names", &mut timings);

    // GP-21: settlement sizes (none in developed, which draws nothing).
    let drawn = draw_sizes(seed, &s.sizes, centers.len());
    lap("sizes", &mut timings);

    // GP-23: monuments, on the final terrain, before the map takes the
    // region array.
    let monuments = monuments::place(
        seed,
        &monuments::MonumentInputs {
            size,
            terrain: &terrain,
            tile_region: &region,
            water_dist: &water_dist,
            centers: &centers,
            settlement_names: &names,
        },
        s,
    );
    lap("monuments", &mut timings);

    let settlements = centers
        .iter()
        .zip(names)
        .zip(&site_region)
        .zip(drawn)
        .map(|(((&center, name), &region), size)| SettlementSite {
            name,
            center,
            region,
            size,
        })
        .collect();
    let map = WorldMap::from_parts(
        s.size,
        s.chunk_size,
        height,
        terrain,
        fertility,
        region,
        sea_level,
    );
    lap("map", &mut timings);
    GeneratedWorld {
        map,
        settlements,
        monuments,
        roads,
        timings,
    }
}

/// Calls `f(index, squared distance)` for tiles within `radius` of each centre.
fn for_each_in_radius(centers: &[IVec2], radius: i32, size: usize, mut f: impl FnMut(usize, i32)) {
    for c in centers {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let d2 = dx * dx + dy * dy;
                let p = *c + IVec2::new(dx, dy);
                if d2 <= radius * radius
                    && p.x >= 0
                    && p.y >= 0
                    && (p.x as usize) < size
                    && (p.y as usize) < size
                {
                    f(p.y as usize * size + p.x as usize, d2);
                }
            }
        }
    }
}

/// Groups settlements into regions around seeds chosen by farthest-point
/// sampling, then floods regions over land from every settlement centre.
/// Returns (region per settlement, region per tile).
fn regions(
    centers: &[IVec2],
    terrain: &[Terrain],
    size: usize,
    count: u32,
) -> (Vec<RegionId>, Vec<RegionId>) {
    let mut tile_region = vec![NO_REGION; terrain.len()];
    if centers.is_empty() {
        return (Vec::new(), tile_region);
    }
    let k = (count as usize)
        .clamp(1, centers.len())
        .min(NO_REGION as usize);
    let v: Vec<_> = centers.iter().map(|c| c.as_vec2()).collect();
    let centroid = v.iter().copied().sum::<bevy_math::Vec2>() / v.len() as f32;
    let first = (0..v.len())
        .min_by(|&a, &b| {
            v[a].distance(centroid)
                .total_cmp(&v[b].distance(centroid))
                .then(a.cmp(&b))
        })
        .expect("non-empty");
    let mut seeds = vec![first];
    while seeds.len() < k {
        let next = (0..v.len())
            .filter(|i| !seeds.contains(i))
            .max_by(|&a, &b| {
                let da = seeds
                    .iter()
                    .map(|&s| v[a].distance(v[s]))
                    .fold(f32::INFINITY, f32::min);
                let db = seeds
                    .iter()
                    .map(|&s| v[b].distance(v[s]))
                    .fold(f32::INFINITY, f32::min);
                da.total_cmp(&db).then(b.cmp(&a))
            })
            .expect("a candidate remains");
        seeds.push(next);
    }
    let site_region: Vec<RegionId> = v
        .iter()
        .map(|p| {
            (0..seeds.len())
                .min_by(|&a, &b| {
                    p.distance(v[seeds[a]])
                        .total_cmp(&p.distance(v[seeds[b]]))
                        .then(a.cmp(&b))
                })
                .expect("seeds non-empty") as RegionId
        })
        .collect();

    let mut queue = VecDeque::new();
    for (c, &r) in centers.iter().zip(&site_region) {
        let i = c.y as usize * size + c.x as usize;
        if tile_region[i] == NO_REGION {
            tile_region[i] = r;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let (x, y) = ((i % size) as i64, (i / size) as i64);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 || nx >= size as i64 || ny >= size as i64 {
                continue;
            }
            let j = ny as usize * size + nx as usize;
            if terrain[j].is_land() && tile_region[j] == NO_REGION {
                tile_region[j] = tile_region[i];
                queue.push_back(j);
            }
        }
    }
    (site_region, tile_region)
}

/// `n` unique names from the syllable lists, in a seeded order.
fn settlement_names(seed: SimSeed, s: &WorldGenSettings, n: usize) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for p in &s.names.prefixes {
        for x in &s.names.suffixes {
            let name = format!("{p}{x}");
            if !all.contains(&name) {
                all.push(name);
            }
        }
    }
    let mut rng = seed.stream(rng_key("sim_world::gen::names"), 0);
    for i in (1..all.len()).rev() {
        let j = rng.random_range(0..=i);
        all.swap(i, j);
    }
    all.truncate(n);
    all
}

/// Stage timings as `name=ms` pairs.
pub fn format_timings(timings: &[(&'static str, Duration)]) -> String {
    timings
        .iter()
        .map(|(n, d)| format!("{n}={:.0}ms", d.as_secs_f64() * 1000.0))
        .collect::<Vec<_>>()
        .join(" ")
}
