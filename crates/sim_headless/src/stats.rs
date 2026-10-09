//! Population statistics for tuning, the CLI and scenario tests.

use std::collections::BTreeMap;

use bevy_app::App;
use bevy_ecs::prelude::*;
use sim_agents::{ActionKind, Activity, FoodSecurity, Needs, Stage, Villager};
use sim_core::{Calendar, NameTable, Position, SimTick};
use sim_economy::{Building, Economy, Occupation, SettlementEconomy, Stockpile};
use sim_world::Settlement;

/// One settlement's state, for `--stats-every` and tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VillageStats {
    pub name: String,
    pub settlement: Option<Entity>,
    pub population: u32,
    pub food_days: f32,
    pub mean_hunger: f32,
    /// Goods in the storehouse, by name.
    pub store: BTreeMap<String, u32>,
    /// Goods waiting at workplaces (not yet hauled), by name.
    pub at_workplaces: BTreeMap<String, u32>,
    /// Villagers by occupation.
    pub occupations: BTreeMap<Occupation, u32>,
    /// Workplaces by kind.
    pub workplaces: BTreeMap<String, u32>,
}

/// A population summary at one tick.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    pub tick: u64,
    pub day: u64,
    pub phase: &'static str,
    pub villagers: u32,
    /// Villagers per (action, performing?) — performing = at the target.
    pub actions: BTreeMap<(ActionKind, bool), u32>,
    pub asleep_at_home: u32,
    pub at_home: u32,
    pub mean_hunger: f32,
    pub max_hunger: f32,
    pub mean_rest: f32,
    /// Workers (non-idle occupation, working age) and how many are working
    /// or walking to work / hauling.
    pub workers: u32,
    pub working_or_commuting: u32,
    /// Workers not working, by occupation and what they're doing instead.
    pub not_working: BTreeMap<(Occupation, ActionKind), u32>,
    /// Completed work cycles (or hauls), by occupation.
    pub cycles: BTreeMap<Occupation, u64>,
    /// Villagers who have eaten at least once.
    pub ever_ate: u32,
    /// A description of up to 10 villagers who never ate (diagnostics).
    pub never_ate: Vec<String>,
    /// Up to 5 villagers choosing sleep but not asleep at home (diagnostics).
    pub restless: Vec<String>,
    /// The hungriest villager and why it's doing what it's doing.
    pub hungriest: String,
    pub villages: Vec<VillageStats>,
}

impl Stats {
    /// Villagers doing `kind`.
    pub fn count(&self, kind: ActionKind) -> u32 {
        self.actions
            .iter()
            .filter(|((k, _), _)| *k == kind)
            .map(|(_, n)| *n)
            .sum()
    }

    /// A one-line summary for the CLI.
    pub fn line(&self) -> String {
        let acts: Vec<String> = ActionKind::ALL
            .iter()
            .filter(|k| self.count(**k) > 0)
            .map(|k| format!("{}={}", short(*k), self.count(*k)))
            .collect();
        let food: Vec<String> = self
            .villages
            .iter()
            .map(|v| format!("{:.1}", v.food_days))
            .collect();
        format!(
            "day {} {:<7} | {} | asleep@home {} | work {}/{} | hunger {:.2} (max {:.2}) rest {:.2} | food days [{}]",
            self.day,
            self.phase,
            acts.join(" "),
            self.asleep_at_home,
            self.working_or_commuting,
            self.workers,
            self.mean_hunger,
            self.max_hunger,
            self.mean_rest,
            food.join(" ")
        )
    }
}

fn short(k: ActionKind) -> &'static str {
    match k {
        ActionKind::Idle => "idle",
        ActionKind::Eat => "eat",
        ActionKind::Fetch => "fetch",
        ActionKind::Sleep => "sleep",
        ActionKind::Work => "work",
        ActionKind::Haul => "haul",
        ActionKind::GoHome => "home",
        ActionKind::Flee => "flee",
        ActionKind::Hide => "hide",
        ActionKind::Fight => "fight",
        ActionKind::Follow => "follow",
        ActionKind::Rout => "rout",
    }
}

/// Summarises the villagers (not soldiers) and every settlement.
pub fn collect(app: &mut App) -> Stats {
    let world = app.world_mut();
    let tick = world.resource::<SimTick>().0;
    let reading = world.resource::<Calendar>().at(tick);
    let mut stats = Stats {
        tick,
        day: reading.day,
        phase: reading.phase.name(),
        ..Default::default()
    };
    let home_tiles: BTreeMap<u64, bevy_math::IVec2> = {
        let mut q = world.query::<(Entity, &Building)>();
        q.iter(world).map(|(e, b)| (e.to_bits(), b.tile)).collect()
    };
    let mut per_settlement: BTreeMap<u64, (f32, u32)> = BTreeMap::new();
    let mut occupations_by_settlement: BTreeMap<u64, BTreeMap<Occupation, u32>> = BTreeMap::new();
    // Villagers at home in their villages (not those serving in an army).
    let mut q = world.query_filtered::<(
        &Villager,
        &Occupation,
        &Activity,
        &Needs,
        &Position,
        &sim_agents::Employment,
        &sim_agents::DecisionTrace,
    ), Without<sim_agents::Enlisted>>();
    let mut hunger_sum = 0.0;
    let mut rest_sum = 0.0;
    for (v, occ, act, needs, pos, job, trace) in q.iter(world) {
        *stats.cycles.entry(*occ).or_default() += job.cycles as u64;
        if needs.meals > 0 {
            stats.ever_ate += 1;
        } else if stats.never_ate.len() < 10 {
            let home = home_tiles.get(&v.home.to_bits()).copied();
            stats.never_ate.push(format!(
                "{} {:?} at {} (home {:?}) {:?}/{:?} step {} hunger {:.2} trace@{} {:?}",
                occ.name(),
                act.kind,
                pos.0,
                home,
                act.stage,
                act.target.map(|t| t.to_bits()),
                act.step,
                needs.hunger,
                trace.tick,
                trace.options
            ));
        }
        stats.villagers += 1;
        let performing = act.stage == Stage::Perform;
        *stats.actions.entry((act.kind, performing)).or_default() += 1;
        let home = home_tiles.get(&v.home.to_bits()).copied();
        let at_home = home == Some(pos.0);
        if at_home {
            stats.at_home += 1;
        }
        if at_home && act.kind == ActionKind::Sleep && performing {
            stats.asleep_at_home += 1;
        } else if act.kind == ActionKind::Sleep && stats.restless.len() < 5 {
            stats.restless.push(format!(
                "{} at {} home {:?} {:?} step {} rest {:.2}",
                occ.name(),
                pos.0,
                home,
                act.stage,
                act.step,
                needs.rest
            ));
        }
        if *occ != Occupation::Idle {
            stats.workers += 1;
            if matches!(act.kind, ActionKind::Work | ActionKind::Haul) {
                stats.working_or_commuting += 1;
            } else {
                *stats.not_working.entry((*occ, act.kind)).or_default() += 1;
            }
        }
        hunger_sum += needs.hunger;
        rest_sum += needs.rest;
        if needs.hunger > stats.max_hunger {
            stats.hungriest = format!(
                "{} hunger {:.2} meals {} {:?} {:?} at {} home {:?} inv {:?} trace {:?}",
                occ.name(),
                needs.hunger,
                needs.meals,
                act.kind,
                act.stage,
                pos.0,
                home,
                (),
                trace.options
            );
        }
        stats.max_hunger = stats.max_hunger.max(needs.hunger);
        *occupations_by_settlement
            .entry(v.settlement.to_bits())
            .or_default()
            .entry(*occ)
            .or_default() += 1;
        let e = per_settlement.entry(v.settlement.to_bits()).or_default();
        e.0 += needs.hunger;
        e.1 += 1;
    }
    if stats.villagers > 0 {
        stats.mean_hunger = hunger_sum / stats.villagers as f32;
        stats.mean_rest = rest_sum / stats.villagers as f32;
    }
    let econ = world.resource::<Economy>().clone();
    let names = world.resource::<NameTable>().clone();
    let mut sq = world.query::<(Entity, &Settlement, &SettlementEconomy, &FoodSecurity)>();
    let mut villages: Vec<(Entity, Settlement, SettlementEconomy, FoodSecurity)> = sq
        .iter(world)
        .map(|(e, s, se, f)| (e, *s, se.clone(), *f))
        .collect();
    villages.sort_by_key(|(e, ..)| e.to_bits());
    for (e, s, se, fs) in villages {
        let store = world
            .get::<Stockpile>(se.storehouse)
            .map(|st| {
                st.goods
                    .iter()
                    .map(|(g, q)| (econ.good_name(*g).to_string(), *q))
                    .collect()
            })
            .unwrap_or_default();
        let mut at_workplaces: BTreeMap<String, u32> = BTreeMap::new();
        let mut workplace_kinds: BTreeMap<String, u32> = BTreeMap::new();
        for wp in &se.workplaces {
            if let Some(w) = world.get::<sim_economy::Workplace>(*wp) {
                *workplace_kinds
                    .entry(w.kind.name().to_string())
                    .or_default() += 1;
            }
            if *wp == se.storehouse {
                continue;
            }
            if let Some(st) = world.get::<Stockpile>(*wp) {
                for (g, q) in &st.goods {
                    *at_workplaces
                        .entry(econ.good_name(*g).to_string())
                        .or_default() += q;
                }
            }
        }
        let (h, n) = per_settlement
            .get(&e.to_bits())
            .copied()
            .unwrap_or((0.0, 0));
        stats.villages.push(VillageStats {
            name: names.get(s.name).to_string(),
            settlement: Some(e),
            population: se.population,
            food_days: fs.days,
            mean_hunger: if n > 0 { h / n as f32 } else { 0.0 },
            store,
            at_workplaces,
            occupations: occupations_by_settlement
                .remove(&e.to_bits())
                .unwrap_or_default(),
            workplaces: workplace_kinds,
        });
    }
    stats
}

/// One line per army and per faction, for tuning (M3).
pub fn armies(app: &mut App) -> Vec<String> {
    use sim_agents::CombatStats;
    use sim_core::{Motion, SpawnId};
    use sim_military::{Army, ArmyStats, Captain, Follower, describe};
    use sim_world::world_pos;
    let world = app.world_mut();
    let Some(army) = world.get_resource::<Army>().cloned() else {
        return Vec::new();
    };
    let econ = world.resource::<Economy>().clone();
    let hunger_per_day = world
        .resource::<sim_agents::Villagers>()
        .data
        .needs
        .hunger_per_day;
    let mut q = world.query::<(
        Entity,
        &SpawnId,
        &Captain,
        &Position,
        &Motion,
        &CombatStats,
        Option<&Stockpile>,
    )>();
    let mut caps: Vec<_> = q
        .iter(world)
        .map(|(e, id, c, p, m, cs, s)| {
            (
                *id,
                e,
                c.clone(),
                world_pos(p, Some(m)),
                cs.morale,
                s.map_or(0.0, |s| s.food_value(&econ)),
            )
        })
        .collect();
    caps.sort_unstable_by_key(|c| c.0);
    let rallies: Vec<(Entity, Option<std::sync::Arc<sim_world::FlowField>>)> = world
        .resource::<sim_military::ArmyIndex>()
        .armies
        .iter()
        .map(|a| (a.entity, a.rally.clone()))
        .collect();
    let map = world.resource::<sim_world::WorldMap>().clone();
    let mut fq = world.query::<(
        &Follower,
        &CombatStats,
        &Activity,
        &Needs,
        &Position,
        &Motion,
    )>();
    let mut out = Vec::new();
    for (id, e, c, pos, morale, food) in caps {
        let mut n = 0;
        let (mut fighting, mut routed, mut morale_sum, mut hunger_sum, mut spread) =
            (0, 0, 0.0, 0.0, 0.0f32);
        let mut far = String::new();
        for (f, cs, act, needs, p, m) in fq.iter(world) {
            if f.captain != e {
                continue;
            }
            n += 1;
            morale_sum += cs.morale;
            hunger_sum += needs.hunger;
            let d = world_pos(p, Some(m)).distance(pos);
            if d > spread {
                spread = d;
                let wp = world_pos(p, Some(m));
                let rally = rallies
                    .iter()
                    .find(|(c, _)| *c == e)
                    .and_then(|(_, r)| r.clone());
                far = format!(
                    "farthest at {:?} ({}) slot {} {:?} hunger {:.2} morale {:.2} v {:?} rally cost {:?} dir {:?}",
                    p.0,
                    map.terrain(p.0).name(),
                    f.slot,
                    act.kind,
                    needs.hunger,
                    cs.morale,
                    m.velocity,
                    rally.as_ref().and_then(|r| r.cost(wp.floor().as_ivec2())),
                    rally.as_ref().and_then(|r| r.direction(wp)),
                );
            }
            match act.kind {
                ActionKind::Fight => fighting += 1,
                ActionKind::Rout => routed += 1,
                _ => {}
            }
        }
        let people = (n + 1) as f32;
        out.push(format!(
            "{} captain {} ({}) at ({:.0},{:.0}) goal {:?} order {} | {} men, {} fighting, {} routed, max spread {:.0} | morale {:.2}/{:.2} hunger {:.2} | food {:.1} days",
            army.faction_name(c.faction),
            id.0,
            c.posture.name(),
            pos.x,
            pos.y,
            c.goal.map(|g| (g.x, g.y)),
            c.orders.front().map_or("none".into(), |o| describe(&o.spec)),
            n,
            fighting,
            routed,
            spread,
            morale,
            morale_sum / n.max(1) as f32,
            hunger_sum / n.max(1) as f32,
            food / (hunger_per_day * people),
        ));
        if std::env::var_os("STATS_DETAIL").is_some() {
            out.push(format!("  {far}; window {:?}", c.window));
        }
    }
    let stats = world.resource::<ArmyStats>().clone();
    for (i, s) in stats.factions.iter().enumerate() {
        if *s == Default::default() {
            continue;
        }
        out.push(format!(
            "{}: {:?}",
            army.faction_name(sim_core::FactionId(i as u16)),
            s
        ));
    }
    if let Some(line) = run_line(world) {
        out.push(line);
    }
    out
}

/// In a run, one line on band life (GP-22): requests, invitations,
/// drifters, the player's share of each region with its dominion flags
/// (GP-24), captain slots (GP-25), the best goodwill toward the player, the
/// rival's lands, garrison and clock (GP-28) and the pacing director's
/// tension and bands (GP-16).
/// `None` outside a run.
pub fn run_line(world: &mut World) -> Option<String> {
    use sim_core::{FactionId, OfferKind, Offers, RunState};
    use sim_military::{RequestBoard, RunProgress};
    if !world.resource::<RunState>().in_run() {
        return None;
    }
    let board = world.resource::<RequestBoard>();
    let invitations = world
        .resource::<Offers>()
        .pending
        .iter()
        .filter(|o| matches!(o.kind, OfferKind::Invitation { .. }))
        .count();
    let line = format!(
        "run: requests open {} posted {} done {} failed {} | invitations pending {invitations} | drifters {}",
        board.open.len(),
        board.posted,
        board.done,
        board.failed,
        world.resource::<RunProgress>().drifters,
    );
    let progress = world.resource::<RunProgress>();
    let shares: Vec<String> = progress
        .dominion
        .iter()
        .enumerate()
        .map(|(r, t)| format!("r{r} {:.0}%/{:02b}", t.share() * 100.0, t.flags))
        .collect();
    let line = format!(
        "{line} | dominion {} | captain slots {}",
        shares.join(" "),
        progress.captain_slots
    );
    let mut q = world.query::<&sim_agents::Goodwill>();
    let best = q
        .iter(world)
        .map(|g| g.get(FactionId::PLAYER))
        .fold(f32::NEG_INFINITY, f32::max);
    // GP-28, GP-16: the rival's garrison and clock, the director's mood.
    let rival = world.resource::<sim_military::RivalState>().clone();
    let garrison = sim_military::rival::garrison(world).map_or(0, |g| {
        let mut q = world.query::<&sim_military::Follower>();
        q.iter(world).filter(|f| f.captain == g).count() + 1
    });
    let mut q = world.query::<(&sim_agents::Owner, &sim_economy::SettlementEconomy)>();
    let (lands, people) = q
        .iter(world)
        .filter(|(o, _)| o.faction == FactionId::RIVAL)
        .fold((0, 0), |(n, p), (_, se)| (n + 1, p + se.population));
    let director = world.resource::<sim_military::Director>().clone();
    Some(format!(
        "{line} | best goodwill {best:.2} | rival: {lands} settlements ({people} people), garrison {garrison}, defence +{:.2}, levied {}, raids {} | director: tension {:.2}, bands {}",
        rival.defence_bonus, rival.levied, rival.raids, director.tension, director.bands
    ))
}

/// UI-09: the run's summary ([`sim_military::RunSummary`]) as lines: how
/// it ended, days, settlements, monuments, battles and veterans. Empty
/// outside a run.
pub fn run_summary(app: &App) -> Vec<String> {
    if !app.world().resource::<sim_core::RunState>().in_run() {
        return Vec::new();
    }
    sim_military::run_summary(app.world()).lines()
}

/// UI-09: every chronicle entry as a sentence
/// ([`sim_military::chronicle::describe`]), oldest first.
pub fn chronicle(app: &App) -> Vec<String> {
    let world = app.world();
    let entries = world.resource::<sim_core::Chronicle>().entries();
    sim_military::describe_entries(world, entries)
}
