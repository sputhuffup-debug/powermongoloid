//! Walking: each villager plans its path in parallel (from the shared
//! [`PathCache`], or by searching), then advances along it in parallel at a
//! speed set by the terrain underfoot.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use std::sync::Mutex;

use sim_core::{Motion, Position};
use sim_world::{PathCache, WorldMap};

use crate::components::{Enlisted, Travel, TravelState};
use crate::data::Villagers;

pub(crate) fn plan_paths(
    map: Res<WorldMap>,
    villagers: Res<Villagers>,
    mut cache: ResMut<PathCache>,
    mut q: Query<(&Position, &mut Travel), Without<Enlisted>>,
) {
    let _span = tracing::info_span!("sim_agents::plan_paths").entered();
    // Each agent plans its own path in parallel: a path is a pure function
    // of the map and its endpoints, so the result never depends on order.
    // Paths the cache lacked are added afterwards, sorted by endpoints.
    cache.sync(&map);
    let found = Mutex::new(Vec::new());
    {
        let cache = &*cache;
        let costs = &villagers.travel_costs;
        q.par_iter_mut().for_each(|(pos, mut travel)| {
            let TravelState::Pending { goal } = travel.state else {
                return;
            };
            let from = pos.0;
            if from == goal {
                travel.state = TravelState::Arrived;
                return;
            }
            let path = cache.peek(from, goal).unwrap_or_else(|| {
                let p = PathCache::search(&map, costs, from, goal);
                found
                    .lock()
                    .expect("no panics while held")
                    .push(((from.x, from.y, goal.x, goal.y), p.clone()));
                p
            });
            travel.state = match path {
                Some(path) if path.len() > 1 => TravelState::Moving {
                    goal,
                    path,
                    next: 1,
                },
                Some(_) => TravelState::Arrived,
                None => TravelState::Failed,
            };
        });
    }
    let mut found = found.into_inner().expect("no panics while held");
    found.sort_by_key(|(k, _)| *k);
    found.dedup_by_key(|(k, _)| *k);
    for ((fx, fy, gx, gy), path) in found {
        cache.insert(IVec2::new(fx, fy), IVec2::new(gx, gy), path);
    }
}

pub(crate) fn move_agents(
    map: Res<WorldMap>,
    villagers: Res<Villagers>,
    mut q: Query<(&mut Position, &mut Motion, &mut Travel), Without<Enlisted>>,
) {
    let _span = tracing::info_span!("sim_agents::move_agents").entered();
    let speeds = &villagers.data.movement.speed;
    q.par_iter_mut()
        .for_each(|(mut pos, mut motion, mut travel)| {
            let TravelState::Moving { path, next, .. } = &mut travel.state else {
                if motion.velocity != Vec2::ZERO {
                    motion.velocity = Vec2::ZERO;
                }
                return;
            };
            let start = pos.0.as_vec2() + Vec2::splat(0.5) + motion.offset;
            let mut p = start;
            let mut budget = speeds.of(map.terrain(pos.0)).max(0.02);
            let mut arrived = false;
            while budget > 0.0 {
                let target = path[*next as usize].as_vec2() + Vec2::splat(0.5);
                let d = target - p;
                let len = d.length();
                if len <= budget {
                    p = target;
                    budget -= len;
                    *next += 1;
                    if *next as usize >= path.len() {
                        arrived = true;
                        break;
                    }
                } else {
                    p += d / len * budget;
                    break;
                }
            }
            let tile = (p - Vec2::splat(0.5)).round().as_ivec2();
            pos.0 = tile;
            motion.offset = p - tile.as_vec2() - Vec2::splat(0.5);
            motion.velocity = p - start;
            if arrived {
                travel.state = TravelState::Arrived;
            }
        });
}
