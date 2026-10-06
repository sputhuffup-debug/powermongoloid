//! Walking: paths come from the shared [`PathCache`] (planned serially in
//! SpawnId order), then each villager advances along its path in parallel
//! at a speed set by the terrain underfoot.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use sim_core::{Motion, Position, SpawnId};
use sim_world::{PathCache, WorldMap};

use crate::components::{Enlisted, Travel, TravelState};
use crate::data::Villagers;

pub(crate) fn plan_paths(
    map: Res<WorldMap>,
    villagers: Res<Villagers>,
    mut cache: ResMut<PathCache>,
    mut q: Query<(&SpawnId, &Position, &mut Travel), Without<Enlisted>>,
) {
    let _span = tracing::info_span!("sim_agents::plan_paths").entered();
    // Plan in SpawnId order so the cache fills the same way every run.
    let mut order: Vec<(SpawnId, IVec2, IVec2)> = q
        .iter()
        .filter_map(|(id, pos, t)| match t.state {
            TravelState::Pending { goal } => Some((*id, pos.0, goal)),
            _ => None,
        })
        .collect();
    if order.is_empty() {
        return;
    }
    order.sort_unstable_by_key(|(id, ..)| *id);
    let results: Vec<(SpawnId, TravelState)> = order
        .into_iter()
        .map(|(id, from, goal)| {
            let state = if from == goal {
                TravelState::Arrived
            } else {
                match cache.get_or_find(&map, &villagers.travel_costs, from, goal) {
                    Some(path) if path.len() > 1 => TravelState::Moving {
                        goal,
                        path,
                        next: 1,
                    },
                    Some(_) => TravelState::Arrived,
                    None => TravelState::Failed,
                }
            };
            (id, state)
        })
        .collect();
    for (id, _, mut travel) in &mut q {
        if matches!(travel.state, TravelState::Pending { .. })
            && let Ok(i) = results.binary_search_by_key(id, |(rid, _)| *rid)
        {
            travel.state = results[i].1.clone();
        }
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
