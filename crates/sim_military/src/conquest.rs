//! Conquest, garrisons and revolts (GP-05, GP-06; ADR 0014).
//!
//! - [`transfer`]: the one way sim_military hands a settlement to a
//!   faction. It goes through [`sim_agents::set_owner`], reports the change
//!   ([`SimEventKind::Conquest`], `Invited` or `Revolt`), seats a faction
//!   that held no land at its first settlement, counts conquests, and ends
//!   the run in a win when the player takes the rival's seat (GP-28).
//! - [`militia_or_foes_near`]: whether anyone still contests a centre. An
//!   `Attack` (in `orders::step_order`) takes a settlement by breaking its
//!   militia, then holding the centre uncontested for `conquest.hold_hours`.
//! - [`revolts`]: once a day, from the progress driver
//!   ([`crate::progression::progress`]). A settlement whose mean loyalty
//!   has been below `loyalty.revolt_threshold` for a day goes independent
//!   at once if it has no garrison, and rises against a weaker one; a
//!   rising that outnumbers what is left of the garrison a day later wins.

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_agents::{
    CombatStats, Enlisted, Loyalty, Needs, Owner, OwnerChange, SettlementLoyalty, Stance, Threat,
    Villager, Villagers,
};
use sim_core::{
    Calendar, Cause, FactionId, NameTable, RunOutcome, RunState, SimEvent, SimEventKind, SimTick,
    SpawnId,
};
use sim_world::Settlement;

use crate::components::{ArmyStats, Factions};
use crate::data::Army;
use crate::index::FighterGrid;
use crate::orders::{call_off, log};
use crate::progression::RunProgress;

/// The faction holding settlement `s` (independent if it has no [`Owner`]).
pub fn owner_of(world: &World, s: Entity) -> FactionId {
    world
        .get::<Owner>(s)
        .map_or(FactionId::INDEPENDENT, |o| o.faction)
}

fn settlement_name(world: &World, s: Entity) -> String {
    let names = world.resource::<NameTable>();
    world
        .get::<Settlement>(s)
        .map_or_else(|| "?".into(), |st| names.get(st.name).to_string())
}

fn faction_name(world: &World, f: FactionId) -> String {
    world.resource::<Army>().faction_name(f).to_string()
}

/// Hands settlement `s` to `to` because of `cause` (GP-05, GP-06), with
/// residents' loyalty seeded from their goodwill toward `to` (or
/// `loyalty.independent_target` when `to` is independent). See
/// [`transfer_seeded`].
pub fn transfer(world: &mut World, s: Entity, to: FactionId, cause: Cause) -> Option<OwnerChange> {
    transfer_seeded(world, s, to, cause, None)
}

/// The event kind a change of hands because of `cause` is reported as
/// (M4a, LORDSHIP_DESIGN section 11): `Invited` for an invitation,
/// `Revolt` for a revolt, `Submitted` for a submission, `Subverted` for a
/// subversion, `Defied` for a defiance, and `Conquest` for everything else
/// (a conquest, a sack, a reprisal's retake, a test's `None`).
pub fn taking_kind(cause: Cause) -> SimEventKind {
    match cause {
        Cause::Invitation => SimEventKind::Invited,
        Cause::Revolt => SimEventKind::Revolt,
        Cause::Submission { .. } => SimEventKind::Submitted,
        Cause::Subversion => SimEventKind::Subverted,
        Cause::Defiance => SimEventKind::Defied,
        _ => SimEventKind::Conquest,
    }
}

/// Hands settlement `s` to `to` because of `cause`, seeding residents'
/// loyalty with `seed` (or as [`sim_agents::set_owner`] does when `None`).
/// Call it from serial steps only.
///
/// - Sets the owner, allegiance and loyalty through `set_owner`.
/// - Writes a [`SimEvent`] of [`taking_kind`]`(cause)`. A `Revolt` or a
///   `Defied` has the old owner as its faction and the new one (the
///   independents) as `other`; every other kind has the taker as its
///   faction and the old owner as `other`.
/// - Only a `Conquest` (a conquest or a sack) counts in the taker's
///   `FactionStats::conquests`; every kind but `Revolt` and `Defied`
///   credits the champion's rank.
/// - M4a (SIM-60, run only): records the taking in the settlement's
///   [`Tenure`](crate::tenure::Tenure) (`tenure::record_taking`):
///   how (from the cause), by whom, the scar, the history.
/// - A faction (not the independents) that held no settlement before gets
///   its seat (where `Return` leads) at this one's centre.
/// - GP-28: when the player takes the rival's seat in a run, the run ends
///   in a win (`RunState::finish`, which does nothing outside a run), and
///   the rival's other settlements become independent
///   ([`crate::rival::free_lands`]).
///
/// Returns `None`, changing nothing, if `s` isn't a populated settlement.
pub fn transfer_seeded(
    world: &mut World,
    s: Entity,
    to: FactionId,
    cause: Cause,
    seed: Option<f32>,
) -> Option<OwnerChange> {
    let _span = tracing::info_span!("sim_military::transfer").entered();
    world.get::<Owner>(s)?;
    let id = world.get::<SpawnId>(s)?.0;
    let center = world.get::<Settlement>(s)?.center;
    let landless = to != FactionId::INDEPENDENT && !holds_land(world, to, s);
    let change = sim_agents::set_owner(world, s, to, seed)?;
    let kind = taking_kind(cause);
    let revolt_like = matches!(kind, SimEventKind::Revolt | SimEventKind::Defied);
    let (faction, other) = if revolt_like {
        (change.previous, to)
    } else {
        (to, change.previous)
    };
    world.write_message(
        SimEvent::new(kind, faction)
            .subject(id)
            .other(other.0 as u64)
            .at(center)
            .amount(change.residents as i32)
            .cause(cause),
    );
    if kind == SimEventKind::Conquest && to != FactionId::INDEPENDENT {
        world.resource_mut::<ArmyStats>().of(to).conquests += 1;
    }
    // SIM-35: a settlement taken or invited into counts toward the
    // champion's rank (a revolt's or a defiance's new owner is nobody).
    if !revolt_like {
        crate::progression::champion::credit(world, to, crate::progression::Feat::Settlement);
    }
    // SIM-60 (M4a): how it was taken, where the town keeps a tenure.
    crate::tenure::record_taking(world, s, cause, to);
    if landless {
        world.resource_mut::<Factions>().set_seat(to, center);
    }
    let name = settlement_name(world, s);
    let text = match kind {
        SimEventKind::Revolt => {
            let who = faction_name(world, change.previous);
            format!("{name} threw off {who} and is free")
        }
        SimEventKind::Defied => {
            let who = faction_name(world, change.previous);
            format!("{name} defied {who} and is free")
        }
        SimEventKind::Invited => format!("{name} invited {} in", faction_name(world, to)),
        SimEventKind::Submitted => format!("{name} knelt to {}", faction_name(world, to)),
        SimEventKind::Subverted => {
            format!("{name} opened its gates to {}", faction_name(world, to))
        }
        _ => format!("{} took {name}", faction_name(world, to)),
    };
    log(world, text);
    // GP-28: taking the rival's seat wins the run, and the rest of the
    // rival's lands go free.
    if to == FactionId::PLAYER
        && world.resource::<RunProgress>().rival_seat == Some(id)
        && world
            .resource_mut::<RunState>()
            .finish(RunOutcome::Won, cause)
    {
        crate::rival::free_lands(world);
    }
    Some(change)
}

/// Whether `faction` owns a populated settlement other than `except`.
fn holds_land(world: &mut World, faction: FactionId, except: Entity) -> bool {
    let mut q = world.query::<(Entity, &Owner)>();
    q.iter(world)
        .any(|(e, o)| e != except && o.faction == faction)
}

/// Whether anyone still contests `at` against `faction` within `r` tiles:
/// militia defending against `faction`, or soldiers of a faction at war
/// with it, who aren't routed. Reads this tick's [`FighterGrid`].
pub fn militia_or_foes_near(world: &World, at: IVec2, faction: FactionId, r: f32) -> bool {
    let army = world.resource::<Army>();
    let grid = world.resource::<FighterGrid>();
    let mut any = false;
    grid.for_each_within(at.as_vec2(), r, |e, _| {
        if e.routed {
            return;
        }
        let contests = match e.side.against {
            Some(f) => f == faction,
            None => army.at_war(e.side.faction, faction),
        };
        any |= contests;
    });
    any
}

/// Who stands within `garrison.radius` of `at`, not routed: soldiers of
/// `owner` (its garrison) and militia rising against `owner` (rebels).
/// Reads this tick's [`FighterGrid`].
pub(crate) fn garrison_and_rebels(world: &World, at: IVec2, owner: FactionId) -> (u32, u32) {
    let radius = world.resource::<Army>().data.garrison.radius;
    let grid = world.resource::<FighterGrid>();
    let (mut guards, mut rebels) = (0, 0);
    grid.for_each_within(at.as_vec2(), radius, |e, _| {
        if e.routed {
            return;
        }
        match e.side.against {
            None if e.side.faction == owner => guards += 1,
            Some(f) if f == owner => rebels += 1,
            _ => {}
        }
    });
    (guards, rebels)
}

/// Residents of `s` who could take up arms (the villager AI's own test for
/// fighting, SIM-13): adult, alive, not starving, civilian, with morale at
/// least `fight.break_morale`. A demand's defence and a submitted town's
/// watch weigh them too (GP-32, `tenure::parley`, `tenure::watch`).
pub(crate) fn would_be_militia(world: &World, s: Entity) -> u32 {
    let villagers = world.resource::<Villagers>();
    let starving = villagers.data.needs.starving;
    let break_morale = villagers.data.ai.fight.break_morale;
    let adult = villagers.data.households.working_age.min;
    let days_per_year = world.resource::<Calendar>().days_per_year().max(1);
    let Some(mut q) =
        world.try_query_filtered::<(&Villager, &CombatStats, &Needs), Without<Enlisted>>()
    else {
        return 0;
    };
    q.iter(world)
        .filter(|(v, cs, needs)| {
            v.settlement == s
                && v.age_days as u32 / days_per_year >= adult
                && cs.alive()
                && cs.stance == Stance::Civilian
                && cs.morale >= break_morale
                && needs.hunger < starving
        })
        .count() as u32
}

/// GP-06: once a day (the progress driver calls it every hour; it acts on
/// day ticks only), settlements in SpawnId order. For each owned one, with
/// its garrison (the owner's soldiers within `garrison.radius` of the
/// centre) and its rebels (militia risen against the owner, there):
///
/// - **A rising under way** (`SettlementLoyalty::revolt_since`) is settled:
///   if no garrison is left, or the rebels still standing outnumber it, the
///   settlement goes independent ([`Cause::Revolt`]). Otherwise it was put
///   down: the provocation ends and every resident loses
///   `garrison.crushed_loyalty`.
/// - **Unrest for a day** (`below_since` at least a day ago): with no
///   garrison the settlement goes independent at once (starving villagers
///   can't fight, so a starved, unguarded conquest is lost without a
///   blow). With a garrison smaller than the residents able to fight, they
///   rise: the settlement is provoked against its owner for
///   `garrison.revolt_hours`, and its militia fights the garrison.
///
/// Draws no randomness and does nothing while no settlement is owned (as
/// in the developed and skirmish worlds until an `Attack` completes).
pub fn revolts(world: &mut World) {
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    if !cal.on_day(tick) {
        return;
    }
    let _span = tracing::info_span!("sim_military::revolts").entered();
    let garrison = world.resource::<Army>().data.garrison.clone();
    let mut q = world.query::<(Entity, &SpawnId, &Owner, &SettlementLoyalty, &Settlement)>();
    let mut owned: Vec<(SpawnId, Entity, FactionId, SettlementLoyalty, IVec2)> = q
        .iter(world)
        .filter(|(_, _, o, ..)| o.faction != FactionId::INDEPENDENT)
        .map(|(e, id, o, sl, st)| (*id, e, o.faction, *sl, st.center))
        .collect();
    owned.sort_unstable_by_key(|o| o.0);
    for (_, s, owner, sl, center) in owned {
        let (guards, rebels) = garrison_and_rebels(world, center, owner);
        if sl.revolt_since.is_some() {
            if guards == 0 || rebels > guards {
                transfer(world, s, FactionId::INDEPENDENT, Cause::Revolt);
            } else {
                crush(world, s, owner, garrison.crushed_loyalty);
            }
            continue;
        }
        let Some(below) = sl.below_since else {
            continue;
        };
        if tick < below.saturating_add(cal.ticks_per_day) {
            continue;
        }
        if guards == 0 {
            transfer(world, s, FactionId::INDEPENDENT, Cause::Revolt);
            continue;
        }
        let able = would_be_militia(world, s);
        if guards < able {
            rise(world, s, owner, tick + cal.hours(garrison.revolt_hours));
        }
    }
}

/// A rising: the settlement's militia takes up arms against its owner
/// until `until`, and the revolt clock starts.
fn rise(world: &mut World, s: Entity, owner: FactionId, until: u64) {
    let tick = world.resource::<SimTick>().0;
    if let Some(mut threat) = world.get_mut::<Threat>(s) {
        threat.provoked_by = Some(owner);
        threat.provoked_until = until;
        threat.level = 1.0;
    }
    if let Some(mut sl) = world.get_mut::<SettlementLoyalty>(s) {
        sl.revolt_since = Some(tick);
    }
    let (name, who) = (settlement_name(world, s), faction_name(world, owner));
    log(world, format!("{name} rises against {who}"));
}

/// A rising put down: the militia stands down and residents (not
/// enlisted, in SpawnId order) lose `penalty` loyalty.
fn crush(world: &mut World, s: Entity, owner: FactionId, penalty: f32) {
    call_off(world, s, owner);
    if let Some(mut sl) = world.get_mut::<SettlementLoyalty>(s) {
        sl.revolt_since = None;
    }
    let mut q =
        world.query_filtered::<(Entity, &SpawnId, &Villager), (With<Loyalty>, Without<Enlisted>)>();
    let mut residents: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, v)| v.settlement == s)
        .map(|(e, id, _)| (*id, e))
        .collect();
    residents.sort_unstable();
    for (_, e) in residents {
        if let Some(mut l) = world.get_mut::<Loyalty>(e) {
            l.0 = (l.0 - penalty).clamp(0.0, 1.0);
        }
    }
    let (name, who) = (settlement_name(world, s), faction_name(world, owner));
    log(world, format!("{who} put down the rising in {name}"));
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::components::{Captain, MilitaryLog};
    use crate::progression::tests::{command, run_app};
    use crate::spawn::{ArmySpec, spawn_army};
    use sim_core::{Allegiance, Chronicle, CommandKind, OrderSpec, SimStepper};

    /// Populated settlements other than the rival's seat, whoever owns
    /// them, in SpawnId order.
    fn any_settlements(world: &mut World) -> Vec<(u64, Entity, IVec2)> {
        let seat = world.resource::<RunProgress>().rival_seat;
        let mut q = world.query_filtered::<(Entity, &SpawnId, &Settlement), With<Owner>>();
        let mut v: Vec<(u64, Entity, IVec2)> = q
            .iter(world)
            .filter(|(_, id, _)| Some(id.0) != seat)
            .map(|(e, id, st)| (id.0, e, st.center))
            .collect();
        v.sort_unstable_by_key(|s| s.0);
        v
    }

    /// Populated independent settlements (never the rival's lands, GP-28),
    /// in SpawnId order.
    pub(crate) fn settlements(world: &mut World) -> Vec<(u64, Entity, IVec2)> {
        let seat = world.resource::<RunProgress>().rival_seat;
        let mut q = world.query::<(Entity, &SpawnId, &Settlement, &Owner)>();
        let mut v: Vec<(u64, Entity, IVec2)> = q
            .iter(world)
            .filter(|(_, id, _, o)| Some(id.0) != seat && o.faction == FactionId::INDEPENDENT)
            .map(|(e, id, st, _)| (id.0, e, st.center))
            .collect();
        v.sort_unstable_by_key(|s| s.0);
        v
    }

    /// (allegiance, loyalty) of `s`'s residents who aren't enlisted, in
    /// SpawnId order.
    fn residents(world: &mut World, s: Entity) -> Vec<(FactionId, f32)> {
        let mut q = world
            .query_filtered::<(&SpawnId, &Villager, &Allegiance, &Loyalty), Without<Enlisted>>();
        let mut v: Vec<(SpawnId, FactionId, f32)> = q
            .iter(world)
            .filter(|(_, v, ..)| v.settlement == s)
            .map(|(id, _, a, l)| (*id, a.0, l.0))
            .collect();
        v.sort_unstable_by_key(|r| r.0);
        v.into_iter().map(|(_, a, l)| (a, l)).collect()
    }

    fn rejected(app: &bevy_app::App) -> u32 {
        app.world()
            .resource::<ArmyStats>()
            .get(FactionId::PLAYER)
            .rejected_commands
    }

    fn issue(app: &mut bevy_app::App, captain: u64, order: OrderSpec) {
        let kind = CommandKind::IssueOrder {
            captain,
            order,
            queue: false,
        };
        command(app, FactionId::PLAYER, kind);
    }

    /// GP-05, GP-06, GP-28: `transfer` hands a settlement and its residents
    /// over, reports it, seats a landless faction at its first settlement
    /// only, and counts conquests; a revolt hands it back to independence.
    /// Garrison is refused on land you don't hold, Attack on land you do.
    /// Taking the rival's seat wins the run.
    #[test]
    fn transfer_hands_over_reports_seats_and_wins_the_seat() {
        let (mut app, start) = run_app(42);
        let champ = app.world().get::<SpawnId>(start.champion).unwrap().0;
        let list = settlements(app.world_mut());
        let ((aid, a, ac), (bid, b, _)) = (list[0], list[1]);

        issue(&mut app, champ, OrderSpec::Garrison { settlement: aid });
        assert_eq!(rejected(&app), 1, "garrison of land you don't hold");

        let world = app.world_mut();
        let tick = world.resource::<SimTick>().0;
        let cause = Cause::Conquest { captain: champ };
        let change = transfer(world, a, FactionId::PLAYER, cause).expect("populated");
        assert_eq!(change.previous, FactionId::INDEPENDENT);
        let people = residents(world, a);
        assert_eq!(change.residents as usize, people.len());
        assert!(!people.is_empty());
        assert!(people.iter().all(|r| r.0 == FactionId::PLAYER));
        assert_eq!(owner_of(world, a), FactionId::PLAYER);
        assert_eq!(world.get::<Owner>(a).unwrap().since, tick);
        let seat = |world: &World| world.resource::<Factions>().seat(FactionId::PLAYER);
        assert_eq!(
            seat(world),
            Some(ac),
            "a landless band is seated at its first village"
        );
        transfer(world, b, FactionId::PLAYER, cause).expect("populated");
        assert_eq!(seat(world), Some(ac), "the seat stays at the first");
        let conquests = |world: &World| {
            world
                .resource::<ArmyStats>()
                .get(FactionId::PLAYER)
                .conquests
        };
        assert_eq!(conquests(world), 2);
        assert!(transfer(world, start.champion, FactionId::PLAYER, cause).is_none());

        issue(&mut app, champ, OrderSpec::Attack { settlement: aid });
        assert_eq!(rejected(&app), 2, "attacking your own village");
        issue(&mut app, champ, OrderSpec::Garrison { settlement: aid });
        assert_eq!(rejected(&app), 2, "garrisoning it is fine");
        let front = app.world().get::<Captain>(start.champion).unwrap().orders[0];
        assert_eq!(front.spec, OrderSpec::Garrison { settlement: aid });

        let world = app.world_mut();
        let change = transfer(world, b, FactionId::INDEPENDENT, Cause::Revolt).unwrap();
        assert_eq!(change.previous, FactionId::PLAYER);
        let free = world
            .resource::<Villagers>()
            .data
            .loyalty
            .independent_target;
        assert!(
            residents(world, b)
                .iter()
                .all(|r| *r == (FactionId::INDEPENDENT, free))
        );
        assert_eq!(conquests(world), 2, "a revolt is no conquest");
        app.step(1);
        let events: Vec<SimEvent> = app
            .world()
            .resource::<Chronicle>()
            .entries()
            .iter()
            .map(|e| e.event)
            .filter(|e| matches!(e.kind, SimEventKind::Conquest | SimEventKind::Revolt))
            .collect();
        let (pl, ind) = (FactionId::PLAYER, FactionId::INDEPENDENT);
        let shape = |e: &SimEvent| (e.kind, e.faction, e.subject, e.other, e.cause);
        let shapes: Vec<_> = events.iter().map(shape).collect();
        assert_eq!(
            shapes,
            [
                (SimEventKind::Conquest, pl, aid, ind.0 as u64, cause),
                (SimEventKind::Conquest, pl, bid, ind.0 as u64, cause),
                (SimEventKind::Revolt, pl, bid, ind.0 as u64, Cause::Revolt),
            ]
        );
        assert_eq!(events[0].at, ac);

        // GP-28: the rival's seat.
        let world = app.world_mut();
        let seat_id = world.resource::<RunProgress>().rival_seat.unwrap();
        let seat_e = crate::orders::find_settlement(world, seat_id).unwrap();
        transfer(world, seat_e, FactionId::PLAYER, cause).unwrap();
        assert!(world.resource::<RunState>().is_over());
        app.step(1);
        let run = *app.world().resource::<RunState>();
        assert_eq!(run.outcome, Some(RunOutcome::Won));
        assert_eq!(run.cause, cause);
    }

    /// M4a (LORDSHIP_DESIGN section 11): every cause maps to its event
    /// kind, faction and other, `conquests` count and champion credit: a
    /// sack is a conquest, a submission and a subversion are takes of their
    /// own with no `conquests` count, a defiance reads like a revolt with no
    /// champion credit.
    #[test]
    fn every_cause_maps_to_its_kind_and_counters() {
        use crate::progression::ChampionRank;
        let (mut app, start) = run_app(42);
        let list = settlements(app.world_mut());
        let (pl, ind) = (FactionId::PLAYER, FactionId::INDEPENDENT);
        // (cause, to, kind, event faction, event other, conquests +, credit)
        let cases = [
            (
                Cause::Conquest { captain: 1 },
                pl,
                SimEventKind::Conquest,
                pl,
                ind,
                1,
                true,
            ),
            (Cause::Revolt, ind, SimEventKind::Revolt, pl, ind, 0, false),
            (
                Cause::Sack { captain: 1 },
                pl,
                SimEventKind::Conquest,
                pl,
                ind,
                1,
                true,
            ),
            (
                Cause::Defiance,
                ind,
                SimEventKind::Defied,
                pl,
                ind,
                0,
                false,
            ),
            (
                Cause::Invitation,
                pl,
                SimEventKind::Invited,
                pl,
                ind,
                0,
                true,
            ),
            (Cause::Revolt, ind, SimEventKind::Revolt, pl, ind, 0, false),
            (
                Cause::Submission { captain: 1 },
                pl,
                SimEventKind::Submitted,
                pl,
                ind,
                0,
                true,
            ),
            (Cause::Revolt, ind, SimEventKind::Revolt, pl, ind, 0, false),
            (
                Cause::Subversion,
                pl,
                SimEventKind::Subverted,
                pl,
                ind,
                0,
                true,
            ),
        ];
        let (sid, s, _) = list[3];
        for (cause, to, kind, faction, other, conquests, credit) in cases {
            assert_eq!(taking_kind(cause), kind, "{cause:?}");
            let world = app.world_mut();
            let before = world.resource::<ArmyStats>().get(pl).conquests;
            let points = world.get::<ChampionRank>(start.champion).unwrap().points;
            transfer(world, s, to, cause).expect("populated");
            let after = world.resource::<ArmyStats>().get(pl).conquests;
            assert_eq!(after - before, conquests, "{cause:?}");
            let gained = world.get::<ChampionRank>(start.champion).unwrap().points > points;
            assert_eq!(gained, credit && to == pl, "{cause:?}");
            app.step(1);
            let last = *app
                .world()
                .resource::<Chronicle>()
                .entries()
                .iter()
                .rev()
                .find(|e| e.event.subject == sid)
                .expect("chronicled");
            let e = last.event;
            assert_eq!(
                (e.kind, e.faction, e.other, e.cause),
                (kind, faction, other.0 as u64, cause),
                "{cause:?}"
            );
        }
    }

    /// GP-06: a day of unrest frees an unguarded conquest at once, raises
    /// a rising against a garrison smaller than the residents able to
    /// fight, and leaves a strong garrison's village alone. A rising the
    /// garrison outlasts is put down: the provocation ends and every
    /// resident loses `garrison.crushed_loyalty`. Calls [`revolts`]
    /// directly at day ticks, after one tick has built the fighter grid.
    #[test]
    fn unrest_frees_the_unguarded_and_raises_the_weakly_held() {
        let (mut app, _) = run_app(42);
        let list = settlements(app.world_mut());
        let (_, a, _) = list[0];
        let (_, b, bc) = list[1];
        let (_, c, cc) = list[2];
        let world = app.world_mut();
        let cause = Cause::Conquest { captain: 0 };
        for s in [a, b, c] {
            transfer(world, s, FactionId::PLAYER, cause).unwrap();
        }
        let able = would_be_militia(world, c);
        assert!(able > 1, "only {able} could fight");
        spawn_army(world, &ArmySpec::new(FactionId::PLAYER, bc, 1));
        spawn_army(world, &ArmySpec::new(FactionId::PLAYER, cc, able + 5));
        app.step(1);

        let world = app.world_mut();
        let (guards_b, _) = garrison_and_rebels(world, bc, FactionId::PLAYER);
        let (guards_c, _) = garrison_and_rebels(world, cc, FactionId::PLAYER);
        assert_eq!(guards_b, 2);
        assert!(guards_c >= able, "{guards_c} guards, {able} able");
        assert!(guards_b < would_be_militia(world, b));
        let day = world.resource::<Calendar>().ticks_per_day;
        for s in [a, b, c] {
            world.get_mut::<SettlementLoyalty>(s).unwrap().below_since = Some(0);
        }
        world.resource_mut::<SimTick>().0 = day - 1;
        revolts(world);
        assert_eq!(owner_of(world, a), FactionId::PLAYER, "not a day tick");

        world.resource_mut::<SimTick>().0 = day;
        revolts(world);
        assert_eq!(owner_of(world, a), FactionId::INDEPENDENT, "no garrison");
        assert_eq!(owner_of(world, b), FactionId::PLAYER);
        let threat = *world.get::<Threat>(b).unwrap();
        let until = day + world.resource::<Calendar>().hours(24.0);
        assert_eq!(threat.provoked_by, Some(FactionId::PLAYER));
        assert_eq!(threat.provoked_until, until);
        assert_eq!(
            world.get::<SettlementLoyalty>(b).unwrap().revolt_since,
            Some(day)
        );
        assert_eq!(owner_of(world, c), FactionId::PLAYER);
        assert_eq!(
            world.get::<SettlementLoyalty>(c).unwrap().revolt_since,
            None
        );
        assert_eq!(world.get::<Threat>(c).unwrap().provoked_by, None);

        // A day later no rebel stands in the (unchanged) grid: put down.
        let before = residents(world, b);
        world.resource_mut::<SimTick>().0 = 2 * day;
        revolts(world);
        assert_eq!(owner_of(world, b), FactionId::PLAYER);
        assert_eq!(
            world.get::<SettlementLoyalty>(b).unwrap().revolt_since,
            None
        );
        assert_eq!(world.get::<Threat>(b).unwrap().provoked_by, None);
        let penalty = world.resource::<Army>().data.garrison.crushed_loyalty;
        let after = residents(world, b);
        assert_eq!(before.len(), after.len());
        for (x, y) in before.iter().zip(&after) {
            assert_eq!(y.1, (x.1 - penalty).clamp(0.0, 1.0));
        }
        let log = &world.resource::<MilitaryLog>().entries;
        let lines: Vec<&str> = log.iter().map(|(_, l)| l.as_str()).collect();
        assert!(lines.iter().any(|l| l.contains("is free")), "{lines:?}");
        assert!(
            lines.iter().any(|l| l.contains("rises against")),
            "{lines:?}"
        );
        assert!(lines.iter().any(|l| l.contains("put down")), "{lines:?}");
    }

    /// GP-05: Garrison points the band at the settlement's centre, however
    /// far away it stands, and ends once the settlement is lost.
    #[test]
    fn a_garrison_marches_to_its_village_and_ends_when_it_is_lost() {
        let (mut app, start) = run_app(42);
        let champ = app.world().get::<SpawnId>(start.champion).unwrap().0;
        let (cid, c, cc) = settlements(app.world_mut())[2];
        let world = app.world_mut();
        transfer(world, c, FactionId::PLAYER, Cause::None).unwrap();
        let radius = world.resource::<Army>().data.garrison.radius;
        let tile = world.get::<sim_core::Position>(start.champion).unwrap().0;
        assert!(
            (cc - tile).as_vec2().length() > radius,
            "the champion starts inside the garrison radius"
        );
        issue(&mut app, champ, OrderSpec::Garrison { settlement: cid });
        assert_eq!(rejected(&app), 0);
        app.step(1);
        let captain = app.world().get::<Captain>(start.champion).unwrap();
        assert_eq!(captain.goal, Some(cc), "marching on the centre");
        assert_eq!(
            captain.orders[0].spec,
            OrderSpec::Garrison { settlement: cid }
        );

        transfer(app.world_mut(), c, FactionId::INDEPENDENT, Cause::Revolt).unwrap();
        app.step(1);
        let captain = app.world().get::<Captain>(start.champion).unwrap();
        assert!(
            captain
                .orders
                .front()
                .is_none_or(|o| o.spec != OrderSpec::Garrison { settlement: cid }),
            "still garrisoning a village it lost"
        );
        let log = &app.world().resource::<MilitaryLog>().entries;
        assert!(
            log.iter().any(|(_, l)| l.contains("no longer holds")),
            "{log:?}"
        );
    }

    /// GP-06: a rising against a weak garrison wins. A day of unrest under
    /// a two-man garrison raises the residents against their owner; their
    /// militia (`Stance::Militia { against: owner }`) takes the field, and
    /// at the next daily check the settlement goes independent. Runs the
    /// real schedule, so the villager AI, the threat exception for a
    /// rising and the fighter grid all take part.
    #[test]
    fn a_rising_beats_a_weak_garrison() {
        let (mut app, _) = run_app(42);
        // Past tick 0's day tick, so the first drift is the one at `day`.
        app.step(1);
        // Settlement #1 (the rival's at the start; transfer replaces it).
        let (bid, b, bc) = any_settlements(app.world_mut())[1];
        let world = app.world_mut();
        transfer(world, b, FactionId::PLAYER, Cause::None).unwrap();
        spawn_army(world, &ArmySpec::new(FactionId::PLAYER, bc, 1));
        // A day of unrest already, and loyalty low enough that the first
        // day tick's drift keeps the mean below the threshold.
        world.get_mut::<SettlementLoyalty>(b).unwrap().below_since = Some(0);
        let mut q = world.query::<(&Villager, &mut Loyalty)>();
        for (v, mut l) in q.iter_mut(world) {
            if v.settlement == b {
                l.0 = 0.0;
            }
        }
        let cal = world.resource::<Calendar>().clone();
        let day = cal.ticks_per_day;
        let now = world.resource::<SimTick>().0;
        app.step(day - now + 1);

        let world = app.world_mut();
        let sl = *world.get::<SettlementLoyalty>(b).unwrap();
        assert_eq!(sl.revolt_since, Some(day), "no rising at the day tick");
        assert_eq!(owner_of(world, b), FactionId::PLAYER);
        assert_eq!(
            world.get::<Threat>(b).unwrap().provoked_by,
            Some(FactionId::PLAYER)
        );

        app.step(cal.hours(6.0));
        let world = app.world_mut();
        let mut q = world.query::<(&Villager, &CombatStats)>();
        let risen = q
            .iter(world)
            .filter(|(v, cs)| {
                v.settlement == b
                    && cs.stance
                        == Stance::Militia {
                            against: FactionId::PLAYER,
                        }
            })
            .count();
        assert!(risen > 0, "nobody took up arms against the owner");
        let (_, rebels) = garrison_and_rebels(world, bc, FactionId::PLAYER);
        assert!(rebels > 0, "no rebel in the field");

        let now = world.resource::<SimTick>().0;
        app.step(2 * day - now + 1);
        let world = app.world();
        assert_eq!(owner_of(world, b), FactionId::INDEPENDENT);
        let revolt = world
            .resource::<Chronicle>()
            .entries()
            .iter()
            .map(|e| e.event)
            .find(|e| e.kind == SimEventKind::Revolt && e.subject == bid)
            .expect("a Revolt in the chronicle");
        assert_eq!(revolt.cause, Cause::Revolt);
        assert_eq!(revolt.faction, FactionId::PLAYER);
    }
}
