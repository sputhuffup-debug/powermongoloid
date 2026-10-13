//! A scripted player for runs (GP-16, GP-21): reads the sim between ticks
//! and plays only through ordinary player commands, so a bot run records
//! and replays like a human's.
//!
//! M4 step 11: the bot plays to win. Every [`THINK_EVERY`] ticks the
//! [`Bot`] reads a snapshot of the run (settlements, its captains, the
//! rival's seat; every query sorted by SpawnId) and:
//! 1. answers every offer: boons and traits by a fixed score of their
//!    effects (food and arms early, captain slots, attack, defence and
//!    recruiting later), invitations accepted; a band for hire (GP-27) is
//!    hired once the champion stands within `hire.radius` of it and can
//!    pay with [`HIRE_RESERVE_DAYS`] of food left for everyone, else asked
//!    again next hour (it never declines; an unaffordable band lapses);
//! 2. keeps the champion's army (the main army) fed: supplies from owned
//!    settlements, asks independents, forages when nothing else is near;
//! 3. grows it: recruits in non-rival settlements (owned ones first, at
//!    most an eighth of a village per visit, each village rested half a
//!    day), takes arms from storehouses holding weapons: where it
//!    recruits, at a settlement within `ARMS_STOP` tiles (not the rival's)
//!    before its next conquest, and (secondary captains) at the settlement
//!    each holds;
//! 4. takes the nearest monuments nobody holds early on, answers food
//!    requests it can spare, and attacks weak settlements (independent or
//!    the rival's, never the seat) once it outnumbers their estimated
//!    militia by `CONQUEST_MARGIN` (1.6), and the seat's garrison too when
//!    the target or the straight march to it lies within `SEAT_BERTH` (80
//!    tiles) of the seat;
//! 5. appoints captains up to the slots, each garrisoning an owned
//!    settlement and recruiting there;
//! 6. marches on the rival's seat with every captain once its soldiers
//!    outnumber the seat's estimated defence (garrison plus militia,
//!    times the seat's defence bonus) by `ASSAULT_MARGIN` (1.5), or by
//!    less as the clock runs on (`assault_margin`: 1.25 from day 8, 1.0
//!    from day 16, 0.75 from day 24, any army from day 32): first to a
//!    staging point, then an `Attack` on the seat together.
//!
//! 7. governs once a game day (`Bot::govern`, LORDSHIP_DESIGN section
//!    12): Forge at the best smithy town while it has two days of food,
//!    Tithe at fed, loyal towns within `GOVERN_RANGE` of a captain, a
//!    Levy at one big loyal town at a time, never Ration or Feast. Only
//!    commands the sim would accept are sent.
//!
//! Tie-breaks draw from stream `sim_headless::bot` at the tick. Nothing in
//! the bot depends on wall time or hash order, so a recorded bot run
//! replays.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use rand::RngExt;
use sim_agents::{
    CombatStats, Edicts, FoodSecurity, Owner, SettlementLoyalty, Stance, Villager, Villagers,
};
use sim_core::rng::rng_key;
use sim_core::{
    Calendar, Cause, Checkpoint, Chronicle, ChronicleEntry, CommandKind, CommandLog, Dues,
    FactionId, GovernAct, LoggedCommand, OfferKind, Offers, OrderSpec, Position, RunOutcome,
    RunState, SimCommand, SimSeed, SimStepper, SimTick, SpawnId, Stat, WorkFocus,
    compute_state_hash,
};
use sim_economy::{Economy, GoodId, SettlementEconomy, Stockpile, Workplace, WorkplaceKind};
use sim_military::progression::Effect;
use sim_military::{
    Army, ArmyIndex, Captain, Equipment, Follower, Guard, Holder, Progression, RequestBoard,
    RequestKind, RivalState, RunProgress,
};
use sim_world::{Monument, Settlement};

use crate::{App, HeadlessConfig, headless_app};

/// How often the bot thinks (ticks): once a game hour.
pub const THINK_EVERY: u64 = 100;

/// The bot leaves monuments within this many tiles of a guard camp alone.
const CAMP_BERTH: f32 = 40.0;

/// The share of a settlement's people the bot expects to take up arms
/// when attacked, before it has seen one fight.
const MILITIA_SHARE: f32 = 0.12;

/// Attack a settlement once the army outnumbers its expected militia by
/// this much.
const CONQUEST_MARGIN: f32 = 1.6;

/// The seat's garrison counts against an attack whose target, or straight
/// march to it, comes within this many tiles of the seat: an army that
/// marches past the seat meets the garrison on the way (PROGRESS_BOT.md,
/// "Village food in a run").
const SEAT_BERTH: f32 = 80.0;

/// March on the seat once all captains' soldiers outnumber its expected
/// defence by this much (early in the run; see [`assault_margin`]).
const ASSAULT_MARGIN: f32 = 1.5;

/// The main army keeps growing to at least this many people.
const MIN_ARMY: u32 = 40;

/// M4a (SIM-64, weapons exist since the smithy chain): the main army turns
/// aside to arm at a settlement (not the rival's) within this many tiles
/// holding at least [`ARMS_MIN`] weapons before it looks for its next
/// conquest.
const ARMS_STOP: f32 = 80.0;

/// The fewest weapons in a storehouse worth a stop for the main army (a
/// neutral army takes half of them).
const ARMS_MIN: u32 = 4;

/// A secondary captain takes arms at the settlement it holds at most this
/// often (ticks): a neutral army takes half the stock each time.
const ARMS_REST: u64 = 600;

/// GP-27: hire a band only if, after paying, the baggage still holds this
/// many days of food for the main army and the newcomers.
pub const HIRE_RESERVE_DAYS: f32 = 0.5;

/// Tithes and levies reach captains within this many tiles (`orders::lands`);
/// the bot only sets them for towns inside it.
const GOVERN_RANGE: f32 = 60.0;

/// Forge needs this many days of food in the town, and is set back to
/// Balanced under [`FORGE_OFF_DAYS`].
const FORGE_DAYS: f32 = 2.0;
const FORGE_OFF_DAYS: f32 = 1.0;

/// Tithe needs more than this many days of food and this loyalty; it is
/// kept (hysteresis) down to [`TITHE_KEEP_DAYS`] and [`TITHE_KEEP_LOYALTY`].
const TITHE_DAYS: f32 = 1.5;
const TITHE_LOYALTY: f32 = 0.45;
const TITHE_KEEP_DAYS: f32 = 1.0;
const TITHE_KEEP_LOYALTY: f32 = 0.40;

/// A Levy needs a town of this many people and this loyalty (kept down to
/// [`LEVY_KEEP_LOYALTY`]).
const LEVY_POP: u32 = 100;
const LEVY_LOYALTY: f32 = 0.6;
const LEVY_KEEP_LOYALTY: f32 = 0.5;

/// Most Govern commands in one day.
const GOVERN_PER_DAY: usize = 4;

/// An owned town as `govern` sees it.
#[derive(Debug, Clone)]
struct Town {
    id: u64,
    entity: Entity,
    pop: u32,
    smithies: u32,
    days: f32,
    loyalty: f32,
    work: WorkFocus,
    dues: Dues,
    /// Whether a captain of the bot stands within [`GOVERN_RANGE`].
    near: bool,
}

/// The dial settings the bot wants, as (town, act), in SpawnId order. Pure:
/// the caller drops what the sim would refuse.
fn plan_govern(towns: &[Town]) -> Vec<(u64, GovernAct)> {
    let mut out = Vec::new();
    // The forge town: most smithies, then most people, then lowest id.
    let forge = towns
        .iter()
        .filter(|t| t.smithies > 0)
        .max_by_key(|t| (t.smithies, t.pop, std::cmp::Reverse(t.id)))
        .map(|t| t.id);
    // One levying town at a time: the sitting one while it holds, else the
    // biggest eligible.
    let sitting = towns.iter().find(|t| {
        t.dues == Dues::Levy
            && t.near
            && t.pop >= LEVY_POP
            && t.loyalty >= LEVY_KEEP_LOYALTY
            && t.days > TITHE_KEEP_DAYS
    });
    let levy = sitting.map(|t| t.id).or_else(|| {
        if towns.iter().any(|t| t.dues == Dues::Levy) {
            return None;
        }
        towns
            .iter()
            .filter(|t| {
                t.near && t.pop >= LEVY_POP && t.loyalty >= LEVY_LOYALTY && t.days > TITHE_DAYS
            })
            .max_by_key(|t| (t.pop, std::cmp::Reverse(t.id)))
            .map(|t| t.id)
    });
    for t in towns {
        let work = if Some(t.id) == forge {
            if t.days >= FORGE_DAYS {
                WorkFocus::Forge
            } else if t.work == WorkFocus::Forge && t.days < FORGE_OFF_DAYS {
                WorkFocus::Balanced
            } else {
                t.work
            }
        } else if t.work == WorkFocus::Forge {
            WorkFocus::Balanced
        } else {
            t.work
        };
        if work != t.work {
            out.push((t.id, GovernAct::Work(work)));
        }
        let tithe = if t.dues == Dues::Tithe {
            t.days > TITHE_KEEP_DAYS && t.loyalty >= TITHE_KEEP_LOYALTY
        } else {
            t.days > TITHE_DAYS && t.loyalty >= TITHE_LOYALTY
        };
        let dues = if Some(t.id) == levy {
            Dues::Levy
        } else if t.near && tithe {
            Dues::Tithe
        } else {
            Dues::Light
        };
        if dues != t.dues {
            out.push((t.id, GovernAct::Dues(dues)));
        }
    }
    out
}

/// The bot's own record of what each of its captains is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Task {
    /// Food from the land here.
    Forage,
    /// Walking to fresh ground, then foraging.
    Wander(IVec2),
    /// Food from a settlement (SpawnId).
    Gather(u64),
    /// Recruiting at a settlement, then asking it for food.
    Recruit(u64),
    /// Arms from a settlement's storehouse.
    Equip(u64),
    /// Bringing food to a settlement asking for it.
    Give(u64),
    /// Standing at a monument (SpawnId) to take it, since the tick.
    Monument(u64, u64),
    /// Taking a settlement by force.
    Attack(u64),
    /// Holding an owned settlement (secondary captains).
    Hold(u64),
    /// Waiting at the staging point before the assault.
    Stage(IVec2),
    /// The assault on the rival's seat.
    Assault(u64),
}

/// The assault's phase, shared by every captain.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Plan {
    /// Growing and taking land.
    Grow,
    /// Gathering at a staging point since the tick.
    Stage { at: IVec2, since: u64 },
    /// Attacking the seat since the tick.
    Strike { since: u64 },
}

/// The scripted player. See the module docs.
#[derive(Debug, Clone)]
pub struct Bot {
    seed: SimSeed,
    tasks: BTreeMap<u64, Task>,
    /// The tick by which each captain's task should be done.
    due: BTreeMap<u64, u64>,
    /// When each settlement was last recruited from.
    recruited: BTreeMap<u64, u64>,
    /// When a secondary captain last took arms at each settlement.
    armed: BTreeMap<u64, u64>,
    /// Failed attacks per settlement (raises its militia estimate).
    failed: BTreeMap<u64, u32>,
    /// Settlements a captain couldn't reach in time, left alone until the
    /// tick.
    shunned: BTreeMap<u64, u64>,
    /// The most militia seen standing at each settlement.
    seen: BTreeMap<u64, u32>,
    plan: Plan,
    /// The game day `govern` last ran.
    governed: Option<u64>,
    /// Captains (SpawnIds) the bot leaves to someone else.
    ignored: Vec<u64>,
    /// What the bot decided, with the tick, for reports.
    pub notes: Vec<(u64, String)>,
}

/// The outcome of [`run_bot`].
#[derive(Debug, Clone, PartialEq)]
pub struct BotRun {
    /// How the run ended, if it did.
    pub outcome: Option<RunOutcome>,
    /// Why it ended (`Cause::None` if it didn't).
    pub cause: Cause,
    /// The tick the bot stopped at (the run's end, or `max_ticks`).
    pub end_tick: u64,
    /// Every player command applied, as recorded for a replay.
    pub log: Vec<LoggedCommand>,
    /// State hashes every `checkpoint_every` ticks and at the end, as
    /// [`crate::run`] takes them: replaying [`BotRun::log`] for the same
    /// ticks gives the same checkpoints.
    pub checkpoints: Vec<Checkpoint>,
    /// The state hash at the end.
    pub final_hash: u64,
    /// The run's chronicle at the end.
    pub chronicle: Vec<ChronicleEntry>,
    /// What the bot decided ([`Bot::notes`]).
    pub notes: Vec<(u64, String)>,
}

/// A settlement as the bot sees it.
#[derive(Debug, Clone)]
struct Site {
    id: u64,
    center: IVec2,
    owner: FactionId,
    pop: u32,
    weapons: u32,
    /// Food value beyond a day for its people.
    spare: f32,
    /// Food value in its storehouse.
    food: f32,
    /// Its people standing as militia against the player now.
    militia: u32,
}

/// One of the bot's captains as it sees it.
#[derive(Debug, Clone)]
struct Cap {
    id: u64,
    at: IVec2,
    people: u32,
    unarmed: u32,
    days: f32,
    orders: usize,
    front: Option<OrderSpec>,
    champion: bool,
    /// The lowest-SpawnId follower (for appointing).
    first_follower: Option<u64>,
}

/// What the bot reads each time it thinks.
struct View {
    tick: u64,
    day: f32,
    sites: Vec<Site>,
    caps: Vec<Cap>,
    seat: Option<u64>,
    seat_center: IVec2,
    /// The rival garrison's people.
    garrison: u32,
    defence: f32,
    slots: u32,
    hostiles: Vec<(Vec2, u32)>,
}

impl View {
    fn read(world: &mut World, tick: u64, ignored: &[u64]) -> Self {
        let day = tick as f32 / world.resource::<Calendar>().ticks_per_day as f32;
        let econ = world.resource::<Economy>().clone();
        let weapons = good_named(&econ, "weapons");
        let per_day = world.resource::<Villagers>().data.needs.hunger_per_day;
        let mut q = world.query::<(&Villager, &CombatStats)>();
        let mut militia: BTreeMap<Entity, u32> = BTreeMap::new();
        for (v, cs) in q.iter(world) {
            if cs.stance
                == (Stance::Militia {
                    against: FactionId::PLAYER,
                })
                && cs.alive()
            {
                *militia.entry(v.settlement).or_default() += 1;
            }
        }
        let mut q = world.query::<(
            Entity,
            &SpawnId,
            &Settlement,
            &SettlementEconomy,
            Option<&Owner>,
        )>();
        let mut sites: Vec<Site> = q
            .iter(world)
            .filter(|(.., se, _)| se.population > 0)
            .map(|(e, id, s, se, o)| {
                let store = world.get::<Stockpile>(se.storehouse);
                let food = store.map_or(0.0, |st| st.food_value(&econ));
                Site {
                    id: id.0,
                    center: s.center,
                    owner: o.map_or(FactionId::INDEPENDENT, |o| o.faction),
                    pop: se.population,
                    weapons: weapons.and_then(|g| store.map(|st| st.get(g))).unwrap_or(0),
                    spare: food - per_day * se.population as f32,
                    food,
                    militia: militia.get(&e).copied().unwrap_or(0),
                }
            })
            .collect();
        sites.sort_unstable_by_key(|s| s.id);
        let champion = sim_military::champion_of(world, FactionId::PLAYER);
        let mut q = world.query::<(&Follower, &SpawnId, &Equipment)>();
        let mut members: BTreeMap<Entity, (u32, u32, Option<u64>)> = BTreeMap::new();
        for (f, id, eq) in q.iter(world) {
            let m = members.entry(f.captain).or_insert((0, 0, None));
            m.0 += 1;
            if !eq.weapon {
                m.1 += 1;
            }
            m.2 = Some(m.2.map_or(id.0, |x: u64| x.min(id.0)));
        }
        let mut q = world.query::<(Entity, &SpawnId, &Captain, &Position, Option<&Stockpile>)>();
        let mut caps: Vec<Cap> = q
            .iter(world)
            .filter(|(_, id, c, ..)| c.faction == FactionId::PLAYER && !ignored.contains(&id.0))
            .map(|(e, id, c, p, bag)| {
                let (n, unarmed, first) = members.get(&e).copied().unwrap_or((0, 0, None));
                let people = n + 1;
                let food = bag.map_or(0.0, |b| b.food_value(&econ));
                Cap {
                    id: id.0,
                    at: p.0,
                    people,
                    unarmed,
                    days: food / (per_day * people as f32).max(1e-3),
                    orders: c.orders.len(),
                    front: c.orders.front().map(|o| o.spec),
                    champion: Some(e) == champion,
                    first_follower: first,
                }
            })
            .collect();
        caps.sort_unstable_by_key(|c| c.id);
        let rival = world.resource::<RivalState>().clone();
        let garrison = sim_military::rival::garrison(world)
            .map_or(0, |g| members.get(&g).map_or(0, |m| m.0) + 1);
        let army = world.resource::<Army>().clone();
        let hostiles = world
            .resource::<ArmyIndex>()
            .armies
            .iter()
            .filter(|a| army.at_war(a.faction, FactionId::PLAYER))
            .map(|a| (a.pos, a.followers + 1))
            .collect();
        let slots = world.resource::<RunProgress>().captain_slots;
        Self {
            tick,
            day,
            sites,
            caps,
            seat: rival.seat,
            seat_center: rival.seat_center,
            garrison,
            defence: rival.defence_bonus,
            slots,
            hostiles,
        }
    }

    fn site(&self, id: u64) -> Option<&Site> {
        self.sites.iter().find(|s| s.id == id)
    }

    fn main(&self) -> Option<&Cap> {
        self.caps.iter().find(|c| c.champion)
    }

    /// People in armies at war with the player within `r` of `at`.
    fn hostiles_near(&self, at: IVec2, r: f32) -> u32 {
        self.hostiles
            .iter()
            .filter(|(p, _)| p.distance(at.as_vec2()) <= r)
            .map(|(_, n)| n)
            .sum()
    }
}

/// The margin the assault needs on `day`: `ASSAULT_MARGIN` (1.5) early, less
/// as the seat's defences grow and the clock runs out.
fn assault_margin(day: f32) -> f32 {
    if day >= 32.0 {
        0.0
    } else if day >= 24.0 {
        0.75
    } else if day >= 16.0 {
        1.0
    } else if day >= 8.0 {
        1.25
    } else {
        ASSAULT_MARGIN
    }
}

fn dist(a: IVec2, b: IVec2) -> f32 {
    a.as_vec2().distance(b.as_vec2())
}

/// How close the straight line from `a` to `b` comes to `p` (tiles).
fn dist_to_line(p: IVec2, a: IVec2, b: IVec2) -> f32 {
    let (p, a, b) = (p.as_vec2(), a.as_vec2(), b.as_vec2());
    let ab = b - a;
    let t = if ab == Vec2::ZERO {
        0.0
    } else {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    };
    p.distance(a + ab * t)
}

fn good_named(econ: &Economy, name: &str) -> Option<GoodId> {
    econ.data
        .goods
        .iter()
        .position(|g| g.name == name)
        .map(|i| GoodId(i as u8))
}

/// How much the bot wants a boon or trait with `effects` (higher first).
fn score(effects: &[Effect], day: f32) -> f32 {
    let early = if day < 3.0 { 1.0 } else { 0.3 };
    effects
        .iter()
        .map(|e| match *e {
            Effect::FoodDays(d) => d / 3.0 * early,
            Effect::ArmBand => 1.3 * early,
            Effect::Goodwill(g) => 2.0 * g,
            Effect::CaptainSlots(n) => 1.5 * n as f32,
            Effect::Sellswords(n) => n as f32 / 10.0,
            Effect::Stat(stat, x) => {
                x * match stat {
                    Stat::Attack => 25.0,
                    Stat::Defence => 22.0,
                    Stat::Morale => 14.0,
                    Stat::RecruitChance => 16.0,
                    Stat::MarchSpeed => 8.0,
                    Stat::FoodYield => 6.0,
                    Stat::LoyaltyDrift => 5.0,
                    Stat::ForageYield => 3.0,
                    Stat::VeteranRate => 2.0,
                    Stat::Desertion => -4.0,
                    Stat::CaptainSlots => 15.0,
                    _ => 1.0,
                }
            }
            _ => 0.0,
        })
        .sum()
}

impl Bot {
    /// A bot for the run of world seed `seed`.
    pub fn new(seed: u64) -> Self {
        Self {
            seed: SimSeed(seed),
            tasks: BTreeMap::new(),
            due: BTreeMap::new(),
            recruited: BTreeMap::new(),
            armed: BTreeMap::new(),
            failed: BTreeMap::new(),
            seen: BTreeMap::new(),
            shunned: BTreeMap::new(),
            plan: Plan::Grow,
            governed: None,
            ignored: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Leaves captain `id` (a SpawnId) alone: the bot neither orders nor
    /// counts it (a test drives it itself).
    pub fn ignore(&mut self, id: u64) {
        self.ignored.push(id);
    }

    /// Reads `app` between ticks and returns the player commands to send
    /// before the next one (none except every [`THINK_EVERY`] ticks).
    pub fn act(&mut self, app: &mut App) -> Vec<SimCommand> {
        let tick = app.world().resource::<SimTick>().0;
        if !tick.is_multiple_of(THINK_EVERY) || !app.world().resource::<RunState>().in_run() {
            return Vec::new();
        }
        let world = app.world_mut();
        let view = View::read(world, tick, &self.ignored);
        let mut out = self.offers(world, &view);
        if view.main().is_none() {
            return out;
        }
        if tick.is_multiple_of(THINK_EVERY * 24) {
            self.report(&view);
        }
        for s in &view.sites {
            if s.militia > 0 {
                let m = self.seen.entry(s.id).or_default();
                *m = (*m).max(s.militia);
            }
        }
        // Forget captains that are gone.
        self.tasks
            .retain(|id, _| view.caps.iter().any(|c| c.id == *id));
        // A task that drags on (a captain that can't reach its goal) is
        // dropped and thought again.
        for cap in &view.caps {
            if self.due.get(&cap.id).is_some_and(|t| tick >= *t) {
                self.due.remove(&cap.id);
                let task = self.tasks.remove(&cap.id);
                if let Some(
                    Task::Gather(s)
                    | Task::Recruit(s)
                    | Task::Equip(s)
                    | Task::Give(s)
                    | Task::Attack(s),
                ) = task
                {
                    self.shunned.insert(s, tick + 2400);
                }
                self.note(tick, format!("captain {}: gave up on {task:?}", cap.id));
            }
        }
        out.extend(self.govern(world, &view));
        self.update_plan(&view);
        if let Some(cmd) = self.appoint(&view) {
            out.push(cmd);
        }
        for cap in &view.caps {
            let task = if cap.champion {
                self.main_task(world, &view, cap)
            } else {
                self.holder_task(&view, cap)
            };
            out.extend(self.issue(world, &view, cap, task));
        }
        out
    }

    /// Once a game day (the first think after each dawn): sets the dials of
    /// the towns the player owns by [`plan_govern`], skipping anything the
    /// sim would refuse (a locked dial, no change).
    fn govern(&mut self, world: &mut World, v: &View) -> Vec<SimCommand> {
        let tpd = world.resource::<Calendar>().ticks_per_day;
        let day = v.tick / tpd;
        if self.governed == Some(day) {
            return Vec::new();
        }
        self.governed = Some(day);
        let mut q = world.query::<(
            Entity,
            &SpawnId,
            &Settlement,
            &SettlementEconomy,
            &Owner,
            &Edicts,
            &FoodSecurity,
            &SettlementLoyalty,
        )>();
        let mut towns: Vec<Town> = q
            .iter(world)
            .filter(|(.., o, _, _, _)| o.faction == FactionId::PLAYER)
            .map(|(e, id, s, se, _, ed, fs, sl)| Town {
                id: id.0,
                entity: e,
                pop: se.population,
                smithies: se
                    .workplaces
                    .iter()
                    .filter(|w| {
                        world
                            .get::<Workplace>(**w)
                            .is_some_and(|w| w.kind == WorkplaceKind::Smithy)
                    })
                    .count() as u32,
                days: fs.days,
                loyalty: sl.mean,
                work: ed.pending_work.unwrap_or(ed.work),
                dues: ed.pending_dues.unwrap_or(ed.dues),
                near: v.caps.iter().any(|c| dist(c.at, s.center) <= GOVERN_RANGE),
            })
            .collect();
        towns.sort_unstable_by_key(|t| t.id);
        let mut out = Vec::new();
        for (id, act) in plan_govern(&towns) {
            let Some(t) = towns.iter().find(|t| t.id == id) else {
                continue;
            };
            if out.len() >= GOVERN_PER_DAY
                || sim_military::govern::refusal(world, FactionId::PLAYER, t.entity, act).is_some()
            {
                continue;
            }
            self.note(v.tick, format!("govern {id}: {act:?}"));
            out.push(SimCommand::player(
                FactionId::PLAYER,
                CommandKind::Govern {
                    settlement: id,
                    act,
                },
            ));
        }
        out
    }

    fn note(&mut self, tick: u64, text: String) {
        self.notes.push((tick, text));
    }

    fn report(&mut self, v: &View) {
        let Some(main) = v.main() else { return };
        let held: Vec<&Site> = v
            .sites
            .iter()
            .filter(|s| s.owner == FactionId::PLAYER)
            .collect();
        let soldiers: u32 = v.caps.iter().map(|c| c.people).sum();
        let arms: u32 = v
            .sites
            .iter()
            .filter(|s| s.owner != FactionId::RIVAL)
            .map(|s| s.weapons)
            .sum();
        let yours: u32 = held.iter().map(|s| s.weapons).sum();
        let text = format!(
            "day {:.0}: main {} ({} unarmed, {:.1} d food), captains {} (slots {}), soldiers {}, held {} ({} people), seat defence est {:.0} (garrison {}, +{:.2}), plan {:?}, arms in stores {arms} ({yours} yours)",
            v.day,
            main.people,
            main.unarmed,
            main.days,
            v.caps.len(),
            v.slots,
            soldiers,
            held.len(),
            held.iter().map(|s| s.pop).sum::<u32>(),
            self.seat_defence(v),
            v.garrison,
            v.defence,
            self.plan
        );
        self.note(v.tick, text);
    }

    /// Step 1: every offer waiting for the player.
    fn offers(&mut self, world: &World, v: &View) -> Vec<SimCommand> {
        let data = world.resource::<Progression>();
        let mut out = Vec::new();
        for o in world.resource::<Offers>().for_faction(FactionId::PLAYER) {
            let choice = match o.kind {
                OfferKind::StartBoon | OfferKind::DominionBoon { .. } => best(&o.choices, |c| {
                    data.boon(c).map_or(0.0, |b| score(&b.effects, v.day))
                }),
                OfferKind::Trait { .. } => best(&o.choices, |c| {
                    data.trait_data(c).map_or(0.0, |t| {
                        let effects: Vec<Effect> =
                            t.effects.iter().map(|&(s, x)| Effect::Stat(s, x)).collect();
                        score(&effects, v.day)
                    })
                }),
                // Accept lordship.
                OfferKind::Invitation { .. } => 0,
                // GP-27: hire a band the main army can reach and feed; ask
                // again next hour.
                OfferKind::Hire {
                    captain,
                    men,
                    price,
                } => {
                    if !can_hire(world, v, captain, men, price) {
                        continue;
                    }
                    0
                }
            };
            out.push(SimCommand::player(
                FactionId::PLAYER,
                CommandKind::ChooseOffer {
                    offer: o.id,
                    choice,
                },
            ));
        }
        for c in &out {
            if let CommandKind::ChooseOffer { offer, choice } = c.kind {
                self.notes
                    .push((v.tick, format!("offer {offer}: choice {choice}")));
            }
        }
        out
    }

    /// Whether the bot leaves settlement `s` alone for now.
    fn shuns(&self, s: u64, tick: u64) -> bool {
        self.shunned.get(&s).is_some_and(|t| tick < *t)
    }

    /// What the seat's defence is worth in soldiers: the garrison and the
    /// seat's militia, times its defence bonus.
    fn seat_defence(&self, v: &View) -> f32 {
        let militia = v
            .seat
            .and_then(|s| v.site(s))
            .map_or(0.0, |s| self.militia(s));
        (v.garrison as f32 + militia) * (1.0 + v.defence)
    }

    /// The militia the bot expects at `s`.
    fn militia(&self, s: &Site) -> f32 {
        let fails = self.failed.get(&s.id).copied().unwrap_or(0);
        let seen = self.seen.get(&s.id).copied().unwrap_or(0) as f32;
        (s.pop as f32 * MILITIA_SHARE).max(seen) * 1.5f32.powi(fails as i32)
    }

    /// Step 6: grow until strong enough, then stage, then strike.
    fn update_plan(&mut self, v: &View) {
        let Some(seat) = v.seat else { return };
        let soldiers: u32 = v
            .caps
            .iter()
            .filter(|c| c.people >= 6 || c.champion)
            .map(|c| c.people)
            .sum();
        let need = self.seat_defence(v) * assault_margin(v.day);
        match self.plan {
            Plan::Grow => {
                let fed = v.main().is_some_and(|m| m.days >= 1.5);
                // A starving army strikes rather than wastes away.
                let starving = v.main().is_some_and(|m| m.days < 0.4);
                if soldiers as f32 >= need && (fed || v.day >= 32.0)
                    || starving && soldiers as f32 >= need * 0.7
                {
                    let at = self.staging(v);
                    self.note(
                        v.tick,
                        format!(
                            "assault: {soldiers} soldiers against an estimated {:.0} (need {need:.0}); staging at {at}",
                            self.seat_defence(v)
                        ),
                    );
                    self.plan = Plan::Stage { at, since: v.tick };
                }
            }
            Plan::Stage { at, since } => {
                let ready = v
                    .caps
                    .iter()
                    .filter(|c| c.people >= 6 || c.champion)
                    .all(|c| dist(c.at, at) <= 40.0);
                let waited = v.tick - since;
                if ready || waited >= 1000 {
                    self.note(
                        v.tick,
                        format!(
                            "strike on the seat #{seat} ({soldiers} soldiers, garrison {})",
                            v.garrison
                        ),
                    );
                    self.plan = Plan::Strike { since: v.tick };
                }
            }
            Plan::Strike { since } => {
                // An assault that is going nowhere: regroup.
                let left = soldiers as f32;
                if v.tick - since >= 2 * 2400 || left < need * 0.4 && v.day < 34.0 {
                    let seen = self.seen.get(&seat).copied().unwrap_or(0);
                    self.note(
                        v.tick,
                        format!("assault called off ({left} soldiers left, seat militia seen {seen}, garrison {})", v.garrison),
                    );
                    self.failed.entry(seat).and_modify(|f| *f += 1).or_insert(1);
                    self.plan = Plan::Grow;
                }
            }
        }
    }

    /// A tile about 70 tiles from the seat, toward the main army.
    fn staging(&self, v: &View) -> IVec2 {
        let from = v.main().map_or(v.seat_center, |m| m.at).as_vec2();
        let seat = v.seat_center.as_vec2();
        let dir = (from - seat).normalize_or(Vec2::X);
        (seat + dir * 70.0).round().as_ivec2()
    }

    /// Step 5: appoint a captain when a slot is free and the main army is big
    /// enough to split.
    fn appoint(&mut self, v: &View) -> Option<SimCommand> {
        if self.plan != Plan::Grow {
            return None;
        }
        let main = v.main()?;
        let others = v.caps.len() as u32 - 1;
        let held = v
            .sites
            .iter()
            .filter(|s| s.owner == FactionId::PLAYER)
            .count() as u32;
        // The first captain early (an heir if the champion falls), later
        // ones only from a big army.
        let big = if others == 0 { 24 } else { 40 };
        if others >= v.slots || others >= held || main.people < big || main.days < 1.0 {
            return None;
        }
        // Not in the middle of a fight or an attack.
        if matches!(main.front, Some(OrderSpec::Attack { .. })) {
            return None;
        }
        let follower = main.first_follower?;
        self.note(v.tick, format!("appoint {follower}"));
        Some(SimCommand::player(
            FactionId::PLAYER,
            CommandKind::AppointCaptain { follower },
        ))
    }

    /// Whether `task` is still under way for `cap`.
    fn going(&self, v: &View, cap: &Cap, task: Task) -> bool {
        match task {
            // Until it is held (checked by the caller) or overdue.
            Task::Monument(..) => true,
            Task::Hold(s) => v.site(s).is_some_and(|s| s.owner == FactionId::PLAYER),
            Task::Stage(_) => matches!(self.plan, Plan::Stage { .. }),
            _ => cap.orders > 0,
        }
    }

    /// Steps 2-6 for the champion's army.
    fn main_task(&mut self, world: &mut World, v: &View, cap: &Cap) -> Task {
        let current = self.tasks.get(&cap.id).copied();
        match self.plan {
            Plan::Stage { at, .. } => {
                if cap.days < 1.0 && current.is_some_and(|t| matches!(t, Task::Gather(_))) {
                    return current.unwrap();
                }
                return Task::Stage(at);
            }
            Plan::Strike { .. } => return Task::Assault(v.seat.unwrap_or(0)),
            Plan::Grow => {}
        }
        // An attack or a monument under way goes on; anything else gives
        // way to hunger.
        if let Some(t) = current {
            match t {
                Task::Attack(_) if cap.orders > 0 && cap.days > 0.3 => return t,
                Task::Attack(s) if cap.orders == 0 => {
                    let taken = v.site(s).is_some_and(|s| s.owner == FactionId::PLAYER);
                    let pop = v.site(s).map_or(0, |s| s.pop);
                    let seen = self.seen.get(&s).copied().unwrap_or(0);
                    if !taken {
                        self.failed.entry(s).and_modify(|f| *f += 1).or_insert(1);
                        self.note(
                            v.tick,
                            format!(
                                "attack on #{s} failed (pop {pop}, militia {seen}, army {})",
                                cap.people
                            ),
                        );
                    } else {
                        self.note(
                            v.tick,
                            format!("took #{s} (pop {pop}, militia {seen}, army {})", cap.people),
                        );
                    }
                }
                Task::Monument(m, _) => {
                    if monument_holder(world, m) == Some(FactionId::PLAYER) {
                        self.note(v.tick, format!("holds monument #{m}"));
                    } else if self.going(v, cap, t) && cap.days >= 0.5 {
                        return t;
                    }
                }
                Task::Forage | Task::Wander(_) if cap.orders > 0 && cap.days < 2.0 => return t,
                // Waiting is re-thought every hour.
                Task::Hold(_) => {}
                _ if self.going(v, cap, t) && cap.days >= 0.7 => return t,
                _ => {}
            }
        }
        // Food.
        if cap.days < 1.2 {
            return self.food(v, cap, current);
        }
        // Recruits: enough for the assault (with the early margin),
        // counting the other captains; a small army grows before it
        // fights.
        let others: u32 = v
            .caps
            .iter()
            .filter(|c| !c.champion)
            .map(|c| c.people)
            .sum();
        let target = ((self.seat_defence(v) * ASSAULT_MARGIN * 1.1) as u32).max(MIN_ARMY);
        let recruit = if cap.people + others < target && (cap.days >= 1.5 || cap.people < 8) {
            self.recruit_site(v, cap)
        } else {
            None
        };
        if let Some(s) = recruit.filter(|_| cap.people < 30) {
            return Task::Recruit(s);
        }
        // Arms close at hand, before the next fight.
        if let Some(s) = self.arms_stop(v, cap) {
            return Task::Equip(s);
        }
        // Weak settlements nearby.
        if let Some(s) = self.conquest_target(v, cap) {
            return Task::Attack(s);
        }
        if let Some(s) = recruit {
            return Task::Recruit(s);
        }
        // Food requests it can spare.
        let keep = world.resource::<Army>().data.give_keep_days;
        let board = world.resource::<RequestBoard>().clone();
        let give = board
            .open
            .iter()
            .filter_map(|r| match r.kind {
                RequestKind::BringFood { need, delivered }
                    if delivered < need && cap.days >= keep + 1.0 =>
                {
                    let s = v.site(r.settlement)?;
                    let d = dist(s.center, cap.at);
                    (d <= 150.0).then_some((d, s.id))
                }
                _ => None,
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if let Some((_, s)) = give {
            return Task::Give(s);
        }
        // Arms.
        if cap.unarmed >= 5 {
            let mut best: Option<(f32, u64)> = None;
            for s in &v.sites {
                if s.owner == FactionId::RIVAL || s.weapons < 4 || self.shuns(s.id, v.tick) {
                    continue;
                }
                let d = dist(s.center, cap.at)
                    - if s.owner == FactionId::PLAYER {
                        50.0
                    } else {
                        0.0
                    };
                if dist(s.center, cap.at) <= 250.0 && best.is_none_or(|b| d < b.0) {
                    best = Some((d, s.id));
                }
            }
            if let Some((_, s)) = best {
                return Task::Equip(s);
            }
        }
        // Monuments nobody holds, early on.
        if v.day < 12.0
            && let Some(m) = nearest_monument(world, cap.at, 200.0)
        {
            return Task::Monument(m, v.tick);
        }
        if cap.days < 3.0 {
            return self.food(v, cap, current);
        }
        // Wait at the nearest owned settlement.
        v.sites
            .iter()
            .filter(|s| s.owner == FactionId::PLAYER)
            .min_by(|a, b| dist(a.center, cap.at).total_cmp(&dist(b.center, cap.at)))
            .map_or(Task::Forage, |s| Task::Hold(s.id))
    }

    /// Food for `cap`: an owned settlement's spare, an independent's, or
    /// the land.
    fn food(&mut self, v: &View, cap: &Cap, current: Option<Task>) -> Task {
        let need = (2.0 - cap.days).max(0.5) * cap.people as f32 * 0.5;
        let mut best: Option<(f32, u64)> = None;
        let starving = cap.days < 0.5;
        for s in &v.sites {
            if s.owner == FactionId::RIVAL || self.shuns(s.id, v.tick) {
                continue;
            }
            // A hungry neutral army takes a day's food even from the
            // reserve.
            let enough = if starving {
                s.food >= 10.0
            } else {
                s.spare >= need.min(20.0)
            };
            if !enough {
                continue;
            }
            let d = dist(s.center, cap.at);
            let range = if starving { 200.0 } else { 300.0 };
            let d = d - if s.owner == FactionId::PLAYER {
                60.0
            } else {
                0.0
            };
            if dist(s.center, cap.at) <= range && best.is_none_or(|b| d < b.0) {
                best = Some((d, s.id));
            }
        }
        if let Some((_, s)) = best {
            return Task::Gather(s);
        }
        if current == Some(Task::Forage) {
            // The last spot ran bare: fresh ground 48 tiles away.
            let mut rng = self
                .seed
                .stream(rng_key("sim_headless::bot") ^ cap.id, v.tick);
            let a = rng.random_range(0.0..std::f32::consts::TAU);
            let to = (cap.at.as_vec2() + Vec2::from_angle(a) * 48.0)
                .round()
                .as_ivec2();
            return Task::Wander(to);
        }
        Task::Forage
    }

    /// A settlement not the rival's within [`ARMS_STOP`] tiles whose
    /// storehouse holds at least [`ARMS_MIN`] weapons, nearest first, when
    /// at least five of `cap`'s men are unarmed and it is fed for the walk.
    /// (A neutral army takes half the arms, unpaid and without provoking
    /// anyone, at an independent settlement too.)
    fn arms_stop(&self, v: &View, cap: &Cap) -> Option<u64> {
        if cap.unarmed < 5 || cap.days < 1.5 {
            return None;
        }
        v.sites
            .iter()
            .filter(|s| {
                s.owner != FactionId::RIVAL
                    && s.weapons >= ARMS_MIN
                    && !self.shuns(s.id, v.tick)
                    && dist(s.center, cap.at) <= ARMS_STOP
            })
            .min_by(|a, b| {
                dist(a.center, cap.at)
                    .total_cmp(&dist(b.center, cap.at))
                    .then(a.id.cmp(&b.id))
            })
            .map(|s| s.id)
    }

    /// The nearest settlement the army can take (not the seat, not one
    /// the player owns).
    fn conquest_target(&self, v: &View, cap: &Cap) -> Option<u64> {
        if cap.days < 1.5 || cap.people < 20 {
            return None;
        }
        let mut best: Option<(f32, u64)> = None;
        for s in &v.sites {
            if s.owner == FactionId::PLAYER || Some(s.id) == v.seat || self.shuns(s.id, v.tick) {
                continue;
            }
            let d = dist(s.center, cap.at);
            if d > 260.0 {
                continue;
            }
            // The garrison meets an army at the target or on the way past
            // the seat.
            let near_seat = dist_to_line(v.seat_center, cap.at, s.center) < SEAT_BERTH;
            let foes = self.militia(s)
                + v.hostiles_near(s.center, 45.0) as f32
                + if near_seat { v.garrison as f32 } else { 0.0 };
            if (cap.people as f32) < foes * CONQUEST_MARGIN + 5.0 {
                continue;
            }
            // Big settlements are worth a longer walk.
            let score = d - s.pop as f32 * 0.3;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, s.id));
            }
        }
        best.map(|b| b.1)
    }

    /// The best settlement to recruit in: owned or independent, rested,
    /// near.
    fn recruit_site(&self, v: &View, cap: &Cap) -> Option<u64> {
        let rest = 1200;
        let mut best: Option<(f32, u64)> = None;
        for s in &v.sites {
            if s.owner == FactionId::RIVAL || s.pop < 30 || self.shuns(s.id, v.tick) {
                continue;
            }
            if self.recruited.get(&s.id).is_some_and(|t| v.tick < t + rest) {
                continue;
            }
            let d = dist(s.center, cap.at);
            if d > 220.0 || v.hostiles_near(s.center, 45.0) > cap.people / 2 {
                continue;
            }
            let score = d - if s.owner == FactionId::PLAYER {
                60.0
            } else {
                0.0
            };
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, s.id));
            }
        }
        best.map(|b| b.1)
    }

    /// Secondary captains: hold an owned settlement, recruit there, join
    /// the assault.
    fn holder_task(&mut self, v: &View, cap: &Cap) -> Task {
        match self.plan {
            Plan::Stage { at, .. } if cap.people >= 6 => return Task::Stage(at),
            Plan::Strike { .. } if cap.people >= 6 => {
                return Task::Assault(v.seat.unwrap_or(0));
            }
            _ => {}
        }
        let current = self.tasks.get(&cap.id).copied();
        if let Some(Task::Hold(s)) = current
            && self.going(v, cap, Task::Hold(s))
        {
            // Recruit at home when small, at most every half day.
            let home = v.site(s).expect("held");
            let rested = self.recruited.get(&s).is_none_or(|t| v.tick >= t + 1200);
            if cap.people < 16 && home.pop >= 60 && rested && cap.orders <= 1 {
                return Task::Recruit(s);
            }
            // Arms from home's storehouse for the unarmed (M4a, SIM-64).
            let rested = self.armed.get(&s).is_none_or(|t| v.tick >= t + ARMS_REST);
            if cap.unarmed > 0 && home.weapons >= 2 && rested && cap.orders <= 1 {
                self.armed.insert(s, v.tick);
                return Task::Equip(s);
            }
            return Task::Hold(s);
        }
        if let Some(t @ (Task::Recruit(s) | Task::Equip(s))) = current {
            return match cap.front {
                Some(
                    OrderSpec::Recruit { .. }
                    | OrderSpec::GatherFood { .. }
                    | OrderSpec::Equip { .. },
                ) => t,
                _ => Task::Hold(s),
            };
        }
        // An owned settlement no other captain holds, nearest first.
        // (A secondary captain taking arms at home still holds it.)
        let main = v.main().map(|m| m.id);
        let held: Vec<u64> = self
            .tasks
            .iter()
            .filter(|(id, _)| **id != cap.id)
            .filter_map(|(id, t)| match t {
                Task::Hold(s) | Task::Recruit(s) => Some(*s),
                Task::Equip(s) if Some(*id) != main => Some(*s),
                _ => None,
            })
            .collect();
        v.sites
            .iter()
            .filter(|s| s.owner == FactionId::PLAYER && !held.contains(&s.id))
            .min_by(|a, b| {
                dist(a.center, cap.at)
                    .total_cmp(&dist(b.center, cap.at))
                    .then(a.id.cmp(&b.id))
            })
            .map_or_else(
                || match current {
                    Some(Task::Hold(s)) => Task::Hold(s),
                    _ => self.food_or_wait(v, cap),
                },
                |s| Task::Hold(s.id),
            )
    }

    fn food_or_wait(&self, v: &View, cap: &Cap) -> Task {
        // Nothing to hold: follow the champion.
        match v.main() {
            Some(m) if dist(m.at, cap.at) > 20.0 => Task::Stage(m.at),
            _ => Task::Forage,
        }
    }

    /// Orders for `task` when it is new for `cap`, or when the captain
    /// stopped short of it.
    fn issue(&mut self, world: &mut World, v: &View, cap: &Cap, task: Task) -> Vec<SimCommand> {
        let prev = self.tasks.insert(cap.id, task);
        let fresh = prev != Some(task);
        if fresh {
            // The walk there at about 8 ticks a tile, half again, plus the
            // work itself; foraging, holding and staging end on their own.
            let goal = match task {
                Task::Gather(s)
                | Task::Recruit(s)
                | Task::Equip(s)
                | Task::Give(s)
                | Task::Attack(s)
                | Task::Assault(s) => v.site(s).map(|s| s.center),
                Task::Wander(at) => Some(at),
                Task::Monument(m, _) => monument_tile(world, m),
                Task::Forage | Task::Hold(_) | Task::Stage(_) => None,
            };
            let work = match task {
                Task::Attack(_) | Task::Assault(_) => 3600,
                Task::Monument(..) => 600,
                _ => 300,
            };
            match goal {
                Some(at) => {
                    let walk = (dist(at, cap.at) * 12.0) as u64;
                    self.due.insert(cap.id, v.tick + walk + work);
                }
                None => {
                    self.due.remove(&cap.id);
                }
            }
        }
        let idle = cap.orders == 0;
        let at_center = |s: u64, r: f32| v.site(s).is_some_and(|s| dist(s.center, cap.at) <= r);
        let reissue = match task {
            Task::Hold(s) => idle && !at_center(s, 10.0),
            Task::Stage(at) => idle && dist(at, cap.at) > 15.0,
            Task::Monument(..) => false,
            Task::Assault(_) => idle,
            _ => false,
        };
        if !fresh && !reissue {
            return Vec::new();
        }
        let forage = OrderSpec::Forage {
            radius: world.resource::<Progression>().data.forage.default_radius,
        };
        let orders = match task {
            Task::Forage => vec![forage],
            Task::Wander(to) => {
                let to = passable(world, to).unwrap_or(cap.at);
                vec![OrderSpec::MoveTo { x: to.x, y: to.y }, forage]
            }
            Task::Gather(s) => vec![OrderSpec::GatherFood { settlement: s }],
            Task::Recruit(s) => {
                let pop = v.site(s).map_or(0, |s| s.pop);
                self.recruited.insert(s, v.tick);
                // Keep at least a day and a half of food for everyone.
                let fed = (cap.people as f32 * (cap.days / 1.5 - 1.0)).max(2.0) as u32;
                let count = (pop / 8).min(fed).clamp(2, 20) as u16;
                let mut o = vec![OrderSpec::Recruit {
                    settlement: s,
                    count,
                }];
                // The recruits come unarmed: arm them there (M4a, SIM-64).
                if v.site(s).is_some_and(|s| s.weapons >= 2) {
                    o.push(OrderSpec::Equip { settlement: s });
                }
                o.push(OrderSpec::GatherFood { settlement: s });
                if !cap.champion {
                    o.push(OrderSpec::Garrison { settlement: s });
                }
                o
            }
            Task::Equip(s) => vec![OrderSpec::Equip { settlement: s }],
            Task::Give(s) => vec![OrderSpec::Give { settlement: s }],
            Task::Monument(m, _) => match monument_tile(world, m) {
                Some(at) => vec![OrderSpec::MoveTo { x: at.x, y: at.y }],
                None => Vec::new(),
            },
            Task::Attack(s) | Task::Assault(s) => vec![OrderSpec::Attack { settlement: s }],
            Task::Hold(s) => vec![OrderSpec::Garrison { settlement: s }],
            Task::Stage(at) => {
                let to = passable(world, at).unwrap_or(at);
                vec![OrderSpec::MoveTo { x: to.x, y: to.y }]
            }
        };
        if fresh {
            self.note(v.tick, format!("captain {}: {task:?}", cap.id));
        }
        orders
            .into_iter()
            .enumerate()
            .map(|(i, order)| {
                SimCommand::player(
                    FactionId::PLAYER,
                    CommandKind::IssueOrder {
                        captain: cap.id,
                        order,
                        queue: i > 0,
                    },
                )
            })
            .collect()
    }
}

/// GP-27: whether the main army can hire the band led by `captain` (a
/// SpawnId; `men` for `price` food value) now: the champion within
/// progression.ron `hire.radius` of the band's captain, with the price in
/// its baggage and [`HIRE_RESERVE_DAYS`] of food left for everyone after.
fn can_hire(world: &World, v: &View, captain: u64, men: u16, price: u16) -> bool {
    let Some(main) = v.main() else {
        return false;
    };
    let Some(at) = world
        .try_query::<(&SpawnId, &Captain, &Position)>()
        .and_then(|mut q| {
            q.iter(world)
                .find(|(id, ..)| id.0 == captain)
                .map(|(.., p)| p.0)
        })
    else {
        return false;
    };
    if dist(main.at, at) > world.resource::<Progression>().data.hire.radius {
        return false;
    }
    let per_day = world.resource::<Villagers>().data.needs.hunger_per_day;
    let food = main.days * per_day * main.people as f32;
    let people = (main.people + men as u32) as f32;
    food >= price as f32 + per_day * people * HIRE_RESERVE_DAYS
}

/// The index of the choice scoring highest (the first on ties).
fn best(choices: &[u16], score: impl Fn(u16) -> f32) -> u8 {
    let mut best = (0usize, f32::NEG_INFINITY);
    for (i, &c) in choices.iter().enumerate() {
        let s = score(c);
        if s > best.1 {
            best = (i, s);
        }
    }
    best.0 as u8
}

/// The nearest monument within `range` nobody holds, away from guard
/// camps.
fn nearest_monument(world: &mut World, at: IVec2, range: f32) -> Option<u64> {
    let mut q = world.query_filtered::<&Position, (With<Captain>, With<Guard>)>();
    let camps: Vec<Vec2> = q.iter(world).map(|p| p.0.as_vec2()).collect();
    let mut q = world.query::<(&SpawnId, &Position, &Holder)>();
    q.iter(world)
        .filter(|(_, p, h)| {
            h.faction == FactionId::INDEPENDENT
                && camps.iter().all(|c| c.distance(p.0.as_vec2()) > CAMP_BERTH)
        })
        .map(|(id, p, _)| (dist(p.0, at), id.0))
        .filter(|(d, _)| *d <= range)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id)| id)
}

fn monument_holder(world: &mut World, id: u64) -> Option<FactionId> {
    let mut q = world.query::<(&SpawnId, &Holder)>();
    q.iter(world)
        .find(|(s, _)| s.0 == id)
        .map(|(_, h)| h.faction)
}

fn passable(world: &World, at: IVec2) -> Option<IVec2> {
    let costs = &world.resource::<Villagers>().travel_costs;
    sim_military::nearest_passable(world.resource::<sim_world::WorldMap>(), costs, at)
}

fn monument_tile(world: &mut World, id: u64) -> Option<IVec2> {
    let mut q = world.query_filtered::<(&SpawnId, &Position), With<Monument>>();
    q.iter(world).find(|(s, _)| s.0 == id).map(|(_, p)| p.0)
}

/// The bot's commands for this tick, sent, then one tick.
pub fn step(bot: &mut Bot, app: &mut App) {
    for command in bot.act(app) {
        app.world_mut().write_message(command);
    }
    app.step(1);
}

/// GP-16, GP-21: plays a run with a fresh [`Bot`] until the run ends or
/// `max_ticks` ticks have passed, taking a state-hash checkpoint every
/// `checkpoint_every` ticks (0: only at the end). Its commands are
/// ordinary recorded player commands: replaying [`BotRun::log`] (e.g. with
/// [`crate::record_replay`]) reproduces the run and its checkpoints.
pub fn run_bot(config: HeadlessConfig, max_ticks: u64, checkpoint_every: u64) -> BotRun {
    play_bot(config, max_ticks, checkpoint_every).1
}

/// [`run_bot`], also handing back the app as the run left it (for its
/// chronicle and summary, UI-09).
pub fn play_bot(config: HeadlessConfig, max_ticks: u64, checkpoint_every: u64) -> (App, BotRun) {
    play_bot_with(config, max_ticks, checkpoint_every, |_| {})
}

/// [`play_bot`], calling `after_tick` on the app after every tick (tests
/// watch the run as it goes, e.g. the director's quiet limit).
pub fn play_bot_with(
    config: HeadlessConfig,
    max_ticks: u64,
    checkpoint_every: u64,
    mut after_tick: impl FnMut(&mut App),
) -> (App, BotRun) {
    let mut app = headless_app(config);
    let mut bot = Bot::new(config.seed);
    let mut checkpoints = Vec::new();
    while app.world().resource::<SimTick>().0 < max_ticks
        && app.world().resource::<RunState>().outcome.is_none()
    {
        step(&mut bot, &mut app);
        after_tick(&mut app);
        let tick = app.world().resource::<SimTick>().0;
        let ended = app.world().resource::<RunState>().outcome.is_some();
        if tick == max_ticks
            || ended
            || (checkpoint_every > 0 && tick.is_multiple_of(checkpoint_every))
        {
            checkpoints.push(Checkpoint {
                tick,
                hash: compute_state_hash(app.world_mut()),
            });
        }
    }
    let final_hash = compute_state_hash(app.world_mut());
    let run = BotRun {
        outcome: app.world().resource::<RunState>().outcome,
        cause: app.world().resource::<RunState>().cause,
        end_tick: app.world().resource::<SimTick>().0,
        log: app.world().resource::<CommandLog>().0.clone(),
        checkpoints,
        final_hash,
        chronicle: app.world().resource::<Chronicle>().entries().to_vec(),
        notes: bot.notes,
    };
    (app, run)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn town(id: u64) -> Town {
        Town {
            id,
            entity: Entity::PLACEHOLDER,
            pop: 60,
            smithies: 0,
            days: 3.0,
            loyalty: 0.7,
            work: WorkFocus::Balanced,
            dues: Dues::Light,
            near: true,
        }
    }

    /// `govern` forges at the town with the most smithies, tithes the fed
    /// loyal ones near a captain, and levies at only one big town.
    #[test]
    fn govern_picks_the_smithy_town_and_one_levy() {
        let mut a = town(1);
        a.smithies = 1;
        let mut b = town(2);
        b.smithies = 2;
        b.pop = 120;
        let mut c = town(3);
        c.pop = 150;
        c.near = false;
        let mut d = town(4);
        d.pop = 140;
        d.days = 1.0;
        let plan = plan_govern(&[a, b, c, d]);
        assert!(plan.contains(&(2, GovernAct::Work(WorkFocus::Forge))));
        assert!(
            !plan
                .iter()
                .any(|(id, a)| *id == 1 && matches!(a, GovernAct::Work(_)))
        );
        // b is the biggest eligible levy town; a tithes; c is out of reach;
        // d is hungry.
        assert!(plan.contains(&(2, GovernAct::Dues(Dues::Levy))));
        assert!(plan.contains(&(1, GovernAct::Dues(Dues::Tithe))));
        assert!(!plan.iter().any(|(id, _)| *id == 3 || *id == 4));
    }

    /// A forge town under a day of food goes back to Balanced; between one
    /// and two days it is left as it is.
    #[test]
    fn govern_unforges_a_starving_town() {
        let mut t = town(1);
        t.smithies = 1;
        t.work = WorkFocus::Forge;
        t.days = 0.8;
        assert!(plan_govern(&[t.clone()]).contains(&(1, GovernAct::Work(WorkFocus::Balanced))));
        t.days = 1.5;
        assert!(
            !plan_govern(&[t])
                .iter()
                .any(|(_, a)| matches!(a, GovernAct::Work(_)))
        );
    }

    /// The seat counts against an attack whose straight march passes near
    /// it, not only one whose target is near it (`conquest_target`,
    /// `SEAT_BERTH`): seed 3 of `bot_run` lost its main army marching past
    /// the seat from one side to a village on the other.
    #[test]
    fn a_march_past_the_seat_comes_near_it() {
        let seat = IVec2::new(395, 813);
        // Seed 3: from #2 to #6, both about 110 tiles from the seat; the
        // march passes 24 tiles from it.
        let (from, to) = (IVec2::new(288, 841), IVec2::new(505, 833));
        assert!(dist(to, seat) > SEAT_BERTH);
        let d = dist_to_line(seat, from, to);
        assert!((23.0..25.0).contains(&d), "{d}");
        // The nearest point is an end when the seat lies beyond it, and
        // a march of no length is its start.
        assert_eq!(
            dist_to_line(IVec2::new(0, 0), IVec2::new(3, 4), IVec2::new(10, 4)),
            5.0
        );
        assert_eq!(dist_to_line(seat, from, from), dist(seat, from));
    }
}
