//! The full simulation as one plugin group, shared by the headless binary,
//! integration tests, benchmarks and `game_app`; plus the determinism
//! harness (NFR-08, ADR 0004).

pub mod stats;

use std::time::{Duration, Instant};

use bevy_app::{App, FixedUpdate, PluginGroup, PluginGroupBuilder, TaskPoolPlugin};
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

#[derive(Debug, Clone, Copy)]
pub struct HeadlessConfig {
    pub seed: u64,
    pub population: u32,
    pub threading: Threading,
    /// Map size in tiles; the shipped settings' size (2048) by default.
    pub world_size: u32,
}

impl HeadlessConfig {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            population: default_population(),
            threading: Threading::Multi,
            world_size: WorldGenSettings::default_data().size,
        }
    }

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
            command: SimCommand {
                issuer: FactionId(rng.random_range(0..4)),
                kind: CommandKind::Noop,
            },
        })
        .collect();
    commands.sort_by_key(|c| c.tick);
    commands
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
