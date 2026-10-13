//! The scripted hostile band (M3 stand-in for rival warlords, which arrive
//! with GP-09 in M5). It decides from the sim state and issues ordinary
//! AI-origin commands, so it uses the same order path as the player and
//! replays without being recorded.
//!
//! In a run (GP-16, ADR 0018) a band the director aimed raids its target,
//! and when it then has nothing to do (no prey, no village of the
//! player's) it lingers by the village for director.ron
//! `band_linger_hours`, then withdraws beyond army.ron `threat.radius` of
//! every settlement (`withdraw_tile`), so the village goes back to work.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use sim_agents::Owner;
use sim_core::{
    Calendar, CommandKind, FactionId, OrderSpec, Position, RunState, SimCommand, SimTick, SpawnId,
};
use sim_economy::SettlementEconomy;
use sim_world::{NO_REGION, Settlement, TravelCosts, WorldMap};

use crate::components::{Bandits, Captain, Raider};
use crate::data::Army;
use crate::director::DirectorData;
use crate::index::{ArmyIndex, FighterGrid};
use crate::influence::Influence;
use crate::monuments::Guard;
use crate::progression::Progression;
use crate::rival::RivalState;
use crate::spawn::nearest_passable;

/// How often a raider reconsiders (ticks).
const THINK_EVERY: u64 = 300;
/// Raiders with less food than this (days) raid a village first.
const RAID_BELOW_DAYS: f32 = 2.0;
/// A band withdrawing in a run (GP-16) heads this many tiles beyond
/// army.ron `threat.radius` from the settlement nearest it.
const WITHDRAW_BEYOND: f32 = 10.0;

/// Runs in `SimInputSet::Ai`, after scripted and player input, so its
/// commands always arrive in the same order (guide G2). Raiders never hunt
/// a monument's guard camp (GP-23). In a run, their prey is the player's
/// alone (guide D2): they hunt only the player's armies no bigger than
/// themselves and raid only the player's settlements, and when hungry with
/// none of the player's settlements to raid they hunt anyway. A band the
/// pacing director aimed (`Raider::target`, GP-16) first raids its target.
/// In a run, a band with nothing to do lingers `band_linger_hours` after
/// that raid, then withdraws ([`withdraw_tile`], ADR 0018); outside a run
/// it holds where it is.
///
/// GP-33 (M4a, run only): broken men ([`Bandits`]) have their own mind
/// ([`bandit_target`]): each think they raid the populated town, not the
/// rival's seat, not their home and not the last raided, with the best
/// [`crate::fallout::bandit_score`], and count the raid; they never hunt
/// armies.
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Bevy system parameters.
pub(crate) fn think(
    tick: Res<SimTick>,
    run: Res<RunState>,
    army: Res<Army>,
    index: Res<ArmyIndex>,
    villagers: Res<sim_agents::Villagers>,
    (cal, director, map): (Res<Calendar>, Res<DirectorData>, Res<WorldMap>),
    (rival, grid, progression): (Res<RivalState>, Res<FighterGrid>, Res<Progression>),
    mut raiders: Query<(
        Entity,
        &SpawnId,
        &Captain,
        &Position,
        &mut Raider,
        Option<&mut Bandits>,
    )>,
    captains: Query<(Entity, &SpawnId, &Captain, &Position), (Without<Raider>, Without<Guard>)>,
    settlements: Query<(
        &SpawnId,
        &Settlement,
        &SettlementEconomy,
        Option<&Owner>,
        Option<&Influence>,
    )>,
    mut out: MessageWriter<SimCommand>,
) {
    let _span = tracing::info_span!("sim_military::raiders").entered();
    let mut ordered: Vec<_> = raiders.iter_mut().collect();
    ordered.sort_unstable_by_key(|(_, id, ..)| **id);
    for (e, id, captain, pos, mut raider, bandits) in ordered {
        if tick.0 < raider.next_think {
            continue;
        }
        raider.next_think = tick.0 + THINK_EVERY;
        let here = pos.0.as_vec2();
        // Food first; once fed, hunt the nearest enemy army.
        let days = index.get(e).map_or(0.0, |a| {
            a.food / (villagers.data.needs.hunger_per_day * (a.followers + 1) as f32).max(1e-3)
        });
        let raiding = matches!(
            captain.orders.front().map(|o| o.spec),
            Some(OrderSpec::GatherFood { .. })
        );
        if raiding {
            continue;
        }
        // GP-33: broken men raid where lords are weak.
        if run.in_run()
            && let Some(mut b) = bandits
        {
            let towns: Vec<BanditView> = settlements
                .iter()
                .filter(|(_, _, se, ..)| se.population > 0)
                .map(|(sid, s, _, owner, influence)| {
                    let owner = owner.map_or(FactionId::INDEPENDENT, |o| o.faction);
                    BanditView {
                        id: sid.0,
                        centre: s.center,
                        owner,
                        garrison: garrison_at(&grid, s.center, owner, army.data.garrison.radius),
                        player_share: influence.map_or(0.0, |i| i.player),
                    }
                })
                .collect();
            let target = bandit_target(
                &progression.data.fallout,
                &towns,
                pos.0,
                &b,
                rival.seat,
                raider.last_target,
            );
            if let Some(t) = target {
                raider.last_target = Some(t);
                b.raids += 1;
                out.write(SimCommand::ai(
                    captain.faction,
                    CommandKind::IssueOrder {
                        captain: id.0,
                        order: OrderSpec::GatherFood { settlement: t },
                        queue: false,
                    },
                ));
            }
            continue;
        }
        // GP-16: the director's target first, once.
        let mut just_raided = false;
        if let Some(t) = raider.target {
            if raider.last_target != Some(t)
                && settlements
                    .iter()
                    .any(|(sid, _, se, ..)| sid.0 == t && se.population > 0)
            {
                raider.last_target = Some(t);
                out.write(SimCommand::ai(
                    captain.faction,
                    CommandKind::IssueOrder {
                        captain: id.0,
                        order: OrderSpec::GatherFood { settlement: t },
                        queue: false,
                    },
                ));
                continue;
            }
            raider.target = None;
            just_raided = true;
        }
        // In a run, only an army no bigger than the band is prey.
        let people = |e: Entity| index.get(e).map_or(1, |a| a.followers + 1);
        let strength = people(e);
        let prey = captains
            .iter()
            .filter(|(ce, _, c, _)| {
                army.at_war(c.faction, captain.faction)
                    && (!run.in_run()
                        || (c.faction == FactionId::PLAYER && people(*ce) <= strength))
            })
            .map(|(_, cid, _, p)| (p.0.as_vec2().distance(here), *cid, p.0))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let order = if let Some((_, _, at)) = prey.filter(|_| days >= RAID_BELOW_DAYS) {
            OrderSpec::MoveTo { x: at.x, y: at.y }
        } else {
            // Otherwise raid the nearest village not raided last (in a
            // run, the player's).
            let village = settlements
                .iter()
                .filter(|(sid, _, se, owner, _)| {
                    se.population > 0
                        && Some(sid.0) != raider.last_target
                        && (!run.in_run() || owner.is_some_and(|o| o.faction == FactionId::PLAYER))
                })
                .map(|(sid, s, ..)| (s.center.as_vec2().distance(here), sid.0))
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            match (village, prey) {
                (Some((_, sid)), _) => {
                    raider.last_target = Some(sid);
                    OrderSpec::GatherFood { settlement: sid }
                }
                (None, Some((_, _, at))) if run.in_run() => OrderSpec::MoveTo { x: at.x, y: at.y },
                // GP-16 (ADR 0018): nothing to do after the director's
                // raid: stay by the village a while (it hides; its
                // DriveOff request stands), then withdraw.
                _ if run.in_run() && just_raided => {
                    raider.next_think = tick.0 + cal.hours(director.band_linger_hours);
                    continue;
                }
                _ if run.in_run() && captain.orders.is_empty() => {
                    let mut centres: Vec<(u64, IVec2)> = settlements
                        .iter()
                        .filter(|(_, _, se, ..)| se.population > 0)
                        .map(|(sid, s, ..)| (sid.0, s.center))
                        .collect();
                    centres.sort_unstable_by_key(|c| c.0);
                    let centres: Vec<IVec2> = centres.into_iter().map(|c| c.1).collect();
                    let radius = army.data.threat.radius;
                    match withdraw_tile(pos.0, &centres, &map, &villagers.travel_costs, radius) {
                        Some(t) => OrderSpec::MoveTo { x: t.x, y: t.y },
                        None => continue,
                    }
                }
                _ => continue,
            }
        };
        out.write(SimCommand::ai(
            captain.faction,
            CommandKind::IssueOrder {
                captain: id.0,
                order,
                queue: false,
            },
        ));
    }
}

/// A town as a bandit band sees it (GP-33).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BanditView {
    /// Its SpawnId.
    pub id: u64,
    /// Its centre.
    pub centre: IVec2,
    /// Its owner.
    pub owner: FactionId,
    /// The owner's soldiers standing at it.
    pub garrison: u32,
    /// The player's influence share there.
    pub player_share: f32,
}

/// The owner's soldiers (not militia, not routed) within `radius` of
/// `at`, from the fighter grid; none for an unowned town.
fn garrison_at(grid: &FighterGrid, at: IVec2, owner: FactionId, radius: f32) -> u32 {
    if owner == FactionId::INDEPENDENT {
        return 0;
    }
    let mut n = 0;
    grid.for_each_within(at.as_vec2(), radius, |e, _| {
        if !e.routed && e.side.against.is_none() && e.side.faction == owner {
            n += 1;
        }
    });
    n
}

/// GP-33: the town a bandit band at `here` raids next: among populated
/// `towns`, not the rival's `seat`, not the band's home and not `last`,
/// the best [`crate::fallout::bandit_score`] (ties: the lowest SpawnId).
pub(crate) fn bandit_target(
    data: &crate::fallout::FalloutData,
    towns: &[BanditView],
    here: IVec2,
    band: &Bandits,
    seat: Option<u64>,
    last: Option<u64>,
) -> Option<u64> {
    towns
        .iter()
        .filter(|t| Some(t.id) != seat && t.id != band.home && Some(t.id) != last)
        .map(|t| {
            let d = (t.centre - here).as_vec2().length();
            let score = crate::fallout::bandit_score(
                data,
                t.owner,
                band.raids,
                d,
                t.garrison,
                t.player_share,
            );
            (score, t.id)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map(|(_, id)| id)
}

/// GP-16 (ADR 0018): where a band at `here` withdraws to, out of reach of
/// every settlement centre in `centres` (populated, by SpawnId): `None` if
/// it is already at least `radius` from all of them. Otherwise it walks
/// away from the nearest centre (ties: the first), trying the direction
/// from that centre to the band, then 30, -30, 60, -60, ... 150 degrees
/// off it, a point `radius + WITHDRAW_BEYOND` tiles from that centre
/// snapped to the nearest passable tile: the first such tile in a region
/// (land the band can reach: never water or an islet, which are
/// `NO_REGION`, as the director's spawn checks) at least `radius` from
/// every centre. `None` if none is.
pub(crate) fn withdraw_tile(
    here: IVec2,
    centres: &[IVec2],
    map: &WorldMap,
    costs: &TravelCosts,
    radius: f32,
) -> Option<IVec2> {
    let clear = |t: IVec2| {
        centres
            .iter()
            .all(|c| (*c - t).as_vec2().length() >= radius)
    };
    if clear(here) {
        return None;
    }
    let dist = |c: &IVec2| (*c - here).as_vec2().length();
    let centre = *centres.iter().min_by(|a, b| dist(a).total_cmp(&dist(b)))?;
    let away = (here - centre).as_vec2();
    let base = if away == Vec2::ZERO {
        0.0
    } else {
        away.y.atan2(away.x)
    };
    let reach = radius + WITHDRAW_BEYOND;
    for k in [0, 1, -1, 2, -2, 3, -3, 4, -4, 5, -5] {
        let angle = base + (k as f32 * 30.0).to_radians();
        let at = (centre.as_vec2() + Vec2::from_angle(angle) * reach)
            .round()
            .as_ivec2();
        if let Some(t) = nearest_passable(map, costs, at)
            && map.region(t) != NO_REGION
            && clear(t)
        {
            return Some(t);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_world::Terrain;

    /// GP-16 (ADR 0018): a band withdraws straight away from the nearest
    /// settlement, beyond the radius of every one; one already clear of
    /// them all stays; when straight away is blocked it turns aside.
    #[test]
    fn withdrawing_walks_out_of_reach_of_every_settlement() {
        let mut map = WorldMap::flat(200, 50, Terrain::Grass);
        let costs = sim_agents::Villagers::default_data(&Calendar::default_data()).travel_costs;
        let centres = [IVec2::new(100, 100), IVec2::new(40, 100)];
        let r = 45.0;
        // Already clear of both.
        assert_eq!(
            withdraw_tile(IVec2::new(100, 170), &centres, &map, &costs, r),
            None
        );
        // East of the first centre: further east.
        let t = withdraw_tile(IVec2::new(110, 100), &centres, &map, &costs, r).unwrap();
        assert_eq!(t, IVec2::new(155, 100));
        // Water from x = 140: straight away the shore is too near, so it
        // turns 30 degrees (south first) and stops at the shore.
        for y in 0..200 {
            for x in 140..200 {
                map.set_terrain(IVec2::new(x, y), Terrain::Water);
            }
        }
        let t = withdraw_tile(IVec2::new(110, 100), &centres, &map, &costs, r).unwrap();
        for c in centres {
            assert!((c - t).as_vec2().length() >= r, "{t}");
        }
        assert!(t.x < 140 && t.y > 100, "turned aside, on land: {t}");
        // Nowhere to go: the whole map is within the radius.
        let small = WorldMap::flat(60, 20, Terrain::Grass);
        assert_eq!(
            withdraw_tile(IVec2::new(30, 30), &[IVec2::new(30, 30)], &small, &costs, r),
            None
        );
    }

    /// GP-16 (ADR 0018): a band never withdraws to land it can't reach. On
    /// seed 42's run island (which has islets), a settlement placed so
    /// that straight away from it lies an islet tile: the band turns aside
    /// to a tile in a region, or stays.
    #[test]
    fn withdrawing_never_heads_for_an_islet() {
        use sim_world::{WorldGenSettings, WorldPreset, generate_world};
        let map = generate_world(42, &WorldGenSettings::preset(WorldPreset::Run), 1).map;
        let costs = sim_agents::Villagers::default_data(&Calendar::default_data()).travel_costs;
        let r = 45.0;
        let size = map.size() as usize;
        let islet = (0..size * size)
            .map(|i| map.pos(i))
            .find(|&p| {
                map.terrain(p).is_land()
                    && map.region(p) == NO_REGION
                    && nearest_passable(&map, &costs, p) == Some(p)
            })
            .expect("seed 42's run island has islets; pick a seed that does");
        // A centre 55 tiles (radius + WITHDRAW_BEYOND) west of the islet
        // tile, and the band just east of it: straight away is the islet.
        let centre = islet - IVec2::new((r + WITHDRAW_BEYOND) as i32, 0);
        let here = centre + IVec2::new(5, 0);
        let t = withdraw_tile(here, &[centre], &map, &costs, r);
        assert_ne!(t, Some(islet), "headed for the islet at {islet}");
        if let Some(t) = t {
            assert_ne!(map.region(t), NO_REGION, "{t} is in no region");
            assert!((t - centre).as_vec2().length() >= r, "{t}");
        }
    }
}
