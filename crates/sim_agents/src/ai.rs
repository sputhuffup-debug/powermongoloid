//! Utility AI (SIM-12): every option is scored from needs, the phase of the
//! day, what's available and how far away it is. No scripted schedules: a
//! villager goes to work in the morning because Work scores highest then.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_math::IVec2;
use rand::RngExt;
use sim_core::rng::rng_key;
use sim_core::{Calendar, Phase, Position, SimSeed, SimTick, SpawnId};
use sim_economy::{
    Building, Economy, GoodId, Inventory, Occupation, SettlementEconomy, Stockpile, Workplace,
    WorkplaceKind,
};
use sim_world::{Terrain, WorldMap};

use crate::components::{
    ActionKind, Activity, CombatStats, DecisionTrace, Employment, Factors, Needs, NextDecision,
    ScoredOption, Stage, Threat, Travel, TravelState, Villager,
};
use crate::data::Villagers;

/// Goods a carter won't bother collecting below.
pub const MIN_LOAD: u32 = 8;

/// Read-only world state the AI looks at.
#[derive(SystemParam)]
pub struct AiWorld<'w, 's> {
    pub buildings: Query<
        'w,
        's,
        (
            &'static Building,
            &'static Stockpile,
            Option<&'static Workplace>,
            &'static SpawnId,
        ),
    >,
    pub settlements: Query<'w, 's, &'static SettlementEconomy>,
    pub homes: Query<'w, 's, &'static sim_economy::Home>,
    pub threats: Query<'w, 's, &'static Threat>,
    pub econ: Res<'w, Economy>,
    pub calendar: Res<'w, Calendar>,
    pub tick: Res<'w, SimTick>,
    pub villagers: Res<'w, Villagers>,
    pub seed: Res<'w, SimSeed>,
    pub map: Res<'w, WorldMap>,
}

/// Whether a worker can produce right now.
#[derive(Debug, Clone, Copy, PartialEq)]
enum WorkState {
    Ready,
    /// The workplace is full; this output is waiting to be hauled.
    Full(GoodId),
    /// This input is missing.
    NeedsInput(GoodId),
    Impossible,
}

/// A candidate action with where it happens.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub option: ScoredOption,
    pub target: Option<Entity>,
    pub dropoff: Option<Entity>,
    pub good: Option<GoodId>,
    /// Tile to walk to first (None = act in place).
    pub goal: Option<IVec2>,
}

fn phase_factor(phase: Phase, morning: f32, work: f32, evening: f32, night: f32) -> f32 {
    match phase {
        Phase::Morning => morning,
        Phase::Work => work,
        Phase::Evening => evening,
        Phase::Night => night,
    }
}

/// What a villager is, for scoring.
pub struct Me<'a> {
    pub id: SpawnId,
    pub villager: &'a Villager,
    pub occupation: Occupation,
    pub employment: &'a Employment,
    pub needs: &'a Needs,
    pub inventory: &'a Inventory,
    pub tile: IVec2,
    pub adult: bool,
    pub morale: f32,
}

impl AiWorld<'_, '_> {
    fn tile_of(&self, e: Entity) -> Option<IVec2> {
        self.buildings.get(e).ok().map(|(b, ..)| b.tile)
    }

    fn stock(&self, e: Entity) -> Option<&Stockpile> {
        self.buildings.get(e).ok().map(|(_, s, ..)| s)
    }

    fn distance_factor(&self, from: IVec2, to: Option<IVec2>) -> f32 {
        let d = to.map_or(0.0, |t| (t - from).as_vec2().length());
        1.0 / (1.0 + self.villagers.data.ai.distance_falloff * d)
    }

    /// Scores every option for one villager.
    pub fn candidates(&self, me: &Me) -> Vec<Candidate> {
        let ai = &self.villagers.data.ai;
        let needs_data = &self.villagers.data.needs;
        let phase = self.calendar.at(self.tick.0).phase;
        let home_tile = self.tile_of(me.villager.home);
        let at_home = home_tile == Some(me.tile);
        let settlement = self.settlements.get(me.villager.settlement).ok();
        let store = settlement.map(|s| s.storehouse);
        let store_tile = store.and_then(|s| self.tile_of(s));
        let store_has_food = store
            .and_then(|s| self.stock(s))
            .is_some_and(|s| s.has_food(&self.econ));
        let home_stock = self.stock(me.villager.home);
        let home_has_food = home_stock.is_some_and(|s| s.has_food(&self.econ));
        let carrying_food = me.inventory.contents().any(|(g, _)| self.econ.is_food(g));
        let fed = me.needs.hunger < needs_data.starving;
        let rested = me.needs.rest > 0.15;
        let mut out = Vec::with_capacity(7);

        // Eat: from what's carried, else at home, else at the storehouse.
        {
            let hunger = ai.eat.hunger.eval(me.needs.hunger);
            let (target, goal, source) = if carrying_food {
                (None, None, 1.0)
            } else if home_has_food {
                (Some(me.villager.home), home_tile, 1.0)
            } else if store_has_food {
                (store, store_tile, 1.0)
            } else {
                (None, None, 0.0)
            };
            let dist = self.distance_factor(me.tile, goal);
            out.push(Candidate {
                option: ScoredOption {
                    kind: ActionKind::Eat,
                    score: ai.eat.weight * hunger * source * dist,
                    factors: Factors::new(&[
                        ("hunger", hunger),
                        ("food available", source),
                        ("distance", dist),
                    ]),
                },
                target,
                dropoff: None,
                good: None,
                goal,
            });
        }

        // Fetch: adults restock the household from the storehouse.
        if me.adult {
            let residents = self
                .homes
                .get(me.villager.home)
                .map_or(1, |h| h.residents.len().max(1)) as f32;
            let home_value = home_stock.map_or(0.0, |s| s.food_value(&self.econ));
            let need =
                self.villagers.data.needs.hunger_per_day * residents * ai.fetch.home_stock_low;
            let low = if home_value < need { 1.0 } else { 0.0 };
            let avail = if store_has_food { 1.0 } else { 0.0 };
            let daytime = phase_factor(phase, 1.0, 0.6, 0.8, 0.0);
            let dist = self.distance_factor(me.tile, store_tile);
            out.push(Candidate {
                option: ScoredOption {
                    kind: ActionKind::Fetch,
                    score: ai.fetch.weight * low * avail * daytime * dist,
                    factors: Factors::new(&[
                        ("home stock low", low),
                        ("storehouse has food", avail),
                        ("time of day", daytime),
                        ("distance", dist),
                    ]),
                },
                target: store,
                dropoff: Some(me.villager.home),
                good: None,
                goal: store_tile,
            });
        }

        // Sleep at home.
        {
            let mut tired = ai.sleep.tiredness.eval(1.0 - me.needs.rest);
            if phase == Phase::Night {
                tired = tired.max(ai.sleep.night_drive);
            }
            let time = phase_factor(
                phase,
                ai.sleep.day,
                ai.sleep.day,
                ai.sleep.evening,
                ai.sleep.night,
            );
            let dist = self.distance_factor(me.tile, home_tile);
            out.push(Candidate {
                option: ScoredOption {
                    kind: ActionKind::Sleep,
                    score: ai.sleep.weight * tired * time * dist,
                    factors: Factors::new(&[
                        ("tiredness", tired),
                        ("time of day", time),
                        ("distance", dist),
                    ]),
                },
                target: Some(me.villager.home),
                dropoff: None,
                good: None,
                goal: home_tile,
            });
        }

        // Work (or haul, for carters).
        let w = &ai.work;
        let time = phase_factor(phase, w.morning, w.work, w.evening, w.night);
        // Hunger pulls villagers off work before it becomes desperate.
        let appetite = ai.eat.hunger.eval(me.needs.hunger);
        let fit = (1.0 - 0.6 * appetite)
            * (if fed { 1.0 } else { 0.2 })
            * (if rested { 1.0 } else { 0.2 });
        if me.occupation == Occupation::Carter {
            if let (Some(settlement), Some(store), Some(store_tile)) =
                (settlement, store, store_tile)
            {
                let (job, value) = self.haul_job(me, settlement, store, store_tile);
                let has_job = if job.is_some() { 1.0 } else { 0.0 };
                let goal = job.as_ref().and_then(|j| self.tile_of(j.0));
                let dist = self.distance_factor(me.tile, goal);
                out.push(Candidate {
                    option: ScoredOption {
                        kind: ActionKind::Haul,
                        score: w.weight * time * fit * has_job * dist,
                        factors: Factors::new(&[
                            ("time of day", time),
                            ("fit to work", fit),
                            ("goods waiting", has_job.min(value)),
                            ("distance", dist),
                        ]),
                    },
                    target: job.map(|j| j.0),
                    dropoff: job.map(|j| j.1),
                    good: job.map(|j| j.2),
                    goal,
                });
            }
        } else if let Some(workplace) = me.employment.workplace {
            let wp_tile = self.tile_of(workplace);
            // When the workplace is full or short of inputs, the worker
            // hauls for it instead of standing idle.
            let (kind, target, dropoff, good, possible) = match self.work_state(me, workplace) {
                WorkState::Ready => (ActionKind::Work, Some(workplace), None, None, 1.0),
                WorkState::Full(g) => (ActionKind::Haul, Some(workplace), store, Some(g), 1.0),
                WorkState::NeedsInput(g) => {
                    let in_store = store.and_then(|s| self.stock(s)).map_or(0, |s| s.get(g));
                    let ok = if in_store > 0 { 1.0 } else { 0.0 };
                    (ActionKind::Haul, store, Some(workplace), Some(g), ok)
                }
                WorkState::Impossible => (ActionKind::Work, Some(workplace), None, None, 0.0),
            };
            let goal = target.and_then(|t| self.tile_of(t)).or(wp_tile);
            let dist = self.distance_factor(me.tile, goal);
            out.push(Candidate {
                option: ScoredOption {
                    kind,
                    score: w.weight * time * fit * possible * dist,
                    factors: Factors::new(&[
                        ("time of day", time),
                        ("fit to work", fit),
                        ("work possible", possible),
                        ("distance", dist),
                    ]),
                },
                target,
                dropoff,
                good,
                goal,
            });
        }

        // Go home in the evening.
        {
            let g = &ai.go_home;
            let time = phase_factor(phase, g.day, g.day, g.evening, g.night);
            let away = if at_home { 0.0 } else { 1.0 };
            out.push(Candidate {
                option: ScoredOption {
                    kind: ActionKind::GoHome,
                    score: g.weight * time * away,
                    factors: Factors::new(&[("time of day", time), ("away from home", away)]),
                },
                target: Some(me.villager.home),
                dropoff: None,
                good: None,
                goal: home_tile,
            });
        }

        // Reactions to armies (SIM-13): fear comes from the safety need.
        let threat = self
            .threats
            .get(me.villager.settlement)
            .copied()
            .unwrap_or_default();
        let fear = 1.0 - me.needs.safety;
        let afraid = fear > 0.02 || threat.level > 0.0;
        if afraid && home_tile.is_some() {
            let f = ai.hide.fear.eval(fear);
            out.push(Candidate {
                option: ScoredOption {
                    kind: ActionKind::Hide,
                    score: ai.hide.weight * f,
                    factors: Factors::new(&[("fear", f)]),
                },
                target: Some(me.villager.home),
                dropoff: None,
                good: None,
                goal: home_tile,
            });
        }
        if afraid {
            let d = (me.tile - threat.from).as_vec2().length();
            let near = if threat.level > 0.0 && d <= ai.flee.radius {
                1.0
            } else {
                0.0
            };
            let f = ai.flee.fear.eval(fear);
            out.push(Candidate {
                option: ScoredOption {
                    kind: ActionKind::Flee,
                    score: ai.flee.weight * f * near,
                    factors: Factors::new(&[("fear", f), ("army close", near)]),
                },
                target: None,
                dropoff: None,
                good: None,
                goal: Some(self.flee_spot(me, threat.from)),
            });
        }
        if me.adult && threat.provoked_by.is_some() {
            let fight = &ai.fight;
            // A stable per-person spread: some are braver than others.
            let spread =
                ((me.id.0.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40) % 1001) as f32 / 1000.0 * 2.0
                    - 1.0;
            let courage = fight.courage.eval(me.morale + spread * fight.spread);
            let able = if me.morale >= fight.break_morale && fed {
                1.0
            } else {
                0.0
            };
            out.push(Candidate {
                option: ScoredOption {
                    kind: ActionKind::Fight,
                    score: fight.weight * courage * able,
                    factors: Factors::new(&[("courage", courage), ("able to fight", able)]),
                },
                target: None,
                dropoff: None,
                good: None,
                goal: None,
            });
        }

        // Idle nearby.
        out.push(Candidate {
            option: ScoredOption {
                kind: ActionKind::Idle,
                score: ai.idle.weight,
                factors: Factors::new(&[("baseline", 1.0)]),
            },
            target: None,
            dropoff: None,
            good: None,
            goal: Some(self.idle_spot(me, home_tile.unwrap_or(me.tile))),
        });
        out
    }

    /// Whether the villager can do a work cycle at its workplace.
    fn work_state(&self, me: &Me, workplace: Entity) -> WorkState {
        let Ok((_, stock, Some(wp), _)) = self.buildings.get(workplace) else {
            return WorkState::Impossible;
        };
        let Some(recipe) = wp.recipe.map(|r| self.econ.recipe(r)) else {
            return WorkState::Impossible;
        };
        let out_units: u32 = recipe.outputs.iter().map(|(_, q)| *q as u32).sum();
        // Haul once the workplace is well stocked, before it fills up.
        let well_stocked = stock.total() * 10 >= stock.capacity * 6;
        if (well_stocked || stock.room() < out_units) && me.inventory.room() >= MIN_LOAD as u16 / 2
        {
            // Full: carry the largest pile of output to the storehouse.
            let outputs: Vec<GoodId> = recipe.outputs.iter().map(|(g, _)| *g).collect();
            return match stock
                .goods
                .iter()
                .filter(|(g, _)| outputs.contains(g))
                .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            {
                Some((g, _)) => WorkState::Full(*g),
                None => WorkState::Impossible,
            };
        }
        for (g, q) in &recipe.inputs {
            if stock.get(*g) + (me.inventory.get(*g) as u32) < *q as u32 {
                return WorkState::NeedsInput(*g);
            }
        }
        WorkState::Ready
    }

    /// The best haul for a carter: (pickup, drop-off, good), and its value.
    fn haul_job(
        &self,
        me: &Me,
        settlement: &SettlementEconomy,
        store: Entity,
        store_tile: IVec2,
    ) -> (Option<(Entity, Entity, GoodId)>, f32) {
        let mut best: Option<(f32, (Entity, Entity, GoodId))> = None;
        let store_stock = self.stock(store);
        for &wp_entity in &settlement.workplaces {
            let Ok((building, stock, Some(wp), wp_id)) = self.buildings.get(wp_entity) else {
                continue;
            };
            if wp.kind == WorkplaceKind::Storehouse {
                continue;
            }
            let recipe = wp.recipe.map(|r| self.econ.recipe(r));
            let inputs: Vec<GoodId> = recipe
                .map(|r| r.inputs.iter().map(|(g, _)| *g).collect())
                .unwrap_or_default();
            // Spread carters over jobs with a stable per-pair jitter.
            let jitter = ((me.id.0.wrapping_mul(0x9E37_79B9) ^ wp_id.0.wrapping_mul(0x85EB_CA6B))
                % 9) as f32;
            let dist = (building.tile - me.tile).as_vec2().length()
                + (building.tile - store_tile).as_vec2().length();
            // Collect outputs.
            if let Some((good, qty)) = stock
                .goods
                .iter()
                .filter(|(g, _)| !inputs.contains(g))
                .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
                && *qty >= MIN_LOAD
            {
                // Food first: a village can live without ore, not without grain.
                let urgency = if self.econ.is_food(*good) { 3.0 } else { 1.0 };
                let value = *qty as f32 * urgency - 0.3 * dist + jitter;
                if best.as_ref().is_none_or(|(v, _)| value > *v) {
                    best = Some((value, (wp_entity, store, *good)));
                }
            }
            // Deliver inputs to workshops running low.
            if let Some(recipe) = recipe {
                for (good, q) in &recipe.inputs {
                    let have = stock.get(*good);
                    let in_store = store_stock.map_or(0, |s| s.get(*good));
                    if have < 4 * *q as u32 && in_store >= *q as u32 {
                        let value = 60.0 - 0.3 * dist + jitter;
                        if best.as_ref().is_none_or(|(v, _)| value > *v) {
                            best = Some((value, (store, wp_entity, *good)));
                        }
                    }
                }
            }
        }
        match best {
            Some((v, job)) => (Some(job), (v / 30.0).clamp(0.1, 1.0)),
            None => (None, 0.0),
        }
    }

    /// A passable tile `flee.distance` tiles from home, away from `from`.
    fn flee_spot(&self, me: &Me, from: IVec2) -> IVec2 {
        let home = self.tile_of(me.villager.home).unwrap_or(me.tile);
        let away = (home - from).as_vec2();
        let dir = if away.length_squared() > 0.0 {
            away.normalize()
        } else {
            bevy_math::Vec2::X
        };
        let dist = self.villagers.data.ai.flee.distance as f32;
        for step in 0..8 {
            let d = dist * (1.0 - step as f32 * 0.12);
            let p = home + (dir * d).round().as_ivec2();
            if self.map.in_bounds(p)
                && !matches!(self.map.terrain(p), Terrain::Water | Terrain::Mountain)
            {
                return p;
            }
        }
        home
    }

    fn idle_spot(&self, me: &Me, home: IVec2) -> IVec2 {
        let r = self.villagers.data.idle.wander_radius;
        let mut rng = self
            .seed
            .stream(rng_key("sim_agents::idle_spot") ^ me.id.0, self.tick.0);
        for _ in 0..8 {
            let p = home + IVec2::new(rng.random_range(-r..=r), rng.random_range(-r..=r));
            if self.map.in_bounds(p)
                && !matches!(self.map.terrain(p), Terrain::Water | Terrain::Mountain)
            {
                return p;
            }
        }
        home
    }
}

/// Picks the best candidate; the current action gets an inertia bonus.
pub fn choose(candidates: &[Candidate], current: Option<ActionKind>, inertia: f32) -> usize {
    let mut best = 0;
    let mut best_score = f32::MIN;
    for (i, c) in candidates.iter().enumerate() {
        let bonus = if Some(c.option.kind) == current {
            inertia
        } else {
            0.0
        };
        let s = c.option.score + bonus;
        // Strictly greater: ties keep the earlier (fixed-order) option.
        if s > best_score {
            best_score = s;
            best = i;
        }
    }
    best
}

#[allow(clippy::type_complexity)]
pub(crate) fn decide(
    world: AiWorld,
    mut agents: Query<
        (
            &SpawnId,
            &Villager,
            &Occupation,
            &Employment,
            &Needs,
            &Inventory,
            &Position,
            &CombatStats,
            &mut Activity,
            &mut Travel,
            &mut NextDecision,
            &mut DecisionTrace,
        ),
        Without<crate::components::Enlisted>,
    >,
) {
    let _span = tracing::info_span!("sim_agents::decide").entered();
    let tick = world.tick.0;
    let interval = world.villagers.data.ai.decision_interval;
    let inertia = world.villagers.data.ai.inertia;
    let days_per_year = world.calendar.days_per_year();
    let working = world.villagers.data.households.working_age;
    agents.par_iter_mut().for_each(
        |(
            id,
            villager,
            occupation,
            employment,
            needs,
            inventory,
            pos,
            combat,
            mut activity,
            mut travel,
            mut next,
            mut trace,
        )| {
            let done = activity.stage == Stage::Done;
            if tick < next.0 && !done {
                return;
            }
            if tick >= next.0 {
                next.0 = tick + interval;
            }
            let years = villager.age_days as u32 / days_per_year;
            let me = Me {
                id: *id,
                villager,
                occupation: *occupation,
                employment,
                needs,
                inventory,
                tile: pos.0,
                adult: years >= working.min,
                morale: combat.morale,
            };
            let candidates = world.candidates(&me);
            let current = (!done).then_some(activity.kind);
            let pick = choose(&candidates, current, inertia);

            let mut order: Vec<usize> = (0..candidates.len()).collect();
            order.sort_by(|&a, &b| {
                let (a, b) = (&candidates[a].option, &candidates[b].option);
                b.score.total_cmp(&a.score).then(a.kind.cmp(&b.kind))
            });
            trace.tick = tick;
            trace.options.clear();
            trace
                .options
                .extend(order.iter().take(3).map(|&i| candidates[i].option.clone()));

            let c = &candidates[pick];
            if Some(c.option.kind) == current {
                return;
            }
            *activity = Activity {
                kind: c.option.kind,
                target: c.target,
                dropoff: c.dropoff,
                good: c.good,
                stage: Stage::Travel,
                step: 0,
                until_tick: 0,
                started_tick: tick,
            };
            match c.goal {
                Some(goal) if goal != pos.0 => travel.go_to(goal),
                _ => travel.state = TravelState::Arrived,
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(kind: ActionKind, score: f32) -> Candidate {
        Candidate {
            option: ScoredOption {
                kind,
                score,
                factors: Factors::new(&[]),
            },
            target: None,
            dropoff: None,
            good: None,
            goal: None,
        }
    }

    #[test]
    fn choose_takes_the_highest_score() {
        let c = [
            cand(ActionKind::Eat, 0.2),
            cand(ActionKind::Sleep, 0.7),
            cand(ActionKind::Idle, 0.1),
        ];
        assert_eq!(choose(&c, None, 0.0), 1);
    }

    #[test]
    fn inertia_keeps_the_current_action_when_close() {
        let c = [cand(ActionKind::Eat, 0.5), cand(ActionKind::Work, 0.45)];
        assert_eq!(choose(&c, Some(ActionKind::Work), 0.08), 1);
        assert_eq!(choose(&c, Some(ActionKind::Work), 0.01), 0);
    }

    #[test]
    fn ties_keep_the_earlier_option() {
        let c = [cand(ActionKind::Eat, 0.5), cand(ActionKind::Sleep, 0.5)];
        assert_eq!(choose(&c, None, 0.0), 0);
    }
}
