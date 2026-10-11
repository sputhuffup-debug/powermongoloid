use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;

use crate::calendar::Calendar;
use crate::hash::{StateHash, StateHasher};
use crate::run::run_in_progress;

/// Simulation ticks per game-second at 1x speed (SIM-03).
pub const SIM_TICK_HZ: f64 = 10.0;

/// The nine ordered phases of a simulation tick (SEED.md "Tick schedule").
/// Every simulation system must live in exactly one of these sets.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimSet {
    Input,
    Knowledge,
    Decide,
    Move,
    Act,
    Needs,
    Aggregate,
    Lod,
    Bookkeeping,
}

impl SimSet {
    /// Every set, in tick order.
    pub const ALL: [SimSet; 9] = [
        SimSet::Input,
        SimSet::Knowledge,
        SimSet::Decide,
        SimSet::Move,
        SimSet::Act,
        SimSet::Needs,
        SimSet::Aggregate,
        SimSet::Lod,
        SimSet::Bookkeeping,
    ];

    /// Display name (debug panel, `--timings`).
    pub fn name(self) -> &'static str {
        match self {
            SimSet::Input => "Input",
            SimSet::Knowledge => "Knowledge",
            SimSet::Decide => "Decide",
            SimSet::Move => "Move",
            SimSet::Act => "Act",
            SimSet::Needs => "Needs",
            SimSet::Aggregate => "Aggregate",
            SimSet::Lod => "Lod",
            SimSet::Bookkeeping => "Bookkeeping",
        }
    }
}

/// Ordering inside [`SimSet::Bookkeeping`], chained: what happened this tick
/// is recorded, then the tick counter advances. Systems that read
/// [`SimTick`] in Bookkeeping go in `Record`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookkeepingSet {
    /// `record_events`, then `settle_outcome` (sim_core).
    Record,
    /// `advance_tick`: always the last sim system of a tick.
    Advance,
}

/// Current simulation tick. Advanced once per fixed step, at the end of
/// [`SimSet::Bookkeeping`], so every other system in a tick sees the same value.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimTick(pub u64);

impl StateHash for SimTick {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0);
    }
}

crate::save_newtype!(SimTick);

/// Run condition: the first tick of a game hour (an hour is a 24th of a day,
/// [`Calendar::ticks_per_hour`]). Tick 0 counts. For hourly serial steps.
pub fn on_hour(tick: Res<SimTick>, cal: Res<Calendar>) -> bool {
    cal.on_hour(tick.0)
}

/// Run condition: the first tick of a day. Tick 0 counts. For daily steps.
pub fn on_day(tick: Res<SimTick>, cal: Res<Calendar>) -> bool {
    cal.on_day(tick.0)
}

/// Run condition: the first tick (tick 0), before any tick has run. For
/// what a tick tells once about what setup built (a run's start).
pub fn first_tick(tick: Res<SimTick>) -> bool {
    tick.0 == 0
}

/// Chains the nine [`SimSet`]s and [`BookkeepingSet`], and gates every
/// `SimSet` on [`run_in_progress`]: once a run has ended, ticks do nothing,
/// so [`SimTick`] and the state hash freeze (GP-21).
pub(crate) fn configure_sets(app: &mut App) {
    use SimSet::*;
    for set in SimSet::ALL {
        app.configure_sets(FixedUpdate, set.run_if(run_in_progress));
    }
    app.configure_sets(
        FixedUpdate,
        (BookkeepingSet::Record, BookkeepingSet::Advance)
            .chain()
            .in_set(Bookkeeping),
    );
    app.configure_sets(
        FixedUpdate,
        (
            Input,
            Knowledge,
            Decide,
            Move,
            Act,
            Needs,
            Aggregate,
            Lod,
            Bookkeeping,
        )
            .chain(),
    );
}

/// Advances [`SimTick`]; the last sim system of every tick
/// ([`BookkeepingSet::Advance`]).
pub(crate) fn advance_tick(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}
