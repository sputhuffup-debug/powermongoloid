//! Population statistics for tuning, the CLI and scenario tests.

use std::collections::BTreeMap;

use bevy_app::App;
use bevy_ecs::prelude::*;
use sim_agents::{ActionKind, Activity, FoodSecurity, Needs, Stage, Villager};
use sim_core::{Calendar, NameTable, Position, SimTick};
use sim_economy::{Building, Economy, Occupation, SettlementEconomy, Stockpile};
use sim_world::Settlement;

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
    pub fn count(&self, kind: ActionKind) -> u32 {
        self.actions
            .iter()
            .filter(|((k, _), _)| *k == kind)
            .map(|(_, n)| *n)
            .sum()
    }

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
    }
}

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
    let mut q = world.query::<(
        &Villager,
        &Occupation,
        &Activity,
        &Needs,
        &Position,
        &sim_agents::Employment,
        &sim_agents::DecisionTrace,
    )>();
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
