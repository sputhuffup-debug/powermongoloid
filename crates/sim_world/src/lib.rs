//! The world: terrain, world generation, settlements and roads (M1), paths
//! and the spatial grid (M2), and flow fields for crowds (M3).
//!
//! # Main pieces
//! - [`WorldMap`]: terrain, height, fertility and region arrays in 64-tile
//!   chunks. Change tiles only through [`WorldMap::set_terrain`], which bumps
//!   chunk revisions (for the renderer) and keeps the hash current.
//! - [`generate_world`] and [`WorldGenSettings`]: deterministic generation
//!   from the seed (`worldgen.ron`, ADR 0005).
//! - [`Settlement`] and [`RoadNetwork`].
//! - Presets (M4, GP-21): `worldgen.ron` holds a `developed` and a `run`
//!   [`WorldPreset`]. The run island draws a [`SettlementSize`] for every
//!   settlement and places [`Monument`]s (GP-23); developed has neither.
//! - [`find_path`] and [`PathCache`]: integer A* for villagers.
//! - [`FlowField`] and [`FlowCache`]: Dijkstra fields that armies descend
//!   (ADR 0011).
//! - [`SpatialGrid`]: moving entities in 8-tile cells, rebuilt in
//!   Bookkeeping.
//!
//! The caches and the grid are pure functions of hashed state, so they are
//! exempt from the hash. Never depends on rendering, windowing, audio or
//! asset crates.

pub mod flow;
pub mod map;
pub mod monuments;
pub mod noise;
pub mod path;
pub mod settings;
pub mod spatial;
pub mod worldgen;

use std::time::Duration;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::{AppHashExt, NameId, NameTable, NextSpawnId, Position, SimSeed, SimSet, SpawnId};
use sim_core::{StateHash, StateHasher};

pub use flow::{FlowCache, FlowField, FlowKey};
pub use map::{NO_REGION, RegionId, Terrain, WorldMap};
pub use monuments::{Monument, MonumentKind, MonumentSite};
pub use path::{PathCache, SharedPath, TravelCosts, find_path};
pub use settings::{WorldGenPresets, WorldGenSettings, WorldPreset};
pub use spatial::{Mobile, SpatialGrid, world_pos};
pub use worldgen::{GeneratedWorld, draw_sizes, generate_world};

/// A settlement (SEED.md "Core data model").
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settlement {
    pub name: NameId,
    pub region: RegionId,
    pub center: IVec2,
}

impl StateHash for Settlement {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.name.0);
        h.u8(self.region);
        h.ivec2(self.center);
    }
}

/// A settlement's size tier (GP-21), drawn by world generation in the `run`
/// preset. Append only: saves and the state hash store the variant index.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[repr(u8)]
pub enum SizeTier {
    /// A few households (20-60 people in the run preset).
    Hamlet = 0,
    /// A village (60-200).
    Village = 1,
    /// A town (200-400); the rival's seat is the largest town.
    Town = 2,
}

impl SizeTier {
    /// Every tier, smallest first.
    pub const ALL: [SizeTier; 3] = [SizeTier::Hamlet, SizeTier::Village, SizeTier::Town];

    /// Lower-case display name.
    pub fn name(self) -> &'static str {
        match self {
            SizeTier::Hamlet => "hamlet",
            SizeTier::Village => "village",
            SizeTier::Town => "town",
        }
    }
}

/// A settlement's drawn size (GP-21). Inserted on the settlement entity only
/// when world generation drew sizes (the `run` preset); population then
/// fills every settlement to `people` instead of apportioning one region.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettlementSize {
    /// The tier it was drawn from.
    pub tier: SizeTier,
    /// People to populate it with.
    pub people: u32,
}

impl StateHash for SettlementSize {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.tier as u8);
        h.u32(self.people);
    }
}

/// A road between two settlements, by stable ID.
#[derive(Debug, Clone, PartialEq)]
pub struct Road {
    pub a: SpawnId,
    pub b: SpawnId,
    pub path: Vec<IVec2>,
}

/// Every road built by world generation (used by carters in M4, SIM-23).
#[derive(Resource, Debug, Clone, Default)]
pub struct RoadNetwork {
    pub roads: Vec<Road>,
}

impl StateHash for RoadNetwork {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.roads.len() as u64);
        for r in &self.roads {
            h.u64(r.a.0);
            h.u64(r.b.0);
            h.u64(r.path.len() as u64);
            for p in &r.path {
                h.ivec2(*p);
            }
        }
    }
}

/// Generation diagnostics for the debug panel. Not simulation state.
#[derive(Resource, Debug, Clone, Default)]
pub struct WorldGenReport {
    pub total: Duration,
    pub stages: Vec<(&'static str, Duration)>,
}

/// Generates the world from [`SimSeed`] when the app finishes building, so
/// it exists before the first tick in both the game and headless runs.
pub struct WorldPlugin {
    pub settings: WorldGenSettings,
    /// Worker threads for generation; does not affect the result.
    pub threads: usize,
}

impl Default for WorldPlugin {
    fn default() -> Self {
        Self {
            settings: WorldGenSettings::default_data(),
            threads: worldgen::default_threads(),
        }
    }
}

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.settings.clone())
            .init_resource::<RoadNetwork>()
            .init_resource::<WorldGenReport>()
            .register_hashed_resource::<WorldGenSettings>()
            .register_hashed_resource::<WorldMap>()
            .register_hashed_resource::<RoadNetwork>()
            .register_hashed::<Settlement>()
            .register_hashed::<SettlementSize>()
            .register_hashed::<Monument>()
            .init_resource::<PathCache>()
            .init_resource::<SpatialGrid>()
            .register_hashed::<Mobile>()
            .exempt_from_hash::<WorldGenReport>("wall-clock diagnostics")
            .exempt_from_hash::<PathCache>("memoised pure function of the map and endpoints")
            .init_resource::<FlowCache>()
            .exempt_from_hash::<FlowCache>("memoised pure function of the map, goal and window")
            .exempt_from_hash::<SpatialGrid>("rebuilt from positions every tick")
            .add_systems(
                FixedUpdate,
                (map::refresh_chunk_digests, spatial::rebuild_grid).in_set(SimSet::Bookkeeping),
            );
    }

    fn finish(&self, app: &mut App) {
        let seed = app.world().resource::<SimSeed>().0;
        let start = std::time::Instant::now();
        let generated = generate_world(seed, &self.settings, self.threads);
        let total = start.elapsed();
        install_world(app.world_mut(), generated, total);
    }
}

/// Inserts a generated world: the map, settlement entities (with their
/// [`SettlementSize`] when one was drawn), then monument entities, and
/// roads. Settlements always get the first SpawnIds, so a world without
/// monuments or sizes installs exactly as before M4 (D6).
pub fn install_world(world: &mut World, generated: GeneratedWorld, total: Duration) {
    let GeneratedWorld {
        map,
        settlements,
        monuments,
        roads,
        timings,
    } = generated;
    let mut ids = Vec::with_capacity(settlements.len());
    for site in settlements {
        let name = world.resource_mut::<NameTable>().push(site.name);
        let id = world.resource_mut::<NextSpawnId>().allocate();
        let mut e = world.spawn((
            id,
            Settlement {
                name,
                region: site.region,
                center: site.center,
            },
            Position(site.center),
        ));
        if let Some(size) = site.size {
            e.insert(size);
        }
        ids.push(id);
    }
    // GP-23: monuments stand still, so no Mobile; sim_military adds their
    // holder and guards when a run is set up.
    for site in monuments {
        let name = world.resource_mut::<NameTable>().push(site.name);
        let id = world.resource_mut::<NextSpawnId>().allocate();
        world.spawn((
            id,
            Monument {
                kind: site.kind,
                region: site.region,
                name,
            },
            Position(site.center),
        ));
    }
    let roads = roads
        .into_iter()
        .map(|r| Road {
            a: ids[r.a],
            b: ids[r.b],
            path: r.path,
        })
        .collect();
    world.insert_resource(map);
    world.insert_resource(RoadNetwork { roads });
    world.insert_resource(WorldGenReport {
        total,
        stages: timings,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::SettlementSite;

    #[test]
    fn install_spawns_settlements_then_monuments() {
        let site = |name: &str, x, size| SettlementSite {
            name: name.into(),
            center: IVec2::new(x, 10),
            region: 0,
            size,
        };
        let town = SettlementSize {
            tier: SizeTier::Town,
            people: 300,
        };
        let generated = GeneratedWorld {
            map: WorldMap::flat(64, 64, Terrain::Grass),
            settlements: vec![site("Ashby", 5, Some(town)), site("Elmford", 30, None)],
            monuments: vec![MonumentSite {
                kind: MonumentKind::Watchtower,
                center: IVec2::new(50, 50),
                region: 0,
                name: "Kestmoor Watchtower".into(),
            }],
            roads: Vec::new(),
            timings: Vec::new(),
        };
        let mut world = World::new();
        world.init_resource::<NameTable>();
        world.init_resource::<NextSpawnId>();
        install_world(&mut world, generated, Duration::ZERO);

        let mut q = world.query::<(&SpawnId, &Settlement, Option<&SettlementSize>)>();
        let mut settlements: Vec<_> = q
            .iter(&world)
            .map(|(i, s, z)| (*i, *s, z.copied()))
            .collect();
        settlements.sort_by_key(|(i, _, _)| *i);
        assert_eq!(settlements.len(), 2);
        assert_eq!(settlements[0].0, SpawnId(0));
        assert_eq!(settlements[0].2, Some(town));
        assert_eq!(settlements[1].2, None);

        let mut q = world.query_filtered::<(&SpawnId, &Monument, &Position), Without<Mobile>>();
        let monuments: Vec<_> = q.iter(&world).map(|(i, m, p)| (*i, *m, *p)).collect();
        assert_eq!(monuments.len(), 1);
        let (id, m, p) = monuments[0];
        assert_eq!(id, SpawnId(2), "monuments come after every settlement");
        assert_eq!(m.kind, MonumentKind::Watchtower);
        assert_eq!(p.0, IVec2::new(50, 50));
        assert_eq!(
            world.resource::<NameTable>().get(m.name),
            "Kestmoor Watchtower"
        );
    }
}
