//! Runs the simulation with no window.
//!
//!   sim_headless --seed 42 --ticks 10000            print the final state hash
//!   sim_headless --commands 200 --record run.ron    record a replay file
//!   sim_headless --replay run.ron                   verify a replay file
//!
//! Exit code 1 on bad arguments, 2 if a replay diverges.

use std::process::ExitCode;
use std::time::Instant;

use sim_core::Replay;
use sim_headless::{
    DEFAULT_DEMO_AGENTS, HeadlessConfig, Threading, record_replay, run, sample_commands,
    verify_replay,
};

const USAGE: &str = "\
usage: sim_headless [options]
  --seed <u64>              world seed (default 42)
  --ticks <u64>             ticks to run (default 1000)
  --agents <u32>            M0 placeholder agents (default 256)
  --world-size <tiles>      map size, a multiple of 64 (default 2048)
  --commands <n>            inject n deterministic no-op commands
  --checkpoint-every <n>    hash every n ticks (default 1000; 0 = end only)
  --single-threaded         run systems one at a time
  --record <file.ron>       write a replay file
  --map-ppm <file.ppm>      write the generated terrain as an image
  --replay <file.ron>       re-run a replay file and check its hashes";

struct Args {
    seed: u64,
    ticks: u64,
    agents: u32,
    world_size: u32,
    commands: usize,
    checkpoint_every: u64,
    threading: Threading,
    record: Option<String>,
    replay: Option<String>,
    map_ppm: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seed: 42,
        ticks: 1000,
        agents: DEFAULT_DEMO_AGENTS,
        world_size: HeadlessConfig::new(0).world_size,
        commands: 0,
        checkpoint_every: 1000,
        threading: Threading::Multi,
        record: None,
        replay: None,
        map_ppm: None,
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
            "--agents" => args.agents = num(&flag, value()?)?,
            "--world-size" => args.world_size = num(&flag, value()?)?,
            "--commands" => args.commands = num(&flag, value()?)?,
            "--checkpoint-every" => args.checkpoint_every = num(&flag, value()?)?,
            "--single-threaded" => args.threading = Threading::Single,
            "--record" => args.record = Some(value()?),
            "--replay" => args.replay = Some(value()?),
            "--map-ppm" => args.map_ppm = Some(value()?),
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    if args.record.is_some() && args.replay.is_some() {
        return Err("--record and --replay can't be combined".to_string());
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
    let settlements = world.resource::<sim_core::NameTable>().len();
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
}

/// Writes the terrain as a binary PPM image (1 pixel per tile), with
/// settlement centres marked, for eyeballing world generation.
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
        let outcome = verify_replay(&replay, args.threading);
        report_speed(replay.ticks, start);
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

    let config = HeadlessConfig {
        seed: args.seed,
        demo_agents: args.agents,
        threading: args.threading,
        world_size: args.world_size,
    };
    if let Err(e) = config.worldgen().validate() {
        eprintln!("--world-size: {e}");
        return ExitCode::FAILURE;
    }
    {
        let app = sim_headless::headless_app(config);
        print_world_summary(&app);
        if let Some(path) = &args.map_ppm {
            if let Err(e) = write_map_ppm(&app, path) {
                eprintln!("{path}: {e}");
                return ExitCode::FAILURE;
            }
            eprintln!("wrote {path}");
        }
    }
    let commands = sample_commands(args.seed, args.ticks, args.commands);
    let start = Instant::now();

    if let Some(path) = &args.record {
        let replay = record_replay(config, args.ticks, &commands, args.checkpoint_every);
        report_speed(args.ticks, start);
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

    let result = run(config, args.ticks, &commands, args.checkpoint_every);
    let _ = start;
    eprintln!(
        "setup={:.3}s ticks={:.3}s ticks_per_sec={:.0}",
        result.setup_time.as_secs_f64(),
        result.tick_time.as_secs_f64(),
        args.ticks as f64 / result.tick_time.as_secs_f64().max(f64::EPSILON)
    );
    println!(
        "seed={} ticks={} agents={}",
        args.seed, args.ticks, args.agents
    );
    for c in &result.checkpoints[..result.checkpoints.len().saturating_sub(1)] {
        println!("checkpoint tick={} hash=0x{:016x}", c.tick, c.hash);
    }
    println!("state_hash=0x{:016x}", result.final_hash());
    ExitCode::SUCCESS
}
