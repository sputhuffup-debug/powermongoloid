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
//! The queue: `queue_taking` (from `transfer_seeded`, the only writer
//! besides this module) queues a band for a player's conquest or sack of a
//! town of at least `fallout.min_people` ([`band_size`]), and a vow for a
//! player's take (any way) of a town that was the rival's or where the
//! rival's influence share was at least `fallout.shadow`. Nothing spawns
//! there: spawning happens only in [`step`] (BUILD_GUIDE: `spawn_army`'s
//! RNG is keyed by `NextSpawnId`; a band here is made of villagers and
//! spawns nobody, but the rule stands).
//!
//! The bands' minds live in `raiders::think` (the run-only bandit branch,
//! [`bandit_score`]); the reprisal's launch in `rival::clock`.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_agents::Owner;
use sim_core::{
    Calendar, Cause, FactionId, Posture, RunState, SimEvent, SimEventKind, SimTick, SpawnId,
    StateHash, StateHasher,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_world::Settlement;

use crate::components::{Bandits, Captain, Follower, Raider};
use crate::data::Army;
use crate::progression::Progression;
use crate::rival::{RivalState, Vow};

/// A band of broken men waiting to rise (GP-33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingBand {
    /// The town (SpawnId) they come from.
    pub settlement: u64,
    /// Men in the band.
    pub men: u32,
    /// The tick they rise (or, at the cap, the town broods).
    pub due: u64,
    /// The band came due at the cap and its town brooded (once).
    pub brooding: bool,
}

/// A take that calls for a vow of reprisal (GP-33), queued by
/// `queue_taking` (step 7's caught agent pushes one too) and judged by
/// [`step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingVow {
    /// The town (SpawnId) taken.
    pub settlement: u64,
    /// Why: the taking's cause (or `Cause::Caught`).
    pub cause: Cause,
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
    /// Takes that call for a vow, in the order they happened; [`step`]
    /// empties it each hour.
    pub vows: Vec<PendingVow>,
}

impl StateHash for Fallout {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.pending.len() as u32);
        for b in &self.pending {
            h.u64(b.settlement);
            h.u32(b.men);
            h.u64(b.due);
            h.bool(b.brooding);
        }
        h.u64(self.last_violent.map_or(u64::MAX, |t| t));
        h.u32(self.vows.len() as u32);
        for v in &self.vows {
            h.u64(v.settlement);
            v.cause.hash_state(h);
        }
    }
}

sim_core::save_fields!(PendingBand {
    settlement,
    men,
    due,
    brooding
});
sim_core::save_fields!(PendingVow { settlement, cause });
sim_core::save_fields!(Fallout {
    pending,
    last_violent,
    vows
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

/// GP-33: the men a violent take of a town of `people` raises, with
/// `blood` of its militia killed by the taker: `per_people x people +
/// per_kill x blood`, times `sack_mult` for a sack and `haste_mult` for a
/// take hard on the last (`haste`), rounded, at most `max_men`; `None`
/// below `min_band` or for a town under `min_people`. Tarnridge (140
/// people, 11 killed, a conquest) gives 11.
pub fn band_size(
    data: &FalloutData,
    people: u32,
    blood: u32,
    sack: bool,
    haste: bool,
) -> Option<u32> {
    if people < data.min_people {
        return None;
    }
    let mut men = data.per_people * people as f32 + data.per_kill * blood as f32;
    if sack {
        men *= data.sack_mult;
    }
    if haste {
        men *= data.haste_mult;
    }
    let men = (men.round().max(0.0) as u32).min(data.max_men);
    (men >= data.min_band).then_some(men)
}

/// GP-33: called by `conquest::transfer_seeded` (run only, once the town
/// `s` changed hands to `to` because of `cause`, after its tenure recorded
/// the taking): `blood` is what `to` spilt taking it ([`Tenure::blood_of`]
/// before the taking reset it), `previous` its owner before, and
/// `rival_share` the rival's influence share there before the field was
/// refreshed. For the player only: a conquest or sack queues a band
/// ([`band_size`], due `delay_hours` later) and marks the violent take; a
/// take of any kind from the rival, or under its shadow (share at least
/// `fallout.shadow`), queues a vow. Queues only: nothing spawns here.
///
/// [`Tenure::blood_of`]: crate::tenure::Tenure::blood_of
pub(crate) fn queue_taking(
    world: &mut World,
    s: Entity,
    cause: Cause,
    to: FactionId,
    previous: FactionId,
    blood: u32,
    rival_share: f32,
) {
    let data = world.resource::<Progression>().data.fallout;
    if to != FactionId::PLAYER || !data.enabled || !world.resource::<RunState>().in_run() {
        return;
    }
    let Some(id) = world.get::<SpawnId>(s).map(|i| i.0) else {
        return;
    };
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    let people = world
        .get::<SettlementEconomy>(s)
        .map_or(0, |se| se.population);
    let sack = matches!(cause, Cause::Sack { .. });
    let violent = sack || matches!(cause, Cause::Conquest { .. });
    let mut band = None;
    if violent {
        let last = world.resource::<Fallout>().last_violent;
        let haste = last.is_some_and(|t| tick.saturating_sub(t) < cal.days(data.haste_days));
        band = band_size(&data, people, blood, sack, haste).map(|men| PendingBand {
            settlement: id,
            men,
            due: tick + cal.hours(data.delay_hours),
            brooding: false,
        });
    }
    let vow = previous == FactionId::RIVAL || rival_share >= data.shadow;
    let mut fallout = world.resource_mut::<Fallout>();
    if violent {
        fallout.last_violent = Some(tick);
    }
    if let Some(b) = band {
        fallout.pending.push(b);
    }
    if vow {
        fallout.vows.push(PendingVow {
            settlement: id,
            cause,
        });
    }
}

/// GP-33: how much a bandit band wants to raid a town (`fallout` data):
/// `weight(owner) / ((1 + distance / distance_scale) x (1 + garrison /
/// garrison_scale) x (1 + player_share))`, where the weight is
/// `weights.rival` for the rival's towns (`weights.player` once the band
/// has raided `turn_after_raids` times), `weights.player` for the
/// player's and `weights.independent` for the rest; `garrison` counts the
/// owner's soldiers at the town and `player_share` is the player's
/// influence share there (influence protects).
pub fn bandit_score(
    data: &FalloutData,
    owner: FactionId,
    raids: u32,
    distance: f32,
    garrison: u32,
    player_share: f32,
) -> f32 {
    let w = &data.weights;
    let weight = match owner {
        FactionId::RIVAL if raids < data.turn_after_raids => w.rival,
        FactionId::RIVAL | FactionId::PLAYER => w.player,
        _ => w.independent,
    };
    weight
        / ((1.0 + distance / data.distance_scale)
            * (1.0 + garrison as f32 / data.garrison_scale)
            * (1.0 + player_share.max(0.0)))
}

/// GP-33: progress slot 16, hourly, run only (span
/// `sim_military::fallout::step`): due bandit bands rise, and reprisal
/// vows are made, which slot 11 (`rival::clock`) reads the next hour.
///
/// - **Bands.** Each due band (queue order) rises (`raise`) while fewer
///   than `max_bands` bandit bands are alive; at the cap it waits, and
///   its town broods once (its scar deepens by `brood_scar`, at most
///   `tenure.max_sack_scar`). A band whose town is no longer the player's
///   (retaken, risen, freed) never rises.
/// - **Vows.** Each queued take (queue order) makes the rival vow to
///   retake the town, due `vow_delay_hours` later, unless a vow came
///   within `vow_cooldown_days` or the town is no longer the player's;
///   the newest replaces a pending one. A `ReprisalVowed` event says so.
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::fallout::step").entered();
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let data = world.resource::<Progression>().data.fallout;
    if !data.enabled {
        return;
    }
    rise(world, &data);
    vow(world, &data);
}

/// The bandit bands alive.
pub fn bands_alive(world: &mut World) -> u32 {
    let mut q = world.query_filtered::<(), (With<Captain>, With<Bandits>)>();
    q.iter(world).count() as u32
}

fn rise(world: &mut World, data: &FalloutData) {
    if world.resource::<Fallout>().pending.is_empty() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let mut alive = bands_alive(world);
    let pending = std::mem::take(&mut world.resource_mut::<Fallout>().pending);
    let mut keep = Vec::with_capacity(pending.len());
    for mut band in pending {
        if band.due > tick {
            keep.push(band);
            continue;
        }
        let Some(town) = crate::orders::find_settlement(world, band.settlement) else {
            continue;
        };
        if crate::conquest::owner_of(world, town) != FactionId::PLAYER {
            continue;
        }
        if alive >= data.max_bands {
            if !band.brooding {
                band.brooding = true;
                brood(world, town, data.brood_scar);
            }
            keep.push(band);
            continue;
        }
        if raise(world, town, band, data) {
            alive += 1;
        }
    }
    world.resource_mut::<Fallout>().pending = keep;
}

/// A town whose broken men wait at the cap broods: its scar deepens.
fn brood(world: &mut World, town: Entity, by: f32) {
    let cap = world.resource::<Progression>().data.tenure.max_sack_scar;
    if let Some(mut t) = world.get_mut::<crate::tenure::Tenure>(town) {
        let scar = (t.scar + by).min(cap.max(t.scar));
        if scar != t.scar {
            t.scar = scar;
        }
    }
    crate::tenure::rebuild_terms(world, town);
    let name = town_name(world, town);
    crate::orders::log(
        world,
        format!("The broken men of {name} brood: there are bandits enough abroad"),
    );
}

fn town_name(world: &World, s: Entity) -> String {
    world.get::<Settlement>(s).map_or_else(
        || "?".into(),
        |st| {
            world
                .resource::<sim_core::NameTable>()
                .get(st.name)
                .to_string()
        },
    )
}

/// GP-33: the band rises from `town`'s own people: the first `band.men`
/// of its recruit candidates (idle first, `orders::recruit_candidates`),
/// none if fewer than `min_band` are left. The first becomes the band's
/// captain (RAIDERS, aggressive, unarmed, a [`Raider`] and [`Bandits`]),
/// the rest follow (`enlist_into`), so the town's population drops by the
/// band; the baggage holds `food_days` for the band. They gather at the
/// town's edge ([`edge`]: `threat.radius` + 10 tiles out, toward the
/// rival's seat), out of reach of its garrison and militia. Writes `BanditsRose`
/// (the taker's, `Cause::Fallout`). Returns whether it rose.
pub(crate) fn raise(
    world: &mut World,
    town: Entity,
    band: PendingBand,
    data: &FalloutData,
) -> bool {
    let candidates = crate::orders::recruit_candidates(world, town);
    let n = (band.men as usize).min(candidates.len());
    let name = town_name(world, town);
    if n < data.min_band as usize || n == 0 {
        crate::orders::log(
            world,
            format!("Too few of {name} were left to take to the woods"),
        );
        return false;
    }
    let men = &candidates[..n];
    let lead = men[0];
    let tick = world.resource::<SimTick>().0;
    let army = world.resource::<Army>().clone();
    crate::orders::enlist(world, lead, lead, FactionId::RAIDERS, &army);
    let need = world
        .resource::<sim_agents::Villagers>()
        .data
        .needs
        .hunger_per_day
        * n as f32
        * data.food_days;
    let econ = world.resource::<Economy>().clone();
    let mut baggage = Stockpile::with_capacity(army.baggage_capacity(1));
    if let Some(&grain) = econ.foods_by_value().last() {
        let v = econ.food_value(grain).max(0.01);
        baggage.add(grain, (need / v).ceil() as u32);
    }
    let mut captain = Captain::new(FactionId::RAIDERS, Posture::Aggressive);
    captain.last_forage = 0;
    world.entity_mut(lead).remove::<Follower>().insert((
        captain,
        baggage,
        Raider {
            next_think: tick,
            ..Default::default()
        },
        Bandits {
            home: band.settlement,
            raids: 0,
        },
    ));
    if let Some(mut se) = world.get_mut::<SettlementEconomy>(town) {
        se.population = se.population.saturating_sub(1);
    }
    world
        .resource_mut::<crate::components::ArmyStats>()
        .of(FactionId::RAIDERS)
        .recruited += 1;
    for &v in &men[1..] {
        crate::orders::enlist_into(world, v, lead);
    }
    // They gather at the town's edge, beyond the reach of its threat (so
    // neither its garrison nor its militia fall on them as they rise),
    // toward the rival's lands.
    let at = edge(world, town);
    for &v in men {
        if let Some(at) = at {
            world
                .entity_mut(v)
                .insert((sim_core::Position(at), sim_core::Motion::default()));
        }
    }
    let id = world.get::<SpawnId>(lead).map_or(0, |i| i.0);
    let at = world.get::<sim_core::Position>(lead).map_or_else(
        || crate::orders::center(world, town).unwrap_or_default(),
        |p| p.0,
    );
    world.write_message(
        SimEvent::new(SimEventKind::BanditsRose, FactionId::PLAYER)
            .subject(id)
            .other(band.settlement)
            .at(at)
            .amount(n as i32)
            .cause(Cause::Fallout {
                settlement: band.settlement,
            }),
    );
    crate::orders::log(world, format!("{n} men of {name} took to the woods"));
    true
}

/// Where a band of `town` gathers: a land tile `threat.radius` + 10 tiles
/// from its centre, toward the rival's seat (or east without one), turned
/// aside as a withdrawing band turns (`raiders::withdraw_tile`). `None`
/// if there is no such tile (they rise where they stand).
fn edge(world: &World, town: Entity) -> Option<bevy_math::IVec2> {
    let centre = crate::orders::center(world, town)?;
    let state = world.resource::<RivalState>();
    let toward = if state.seat.is_some() && state.seat_center != centre {
        (state.seat_center - centre).as_vec2().normalize_or_zero()
    } else {
        bevy_math::Vec2::X
    };
    let toward = if toward == bevy_math::Vec2::ZERO {
        bevy_math::Vec2::X
    } else {
        toward
    };
    let radius = world.resource::<Army>().data.threat.radius;
    let here = centre + (toward * 2.0).round().as_ivec2();
    crate::raiders::withdraw_tile(
        here,
        &[centre],
        world.resource::<sim_world::WorldMap>(),
        &world.resource::<sim_agents::Villagers>().travel_costs,
        radius,
    )
}

fn vow(world: &mut World, data: &FalloutData) {
    if world.resource::<Fallout>().vows.is_empty() {
        return;
    }
    let vows = std::mem::take(&mut world.resource_mut::<Fallout>().vows);
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    let cooldown = cal.days(data.vow_cooldown_days);
    for v in vows {
        let state = world.resource::<RivalState>();
        if state.seat.is_none() {
            continue;
        }
        if state
            .last_reprisal
            .is_some_and(|t| tick.saturating_sub(t) < cooldown)
        {
            continue;
        }
        let Some(town) = crate::orders::find_settlement(world, v.settlement) else {
            continue;
        };
        if world.get::<Owner>(town).map(|o| o.faction) != Some(FactionId::PLAYER) {
            continue;
        }
        let hours = cal.hours(data.vow_delay_hours);
        let due = tick + hours;
        {
            let mut state = world.resource_mut::<RivalState>();
            state.reprisal = Some(Vow {
                settlement: v.settlement,
                due,
            });
            state.last_reprisal = Some(tick);
        }
        let at = crate::orders::center(world, town).unwrap_or_default();
        world.write_message(
            SimEvent::new(SimEventKind::ReprisalVowed, FactionId::RIVAL)
                .subject(v.settlement)
                .other(due)
                .at(at)
                .amount(data.vow_delay_hours.round() as i32)
                .cause(v.cause),
        );
        let name = town_name(world, town);
        crate::orders::log(world, format!("The rival vowed to retake {name}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GP-33: the design's worked example and the rules around it.
    #[test]
    fn band_sizes_follow_the_design() {
        let d = FalloutData::default();
        // Tarnridge: 140 people, 11 killed, a conquest.
        assert_eq!(band_size(&d, 140, 11, false, false), Some(11));
        assert_eq!(
            band_size(&d, 59, 30, false, false),
            None,
            "too small a town"
        );
        assert_eq!(
            band_size(&d, 60, 0, false, false),
            None,
            "2.4 men: under min_band"
        );
        assert_eq!(
            band_size(&d, 140, 11, true, false),
            Some(16),
            "a sack, capped"
        );
        assert_eq!(
            band_size(&d, 100, 2, false, true),
            Some(6),
            "(4 + 1) x 1.25"
        );
    }

    /// GP-33: bandits prefer the rival until they have raided enough, then
    /// take what is easiest; garrisons, distance and your sphere protect.
    #[test]
    fn bandit_scores_prefer_the_rival_then_the_weak() {
        let d = FalloutData::default();
        let (r, p, i) = (FactionId::RIVAL, FactionId::PLAYER, FactionId::INDEPENDENT);
        let s = |o, raids, dist, g, share| bandit_score(&d, o, raids, dist, g, share);
        assert!(s(r, 0, 100.0, 0, 0.0) > s(p, 0, 100.0, 0, 0.0));
        assert!(s(p, 0, 100.0, 0, 0.0) > s(i, 0, 100.0, 0, 0.0));
        assert_eq!(s(r, 2, 100.0, 0, 0.0), s(p, 2, 100.0, 0, 0.0), "turned");
        assert!(
            s(p, 0, 100.0, 5, 0.0) < s(p, 0, 100.0, 0, 0.0),
            "a garrison"
        );
        assert!(s(p, 0, 100.0, 0, 0.8) < s(p, 0, 100.0, 0, 0.0), "influence");
        assert!(s(r, 0, 300.0, 0, 0.0) < s(r, 0, 50.0, 0, 0.0), "distance");
    }
}
