//! Soldiers (GP-04, SIM-30, SIM-32): each follower decides for itself
//! whether to keep its place, fight or run, then steers through the crowd.
//! Both steps are per-agent and parallel; they read only this tick's army
//! index, fighter grid and spatial grid.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use sim_agents::{
    ActionKind, Activity, CombatStats, DecisionTrace, Enlisted, Factors, ScoredOption, Stage,
    Strike, Villagers,
};
use sim_core::{Allegiance, FactionModifiers, Motion, Position, SimTick, SpawnId, Stat};
use sim_world::{SpatialGrid, TravelCosts, WorldMap, world_pos};

use crate::components::{Captain, Follower};
use crate::data::Army;
use crate::index::{ArmyIndex, FighterGrid, Side};

/// A follower's place relative to its captain: a sunflower disc, so the
/// army is a loose crowd whatever way it faces.
pub fn slot_offset(slot: u16, spacing: f32) -> Vec2 {
    let i = slot as f32;
    let r = spacing * (i + 1.5).sqrt();
    let a = i * 2.399_963;
    Vec2::new(a.cos(), a.sin()) * r
}

/// The leadership aura (SIM-32; heroes reuse it in M4): a follower near
/// its captain hits harder and recovers morale faster.
pub fn led(index: &ArmyIndex, army: &Army, follower: Option<&Follower>, p: Vec2) -> bool {
    follower
        .and_then(|f| index.get(f.captain))
        .is_some_and(|a| a.pos.distance(p) <= army.data.leadership.radius)
}

/// Picks the nearest foe and swings when in reach and ready.
fn aim(strike: &mut Strike, target: Entity, at: Vec2, d: f32, tick: u64, army: &Army) {
    let c = &army.data.combat;
    strike.target = Some(target);
    strike.at = at;
    if d <= c.attack_range && tick >= strike.ready_at {
        strike.swing = true;
        strike.ready_at = tick + c.attack_cooldown;
    }
}

type SoldierQuery<'a> = (
    Entity,
    &'a SpawnId,
    &'a Position,
    &'a Motion,
    &'a Allegiance,
    &'a CombatStats,
    Option<&'a Follower>,
    &'a mut Activity,
    &'a mut Strike,
    &'a mut DecisionTrace,
);

type MilitiaQuery<'a> = (
    &'a Position,
    &'a Motion,
    &'a Allegiance,
    &'a CombatStats,
    &'a mut Strike,
);

/// Soldiers keep their place, fight or rout (parallel); then militia,
/// whom villager AI sent to fight, pick their foe (serial, SpawnId order).
#[allow(clippy::too_many_arguments)] // Bevy system parameters.
pub(crate) fn decide(
    tick: Res<SimTick>,
    army: Res<Army>,
    index: Res<ArmyIndex>,
    grid: Res<FighterGrid>,
    mut soldiers: Query<SoldierQuery, With<Enlisted>>,
    mut militia: Query<MilitiaQuery, Without<Enlisted>>,
) {
    let _span = tracing::info_span!("sim_military::decide").entered();
    let tick = tick.0;
    let c = &army.data.combat;
    let bonus = 1.0 + army.data.leadership.attack_bonus;
    soldiers.par_iter_mut().for_each(
        |(e, id, pos, motion, allegiance, stats, follower, mut act, mut strike, mut trace)| {
            let Some(side) = Side::of(allegiance, stats).filter(|_| stats.alive()) else {
                return;
            };
            let p = world_pos(pos, Some(motion));
            let near = grid.nearest_hostile(e, p, side, c.engage_radius, &army);
            strike.swing = false;
            strike.bonus = if led(&index, &army, follower, p) {
                bonus
            } else {
                1.0
            };
            let routed = act.kind == ActionKind::Rout;
            let kind = if routed {
                if stats.morale >= c.rally_morale && near.is_none() {
                    ActionKind::Follow
                } else {
                    ActionKind::Rout
                }
            } else if stats.morale < c.rout_morale {
                ActionKind::Rout
            } else if near.is_some() {
                ActionKind::Fight
            } else {
                ActionKind::Follow
            };
            match (kind, near) {
                (ActionKind::Fight, Some((t, at, d, _))) => aim(&mut strike, t, at, d, tick, &army),
                (ActionKind::Rout, Some((_, at, ..))) => {
                    strike.target = None;
                    strike.at = at;
                }
                _ => strike.target = None,
            }
            let changed = act.kind != kind;
            if changed {
                *act = Activity {
                    kind,
                    target: strike.target,
                    stage: Stage::Perform,
                    started_tick: tick,
                    ..Default::default()
                };
            } else if act.target != strike.target {
                act.target = strike.target;
            }
            // UI-03: explain the choice (refreshed on change and now and then).
            if changed || (tick + id.0).is_multiple_of(40) {
                let near = grid.count_nearby(e, p, side, c.engage_radius, &army);
                let odds = (near.enemies as f32 / (near.friends + 1) as f32).min(1.0);
                let mut options = vec![
                    ScoredOption {
                        kind: ActionKind::Rout,
                        score: (1.0 - stats.morale / c.rout_morale.max(1e-3)).max(0.0),
                        factors: Factors::new(&[("morale", stats.morale)]),
                    },
                    ScoredOption {
                        kind: ActionKind::Fight,
                        score: if near.enemies > 0 { stats.morale } else { 0.0 },
                        factors: Factors::new(&[
                            ("enemies near", near.enemies as f32),
                            ("odds", 1.0 - odds),
                            ("morale", stats.morale),
                        ]),
                    },
                    ScoredOption {
                        kind: ActionKind::Follow,
                        score: 0.3,
                        factors: Factors::new(&[("baseline", 1.0)]),
                    },
                ];
                options.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.kind.cmp(&b.kind)));
                trace.tick = tick;
                trace.options = options;
            }
        },
    );
    for &(_, e) in &grid.militia {
        let Ok((pos, motion, allegiance, stats, mut strike)) = militia.get_mut(e) else {
            continue;
        };
        let Some(side) = Side::of(allegiance, stats).filter(|_| stats.alive()) else {
            continue;
        };
        let p = world_pos(pos, Some(motion));
        strike.swing = false;
        strike.bonus = 1.0;
        match grid.nearest_hostile(e, p, side, c.engage_radius * 3.0, &army) {
            Some((t, at, d, _)) => aim(&mut strike, t, at, d, tick, &army),
            None => strike.target = None,
        }
    }
}

fn passable(map: &WorldMap, costs: &TravelCosts, p: Vec2) -> bool {
    let t = p.floor().as_ivec2();
    map.in_bounds(t) && costs.per_terrain[map.terrain(t) as usize].is_some()
}

/// Moves `p` by `v`, sliding along walls of impassable terrain. When every
/// way is blocked (an off-centre spot on a narrow strip), it steps toward
/// the middle of its own tile, from where the next tile is reachable.
fn step(map: &WorldMap, costs: &TravelCosts, p: Vec2, v: Vec2) -> Vec2 {
    step_unless(map, costs, p, v, |_| false)
}

/// Like [`step`], but never into another tile for which `full` is true:
/// a crowded tile counts as a wall.
fn step_unless(
    map: &WorldMap,
    costs: &TravelCosts,
    p: Vec2,
    v: Vec2,
    full: impl Fn(IVec2) -> bool,
) -> Vec2 {
    let here = p.floor().as_ivec2();
    for cand in [v, Vec2::new(v.x, 0.0), Vec2::new(0.0, v.y)] {
        let to = p + cand;
        let tile = to.floor().as_ivec2();
        if cand != Vec2::ZERO && passable(map, costs, to) && (tile == here || !full(tile)) {
            return to;
        }
    }
    let center = p.floor() + Vec2::splat(0.5);
    p + (center - p).clamp_length_max(v.length())
}

fn place(pos: &mut Position, motion: &mut Motion, from: Vec2, to: Vec2) {
    let tile = (to - Vec2::splat(0.5)).round().as_ivec2();
    if pos.0 != tile {
        pos.0 = tile;
    }
    motion.offset = to - tile.as_vec2() - Vec2::splat(0.5);
    motion.velocity = to - from;
}

fn speed_at(villagers: &Villagers, map: &WorldMap, p: Vec2) -> f32 {
    let t = p.floor().as_ivec2();
    if !map.in_bounds(t) {
        return 0.0;
    }
    villagers.data.movement.speed.of(map.terrain(t)).max(0.02)
}

/// Captains march along their flow field, slowing for stragglers and
/// halting to fight, at the terrain's speed times the faction's
/// `MarchSpeed` modifier (GP-23). Serial; there are few captains.
#[allow(clippy::type_complexity)]
pub(crate) fn move_captains(
    map: Res<WorldMap>,
    villagers: Res<Villagers>,
    index: Res<ArmyIndex>,
    mods: Res<FactionModifiers>,
    mut q: Query<(
        Entity,
        &mut Position,
        &mut Motion,
        &Captain,
        &Activity,
        &Strike,
        &CombatStats,
    )>,
) {
    let _span = tracing::info_span!("sim_military::move_captains").entered();
    let costs = &villagers.travel_costs;
    for (e, mut pos, mut motion, captain, act, strike, stats) in &mut q {
        let p = world_pos(&pos, Some(&motion));
        if !stats.alive() {
            continue;
        }
        let speed = speed_at(&villagers, &map, p) * mods.mult(captain.faction, Stat::MarchSpeed);
        let v = match act.kind {
            // The army halts to fight; the captain holds its ground in the
            // middle of it and strikes whoever comes within reach.
            ActionKind::Fight => Vec2::ZERO,
            ActionKind::Rout => (p - strike.at).normalize_or_zero() * speed,
            _ => match captain.goal {
                Some(goal) => {
                    let target = goal.as_vec2() + Vec2::splat(0.5);
                    let to = target - p;
                    let info = index.get(e);
                    // Wait for a strung-out army.
                    let slow = match info {
                        Some(a) if a.stragglers * 3 > a.followers.max(1) => 0.25,
                        Some(a) if a.stragglers * 8 > a.followers.max(1) => 0.6,
                        _ => 0.85,
                    };
                    if to.length() < 0.3 {
                        Vec2::ZERO
                    } else {
                        let dir = info
                            .and_then(|a| a.field.as_ref())
                            .and_then(|f| f.direction(p))
                            .unwrap_or_else(|| to.normalize());
                        dir * (speed * slow).min(to.length())
                    }
                }
                None => Vec2::ZERO,
            },
        };
        let to = step(&map, costs, p, v);
        place(&mut pos, &mut motion, p, to);
    }
}

/// Followers keep a loose formation around the captain (descending the
/// flow field when far behind), fight, or flee; militia close with their
/// foe. Everyone pushes away from close neighbours. Followers move at their
/// army's `MarchSpeed` like their captain (GP-23), or the band would string
/// out. Parallel per soldier, then the militia serially.
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Bevy system parameters.
pub(crate) fn move_soldiers(
    map: Res<WorldMap>,
    army: Res<Army>,
    villagers: Res<Villagers>,
    index: Res<ArmyIndex>,
    mods: Res<FactionModifiers>,
    spatial: Res<SpatialGrid>,
    grid: Res<FighterGrid>,
    mut soldiers: Query<
        (
            Entity,
            &mut Position,
            &mut Motion,
            &Activity,
            &CombatStats,
            &Strike,
            Option<&Follower>,
        ),
        (With<Enlisted>, Without<Captain>),
    >,
    mut militia: Query<
        (&mut Position, &mut Motion, &Activity, &CombatStats, &Strike),
        Without<Enlisted>,
    >,
) {
    let _span = tracing::info_span!("sim_military::move_soldiers").entered();
    let costs = &villagers.travel_costs;
    let f = &army.data.formation;
    let c = &army.data.combat;
    let separate = |e: Entity, p: Vec2| {
        let mut push = Vec2::ZERO;
        // Bounded: in a dense crowd, the first few neighbours are enough.
        spatial.for_each_within_budget(p, f.separation, 48, |other, q, d| {
            if other != e && d > 1e-4 {
                push += (p - q) / d * (f.separation - d) / f.separation;
            }
        });
        push * 0.5
    };
    let chase = |strike: &Strike, p: Vec2| {
        let d = strike.at - p;
        if d.length() > c.attack_range * 0.8 {
            d.normalize_or_zero()
        } else {
            Vec2::ZERO
        }
    };
    // `in_melee`: a fighter closing on a foe queues rather than crush into
    // a crowded tile (army.ron `max_per_tile`); marching ignores the cap.
    let apply = |pos: &mut Position,
                 motion: &mut Motion,
                 p: Vec2,
                 want: Vec2,
                 in_melee: bool,
                 march: f32| {
        if want == Vec2::ZERO {
            if motion.velocity != Vec2::ZERO {
                motion.velocity = Vec2::ZERO;
            }
            return;
        }
        let v = want.clamp_length_max(1.2) * (speed_at(&villagers, &map, p) * march);
        let to = step_unless(&map, costs, p, v, |t| {
            in_melee && grid.fighters_on(t) >= f.max_per_tile
        });
        place(pos, motion, p, to);
    };
    soldiers
        .par_iter_mut()
        .for_each(|(e, mut pos, mut motion, act, stats, strike, follower)| {
            if !stats.alive() {
                return;
            }
            let p = world_pos(&pos, Some(&motion));
            let follow = |mult: f32| -> Vec2 {
                let Some(a) = follower.and_then(|fl| index.get(fl.captain)) else {
                    return Vec2::ZERO;
                };
                let slot = follower.map_or(0, |fl| fl.slot);
                // A place on water or other impassable ground moves in
                // toward the captain, so the men don't pile up on the shore.
                let offset = slot_offset(slot, f.spacing);
                let place = [1.0, 0.75, 0.5, 0.25]
                    .into_iter()
                    .map(|k| a.pos + offset * k)
                    .find(|q| passable(&map, costs, *q))
                    .unwrap_or(a.pos);
                let to = place - p;
                let dist = to.length();
                if dist < 0.6 {
                    return Vec2::ZERO;
                }
                // Far from its place: descend the rally field toward the
                // captain (it leads around obstacles). Close: head for the slot.
                let dir = if dist > f.catch_up {
                    a.rally
                        .as_ref()
                        .and_then(|fl| fl.direction(p))
                        .unwrap_or(to / dist)
                } else {
                    to / dist
                };
                let hurry = if dist > f.catch_up { 1.1 } else { 1.0 };
                dir * (dist / 3.0).clamp(0.3, 1.0) * hurry * mult
            };
            let want = match act.kind {
                ActionKind::Fight if strike.target.is_some() => chase(strike, p),
                ActionKind::Rout => {
                    let away = p - strike.at;
                    if strike.at != Vec2::ZERO && away.length() < c.engage_radius * 2.0 {
                        away.normalize_or_zero() * 1.1
                    } else {
                        follow(0.6)
                    }
                }
                _ => follow(1.0),
            };
            let want = want + separate(e, p);
            let march = follower
                .and_then(|fl| index.get(fl.captain))
                .map_or(1.0, |a| mods.mult(a.faction, Stat::MarchSpeed));
            apply(
                &mut pos,
                &mut motion,
                p,
                want,
                act.kind == ActionKind::Fight,
                march,
            );
        });
    for &(_, e) in &grid.militia {
        let Ok((mut pos, mut motion, act, stats, strike)) = militia.get_mut(e) else {
            continue;
        };
        if !stats.alive() || act.kind != ActionKind::Fight || strike.target.is_none() {
            continue;
        }
        let p = world_pos(&pos, Some(&motion));
        let want = chase(strike, p) + separate(e, p);
        apply(&mut pos, &mut motion, p, want, true, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_spread_out_and_stay_loose() {
        let n = 200;
        let r = 1.4 * (n as f32 + 1.5).sqrt();
        let offs: Vec<Vec2> = (0..n).map(|i| slot_offset(i, 1.4)).collect();
        assert!(offs.iter().all(|o| o.length() <= r + 1e-3));
        let mut min = f32::MAX;
        for i in 0..offs.len() {
            for j in i + 1..offs.len() {
                min = min.min(offs[i].distance(offs[j]));
            }
        }
        assert!(min > 0.8, "slots too close: {min}");
    }
}
