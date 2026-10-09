//! Spawning armies for scenarios, tests and the playtest start. Runs before
//! the first tick, like world population.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use rand::RngExt;
use sim_agents::{
    ActionKind, Activity, CombatStats, DecisionTrace, Enlisted, Needs, Stage, Stance, Strike,
    Villagers,
};
use sim_core::rng::rng_key;
use sim_core::{
    Allegiance, DetailLevel, FactionId, Motion, NameTable, NextSpawnId, Position, Posture, SimSeed,
};
use sim_economy::{Economy, Inventory, SettlementEconomy, Stockpile, Transfers};
use sim_world::{FlowField, Mobile, Settlement, TravelCosts, WorldMap};

use crate::components::{Captain, Equipment, Factions, Follower, Raider, Soldier};
use crate::data::{Army, UnitStats};
use crate::soldiers::slot_offset;
use crate::veterans::Veteran;

/// What to spawn. `Copy`, so scenario configs can hold one.
#[derive(Debug, Clone, Copy)]
pub struct ArmySpec {
    /// Whose army it is.
    pub faction: FactionId,
    /// The captain's starting posture.
    pub posture: Posture,
    /// Where it stands (snapped to the nearest passable tile).
    pub at: IVec2,
    /// Followers besides the captain.
    pub followers: u32,
    /// Days of food in the baggage for the whole army.
    pub food_days: f32,
    /// Every soldier starts with a weapon.
    pub armed: bool,
    /// Driven by the scripted raider AI.
    pub raider: bool,
    /// The followers' stats; `None` uses army.ron's `soldier` (GP-27: a
    /// run's band templates set their own).
    pub soldier: Option<UnitStats>,
    /// Every soldier starts with tools.
    pub tools: bool,
}

impl ArmySpec {
    /// A neutral army with three days of food, no weapons or tools, and
    /// army.ron's soldier stats.
    pub fn new(faction: FactionId, at: IVec2, followers: u32) -> Self {
        Self {
            faction,
            posture: Posture::Neutral,
            at,
            followers,
            food_days: 3.0,
            armed: false,
            raider: false,
            soldier: None,
            tools: false,
        }
    }
}

fn passable(map: &WorldMap, costs: &TravelCosts, t: IVec2) -> bool {
    map.in_bounds(t) && costs.per_terrain[map.terrain(t) as usize].is_some()
}

/// The passable tile nearest `at` (ties: row-major), searching outward.
pub fn nearest_passable(map: &WorldMap, costs: &TravelCosts, at: IVec2) -> Option<IVec2> {
    for r in 0i32..64 {
        let mut best: Option<(i32, i32, i32, IVec2)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let t = at + IVec2::new(dx, dy);
                if passable(map, costs, t) {
                    let k = (dx * dx + dy * dy, t.y, t.x, t);
                    if best.is_none_or(|b| (k.0, k.1, k.2) < (b.0, b.1, b.2)) {
                        best = Some(k);
                    }
                }
            }
        }
        if let Some(b) = best {
            return Some(b.3);
        }
    }
    None
}

/// The tile nearest `at` that the field reaches (ties: row-major).
fn nearest_reachable(field: &FlowField, at: IVec2) -> Option<IVec2> {
    for r in 0i32..48 {
        let mut best: Option<(i32, i32, i32, IVec2)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let t = at + IVec2::new(dx, dy);
                if field.cost(t).is_some() {
                    let k = (dx * dx + dy * dy, t.y, t.x, t);
                    if best.is_none_or(|b| (k.0, k.1, k.2) < (b.0, b.1, b.2)) {
                        best = Some(k);
                    }
                }
            }
        }
        if let Some(b) = best {
            return Some(b.3);
        }
    }
    None
}

/// A soldier's combat stats from a unit's base stats.
pub(crate) fn soldier_stats(u: UnitStats) -> CombatStats {
    CombatStats {
        health: u.health,
        max_health: u.health,
        attack: u.attack,
        defense: u.defense,
        morale: u.morale,
        stance: Stance::Soldier,
    }
}

/// Spawns a captain and `followers` soldiers around `spec.at`. Returns the
/// captain. The faction's seat is set here if it has none.
pub fn spawn_army(world: &mut World, spec: &ArmySpec) -> Entity {
    let army = world.resource::<Army>().clone();
    let villagers = world.resource::<Villagers>().clone();
    let econ = world.resource::<Economy>().clone();
    let seed = *world.resource::<SimSeed>();
    let first = world.resource::<NextSpawnId>().0;
    let mut rng = seed.stream(rng_key("sim_military::spawn_army"), first);
    let costs = villagers.travel_costs.clone();
    let (start, tiles) = {
        let map = world.resource::<WorldMap>();
        let start = nearest_passable(map, &costs, spec.at).unwrap_or(spec.at);
        // Only tiles the captain can walk to (no cut-off spits or islands).
        let r = army.formation_radius(spec.followers).ceil() as i32 + 24;
        let reach = FlowField::build(
            map,
            &costs,
            start,
            start - IVec2::splat(r),
            start + IVec2::splat(r),
        );
        let tiles: Vec<IVec2> = (0..spec.followers)
            .map(|i| {
                let want = (start.as_vec2() + slot_offset(i as u16, army.data.formation.spacing))
                    .round()
                    .as_ivec2();
                nearest_reachable(&reach, want).unwrap_or(start)
            })
            .collect();
        (start, tiles)
    };
    {
        let mut factions = world.resource_mut::<Factions>();
        if factions.seat(spec.faction).is_none() {
            factions.set_seat(spec.faction, start);
        }
    }
    let names = &villagers.data.names;
    let mut person = |world: &mut World, tile: IVec2, unit: UnitStats| -> Entity {
        let given = &names.given[rng.random_range(0..names.given.len())];
        let family = &names.family[rng.random_range(0..names.family.len())];
        let name = world
            .resource_mut::<NameTable>()
            .push(format!("{given} {family}"));
        let hunger = rng.random_range(0.1..0.3);
        let id = world.resource_mut::<NextSpawnId>().allocate();
        world
            .spawn((
                id,
                (
                    Soldier { name },
                    Position(tile),
                    Motion::default(),
                    Allegiance(spec.faction),
                    DetailLevel { full: true },
                    Mobile,
                    soldier_stats(unit),
                    Strike::default(),
                ),
                (
                    Needs {
                        hunger,
                        ..Needs::default()
                    },
                    Inventory::with_capacity(army.data.carry),
                    Transfers::default(),
                    Activity {
                        kind: ActionKind::Follow,
                        stage: Stage::Perform,
                        ..Default::default()
                    },
                    DecisionTrace::default(),
                    Enlisted,
                    Equipment {
                        weapon: spec.armed,
                        tools: spec.tools,
                    },
                    Veteran::default(),
                ),
            ))
            .id()
    };
    let captain = person(world, start, army.data.captain);
    let mut baggage = Stockpile::with_capacity(army.baggage_capacity(spec.followers + 1));
    let need = villagers.data.needs.hunger_per_day * (spec.followers + 1) as f32 * spec.food_days;
    if let Some(&grain) = econ.foods_by_value().last() {
        let v = econ.food_value(grain).max(0.01);
        baggage.add(grain, (need / v).ceil() as u32);
    }
    let mut c = Captain::new(spec.faction, spec.posture);
    c.last_forage = 0;
    world.entity_mut(captain).insert((c, baggage));
    if spec.raider {
        world.entity_mut(captain).insert(Raider::default());
    }
    let soldier = spec.soldier.unwrap_or(army.data.soldier);
    for tile in tiles {
        let f = person(world, tile, soldier);
        world.entity_mut(f).insert(Follower {
            captain,
            slot: 0,
            deserting: false,
        });
    }
    captain
}

/// The go/no-go playtest start (M3): the player's champion with 50 men at
/// the edge of the populated region, 2.5 days of food, and a raiding band
/// on the far side of the region.
#[derive(Debug, Clone, Copy)]
pub struct Skirmish {
    pub player: Entity,
    pub raiders: Entity,
}

/// A battle for benchmarks and stress tests: the skirmish start plus two
/// armed armies of `per_side` at war, already ordered onto the midpoint of
/// their camps. Orders are queued for the first tick as AI commands: setup
/// is re-run when a replay is verified, so recording them as player
/// commands would apply them twice (guide G1).
pub fn setup_battle(world: &mut World, per_side: u32) -> Option<[Entity; 2]> {
    use sim_core::{CommandKind, OrderSpec, SimCommand, SpawnId};
    let s = setup_skirmish(world)?;
    let at = |world: &World, e| world.get::<Position>(e).expect("position").0;
    let (a, b) = (at(world, s.player), at(world, s.raiders));
    let mid = (a + b) / 2;
    let mut out = [s.player; 2];
    for (i, (faction, from)) in [(FactionId::PLAYER, a), (FactionId::RAIDERS, b)]
        .into_iter()
        .enumerate()
    {
        let captain = spawn_army(
            world,
            &ArmySpec {
                food_days: 5.0,
                armed: true,
                ..ArmySpec::new(faction, from, per_side)
            },
        );
        let id = world.get::<SpawnId>(captain).expect("id").0;
        world.write_message(SimCommand::ai(
            faction,
            CommandKind::IssueOrder {
                captain: id,
                order: OrderSpec::MoveTo { x: mid.x, y: mid.y },
                queue: false,
            },
        ));
        out[i] = captain;
    }
    Some(out)
}

/// Spawns the playtest armies; `None` if no settlement is populated.
pub fn setup_skirmish(world: &mut World) -> Option<Skirmish> {
    let mut q = world.query::<(&sim_core::SpawnId, &Settlement, &SettlementEconomy)>();
    let mut centers: Vec<(u64, IVec2)> = q
        .iter(world)
        .filter(|(_, _, se)| se.population > 0)
        .map(|(id, s, _)| (id.0, s.center))
        .collect();
    centers.sort_unstable_by_key(|(id, _)| *id);
    if centers.is_empty() {
        return None;
    }
    let mean = centers.iter().map(|(_, c)| c.as_vec2()).sum::<Vec2>() / centers.len() as f32;
    let farthest = |from: Vec2| {
        centers
            .iter()
            .max_by(|a, b| {
                a.1.as_vec2()
                    .distance(from)
                    .total_cmp(&b.1.as_vec2().distance(from))
                    .then(b.0.cmp(&a.0))
            })
            .map(|(_, c)| *c)
            .expect("non-empty")
    };
    let edge = farthest(mean);
    let out = (edge.as_vec2() - mean).normalize_or(Vec2::X);
    let player_at = (edge.as_vec2() + out * 30.0).round().as_ivec2();
    let far = farthest(edge.as_vec2());
    let out2 = (far.as_vec2() - mean).normalize_or(-Vec2::X);
    let raider_at = (far.as_vec2() + out2 * 20.0).round().as_ivec2();
    // Stay on the populated landmass.
    let region = world.resource::<WorldMap>().region(edge);
    let costs = world.resource::<Villagers>().travel_costs.clone();
    let on_land = |world: &World, at: IVec2, fallback: IVec2| {
        let map = world.resource::<WorldMap>();
        match nearest_passable(map, &costs, at) {
            Some(t) if map.region(t) == region => t,
            _ => fallback,
        }
    };
    let player_at = on_land(world, player_at, edge);
    let raider_at = on_land(world, raider_at, far);
    let player = spawn_army(
        world,
        &ArmySpec {
            food_days: 2.5,
            ..ArmySpec::new(FactionId::PLAYER, player_at, 50)
        },
    );
    let raiders = spawn_army(
        world,
        &ArmySpec {
            posture: Posture::Aggressive,
            food_days: 1.0,
            armed: true,
            raider: true,
            ..ArmySpec::new(FactionId::RAIDERS, raider_at, 30)
        },
    );
    Some(Skirmish { player, raiders })
}
