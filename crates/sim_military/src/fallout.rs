//! Conquest fallout, both ways (GP-33, M4a; LORDSHIP_DESIGN section 7,
//! ADR 0020). Run only.
//!
//! - [`Fallout`] (always present; empty outside a run): bands of broken men
//!   waiting to rise from towns the player took by force, queued by
//!   `conquest::transfer_seeded`.
//! - [`step`] is progress slot 16 (hourly, run only): due bands rise from
//!   their town's own villagers (as RAIDERS with a
//!   [`crate::components::Bandits`] marker), and the rival vows reprisals
//!   (`RivalState::reprisal`) that its clock launches.
//!
//! M4a step 1 lands the types and the slot; step 6 fills them in.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_core::{RunState, StateHash, StateHasher};

/// A band of broken men waiting to rise (GP-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingBand {
    /// The town (SpawnId) they come from.
    pub settlement: u64,
    /// Men in the band.
    pub men: u32,
    /// The tick they rise (or, at the cap, the town broods).
    pub due: u64,
}

/// Bands of broken men waiting to rise, and the player's last violent take
/// (GP-33, M4a). Always present; empty outside a run. Hashed and saved.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct Fallout {
    /// Waiting bands, in the order they were queued.
    pub pending: Vec<PendingBand>,
    /// The tick of the player's last conquest or sack (a band is bigger
    /// when the next comes soon after).
    pub last_violent: Option<u64>,
}

impl StateHash for Fallout {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.pending.len() as u32);
        for b in &self.pending {
            h.u64(b.settlement);
            h.u32(b.men);
            h.u64(b.due);
        }
        h.u64(self.last_violent.map_or(u64::MAX, |t| t));
    }
}

sim_core::save_fields!(PendingBand {
    settlement,
    men,
    due
});
sim_core::save_fields!(Fallout {
    pending,
    last_violent
});

/// How much bandits prefer each owner's towns (GP-33): `fallout.weights`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BanditWeights {
    /// The rival's towns.
    pub rival: f32,
    /// The player's towns.
    pub player: f32,
    /// Independent towns.
    pub independent: f32,
}

impl Default for BanditWeights {
    /// The shipped values.
    fn default() -> Self {
        Self {
            rival: 2.0,
            player: 1.0,
            independent: 0.75,
        }
    }
}

/// Bandits and reprisals (GP-33): `fallout` in progression.ron. See
/// docs/dev/DATA.md and LORDSHIP_DESIGN section 7.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FalloutData {
    /// `false` makes fallout a no-op (scenario tests opt out).
    pub enabled: bool,
    /// Towns smaller than this raise no band.
    pub min_people: u32,
    /// Men per person of the town ...
    pub per_people: f32,
    /// ... plus this per militiaman killed in the taking ...
    pub per_kill: f32,
    /// ... times this for a sack ...
    pub sack_mult: f32,
    /// ... and times `haste_mult` when the previous violent take was under
    /// this many days ago.
    pub haste_days: f32,
    /// See `haste_days`.
    pub haste_mult: f32,
    /// A band smaller than this doesn't rise.
    pub min_band: u32,
    /// Men in a band at most.
    pub max_men: u32,
    /// Bandit bands alive at most.
    pub max_bands: u32,
    /// Game hours between the taking and the rising.
    pub delay_hours: f32,
    /// Days of food a band rises with.
    pub food_days: f32,
    /// A town whose band waits at the cap broods: its scar deepens by this.
    pub brood_scar: f32,
    /// Target weights by owner.
    pub weights: BanditWeights,
    /// After this many raids the rival's weight drops to the player's.
    pub turn_after_raids: u32,
    /// Distance falloff scale of a target's score (tiles).
    pub distance_scale: f32,
    /// Garrison falloff scale of a target's score (soldiers).
    pub garrison_scale: f32,
    /// A take where the rival's share is at least this brings a vow.
    pub shadow: f32,
    /// A vow comes due this many game hours after it is made.
    pub vow_delay_hours: f32,
    /// No new vow within this many days of the last.
    pub vow_cooldown_days: f32,
}

impl Default for FalloutData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            enabled: true,
            min_people: 60,
            per_people: 0.04,
            per_kill: 0.5,
            sack_mult: 1.5,
            haste_days: 2.0,
            haste_mult: 1.25,
            min_band: 4,
            max_men: 16,
            max_bands: 2,
            delay_hours: 2.0,
            food_days: 2.0,
            brood_scar: 0.05,
            weights: BanditWeights::default(),
            turn_after_raids: 2,
            distance_scale: 150.0,
            garrison_scale: 5.0,
            shadow: 0.3,
            vow_delay_hours: 18.0,
            vow_cooldown_days: 2.0,
        }
    }
}

impl FalloutData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        let non_negative = |v: f32| v.is_finite() && v >= 0.0;
        let positive = |v: f32| v.is_finite() && v > 0.0;
        let w = &self.weights;
        if ![
            self.per_people,
            self.per_kill,
            self.sack_mult,
            self.haste_days,
            self.haste_mult,
            self.delay_hours,
            self.food_days,
            self.brood_scar,
            self.vow_delay_hours,
            self.vow_cooldown_days,
            w.rival,
            w.player,
            w.independent,
        ]
        .into_iter()
        .all(non_negative)
            || !positive(self.distance_scale)
            || !positive(self.garrison_scale)
            || !(0.0..=1.0).contains(&self.shadow)
            || self.min_band > self.max_men
        {
            return Err(
                "progression: fallout values must be >= 0, scales > 0, shadow in 0..1, min_band <= max_men"
                    .into(),
            );
        }
        Ok(())
    }
}

/// GP-33: progress slot 16, hourly, run only (span
/// `sim_military::fallout::step`): due bandit bands rise, and reprisal
/// vows are made, which slot 11 (`rival::clock`) reads the next hour.
/// Empty until M4a step 6.
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::fallout::step").entered();
    if world.resource::<RunState>().in_run() {
        rise_and_vow(world);
    }
}

/// Due bands rise; vows are made (M4a step 6).
fn rise_and_vow(_world: &mut World) {}
