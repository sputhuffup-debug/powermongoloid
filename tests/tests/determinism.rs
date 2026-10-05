//! NFR-08: same seed + same command stream -> identical state hash.

use sim_headless::{DEFAULT_DEMO_AGENTS, run_and_hash};

const TICKS: u64 = 1_000;

#[test]
fn same_seed_gives_identical_hash() {
    let a = run_and_hash(42, TICKS, DEFAULT_DEMO_AGENTS);
    let b = run_and_hash(42, TICKS, DEFAULT_DEMO_AGENTS);
    assert_eq!(a, b, "state hash diverged between identical runs");
}

#[test]
fn different_seeds_give_different_hashes() {
    let a = run_and_hash(42, TICKS, DEFAULT_DEMO_AGENTS);
    let b = run_and_hash(43, TICKS, DEFAULT_DEMO_AGENTS);
    assert_ne!(a, b, "seed has no effect on state");
}

#[test]
fn hash_changes_as_the_sim_advances() {
    let a = run_and_hash(42, TICKS, DEFAULT_DEMO_AGENTS);
    let b = run_and_hash(42, TICKS + 1, DEFAULT_DEMO_AGENTS);
    assert_ne!(a, b, "state did not change over one tick");
}
