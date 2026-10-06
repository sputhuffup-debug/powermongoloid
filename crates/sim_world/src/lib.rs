//! Terrain, worldgen, regions (M1); spatial grid and flow fields arrive with
//! M2 and M3.
//!
//! Never depends on rendering, windowing, audio or asset crates.

pub mod map;
pub mod noise;
pub mod settings;
pub mod worldgen;

use std::time::Duration;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::demo::DemoAnchors;
use sim_core::{AppHashExt, NameId, NameTable, NextSpawnId, Position, SimSeed, SimSet, SpawnId};
use sim_core::{StateHash, StateHasher};

pub use map::{NO_REGION, RegionId, Terrain, WorldMap};
pub use settings::WorldGenSettings;
pub use worldgen::{GeneratedWorld, generate_world};

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
            .exempt_from_hash::<WorldGenReport>("wall-clock diagnostics")
            .add_systems(
                FixedUpdate,
                map::refresh_chunk_digests.in_set(SimSet::Bookkeeping),
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

/// Inserts a generated world: the map, settlement entities and roads.
pub fn install_world(world: &mut World, generated: GeneratedWorld, total: Duration) {
    let GeneratedWorld {
        map,
        settlements,
        roads,
        timings,
    } = generated;
    let mut ids = Vec::with_capacity(settlements.len());
    let mut anchors = Vec::with_capacity(settlements.len());
    for site in settlements {
        let name = world.resource_mut::<NameTable>().push(site.name);
        let id = world.resource_mut::<NextSpawnId>().allocate();
        world.spawn((
            id,
            Settlement {
                name,
                region: site.region,
                center: site.center,
            },
            Position(site.center),
        ));
        ids.push(id);
        anchors.push(site.center);
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
    if let Some(mut demo) = world.get_resource_mut::<DemoAnchors>() {
        demo.0 = anchors;
    }
}
