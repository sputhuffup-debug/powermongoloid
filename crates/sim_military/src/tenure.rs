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
//!   expose (`influence::contest_term`, `govern::dues_term`,
//!   `govern::table_term`, `govern::steward_term`), and on the day tick
//!   fades scars by `tenure.scar_fade_per_day`.
//!
//! The data blocks of the ways to take a town live here too: `tenure`,
//! `demand`, `court` and `subvert` (progression.ron).

use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_agents::{LoyaltyTerm, LoyaltyTerms};
use sim_core::{Calendar, Cause, FactionId, RunState, SimTick, SpawnId, StateHash, StateHasher};

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
    /// a demand's cooldown stays; the taking joins the history ring.
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
/// ([`Tenure::blood_of`]), and rebuilds the town's `LoyaltyTerms` at once
/// ([`rebuild_terms`]), so the terms change in the same tick as the owner.
pub(crate) fn retake(world: &mut World, s: Entity, how: TakenBy, to: FactionId) -> Option<TakenBy> {
    let blood = world.get::<Tenure>(s)?.blood_of(to);
    let scar = world.resource::<Progression>().data.tenure.scar(how, blood);
    let tick = world.resource::<SimTick>().0;
    let day = day_of(world, tick);
    world.get_mut::<Tenure>(s)?.retake(how, to, tick, day, scar);
    rebuild_terms(world, s);
    Some(how)
}

/// SIM-60: rebuilds `s`'s `LoyaltyTerms` (if it has them and a
/// [`Tenure`]): Resentment `-scar`, Stirred `-stir`, Contest from
/// [`crate::influence::contest_term`], Dues, Table and Steward from
/// [`crate::govern`]'s `dues_term`, `table_term` and `steward_term`. Awe
/// and Bond come with M4a step 5 and stay 0 until then. Called by [`step`]
/// hourly and by a taking ([`retake`]); serial steps only.
pub(crate) fn rebuild_terms(world: &mut World, s: Entity) {
    let Some(t) = world.get::<Tenure>(s) else {
        return;
    };
    // `0.0 - x`, not `-x`: no negative zero for an unscarred town.
    let mut next = LoyaltyTerms::default();
    next.set(LoyaltyTerm::Resentment, 0.0 - t.scar);
    next.set(LoyaltyTerm::Stirred, 0.0 - t.stir);
    next.set(
        LoyaltyTerm::Contest,
        crate::influence::contest_term(world, s),
    );
    next.set(LoyaltyTerm::Dues, crate::govern::dues_term(world, s));
    next.set(LoyaltyTerm::Table, crate::govern::table_term(world, s));
    next.set(LoyaltyTerm::Steward, crate::govern::steward_term(world, s));
    if let Some(mut terms) = world.get_mut::<LoyaltyTerms>(s)
        && *terms != next
    {
        *terms = next;
    }
}

/// SIM-60: progress slot 14, hourly, run only (span
/// `sim_military::tenure::step`). For each settlement with a [`Tenure`],
/// in SpawnId order: on the day tick its scar fades by
/// `tenure.scar_fade_per_day` (unless it was taken this very tick: a
/// taking's scar stands for its first day tick); then its `LoyaltyTerms`
/// are rebuilt (`rebuild_terms`).
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::tenure::step").entered();
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let tick = world.resource::<SimTick>().0;
    let on_day = world.resource::<Calendar>().on_day(tick);
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
        rebuild_terms(world, s);
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
    /// `Tenure`) it does nothing.
    #[test]
    fn scars_fade_and_write_the_resentment_term() {
        let (mut app, _) = run_app(42);
        let (_, s, _) = settlements(app.world_mut())[0];
        let world = app.world_mut();
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
