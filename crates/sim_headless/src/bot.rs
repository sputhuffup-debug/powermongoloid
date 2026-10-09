//! A scripted player for runs (GP-16, GP-21): reads the sim between ticks
//! and plays only through ordinary player commands, so a bot run records
//! and replays like a human's.
//!
//! This is the M4 step 7 skeleton. Every [`THINK_EVERY`] ticks the
//! [`Bot`]:
//! 1. answers every offer waiting for the player with its first choice
//!    (the start boon, traits, dominion boons) and accepts invitations;
//! 2. forages when its baggage holds less than a day (moving on to fresh
//!    ground when a spot is bare), before anything else;
//! 3. answers a village's request for food with a `Give` when the
//!    champion's baggage can spare it, and marches to a village asking to
//!    be rid of raiders when it outnumbers them half again;
//! 4. otherwise takes the nearest monument nobody holds or guards (and no
//!    guard camp stands near), by standing at it.
//!
//! State is read with SpawnId-sorted queries; tie-breaks draw from stream
//! `sim_headless::bot` at the tick. Step 11 teaches it to win.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use rand::RngExt;
use sim_core::rng::rng_key;
use sim_core::{
    Cause, Checkpoint, Chronicle, ChronicleEntry, CommandKind, CommandLog, FactionId,
    LoggedCommand, Offers, OrderSpec, Position, RunOutcome, RunState, SimCommand, SimSeed,
    SimStepper, SimTick, SpawnId, compute_state_hash,
};
use sim_economy::{Economy, Stockpile};
use sim_military::{
    Army, Captain, Follower, Guard, Holder, Progression, RequestBoard, RequestKind,
};
use sim_world::{Monument, Settlement};

use crate::{App, HeadlessConfig, headless_app};

/// How often the bot thinks (ticks): once a game hour.
pub const THINK_EVERY: u64 = 100;

/// The bot leaves monuments within this many tiles of a guard camp alone.
const CAMP_BERTH: f32 = 40.0;

/// What the bot's champion is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    /// Bringing food to settlement (SpawnId).
    Give(u64),
    /// Marching to a settlement asking to be rid of raiders.
    Defend(u64),
    /// Living off the land here.
    Forage,
    /// Walking to fresh ground, then foraging.
    Wander(IVec2),
    /// Standing at a monument (SpawnId) to take it.
    Monument(u64),
}

/// The scripted player. See the module docs.
#[derive(Debug, Clone)]
pub struct Bot {
    seed: SimSeed,
    intent: Option<Intent>,
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

impl Bot {
    /// A bot for the run of world seed `seed`.
    pub fn new(seed: u64) -> Self {
        Self {
            seed: SimSeed(seed),
            intent: None,
            notes: Vec::new(),
        }
    }

    /// Reads `app` between ticks and returns the player commands to send
    /// before the next one (none except every [`THINK_EVERY`] ticks).
    pub fn act(&mut self, app: &mut App) -> Vec<SimCommand> {
        let tick = app.world().resource::<SimTick>().0;
        if !tick.is_multiple_of(THINK_EVERY) || !app.world().resource::<RunState>().in_run() {
            return Vec::new();
        }
        let mut out: Vec<SimCommand> = app
            .world()
            .resource::<Offers>()
            .for_faction(FactionId::PLAYER)
            .map(|o| o.id)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|offer| {
                SimCommand::player(
                    FactionId::PLAYER,
                    CommandKind::ChooseOffer { offer, choice: 0 },
                )
            })
            .collect();
        let world = app.world_mut();
        let Some(champion) = sim_military::champion_of(world, FactionId::PLAYER) else {
            return out;
        };
        let Some(view) = Champion::read(world, champion) else {
            return out;
        };
        let intent = self.choose(world, &view, tick);
        // Re-issue only on a new intent, or when the band stopped short of
        // it (not when it stands at the monument or village it holds).
        let near =
            |t: Option<IVec2>, r: f32| t.is_some_and(|t| (t - view.at).as_vec2().length() <= r);
        let arrived = match intent {
            Some(Intent::Monument(m)) => near(monument_tile(world, m), 3.0),
            Some(Intent::Defend(s)) => near(settlement_center(world, s), 10.0),
            _ => false,
        };
        let issue = intent != self.intent || (view.front.is_none() && !arrived);
        if let Some(intent) = intent.filter(|_| issue) {
            let orders = match intent {
                Intent::Give(s) => vec![OrderSpec::Give { settlement: s }],
                Intent::Defend(s) => {
                    let at = settlement_center(world, s).unwrap_or(view.at);
                    vec![OrderSpec::MoveTo { x: at.x, y: at.y }]
                }
                Intent::Forage => vec![forage(world)],
                Intent::Wander(to) => vec![OrderSpec::MoveTo { x: to.x, y: to.y }, forage(world)],
                Intent::Monument(m) => {
                    let at = monument_tile(world, m).unwrap_or(view.at);
                    vec![OrderSpec::MoveTo { x: at.x, y: at.y }]
                }
            };
            for (i, order) in orders.into_iter().enumerate() {
                out.push(SimCommand::player(
                    FactionId::PLAYER,
                    CommandKind::IssueOrder {
                        captain: view.id,
                        order,
                        queue: i > 0,
                    },
                ));
            }
            self.notes.push((tick, format!("{intent:?}")));
        }
        self.intent = intent;
        out
    }

    /// The bot's priorities (module docs).
    fn choose(&mut self, world: &mut World, view: &Champion, tick: u64) -> Option<Intent> {
        let keep = world.resource::<Army>().data.give_keep_days;
        // 2. Food from the land first: a starving band can't help anyone.
        let foraging = matches!(view.front, Some(OrderSpec::Forage { .. }));
        let wandering = matches!(self.intent, Some(Intent::Wander(_))) && view.front.is_some();
        if view.days < 1.0 {
            if foraging || wandering {
                return self.intent;
            }
            if self.intent == Some(Intent::Forage) {
                // The last spot ran bare: fresh ground 48 tiles away.
                let mut rng = self.seed.stream(rng_key("sim_headless::bot"), tick);
                let a = rng.random_range(0.0..std::f32::consts::TAU);
                let to = (view.at.as_vec2() + Vec2::from_angle(a) * 48.0)
                    .round()
                    .as_ivec2();
                let to = passable(world, to).unwrap_or(view.at);
                return Some(Intent::Wander(to));
            }
            return Some(Intent::Forage);
        }
        // 3. Requests: food first, then raiders.
        let board = world.resource::<RequestBoard>().clone();
        let mut asks: Vec<(f32, u64, Intent)> = Vec::new();
        for r in &board.open {
            let Some(center) = settlement_center(world, r.settlement) else {
                continue;
            };
            let d = center.as_vec2().distance(view.at.as_vec2());
            match r.kind {
                RequestKind::BringFood { need, delivered }
                    if delivered < need && d <= 200.0 && view.days >= keep + 0.5 =>
                {
                    asks.push((d, r.settlement, Intent::Give(r.settlement)));
                }
                RequestKind::DriveOff { .. }
                    if d <= 150.0 && view.people >= 3 * hostiles_near(world, center) / 2 =>
                {
                    asks.push((d + 1000.0, r.settlement, Intent::Defend(r.settlement)));
                }
                _ => {}
            }
        }
        asks.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if let Some(&(_, _, intent)) = asks.first() {
            return Some(intent);
        }
        // 4. The nearest monument nobody holds (a guarded one is held by
        // its camp), away from guard camps.
        let mut q = world.query_filtered::<&Position, (With<Captain>, With<Guard>)>();
        let camps: Vec<Vec2> = q.iter(world).map(|p| p.0.as_vec2()).collect();
        let mut q = world.query::<(&SpawnId, &Position, &Holder)>();
        let monument = q
            .iter(world)
            .filter(|(_, p, h)| {
                h.faction == FactionId::INDEPENDENT
                    && camps.iter().all(|c| c.distance(p.0.as_vec2()) > CAMP_BERTH)
            })
            .map(|(id, p, _)| (p.0.as_vec2().distance(view.at.as_vec2()), id.0))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if let Some((_, m)) = monument {
            return Some(Intent::Monument(m));
        }
        if view.days < 3.0 {
            return Some(Intent::Forage);
        }
        None
    }
}

/// What the bot knows of its champion's army.
struct Champion {
    id: u64,
    at: IVec2,
    front: Option<OrderSpec>,
    days: f32,
    people: u32,
}

impl Champion {
    fn read(world: &mut World, e: Entity) -> Option<Self> {
        let id = world.get::<SpawnId>(e)?.0;
        let at = world.get::<Position>(e)?.0;
        let front = world.get::<Captain>(e)?.orders.front().map(|o| o.spec);
        let mut q = world.query::<&Follower>();
        let people = q.iter(world).filter(|f| f.captain == e).count() as u32 + 1;
        let econ = world.resource::<Economy>();
        let food = world
            .get::<Stockpile>(e)
            .map_or(0.0, |s| s.food_value(econ));
        let per_day = world
            .resource::<sim_agents::Villagers>()
            .data
            .needs
            .hunger_per_day;
        let days = food / (per_day * people as f32).max(1e-3);
        Some(Self {
            id,
            at,
            front,
            days,
            people,
        })
    }
}

/// People in armies at war with the player within the threat radius of
/// `at` (guard camps included).
fn hostiles_near(world: &World, at: IVec2) -> u32 {
    let army = world.resource::<Army>();
    let radius = army.data.threat.radius;
    world
        .resource::<sim_military::ArmyIndex>()
        .armies
        .iter()
        .filter(|a| {
            army.at_war(a.faction, FactionId::PLAYER) && a.pos.distance(at.as_vec2()) <= radius
        })
        .map(|a| a.followers + 1)
        .sum()
}

fn forage(world: &World) -> OrderSpec {
    OrderSpec::Forage {
        radius: world.resource::<Progression>().data.forage.default_radius,
    }
}

fn passable(world: &World, at: IVec2) -> Option<IVec2> {
    let costs = &world.resource::<sim_agents::Villagers>().travel_costs;
    sim_military::nearest_passable(world.resource::<sim_world::WorldMap>(), costs, at)
}

fn settlement_center(world: &mut World, id: u64) -> Option<IVec2> {
    let mut q = world.query::<(&SpawnId, &Settlement)>();
    q.iter(world)
        .find(|(s, _)| s.0 == id)
        .map(|(_, s)| s.center)
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
    let mut app = headless_app(config);
    let mut bot = Bot::new(config.seed);
    let mut checkpoints = Vec::new();
    while app.world().resource::<SimTick>().0 < max_ticks
        && app.world().resource::<RunState>().outcome.is_none()
    {
        step(&mut bot, &mut app);
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
