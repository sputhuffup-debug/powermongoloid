//! NFR-08: same seed + same command stream -> bit-identical state hash.
//! See ADR 0003 (state hash) and ADR 0004 (harness and replays).

use bevy_ecs::prelude::*;
use sim_core::{
    CommandKind, CommandLog, CommandScript, Difficulty, FactionId, LoggedCommand, OrderSpec,
    Replay, RunConfig, RunState, SimCommand, SimStepper, SimTick, SpawnId, unhashed_state,
};
use sim_headless::{
    HeadlessConfig, RunResult, Scenario, Threading, headless_app, record_replay, run,
    sample_commands, sample_orders, start_boon_pick, verify_replay,
};
use sim_military::{ArmySpec, ArmyStats, Captain, RunProgress, spawn_army};
use sim_world::{Settlement, SettlementSize, SizeTier};

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

/// The same coverage on a run (M4, GP-21, GP-27): the run island, the
/// start boon picked at tick 0 and random orders, so the run's components
/// and resources exist. Later M4 steps extend this run with their state.
/// Step 2 (GP-05, GP-06): a second warband, spawned before the first tick
/// beside the hamlet nearest the start, takes it with an Attack and then
/// garrisons it, so owned land, its loyalty and a garrison exist too.
/// This run ends before a day tick reaches the conquest; serial versus
/// parallel with an owned settlement at day ticks (loyalty drift, the
/// unrest clock, a revolt) is `conquest.rs`'s
/// `a_conquest_and_its_revolt_are_thread_independent`.
/// Step 3 (GP-22, SIM-38): another settlement's stores spoil before the
/// first tick, so it posts a request; the attack's provocation and
/// killings move goodwill; and once the random orders are over the
/// champion forages, so wild food is taken.
/// Step 4 (GP-23): guard camps stand at some monuments from the setup, and
/// a 4-man band spawned before the first tick at a quiet unguarded
/// monument takes it by holding it, so a `Holder` changes hands and
/// `FactionModifiers` carries a monument bonus.
/// Step 6 (SIM-35, SIM-36): the warband's assault is a battle that ends
/// within the run, so soldiers count battles and rank up, and the champion
/// carries points from the conquest and the monument.
#[test]
fn run_state_hash_covers_every_sim_component() {
    let mut app = headless_app(HeadlessConfig::run(42, RunConfig::default()));
    let mut commands: Vec<LoggedCommand> = start_boon_pick(&app, 0).into_iter().collect();
    commands.extend(sample_orders(&mut app, 42, 600, 30));
    app.insert_resource(CommandScript::from_log(&commands));
    let (hamlet, hamlet_id, warband) = spawn_beside_nearest_hamlet(&mut app);
    let warband_id = app.world().get::<SpawnId>(warband).unwrap().0;
    let hungry = spoil_food_near_start(&mut app, hamlet);
    let monument = band_at_a_quiet_monument(&mut app);
    // Step 7 (GP-28): the rival levies and raids early (its clock moved
    // forward before the first tick), so its bands are on the field.
    {
        let mut rival = app.world_mut().resource_mut::<sim_military::RivalState>();
        rival.next_levy = 200;
        rival.next_raid = 300;
    }
    let champion = sim_military::champion_of(app.world_mut(), FactionId::PLAYER).unwrap();
    let champion_id = app.world().get::<SpawnId>(champion).unwrap().0;
    let issue = |app: &mut sim_headless::App, order: OrderSpec| {
        let kind = CommandKind::IssueOrder {
            captain: warband_id,
            order,
            queue: false,
        };
        app.world_mut()
            .write_message(SimCommand::player(FactionId::PLAYER, kind));
    };
    issue(
        &mut app,
        OrderSpec::Attack {
            settlement: hamlet_id,
        },
    );
    let mut garrisoned = false;
    for _ in 0..15 {
        if app.world().resource::<SimTick>().0 == 700 {
            // The random orders are over: the champion lives off the land.
            let forage = OrderSpec::Forage {
                radius: app
                    .world()
                    .resource::<sim_military::Progression>()
                    .data
                    .forage
                    .default_radius,
            };
            let kind = CommandKind::IssueOrder {
                captain: champion_id,
                order: forage,
                queue: false,
            };
            app.world_mut()
                .write_message(SimCommand::player(FactionId::PLAYER, kind));
        }
        app.step(100);
        if !garrisoned && sim_military::owner_of(app.world(), hamlet) == FactionId::PLAYER {
            let garrison = OrderSpec::Garrison {
                settlement: hamlet_id,
            };
            issue(&mut app, garrison);
            garrisoned = true;
        }
    }
    assert_eq!(
        app.world().resource::<SimTick>().0,
        1_500,
        "the run goes on"
    );
    assert_eq!(app.world().resource::<RunState>().outcome, None);
    assert_eq!(
        app.world().resource::<RunProgress>().boons.len(),
        1,
        "the start boon was picked"
    );
    let stats = app.world().resource::<ArmyStats>().get(FactionId::PLAYER);
    let taken = app
        .world()
        .resource::<sim_core::Chronicle>()
        .entries()
        .iter()
        .find(|e| e.event.kind == sim_core::SimEventKind::Conquest && e.event.subject == hamlet_id)
        .map(|e| e.tick);
    println!(
        "hamlet #{hamlet_id} taken at {taken:?}; conquests {}",
        stats.conquests
    );
    assert!(stats.conquests >= 1, "no conquest: {stats:?}");
    let board = app.world().resource::<sim_military::RequestBoard>().clone();
    let liked = {
        let world = app.world_mut();
        let mut q = world.query::<&sim_agents::Goodwill>();
        q.iter(world).filter(|g| !g.entries().is_empty()).count()
    };
    println!(
        "requests posted {} (#{hungry} asks: {:?}); foraged {:.2}; {liked} settlements with goodwill",
        board.posted,
        board.of(hungry).map(|r| r.kind),
        stats.foraged
    );
    assert!(
        board.posted >= 1 && board.of(hungry).is_some(),
        "no request posted"
    );
    assert!(stats.foraged > 0.0, "the champion foraged nothing");
    assert!(liked > 0, "no deed moved goodwill");
    assert!(garrisoned, "hamlet #{hamlet_id} not taken");
    let held = app
        .world()
        .resource::<sim_core::Chronicle>()
        .entries()
        .iter()
        .find(|e| {
            e.event.kind == sim_core::SimEventKind::MonumentTaken && e.event.subject == monument
        })
        .map(|e| (e.tick, e.event.faction));
    let bonuses = app
        .world()
        .resource::<sim_core::FactionModifiers>()
        .breakdown
        .iter()
        .filter(|e| e.faction == Some(FactionId::PLAYER))
        .count();
    println!("monument #{monument} taken: {held:?}; {bonuses} player modifier entries");
    assert_eq!(
        held.map(|h| h.1),
        Some(FactionId::PLAYER),
        "monument #{monument} not taken"
    );
    assert!(bonuses >= 1, "the monument's bonus is in the modifiers");
    let front = app
        .world()
        .get::<Captain>(warband)
        .and_then(|c| c.orders.front().copied());
    assert_eq!(
        front.map(|o| o.spec),
        Some(OrderSpec::Garrison {
            settlement: hamlet_id
        }),
        "the warband garrisons its hamlet"
    );
    // Step 5 (GP-24, GP-25): the hourly tally counts the conquered hamlet
    // in its region, and the stored captain slots match the live rule.
    let regions = app.world().resource::<sim_world::WorldMap>().region_count();
    let progress = app.world().resource::<RunProgress>().clone();
    let held: u32 = progress.dominion.iter().map(|t| t.held_by_player).sum();
    println!(
        "dominion {:?}, captain slots {}",
        progress.dominion, progress.captain_slots
    );
    assert_eq!(progress.dominion.len(), regions);
    assert!(held > 0, "the hamlet's people are tallied as held");
    assert_eq!(
        Some(progress.captain_slots),
        sim_military::captain_slots(app.world_mut(), FactionId::PLAYER)
    );
    // Step 6 (SIM-35, SIM-36): the assault on the hamlet was a battle,
    // which ended an hour after its last blow; its fighters count it and
    // rank up, and the conquest and the monument earned the champion
    // points.
    let battle = app
        .world()
        .resource::<sim_core::Chronicle>()
        .entries()
        .iter()
        .find(|e| e.event.kind == sim_core::SimEventKind::Battle && e.event.subject == warband_id)
        .map(|e| (e.tick, e.event.faction, e.event.other, e.event.amount));
    let veterans = {
        let world = app.world_mut();
        let mut q = world.query::<&sim_military::Veteran>();
        q.iter(world)
            .filter(|v| v.battles > 0 && v.rank > 0)
            .count()
    };
    let rank = *app
        .world()
        .get::<sim_military::ChampionRank>(champion)
        .unwrap();
    println!(
        "warband battle (tick, faction, fought, lost) {battle:?}; {veterans} ranked; {rank:?}"
    );
    assert_eq!(
        battle.map(|b| b.1),
        Some(FactionId::PLAYER),
        "no battle ended"
    );
    assert!(veterans > 0, "no soldier ranked up");
    let points = app
        .world()
        .resource::<sim_military::Progression>()
        .data
        .champion
        .points;
    assert!(
        rank.points >= points.settlement + points.monument,
        "{rank:?}"
    );
    // Step 7 (GP-28, GP-16): the rival levied and sent a band; the
    // director's state is kept (its bands, if any, carry `Raider`).
    let rival = app.world().resource::<sim_military::RivalState>().clone();
    let director = app.world().resource::<sim_military::Director>().clone();
    let bands = {
        let world = app.world_mut();
        let mut q = world.query::<&sim_military::RivalBand>();
        q.iter(world).count()
    };
    println!("rival {rival:?}; {bands} rival bands; director {director:?}");
    assert!(rival.levied > 0, "the rival levied nobody");
    assert!(rival.raids >= 1 && bands >= 1, "no rival band out");
    let missing = unhashed_state(app.world_mut());
    assert!(
        missing.is_empty(),
        "run state not covered by the hash; register it with register_hashed / \
         register_hashed_resource, or exempt_from_hash with a reason:\n  {}",
        missing.join("\n  ")
    );
}

/// The hamlet nearest the run's start (never the rival's seat), and a
/// 30-man armed player warband spawned before the first tick 20 tiles from
/// its centre, toward the start: (hamlet, its SpawnId, the captain).
fn spawn_beside_nearest_hamlet(app: &mut sim_headless::App) -> (Entity, u64, Entity) {
    let world = app.world_mut();
    let run = world.resource::<RunProgress>();
    let (start, seat) = (run.start, run.rival_seat);
    let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementSize)>();
    let (_, id, hamlet, center) = q
        .iter(world)
        .filter(|(_, id, _, size)| size.tier == SizeTier::Hamlet && Some(id.0) != seat)
        .map(|(e, id, s, _)| ((s.center - start).as_vec2().length(), id.0, e, s.center))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .expect("a hamlet on the run island");
    let toward = (start - center).as_vec2().normalize_or_zero();
    let at = center + (toward * 20.0).round().as_ivec2();
    let spec = ArmySpec {
        armed: true,
        ..ArmySpec::new(FactionId::PLAYER, at, 30)
    };
    (hamlet, id, spawn_army(world, &spec))
}

/// Step 4 (GP-23): a 4-man player band spawned before the first tick at
/// the first unguarded monument (SpawnId order) more than 40 tiles from
/// the start and from every guard camp, so it takes the monument by
/// holding it and `FactionModifiers` carries its bonus. Returns the
/// monument's SpawnId.
fn band_at_a_quiet_monument(app: &mut sim_headless::App) -> u64 {
    let world = app.world_mut();
    let start = world.resource::<RunProgress>().start;
    let mut q = world.query::<(&SpawnId, &sim_core::Position, &sim_military::Holder)>();
    let mut sites: Vec<(u64, bevy_math::IVec2, FactionId)> = q
        .iter(world)
        .map(|(id, p, h)| (id.0, p.0, h.faction))
        .collect();
    sites.sort_unstable_by_key(|s| s.0);
    let far = |a: bevy_math::IVec2, b: bevy_math::IVec2| (a - b).as_vec2().length() > 40.0;
    let &(id, at, _) = sites
        .iter()
        .find(|(_, at, f)| {
            *f == FactionId::INDEPENDENT
                && far(*at, start)
                && sites
                    .iter()
                    .filter(|s| s.2 != FactionId::INDEPENDENT)
                    .all(|s| far(s.1, *at))
        })
        .expect("a quiet unguarded monument");
    spawn_army(world, &ArmySpec::new(FactionId::PLAYER, at, 4));
    id
}

/// Spoils the food in the storehouse and homes of the populated settlement
/// nearest the run's start other than `except` (a setup edit before the
/// first tick), so it asks for food within the hour (GP-22). Returns its
/// SpawnId.
fn spoil_food_near_start(app: &mut sim_headless::App, except: Entity) -> u64 {
    use sim_economy::{Economy, SettlementEconomy, Stockpile};
    let world = app.world_mut();
    let start = world.resource::<RunProgress>().start;
    let econ = world.resource::<Economy>().clone();
    let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementEconomy)>();
    let (_, id, se) = q
        .iter(world)
        .filter(|(e, ..)| *e != except)
        .map(|(_, id, s, se)| ((s.center - start).as_vec2().length(), id.0, se.clone()))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .expect("a second settlement");
    for store in std::iter::once(se.storehouse).chain(se.homes.iter().copied()) {
        if let Some(mut stock) = world.get_mut::<Stockpile>(store) {
            stock.goods.retain(|g, _| !econ.is_food(*g));
        }
    }
    id
}

/// NFR-08 for runs (GP-21): a run's replay carries its `RunConfig` in the
/// header and reproduces in parallel and serially, with the start boon
/// pick at tick 0 and random orders (some refused).
#[test]
fn a_run_replays() {
    const RUN_TICKS: u64 = 2_000;
    let cfg = RunConfig {
        difficulty: Difficulty::Harsh,
    };
    let config = HeadlessConfig::run(9, cfg);
    let commands = {
        let mut probe = headless_app(config);
        let mut c: Vec<LoggedCommand> = start_boon_pick(&probe, 1).into_iter().collect();
        assert_eq!(c.len(), 1, "a start boon to pick");
        c.extend(sample_orders(&mut probe, 9, RUN_TICKS, 60));
        c
    };
    let replay = record_replay(config, RUN_TICKS, &commands, 500);
    assert_eq!(replay.scenario, "run");
    assert_eq!(replay.run, Some(cfg), "the run's config is in the header");
    assert_eq!(
        replay.commands, commands,
        "every player command is logged once, in order"
    );
    let reloaded = Replay::from_ron(&replay.to_ron()).expect("replay parses");
    assert_eq!(reloaded, replay);
    verify_replay(&reloaded, Threading::Multi).expect("run replay reproduces");
    verify_replay(&reloaded, Threading::Single).expect("run replay reproduces serially");
}

/// NFR-08, GP-16: the scripted player (`sim_headless::bot`) plays only
/// through recorded player commands, so its log reproduces its run. The
/// bot plays two game days in parallel; its log and checkpoints are
/// packed as a replay, written as RON and read back, and the replay
/// verifies serially.
#[test]
fn a_bot_run_replays() {
    const BOT_TICKS: u64 = 4_800;
    const EVERY: u64 = 1_200;
    let config = HeadlessConfig::run(42, RunConfig::default());
    let played = sim_headless::bot::run_bot(config, BOT_TICKS, EVERY);
    println!(
        "bot: {} commands, outcome {:?}, end tick {}",
        played.log.len(),
        played.outcome,
        played.end_tick,
    );
    assert_eq!(played.outcome, None, "the run goes on");
    assert_eq!(played.end_tick, BOT_TICKS);
    let first = played.log.first().expect("the bot played");
    assert_eq!(first.tick, 0);
    assert!(
        matches!(first.command.kind, CommandKind::ChooseOffer { .. }),
        "the bot picks a start boon at tick 0: {first:?}"
    );
    for kind in ["IssueOrder", "AppointCaptain"] {
        let n = played
            .log
            .iter()
            .filter(|c| format!("{:?}", c.command.kind).starts_with(kind))
            .count();
        println!("{kind}: {n}");
    }
    assert!(
        played
            .log
            .iter()
            .any(|c| matches!(c.command.kind, CommandKind::IssueOrder { .. })),
        "the bot gives orders"
    );
    let ticks: Vec<u64> = played.checkpoints.iter().map(|c| c.tick).collect();
    assert_eq!(ticks, [1_200, 2_400, 3_600, 4_800]);
    let replay = Replay {
        format: sim_core::REPLAY_FORMAT,
        seed: config.seed,
        population: config.population,
        world_size: config.world_size,
        scenario: config.scenario.name(),
        run: config.scenario.run_config(),
        ticks: BOT_TICKS,
        checkpoint_every: EVERY,
        commands: played.log.clone(),
        checkpoints: played.checkpoints.clone(),
    };
    let reloaded = Replay::from_ron(&replay.to_ron()).expect("replay parses");
    assert_eq!(reloaded, replay, "RON round-trip");
    verify_replay(&reloaded, Threading::Single).expect("the bot's run replays serially");
}

/// NFR-08, guide G1: a battle's setup orders are AI commands, so a
/// `--battle` replay logs none of them, and verifying it (which re-runs
/// the setup) applies them once, in parallel and serially. As player
/// commands they were logged and then applied twice on replay.
#[test]
fn a_battle_replays_without_logging_setup_orders() {
    let config = HeadlessConfig {
        scenario: Scenario::Battle(20),
        // Enough villages for the skirmish start, fewer to set up.
        population: 1_500,
        ..config(3)
    };
    let replay = record_replay(config, 200, &[], 100);
    assert_eq!(replay.scenario, "battle:20");
    assert!(
        replay.commands.is_empty(),
        "setup orders are AI-origin and never recorded (G1): {:?}",
        replay.commands
    );
    verify_replay(&replay, Threading::Multi).expect("battle replay reproduces");
    verify_replay(&replay, Threading::Single).expect("battle replay reproduces serially");
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
