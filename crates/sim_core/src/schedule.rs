use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;

use crate::hash::{StateHash, StateHasher};

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

/// Current simulation tick. Advanced once per fixed step, at the end of
/// [`SimSet::Bookkeeping`], so every other system in a tick sees the same value.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimTick(pub u64);

impl StateHash for SimTick {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0);
    }
}

pub(crate) fn configure_sets(app: &mut App) {
    use SimSet::*;
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

pub(crate) fn advance_tick(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}
