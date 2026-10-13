//! Carrying out actions: eating, fetching, sleeping, working, hauling.
//! Runs in parallel per villager; anything touching a building's stock is
//! posted as a [`Transfers`] order and applied by the economy's resolver
//! later in the same tick.
//!
//! Work reads the settlement owner's `FactionModifiers` (GP-23) for recipes
//! tagged in economy.ron: `modifier: Some(FoodYield)` scales the outputs
//! ([`yield_units`], stochastic rounding), `Some(SmithSpeed)` shortens the
//! cycle ([`smith_ticks`]). Without modifiers both are exact no-ops.
//!
//! In a run with economy.ron `haul.run_fixes` (SIM-64, ADR 0020), an input
//! delivery picks up at most `haul.input_target` cycles' worth minus what
//! the workplace holds ([`input_cap`]), and a worker whose hands are too
//! full for a cycle stops so the AI can send it to empty them.

use bevy_ecs::prelude::*;
use rand::RngExt;
use sim_core::rng::rng_key;
use sim_core::{Calendar, FactionId, FactionModifiers, Phase, SimSeed, SimTick, SpawnId, Stat};
use sim_economy::{
    Building, Economy, GoodId, Inventory, Stockpile, TransferOp, Transfers, Workplace,
};

use crate::components::{
    ActionKind, Activity, CombatStats, Employment, Needs, Stage, Stance, Threat, Travel,
    TravelState, Villager,
};
use crate::data::Villagers;
use crate::standing::Owner;

/// GP-23: the units a work cycle yields when a recipe's `q` units are
/// scaled by `mult` (a `FoodYield` multiplier): `q x mult`, its fraction
/// rounded up with that chance. `draw` gives a uniform 0..1 number and is
/// called only when there is a fraction, so `mult == 1.0` yields exactly
/// `q` and draws nothing.
pub fn yield_units(q: u16, mult: f32, draw: impl FnOnce() -> f32) -> u16 {
    let x = (q as f32 * mult).max(0.0);
    let whole = x.floor();
    let fraction = x - whole;
    let extra = fraction > 0.0 && draw() < fraction;
    (whole as u32 + extra as u32).min(u16::MAX as u32) as u16
}

/// GP-23: a recipe's `duration` ticks at a `SmithSpeed` multiplier of
/// `mult`: `duration / mult`, rounded, at least 1. Exactly `duration` at
/// 1.0; a non-positive multiplier changes nothing.
pub fn smith_ticks(duration: u32, mult: f32) -> u64 {
    if mult <= 0.0 || !mult.is_finite() {
        return duration as u64;
    }
    (duration as f32 / mult).round().max(1.0) as u64
}

/// GP-34: a work cycle of `ticks` under the Ration edict's `work`
/// multiplier: `ticks x work`, rounded, at least 1; a non-positive or
/// non-finite multiplier changes nothing.
pub fn rationed_ticks(ticks: u64, work: f32) -> u64 {
    if work <= 0.0 || !work.is_finite() {
        return ticks;
    }
    (ticks as f32 * work).round().max(1.0) as u64
}

/// Puts `out` units of a recipe output into the worker's hands, of which
/// the recipe's own `base` go in as always (up to the carry limit), and any
/// bonus beyond `base` over the limit, so a yield bonus isn't lost to a
/// full pair of hands (guide G23).
fn add_output(inv: &mut Inventory, g: GoodId, base: u16, out: u16) {
    if out <= base {
        inv.add(g, out);
    } else {
        inv.add(g, base);
        inv.add_over_capacity(g, out - base);
    }
}
/// SIM-64: the most an input delivery of `good` to a workplace running
/// `inputs` (its recipe's) may bring: `input_target` cycles' worth minus
/// `held`, what the workplace already has. `None` if `good` isn't an input.
pub fn input_cap(
    inputs: &[(GoodId, u16)],
    good: GoodId,
    held: u32,
    input_target: u32,
) -> Option<u32> {
    inputs
        .iter()
        .find(|(g, _)| *g == good)
        .map(|(_, q)| (input_target * *q as u32).saturating_sub(held))
}

/// SIM-64: whether a worker's hands are too full for a work cycle of a
/// recipe taking `inputs` and yielding `outputs` (`out_units` in all), and
/// if so the good to carry off first: the largest pile that isn't an
/// input, or holds more of one than a cycle takes. Too full is too few
/// units of room, or too few of the four slots for the cycle's goods (an
/// output with no slot would be lost). `None` if the hands have room, or
/// nothing in them is in the way.
pub fn blocking_good(
    inv: &Inventory,
    inputs: &[(GoodId, u16)],
    outputs: &[(GoodId, u16)],
    out_units: u32,
) -> Option<GoodId> {
    let missing: u32 = inputs
        .iter()
        .map(|(g, q)| q.saturating_sub(inv.get(*g)) as u32)
        .sum();
    let mut kinds: Vec<GoodId> = inv
        .contents()
        .map(|(g, _)| g)
        .chain(inputs.iter().map(|(g, _)| *g))
        .chain(outputs.iter().map(|(g, _)| *g))
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    if inv.room() as u32 >= missing + out_units
        && kinds.len() <= sim_economy::goods::INVENTORY_SLOTS
    {
        return None;
    }
    let need = |g: GoodId| inputs.iter().find(|(i, _)| *i == g).map(|(_, q)| *q);
    inv.contents()
        .filter(|(g, n)| need(*g).is_none_or(|q| *n > q))
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        .map(|(g, _)| g)
}

/// Eats carried food, best first, until sated or out of food. Returns
/// whether anything was eaten.
pub fn eat_from(inv: &mut Inventory, needs: &mut Needs, econ: &Economy, sated: f32) -> bool {
    let mut ate = false;
    for good in econ.foods_by_value() {
        while needs.hunger > sated && inv.get(good) > 0 {
            inv.take(good, 1);
            needs.hunger = (needs.hunger - econ.food_value(good)).max(0.0);
            ate = true;
        }
    }
    if ate {
        needs.meals += 1;
    }
    ate
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Bevy system parameters.
pub(crate) fn perform(
    tick: Res<SimTick>,
    calendar: Res<Calendar>,
    econ: Res<Economy>,
    villagers: Res<Villagers>,
    seed: Res<SimSeed>,
    buildings: Query<&Building>,
    workplaces: Query<&Workplace>,
    threats: Query<&Threat>,
    owners: Query<&Owner>,
    mods: Res<FactionModifiers>,
    (run, stocks): (Option<Res<sim_core::RunState>>, Query<&Stockpile>),
    mut agents: Query<
        (
            &SpawnId,
            &Villager,
            &mut CombatStats,
            &mut Activity,
            &mut Travel,
            &mut Needs,
            &mut Inventory,
            &mut Transfers,
            &mut Employment,
            Option<&crate::edicts::Rationed>,
        ),
        Without<crate::components::Enlisted>,
    >,
) {
    let _span = tracing::info_span!("sim_agents::perform").entered();
    let tick = tick.0;
    let phase = calendar.at(tick).phase;
    let needs_data = &villagers.data.needs;
    let idle = &villagers.data.idle;
    // SIM-64: the smithy-chain fixes, in a run.
    let fixes = crate::ai::run_fixes_on(run.as_deref(), &econ);
    // The cap on an input delivery from `store` to `dropoff`, with the fixes.
    let delivery_cap = |dropoff: Option<Entity>, good: GoodId| -> Option<u16> {
        let d = dropoff.filter(|_| fixes)?;
        let recipe = econ.recipe(workplaces.get(d).ok()?.recipe?);
        let held = stocks.get(d).map_or(0, |s| s.get(good));
        input_cap(&recipe.inputs, good, held, econ.input_target())
            .map(|c| c.min(u16::MAX as u32) as u16)
    };
    agents.par_iter_mut().for_each(
        |(
            id,
            villager,
            mut combat,
            mut act,
            mut travel,
            mut needs,
            mut inv,
            mut transfers,
            mut job,
            rationed,
        )| {
            // Only a villager who is fighting is militia (SIM-13).
            let fighting = act.kind == ActionKind::Fight && act.stage != Stage::Done;
            if !fighting && combat.stance != Stance::Civilian {
                combat.stance = Stance::Civilian;
            }
            match act.stage {
                Stage::Done => {}
                Stage::Travel => match travel.state {
                    TravelState::Arrived | TravelState::Idle => {
                        travel.state = TravelState::Idle;
                        act.stage = Stage::Perform;
                        act.step = 0;
                        act.until_tick = 0;
                    }
                    TravelState::Failed => {
                        travel.state = TravelState::Idle;
                        act.stage = Stage::Done;
                    }
                    _ => {}
                },
                Stage::Deliver => match travel.state {
                    TravelState::Arrived | TravelState::Idle => {
                        travel.state = TravelState::Idle;
                        if let Some(dropoff) = act.dropoff {
                            transfers.post(dropoff, vec![TransferOp::PutAll]);
                        }
                        if act.kind == ActionKind::Haul {
                            job.cycles += 1;
                        }
                        act.stage = Stage::Done;
                    }
                    TravelState::Failed => {
                        travel.state = TravelState::Idle;
                        act.stage = Stage::Done;
                    }
                    _ => {}
                },
                Stage::Perform => match act.kind {
                    ActionKind::Eat => match act.step {
                        0 | 1 => {
                            if eat_from(&mut inv, &mut needs, &econ, needs_data.sated) {
                                act.until_tick = tick + needs_data.meal_ticks as u64;
                                act.step = 2;
                            } else if act.step == 0
                                && let Some(target) = act.target
                            {
                                let units = (((needs.hunger - needs_data.sated) / 0.15).ceil()
                                    as u16)
                                    .max(1);
                                transfers.post(target, vec![TransferOp::TakeMeal { max: units }]);
                                act.step = 1;
                            } else {
                                act.stage = Stage::Done;
                            }
                        }
                        _ => {
                            if tick >= act.until_tick {
                                act.stage = Stage::Done;
                            }
                        }
                    },
                    ActionKind::Fetch | ActionKind::Haul => match act.step {
                        0 => {
                            if let Some(target) = act.target {
                                let max = inv.room();
                                let cap = match (act.kind, act.good) {
                                    (ActionKind::Haul, Some(good)) => {
                                        delivery_cap(act.dropoff, good)
                                    }
                                    _ => None,
                                };
                                let op = match (act.kind, act.good) {
                                    (ActionKind::Fetch, _) => TransferOp::TakeFood { max },
                                    (_, Some(good)) => TransferOp::Take {
                                        good,
                                        max: cap.map_or(max, |cap| cap.min(max)),
                                    },
                                    (_, None) => TransferOp::TakeAny { max },
                                };
                                // SIM-64: at the storehouse, hands are
                                // emptied first: goods (not food) in the
                                // hands of someone fetching food, so they
                                // don't go home where nobody sees them, and
                                // leftovers of an interrupted haul in the
                                // hands of someone picking up an input
                                // delivery, so only the capped input goes.
                                let ops = if fixes
                                    && act.kind == ActionKind::Fetch
                                    && inv.contents().any(|(g, _)| !econ.is_food(g))
                                {
                                    vec![
                                        TransferOp::PutAll,
                                        TransferOp::TakeFood { max: inv.capacity },
                                    ]
                                } else if let (Some(cap), Some(good)) = (cap, act.good)
                                    && !inv.is_empty()
                                {
                                    vec![
                                        TransferOp::PutAll,
                                        TransferOp::Take {
                                            good,
                                            max: cap.min(inv.capacity),
                                        },
                                    ]
                                } else {
                                    vec![op]
                                };
                                transfers.post(target, ops);
                                act.step = 1;
                            } else {
                                act.stage = Stage::Done;
                            }
                        }
                        _ => {
                            let dropoff_tile = act
                                .dropoff
                                .and_then(|d| buildings.get(d).ok())
                                .map(|b| b.tile);
                            match dropoff_tile {
                                Some(tile) if !inv.is_empty() => {
                                    travel.go_to(tile);
                                    act.stage = Stage::Deliver;
                                }
                                _ => act.stage = Stage::Done,
                            }
                        }
                    },
                    ActionKind::Sleep => {
                        // Sleep through the night; by day, wake once rested.
                        let daytime = matches!(phase, Phase::Morning | Phase::Work);
                        if daytime && needs.rest > 0.6 {
                            act.stage = Stage::Done;
                        }
                    }
                    ActionKind::Work => {
                        let recipe = act
                            .target
                            .and_then(|t| workplaces.get(t).ok())
                            .and_then(|w| w.recipe)
                            .map(|r| econ.recipe(r));
                        let (Some(recipe), Some(target)) = (recipe, act.target) else {
                            act.stage = Stage::Done;
                            return;
                        };
                        let can_work = phase == Phase::Work
                            && needs.hunger < needs_data.starving
                            && needs.rest > 0.1;
                        // GP-23: the settlement owner's modifier, for
                        // recipes tagged with one (1.0 without modifiers).
                        let mult = |stat: Stat| {
                            let owner = owners
                                .get(villager.settlement)
                                .map_or(FactionId::INDEPENDENT, |o| o.faction);
                            mods.mult(owner, stat)
                        };
                        match act.step {
                            // Ready to start a cycle.
                            0 | 1 => {
                                // SIM-64: hands too full for a cycle stop
                                // it; the AI sends the worker to empty them.
                                // As `work_state` counts them: room for
                                // a FoodYield bonus's best case.
                                let food_mult = match recipe.modifier {
                                    Some(Stat::FoodYield) if fixes => mult(Stat::FoodYield),
                                    _ => 1.0,
                                };
                                let out: u32 = recipe
                                    .outputs
                                    .iter()
                                    .map(|(_, q)| (*q as f32 * food_mult).max(0.0).ceil() as u32)
                                    .sum();
                                if !can_work
                                    || fixes
                                        && !recipe.inputs.is_empty()
                                        && blocking_good(&inv, &recipe.inputs, &recipe.outputs, out)
                                            .is_some()
                                {
                                    act.stage = Stage::Done;
                                } else if inv.has_all(&recipe.inputs) {
                                    for (g, q) in &recipe.inputs {
                                        inv.take(*g, *q);
                                    }
                                    let ticks = match recipe.modifier {
                                        Some(Stat::SmithSpeed) => {
                                            smith_ticks(recipe.duration, mult(Stat::SmithSpeed))
                                        }
                                        _ => recipe.duration as u64,
                                    };
                                    // GP-34: rationed hands work slower.
                                    let ticks = match rationed {
                                        Some(r) => rationed_ticks(ticks, r.work),
                                        None => ticks,
                                    };
                                    act.until_tick = tick + ticks;
                                    act.step = 2;
                                } else if act.step == 0 {
                                    let ops = recipe
                                        .inputs
                                        .iter()
                                        .map(|(g, q)| TransferOp::Take {
                                            good: *g,
                                            max: q.saturating_sub(inv.get(*g)),
                                        })
                                        .collect();
                                    transfers.post(target, ops);
                                    act.step = 1;
                                } else {
                                    // Inputs never arrived.
                                    act.stage = Stage::Done;
                                }
                            }
                            // Working.
                            2 => {
                                if tick >= act.until_tick {
                                    let food = (recipe.modifier == Some(Stat::FoodYield))
                                        .then(|| mult(Stat::FoodYield));
                                    let mut rng = None;
                                    for (g, q) in &recipe.outputs {
                                        let out = match food {
                                            Some(m) => yield_units(*q, m, || {
                                                rng.get_or_insert_with(|| {
                                                    seed.stream(
                                                        rng_key("sim_agents::yield") ^ id.0,
                                                        tick,
                                                    )
                                                })
                                                .random::<f32>()
                                            }),
                                            None => *q,
                                        };
                                        add_output(&mut inv, *g, *q, out);
                                    }
                                    job.cycles += 1;
                                    transfers.post(target, vec![TransferOp::PutAll]);
                                    act.until_tick = 0;
                                    act.step = 3;
                                }
                            }
                            // Outputs handed over; if they didn't fit, stop.
                            _ => {
                                let leftover = recipe.outputs.iter().any(|(g, _)| inv.get(*g) > 0);
                                act.step = 0;
                                if leftover {
                                    act.stage = Stage::Done;
                                }
                            }
                        }
                    }
                    ActionKind::Hide => {
                        if needs.safety > 0.8 {
                            act.stage = Stage::Done;
                        }
                    }
                    ActionKind::Flee => {
                        if act.step == 0 {
                            act.until_tick = tick + 200;
                            act.step = 1;
                        } else if tick >= act.until_tick && needs.safety > 0.6 {
                            act.stage = Stage::Done;
                        }
                    }
                    ActionKind::Fight => {
                        // sim_military moves and strikes for militia.
                        let threat = threats.get(villager.settlement).ok();
                        match threat.and_then(|t| t.provoked_by) {
                            Some(against)
                                if combat.morale >= villagers.data.ai.fight.break_morale
                                    && combat.alive() =>
                            {
                                combat.stance = Stance::Militia { against };
                            }
                            _ => {
                                combat.stance = Stance::Civilian;
                                act.stage = Stage::Done;
                            }
                        }
                    }
                    ActionKind::Follow | ActionKind::Rout => act.stage = Stage::Done,
                    ActionKind::GoHome | ActionKind::Idle => {
                        if act.step == 0 {
                            let ticks = if act.kind == ActionKind::Idle {
                                let mut rng =
                                    seed.stream(rng_key("sim_agents::idle_ticks") ^ id.0, tick);
                                rng.random_range(
                                    idle.min_ticks..=idle.max_ticks.max(idle.min_ticks),
                                )
                            } else {
                                120
                            };
                            act.until_tick = tick + ticks as u64;
                            act.step = 1;
                        } else if tick >= act.until_tick {
                            act.stage = Stage::Done;
                        }
                    }
                },
            }
        },
    );
}

/// Needs change over time (SIM-10): hunger rises (more slowly for a
/// villager under the Ration edict, GP-34); rest falls while awake
/// and recovers while asleep; safety follows the settlement's threat level
/// (SIM-13): it drops at once and recovers slowly.
pub(crate) fn update_needs(
    villagers: Res<Villagers>,
    threats: Query<&Threat>,
    mut q: Query<
        (
            &Villager,
            &Activity,
            &mut Needs,
            Option<&crate::edicts::Rationed>,
        ),
        Without<crate::components::Enlisted>,
    >,
) {
    let _span = tracing::info_span!("sim_agents::update_needs").entered();
    let v = &*villagers;
    q.par_iter_mut()
        .for_each(|(villager, act, mut needs, rationed)| {
            // GP-34: the Ration edict slows hunger (no change without it).
            let gain = match rationed {
                Some(r) => v.hunger_per_tick * r.hunger,
                None => v.hunger_per_tick,
            };
            needs.hunger = (needs.hunger + gain).min(1.0);
            let asleep = act.kind == ActionKind::Sleep && act.stage == Stage::Perform;
            needs.rest = if asleep {
                (needs.rest + v.rest_gain_per_tick).min(1.0)
            } else {
                (needs.rest - v.rest_loss_per_tick).max(0.0)
            };
            let safe = 1.0 - threats.get(villager.settlement).map_or(0.0, |t| t.level);
            needs.safety = if safe < needs.safety {
                (needs.safety - 0.05).max(safe)
            } else {
                (needs.safety + 0.001).min(safe)
            };
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_economy::GoodId;

    /// GP-23: with no modifier a yield and a smithing cycle are exactly the
    /// recipe's, and nothing is drawn; a bonus rounds its fraction by
    /// chance, a malus never yields more than the recipe.
    #[test]
    fn modifiers_scale_yields_and_cycles_and_are_no_ops_at_one() {
        for q in [1u16, 2, 8, 1000] {
            assert_eq!(yield_units(q, 1.0, || panic!("no draw at 1.0")), q);
        }
        assert_eq!(yield_units(2, 1.5, || panic!("no fraction")), 3);
        assert_eq!(
            yield_units(8, 1.1, || 0.0),
            9,
            "8.8 rounds up on a low draw"
        );
        assert_eq!(yield_units(8, 1.1, || 0.9), 8, "and down on a high one");
        assert_eq!(yield_units(8, 0.85, || 0.0), 7);
        assert_eq!(yield_units(8, 0.85, || 0.99), 6);
        assert_eq!(yield_units(2, -1.0, || panic!("no fraction")), 0);
        for d in [1u32, 150, 400, 450] {
            assert_eq!(smith_ticks(d, 1.0), d as u64);
        }
        assert_eq!(smith_ticks(400, 1.1), 364);
        assert_eq!(smith_ticks(1, 4.0), 1, "at least a tick");
        assert_eq!(smith_ticks(400, 0.0), 400);
        // GP-34: rationing lengthens a cycle by its multiplier.
        assert_eq!(rationed_ticks(450, 1.1), 495);
        assert_eq!(rationed_ticks(450, 1.0), 450);
        assert_eq!(rationed_ticks(450, 0.0), 450);

        // A bonus unit goes over the carry limit; the base respects it.
        let mut inv = Inventory::with_capacity(8);
        inv.add(GoodId(0), 1);
        add_output(&mut inv, GoodId(0), 8, 9);
        assert_eq!(inv.get(GoodId(0)), 9, "1 carried + 7 base (full) + 1 bonus");
        let mut inv = Inventory::with_capacity(8);
        add_output(&mut inv, GoodId(0), 8, 7);
        assert_eq!(inv.get(GoodId(0)), 7);
    }

    /// SIM-64: an input delivery brings `input_target` cycles' worth minus
    /// the stock, nothing once that is there, and isn't capped for a good
    /// the recipe doesn't take.
    #[test]
    fn input_deliveries_are_capped() {
        let (ore, wood, grain) = (GoodId(5), GoodId(4), GoodId(0));
        let inputs = [(ore, 2), (wood, 1)];
        assert_eq!(input_cap(&inputs, ore, 0, 10), Some(20));
        assert_eq!(input_cap(&inputs, ore, 14, 10), Some(6));
        assert_eq!(input_cap(&inputs, ore, 190, 10), Some(0));
        assert_eq!(input_cap(&inputs, wood, 3, 10), Some(7));
        assert_eq!(input_cap(&inputs, grain, 0, 10), None);
    }

    /// SIM-64: hands too full for a cycle (units or slots) name the pile
    /// to carry off: never an input a cycle needs, the largest first.
    #[test]
    fn full_hands_name_what_is_in_the_way() {
        let (grain, bread, wood, ore, weapons) =
            (GoodId(0), GoodId(3), GoodId(4), GoodId(5), GoodId(7));
        let inputs = [(ore, 2), (wood, 1)];
        let outputs = [(weapons, 1)];
        let mut inv = Inventory::with_capacity(8);
        assert_eq!(blocking_good(&inv, &inputs, &outputs, 1), None);
        inv.add(weapons, 5);
        assert_eq!(blocking_good(&inv, &inputs, &outputs, 1), Some(weapons));
        // Room for the cycle: 3 units of inputs and 1 of output.
        inv.take(weapons, 1);
        assert_eq!(blocking_good(&inv, &inputs, &outputs, 1), None);
        // Inputs already in hand need no room; too much of one is in the way.
        let mut inv = Inventory::with_capacity(8);
        inv.add(ore, 7);
        assert_eq!(blocking_good(&inv, &inputs, &outputs, 1), Some(ore));
        inv.take(ore, 5);
        assert_eq!(blocking_good(&inv, &inputs, &outputs, 1), None);
        // Two kinds of leftovers leave no slot for the output.
        let mut inv = Inventory::with_capacity(8);
        inv.add(grain, 1);
        inv.add(bread, 2);
        assert_eq!(blocking_good(&inv, &inputs, &outputs, 1), Some(bread));
    }

    #[test]
    fn eating_prefers_better_food_and_stops_when_sated() {
        let econ = Economy::default_data();
        let mut inv = Inventory::with_capacity(20);
        inv.add(GoodId(0), 5); // grain 0.15
        inv.add(GoodId(2), 1); // mutton 0.35
        let mut needs = Needs {
            hunger: 0.6,
            ..Default::default()
        };
        assert!(eat_from(&mut inv, &mut needs, &econ, 0.05));
        assert_eq!(inv.get(GoodId(2)), 0, "mutton first");
        assert!(needs.hunger <= 0.05);
        assert!(inv.get(GoodId(0)) >= 2, "stops once sated");
    }
}
