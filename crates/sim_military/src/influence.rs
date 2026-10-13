//! Influence: a sphere you can see grow (GP-31, M4a; LORDSHIP_DESIGN
//! section 6, ADR 0020). Run only.
//!
//! - [`Influence`] sits on every populated settlement of a run (inserted
//!   by `progression::setup_run`): the power the town projects and the
//!   player's and the rival's shares of the pull there.
//! - [`step`] is progress slot 13 (hourly, run only); [`contest_term`] is
//!   the Contest loyalty term `tenure::step` sums.
//!
//! M4a step 1 lands the types and the slot; step 3 fills them in (the
//! field, influence deeds, envoys, the contest term).

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_core::{RunState, StateHash, StateHasher};

/// A run settlement's influence (GP-31, M4a): `power` it projects (held
/// towns only) and the shares of the pull there, each in 0..1. Written by
/// [`step`] only. Hashed and saved.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Influence {
    /// The power the town projects, if held: people and loyalty.
    pub power: f32,
    /// The player's share of the pull at the town.
    pub player: f32,
    /// The rival's share of the pull at the town.
    pub rival: f32,
}

impl Influence {
    /// The player's share minus the rival's, in -1..1.
    pub fn net(&self) -> f32 {
        self.player - self.rival
    }
}

impl StateHash for Influence {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.power);
        h.f32(self.player);
        h.f32(self.rival);
    }
}

sim_core::save_fields!(Influence {
    power,
    player,
    rival
});

/// Influence (GP-31): `influence` in progression.ron. See docs/dev/DATA.md
/// and LORDSHIP_DESIGN section 6.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InfluenceData {
    /// `false` makes influence a no-op (scenario tests opt out).
    pub enabled: bool,
    /// A town's pull falls to 0 at this many tiles.
    pub reach: f32,
    /// Power grows with the square root of people over this.
    pub people_unit: f32,
    /// Power's loyalty factor at loyalty 0 (1.0 at loyalty 1).
    pub loyalty_floor: f32,
    /// The rival's seat projects this times its power.
    pub seat_power: f32,
    /// A town's own independence in the shares.
    pub independence: f32,
    /// A town you don't own warms when the net is above this ...
    pub warm_above: f32,
    /// ... by this times the excess, a day ...
    pub goodwill_per_day: f32,
    /// ... until its goodwill toward you reaches this.
    pub goodwill_cap: f32,
    /// An independent town at this net (and liking you) sends envoys.
    pub envoy_net: f32,
    /// Captain-free invitations a day, across the island.
    pub max_envoys_per_day: u32,
    /// The Contest term per share the other side leads by.
    pub contest: f32,
    /// The Contest term in your heartland.
    pub heartland: f32,
    /// The overlay's "warming" line (net).
    pub warming: f32,
    /// The overlay's sphere edge (net).
    pub sphere: f32,
    /// The overlay's front (|net| under this).
    pub front: f32,
    /// The champion gains `rank_points` per this many people held, a day.
    pub rank_people: f32,
    /// See `rank_people`.
    pub rank_points: u32,
    /// Drifters' goodwill line drops by this inside your sphere.
    pub drifter_ease: f32,
}

impl Default for InfluenceData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            enabled: true,
            reach: 200.0,
            people_unit: 100.0,
            loyalty_floor: 0.25,
            seat_power: 2.0,
            independence: 0.5,
            warm_above: 0.1,
            goodwill_per_day: 0.12,
            goodwill_cap: 0.6,
            envoy_net: 0.5,
            max_envoys_per_day: 1,
            contest: 0.25,
            heartland: 0.05,
            warming: 0.2,
            sphere: 0.5,
            front: 0.1,
            rank_people: 500.0,
            rank_points: 1,
            drifter_ease: 0.1,
        }
    }
}

impl InfluenceData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |v: f32| v.is_finite() && v > 0.0;
        let non_negative = |v: f32| v.is_finite() && v >= 0.0;
        let unit = |v: f32| (0.0..=1.0).contains(&v);
        if !positive(self.reach)
            || !positive(self.people_unit)
            || !positive(self.rank_people)
            || !positive(self.independence)
            || !non_negative(self.seat_power)
            || !non_negative(self.goodwill_per_day)
            || !non_negative(self.contest)
            || !non_negative(self.heartland)
            || ![
                self.loyalty_floor,
                self.warm_above,
                self.goodwill_cap,
                self.envoy_net,
                self.warming,
                self.sphere,
                self.front,
                self.drifter_ease,
            ]
            .into_iter()
            .all(unit)
        {
            return Err(
                "progression: influence needs reach, people_unit, rank_people and independence > 0, \
                 weights >= 0 and shares in 0..1"
                    .into(),
            );
        }
        Ok(())
    }
}

/// GP-31: progress slot 13, hourly, run only (span
/// `sim_military::influence::step`): recomputes every town's
/// [`Influence`]; on the day tick, influence deeds and envoys. Empty until
/// M4a step 3.
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::influence::step").entered();
    if world.resource::<RunState>().in_run() {
        refresh(world);
    }
}

/// The hourly field, then on the day tick influence deeds and envoys
/// (M4a step 3).
fn refresh(_world: &mut World) {}

/// GP-31: the Contest loyalty term at owned town `s` (summed by
/// `tenure::step`): up to `-influence.contest` where the other side's
/// share leads, `influence.heartland` deep in the owner's. 0.0 until M4a
/// step 3.
pub fn contest_term(_world: &World, _s: Entity) -> f32 {
    0.0
}
