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
  --commands <n>            inject n deterministic no-op commands
  --checkpoint-every <n>    hash every n ticks (default 1000; 0 = end only)
  --single-threaded         run systems one at a time
  --record <file.ron>       write a replay file
  --replay <file.ron>       re-run a replay file and check its hashes";

struct Args {
    seed: u64,
    ticks: u64,
    agents: u32,
    commands: usize,
    checkpoint_every: u64,
    threading: Threading,
    record: Option<String>,
    replay: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seed: 42,
        ticks: 1000,
        agents: DEFAULT_DEMO_AGENTS,
        commands: 0,
        checkpoint_every: 1000,
        threading: Threading::Multi,
        record: None,
        replay: None,
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
            "--commands" => args.commands = num(&flag, value()?)?,
            "--checkpoint-every" => args.checkpoint_every = num(&flag, value()?)?,
            "--single-threaded" => args.threading = Threading::Single,
            "--record" => args.record = Some(value()?),
            "--replay" => args.replay = Some(value()?),
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
    };
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
    report_speed(args.ticks, start);
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
