//! Scenario `smithy_chain` (SIM-64, ADR 0020): in a run, the smithy chain
//! works without any edict. Seed 1's island is left to itself for three
//! days (no player commands, so no edicts) and sampled every half hour.
//!
//! - `the_island_makes_weapons`: at least 40 weapons exist on the island
//!   after three days (about 10 before the fixes), and they don't wait at
//!   forges or in homes, where Equip and the bot don't look: none are in a
//!   home and at most a tenth (the last hours' work) at a forge.
//! - `no_smithy_stalls_full_of_inputs`: no smithy sits out a day's work
//!   hours holding the inputs of a cycle, with a fit worker of its own,
//!   and nobody working a cycle there (the M2 rules let a smithy holding
//!   190 ore and no wood, or no room, sit idle for days); and no smithy
//!   holds more than `haul.input_target` cycles' worth of an input plus
//!   its smiths' own loads (carters' input deliveries are capped). A smithy whose
//!   workers are all starving, exhausted (a night spent fighting),
//!   enlisted or gone isn't stalled: nobody there is fit to work it
//!   (`act::perform`'s own test: hunger under `starving`, rest above 0.1).
//!
//! Both read one shared three-day run.

mod common;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use bevy_ecs::prelude::*;
use common::*;
use sim_agents::{ActionKind, Activity, Enlisted, Needs, Stage, Villagers};
use sim_core::{Calendar, Phase, SimStepper, SimTick, SpawnId};
use sim_economy::{Economy, Stockpile, Workplace, WorkplaceKind};
use sim_headless::stats::{GoodCensus, census};

const SEED: u64 = 1;
/// Three days.
const TICKS: u64 = 7200;
/// Ticks between samples (half an hour).
const SAMPLE: u64 = 50;

/// What the shared run saw.
struct Seen {
    weapons: GoodCensus,
    /// (smithy SpawnId, day): (work-hour samples, of which stalled).
    work_hours: BTreeMap<(u64, u64), (u32, u32)>,
    /// Per (smithy, input): the most it ever held, that input's cap
    /// (`input_target` cycles' worth) and the smithy's workers.
    most_input: BTreeMap<(u64, u8), (u32, u32, u32)>,
}

fn seen() -> &'static Seen {
    static SEEN: OnceLock<Seen> = OnceLock::new();
    SEEN.get_or_init(|| {
        let mut app = run_app(SEED);
        let econ = app.world().resource::<Economy>().clone();
        assert!(econ.data.haul.run_fixes, "the fixes ship on");
        let starving = app.world().resource::<Villagers>().data.needs.starving;
        let mut work_hours: BTreeMap<(u64, u64), (u32, u32)> = BTreeMap::new();
        let mut most_input: BTreeMap<(u64, u8), (u32, u32, u32)> = BTreeMap::new();
        while app.world().resource::<SimTick>().0 < TICKS {
            app.step(SAMPLE);
            let world = app.world_mut();
            let tick = world.resource::<SimTick>().0;
            let clock = world.resource::<Calendar>().at(tick);
            // Smithies with someone working a cycle there right now.
            let mut busy: Vec<Entity> = world
                .query::<&Activity>()
                .iter(world)
                .filter(|a| a.kind == ActionKind::Work && a.stage == Stage::Perform && a.step == 2)
                .filter_map(|a| a.target)
                .collect();
            busy.sort_unstable();
            // Villagers fit to work, and still at home in the village.
            let mut fit: Vec<Entity> = world
                .query_filtered::<(Entity, &Needs), Without<Enlisted>>()
                .iter(world)
                .filter(|(_, n)| n.hunger < starving && n.rest > 0.1)
                .map(|(e, _)| e)
                .collect();
            fit.sort_unstable();
            let mut q = world.query::<(Entity, &SpawnId, &Workplace, &Stockpile)>();
            for (e, id, wp, stock) in q.iter(world) {
                if wp.kind != WorkplaceKind::Smithy {
                    continue;
                }
                let Some(recipe) = wp.recipe.map(|r| econ.recipe(r)) else {
                    continue;
                };
                for (g, q) in &recipe.inputs {
                    let cap = econ.input_target() * *q as u32;
                    let m = most_input.entry((id.0, g.0)).or_insert((0, cap, 0));
                    m.0 = m.0.max(stock.get(*g));
                    m.2 = m.2.max(wp.workers.len() as u32);
                }
                if clock.phase != Phase::Work {
                    continue;
                }
                let stocked = recipe
                    .inputs
                    .iter()
                    .all(|(g, q)| stock.get(*g) >= *q as u32);
                let staffed = wp.workers.iter().any(|w| fit.binary_search(w).is_ok());
                let stalled = stocked && staffed && busy.binary_search(&e).is_err();
                let h = work_hours.entry((id.0, clock.day)).or_default();
                h.0 += 1;
                h.1 += stalled as u32;
            }
        }
        let weapons = census(app.world_mut(), "weapons");
        Seen {
            weapons,
            work_hours,
            most_input,
        }
    })
}

#[test]
fn the_island_makes_weapons() {
    let w = seen().weapons;
    println!("weapons after three days on seed {SEED}: {w:?}");
    assert!(w.total() >= 40, "only {} weapons: {w:?}", w.total());
    // Carters collect a single weapon and smiths carry in four, so only a
    // few made in the last hours can still be at a forge.
    assert!(
        w.homes == 0 && w.workplaces * 10 <= w.total(),
        "weapons waiting where nobody looks: {w:?}"
    );
}

#[test]
fn no_smithy_stalls_full_of_inputs() {
    let s = seen();
    assert!(!s.work_hours.is_empty(), "the island has smithies");
    for ((id, day), (samples, stalled)) in &s.work_hours {
        assert!(
            stalled < samples,
            "smithy #{id} stalled through day {day}'s work hours ({stalled} of {samples} samples)"
        );
    }
    // A carter's delivery is capped where it is picked up; the smiths'
    // own fetches (a load each) can top it up past the cap. Under the M2
    // rules a smithy held up to its 200 units.
    let carry = app_carry();
    for ((id, good), (most, cap, workers)) in &s.most_input {
        assert!(
            *most <= cap + workers * carry,
            "smithy #{id} held {most} of good #{good} (cap {cap}, {workers} smiths)"
        );
    }
}

/// economy.ron `carry`: a worker's load.
fn app_carry() -> u32 {
    Economy::default_data().data.carry as u32
}
