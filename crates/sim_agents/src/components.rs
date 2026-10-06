//! Villager components (SIM-10).

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::{NameId, StateHash, StateHasher};
use sim_economy::GoodId;
use sim_world::SharedPath;

/// A villager (SEED.md "Core data model").
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Villager {
    pub name: NameId,
    pub age_days: u16,
    pub home: Entity,
    pub settlement: Entity,
}

impl StateHash for Villager {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.name.0);
        h.u16(self.age_days);
        h.entity(self.home);
        h.entity(self.settlement);
    }
}

/// Where a villager works, if anywhere, and how many work cycles (or
/// hauls) it has completed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Employment {
    pub workplace: Option<Entity>,
    pub cycles: u32,
}

impl StateHash for Employment {
    fn hash_state(&self, h: &mut StateHasher) {
        match self.workplace {
            Some(e) => {
                h.u8(1);
                h.entity(e);
            }
            None => h.u8(0),
        }
        h.u32(self.cycles);
    }
}

/// Needs on 0..1 scales: hunger rises (1 = starving); rest falls (0 =
/// exhausted); safety falls under threat (M3).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Needs {
    pub hunger: f32,
    pub rest: f32,
    pub safety: f32,
    /// Meals eaten since the game started.
    pub meals: u32,
}

impl Default for Needs {
    fn default() -> Self {
        Self {
            hunger: 0.2,
            rest: 0.9,
            safety: 1.0,
            meals: 0,
        }
    }
}

impl StateHash for Needs {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.hunger);
        h.f32(self.rest);
        h.f32(self.safety);
        h.u32(self.meals);
    }
}

/// What a villager can decide to do (SIM-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ActionKind {
    #[default]
    Idle,
    Eat,
    Fetch,
    Sleep,
    Work,
    Haul,
    GoHome,
}

impl ActionKind {
    pub const ALL: [ActionKind; 7] = [
        ActionKind::Idle,
        ActionKind::Eat,
        ActionKind::Fetch,
        ActionKind::Sleep,
        ActionKind::Work,
        ActionKind::Haul,
        ActionKind::GoHome,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ActionKind::Idle => "idling",
            ActionKind::Eat => "eating",
            ActionKind::Fetch => "fetching food",
            ActionKind::Sleep => "sleeping",
            ActionKind::Work => "working",
            ActionKind::Haul => "hauling goods",
            ActionKind::GoHome => "going home",
        }
    }
}

/// Where an action is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// Walking to the action's target.
    #[default]
    Travel,
    /// Doing the action at the target.
    Perform,
    /// Carrying goods to the drop-off (fetch, haul).
    Deliver,
    /// Finished; the next decision picks something else.
    Done,
}

/// The villager's current action (SEED.md `CurrentAction`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Activity {
    pub kind: ActionKind,
    pub target: Option<Entity>,
    /// Second stop for fetch and haul.
    pub dropoff: Option<Entity>,
    /// Good to haul (None = whatever is there).
    pub good: Option<GoodId>,
    pub stage: Stage,
    /// Sub-step within the stage (e.g. "transfer requested").
    pub step: u8,
    /// When the current timed part (meal, work cycle, rest) ends; 0 = none.
    pub until_tick: u64,
    pub started_tick: u64,
}

impl StateHash for Activity {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.kind as u8);
        for e in [self.target, self.dropoff] {
            match e {
                Some(e) => {
                    h.u8(1);
                    h.entity(e);
                }
                None => h.u8(0),
            }
        }
        h.u16(self.good.map_or(u16::MAX, |g| g.0 as u16));
        h.u8(self.stage as u8);
        h.u8(self.step);
        h.u64(self.until_tick);
        h.u64(self.started_tick);
    }
}

/// Movement towards a tile.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TravelState {
    #[default]
    Idle,
    /// Waiting for a path to `goal`.
    Pending {
        goal: IVec2,
    },
    /// Following `path`; `next` is the index of the next waypoint.
    Moving {
        goal: IVec2,
        path: SharedPath,
        next: u32,
    },
    Arrived,
    /// No path exists.
    Failed,
}

#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct Travel {
    pub state: TravelState,
}

impl Travel {
    pub fn go_to(&mut self, goal: IVec2) {
        self.state = TravelState::Pending { goal };
    }
}

impl StateHash for Travel {
    fn hash_state(&self, h: &mut StateHasher) {
        // A path is a pure function of its endpoints and the map, so the
        // goal and progress identify the state.
        match &self.state {
            TravelState::Idle => h.u8(0),
            TravelState::Pending { goal } => {
                h.u8(1);
                h.ivec2(*goal);
            }
            TravelState::Moving { goal, path, next } => {
                h.u8(2);
                h.ivec2(*goal);
                h.u32(path.len() as u32);
                h.ivec2(path[0]);
                h.u32(*next);
            }
            TravelState::Arrived => h.u8(3),
            TravelState::Failed => h.u8(4),
        }
    }
}

/// The tick at which the villager next reconsiders what to do.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct NextDecision(pub u64);

impl StateHash for NextDecision {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.0);
    }
}

/// One scored option in a decision.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScoredOption {
    pub kind: ActionKind,
    pub score: f32,
    /// The considerations that produced the score, as (name, value).
    pub factors: Vec<(&'static str, f32)>,
}

/// Why the villager is doing what it's doing (UI-03): the best options at
/// its last decision. Derived from state and never read by the sim, so it
/// is exempt from the state hash.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct DecisionTrace {
    pub tick: u64,
    pub options: Vec<ScoredOption>,
}
