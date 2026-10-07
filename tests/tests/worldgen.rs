//! M1 acceptance: a seed always produces the same 2048 x 2048 island with
//! every terrain type, and 40 to 80 settlements on suitable land linked by
//! roads (SIM-01, SIM-02).
//!
//! M4 (GP-21, GP-23): the `run` preset is a 1024-tile island of four
//! regions whose settlements all have a drawn size, the largest being the
//! rival's seat, with monuments in every region away from settlements. The
//! `developed` preset has neither (D6).

use std::collections::BTreeSet;

use bevy_math::IVec2;
use sim_core::{NameTable, SpawnId};
use sim_headless::{HeadlessConfig, headless_app};
use sim_world::worldgen::{GeneratedWorld, MAX_SITE_SLOPE};
use sim_world::{
    Monument, NO_REGION, RoadNetwork, Settlement, SettlementSize, Terrain, WorldGenSettings,
    WorldPreset, generate_world,
};

const SEEDS: [u64; 4] = [42, 7, 1234, 2026];

fn settings() -> WorldGenSettings {
    WorldGenSettings::default_data()
}

fn run_settings() -> WorldGenSettings {
    WorldGenSettings::preset(WorldPreset::Run)
}

fn gen_world(seed: u64, threads: usize) -> GeneratedWorld {
    generate_world(seed, &settings(), threads)
}

#[test]
fn shipped_world_is_2048_square_in_64_tile_chunks() {
    let s = settings();
    assert_eq!((s.size, s.chunk_size), (2048, 64));
}

#[test]
fn same_seed_same_island_regardless_of_threads() {
    let a = gen_world(42, 1);
    let b = gen_world(42, 4);
    let c = gen_world(42, 4);
    assert_eq!(a.map.size(), 2048);
    for other in [&b, &c] {
        assert_eq!(a.map.digest(), other.map.digest(), "map differs");
        assert_eq!(a.settlements, other.settlements, "settlements differ");
        assert_eq!(a.roads, other.roads, "roads differ");
    }
    // D6: the developed world draws no sizes and places no monuments.
    assert!(a.monuments.is_empty());
    assert!(a.settlements.iter().all(|s| s.size.is_none()));
}

#[test]
fn run_preset_is_the_same_regardless_of_threads() {
    let s = run_settings();
    let a = generate_world(42, &s, 1);
    let b = generate_world(42, &s, 4);
    assert_eq!(a.map.size(), 1024);
    assert_eq!(a.map.digest(), b.map.digest(), "map differs");
    assert_eq!(a.settlements, b.settlements, "settlements or sizes differ");
    assert_eq!(a.monuments, b.monuments, "monuments differ");
    assert_eq!(a.roads, b.roads, "roads differ");
    assert!(!a.monuments.is_empty());
}

#[test]
fn different_seeds_give_different_islands() {
    let a = gen_world(42, 4);
    let b = gen_world(43, 4);
    assert_ne!(a.map.digest(), b.map.digest());
}

#[test]
fn every_seed_meets_the_m1_criteria() {
    let s = settings();
    for seed in SEEDS {
        let w = gen_world(seed, 4);
        check_terrain(seed, &w);
        check_settlements(seed, &w, &s);
        check_roads(seed, &w);
    }
}

fn check_terrain(seed: u64, w: &GeneratedWorld) {
    let counts = w.map.terrain_counts();
    for t in Terrain::ALL {
        assert!(counts[t as usize] > 0, "seed {seed}: no {} tiles", t.name());
    }
    // An island: the whole border is sea.
    let n = w.map.size() as i32;
    for i in 0..n {
        for p in [
            IVec2::new(i, 0),
            IVec2::new(i, n - 1),
            IVec2::new(0, i),
            IVec2::new(n - 1, i),
        ] {
            assert_eq!(
                w.map.terrain(p),
                Terrain::Water,
                "seed {seed}: land at border {p}"
            );
        }
    }
}

fn check_settlements(seed: u64, w: &GeneratedWorld, s: &WorldGenSettings) {
    let n = w.settlements.len() as u32;
    assert!(
        (s.settlements.min..=s.settlements.max).contains(&n),
        "seed {seed}: {n} settlements"
    );
    let min_gap = s.scaled(s.settlements.min_spacing_tiles_at_2048) * 0.5;
    for (i, a) in w.settlements.iter().enumerate() {
        let t = w.map.terrain(a.center);
        assert!(
            matches!(t, Terrain::Grass | Terrain::Farmland | Terrain::Road),
            "seed {seed}: {} sits on {}",
            a.name,
            t.name()
        );
        // Flat ground: no neighbour differs by more than the placement limit.
        for d in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let diff = (w.map.height(a.center) - w.map.height(a.center + d)).abs();
            assert!(
                diff <= MAX_SITE_SLOPE,
                "seed {seed}: {} is on a slope",
                a.name
            );
        }
        assert_ne!(a.region, NO_REGION, "seed {seed}: {} has no region", a.name);
        assert_eq!(w.map.region(a.center), a.region);
        for b in &w.settlements[i + 1..] {
            let d = a.center.as_vec2().distance(b.center.as_vec2());
            assert!(
                d >= min_gap,
                "seed {seed}: {} and {} are {d} apart",
                a.name,
                b.name
            );
            assert_ne!(a.name, b.name, "seed {seed}: duplicate name");
        }
    }
}

fn check_roads(seed: u64, w: &GeneratedWorld) {
    let n = w.settlements.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        if p[i] != i {
            let r = find(p, p[i]);
            p[i] = r;
        }
        p[i]
    }
    let mut degree = vec![0; n];
    for road in &w.roads {
        let (a, b) = (&w.settlements[road.a], &w.settlements[road.b]);
        assert_eq!(
            road.path.first(),
            Some(&a.center),
            "seed {seed}: road start"
        );
        assert_eq!(road.path.last(), Some(&b.center), "seed {seed}: road end");
        for step in road.path.windows(2) {
            assert_eq!(
                (step[1] - step[0]).abs().max_element(),
                1,
                "seed {seed}: road gap"
            );
        }
        for p in &road.path {
            assert_eq!(
                w.map.terrain(*p),
                Terrain::Road,
                "seed {seed}: road tile {p} not Road"
            );
        }
        degree[road.a] += 1;
        degree[road.b] += 1;
        let (ra, rb) = (find(&mut parent, road.a), find(&mut parent, road.b));
        parent[ra] = rb;
    }
    for (i, d) in degree.iter().enumerate() {
        assert!(
            *d >= 1,
            "seed {seed}: {} has no road",
            w.settlements[i].name
        );
    }
    let root = find(&mut parent, 0);
    for i in 0..n {
        assert_eq!(
            find(&mut parent, i),
            root,
            "seed {seed}: road network is split"
        );
    }
}

#[test]
fn run_preset_meets_its_criteria() {
    let s = run_settings();
    for seed in SEEDS {
        let w = generate_world(seed, &s, 4);
        check_terrain(seed, &w);
        check_settlements(seed, &w, &s);
        check_roads(seed, &w);
        check_regions(seed, &w, &s);
        check_monuments(seed, &w, &s);
        check_sizes(seed, &w, &s);
    }
}

/// Exactly the requested regions, each with at least one settlement.
fn check_regions(seed: u64, w: &GeneratedWorld, s: &WorldGenSettings) {
    assert_eq!(w.map.region_count(), s.regions as usize, "seed {seed}");
    let used: BTreeSet<u8> = w.settlements.iter().map(|x| x.region).collect();
    assert_eq!(used.len(), s.regions as usize, "seed {seed}: {used:?}");
}

/// GP-23: 4 to 8 monuments per region on allowed ground, outside every
/// village's threat radius, spaced apart, with unique names.
fn check_monuments(seed: u64, w: &GeneratedWorld, s: &WorldGenSettings) {
    let m = &s.monuments;
    let mut per_region = vec![0; w.map.region_count()];
    let mut names = BTreeSet::new();
    let settlement_names: BTreeSet<&str> = w.settlements.iter().map(|x| x.name.as_str()).collect();
    let mut closest_settlement = f32::INFINITY;
    let mut closest_pair = f32::INFINITY;
    for (i, a) in w.monuments.iter().enumerate() {
        per_region[a.region as usize] += 1;
        let t = w.map.terrain(a.center);
        assert!(
            !matches!(t, Terrain::Water | Terrain::Road | Terrain::Mountain),
            "seed {seed}: {} sits on {}",
            a.name,
            t.name()
        );
        assert_eq!(w.map.region(a.center), a.region, "seed {seed}: {}", a.name);
        for x in &w.settlements {
            let d = a.center.as_vec2().distance(x.center.as_vec2());
            closest_settlement = closest_settlement.min(d);
            assert!(
                d >= m.min_settlement_distance - 1e-3 && d > 45.0,
                "seed {seed}: {} is {d} from {}",
                a.name,
                x.name
            );
        }
        for b in &w.monuments[i + 1..] {
            let d = a.center.as_vec2().distance(b.center.as_vec2());
            closest_pair = closest_pair.min(d);
            assert!(
                d >= m.min_spacing - 1e-3,
                "seed {seed}: {} and {} are {d} apart",
                a.name,
                b.name
            );
        }
        assert!(
            names.insert(a.name.clone()),
            "seed {seed}: duplicate {}",
            a.name
        );
        let place = a.name.split(' ').next().unwrap_or_default();
        assert!(
            !settlement_names.contains(place),
            "seed {seed}: {} reuses a settlement name",
            a.name
        );
    }
    let kinds: BTreeSet<_> = w.monuments.iter().map(|a| a.kind).collect();
    println!(
        "seed {seed}: monuments per region {per_region:?}, {} kinds, nearest settlement {closest_settlement:.0}, closest pair {closest_pair:.0}",
        kinds.len()
    );
    for (r, n) in per_region.iter().enumerate() {
        assert!(
            (4..=8).contains(n),
            "seed {seed}: region {r} has {n} monuments"
        );
    }
}

/// GP-21: every settlement has a size within its tier; the seat is the
/// unique largest; the island holds roughly 4,000-5,000 people.
fn check_sizes(seed: u64, w: &GeneratedWorld, s: &WorldGenSettings) {
    let sizes: Vec<SettlementSize> = w
        .settlements
        .iter()
        .map(|x| {
            x.size
                .unwrap_or_else(|| panic!("seed {seed}: {} has no size", x.name))
        })
        .collect();
    let max = sizes.iter().map(|z| z.people).max().expect("settlements");
    assert_eq!(
        sizes.iter().filter(|z| z.people == max).count(),
        1,
        "seed {seed}: the seat must be the unique largest"
    );
    assert!(max >= s.sizes.seat_min, "seed {seed}: seat has {max}");
    for z in sizes.iter().filter(|z| z.people != max) {
        let rule = s
            .sizes
            .tiers
            .iter()
            .find(|r| r.tier == z.tier)
            .expect("tier");
        assert!(
            (rule.min..=rule.max).contains(&z.people),
            "seed {seed}: {z:?}"
        );
    }
    let total: u32 = sizes.iter().map(|z| z.people).sum();
    println!(
        "seed {seed}: {} settlements, {total} people, seat {max}",
        sizes.len()
    );
    assert!(
        (3_500..=5_500).contains(&total),
        "seed {seed}: {total} people"
    );
}

#[test]
fn the_sim_spawns_settlements_and_roads() {
    let s = settings();
    let mut app = headless_app(HeadlessConfig::new(42));
    let world = app.world_mut();
    let mut q = world.query::<(&SpawnId, &Settlement)>();
    let settlements: Vec<(SpawnId, Settlement)> = q.iter(world).map(|(i, s)| (*i, *s)).collect();
    assert!((s.settlements.min..=s.settlements.max).contains(&(settlements.len() as u32)));
    // D6: no monuments and no drawn sizes in the developed world.
    assert_eq!(world.query::<&Monument>().iter(world).count(), 0);
    assert_eq!(world.query::<&SettlementSize>().iter(world).count(), 0);
    let names = world.resource::<NameTable>();
    for (_, s) in &settlements {
        assert!(!names.get(s.name).is_empty());
    }
    let ids: Vec<SpawnId> = settlements.iter().map(|(i, _)| *i).collect();
    let roads = world.resource::<RoadNetwork>();
    assert!(!roads.roads.is_empty());
    for r in &roads.roads {
        assert!(
            ids.contains(&r.a) && ids.contains(&r.b),
            "road to an unknown settlement"
        );
    }
}
