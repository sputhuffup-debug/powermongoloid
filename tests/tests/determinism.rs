//! NFR-08: same seed + same command stream -> bit-identical state hash.
//! See ADR 0003 (state hash) and ADR 0004 (harness and replays).

use sim_core::{
    CommandKind, CommandLog, FactionId, Replay, SimCommand, SimStepper, unhashed_state,
};
use sim_headless::{
    HeadlessConfig, RunResult, Scenario, Threading, headless_app, record_replay, run,
    sample_commands, sample_orders, verify_replay,
};

/// SEED.md "Testing requirements": 10,000 ticks with a command log.
const TICKS: u64 = 10_000;
const CHECKPOINT_EVERY: u64 = 1_000;
const COMMANDS: usize = 200;

fn config(seed: u64) -> HeadlessConfig {
    HeadlessConfig::new(seed)
}

/// Asserts two runs match, naming the first checkpoint that differs.
fn assert_same(a: &RunResult, b: &RunResult, what: &str) {
    if let Some((x, y)) = a
        .checkpoints
        .iter()
        .zip(&b.checkpoints)
        .find(|(x, y)| x != y)
    {
        panic!(
            "{what}: diverged by tick {}: 0x{:016x} vs 0x{:016x}",
            x.tick, x.hash, y.hash
        );
    }
    assert_eq!(
        a.checkpoints.len(),
        b.checkpoints.len(),
        "{what}: checkpoint count"
    );
}

#[test]
fn ten_thousand_ticks_with_command_log_are_identical() {
    let commands = sample_commands(42, TICKS, COMMANDS);
    let a = run(config(42), TICKS, &commands, CHECKPOINT_EVERY);
    let b = run(config(42), TICKS, &commands, CHECKPOINT_EVERY);
    assert_eq!(a.checkpoints.len(), 10);
    assert_same(&a, &b, "identical runs");
    assert_eq!(
        a.log, commands,
        "every scripted command is applied once, in order"
    );
}

#[test]
fn parallel_and_serial_execution_agree() {
    let commands = sample_commands(7, TICKS, COMMANDS);
    let multi = run(config(7), TICKS, &commands, CHECKPOINT_EVERY);
    let single = run(
        HeadlessConfig {
            threading: Threading::Single,
            ..config(7)
        },
        TICKS,
        &commands,
        CHECKPOINT_EVERY,
    );
    assert_same(&multi, &single, "multi- vs single-threaded");
}

#[test]
fn live_commands_record_and_replay_identically() {
    // Commands written as messages between ticks, the way the game's UI does.
    let live_ticks = [0u64, 3, 3, 250, 999];
    let mut app = headless_app(config(11));
    for t in 0..1_000u64 {
        for _ in live_ticks.iter().filter(|&&lt| lt == t) {
            app.world_mut()
                .write_message(SimCommand::player(FactionId(2), CommandKind::Noop));
        }
        app.step(1);
    }
    let live_log = app.world().resource::<CommandLog>().0.clone();
    let live_hash = sim_core::compute_state_hash(app.world_mut());
    let logged_ticks: Vec<u64> = live_log.iter().map(|c| c.tick).collect();
    assert_eq!(logged_ticks, live_ticks);

    // Package the log as a replay, round-trip it through RON, and re-run it.
    let replay = record_replay(config(11), 1_000, &live_log, 100);
    assert_eq!(replay.checkpoints.last().unwrap().hash, live_hash);
    let reloaded = Replay::from_ron(&replay.to_ron()).expect("replay parses");
    assert_eq!(reloaded, replay);
    verify_replay(&reloaded, Threading::Multi).expect("replay reproduces");
    verify_replay(&reloaded, Threading::Single).expect("replay reproduces serially");
}

#[test]
fn a_dropped_command_is_detected() {
    let commands = sample_commands(42, 2_000, 20);
    let mut replay = record_replay(config(42), 2_000, &commands, 500);
    replay.commands.remove(10);
    let divergence = verify_replay(&replay, Threading::Multi).expect_err("must diverge");
    assert!(divergence.expected.tick <= 2_000);
}

#[test]
fn different_seeds_give_different_hashes() {
    let a = run(config(42), 1_000, &[], 0);
    let b = run(config(43), 1_000, &[], 0);
    assert_ne!(
        a.final_hash(),
        b.final_hash(),
        "seed has no effect on state"
    );
}

#[test]
fn hash_changes_as_the_sim_advances() {
    let a = run(config(42), 1_000, &[], 0);
    let b = run(config(42), 1_001, &[], 0);
    assert_ne!(
        a.final_hash(),
        b.final_hash(),
        "state did not change over one tick"
    );
}

#[test]
fn hashing_does_not_change_state() {
    let mut app = headless_app(config(42));
    app.step(100);
    let first = sim_core::compute_state_hash(app.world_mut());
    let second = sim_core::compute_state_hash(app.world_mut());
    assert_eq!(first, second);
}

/// Every component on a simulation entity and every resource from a `sim_*`
/// crate must be hashed or explicitly exempted (ADR 0004).
#[test]
fn state_hash_covers_every_sim_component() {
    // With armies fighting, so every military component exists.
    let config = skirmish(42);
    let mut app = headless_app(config);
    let commands = sample_orders(&mut app, 42, 600, 30);
    app.insert_resource(sim_core::CommandScript::from_log(&commands));
    app.step(1_500);
    let missing = unhashed_state(app.world_mut());
    assert!(
        missing.is_empty(),
        "state not covered by the hash; register it with register_hashed / \
         register_hashed_resource, or exempt_from_hash with a reason:\n  {}",
        missing.join("\n  ")
    );
}

fn skirmish(seed: u64) -> HeadlessConfig {
    HeadlessConfig {
        scenario: Scenario::Skirmish,
        ..config(seed)
    }
}

/// NFR-08 with armies (M3): the playtest start plus 200 random orders of
/// every kind (some invalid) gives the same hashes run in parallel or
/// serially, and replays from its recorded log.
#[test]
fn armies_with_orders_are_deterministic_and_replay() {
    const ARMY_TICKS: u64 = 6_000;
    let commands = {
        let mut probe = headless_app(skirmish(5));
        sample_orders(&mut probe, 5, ARMY_TICKS, COMMANDS)
    };
    let multi = run(skirmish(5), ARMY_TICKS, &commands, CHECKPOINT_EVERY);
    let single = run(
        HeadlessConfig {
            threading: Threading::Single,
            ..skirmish(5)
        },
        ARMY_TICKS,
        &commands,
        CHECKPOINT_EVERY,
    );
    assert_same(&multi, &single, "armies: multi- vs single-threaded");
    assert_eq!(multi.log.len(), COMMANDS, "every player order is logged");
    let replay = record_replay(skirmish(5), ARMY_TICKS, &commands, CHECKPOINT_EVERY);
    assert_eq!(replay.scenario, "skirmish");
    let reloaded = Replay::from_ron(&replay.to_ron()).expect("replay parses");
    verify_replay(&reloaded, Threading::Multi).expect("army replay reproduces");
}
