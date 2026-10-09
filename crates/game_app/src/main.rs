//! `cargo run -p game_app [-- --seed <u64>] [--difficulty <steady|chaotic|harsh>]
//! [--skirmish | --villages | --stress <agents>]`
//!
//! By default the game opens a run on the run island (GP-21): the new-game
//! screen shows the seed (from the clock unless `--seed` is given), the
//! difficulty, the run modifiers drawn and three start-boon cards; picking
//! one starts the clock. The other starts use the developed world (seed 42
//! unless given): `--skirmish` is the M3 playtest start (your captain with
//! 50 men at the edge of the populated region and a raiding band on the far
//! side), `--villages` the villages alone (M2), and `--stress <n>` adds two
//! armies meeting in battle so about n agents are simulated and drawn,
//! frames the camera on them, and logs frame and tick times every 5 s
//! (RND-04 measurement on the reference machine). `--run` is accepted and
//! means the default.
//!
//! A new run (the new-game screen with another seed or difficulty, or the
//! end screen's "New run" and "Same seed again") restarts the process with
//! the new arguments: winit can't build a second event loop in one process,
//! and resetting every sim resource in place is fragile.
//! TODO(M4 step 8): once `sim_core::save` exists, do it in-process instead:
//! build the new run headless, `load_world` it into this app, and bump
//! `game_render::WorldEpoch` (render and UI caches already clear on it).
//! Step 8 also adds F5/F9 quicksave/quickload and save-on-exit here.

use std::time::Duration;

use bevy::app::AppExit;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use game_render::CameraRig;
use game_ui::NewRunRequest;
use sim_core::{Difficulty, Position, RunConfig, SIM_TICK_HZ, SimTimings};
use sim_headless::{Scenario, ScenarioSetup, SimPlugins, set_up_scenario};
use sim_world::Mobile;

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
enum Start {
    Villages,
    Skirmish,
    Stress(u32),
    /// A run on the run island (GP-21): the default.
    Run(RunConfig),
}

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

/// Systems of the game binary.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AppSet {
    /// New runs, the opening shot and the stress log.
    Game,
}

fn main() {
    let start = if let Some(n) = arg("--stress").and_then(|s| s.parse().ok()) {
        Start::Stress(n)
    } else if flag("--villages") {
        Start::Villages
    } else if flag("--skirmish") {
        Start::Skirmish
    } else {
        let difficulty = arg("--difficulty")
            .and_then(|d| Difficulty::parse(&d))
            .unwrap_or_default();
        Start::Run(RunConfig { difficulty })
    };
    let seed = arg("--seed")
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| match start {
            Start::Run(_) => game_ui::screens::random_seed(),
            _ => 42,
        });
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Warlord Sim".into(),
            ..default()
        }),
        ..default()
    }))
    // SIM-03: the simulation advances only in FixedUpdate, at 10 Hz.
    .insert_resource(Time::<Fixed>::from_hz(SIM_TICK_HZ))
    .insert_resource(start)
    .add_plugins(match start {
        Start::Run(config) => SimPlugins::run(seed, config),
        _ => SimPlugins::new(seed),
    })
    .add_plugins((game_render::RenderPlugin, game_ui::UiPlugin))
    // Scenario setup happens before the first tick, like population.
    .add_systems(Startup, set_up_start)
    .add_systems(
        Update,
        (
            new_run,
            frame_start.run_if(resource_exists_and_changed::<OpeningShot>),
            stress_log.run_if(resource_equals(StressOn(true))),
        )
            .in_set(AppSet::Game),
    )
    .insert_resource(StressOn(matches!(start, Start::Stress(_))));
    // The dev debug panel may have added it already.
    if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default());
    }
    app.run();
}

#[derive(Resource, PartialEq)]
struct StressOn(bool);

/// Where the camera should open.
#[derive(Resource)]
struct OpeningShot(Vec2, f32);

/// Sets up the start through `sim_headless::set_up_scenario`, exactly as
/// headless runs do, and picks the opening camera shot.
fn set_up_start(world: &mut World) {
    let scenario = match *world.resource::<Start>() {
        Start::Villages => Scenario::Villages,
        Start::Skirmish => Scenario::Skirmish,
        Start::Stress(n) => {
            let villagers = world.query::<&sim_agents::Villager>().iter(world).count() as u32;
            let per_side = n.saturating_sub(villagers) / 2;
            info!("stress: {villagers} villagers + 2 x {per_side} soldiers");
            Scenario::Battle(per_side.max(1))
        }
        Start::Run(config) => Scenario::Run(config),
    };
    let at = |world: &World, e: Entity| world.get::<Position>(e).expect("captain").0.as_vec2();
    let shot = match set_up_scenario(world, scenario) {
        ScenarioSetup::Nothing => None,
        ScenarioSetup::Skirmish(s) => Some(OpeningShot(at(world, s.player), 90.0)),
        ScenarioSetup::Battle([a, b]) => {
            let (pa, pb) = (at(world, a), at(world, b));
            Some(OpeningShot((pa + pb) / 2.0, pa.distance(pb).max(150.0)))
        }
        ScenarioSetup::Run(r) => Some(OpeningShot(r.start.as_vec2(), 90.0)),
    };
    if let Some(shot) = shot {
        world.insert_resource(shot);
    }
}

/// Glides the camera from the island view to the opening shot (when it is
/// set: at the start, and again whenever a new world sets it).
fn frame_start(shot: Option<Res<OpeningShot>>, mut rigs: Query<&mut CameraRig>) {
    let _span = info_span!("game_app::frame_start").entered();
    let (Some(shot), Ok(mut rig)) = (shot, rigs.single_mut()) else {
        return;
    };
    rig.target_focus = shot.0;
    rig.target_distance = shot.1;
}

/// A new run was asked for: restart the game with its seed and difficulty
/// (see the crate docs for why not in-process).
fn new_run(mut requests: MessageReader<NewRunRequest>, mut exit: MessageWriter<AppExit>) {
    let _span = info_span!("game_app::new_run").entered();
    let Some(r) = requests.read().last().copied() else {
        return;
    };
    let args = [
        "--seed".to_string(),
        r.seed.to_string(),
        "--difficulty".to_string(),
        r.difficulty.name().to_string(),
    ];
    let spawned =
        std::env::current_exe().and_then(|exe| std::process::Command::new(exe).args(&args).spawn());
    match spawned {
        Ok(_) => {
            info!("new run: seed {}, {}", r.seed, r.difficulty.name());
            exit.write(AppExit::Success);
        }
        Err(e) => error!("new run: could not restart the game: {e}"),
    }
}

/// RND-04: frame time and sim tick time, every 5 seconds.
fn stress_log(
    time: Res<Time<Real>>,
    mut last: Local<Duration>,
    diagnostics: Res<DiagnosticsStore>,
    timings: Res<SimTimings>,
    agents: Query<(), (With<Mobile>, With<Position>)>,
    visuals: Option<Res<game_render::AgentVisuals>>,
) {
    let _span = info_span!("game_app::stress_log").entered();
    if time.elapsed() < *last + Duration::from_secs(5) {
        return;
    }
    *last = time.elapsed();
    let frame = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let bands = visuals.map(|v| v.per_band).unwrap_or_default();
    info!(
        "stress: {} agents | {fps:.1} fps, frame {frame:.2} ms | sim tick {:.2} ms (avg {:.2}) | bands detail/lowpoly/billboard/dot {:?}",
        agents.iter().count(),
        timings.total.as_secs_f64() * 1000.0,
        timings.total_avg.as_secs_f64() * 1000.0,
        bands
    );
}
