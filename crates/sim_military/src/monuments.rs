//! Monuments (GP-23): who holds them, who guards them, and how they change
//! hands. Their bonuses are folded into `FactionModifiers` by
//! [`crate::progression::rebuild_modifiers`]. Design: ADR 0015.
//!
//! - **Setup** ([`setup`], from `setup_run`, before the first tick): every
//!   monument gets a [`Holder`]. Each one, in SpawnId order, draws its guard
//!   from progression.ron `monuments.guards` (stream
//!   `sim_military::monuments::guards ^ monument` at tick 0): a raider camp
//!   (`FactionId::RAIDERS`, without the `Raider` AI) or a beast lair
//!   (`FactionId::BEASTS`), or none. A guard is an ordinary army
//!   ([`spawn_army`]) of `base + per_tile x distance from the start`
//!   followers (at most `max`), so camps far from the start are stronger.
//!   Every member carries [`Guard`] and [`NoUpkeep`]; the guard faction
//!   holds the monument. A guard captain has no orders: it holds its tile
//!   and its men fight anything at war that comes within `engage_radius`.
//! - **Holding** ([`hold`], hourly, slot 2 of the progress driver):
//!   monuments in SpawnId order. Soldiers (not militia, not routed) within
//!   `monuments.hold_radius` are counted by faction from this tick's
//!   [`FighterGrid`]. One faction alone there that isn't the holder becomes
//!   the contender; after `monuments.hold_hours` alone it takes the
//!   monument (`MonumentTaken`, and `MonumentLost` for a previous holder,
//!   cause `Cause::Hold`). A holder whose men have not stood there for
//!   `monuments.drift_days`, and that owns no settlement within
//!   `monuments.drift_distance`, loses it to nobody (`MonumentLost`).

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use rand::RngExt;
use sim_agents::Owner;
use sim_core::rng::rng_key;
use sim_core::{
    Calendar, Cause, FactionId, NameTable, Position, SimEvent, SimEventKind, SimSeed, SimTick,
    SpawnId, StateHash, StateHasher,
};
use sim_world::{Monument, Settlement};

use crate::components::{Captain, Follower};
use crate::index::FighterGrid;
use crate::orders::{faction_name, log};
use crate::progression::{GuardData, Progression};
use crate::spawn::{ArmySpec, spawn_army};

/// Who holds a monument (GP-23). On every monument in a run, from setup.
/// Hashed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Holder {
    /// The holding faction; `FactionId::INDEPENDENT` for nobody.
    pub faction: FactionId,
    /// Tick the holder took it (0 at setup).
    pub since: u64,
    /// A faction standing there alone, not the holder, and the hourly
    /// sample since which it has.
    pub contender: Option<(FactionId, u64)>,
    /// The last hourly sample with the holder's soldiers at the monument
    /// (drift is measured from it).
    pub last_presence: u64,
}

impl Default for Holder {
    /// Held by nobody since tick 0.
    fn default() -> Self {
        Self {
            faction: FactionId::INDEPENDENT,
            since: 0,
            contender: None,
            last_presence: 0,
        }
    }
}

impl StateHash for Holder {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u16(self.faction.0);
        h.u64(self.since);
        match self.contender {
            Some((f, t)) => {
                h.u8(1);
                h.u16(f.0);
                h.u64(t);
            }
            None => h.u8(0),
        }
        h.u64(self.last_presence);
    }
}

/// A member of the guard camp of `monument` (a SpawnId), captain and
/// followers alike, so the camp stays a camp when its captain falls and a
/// follower takes command. Hashed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guard {
    /// The guarded monument's SpawnId.
    pub monument: u64,
}

impl StateHash for Guard {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.monument);
    }
}

/// A soldier who never gets hungry (monument guards): `soldier_upkeep`
/// leaves its hunger alone and it never eats. Hashed (a marker).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoUpkeep;

impl StateHash for NoUpkeep {
    fn hash_state(&self, _h: &mut StateHasher) {}
}

/// The guard `u` (a uniform draw in 0..1) picks: the entries' chances are
/// laid end to end; past their sum, none.
pub(crate) fn pick_guard(guards: &[GuardData], u: f32) -> Option<&GuardData> {
    let mut acc = 0.0;
    guards.iter().find(|g| {
        acc += g.chance;
        u < acc
    })
}

/// Followers of guard `g` at `distance` tiles from the start.
pub(crate) fn guard_size(g: &GuardData, distance: f32) -> u32 {
    ((g.base as f32 + g.per_tile * distance.max(0.0)).round() as u32).min(g.max)
}

/// GP-23, run setup: gives every monument a [`Holder`] and spawns the guard
/// camps (see the module docs). `start` is the band's start spot. Edits
/// state directly, before the first tick.
pub fn setup(world: &mut World, start: IVec2) {
    let _span = tracing::info_span!("sim_military::monuments::setup").entered();
    let guards = world
        .resource::<Progression>()
        .data
        .monuments
        .guards
        .clone();
    let seed = *world.resource::<SimSeed>();
    let mut q = world.query::<(Entity, &SpawnId, &Position, &Monument)>();
    let mut monuments: Vec<(SpawnId, Entity, IVec2)> =
        q.iter(world).map(|(e, id, p, _)| (*id, e, p.0)).collect();
    monuments.sort_unstable_by_key(|m| m.0);
    for (id, e, at) in monuments {
        let mut rng = seed.stream(rng_key("sim_military::monuments::guards") ^ id.0, 0);
        let guard = pick_guard(&guards, rng.random::<f32>()).copied();
        let faction = guard.map_or(FactionId::INDEPENDENT, |g| g.kind.faction());
        world.entity_mut(e).insert(Holder {
            faction,
            ..Holder::default()
        });
        let Some(g) = guard else {
            continue;
        };
        let followers = guard_size(&g, (at - start).as_vec2().length());
        let captain = spawn_army(
            world,
            &ArmySpec {
                food_days: 0.0,
                armed: g.armed,
                ..ArmySpec::new(faction, at, followers)
            },
        );
        let mut q = world.query::<(Entity, &Follower)>();
        let mut members: Vec<Entity> = q
            .iter(world)
            .filter(|(_, f)| f.captain == captain)
            .map(|(m, _)| m)
            .collect();
        members.push(captain);
        for m in members {
            world
                .entity_mut(m)
                .insert((Guard { monument: id.0 }, NoUpkeep));
        }
    }
}

/// Soldiers (not militia, not routed) within `radius` of `at`, counted by
/// faction (never faction 0), sorted by faction. Reads this tick's
/// [`FighterGrid`].
pub(crate) fn soldiers_at(grid: &FighterGrid, at: IVec2, radius: f32) -> Vec<(FactionId, u32)> {
    let mut present: Vec<(FactionId, u32)> = Vec::new();
    let centre = at.as_vec2() + bevy_math::Vec2::splat(0.5);
    grid.for_each_within(centre, radius, |e, _| {
        if e.routed || e.side.against.is_some() || e.side.faction == FactionId::INDEPENDENT {
            return;
        }
        match present.iter_mut().find(|(f, _)| *f == e.side.faction) {
            Some((_, n)) => *n += 1,
            None => present.push((e.side.faction, 1)),
        }
    });
    present.sort_unstable();
    present
}

fn name_of(world: &World, m: &Monument) -> String {
    world.resource::<NameTable>().get(m.name).to_string()
}

/// Whether `faction` owns no settlement within `distance` tiles of `at`.
fn far_from_lands(world: &mut World, at: IVec2, faction: FactionId, distance: f32) -> bool {
    let mut q = world.query::<(&Settlement, &Owner)>();
    !q.iter(world)
        .any(|(s, o)| o.faction == faction && (s.center - at).as_vec2().length() <= distance)
}

/// GP-23: the progress driver's monument step (slot 2), hourly, in every
/// preset (outside a run there are no monuments). Monuments in SpawnId
/// order; see the module docs. Span `sim_military::monuments::hold`.
pub fn hold(world: &mut World) {
    let _span = tracing::info_span!("sim_military::monuments::hold").entered();
    let mut q = world.query::<(Entity, &SpawnId, &Position, &Monument, &Holder)>();
    let mut monuments: Vec<(SpawnId, Entity, IVec2, Monument, Holder)> = q
        .iter(world)
        .map(|(e, id, p, m, h)| (*id, e, p.0, *m, *h))
        .collect();
    if monuments.is_empty() {
        return;
    }
    monuments.sort_unstable_by_key(|m| m.0);
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    let data = world.resource::<Progression>().data.monuments.clone();
    let hold_ticks = cal.hours(data.hold_hours);
    let drift_ticks = cal.days(data.drift_days);
    for (id, e, at, m, before) in monuments {
        let present = soldiers_at(world.resource::<FighterGrid>(), at, data.hold_radius);
        let mut h = before;
        if h.faction != FactionId::INDEPENDENT && present.iter().any(|p| p.0 == h.faction) {
            h.last_presence = tick;
        }
        match present.as_slice() {
            &[(f, _)] if f != h.faction => {
                let since = match h.contender {
                    Some((c, s)) if c == f => s,
                    _ => tick,
                };
                h.contender = Some((f, since));
                if tick - since >= hold_ticks {
                    let previous = h.faction;
                    h = Holder {
                        faction: f,
                        since: tick,
                        contender: None,
                        last_presence: tick,
                    };
                    changed_hands(world, id, at, &m, f, previous, Cause::Hold);
                }
            }
            _ => h.contender = None,
        }
        if h.faction != FactionId::INDEPENDENT
            && tick.saturating_sub(h.last_presence) >= drift_ticks
            && far_from_lands(world, at, h.faction, data.drift_distance)
        {
            let previous = h.faction;
            h = Holder {
                faction: FactionId::INDEPENDENT,
                since: tick,
                contender: None,
                last_presence: tick,
            };
            changed_hands(
                world,
                id,
                at,
                &m,
                FactionId::INDEPENDENT,
                previous,
                Cause::None,
            );
        }
        if h != before {
            *world.get_mut::<Holder>(e).expect("a holder") = h;
        }
    }
}

/// Reports monument `id` passing from `previous` to `to`: `MonumentTaken`
/// for `to` (unless nobody) and `MonumentLost` for `previous` (unless
/// nobody), each with `other` = the other faction, `at` = the monument and
/// `amount` = its kind; plus a log line.
fn changed_hands(
    world: &mut World,
    id: SpawnId,
    at: IVec2,
    m: &Monument,
    to: FactionId,
    previous: FactionId,
    cause: Cause,
) {
    let event = |kind, faction: FactionId, other: FactionId| {
        SimEvent::new(kind, faction)
            .subject(id.0)
            .other(other.0 as u64)
            .at(at)
            .amount(m.kind as i32)
            .cause(cause)
    };
    if to != FactionId::INDEPENDENT {
        world.write_message(event(SimEventKind::MonumentTaken, to, previous));
        // SIM-35: a monument taken counts toward the champion's rank.
        crate::progression::champion::credit(world, to, crate::progression::Feat::Monument);
    }
    if previous != FactionId::INDEPENDENT {
        world.write_message(event(SimEventKind::MonumentLost, previous, to));
    }
    let name = name_of(world, m);
    let text = if to == FactionId::INDEPENDENT {
        format!("{name} is held by nobody now")
    } else {
        format!("{} took {name}", faction_name(world, to))
    };
    log(world, text);
}

/// Monuments (SpawnId, centre) whose guard camp still has a captain and
/// still holds the monument, in SpawnId order: the camps a ClearCamp
/// request can name. A camp whose men routed while someone else held the
/// monument for `hold_hours` has lost it, wherever its captain rallied (a
/// guard captain has no orders and never walks back).
pub(crate) fn guarded(world: &mut World) -> Vec<(u64, IVec2)> {
    let mut q = world.query::<(&Guard, &Captain)>();
    let mut camps: Vec<(u64, FactionId)> = q
        .iter(world)
        .map(|(g, c)| (g.monument, c.faction))
        .collect();
    camps.sort_unstable();
    camps.dedup();
    if camps.is_empty() {
        return Vec::new();
    }
    let mut q = world.query::<(&SpawnId, &Position, &Holder)>();
    let mut out: Vec<(u64, IVec2)> = q
        .iter(world)
        .filter(|(id, _, h)| camps.binary_search(&(id.0, h.faction)).is_ok())
        .map(|(id, p, _)| (id.0, p.0))
        .collect();
    out.sort_unstable_by_key(|m| m.0);
    out
}

/// The tile of monument `id` (a SpawnId), if it exists.
pub(crate) fn centre_of(world: &mut World, id: u64) -> Option<IVec2> {
    let mut q = world.query_filtered::<(&SpawnId, &Position), With<Monument>>();
    q.iter(world).find(|(s, _)| s.0 == id).map(|(_, p)| p.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progression::tests::{command, run_app};
    use crate::requests::{RequestBoard, RequestKind};
    use bevy_app::App;
    use sim_agents::{CombatStats, Needs};
    use sim_core::{
        Chronicle, CommandKind, FactionModifiers, ModifierSource, OrderSpec, SimStepper,
    };

    /// (SpawnId, entity, tile, monument, holder) of every monument, in
    /// SpawnId order.
    fn monuments(world: &mut World) -> Vec<(u64, Entity, IVec2, Monument, Holder)> {
        let mut q = world.query::<(Entity, &SpawnId, &Position, &Monument, &Holder)>();
        let mut v: Vec<_> = q
            .iter(world)
            .map(|(e, id, p, m, h)| (id.0, e, p.0, *m, *h))
            .collect();
        v.sort_unstable_by_key(|m| m.0);
        v
    }

    fn holder(app: &App, e: Entity) -> Holder {
        *app.world().get::<Holder>(e).unwrap()
    }

    /// Members of `monument`'s guard camp, in SpawnId order.
    fn guards(world: &mut World, monument: u64) -> Vec<(SpawnId, Entity)> {
        let mut q = world.query::<(Entity, &SpawnId, &Guard)>();
        let mut v: Vec<_> = q
            .iter(world)
            .filter(|(.., g)| g.monument == monument)
            .map(|(e, id, _)| (*id, e))
            .collect();
        v.sort_unstable();
        v
    }

    /// GP-23: setup gives every monument a holder and guards some of them:
    /// the guard faction holds a guarded monument, every member is marked
    /// `Guard` and `NoUpkeep`, the camp's size follows its distance from the
    /// start, and the guards stand their ground without growing hungry.
    #[test]
    fn guard_camps_hold_their_ground_and_never_hunger() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        let data = world.resource::<Progression>().data.monuments.clone();
        let start = world.resource::<crate::RunProgress>().start;
        let all = monuments(world);
        assert!(!all.is_empty());
        let mut camps = 0;
        let mut watched = Vec::new();
        for (id, _, at, _, h) in &all {
            let members = guards(world, *id);
            if h.faction == FactionId::INDEPENDENT {
                assert!(members.is_empty(), "monument {id} is unguarded");
                continue;
            }
            camps += 1;
            let g = data
                .guards
                .iter()
                .find(|g| g.kind.faction() == h.faction)
                .expect("a guard kind of the holder's faction");
            let want = guard_size(g, (*at - start).as_vec2().length()) as usize + 1;
            assert_eq!(members.len(), want, "monument {id}: captain and followers");
            let mut captains = 0;
            for &(_, e) in &members {
                assert!(world.get::<NoUpkeep>(e).is_some());
                assert_eq!(world.get::<sim_core::Allegiance>(e).unwrap().0, h.faction);
                if let Some(c) = world.get::<Captain>(e) {
                    captains += 1;
                    assert!(c.orders.is_empty());
                    watched.push((e, *at, world.get::<Needs>(e).unwrap().hunger));
                }
            }
            assert_eq!(captains, 1, "monument {id}: one captain");
        }
        assert!(
            camps > 0 && camps < all.len(),
            "seed 42 guards some: {camps}"
        );
        app.step(600);
        for (e, at, hunger) in watched {
            let world = app.world();
            assert_eq!(world.get::<Needs>(e).unwrap().hunger, hunger, "no hunger");
            let p = world.get::<Position>(e).unwrap().0;
            assert!(
                (p - at).as_vec2().length() <= 2.0,
                "the camp stays at {at}: {p}"
            );
        }
        let world = app.world_mut();
        for (_, e, _, _, h) in monuments(world) {
            assert_eq!(
                *world.get::<Holder>(e).unwrap(),
                h,
                "guards keep their monuments"
            );
        }
    }

    /// GP-23: one faction's soldiers alone at an unguarded monument for
    /// `hold_hours` take it (MonumentTaken, cause Hold), and the hourly
    /// rebuild adds its bonus to the faction's modifiers. Left alone far
    /// from the faction's lands for `drift_days`, it falls back to nobody
    /// (MonumentLost) and the bonus goes.
    #[test]
    fn an_unguarded_monument_is_taken_by_holding_it_and_drifts_back() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        {
            let mut p = world.resource_mut::<Progression>();
            p.data.monuments.drift_days = 0.25;
        }
        let data = world.resource::<Progression>().data.monuments.clone();
        let cal = world.resource::<Calendar>().clone();
        let (id, e, at, m, _) = monuments(world)
            .into_iter()
            .find(|m| m.4.faction == FactionId::INDEPENDENT)
            .expect("an unguarded monument");
        let captain = spawn_army(world, &ArmySpec::new(FactionId::PLAYER, at, 4));
        let cid = world.get::<SpawnId>(captain).unwrap().0;
        let kind_bonus = |app: &App| {
            app.world()
                .resource::<FactionModifiers>()
                .breakdown
                .iter()
                .find(|b| {
                    b.faction == Some(FactionId::PLAYER)
                        && b.source == ModifierSource::Monuments { kind: m.kind as u8 }
                })
                .map(|b| b.value)
        };
        let hold = cal.hours(data.hold_hours);
        app.step(hold);
        assert_eq!(holder(&app, e).faction, FactionId::INDEPENDENT, "not yet");
        assert_eq!(holder(&app, e).contender, Some((FactionId::PLAYER, 0)));
        assert_eq!(kind_bonus(&app), None);
        app.step(1);
        let h = holder(&app, e);
        assert_eq!((h.faction, h.since), (FactionId::PLAYER, hold));
        let taken: Vec<_> = app
            .world()
            .resource::<Chronicle>()
            .entries()
            .iter()
            .filter(|c| c.event.kind == SimEventKind::MonumentTaken)
            .map(|c| (c.tick, c.event.faction, c.event.subject, c.event.cause))
            .collect();
        assert_eq!(taken, vec![(hold, FactionId::PLAYER, id, Cause::Hold)]);
        let b = data.bonus(m.kind).unwrap();
        assert_eq!(kind_bonus(&app), Some(b.bonus.min(b.cap)));

        // March off, far from anything the player owns (nothing): it
        // drifts back to nobody `drift_days` after the last hour the men
        // stood there.
        let away = at + IVec2::new(0, 40);
        let away = crate::spawn::nearest_passable(
            app.world().resource::<sim_world::WorldMap>(),
            &app.world().resource::<sim_agents::Villagers>().travel_costs,
            away,
        )
        .unwrap();
        command(
            &mut app,
            FactionId::PLAYER,
            CommandKind::IssueOrder {
                captain: cid,
                order: OrderSpec::MoveTo {
                    x: away.x,
                    y: away.y,
                },
                queue: false,
            },
        );
        let drift = cal.days(data.drift_days);
        let mut lost_at = None;
        for _ in 0..(drift / 100 + 4) {
            app.step(100);
            if holder(&app, e).faction == FactionId::INDEPENDENT {
                lost_at = Some(holder(&app, e).since);
                break;
            }
        }
        let lost_at = lost_at.expect("the monument drifted back");
        let h = holder(&app, e);
        assert!(
            lost_at >= h.last_presence && lost_at - hold >= drift,
            "{h:?}"
        );
        let lost: Vec<_> = app
            .world()
            .resource::<Chronicle>()
            .entries()
            .iter()
            .filter(|c| c.event.kind == SimEventKind::MonumentLost)
            .map(|c| (c.tick, c.event.faction, c.event.subject, c.event.other))
            .collect();
        assert_eq!(lost, vec![(lost_at, FactionId::PLAYER, id, 0)]);
        assert_eq!(kind_bonus(&app), None, "the bonus went");
    }

    /// GP-22, GP-23: a camp that has lost its monument (its men routed
    /// while someone held it for `hold_hours`) is no longer a camp to
    /// clear, though its captain lives: no request may name it, and an
    /// open one is answered by whoever stands at the monument.
    #[test]
    fn a_camp_that_lost_its_monument_is_no_camp_to_clear() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        let before = guarded(world);
        let (id, e, ..) = monuments(world)
            .into_iter()
            .find(|m| m.4.faction != FactionId::INDEPENDENT)
            .expect("a guarded monument");
        assert!(before.iter().any(|g| g.0 == id), "{before:?}");
        world.get_mut::<Holder>(e).unwrap().faction = FactionId::PLAYER;
        let after = guarded(world);
        assert!(!after.iter().any(|g| g.0 == id), "{after:?}");
        assert_eq!(after.len() + 1, before.len());
        assert!(!guards(world, id).is_empty(), "its men still stand");
    }

    /// GP-22, GP-23: a settlement near a guard camp asks for it to be
    /// cleared; once the camp has no captain left and a player captain is
    /// at the monument, the request is done, and the player's soldiers,
    /// alone there, then take the monument.
    #[test]
    fn a_cleared_camp_answers_its_request_and_frees_the_monument() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        world
            .resource_mut::<Progression>()
            .data
            .requests
            .camp_radius = 1000.0;
        let (id, e, at, ..) = monuments(world)
            .into_iter()
            .find(|m| m.4.faction != FactionId::INDEPENDENT)
            .expect("a guarded monument");
        app.step(1);
        let posted = app
            .world()
            .resource::<RequestBoard>()
            .camp(id)
            .cloned()
            .expect("a ClearCamp request");
        assert_eq!(posted.kind, RequestKind::ClearCamp { monument: id });
        let world = app.world_mut();
        // The guards fall (a test edit between ticks); the player's band
        // stands at the monument.
        for (_, g) in guards(world, id) {
            world.get_mut::<CombatStats>(g).unwrap().health = 0.0;
        }
        spawn_army(world, &ArmySpec::new(FactionId::PLAYER, at, 4));
        app.step(100);
        let board = app.world().resource::<RequestBoard>();
        assert!(board.camp(id).is_none(), "the request closed");
        assert_eq!(board.done, 1);
        assert!(guards(app.world_mut(), id).is_empty(), "the camp is gone");
        let world = app.world_mut();
        let cal = world.resource::<Calendar>().clone();
        let hold = cal.hours(world.resource::<Progression>().data.monuments.hold_hours);
        app.step(hold + 1);
        assert_eq!(holder(&app, e).faction, FactionId::PLAYER);
    }
}
