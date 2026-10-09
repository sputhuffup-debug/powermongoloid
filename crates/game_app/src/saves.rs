//! Saves in the game (GP-12, ADR 0016): one slot per run, F5/F9, saving on
//! exit, "Resume" on the new-game screen, and new runs loaded in place.
//!
//! **Where.** Slots live in the save directory: `--save-dir <dir>`, else
//! `saves/` next to the game binary (`target/debug/saves` for `cargo run`).
//! A run's slot is `run-<seed>-<difficulty>.sav`; the developed starts
//! (`--skirmish`, `--villages`, `--stress`) quicksave to `world-<seed>.sav`
//! and are never saved on exit or offered for resuming.
//!
//! **When.** F5 saves this run's slot and F9 loads it (development aids,
//! M4_PLAN). Leaving a run, by closing the game or by starting another run
//! in place, saves it if it has started (tick > 0) and isn't over; a
//! finished run's slot is deleted then. Everything happens between ticks:
//! [`handle_saves`] runs in `Update` before the render and UI sets, and
//! [`save_on_exit`] in `Last`, after Bevy decides to exit.
//!
//! **Loading** is `sim_core::load_world` into the running world. The world
//! is first saved to memory, and restored if the save fails to load. After
//! a load the clock is paused, `game_render::WorldEpoch` is bumped (render
//! and UI caches clear, the UI decides which screen is up) and the opening
//! shot is set again: the champion, else the run's start, else your first
//! captain.
//!
//! **New runs** are built on a worker thread exactly as a fresh process
//! would build them (`sim_headless::headless_app` with
//! `HeadlessConfig::run`), saved to memory and loaded the same way, so the
//! window stays open. If the worker or the load fails, the game restarts
//! itself with the new run's arguments, as before M4 step 10b.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime};

use bevy::app::AppExit;
use bevy::prelude::*;
use game_render::WorldEpoch;
use game_ui::{NewRunRequest, Notice, SaveRequest, SavedRun, SavedRuns};
use sim_core::{
    Allegiance, Calendar, FactionId, Position, RunConfig, RunState, SaveError, SimSeed, SimTick,
};
use sim_headless::HeadlessConfig;
use sim_military::{Captain, Champion, RunProgress};

use crate::OpeningShot;

/// The directory holding the save slots.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct SaveDir(pub PathBuf);

impl SaveDir {
    /// `--save-dir`, else `saves/` next to the game binary.
    pub fn from_arg(arg: Option<String>) -> Self {
        let dir = arg.map(PathBuf::from).unwrap_or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|p| p.join("saves")))
                .unwrap_or_else(|| PathBuf::from("saves"))
        });
        SaveDir(dir)
    }
}

/// A new run being built on a worker thread.
#[derive(Resource, Default)]
pub struct Building(Option<(NewRunRequest, JoinHandle<Vec<u8>>)>);

/// The slot file of a world: one per run (seed and difficulty).
pub fn slot_name(seed: u64, run: Option<RunConfig>) -> String {
    match run {
        Some(c) => format!("run-{seed}-{}.sav", c.difficulty.name()),
        None => format!("world-{seed}.sav"),
    }
}

/// The saved runs in `dir`, newest first (files that aren't run saves of
/// this format are skipped). Reads only each file's header.
pub fn scan(dir: &Path) -> Vec<SavedRun> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(SystemTime, SavedRun)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "sav" {
                return None;
            }
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let mut head = Vec::new();
            fs::File::open(&path)
                .ok()?
                .take(4096)
                .read_to_end(&mut head)
                .ok()?;
            let header = sim_core::read_header(&head).ok()?;
            header
                .run
                .is_some()
                .then_some((modified, SavedRun { path, header }))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    found.into_iter().map(|(_, run)| run).collect()
}

/// This world's slot.
fn current_slot(world: &World) -> PathBuf {
    let seed = world.resource::<SimSeed>().0;
    let run = world.resource::<RunState>().config;
    world.resource::<SaveDir>().0.join(slot_name(seed, run))
}

/// "day 3 14:20" for a tick.
fn when(world: &World, tick: u64) -> String {
    let at = world.resource::<Calendar>().at(tick);
    let (h, m) = at.hours_minutes();
    format!("day {} {h:02}:{m:02}", at.day + 1)
}

fn notice(world: &mut World, text: impl Into<String>, secs: f32) {
    let text = text.into();
    info!("saves: {text}");
    let now = world.resource::<Time<Real>>().elapsed_secs();
    world.resource_mut::<Notice>().show(text, now, secs);
}

fn refresh_saved(world: &mut World) {
    let runs = scan(&world.resource::<SaveDir>().0);
    world.insert_resource(SavedRuns(runs));
}

/// Writes the world to `path` (through a temporary file, so a crash never
/// leaves half a save).
fn write_slot(world: &mut World, path: &Path) -> std::io::Result<usize> {
    let started = Instant::now();
    let bytes = sim_core::save_world(world);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("sav.tmp");
    fs::write(&tmp, &bytes)?;
    fs::rename(&tmp, path)?;
    info!(
        "saved {} ({} KB) in {:.2} s",
        path.display(),
        bytes.len() / 1024,
        started.elapsed().as_secs_f32()
    );
    Ok(bytes.len())
}

/// Requests from the UI (F5, F9, "Resume", new runs) and the new run being
/// built. Exclusive, between ticks, before the render and UI sets.
pub fn handle_saves(world: &mut World) {
    let _span = info_span!("game_app::handle_saves").entered();
    let requests: Vec<SaveRequest> = world
        .resource_mut::<Messages<SaveRequest>>()
        .drain()
        .collect();
    let new_runs: Vec<NewRunRequest> = world
        .resource_mut::<Messages<NewRunRequest>>()
        .drain()
        .collect();
    for request in requests {
        match request {
            SaveRequest::Save => quicksave(world),
            SaveRequest::Load => quickload(world),
            SaveRequest::Resume(path) => resume(world, &path),
        }
    }
    if let Some(&r) = new_runs.last() {
        start_new_run(world, r);
    }
    finish_new_run(world);
}

/// Saves when the game exits (GP-12: one slot per run, saved on exit).
pub fn save_on_exit(world: &mut World, mut done: Local<bool>) {
    let _span = info_span!("game_app::save_on_exit").entered();
    if *done || world.resource::<Messages<AppExit>>().is_empty() {
        return;
    }
    *done = true;
    leave_run(world);
}

/// Leaving a run: save it if it has started and isn't over; a finished
/// run's slot goes.
fn leave_run(world: &mut World) {
    let run = *world.resource::<RunState>();
    if !run.in_run() {
        return;
    }
    let path = current_slot(world);
    if run.outcome.is_some() {
        match fs::remove_file(&path) {
            Ok(()) => info!("the run is over: removed {}", path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!("could not remove {}: {e}", path.display()),
        }
    } else if world.resource::<SimTick>().0 > 0
        && let Err(e) = write_slot(world, &path)
    {
        error!("could not save {}: {e}", path.display());
    }
}

/// F5.
fn quicksave(world: &mut World) {
    let run = *world.resource::<RunState>();
    if run.outcome.is_some() || run.pending.is_some() {
        notice(world, "The run is over: nothing to save", 4.0);
        return;
    }
    let path = current_slot(world);
    let tick = world.resource::<SimTick>().0;
    let text = match write_slot(world, &path) {
        Ok(_) => {
            refresh_saved(world);
            format!("Saved {} (F9 loads it)", when(world, tick))
        }
        Err(e) => format!("Could not save {}: {e}", path.display()),
    };
    notice(world, text, 4.0);
}

/// F9.
fn quickload(world: &mut World) {
    let path = current_slot(world);
    match fs::read(&path) {
        Ok(bytes) => {
            let _ = load_bytes(world, &bytes, "the quicksave");
        }
        Err(_) => notice(world, "No save for this run yet (F5 saves)", 4.0),
    }
}

/// "Resume" on the new-game screen.
fn resume(world: &mut World, path: &Path) {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            notice(
                world,
                format!("Could not read {}: {e}", path.display()),
                6.0,
            );
            refresh_saved(world);
            return;
        }
    };
    leave_run(world);
    let _ = load_bytes(world, &bytes, "the saved run");
}

/// Loads `bytes` into the running world. A save of another format is
/// refused before anything changes; any later failure restores the world
/// as it was.
fn load_bytes(world: &mut World, bytes: &[u8], what: &str) -> Result<(), SaveError> {
    let started = Instant::now();
    if let Err(e) = sim_core::read_header(bytes) {
        notice(world, format!("Could not load {what}: {e}"), 6.0);
        return Err(e);
    }
    let backup = sim_core::save_world(world);
    let result = sim_core::load_world(world, bytes);
    if let Err(e) = &result {
        error!("loading {what} failed ({e}); restoring the world");
        sim_core::load_world(world, &backup).expect("the world saved a moment ago loads");
    }
    after_load(world);
    let tick = world.resource::<SimTick>().0;
    let hash = sim_core::compute_state_hash(world);
    info!(
        "loaded {what} in {:.2} s: seed {}, tick {tick}, state hash {hash:#018x}",
        started.elapsed().as_secs_f32(),
        world.resource::<SimSeed>().0,
    );
    let text = match &result {
        Ok(()) => format!("Loaded {}: {} (paused)", what, when(world, tick)),
        Err(e) => format!("Could not load {what}: {e}"),
    };
    notice(world, text, 5.0);
    result
}

/// After the world was replaced: tell render and UI, pause, frame the
/// camera again.
fn after_load(world: &mut World) {
    world.resource_mut::<WorldEpoch>().0 += 1;
    world.resource_mut::<Time<Virtual>>().pause();
    if let Some(shot) = opening_shot(world) {
        world.insert_resource(shot);
    }
    refresh_saved(world);
}

/// Where the camera goes after a load: the champion, else the run's start,
/// else the player's first captain (by SpawnId).
fn opening_shot(world: &mut World) -> Option<OpeningShot> {
    let mut champions = world.query_filtered::<&Position, With<Champion>>();
    if let Some(p) = champions.iter(world).next() {
        return Some(OpeningShot(p.0.as_vec2(), 90.0));
    }
    if world.resource::<RunState>().in_run() {
        let start = world.resource::<RunProgress>().start;
        return Some(OpeningShot(start.as_vec2(), 90.0));
    }
    let mut captains =
        world.query_filtered::<(&sim_core::SpawnId, &Position, &Allegiance), With<Captain>>();
    captains
        .iter(world)
        .filter(|(.., a)| a.0 == FactionId::PLAYER)
        .min_by_key(|(id, ..)| id.0)
        .map(|(_, p, _)| OpeningShot(p.0.as_vec2(), 90.0))
}

/// A new run was asked for: leave this one and build the new island on a
/// worker thread.
fn start_new_run(world: &mut World, r: NewRunRequest) {
    if world.resource::<Building>().0.is_some() {
        notice(world, "Already building an island", 3.0);
        return;
    }
    leave_run(world);
    world.resource_mut::<Time<Virtual>>().pause();
    let config = HeadlessConfig::run(
        r.seed,
        RunConfig {
            difficulty: r.difficulty,
        },
    );
    let worker = std::thread::Builder::new()
        .name("new-run".into())
        .spawn(move || {
            let started = Instant::now();
            let mut app = sim_headless::headless_app(config);
            let bytes = sim_headless::save_app(&mut app);
            info!(
                "new run: island for seed {} built in {:.2} s",
                config.seed,
                started.elapsed().as_secs_f32()
            );
            bytes
        });
    match worker {
        Ok(handle) => {
            world.resource_mut::<Building>().0 = Some((r, handle));
            notice(
                world,
                format!(
                    "Building the island for seed {} ({})...",
                    r.seed,
                    r.difficulty.name()
                ),
                120.0,
            );
        }
        Err(e) => {
            error!("new run: no worker thread ({e})");
            restart(world, r);
        }
    }
}

/// Loads the new run once its island is built.
fn finish_new_run(world: &mut World) {
    let done = world
        .resource::<Building>()
        .0
        .as_ref()
        .is_some_and(|(_, h)| h.is_finished());
    if !done {
        return;
    }
    let Some((r, handle)) = world.resource_mut::<Building>().0.take() else {
        return;
    };
    match handle.join() {
        Ok(bytes) => {
            if load_bytes(world, &bytes, "the new run").is_err() {
                restart(world, r);
            }
        }
        Err(_) => {
            error!("new run: building the island failed");
            restart(world, r);
        }
    }
}

/// The fallback: restart the game with the new run's arguments.
fn restart(world: &mut World, r: NewRunRequest) {
    let dir = world.resource::<SaveDir>().0.clone();
    let args = [
        "--seed".to_string(),
        r.seed.to_string(),
        "--difficulty".to_string(),
        r.difficulty.name().to_string(),
        "--save-dir".to_string(),
        dir.display().to_string(),
    ];
    let spawned =
        std::env::current_exe().and_then(|exe| std::process::Command::new(exe).args(&args).spawn());
    match spawned {
        Ok(_) => {
            info!(
                "new run: restarting as seed {}, {}",
                r.seed,
                r.difficulty.name()
            );
            world.write_message(AppExit::Success);
        }
        Err(e) => notice(world, format!("Could not start the new run: {e}"), 8.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::Difficulty;

    /// GP-12: a save loads into a world that already holds another run (as
    /// F9, "Resume" and in-place new runs do in the game), and both worlds
    /// go on alike.
    #[test]
    fn a_save_loads_over_another_run() {
        use sim_core::{SimStepper, compute_state_hash};
        let mut a = sim_headless::headless_app(HeadlessConfig::run(5, RunConfig::default()));
        a.step(30);
        let bytes = sim_headless::save_app(&mut a);
        let harsh = RunConfig {
            difficulty: Difficulty::Harsh,
        };
        let mut b = sim_headless::headless_app(HeadlessConfig::run(6, harsh));
        b.step(10);
        sim_core::load_world(b.world_mut(), &bytes).expect("the save loads");
        assert_eq!(b.world().resource::<SimSeed>().0, 5);
        assert_eq!(
            compute_state_hash(a.world_mut()),
            compute_state_hash(b.world_mut())
        );
        a.step(60);
        b.step(60);
        assert_eq!(
            compute_state_hash(a.world_mut()),
            compute_state_hash(b.world_mut())
        );
    }

    #[test]
    fn one_slot_per_run() {
        let harsh = RunConfig {
            difficulty: Difficulty::Harsh,
        };
        assert_eq!(slot_name(42, Some(harsh)), "run-42-harsh.sav");
        assert_eq!(slot_name(7, Some(RunConfig::default())), "run-7-steady.sav");
        assert_eq!(slot_name(42, None), "world-42.sav");
    }

    /// GP-12: the resume list reads run saves' headers and skips anything
    /// else in the directory.
    #[test]
    fn the_resume_list_holds_run_saves_only() {
        let dir = std::env::temp_dir().join(format!("warlord-saves-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut world = World::new();
        world.insert_resource(SimSeed(9));
        world.insert_resource(SimTick(1234));
        world.insert_resource(RunState {
            config: Some(RunConfig {
                difficulty: Difficulty::Chaotic,
            }),
            ..default()
        });
        fs::write(
            dir.join("run-9-chaotic.sav"),
            sim_core::save_world(&mut world),
        )
        .unwrap();
        world.insert_resource(RunState::default());
        fs::write(dir.join("world-9.sav"), sim_core::save_world(&mut world)).unwrap();
        fs::write(dir.join("junk.sav"), b"not a save").unwrap();
        fs::write(dir.join("notes.txt"), b"hello").unwrap();
        let runs = scan(&dir);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].path.file_name().unwrap(), "run-9-chaotic.sav");
        assert_eq!(runs[0].header.seed, 9);
        assert_eq!(runs[0].header.tick, 1234);
        assert_eq!(
            runs[0].header.run.map(|r| r.difficulty),
            Some(Difficulty::Chaotic)
        );
        assert!(scan(&dir).is_empty());
    }
}
