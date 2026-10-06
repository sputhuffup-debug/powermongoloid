//! Villager components (SIM-10).

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::{FactionId, NameId, StateHash, StateHasher};
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
    /// Run from a nearby army (SIM-13).
    Flee,
    /// Shelter at home while an army is near (SIM-13).
    Hide,
    /// Defend the settlement as militia (SIM-13).
    Fight,
    /// A soldier keeping its place in the army (GP-04).
    Follow,
    /// A soldier whose morale broke, running from the enemy (SIM-32).
    Rout,
}

impl ActionKind {
    /// Every action, in a fixed order (used for tables and stats).
    pub const ALL: [ActionKind; 12] = [
        ActionKind::Idle,
        ActionKind::Eat,
        ActionKind::Fetch,
        ActionKind::Sleep,
        ActionKind::Work,
        ActionKind::Haul,
        ActionKind::GoHome,
        ActionKind::Flee,
        ActionKind::Hide,
        ActionKind::Fight,
        ActionKind::Follow,
        ActionKind::Rout,
    ];

    /// Present-tense description for the inspector ("hauling goods").
    pub fn name(self) -> &'static str {
        match self {
            ActionKind::Idle => "idling",
            ActionKind::Eat => "eating",
            ActionKind::Fetch => "fetching food",
            ActionKind::Sleep => "sleeping",
            ActionKind::Work => "working",
            ActionKind::Haul => "hauling goods",
            ActionKind::GoHome => "going home",
            ActionKind::Flee => "fleeing",
            ActionKind::Hide => "hiding",
            ActionKind::Fight => "fighting",
            ActionKind::Follow => "following the captain",
            ActionKind::Rout => "routing",
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

/// Walking toward a tile: path planning and progress (see `movement`).
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct Travel {
    pub state: TravelState,
}

impl Travel {
    /// Asks for a path to `goal`; `plan_paths` fills it in this tick.
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

/// The considerations behind a score, as (name, value); up to four, held
/// inline so scoring allocates nothing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Factors {
    items: [(&'static str, f32); 4],
    len: u8,
}

impl Factors {
    /// Up to four `(name, value)` factors; extra entries are ignored.
    pub fn new(list: &[(&'static str, f32)]) -> Self {
        debug_assert!(list.len() <= 4, "at most four factors");
        let mut f = Self::default();
        for (slot, item) in f.items.iter_mut().zip(list) {
            *slot = *item;
        }
        f.len = list.len().min(4) as u8;
        f
    }
}

impl std::ops::Deref for Factors {
    type Target = [(&'static str, f32)];
    fn deref(&self) -> &Self::Target {
        &self.items[..self.len as usize]
    }
}

/// One scored option in a decision.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScoredOption {
    pub kind: ActionKind,
    pub score: f32,
    /// The considerations that produced the score, as (name, value).
    pub factors: Factors,
}

/// Why the villager is doing what it's doing (UI-03): the best options at
/// its last decision. Derived from state and never read by the sim, so it
/// is exempt from the state hash.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct DecisionTrace {
    pub tick: u64,
    pub options: Vec<ScoredOption>,
}

/// Under military command (a follower or captain): villager AI leaves this
/// person alone; `sim_military` drives it.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Enlisted;

impl StateHash for Enlisted {
    fn hash_state(&self, _h: &mut StateHasher) {}
}

/// How a person takes part in fighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stance {
    /// Never a target, never attacks.
    #[default]
    Civilian,
    /// A villager defending their settlement against one faction (SIM-13).
    Militia { against: FactionId },
    /// A follower or captain.
    Soldier,
}

/// Fighting state (SIM-32). Every person has one; most are civilians.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct CombatStats {
    pub health: f32,
    pub max_health: f32,
    pub attack: f32,
    pub defense: f32,
    /// 0..1; routing below the army's rout threshold.
    pub morale: f32,
    pub stance: Stance,
}

impl CombatStats {
    /// Whether health remains.
    pub fn alive(&self) -> bool {
        self.health > 0.0
    }
}

impl StateHash for CombatStats {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.health);
        h.f32(self.max_health);
        h.f32(self.attack);
        h.f32(self.defense);
        h.f32(self.morale);
        match self.stance {
            Stance::Civilian => h.u8(0),
            Stance::Militia { against } => {
                h.u8(1);
                h.u16(against.0);
            }
            Stance::Soldier => h.u8(2),
        }
    }
}

/// Danger to a settlement from nearby armies, written by `sim_military`
/// each tick and read by villagers (SIM-13).
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Threat {
    /// 0 (safe) .. 1 (terrifying).
    pub level: f32,
    /// Where the danger is.
    pub from: bevy_math::IVec2,
    /// Set when the settlement has been provoked (attacked or plundered):
    /// its militia fights this faction until `provoked_until`.
    pub provoked_by: Option<FactionId>,
    pub provoked_until: u64,
}

impl StateHash for Threat {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.level);
        h.ivec2(self.from);
        h.u16(self.provoked_by.map_or(u16::MAX, |f| f.0));
        h.u64(self.provoked_until);
    }
}

/// A person's next blow (SIM-32). The fighter's own parallel step picks a
/// target and sets `swing`; `sim_military` resolves every swing serially in
/// SpawnId order.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Strike {
    pub target: Option<Entity>,
    /// Where the target was when it was picked (tile coordinates).
    pub at: bevy_math::Vec2,
    /// Swinging this tick.
    pub swing: bool,
    /// Attack multiplier from a leader nearby.
    pub bonus: f32,
    /// No new swing before this tick.
    pub ready_at: u64,
}

impl StateHash for Strike {
    fn hash_state(&self, h: &mut StateHasher) {
        match self.target {
            Some(e) => {
                h.u8(1);
                h.entity(e);
            }
            None => h.u8(0),
        }
        h.vec2(self.at);
        h.bool(self.swing);
        h.f32(self.bonus);
        h.u64(self.ready_at);
    }
}
