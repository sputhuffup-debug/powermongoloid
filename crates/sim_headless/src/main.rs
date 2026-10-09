//! Runs the simulation with no window.
//!
//!   sim_headless --seed 42 --ticks 10000            print the final state hash
//!   sim_headless --commands 200 --record run.ron    record a replay file
//!   sim_headless --replay run.ron                   verify a replay file
//!   sim_headless --run --difficulty harsh           a run on the run island
//!   sim_headless --run --bot --ticks 24000          the scripted player plays it
//!   sim_headless --run --ticks 2400 --save day1.sav save the run after a day
//!   sim_headless --load day1.sav --ticks 1000       resume it for 1000 more ticks
//!
//! Exit code 1 on bad arguments, 2 if a replay diverges.

use std::process::ExitCode;
use std::time::Instant;

use sim_core::{Difficulty, Replay, RunConfig};
use sim_headless::{
    HeadlessConfig, Scenario, Threading, record_replay, sample_commands, verify_replay_saving,
};

const USAGE: &str = "\
usage: sim_headless [options]
  --seed <u64>              world seed (default 42)
  --ticks <u64>             ticks to run (default 1000)
  --population <u32>        villagers to populate (default: villagers.ron,
                            5000; not with --run, whose settlements are
                            populated at their drawn sizes)
  --world-size <tiles>      map size, a multiple of 64 (default 2048; 1024
                            with --run)
  --commands <n>            inject n deterministic no-op commands
  --checkpoint-every <n>    hash every n ticks (default 1000; 0 = end only)
  --single-threaded         run systems one at a time
  --stats-every <n>         print a population summary every n ticks
  --record <file.ron>       write a replay file
  --map-ppm <file.ppm>      write the generated terrain as an image
  --replay <file.ron>       re-run a replay file and check its hashes
  --timings                 print the mean wall-clock time of each sim stage
  --skirmish                set up the M3 playtest armies (with --commands:
                            random military orders for them)
  --battle <n>              the skirmish plus two armies of n meeting in battle
  --run                     a run on the run island (GP-21): the band, its
                            champion and the start boon offer; the first
                            boon is picked at tick 0 (with --commands: plus
                            random military orders)
  --difficulty <name>       the run's difficulty: steady (default), chaotic
                            or harsh (needs --run)
  --bot                     the scripted player plays the run (needs --run;
                            not with --commands); with --record,
                            --stats-every or --timings its commands are
                            replayed
  --save <file>             save the game (GP-12) at the end of the run, or
                            of the --replay
  --save-at <tick>          save after this tick instead of the end (the
                            run carries on)
  --load <file>             resume a saved game: --ticks more ticks, or with
                            --replay, the rest of that replay (its commands
                            from the saved tick on), checking its hashes
  --chronicle               at the end, print the chronicle in words and the
                            run summary (UI-09; a plain run, --bot or --load)";

struct Args {
    seed: u64,
    ticks: u64,
    /// `None`: villagers.ron's total. Refused with `--run`, which populates
    /// every settlement at its drawn size (worldgen.ron `sizes`).
    population: Option<u32>,
    /// `None`: the scenario's preset size.
    world_size: Option<u32>,
    commands: usize,
    checkpoint_every: u64,
    threading: Threading,
    record: Option<String>,
    replay: Option<String>,
    map_ppm: Option<String>,
    stats_every: u64,
    scenario: Scenario,
    difficulty: Option<Difficulty>,
    timings: bool,
    bot: bool,
    save: Option<String>,
    save_at: Option<u64>,
    load: Option<String>,
    chronicle: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seed: 42,
        ticks: 1000,
        population: None,
        world_size: None,
        commands: 0,
        checkpoint_every: 1000,
        threading: Threading::Multi,
        record: None,
        replay: None,
        map_ppm: None,
        stats_every: 0,
        scenario: Scenario::Villages,
        difficulty: None,
        timings: false,
        bot: false,
        save: None,
        save_at: None,
        load: None,
        chronicle: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        fn num<T: std::str::FromStr>(flag: &str, v: String) -> Result<T, String>
        where
            T::Err: std::fmt::Display,
        {
            v.parse().map_err(|e| format!("{flag}: {e}"))
        }
        match flag.as_str() {
            "--seed" => args.seed = num(&flag, value()?)?,
            "--ticks" => args.ticks = num(&flag, value()?)?,
            "--population" => args.population = Some(num(&flag, value()?)?),
            "--world-size" => args.world_size = Some(num(&flag, value()?)?),
            "--commands" => args.commands = num(&flag, value()?)?,
            "--checkpoint-every" => args.checkpoint_every = num(&flag, value()?)?,
            "--single-threaded" => args.threading = Threading::Single,
            "--record" => args.record = Some(value()?),
            "--replay" => args.replay = Some(value()?),
            "--map-ppm" => args.map_ppm = Some(value()?),
            "--stats-every" => args.stats_every = num(&flag, value()?)?,
            "--skirmish" => args.scenario = Scenario::Skirmish,
            "--battle" => args.scenario = Scenario::Battle(num(&flag, value()?)?),
            "--run" => args.scenario = Scenario::Run(RunConfig::default()),
            "--difficulty" => {
                let v = value()?;
                args.difficulty = Some(Difficulty::parse(&v).ok_or(format!(
                    "--difficulty: unknown {v:?} (steady, chaotic or harsh)"
                ))?);
            }
            "--timings" => args.timings = true,
            "--bot" => args.bot = true,
            "--save" => args.save = Some(value()?),
            "--save-at" => args.save_at = Some(num(&flag, value()?)?),
            "--load" => args.load = Some(value()?),
            "--chronicle" => args.chronicle = true,
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    if args.record.is_some() && args.replay.is_some() {
        return Err("--record and --replay can't be combined".to_string());
    }
    if let Some(difficulty) = args.difficulty {
        let Scenario::Run(cfg) = &mut args.scenario else {
            return Err("--difficulty needs --run".to_string());
        };
        cfg.difficulty = difficulty;
    }
    if args.bot {
        if !matches!(args.scenario, Scenario::Run(_)) {
            return Err("--bot needs --run".to_string());
        }
        if args.commands > 0 {
            return Err("--bot can't be combined with --commands".to_string());
        }
    }
    if args.chronicle
        && (args.replay.is_some() && args.load.is_none()
            || args.record.is_some()
            || args.stats_every > 0
            || args.timings)
    {
        return Err(
            "--chronicle goes with a plain run, --bot or --load (not --record, --replay, \
             --stats-every or --timings)"
                .to_string(),
        );
    }
    if args.save_at.is_some() && args.save.is_none() {
        return Err("--save-at needs --save".to_string());
    }
    if args.save.is_some()
        && (args.record.is_some() || args.bot || args.stats_every > 0 || args.timings)
    {
        return Err(
            "--save works with a plain run, --replay or --load (not --record, --bot, --stats-every or --timings)"
                .to_string(),
        );
    }
    if args.load.is_some()
        && (args.scenario != Scenario::Villages
            || args.difficulty.is_some()
            || args.population.is_some()
            || args.world_size.is_some()
            || args.commands > 0
            || args.record.is_some()
            || args.map_ppm.is_some()
            || args.stats_every > 0
            || args.timings)
    {
        return Err(
            "--load takes the world from the save: only --ticks, --replay, --checkpoint-every, \
             --single-threaded and --save go with it"
                .to_string(),
        );
    }
    if args.population.is_some() && matches!(args.scenario, Scenario::Run(_)) {
        return Err(
            "--population has no effect with --run (settlements use their drawn sizes)".to_string(),
        );
    }
    Ok(args)
}

fn report_speed(ticks: u64, start: Instant) {
    let secs = start.elapsed().as_secs_f64();
    eprintln!(
        "elapsed={secs:.3}s ticks_per_sec={:.0}",
        ticks as f64 / secs.max(f64::EPSILON)
    );
}

/// Prints what the generated world contains (SIM-01).
fn print_world_summary(app: &bevy_app::App) {
    use sim_world::{RoadNetwork, Terrain, WorldGenReport, WorldMap, worldgen};
    let world = app.world();
    let map = world.resource::<WorldMap>();
    let report = world.resource::<WorldGenReport>();
    let roads = world.resource::<RoadNetwork>();
    let settlements = world
        .try_query::<&sim_world::Settlement>()
        .map_or(0, |mut q| q.iter(world).count());
    let counts = map.terrain_counts();
    let total: u64 = counts.iter().sum();
    let mix: Vec<String> = Terrain::ALL
        .iter()
        .map(|t| {
            format!(
                "{}={:.1}%",
                t.name(),
                100.0 * counts[*t as usize] as f64 / total as f64
            )
        })
        .collect();
    eprintln!(
        "world: {0}x{0} generated in {1:.0}ms ({2})",
        map.size(),
        report.total.as_secs_f64() * 1000.0,
        worldgen::format_timings(&report.stages)
    );
    eprintln!("terrain: {}", mix.join(" "));
    eprintln!(
        "settlements={} roads={} road_tiles={}",
        settlements,
        roads.roads.len(),
        counts[Terrain::Road as usize]
    );
    print_run_summary(app);
}

/// Prints the regions, monuments, settlement sizes and people, and, in a
/// run, the rival's seat and the start draws (GP-21, GP-23, GP-27).
fn print_run_summary(app: &bevy_app::App) {
    use sim_core::{NameTable, Offers, SpawnId};
    use sim_military::{Champion, Progression, RunProgress, Traits};
    use sim_world::{Monument, MonumentKind, Settlement, SettlementSize, SizeTier, WorldMap};
    let world = app.world();
    let names = world.resource::<NameTable>();
    let mut by_kind = [0u32; MonumentKind::COUNT];
    let mut monuments = 0;
    if let Some(mut q) = world.try_query::<&Monument>() {
        for m in q.iter(world) {
            by_kind[m.kind.index()] += 1;
            monuments += 1;
        }
    }
    let kinds: Vec<String> = MonumentKind::ALL
        .iter()
        .filter(|k| by_kind[k.index()] > 0)
        .map(|k| format!("{} {}", k.name(), by_kind[k.index()]))
        .collect();
    let villagers = world
        .try_query::<&sim_agents::Villager>()
        .map_or(0, |mut q| q.iter(world).count());
    let kinds = if kinds.is_empty() {
        String::new()
    } else {
        format!(" ({})", kinds.join(", "))
    };
    eprintln!(
        "regions={} monuments={monuments}{kinds} villagers={villagers}",
        world.resource::<WorldMap>().region_count(),
    );
    let mut tiers = [(0u32, 0u32); 3];
    if let Some(mut q) = world.try_query::<&SettlementSize>() {
        for z in q.iter(world) {
            tiers[z.tier as usize].0 += 1;
            tiers[z.tier as usize].1 += z.people;
        }
    }
    if tiers.iter().any(|t| t.0 > 0) {
        let line: Vec<String> = SizeTier::ALL
            .iter()
            .map(|t| {
                let (n, people) = tiers[*t as usize];
                format!("{} {n} ({people} people)", t.name())
            })
            .collect();
        eprintln!("sizes: {}", line.join(", "));
    }
    let progress = world.resource::<RunProgress>();
    let Some(seat) = progress.rival_seat else {
        return;
    };
    if let Some(mut q) = world.try_query::<(&SpawnId, &Settlement, Option<&SettlementSize>)>()
        && let Some((_, st, size)) = q.iter(world).find(|(id, ..)| id.0 == seat)
    {
        eprintln!(
            "seat: {} (#{seat}, {} people, region {}) at ({}, {})",
            names.get(st.name),
            size.map_or(0, |z| z.people),
            st.region,
            st.center.x,
            st.center.y
        );
    }
    let data = &world.resource::<Progression>().data;
    let card = |list: &[String], i: u16| list.get(i as usize).cloned().unwrap_or_default();
    let boon_names: Vec<String> = data.boons.iter().map(|b| b.name.clone()).collect();
    let mod_names: Vec<String> = data.modifiers.iter().map(|m| m.name.clone()).collect();
    let trait_names: Vec<String> = data.traits.iter().map(|t| t.name.clone()).collect();
    let band = data.bands.get(progress.band as usize);
    let traits: Vec<String> = world
        .try_query_filtered::<&Traits, bevy_ecs::query::With<Champion>>()
        .map(|mut q| {
            q.iter(world)
                .flat_map(|t| t.0.iter().map(|&i| card(&trait_names, i)))
                .collect()
        })
        .unwrap_or_default();
    eprintln!(
        "start: ({}, {}) region {}, {}; band {:?} ({} + champion, {:.1} days of food), traits [{}]",
        progress.start.x,
        progress.start.y,
        progress.start_region,
        progress.terrain.name(),
        band.map_or("?", |b| b.name.as_str()),
        band.map_or(0, |b| b.followers()),
        band.map_or(0.0, |b| b.food_days),
        traits.join(", ")
    );
    let mods: Vec<String> = progress
        .modifiers
        .iter()
        .map(|&m| card(&mod_names, m))
        .collect();
    let offers: Vec<String> = world
        .resource::<Offers>()
        .pending
        .iter()
        .map(|o| {
            let choices: Vec<String> = o.choices.iter().map(|&c| card(&boon_names, c)).collect();
            format!("#{} {:?}: {}", o.id, o.kind, choices.join(" / "))
        })
        .collect();
    eprintln!(
        "modifiers: [{}] offers: [{}]",
        mods.join(", "),
        offers.join("; ")
    );
}

/// Writes the terrain as a binary PPM image (1 pixel per tile), with
/// settlement centres (red crosses), monuments (white squares) and, in a
/// run, the start (cyan) and the rival's seat (magenta) marked, for
/// eyeballing world generation.
fn write_map_ppm(app: &bevy_app::App, path: &str) -> std::io::Result<()> {
    use sim_world::{Settlement, Terrain, WorldMap};
    let world = app.world();
    let map = world.resource::<WorldMap>();
    let size = map.size() as usize;
    let mut px = vec![0u8; size * size * 3];
    for (i, t) in map.terrain_slice().iter().enumerate() {
        let shade = 0.75 + 0.5 * (map.height_slice()[i] - map.sea_level()).max(0.0);
        let rgb: [f32; 3] = match t {
            Terrain::Water => [40.0, 90.0, 160.0],
            Terrain::Beach => [225.0, 210.0, 150.0],
            Terrain::Grass => [110.0, 165.0, 80.0],
            Terrain::Farmland => [200.0, 180.0, 70.0],
            Terrain::Forest => [40.0, 105.0, 45.0],
            Terrain::Hills => [140.0, 130.0, 95.0],
            Terrain::Mountain => [150.0, 150.0, 155.0],
            Terrain::Road => [120.0, 70.0, 40.0],
        };
        let s = if *t == Terrain::Water { 1.0 } else { shade };
        // Image rows run top to bottom; tile y runs bottom to top.
        let (x, y) = (i % size, size - 1 - i / size);
        for c in 0..3 {
            px[(y * size + x) * 3 + c] = (rgb[c] * s).clamp(0.0, 255.0) as u8;
        }
    }
    let mut q = world
        .try_query::<&Settlement>()
        .expect("Settlement is registered");
    for s in q.iter(world) {
        for d in -3..=3i32 {
            for p in [
                s.center + bevy_math::IVec2::new(d, 0),
                s.center + bevy_math::IVec2::new(0, d),
            ] {
                if map.in_bounds(p) {
                    let (x, y) = (p.x as usize, size - 1 - p.y as usize);
                    px[(y * size + x) * 3..(y * size + x) * 3 + 3].copy_from_slice(&[255, 30, 30]);
                }
            }
        }
    }
    // A filled square of `r` tiles around `c`.
    let mut mark = |c: bevy_math::IVec2, r: i32, rgb: [u8; 3]| {
        for dy in -r..=r {
            for dx in -r..=r {
                let p = c + bevy_math::IVec2::new(dx, dy);
                if map.in_bounds(p) {
                    let (x, y) = (p.x as usize, size - 1 - p.y as usize);
                    px[(y * size + x) * 3..(y * size + x) * 3 + 3].copy_from_slice(&rgb);
                }
            }
        }
    };
    if let Some(mut q) = world.try_query::<(&sim_world::Monument, &sim_core::Position)>() {
        for (_, p) in q.iter(world) {
            mark(p.0, 2, [255, 255, 255]);
        }
    }
    let progress = world.resource::<sim_military::RunProgress>();
    if progress.rival_seat.is_some() {
        mark(progress.seat_center, 4, [230, 40, 230]);
        mark(progress.start, 4, [40, 230, 230]);
    }
    let mut out = format!("P6\n{size} {size}\n255\n").into_bytes();
    out.extend_from_slice(&px);
    std::fs::write(path, out)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(path) = &args.load {
        return resume(&args, path);
    }

    if let Some(path) = &args.replay {
        let replay = match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| Replay::from_ron(&text).map_err(|e| e.to_string()))
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::FAILURE;
            }
        };
        let start = Instant::now();
        let save_at = args
            .save
            .as_ref()
            .map(|_| args.save_at.unwrap_or(replay.ticks));
        let (outcome, saved) = verify_replay_saving(&replay, args.threading, save_at);
        report_speed(replay.ticks, start);
        if let Err(code) = write_save(args.save.as_deref(), saved) {
            return code;
        }
        return match outcome {
            Ok(result) => {
                println!(
                    "replay ok: seed={} ticks={} commands={} checkpoints={}",
                    replay.seed,
                    replay.ticks,
                    replay.commands.len(),
                    result.checkpoints.len()
                );
                println!("state_hash=0x{:016x}", result.final_hash());
                ExitCode::SUCCESS
            }
            Err(divergence) => {
                println!("replay FAILED: {divergence}");
                ExitCode::from(2)
            }
        };
    }

    let config = {
        let base = match args.scenario {
            Scenario::Run(cfg) => HeadlessConfig::run(args.seed, cfg),
            scenario => HeadlessConfig {
                scenario,
                ..HeadlessConfig::new(args.seed)
            },
        };
        HeadlessConfig {
            population: args.population.unwrap_or(base.population),
            threading: args.threading,
            world_size: args.world_size.unwrap_or(base.world_size),
            ..base
        }
    };
    if let Err(e) = config.worldgen().validate() {
        eprintln!("--world-size: {e}");
        return ExitCode::FAILURE;
    }
    let commands;
    {
        let mut app = sim_headless::headless_app(config);
        print_world_summary(&app);
        commands = if args.scenario != Scenario::Villages {
            // In a run, the start boon's first choice is picked at tick 0.
            let mut out: Vec<_> = sim_headless::start_boon_pick(&app, 0).into_iter().collect();
            out.extend(sim_headless::sample_orders(
                &mut app,
                args.seed,
                args.ticks,
                args.commands,
            ));
            out
        } else {
            sample_commands(args.seed, args.ticks, args.commands)
        };
        if let Some(path) = &args.map_ppm {
            if let Err(e) = write_map_ppm(&app, path) {
                eprintln!("{path}: {e}");
                return ExitCode::FAILURE;
            }
            eprintln!("wrote {path}");
        }
    }
    let start = Instant::now();

    // The bot plays first; its recorded commands then drive the replay.
    let mut played_checkpoints = Vec::new();
    let commands = if args.bot {
        let (bot_app, played) =
            sim_headless::bot::play_bot(config, args.ticks, args.checkpoint_every);
        for (tick, note) in &played.notes {
            eprintln!("bot @{tick}: {note}");
        }
        eprintln!(
            "bot: {} commands, outcome {:?} ({:?}) at tick {}",
            played.log.len(),
            played.outcome.map(|o| o.name()),
            played.cause,
            played.end_tick
        );
        // What happened, by kind, with the first few ticks of each.
        let mut kinds: std::collections::BTreeMap<&str, Vec<u64>> = Default::default();
        for e in &played.chronicle {
            kinds.entry(e.event.kind.name()).or_default().push(e.tick);
        }
        for (kind, ticks) in kinds {
            let first: Vec<u64> = ticks.iter().take(8).copied().collect();
            eprintln!("chronicle: {kind} x{} at {first:?}", ticks.len());
        }
        if args.record.is_none() && args.stats_every == 0 && !args.timings {
            report_speed(played.end_tick, start);
            println!(
                "seed={} ticks={} bot outcome={} end_tick={}",
                args.seed,
                args.ticks,
                played.outcome.map_or("none", |o| o.name()),
                played.end_tick
            );
            println!("state_hash=0x{:016x}", played.final_hash);
            if args.chronicle {
                print_chronicle(&bot_app);
            }
            return ExitCode::SUCCESS;
        }
        played_checkpoints = played.checkpoints;
        played.log
    } else {
        commands
    };

    if let Some(path) = &args.record {
        let replay = record_replay(config, args.ticks, &commands, args.checkpoint_every);
        report_speed(args.ticks, start);
        // The bot's own run and the replay of its commands must agree.
        for c in &played_checkpoints {
            if let Some(r) = replay.checkpoints.iter().find(|r| r.tick == c.tick)
                && r.hash != c.hash
            {
                eprintln!(
                    "the bot's commands did not reproduce its run at tick {}: 0x{:016x} vs 0x{:016x}",
                    c.tick, c.hash, r.hash
                );
                return ExitCode::from(2);
            }
        }
        if let Err(e) = std::fs::write(path, replay.to_ron()) {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
        println!(
            "recorded {path}: seed={} ticks={} commands={}",
            replay.seed,
            replay.ticks,
            replay.commands.len()
        );
        println!(
            "state_hash=0x{:016x}",
            replay.checkpoints.last().map_or(0, |c| c.hash)
        );
        return ExitCode::SUCCESS;
    }

    if args.timings {
        use sim_core::{SimSet, SimStepper, SimTimings};
        let mut app = sim_headless::headless_app(config);
        app.insert_resource(sim_core::CommandScript::from_log(&commands));
        let mut sums = [0.0f64; SimSet::ALL.len()];
        let mut total = 0.0;
        // With --stats-every n: mean and worst tick, and the costliest
        // stages, for each window of n ticks.
        let mut window = ([0.0f64; SimSet::ALL.len()], 0.0f64, 0.0f64);
        for tick in 1..=args.ticks {
            app.step(1);
            let t = app.world().resource::<SimTimings>();
            for (i, d) in t.per_set.iter().enumerate() {
                sums[i] += d.as_secs_f64();
                window.0[i] += d.as_secs_f64();
            }
            total += t.total.as_secs_f64();
            window.1 += t.total.as_secs_f64();
            window.2 = window.2.max(t.total.as_secs_f64());
            if args.stats_every > 0 && tick % args.stats_every == 0 {
                let n = args.stats_every as f64;
                let mut sets: Vec<_> = SimSet::ALL.iter().zip(window.0).collect();
                sets.sort_by(|a, b| b.1.total_cmp(&a.1));
                let top: Vec<String> = sets[..3]
                    .iter()
                    .map(|(set, s)| format!("{} {:.2}", set.name(), s / n * 1000.0))
                    .collect();
                let agents = app
                    .world_mut()
                    .query_filtered::<(), bevy_ecs::query::With<sim_world::Mobile>>()
                    .iter(app.world())
                    .count();
                println!(
                    "tick {tick:>6}: {agents:>6} agents | mean {:.2} ms, worst {:.2} ms | {}",
                    window.1 / n * 1000.0,
                    window.2 * 1000.0,
                    top.join(", ")
                );
                window = ([0.0; SimSet::ALL.len()], 0.0, 0.0);
            }
        }
        let n = args.ticks.max(1) as f64;
        for (set, s) in SimSet::ALL.iter().zip(sums) {
            println!("{:<12} {:>8.3} ms", set.name(), s / n * 1000.0);
        }
        println!("{:<12} {:>8.3} ms", "total", total / n * 1000.0);
        return ExitCode::SUCCESS;
    }

    if args.stats_every > 0 {
        use sim_core::SimStepper;
        let mut app = sim_headless::headless_app(config);
        app.insert_resource(sim_core::CommandScript::from_log(&commands));
        let start = Instant::now();
        let mut done = 0;
        while done < args.ticks {
            let n = args.stats_every.min(args.ticks - done);
            app.step(n);
            done += n;
            let st = sim_headless::stats::collect(&mut app);
            println!("{}", st.line());
            for line in sim_headless::stats::armies(&mut app) {
                println!("    {line}");
            }
            if std::env::var_os("STATS_DETAIL").is_some() {
                println!("    not working: {:?}", st.not_working);
                println!("    restless: {:?}", st.restless);
                println!("    hungriest: {}", st.hungriest);
            }
            // A run that has ended stands still: no more windows to report.
            let state = *app.world().resource::<sim_core::RunState>();
            if let Some(outcome) = state.outcome {
                println!(
                    "run ended: {} at tick {} ({})",
                    outcome.name(),
                    state.ended_at.unwrap_or(0),
                    state.cause.describe()
                );
                for line in sim_headless::stats::run_summary(&app) {
                    println!("    {line}");
                }
                break;
            }
        }
        let stats = sim_headless::stats::collect(&mut app);
        println!("cycles by occupation: {:?}", stats.cycles);
        println!("ever ate: {}/{}", stats.ever_ate, stats.villagers);
        for line in &stats.never_ate {
            println!("  never ate: {line}");
        }
        for v in &stats.villages {
            println!(
                "  {:<14} pop {:>4} food {:>5.2} days hunger {:.2} store {:?} waiting {:?}",
                v.name, v.population, v.food_days, v.mean_hunger, v.store, v.at_workplaces
            );
            if std::env::var_os("STATS_DETAIL").is_some() {
                println!("      jobs {:?}", v.occupations);
                println!("      workplaces {:?}", v.workplaces);
            }
        }
        report_speed(args.ticks, start);
        return ExitCode::SUCCESS;
    }
    let save_at = args
        .save
        .as_ref()
        .map(|_| args.save_at.unwrap_or(args.ticks));
    let mut app = sim_headless::headless_app(config);
    let setup_time = start.elapsed();
    let (mut result, saved) = sim_headless::continue_run(
        &mut app,
        args.ticks,
        &commands,
        args.checkpoint_every,
        save_at,
    );
    result.setup_time = setup_time;
    if let Err(code) = write_save(args.save.as_deref(), saved) {
        return code;
    }
    eprintln!(
        "setup={:.3}s ticks={:.3}s ticks_per_sec={:.0}",
        result.setup_time.as_secs_f64(),
        result.tick_time.as_secs_f64(),
        args.ticks as f64 / result.tick_time.as_secs_f64().max(f64::EPSILON)
    );
    match config.scenario {
        Scenario::Run(cfg) => println!(
            "seed={} ticks={} run difficulty={}",
            args.seed,
            args.ticks,
            cfg.difficulty.name()
        ),
        _ => println!(
            "seed={} ticks={} population={}",
            args.seed, args.ticks, config.population
        ),
    }
    for c in &result.checkpoints[..result.checkpoints.len().saturating_sub(1)] {
        println!("checkpoint tick={} hash=0x{:016x}", c.tick, c.hash);
    }
    println!("state_hash=0x{:016x}", result.final_hash());
    if args.chronicle {
        print_chronicle(&app);
    }
    ExitCode::SUCCESS
}

/// `--chronicle`: the chronicle in words and the run summary (UI-09).
fn print_chronicle(app: &bevy_app::App) {
    println!("chronicle:");
    for line in sim_headless::stats::chronicle(app) {
        println!("  {line}");
    }
    for line in sim_headless::stats::run_summary(app) {
        println!("summary: {line}");
    }
}

/// Writes a save taken during the run (GP-12) to `path`, if one was asked for.
fn write_save(path: Option<&str>, saved: Option<Vec<u8>>) -> Result<(), ExitCode> {
    let Some(path) = path else {
        return Ok(());
    };
    let Some(bytes) = saved else {
        eprintln!("{path}: the run never reached --save-at");
        return Err(ExitCode::FAILURE);
    };
    let header = sim_core::read_header(&bytes).expect("a save we just wrote");
    if let Err(e) = std::fs::write(path, &bytes) {
        eprintln!("{path}: {e}");
        return Err(ExitCode::FAILURE);
    }
    println!("saved {path}: tick {} ({} bytes)", header.tick, bytes.len());
    Ok(())
}

/// `--load`: resumes a saved game (GP-12) for `--ticks` more ticks, or to
/// the end of `--replay`, checking that replay's hashes after the save.
fn resume(args: &Args, path: &str) -> ExitCode {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let start = Instant::now();
    let mut app = match sim_headless::load_app(&bytes, args.threading) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let header = sim_core::read_header(&bytes).expect("a loaded save has a header");
    eprintln!(
        "loaded {path} in {:.0}ms: seed {} {} preset{}, tick {} (build {})",
        start.elapsed().as_secs_f64() * 1000.0,
        header.seed,
        header.preset,
        header
            .run
            .map_or(String::new(), |r| format!(", run {}", r.difficulty.name())),
        header.tick,
        header.build
    );
    let replay = match &args.replay {
        None => None,
        Some(rpath) => match std::fs::read_to_string(rpath)
            .map_err(|e| e.to_string())
            .and_then(|text| Replay::from_ron(&text).map_err(|e| e.to_string()))
        {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("{rpath}: {e}");
                return ExitCode::FAILURE;
            }
        },
    };
    let (until, commands, every) = match &replay {
        Some(r) => (r.ticks, r.commands.clone(), r.checkpoint_every),
        None => (header.tick + args.ticks, Vec::new(), args.checkpoint_every),
    };
    let save_at = args.save.as_ref().map(|_| args.save_at.unwrap_or(until));
    let start = Instant::now();
    let (result, saved) = sim_headless::continue_run(&mut app, until, &commands, every, save_at);
    report_speed(until.saturating_sub(header.tick), start);
    if let Err(code) = write_save(args.save.as_deref(), saved) {
        return code;
    }
    if let Some(replay) = &replay {
        let diverged = replay
            .checkpoints
            .iter()
            .filter(|c| c.tick > header.tick)
            .find(|c| result.checkpoints.iter().find(|a| a.tick == c.tick) != Some(c));
        if let Some(c) = diverged {
            let got = result.checkpoints.iter().find(|a| a.tick == c.tick);
            println!(
                "replay FAILED after the load: tick {} expected 0x{:016x}, got {}",
                c.tick,
                c.hash,
                got.map_or("nothing".to_string(), |a| format!("0x{:016x}", a.hash))
            );
            return ExitCode::from(2);
        }
        println!(
            "replay ok from the save at tick {}: ticks={} commands={} checkpoints={}",
            header.tick,
            replay.ticks,
            result.log.len(),
            result.checkpoints.len()
        );
    } else {
        for c in &result.checkpoints[..result.checkpoints.len().saturating_sub(1)] {
            println!("checkpoint tick={} hash=0x{:016x}", c.tick, c.hash);
        }
    }
    println!("state_hash=0x{:016x}", result.final_hash());
    if args.chronicle {
        print_chronicle(&app);
    }
    ExitCode::SUCCESS
}
