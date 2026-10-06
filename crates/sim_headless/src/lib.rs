//! The full simulation as one plugin group ([`SimPlugins`]), shared by the
//! headless binary, integration tests, benchmarks and `game_app`, plus the
//! determinism harness (NFR-08, ADR 0004).
//!
//! # Main pieces
//! - [`headless_app`] and [`HeadlessConfig`]: a windowless sim, with an
//!   optional [`Scenario`] (villages, skirmish, battle).
//! - [`run`], [`record_replay`], [`verify_replay`]: run and check hashes.
//! - [`sample_commands`] and [`sample_orders`]: deterministic command streams
//!   for tests.
//! - [`stats`]: population and army summaries (`--stats-every`).
//!
//! The CLI (`src/main.rs`) wraps these; run it with `--help`.

pub mod stats;

use std::time::{Duration, Instant};

pub use bevy_app::App;
use bevy_app::{FixedUpdate, PluginGroup, PluginGroupBuilder, TaskPoolPlugin};
use bevy_ecs::schedule::SingleThreadedExecutor;
use rand::RngExt;
use sim_core::rng::rng_key;
use sim_core::{
    Checkpoint, CommandKind, CommandLog, CommandScript, FactionId, LoggedCommand, REPLAY_FORMAT,
    Replay, SimCommand, SimCorePlugin, SimSeed, SimStepper, compute_state_hash,
};
use sim_world::WorldGenSettings;

/// The population in `villagers.ron` (5,000 for M2).
pub fn default_population() -> u32 {
    sim_agents::Villagers::default_data(&sim_core::Calendar::default_data())
        .data
        .population
        .total
}

/// Every `sim_*` plugin, in dependency order.
pub struct SimPlugins {
    pub seed: u64,
    /// Overrides the population in `villagers.ron`.
    pub population: Option<u32>,
    pub worldgen: WorldGenSettings,
}

impl SimPlugins {
    /// The shipped data files.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            population: None,
            worldgen: WorldGenSettings::default_data(),
        }
    }
}

impl PluginGroup for SimPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SimCorePlugin { seed: self.seed })
            .add(sim_world::WorldPlugin {
                settings: self.worldgen,
                threads: sim_world::worldgen::default_threads(),
            })
            .add(sim_economy::EconomyPlugin)
            .add(sim_agents::AgentsPlugin {
                population: self.population,
            })
            .add(sim_military::MilitaryPlugin)
            .add(sim_knowledge::KnowledgePlugin)
            .add(sim_lod::LodPlugin)
            .add(sim_ai::AiPlugin)
    }
}

/// How the `FixedUpdate` schedule executes its systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Threading {
    /// Bevy's parallel executor, as in the game.
    #[default]
    Multi,
    /// One system at a time. Results must match [`Threading::Multi`].
    Single,
}

/// What is set up before the first tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scenario {
    /// The populated region only (M2).
    #[default]
    Villages,
    /// The M3 playtest start: the player's captain and a raiding band
    /// (`sim_military::setup_skirmish`).
    Skirmish,
    /// The skirmish plus two armed armies of this many each, marching on
    /// each other (`sim_military::setup_battle`; benchmarks, stress).
    Battle(u32),
}

impl Scenario {
    /// Name stored in replay files ("" for villages).
    pub fn name(self) -> String {
        match self {
            Scenario::Villages => String::new(),
            Scenario::Skirmish => "skirmish".into(),
            Scenario::Battle(n) => format!("battle:{n}"),
        }
    }

    /// Parses a replay file's scenario name.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "" | "villages" => Some(Scenario::Villages),
            "skirmish" => Some(Scenario::Skirmish),
            _ => s
                .strip_prefix("battle:")
                .and_then(|n| n.parse().ok())
                .map(Scenario::Battle),
        }
    }
}

/// How to build a headless simulation.
#[derive(Debug, Clone, Copy)]
pub struct HeadlessConfig {
    pub seed: u64,
    pub population: u32,
    pub threading: Threading,
    /// Map size in tiles; the shipped settings' size (2048) by default.
    pub world_size: u32,
    pub scenario: Scenario,
}

impl HeadlessConfig {
    /// The shipped settings with `seed`: default population and map size, villages scenario, parallel.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            population: default_population(),
            threading: Threading::Multi,
            world_size: WorldGenSettings::default_data().size,
            scenario: Scenario::Villages,
        }
    }

    /// World generation settings at this map size.
    pub fn worldgen(&self) -> WorldGenSettings {
        WorldGenSettings::default_data().with_size(self.world_size)
    }
}

/// Builds a windowless simulation app, ready to [`SimStepper::step`].
pub fn headless_app(config: HeadlessConfig) -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default())
        .add_plugins(SimPlugins {
            seed: config.seed,
            population: Some(config.population),
            worldgen: config.worldgen(),
        });
    if config.threading == Threading::Single {
        app.edit_schedule(FixedUpdate, |s| {
            s.set_executor(SingleThreadedExecutor::new());
        });
    }
    app.prepare_sim();
    match config.scenario {
        Scenario::Villages => {}
        Scenario::Skirmish => {
            sim_military::setup_skirmish(app.world_mut());
        }
        Scenario::Battle(n) => {
            sim_military::setup_battle(app.world_mut(), n);
        }
    }
    app
}

/// The outcome of a headless run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunResult {
    /// State hash every `checkpoint_every` ticks, always including the end.
    pub checkpoints: Vec<Checkpoint>,
    /// Every command the sim applied, in order.
    pub log: Vec<LoggedCommand>,
    /// Wall-clock time to build the app (including world generation) and to
    /// run the ticks. Diagnostics only.
    pub setup_time: Duration,
    pub tick_time: Duration,
}

impl RunResult {
    /// The state hash at the end of the run.
    pub fn final_hash(&self) -> u64 {
        self.checkpoints.last().map_or(0, |c| c.hash)
    }
}

/// Runs `ticks` ticks, replaying `commands` at their recorded ticks, and
/// hashes the state every `checkpoint_every` ticks (0 = only at the end).
pub fn run(
    config: HeadlessConfig,
    ticks: u64,
    commands: &[LoggedCommand],
    checkpoint_every: u64,
) -> RunResult {
    let start = Instant::now();
    let mut app = headless_app(config);
    let setup_time = start.elapsed();
    let start = Instant::now();
    app.insert_resource(CommandScript::from_log(commands));
    let mut checkpoints = Vec::new();
    for tick in 1..=ticks {
        app.step(1);
        if tick == ticks || (checkpoint_every > 0 && tick % checkpoint_every == 0) {
            checkpoints.push(Checkpoint {
                tick,
                hash: compute_state_hash(app.world_mut()),
            });
        }
    }
    let tick_time = start.elapsed();
    let log = app.world().resource::<CommandLog>().0.clone();
    RunResult {
        checkpoints,
        log,
        setup_time,
        tick_time,
    }
}

/// Runs `ticks` ticks from `seed` and returns the final state hash.
pub fn run_and_hash(seed: u64, ticks: u64, population: u32) -> u64 {
    let config = HeadlessConfig {
        population,
        ..HeadlessConfig::new(seed)
    };
    run(config, ticks, &[], 0).final_hash()
}

/// Runs a session and packages it as a replay file.
pub fn record_replay(
    config: HeadlessConfig,
    ticks: u64,
    commands: &[LoggedCommand],
    checkpoint_every: u64,
) -> Replay {
    let result = run(config, ticks, commands, checkpoint_every);
    Replay {
        format: REPLAY_FORMAT,
        seed: config.seed,
        population: config.population,
        world_size: config.world_size,
        scenario: config.scenario.name(),
        ticks,
        checkpoint_every,
        commands: result.log,
        checkpoints: result.checkpoints,
    }
}

/// A replay that did not reproduce: the first checkpoint that differed.
#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    pub expected: Checkpoint,
    pub actual: Option<Checkpoint>,
}

impl std::fmt::Display for Divergence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.actual {
            Some(a) => write!(
                f,
                "diverged by tick {}: expected 0x{:016x}, got 0x{:016x}",
                self.expected.tick, self.expected.hash, a.hash
            ),
            None => write!(f, "no checkpoint at tick {}", self.expected.tick),
        }
    }
}

/// Re-runs a replay and checks every checkpoint.
pub fn verify_replay(replay: &Replay, threading: Threading) -> Result<RunResult, Divergence> {
    let config = HeadlessConfig {
        seed: replay.seed,
        population: replay.population,
        threading,
        world_size: replay.world_size,
        scenario: Scenario::parse(&replay.scenario).unwrap_or_default(),
    };
    let result = run(
        config,
        replay.ticks,
        &replay.commands,
        replay.checkpoint_every,
    );
    match replay.first_divergence(&result.checkpoints) {
        None => Ok(result),
        Some((expected, actual)) => Err(Divergence { expected, actual }),
    }
}

/// `count` deterministic no-op commands spread over `ticks`, from `seed`.
/// Used by tests and `sim_headless --commands`.
pub fn sample_commands(seed: u64, ticks: u64, count: usize) -> Vec<LoggedCommand> {
    let mut rng = SimSeed(seed).stream(rng_key("sim_headless::sample_commands"), 0);
    let mut commands: Vec<LoggedCommand> = (0..count)
        .map(|_| LoggedCommand {
            tick: rng.random_range(0..ticks.max(1)),
            command: SimCommand::player(FactionId(rng.random_range(0..4)), CommandKind::Noop),
        })
        .collect();
    commands.sort_by_key(|c| c.tick);
    commands
}

/// `count` deterministic military commands for the armies in `app` (after
/// scenario setup), spread over `ticks`, from `seed`: orders of every kind,
/// posture changes, appointments, and some invalid commands (another
/// faction's captain, an unknown settlement) that must be refused. Issued
/// by the player's faction (1). Used by tests and `sim_headless --commands`.
pub fn sample_orders(app: &mut App, seed: u64, ticks: u64, count: usize) -> Vec<LoggedCommand> {
    use bevy_ecs::prelude::*;
    use sim_core::{OrderSpec, Posture, SpawnId};
    use sim_economy::SettlementEconomy;
    use sim_military::{Captain, Follower};
    let world = app.world_mut();
    let mut q = world.query::<(&SpawnId, &Captain, &sim_core::Position)>();
    let mut captains: Vec<(u64, u16, bevy_math::IVec2)> = q
        .iter(world)
        .map(|(id, c, p)| (id.0, c.faction.0, p.0))
        .collect();
    captains.sort_unstable_by_key(|c| c.0);
    let mut q = world.query_filtered::<&SpawnId, With<SettlementEconomy>>();
    let mut settlements: Vec<u64> = q.iter(world).map(|s| s.0).collect();
    settlements.sort_unstable();
    let mut q = world.query::<(&SpawnId, &Follower)>();
    let mut followers: Vec<u64> = q.iter(world).map(|(s, _)| s.0).collect();
    followers.sort_unstable();
    if captains.is_empty() || settlements.is_empty() {
        return sample_commands(seed, ticks, count);
    }
    let mut rng = SimSeed(seed).stream(rng_key("sim_headless::sample_orders"), 0);
    let mut out: Vec<LoggedCommand> = (0..count)
        .map(|_| {
            let (cid, _, at) = captains[rng.random_range(0..captains.len())];
            let s = settlements[rng.random_range(0..settlements.len())];
            let kind = match rng.random_range(0..12) {
                0..=2 => {
                    let (x, y) = near(&mut rng, at);
                    OrderSpec::MoveTo { x, y }
                }
                3 => OrderSpec::GatherFood { settlement: s },
                4 => OrderSpec::Recruit {
                    settlement: s,
                    count: rng.random_range(1..20),
                },
                5 => OrderSpec::Equip { settlement: s },
                6 => OrderSpec::Trade { settlement: s },
                7 => OrderSpec::Attack { settlement: s },
                8 => {
                    let (x, y) = near(&mut rng, at);
                    OrderSpec::Scout { x, y }
                }
                9 => OrderSpec::Return,
                10 => OrderSpec::GatherFood {
                    settlement: u64::MAX,
                },
                _ => OrderSpec::MoveTo { x: -5, y: 3 },
            };
            let command = match rng.random_range(0..10) {
                0 => CommandKind::SetPosture {
                    captain: cid,
                    posture: [Posture::Passive, Posture::Neutral, Posture::Aggressive]
                        [rng.random_range(0..3)],
                },
                1 if !followers.is_empty() => CommandKind::AppointCaptain {
                    follower: followers[rng.random_range(0..followers.len())],
                },
                2 => CommandKind::CancelOrders { captain: cid },
                _ => CommandKind::IssueOrder {
                    captain: cid,
                    order: kind,
                    queue: rng.random_bool(0.3),
                },
            };
            LoggedCommand {
                tick: rng.random_range(0..ticks.max(1)),
                command: SimCommand::player(FactionId(1), command),
            }
        })
        .collect();
    out.sort_by_key(|c| c.tick);
    out
}

/// A tile within 60 tiles of `at` (clamped at 0).
fn near<R: rand::Rng + ?Sized>(rng: &mut R, at: bevy_math::IVec2) -> (i32, i32) {
    (
        (at.x + rng.random_range(-60..=60)).max(0),
        (at.y + rng.random_range(-60..=60)).max(0),
    )
}

/// Scenario helper (M2 famine criterion): turns every farmer in
/// `settlement` idle, as if they had been cut off from their fields.
/// Applied before the first tick, like any other scenario setup. Returns
/// the number of farmers affected.
pub fn cut_off_farmers(app: &mut App, settlement: bevy_ecs::entity::Entity) -> u32 {
    use sim_agents::{Employment, Villager};
    use sim_economy::{Occupation, Workplace};
    let world = app.world_mut();
    let mut q = world.query::<(&Villager, &mut Occupation, &mut Employment)>();
    let mut n = 0;
    let mut freed = Vec::new();
    for (v, mut occ, mut job) in q.iter_mut(world) {
        if v.settlement == settlement && *occ == Occupation::Farmer {
            *occ = Occupation::Idle;
            if let Some(w) = job.workplace.take() {
                freed.push(w);
            }
            n += 1;
        }
    }
    for w in freed {
        if let Some(mut wp) = world.get_mut::<Workplace>(w) {
            wp.workers.clear();
        }
    }
    n
}
