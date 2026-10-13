//! Tenure: what a town remembers about how it was taken (GP-32, SIM-60,
//! M4a; LORDSHIP_DESIGN section 4, ADR 0020). Run only.
//!
//! - [`Tenure`] sits on every populated settlement of a run (inserted by
//!   `progression::setup_run`): how it came to its owner ([`TakenBy`]), by
//!   whom and since when, its scar (resentment), the blood spilt taking
//!   it, an agent's stir, promises kept and broken, a submitted town's
//!   watch and a ring of past takings.
//! - It is written where ownership changes: [`crate::conquest::transfer_seeded`]
//!   takes the [`TakenBy`] from the [`Cause`] (`record_taking`);
//!   [`crate::rival::setup`] writes `Granted` and
//!   [`crate::rival::free_lands`] `Freed`. Militia deaths add to `blood`
//!   (`combat::casualties`, [`Tenure::spill`]), counted for the faction
//!   they fought; a taking reads only its taker's ([`Tenure::blood_of`])
//!   and rebuilds the town's terms at once.
//! - [`step`] (progress slot 14, hourly, run only) rebuilds each town's
//!   `sim_agents::LoyaltyTerms` from its tenure and the terms other modules
//!   expose (`influence::contest_term`, live from M4a step 3,
//!   `govern::dues_term`, `govern::table_term`, `govern::steward_term`),
//!   and on the day tick fades scars by `tenure.scar_fade_per_day`. Slot 13
//!   (`influence::step`) runs just before it, so the Contest term reads the
//!   hour's field.
//!
//! The data blocks of the ways to take a town live here too: `tenure`,
//! `demand`, `court` and `subvert` (progression.ron).

use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use serde::{Deserialize, Serialize};
use sim_agents::{
    Deed, Edicts, FoodSecurity, GoodwillLedger, LoyaltyTerm, LoyaltyTerms, Owner, Threat,
};
use sim_core::{
    Calendar, Cause, Dial, Dues, FactionId, RunState, SimEvent, SimEventKind, SimTick, SpawnId,
    StateHash, StateHasher,
};
use sim_economy::SettlementEconomy;
use sim_world::Settlement;

use crate::components::{Bandits, Captain, Equipment};
use crate::conquest::{garrison_and_rebels, owner_of, transfer, would_be_militia};
use crate::data::Army;
use crate::index::FighterGrid;
use crate::influence::{Influence, Source, field_at};
use crate::orders::{daily_need, faction_name, goodwill_deed, log, settlement_name};
use crate::progression::Progression;

/// How a settlement came to its current owner (GP-32, SIM-60). Append only:
/// saves store the position.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum TakenBy {
    /// Worldgen's own: independent since the start.
    #[default]
    Native,
    /// Taken by force.
    Conquered,
    /// Taken by force at Aggressive posture, its storehouse plundered.
    Sacked,
    /// Knelt to a demand.
    Submitted,
    /// Invited its lord in.
    Invited,
    /// Turned from within by an agent.
    Subverted,
    /// Handed to the rival at the start of the run.
    Granted,
    /// Gone independent by revolt or defiance (or freed when its lord fell).
    Freed,
}

impl TakenBy {
    /// Every value, in position order.
    pub const ALL: [TakenBy; 8] = [
        TakenBy::Native,
        TakenBy::Conquered,
        TakenBy::Sacked,
        TakenBy::Submitted,
        TakenBy::Invited,
        TakenBy::Subverted,
        TakenBy::Granted,
        TakenBy::Freed,
    ];

    /// The tenure a change of hands to `to` because of `cause` leaves: a
    /// town that goes independent is `Freed` (revolt, defiance); otherwise
    /// `Sack` is `Sacked`, `Submission` `Submitted`, `Invitation`
    /// `Invited`, `Subversion` `Subverted`, and every other cause
    /// (`Conquest`, a reprisal's retake, a test's `None`) `Conquered`.
    pub fn of(cause: Cause, to: FactionId) -> Self {
        if to == FactionId::INDEPENDENT {
            return TakenBy::Freed;
        }
        match cause {
            Cause::Sack { .. } => TakenBy::Sacked,
            Cause::Submission { .. } => TakenBy::Submitted,
            Cause::Invitation => TakenBy::Invited,
            Cause::Subversion => TakenBy::Subverted,
            _ => TakenBy::Conquered,
        }
    }

    /// Display name ("by the sword" lines).
    pub fn name(self) -> &'static str {
        match self {
            TakenBy::Native => "native",
            TakenBy::Conquered => "conquered",
            TakenBy::Sacked => "sacked",
            TakenBy::Submitted => "submitted",
            TakenBy::Invited => "sworn",
            TakenBy::Subverted => "turned",
            TakenBy::Granted => "granted",
            TakenBy::Freed => "freed",
        }
    }
}

/// One taking in [`Tenure::history`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Taking {
    /// How.
    pub how: TakenBy,
    /// By whom (the new owner).
    pub by: FactionId,
    /// The day, counted from 0.
    pub day: u32,
}

/// What a run settlement remembers about how it came to its owner
/// (GP-32, SIM-60, M4a). Hashed and saved. See the module doc.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Tenure {
    /// How the current owner took it.
    pub how: TakenBy,
    /// The current owner, as of the taking.
    pub by: FactionId,
    /// The tick of the taking.
    pub since: u64,
    /// Resentment (0..0.45), the Resentment term's size; fades daily.
    pub scar: f32,
    /// Militia of this town killed by [`Tenure::blood_by`] since it last
    /// changed hands (rebels not counted): the next taking's scar (and its
    /// broken men, step 6) reads it through [`Tenure::blood_of`] when that
    /// taking is `blood_by`'s, then it starts again at 0.
    pub blood: u32,
    /// Whose men spilt [`Tenure::blood`] (the independents when none): a
    /// militiaman killed fighting another faction starts the count again,
    /// so a taking never reads blood another faction spilt.
    pub blood_by: FactionId,
    /// An agent's stir (0..`subvert.max_stir`), the Stirred term's size.
    pub stir: f32,
    /// Promises of defence kept (an invited town's bond).
    pub kept: u8,
    /// Promises of defence broken (an invited town forsaken).
    pub broken: u8,
    /// A submitted town's weak days in a row.
    pub weak_days: u8,
    /// A submitted town's watched hours today.
    pub watched_hours: u8,
    /// No new demand before this tick (a refusal's cooldown).
    pub cool_until: u64,
    /// The day (counted from 1; 0: never) of the last gift that counted
    /// (`Deed::Gift`, at most one a day; M4a step 5).
    pub gift_day: u32,
    /// The day (counted from 1; 0: never) the town last saw you defend it
    /// (`Deed::Defended` or an answered drive-off, at most one a day).
    pub defended_day: u32,
    /// The tick + 1 (0: never) one of this town's broken men (a
    /// [`Bandits`] band of this home) last fell: a battle that saw it is
    /// never this town's defence (M4a step 5).
    pub kin_fell: u64,
    /// The last [`Tenure::HISTORY`] takings, oldest first, the current one
    /// last (a ring kept for M5's village memories).
    pub history: VecDeque<Taking>,
}

impl Default for Tenure {
    fn default() -> Self {
        Self::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0)
    }
}

impl Tenure {
    /// Takings kept in [`Tenure::history`].
    pub const HISTORY: usize = 4;

    /// A town held `how` by `by` since `since` (day `day`), with no scar.
    pub fn new(how: TakenBy, by: FactionId, since: u64, day: u32) -> Self {
        Self {
            how,
            by,
            since,
            scar: 0.0,
            blood: 0,
            blood_by: FactionId::INDEPENDENT,
            stir: 0.0,
            kept: 0,
            broken: 0,
            weak_days: 0,
            watched_hours: 0,
            cool_until: 0,
            gift_day: 0,
            defended_day: 0,
            kin_fell: 0,
            history: VecDeque::from([Taking { how, by, day }]),
        }
    }

    /// A militiaman of this town killed fighting `by` (SIM-60; never a
    /// rebel, who fights the town's own owner): one more drop of `by`'s
    /// blood, the count starting again if the last blood was another
    /// faction's.
    pub fn spill(&mut self, by: FactionId) {
        if self.blood_by != by {
            self.blood = 0;
            self.blood_by = by;
        }
        self.blood = self.blood.saturating_add(1);
    }

    /// The blood `taker` spilt here since the town last changed hands:
    /// [`Tenure::blood`] if [`Tenure::blood_by`] is `taker`, else 0.
    pub fn blood_of(&self, taker: FactionId) -> u32 {
        if self.blood_by == taker {
            self.blood
        } else {
            0
        }
    }

    /// A new taking: `how` by `by` at `tick` (day `day`) with `scar`. The
    /// blood, the stir, the bond's promises and the watch start again;
    /// a demand's cooldown and the days of the last gift and defence stay;
    /// the taking joins the history ring.
    pub fn retake(&mut self, how: TakenBy, by: FactionId, tick: u64, day: u32, scar: f32) {
        self.how = how;
        self.by = by;
        self.since = tick;
        self.scar = scar;
        self.blood = 0;
        self.blood_by = FactionId::INDEPENDENT;
        self.stir = 0.0;
        self.kept = 0;
        self.broken = 0;
        self.weak_days = 0;
        self.watched_hours = 0;
        if self.history.len() == Self::HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(Taking { how, by, day });
    }
}

impl StateHash for Tenure {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.how as u8);
        h.u16(self.by.0);
        h.u64(self.since);
        h.f32(self.scar);
        h.u32(self.blood);
        h.u16(self.blood_by.0);
        h.f32(self.stir);
        h.u8(self.kept);
        h.u8(self.broken);
        h.u8(self.weak_days);
        h.u8(self.watched_hours);
        h.u64(self.cool_until);
        h.u32(self.gift_day);
        h.u32(self.defended_day);
        h.u64(self.kin_fell);
        h.u32(self.history.len() as u32);
        for t in &self.history {
            h.u8(t.how as u8);
            h.u16(t.by.0);
            h.u32(t.day);
        }
    }
}

sim_core::save_serde!(TakenBy);
sim_core::save_fields!(Taking { how, by, day });
sim_core::save_fields!(Tenure {
    how,
    by,
    since,
    scar,
    blood,
    blood_by,
    stir,
    kept,
    broken,
    weak_days,
    watched_hours,
    cool_until,
    gift_day,
    defended_day,
    kin_fell,
    history
});

fn non_negative(v: f32) -> bool {
    v.is_finite() && v >= 0.0
}

/// Tenure's scars and the bond (SIM-60): `tenure` in progression.ron. See
/// docs/dev/DATA.md.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TenureData {
    /// A conquest's scar before blood.
    pub conquest_scar: f32,
    /// A sack's scar before blood.
    pub sack_scar: f32,
    /// Scar added per militiaman killed in the taking.
    pub scar_per_kill: f32,
    /// A conquest's scar at most.
    pub max_conquest_scar: f32,
    /// A sack's scar at most.
    pub max_sack_scar: f32,
    /// A turned town's scar.
    pub subverted_scar: f32,
    /// Scars fade this much a day.
    pub scar_fade_per_day: f32,
    /// An invited town's Bond term (M4a step 5).
    pub bond: f32,
    /// Bond added per defence kept ...
    pub kept_bond: f32,
    /// ... up to this.
    pub max_bond: f32,
    /// The Bond term once forsaken `forsaken_after` times.
    pub forsaken_bond: f32,
    /// Times forsaken that turn the bond.
    pub forsaken_after: u8,
}

impl Default for TenureData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            conquest_scar: 0.12,
            sack_scar: 0.25,
            scar_per_kill: 0.01,
            max_conquest_scar: 0.3,
            max_sack_scar: 0.45,
            subverted_scar: 0.1,
            scar_fade_per_day: 0.02,
            bond: 0.05,
            kept_bond: 0.02,
            max_bond: 0.1,
            forsaken_bond: -0.1,
            forsaken_after: 2,
        }
    }
}

impl TenureData {
    /// The scar a taking `how` leaves with `blood` militia killed:
    /// Conquered `conquest_scar + scar_per_kill x blood` (at most
    /// `max_conquest_scar`), Sacked likewise from `sack_scar` (at most
    /// `max_sack_scar`), Subverted `subverted_scar`, anything else 0.
    pub fn scar(&self, how: TakenBy, blood: u32) -> f32 {
        let bloody = |base: f32, max: f32| (base + self.scar_per_kill * blood as f32).min(max);
        match how {
            TakenBy::Conquered => bloody(self.conquest_scar, self.max_conquest_scar),
            TakenBy::Sacked => bloody(self.sack_scar, self.max_sack_scar),
            TakenBy::Subverted => self.subverted_scar,
            _ => 0.0,
        }
    }

    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        let scars = [
            self.conquest_scar,
            self.sack_scar,
            self.scar_per_kill,
            self.max_conquest_scar,
            self.max_sack_scar,
            self.subverted_scar,
            self.scar_fade_per_day,
        ];
        if scars.iter().any(|v| !(0.0..=1.0).contains(v))
            || self.conquest_scar > self.max_conquest_scar
            || self.sack_scar > self.max_sack_scar
        {
            return Err(
                "progression: tenure scars must be in 0..1, each base at most its max".into(),
            );
        }
        if [self.bond, self.kept_bond, self.max_bond, self.forsaken_bond]
            .iter()
            .any(|v| !(-1.0..=1.0).contains(v))
        {
            return Err("progression: tenure bond values must be in -1..1".into());
        }
        Ok(())
    }
}

/// Demanding submission (GP-32, M4a step 5): `demand` in progression.ron.
/// See docs/dev/DATA.md and LORDSHIP_DESIGN section 5.2.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DemandData {
    /// Your strength counts soldiers within this many tiles.
    pub radius: f32,
    /// The parley lasts this many game hours.
    pub parley_hours: f32,
    /// It submits when your strength is at least this times its defence.
    pub ratio: f32,
    /// Defence per able adult (the would-be militia).
    pub militia_weight: f32,
    /// Defence per soldier of its owner's garrison.
    pub garrison_weight: f32,
    /// Strength of an armed soldier (1 unarmed).
    pub armed_weight: f32,
    /// Residents' loyalty after kneeling.
    pub seed: f32,
    /// The Awe term while watched.
    pub awe: f32,
    /// The watch counts your strength and garrisons within this many tiles.
    pub watch_radius: f32,
    /// Watched when your strength there is at least this times its defence.
    pub hold_ratio: f32,
    /// A day with fewer watched hours than this is a weak day.
    pub watch_hours: u8,
    /// A town of yours with a garrison of at least this many watches it.
    pub watch_garrison: u32,
    /// Your other towns' influence share there that watches it.
    pub watch_share: f32,
    /// Weak days in a row before it defies you.
    pub weak_days: u8,
    /// Days after which a submitted town settles.
    pub settle_days: f32,
    /// Days a refusal keeps the town from another demand.
    pub refuse_cooldown_days: f32,
}

impl Default for DemandData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            radius: 40.0,
            parley_hours: 1.0,
            ratio: 1.5,
            militia_weight: 0.25,
            garrison_weight: 1.0,
            armed_weight: 1.5,
            seed: 0.45,
            awe: 0.1,
            watch_radius: 80.0,
            hold_ratio: 0.75,
            watch_hours: 6,
            watch_garrison: 4,
            watch_share: 0.5,
            weak_days: 2,
            settle_days: 10.0,
            refuse_cooldown_days: 2.0,
        }
    }
}

impl DemandData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        let ok = [
            self.radius,
            self.parley_hours,
            self.ratio,
            self.militia_weight,
            self.garrison_weight,
            self.armed_weight,
            self.awe,
            self.watch_radius,
            self.hold_ratio,
            self.settle_days,
            self.refuse_cooldown_days,
        ]
        .iter()
        .all(|v| non_negative(*v));
        if !ok
            || !(0.0..=1.0).contains(&self.seed)
            || !(0.0..=1.0).contains(&self.watch_share)
            || self.watch_hours > 24
        {
            return Err(
                "progression: demand values must be >= 0, seed and watch_share in 0..1, watch_hours at most 24"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Courting with gifts (GP-32, M4a step 5): `court` in progression.ron.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CourtData {
    /// A gift's goodwill per day of the town's need given ...
    pub gift_per_day_of_need: f32,
    /// ... at most this ...
    pub max_gift: f32,
    /// ... times this when the town is hungry.
    pub hungry_mult: f32,
}

impl Default for CourtData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            gift_per_day_of_need: 0.25,
            max_gift: 0.12,
            hungry_mult: 2.0,
        }
    }
}

impl CourtData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        if !non_negative(self.gift_per_day_of_need)
            || !(0.0..=1.0).contains(&self.max_gift)
            || !non_negative(self.hungry_mult)
        {
            return Err("progression: court values must be >= 0, max_gift in 0..1".into());
        }
        Ok(())
    }
}

/// Agents (GP-32, M4a step 7): `subvert` in progression.ron. See
/// LORDSHIP_DESIGN section 5.4.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubvertData {
    /// The bribe in food value ...
    pub bribe_food: f32,
    /// ... or in weapons or tools.
    pub bribe_goods: u32,
    /// Agents alive at most.
    pub max_agents: u32,
    /// A whisper's goodwill toward the rival, a day.
    pub whisper_rival: f32,
    /// A whisper's goodwill toward you, a day.
    pub whisper_you: f32,
    /// Whispers toward the rival times this in a hungry or levied town.
    pub hungry_mult: f32,
    /// Stir added a day.
    pub stir_per_day: f32,
    /// Stir at most (the Stirred term's size).
    pub max_stir: f32,
    /// Suspicion added a day ...
    pub suspicion_base: f32,
    /// ... plus this per 10 rival soldiers within `guard_radius` ...
    pub suspicion_per_10_guards: f32,
    /// ... plus this times the town's loyalty.
    pub suspicion_loyalty: f32,
    /// Rival soldiers within this many tiles count.
    pub guard_radius: f32,
    /// A second purse (a `Give` there) takes this much suspicion off.
    pub purse_relief: f32,
    /// The town turns below this mean loyalty ...
    pub turn_loyalty: f32,
    /// ... with at least this goodwill toward you.
    pub turn_goodwill: f32,
    /// Residents' loyalty after turning.
    pub seed: f32,
    /// Days a turned town stays fragile.
    pub fragile_days: f32,
}

impl Default for SubvertData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            bribe_food: 10.0,
            bribe_goods: 2,
            max_agents: 3,
            whisper_rival: -0.06,
            whisper_you: 0.05,
            hungry_mult: 1.5,
            stir_per_day: 0.1,
            max_stir: 0.4,
            suspicion_base: 0.06,
            suspicion_per_10_guards: 0.1,
            suspicion_loyalty: 0.08,
            guard_radius: 30.0,
            purse_relief: 0.3,
            turn_loyalty: 0.5,
            turn_goodwill: 0.25,
            seed: 0.5,
            fragile_days: 6.0,
        }
    }
}

impl SubvertData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        let ok = [
            self.bribe_food,
            self.hungry_mult,
            self.stir_per_day,
            self.max_stir,
            self.suspicion_base,
            self.suspicion_per_10_guards,
            self.suspicion_loyalty,
            self.guard_radius,
            self.purse_relief,
            self.fragile_days,
        ]
        .iter()
        .all(|v| non_negative(*v));
        let unit = [
            self.turn_loyalty,
            self.seed,
            self.max_stir,
            self.whisper_you.abs(),
            self.whisper_rival.abs(),
        ]
        .iter()
        .all(|v| (0.0..=1.0).contains(v));
        if !ok || !unit || !(-1.0..=1.0).contains(&self.turn_goodwill) {
            return Err(
                "progression: subvert values must be >= 0, whispers, max_stir, turn_loyalty and seed in range"
                    .into(),
            );
        }
        Ok(())
    }
}

/// The tick's day, counted from 0.
pub(crate) fn day_of(world: &World, tick: u64) -> u32 {
    (tick / world.resource::<Calendar>().ticks_per_day.max(1)) as u32
}

/// SIM-60: records a change of hands of `s` to `to` because of `cause`
/// in its [`Tenure`], if it has one (run settlements only): the
/// [`TakenBy::of`] the cause, the scar from `tenure` data and the blood
/// spilt, the history. Called by `conquest::transfer_seeded` after the
/// owner changed; returns the taking (`None` outside a run).
pub(crate) fn record_taking(
    world: &mut World,
    s: Entity,
    cause: Cause,
    to: FactionId,
) -> Option<TakenBy> {
    let how = TakenBy::of(cause, to);
    retake(world, s, how, to)
}

/// Writes a taking `how` by `to` into `s`'s [`Tenure`], if it has one,
/// with the scar the blood `to` spilt there gives it
/// ([`Tenure::blood_of`]), refreshes the influence field (GP-31,
/// `influence::refresh_field`: the town's power now counts for its new
/// owner, with its new scar and loyalty) and rebuilds the town's
/// `LoyaltyTerms` at once ([`rebuild_terms`]), so the terms, the Contest
/// term included, change in the same tick as the owner.
pub(crate) fn retake(world: &mut World, s: Entity, how: TakenBy, to: FactionId) -> Option<TakenBy> {
    let blood = world.get::<Tenure>(s)?.blood_of(to);
    let scar = world.resource::<Progression>().data.tenure.scar(how, blood);
    let tick = world.resource::<SimTick>().0;
    let day = day_of(world, tick);
    world.get_mut::<Tenure>(s)?.retake(how, to, tick, day, scar);
    crate::influence::refresh_field(world);
    rebuild_terms(world, s);
    Some(how)
}

/// SIM-60: rebuilds `s`'s `LoyaltyTerms` (if it has them and a
/// [`Tenure`]): Resentment `-scar`, Stirred `-stir`, Contest from
/// [`crate::influence::contest_term`] (GP-31, M4a step 3: the other side's
/// lead in the town's `Influence`, or the heartland at the player's
/// towns), Dues, Table and Steward from
/// [`crate::govern`]'s `dues_term`, `table_term` and `steward_term`, and
/// (GP-32, M4a step 5) Awe and Bond ([`awe_term`], [`bond_term`]); a town
/// paying tribute has no Dues term (it's the deal). Called by [`step`]
/// hourly and by a taking ([`retake`]); serial steps only.
pub(crate) fn rebuild_terms(world: &mut World, s: Entity) {
    rebuild_terms_watched(world, s, None);
}

/// [`rebuild_terms`] with the hour's watch already known (`watched`), so
/// [`step`] doesn't look twice.
fn rebuild_terms_watched(world: &mut World, s: Entity, watched: Option<bool>) {
    let Some(t) = world.get::<Tenure>(s) else {
        return;
    };
    let tick = world.resource::<SimTick>().0;
    let tribute = paying_tribute(world, t, tick);
    let (scar, stir) = (t.scar, t.stir);
    let bond = bond_term(&world.resource::<Progression>().data.tenure, t);
    let awe = if tribute {
        let watched = watched.unwrap_or_else(|| watch(world, s).is_some_and(|w| w.watched));
        if watched {
            world.resource::<Progression>().data.demand.awe
        } else {
            0.0
        }
    } else {
        0.0
    };
    // `0.0 - x`, not `-x`: no negative zero for an unscarred town.
    let mut next = LoyaltyTerms::default();
    next.set(LoyaltyTerm::Resentment, 0.0 - scar);
    next.set(LoyaltyTerm::Stirred, 0.0 - stir);
    next.set(LoyaltyTerm::Awe, awe);
    next.set(LoyaltyTerm::Bond, bond);
    next.set(
        LoyaltyTerm::Contest,
        crate::influence::contest_term(world, s),
    );
    let dues = if tribute {
        0.0
    } else {
        crate::govern::dues_term(world, s)
    };
    next.set(LoyaltyTerm::Dues, dues);
    next.set(LoyaltyTerm::Table, crate::govern::table_term(world, s));
    next.set(LoyaltyTerm::Steward, crate::govern::steward_term(world, s));
    if let Some(mut terms) = world.get_mut::<LoyaltyTerms>(s)
        && *terms != next
    {
        *terms = next;
    }
}

/// GP-32: the Bond term of a town with tenure `t` (`tenure` data): an
/// invited town's `bond + kept_bond x kept`, at most `max_bond`, or
/// `forsaken_bond` for good once forsaken `forsaken_after` times; 0 for
/// any other tenure.
pub fn bond_term(data: &TenureData, t: &Tenure) -> f32 {
    if t.how != TakenBy::Invited {
        return 0.0;
    }
    if data.forsaken_after > 0 && t.broken >= data.forsaken_after {
        data.forsaken_bond
    } else {
        (data.bond + data.kept_bond * t.kept as f32).min(data.max_bond)
    }
}

/// GP-32: the Awe term a submitted town has while it is watched
/// (`demand.awe`), as `rebuild_terms` last wrote it.
pub fn awe_term(terms: &LoyaltyTerms) -> f32 {
    terms.get(LoyaltyTerm::Awe)
}

/// The tick a town submitted at `since` settles (`demand.settle_days`
/// later): the tribute lock, the watch and the awe end then.
pub fn settles_at(world: &World, since: u64) -> u64 {
    let days = world.resource::<Progression>().data.demand.settle_days;
    since + world.resource::<Calendar>().days(days)
}

/// GP-32: whether a town with tenure `t` pays tribute at `tick`: it knelt
/// to a demand (`Submitted`) and hasn't settled yet ([`settles_at`]).
pub fn paying_tribute(world: &World, t: &Tenure, tick: u64) -> bool {
    t.how == TakenBy::Submitted && tick < settles_at(world, t.since)
}

/// A soldier's strength in a demand or a watch: `armed_weight` if armed,
/// else 1 (`demand` data).
pub fn soldier_strength(data: &DemandData, armed: bool) -> f32 {
    if armed { data.armed_weight } else { 1.0 }
}

/// GP-32: `faction`'s strength within `radius` of `at`: its soldiers (not
/// routed, not militia), 1 each or `demand.armed_weight` if armed
/// ([`soldier_strength`]). Reads this tick's [`FighterGrid`].
pub fn strength_near(world: &World, at: IVec2, radius: f32, faction: FactionId) -> f32 {
    let data = world.resource::<Progression>().data.demand;
    let mut sum = 0.0;
    world
        .resource::<FighterGrid>()
        .for_each_within(at.as_vec2(), radius, |e, _| {
            if e.side.faction == faction && e.side.against.is_none() && !e.routed {
                let armed = world.get::<Equipment>(e.entity).is_some_and(|q| q.weapon);
                sum += soldier_strength(&data, armed);
            }
        });
    sum
}

/// GP-32: a town's defence against a demand: `militia_weight` per able
/// adult plus `garrison_weight` per soldier of its owner's garrison.
pub fn defence(data: &DemandData, able: u32, garrison: u32) -> f32 {
    data.militia_weight * able as f32 + data.garrison_weight * garrison as f32
}

/// A demand's comparison at a town (GP-32): what the parley weighs and
/// what the right-click menu and the panel show ("need N within 40, have
/// M").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parley {
    /// Residents who would fight (`conquest::would_be_militia`).
    pub able: u32,
    /// Its owner's garrison (none at an independent town).
    pub garrison: u32,
    /// [`defence`].
    pub defence: f32,
    /// Strength needed: `demand.ratio x defence`.
    pub need: f32,
    /// The demander's strength within `demand.radius` ([`strength_near`]).
    pub strength: f32,
}

impl Parley {
    /// Whether the town kneels: strength at least the need.
    pub fn submits(&self) -> bool {
        self.strength >= self.need
    }

    /// The need as shown: rounded up.
    pub fn need_shown(&self) -> u32 {
        self.need.ceil() as u32
    }

    /// The strength as shown: rounded down, so it never overstates.
    pub fn have_shown(&self) -> u32 {
        self.strength.floor() as u32
    }
}

/// GP-32: the demand `faction` would make of town `s` now ([`Parley`]):
/// its able adults, its owner's garrison (the owner's soldiers within
/// army.ron `garrison.radius`; none for an independent town), the
/// defence, the need and `faction`'s strength within `demand.radius` of
/// its centre. `None` for a non-settlement. Reads this tick's
/// [`FighterGrid`].
pub fn parley(world: &World, s: Entity, faction: FactionId) -> Option<Parley> {
    let center = world.get::<Settlement>(s)?.center;
    let data = world.resource::<Progression>().data.demand;
    let owner = owner_of(world, s);
    let able = would_be_militia(world, s);
    let garrison = if owner == FactionId::INDEPENDENT {
        0
    } else {
        garrison_and_rebels(world, center, owner).0
    };
    let defence = defence(&data, able, garrison);
    Some(Parley {
        able,
        garrison,
        defence,
        need: data.ratio * defence,
        strength: strength_near(world, center, data.radius, faction),
    })
}

/// A submitted town's watch at one moment (GP-32, design section 5.2): the
/// three ways it can be watched and the verdict.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WatchView {
    /// Your strength within `demand.watch_radius`.
    pub strength: f32,
    /// The strength that watches it: `hold_ratio x` its defence (its able
    /// adults; you are its garrison now).
    pub hold: f32,
    /// A town of yours within `watch_radius` has a garrison of at least
    /// `watch_garrison` (the first by SpawnId).
    pub garrisoned_by: Option<u64>,
    /// Your other towns' influence share there (its own power left out; 0
    /// with influence off).
    pub share: f32,
    /// Any of the three holds.
    pub watched: bool,
}

/// GP-32: whether the player watches town `s` now ([`WatchView`]): (1)
/// your strength within `demand.watch_radius` is at least
/// `hold_ratio` times its defence; (2) a town of yours within that radius
/// has a garrison (your soldiers within army.ron `garrison.radius`) of at
/// least `watch_garrison`; (3) your other towns' influence gives you a
/// share of at least `watch_share` there (`influence::field_at` over every
/// held town but `s`; never with influence off). No army need stand
/// beside it. `None` for a non-settlement. Reads this tick's grid.
pub fn watch(world: &World, s: Entity) -> Option<WatchView> {
    let center = world.get::<Settlement>(s)?.center;
    let data = world.resource::<Progression>().data.demand;
    let player = FactionId::PLAYER;
    let strength = strength_near(world, center, data.watch_radius, player);
    let hold = data.hold_ratio * defence(&data, would_be_militia(world, s), 0);
    let mut towns: Vec<(u64, Entity, IVec2, FactionId, Option<Influence>)> = world
        .try_query::<(Entity, &SpawnId, &Settlement, &Owner, Option<&Influence>)>()
        .map(|mut q| {
            q.iter(world)
                .filter(|(e, ..)| *e != s)
                .map(|(e, id, st, o, inf)| (id.0, e, st.center, o.faction, inf.copied()))
                .collect()
        })
        .unwrap_or_default();
    towns.sort_unstable_by_key(|t| t.0);
    let reach2 = data.watch_radius * data.watch_radius;
    let garrisoned_by = towns
        .iter()
        .filter(|t| t.3 == player && (t.2 - center).as_vec2().length_squared() <= reach2)
        .find(|t| garrison_and_rebels(world, t.2, player).0 >= data.watch_garrison)
        .map(|t| t.0);
    let inf = world.resource::<Progression>().data.influence;
    let share = if inf.enabled {
        let sources: Vec<Source> = towns
            .iter()
            .filter_map(|t| t.4.as_ref().and_then(|i| Source::of(t.0, t.2, t.3, i)))
            .collect();
        field_at(&inf, &sources, center.as_vec2()).player
    } else {
        0.0
    };
    let watched =
        strength >= hold || garrisoned_by.is_some() || (inf.enabled && share >= data.watch_share);
    Some(WatchView {
        strength,
        hold,
        garrisoned_by,
        share,
        watched,
    })
}

/// GP-32: a submitted town's tribute: its Dues dial held at Tithe and
/// locked until it settles (`sim_agents::Edicts`; lane B's `govern` acts
/// on the dial). Only writes what differs.
pub(crate) fn hold_tribute(world: &mut World, s: Entity) {
    let Some(since) = world.get::<Tenure>(s).map(|t| t.since) else {
        return;
    };
    let tick = world.resource::<SimTick>().0;
    let until = settles_at(world, since);
    let Some(e) = world.get::<Edicts>(s) else {
        return;
    };
    let dial = e.dial(Dial::Dues);
    if e.dues == Dues::Tithe && e.pending_dues.is_none() && dial.locked_until >= until {
        return;
    }
    let mut e = world.get_mut::<Edicts>(s).expect("edicts");
    if e.dues != Dues::Tithe {
        e.dues = Dues::Tithe;
        e.dials[Dial::Dues as usize].since = tick;
    }
    e.pending_dues = None;
    let d = &mut e.dials[Dial::Dues as usize];
    d.by_player = false;
    d.locked_until = d.locked_until.max(until);
}

/// GP-32 (court): a gift of `food` (food value) from `faction`'s army at
/// town `s`, in a run: `Deed::Gift` of `court.gift_per_day_of_need x` the
/// days of the town's need given, at most `court.max_gift`, then times
/// `court.hungry_mult` when its food security is under `requests.food_days`;
/// at most one a town a day (`Tenure::gift_day`). Returns the goodwill
/// posted (0 when none counted).
pub(crate) fn gift(world: &mut World, s: Entity, faction: FactionId, food: f32) -> f32 {
    if food <= 0.0 || !world.resource::<RunState>().in_run() {
        return 0.0;
    }
    let tick = world.resource::<SimTick>().0;
    let today = day_of(world, tick) + 1;
    let Some(t) = world.get::<Tenure>(s) else {
        return 0.0;
    };
    if t.gift_day == today {
        return 0.0;
    }
    let Some(id) = world.get::<SpawnId>(s).map(|id| id.0) else {
        return 0.0;
    };
    let people = world
        .get::<SettlementEconomy>(s)
        .map_or(0, |se| se.population);
    let need = daily_need(world, people).max(1e-3);
    let (court, food_days) = {
        let data = &world.resource::<Progression>().data;
        (data.court, data.requests.food_days)
    };
    let hungry = world
        .get::<FoodSecurity>(s)
        .is_some_and(|fs| fs.days < food_days);
    let mut value = (court.gift_per_day_of_need * food / need).min(court.max_gift);
    if hungry {
        value *= court.hungry_mult;
    }
    if value <= 0.0 || !value.is_finite() {
        return 0.0;
    }
    world.get_mut::<Tenure>(s).expect("tenure").gift_day = today;
    world
        .resource_mut::<GoodwillLedger>()
        .push(id, faction, value, Deed::Gift);
    value
}

/// GP-32 (court): an answered drive-off at town `s` (`requests::complete`):
/// the day's defence is spent (no `Defended` deed beside the request's
/// own), and at a town that invited the player, one more promise kept.
pub(crate) fn drove_off(world: &mut World, s: Entity) {
    let tick = world.resource::<SimTick>().0;
    let today = day_of(world, tick) + 1;
    let Some(mut t) = world.get_mut::<Tenure>(s) else {
        return;
    };
    t.defended_day = today;
    if t.how == TakenBy::Invited && t.by == FactionId::PLAYER {
        t.kept = t.kept.saturating_add(1);
    }
    rebuild_terms(world, s);
}

/// GP-32 (court): a drive-off request at town `s` lapsed
/// (`requests::fail`, request `request`). At a town that invited the
/// player and is still the player's, it was forsaken: `Deed::Forsaken`
/// (which spreads), `Tenure::broken` counted, a `Forsaken` event, and at
/// `tenure.forsaken_after` the bond turns. Returns whether it was.
pub(crate) fn forsaken(world: &mut World, s: Entity, request: u64) -> bool {
    let player = FactionId::PLAYER;
    let Some(t) = world.get::<Tenure>(s) else {
        return false;
    };
    if t.how != TakenBy::Invited || t.by != player || owner_of(world, s) != player {
        return false;
    }
    let (Some(id), Some(center)) = (
        world.get::<SpawnId>(s).map(|i| i.0),
        world.get::<Settlement>(s).map(|st| st.center),
    ) else {
        return false;
    };
    let broken = {
        let mut t = world.get_mut::<Tenure>(s).expect("tenure");
        t.broken = t.broken.saturating_add(1);
        t.broken
    };
    goodwill_deed(world, s, player, Deed::Forsaken);
    world.write_message(
        SimEvent::new(SimEventKind::Forsaken, player)
            .subject(id)
            .other(request)
            .at(center)
            .amount(broken as i32)
            .cause(Cause::Lapsed),
    );
    let (name, who) = (settlement_name(world, s), faction_name(world, player));
    let after = world.resource::<Progression>().data.tenure.forsaken_after;
    if broken == after {
        log(
            world,
            format!("{name} called on {who} in vain again: the bond is broken"),
        );
    } else {
        log(world, format!("{name} called on {who} in vain"));
    }
    rebuild_terms(world, s);
    true
}

/// One army's battle report, as [`defended`] reads it.
struct Fight {
    captain: u64,
    foes: Vec<FactionId>,
    at: IVec2,
    kills: u32,
    faction: FactionId,
    started: u64,
}

/// GP-32 (court): `Deed::Defended` from the hour's battle reports. A
/// player army that ended a battle in the last game hour with at least one
/// kill, against raiders or the rival, with its captain within army.ron
/// `threat.radius` of an independent or player town, fought for that
/// town: `goodwill_deeds.defended` toward the player, at most once a town
/// a day (`Tenure::defended_day`, also spent by an answered drive-off), and
/// at a town that invited the player one more promise kept. Never for a
/// battle against bandits whose home is that town ([`Bandits`]: one of
/// them fell since the battle's first blow (`Tenure::kin_fell`, written in
/// `combat::casualties`), or a living band of its own stands within the
/// threat radius of the battle, or a raider report of the hour is that
/// band's).
fn defended(world: &mut World, tick: u64) {
    let hour = world.resource::<Calendar>().ticks_per_hour();
    let radius = world.resource::<Army>().data.threat.radius;
    let (player, raiders) = (FactionId::PLAYER, FactionId::RAIDERS);
    let reports: Vec<Fight> = world
        .resource::<sim_core::Chronicle>()
        .reports()
        .iter()
        .rev()
        .take_while(|r| r.tick + hour > tick)
        .filter(|r| r.tick <= tick)
        .map(|r| Fight {
            captain: r.captain,
            foes: r.foes.clone(),
            at: r.at,
            kills: r.kills,
            faction: r.faction,
            started: r.started,
        })
        .collect();
    let fights: Vec<&Fight> = reports
        .iter()
        .rev()
        .filter(|r| {
            r.faction == player
                && r.kills > 0
                && r.foes
                    .iter()
                    .any(|f| *f == raiders || *f == FactionId::RIVAL)
        })
        .collect();
    if fights.is_empty() {
        return;
    }
    // Bands of broken men by home, and where they stand (captains alive).
    let bands: Vec<(u64, u64, IVec2)> = world
        .try_query::<(&SpawnId, &Bandits, &sim_core::Position, &Captain)>()
        .map(|mut q| {
            q.iter(world)
                .map(|(id, b, p, _)| (id.0, b.home, p.0))
                .collect()
        })
        .unwrap_or_default();
    let raider_captains: Vec<u64> = reports
        .iter()
        .filter(|r| r.faction == raiders)
        .map(|r| r.captain)
        .collect();
    let mut towns: Vec<(u64, Entity, IVec2, FactionId)> = world
        .try_query_filtered::<(Entity, &SpawnId, &Settlement, &Owner), With<Tenure>>()
        .map(|mut q| {
            q.iter(world)
                .map(|(e, id, st, o)| (id.0, e, st.center, o.faction))
                .collect()
        })
        .unwrap_or_default();
    towns.sort_unstable_by_key(|t| t.0);
    let today = day_of(world, tick) + 1;
    let r2 = radius * radius;
    for Fight {
        foes, at, started, ..
    } in fights
    {
        for &(id, s, center, owner) in &towns {
            if !matches!(owner, FactionId::INDEPENDENT | FactionId::PLAYER)
                || (center - *at).as_vec2().length_squared() > r2
            {
                continue;
            }
            let own_sons = foes.contains(&raiders)
                && bands.iter().any(|&(cid, home, pos)| {
                    home == id
                        && (raider_captains.contains(&cid)
                            || (pos - *at).as_vec2().length_squared() <= r2)
                });
            if own_sons {
                continue;
            }
            let Some(mut t) = world.get_mut::<Tenure>(s) else {
                continue;
            };
            // One of its own sons fell in the battle's span.
            if t.kin_fell > *started {
                continue;
            }
            if t.defended_day == today {
                continue;
            }
            t.defended_day = today;
            if t.how == TakenBy::Invited && t.by == player && owner == player {
                t.kept = t.kept.saturating_add(1);
            }
            goodwill_deed(world, s, player, Deed::Defended);
        }
    }
}

/// GP-32: a submitted town's watch for this hour, its day judged on the
/// day tick, defiance and settling (design section 5.2). Returns the
/// hour's watch for the Awe term (`None`: not paying tribute), or `None`
/// after it defied.
fn keep_watch(world: &mut World, s: Entity, tick: u64, on_day: bool) -> Option<bool> {
    let t = world.get::<Tenure>(s)?.clone();
    if t.how != TakenBy::Submitted || t.by != FactionId::PLAYER {
        return None;
    }
    let cal = world.resource::<Calendar>().clone();
    let data = world.resource::<Progression>().data.demand;
    let settle = settles_at(world, t.since);
    if tick >= settle {
        // Settled: once, in the hour it crosses the line.
        if tick < settle + cal.ticks_per_hour() {
            let (name, who) = (
                settlement_name(world, s),
                faction_name(world, FactionId::PLAYER),
            );
            log(
                world,
                format!("{name} has settled under {who}: the tribute and the watch end"),
            );
        }
        return None;
    }
    // The day just ended, if the town knelt before it began.
    if on_day && t.since + cal.ticks_per_day <= tick {
        let weak = t.watched_hours < data.watch_hours;
        let weak_days = if weak {
            t.weak_days.saturating_add(1)
        } else {
            0
        };
        {
            let mut t = world.get_mut::<Tenure>(s).expect("tenure");
            t.weak_days = weak_days;
            t.watched_hours = 0;
        }
        if data.weak_days > 0 && weak_days >= data.weak_days {
            defy(world, s, tick);
            return None;
        }
    } else if on_day {
        world.get_mut::<Tenure>(s).expect("tenure").watched_hours = 0;
    }
    hold_tribute(world, s);
    let watched = watch(world, s).is_some_and(|w| w.watched);
    if watched {
        let mut t = world.get_mut::<Tenure>(s).expect("tenure");
        t.watched_hours = t.watched_hours.saturating_add(1);
    }
    Some(watched)
}

/// GP-32: a submitted town defies the player (two weak days): it goes free
/// ([`Cause::Defiance`], a `Defied` event), its Dues dial is let go (back
/// to its default, unlocked), and if any of the player's
/// soldiers stand at it (army.ron `garrison.radius`) its militia rises
/// against them for `garrison.revolt_hours`.
fn defy(world: &mut World, s: Entity, tick: u64) {
    let Some(center) = world.get::<Settlement>(s).map(|st| st.center) else {
        return;
    };
    let player = FactionId::PLAYER;
    let soldiers = garrison_and_rebels(world, center, player).0;
    transfer(world, s, FactionId::INDEPENDENT, Cause::Defiance);
    // The tribute ends with the town's submission: the dial is let go.
    if let Some(mut e) = world.get_mut::<Edicts>(s) {
        e.dues = Dues::default();
        let d = &mut e.dials[Dial::Dues as usize];
        d.locked_until = d.locked_until.min(tick);
        d.since = tick;
    }
    if soldiers > 0 {
        let hours = world.resource::<Army>().data.garrison.revolt_hours;
        let until = tick + world.resource::<Calendar>().hours(hours);
        if let Some(mut threat) = world.get_mut::<Threat>(s) {
            threat.provoked_by = Some(player);
            threat.provoked_until = until;
            threat.level = 1.0;
        }
        let (name, who) = (settlement_name(world, s), faction_name(world, player));
        log(world, format!("{name} takes up arms against {who}"));
    }
}

/// SIM-60: progress slot 14, hourly, run only (span
/// `sim_military::tenure::step`).
///
/// 1. GP-32 (M4a step 5): `Deed::Defended` from the hour's battle
///    reports (see `defended`).
/// 2. For each settlement with a [`Tenure`], in SpawnId order: on the day
///    tick its scar fades by `tenure.scar_fade_per_day` (unless it was
///    taken this very tick: a taking's scar stands for its first day
///    tick); a submitted town keeps its watch (GP-32: the hour counted if
///    watched, see [`watch`]; on the day tick the day just ended judged, a
///    weak day under `demand.watch_hours` watched hours, and at
///    `demand.weak_days` weak days in a row it defies you; its tribute
///    held; it settles after `demand.settle_days`); then its
///    `LoyaltyTerms` are rebuilt (`rebuild_terms`).
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::tenure::step").entered();
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let on_day = world.resource::<Calendar>().on_day(tick);
    defended(world, tick);
    let fade = world
        .resource::<Progression>()
        .data
        .tenure
        .scar_fade_per_day;
    let mut q = world.query_filtered::<(Entity, &SpawnId), With<Tenure>>();
    let mut towns: Vec<(SpawnId, Entity)> = q.iter(world).map(|(e, id)| (*id, e)).collect();
    towns.sort_unstable();
    for (_, s) in towns {
        if on_day
            && let Some(mut t) = world.get_mut::<Tenure>(s)
            && t.scar > 0.0
            && t.since < tick
        {
            t.scar = (t.scar - fade).max(0.0);
        }
        let watched = keep_watch(world, s, tick, on_day);
        rebuild_terms_watched(world, s, watched);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conquest::tests::settlements;
    use crate::conquest::transfer;
    use crate::progression::tests::run_app;

    /// SIM-60: each cause leaves its tenure: the kind, the owner, the tick
    /// and the scar, Conquered and Sacked deepened by the blood spilt
    /// (capped), which then starts again; the history keeps the last four.
    #[test]
    fn transfer_writes_tenure() {
        let (mut app, _) = run_app(42);
        let list = settlements(app.world_mut());
        let (_, s, _) = list[0];
        let world = app.world_mut();
        let data = world.resource::<Progression>().data.tenure;
        let t0 = world.get::<Tenure>(s).cloned().expect("a run settlement");
        assert_eq!(
            (t0.how, t0.by, t0.since),
            (TakenBy::Native, FactionId::INDEPENDENT, 0)
        );
        assert_eq!(t0.history.len(), 1);

        let p = FactionId::PLAYER;
        let cases = [
            (Cause::Conquest { captain: 1 }, p, TakenBy::Conquered, 3),
            (Cause::Revolt, FactionId::INDEPENDENT, TakenBy::Freed, 0),
            (Cause::Sack { captain: 1 }, p, TakenBy::Sacked, 30),
            (Cause::Defiance, FactionId::INDEPENDENT, TakenBy::Freed, 2),
            (Cause::Submission { captain: 1 }, p, TakenBy::Submitted, 5),
            (Cause::Invitation, p, TakenBy::Invited, 0),
            (Cause::Subversion, p, TakenBy::Subverted, 4),
            (Cause::None, FactionId::RIVAL, TakenBy::Conquered, 0),
        ];
        for (cause, to, how, blood) in cases {
            world.resource_mut::<SimTick>().0 += 7;
            let tick = world.resource::<SimTick>().0;
            {
                let mut t = world.get_mut::<Tenure>(s).unwrap();
                (t.blood, t.blood_by) = (blood, to);
            }
            transfer(world, s, to, cause).expect("populated");
            let t = world.get::<Tenure>(s).unwrap();
            assert_eq!((t.how, t.by, t.since), (how, to, tick), "{cause:?}");
            assert_eq!(t.scar, data.scar(how, blood), "{cause:?}");
            assert_eq!(
                (t.blood, t.blood_by),
                (0, FactionId::INDEPENDENT),
                "blood starts again after a taking"
            );
            assert_eq!(t.history.back().map(|h| (h.how, h.by)), Some((how, to)));
            assert!(t.history.len() <= Tenure::HISTORY);
        }
        assert_eq!(data.scar(TakenBy::Conquered, 3), 0.12 + 0.03);
        assert_eq!(data.scar(TakenBy::Conquered, 40), 0.3, "capped");
        assert_eq!(data.scar(TakenBy::Sacked, 30), 0.45, "capped");
        assert_eq!(data.scar(TakenBy::Subverted, 9), 0.1);
        assert_eq!(data.scar(TakenBy::Invited, 9), 0.0);
        assert_eq!(data.scar(TakenBy::Submitted, 9), 0.0);
    }

    /// SIM-60: `step` writes the Resentment term from the scar every hour
    /// and fades the scar by 0.02 at each day tick; outside a run (no
    /// `Tenure`) it does nothing. Influence is off, so the scar is the
    /// whole bias (the Contest term is `influence`'s, tested there).
    #[test]
    fn scars_fade_and_write_the_resentment_term() {
        let (mut app, _) = run_app(42);
        let (_, s, _) = settlements(app.world_mut())[0];
        let world = app.world_mut();
        world.resource_mut::<Progression>().data.influence.enabled = false;
        transfer(world, s, FactionId::PLAYER, Cause::Conquest { captain: 1 }).unwrap();
        let scar = world.get::<Tenure>(s).unwrap().scar;
        assert!((scar - 0.12).abs() < 1e-6, "{scar}");
        let day = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = day + 100;
        step(world);
        let terms = *world.get::<LoyaltyTerms>(s).unwrap();
        assert_eq!(terms.get(LoyaltyTerm::Resentment), -scar, "not a day tick");
        assert_eq!(terms.bias(), -scar);
        world.resource_mut::<SimTick>().0 = 2 * day;
        step(world);
        let faded = world.get::<Tenure>(s).unwrap().scar;
        assert!((faded - (scar - 0.02)).abs() < 1e-6, "{faded}");
        assert_eq!(
            world
                .get::<LoyaltyTerms>(s)
                .unwrap()
                .get(LoyaltyTerm::Resentment),
            -faded
        );
        for d in 3..20 {
            world.resource_mut::<SimTick>().0 = d * day;
            step(world);
        }
        assert_eq!(world.get::<Tenure>(s).unwrap().scar, 0.0, "faded away");
        assert_eq!(world.get::<LoyaltyTerms>(s).unwrap().bias(), 0.0);
    }

    /// SIM-60 (review of step 1): blood counts for the faction the militia
    /// fought, starting again when another faction kills; a taking reads
    /// only its taker's blood, so a conquest after a raid gets the bare
    /// conquest scar.
    #[test]
    fn a_taking_reads_only_its_takers_blood() {
        let mut t = Tenure::default();
        let (p, raiders) = (FactionId::PLAYER, FactionId::RAIDERS);
        assert_eq!((t.blood_of(p), t.blood_of(FactionId::INDEPENDENT)), (0, 0));
        for _ in 0..12 {
            t.spill(raiders);
        }
        assert_eq!((t.blood, t.blood_by), (12, raiders));
        assert_eq!((t.blood_of(raiders), t.blood_of(p)), (12, 0));
        t.spill(p);
        t.spill(p);
        assert_eq!((t.blood, t.blood_by), (2, p), "another's kill restarts");
        assert_eq!((t.blood_of(p), t.blood_of(raiders)), (2, 0));

        let (mut app, _) = run_app(42);
        let (_, s, _) = settlements(app.world_mut())[0];
        let world = app.world_mut();
        world.resource_mut::<SimTick>().0 = 5;
        let data = world.resource::<Progression>().data.tenure;
        for _ in 0..12 {
            world.get_mut::<Tenure>(s).unwrap().spill(raiders);
        }
        transfer(world, s, p, Cause::Conquest { captain: 1 }).unwrap();
        let t = world.get::<Tenure>(s).unwrap();
        assert_eq!(t.scar, data.conquest_scar, "the raiders' blood is not ours");
        assert_eq!((t.blood, t.blood_by), (0, FactionId::INDEPENDENT));
        // The terms change with the owner, not an hour later.
        assert_eq!(
            world
                .get::<LoyaltyTerms>(s)
                .unwrap()
                .get(LoyaltyTerm::Resentment),
            -data.conquest_scar
        );
    }

    /// SIM-60 (review of step 1): a taking on a day tick keeps its whole
    /// scar through that tick's fade; the next day tick fades it.
    #[test]
    fn a_scar_is_not_faded_on_the_tick_of_its_taking() {
        let (mut app, _) = run_app(42);
        let (_, s, _) = settlements(app.world_mut())[0];
        let world = app.world_mut();
        let day = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = 3 * day;
        transfer(world, s, FactionId::PLAYER, Cause::Conquest { captain: 1 }).unwrap();
        let scar = world.get::<Tenure>(s).unwrap().scar;
        step(world);
        assert_eq!(world.get::<Tenure>(s).unwrap().scar, scar, "not faded yet");
        world.resource_mut::<SimTick>().0 = 4 * day;
        step(world);
        let faded = world.get::<Tenure>(s).unwrap().scar;
        assert!((faded - (scar - 0.02)).abs() < 1e-6, "{faded}");
    }

    #[test]
    fn taken_by_tags_are_positions() {
        for (i, t) in TakenBy::ALL.iter().enumerate() {
            assert_eq!(*t as usize, i, "{}", t.name());
        }
        assert_eq!(
            TakenBy::of(Cause::Sack { captain: 0 }, FactionId::RIVAL),
            TakenBy::Sacked
        );
        assert_eq!(
            TakenBy::of(Cause::Invitation, FactionId::INDEPENDENT),
            TakenBy::Freed
        );
    }
}
