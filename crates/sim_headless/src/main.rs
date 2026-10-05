//! `cargo run -p sim_headless -- --seed 42 --ticks 1000`
//!
//! Runs the simulation with no window and prints the final state hash.

use std::process::ExitCode;
use std::time::Instant;

use sim_core::{SimStepper, compute_state_hash};
use sim_headless::{DEFAULT_DEMO_AGENTS, headless_app};

const USAGE: &str = "usage: sim_headless [--seed <u64>] [--ticks <u64>] [--agents <u32>]";

struct Args {
    seed: u64,
    ticks: u64,
    agents: u32,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seed: 42,
        ticks: 1000,
        agents: DEFAULT_DEMO_AGENTS,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--seed" => args.seed = value()?.parse().map_err(|e| format!("--seed: {e}"))?,
            "--ticks" => args.ticks = value()?.parse().map_err(|e| format!("--ticks: {e}"))?,
            "--agents" => args.agents = value()?.parse().map_err(|e| format!("--agents: {e}"))?,
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    Ok(args)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };

    let mut app = headless_app(args.seed, args.agents);
    let start = Instant::now();
    app.step(args.ticks);
    let elapsed = start.elapsed();
    let hash = compute_state_hash(app.world_mut());

    println!(
        "seed={} ticks={} agents={}",
        args.seed, args.ticks, args.agents
    );
    println!("state_hash=0x{hash:016x}");
    eprintln!(
        "elapsed={:.3}s ticks_per_sec={:.0}",
        elapsed.as_secs_f64(),
        args.ticks as f64 / elapsed.as_secs_f64().max(f64::EPSILON)
    );
    ExitCode::SUCCESS
}
