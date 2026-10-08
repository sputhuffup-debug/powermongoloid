//! Veterans (SIM-36): soldiers rank up by battles survived.
//!
//! - Every soldier and captain carries a [`Veteran`] from enlistment
//!   (`spawn_army`, `orders::enlist`); a deserter loses it.
//! - `combat::resolve_strikes` marks both the striker and the struck as
//!   having [`Veteran::fought`], and puts their armies in a battle: the
//!   captain's `battle_since` (first blow) and `last_engaged` (latest).
//!   `combat::casualties` counts the army's dead in `battle_losses`.
//! - [`end_battles`] (hourly, slot 1 of the progress driver): an army whose
//!   last blow is at least army.ron `veterans.quiet_hours` old ends its
//!   battle. Every living member who fought counts one more battle and has
//!   `fought` cleared; the army reports a `Battle` event (its faction, its
//!   captain, how many fought, its losses), and a champion in the army
//!   earns its battle points ([`crate::progression::champion::credit`]).
//! - In a run only, survivors rank up: rank = the `veterans.thresholds`
//!   that `battles x VeteranRate` reaches (never lower than before), with a
//!   `VeteranRank` event per promotion. Outside a run every rank stays 0,
//!   so the M3 skirmish and battle numbers never move (guide D6).
//! - A rank's `veterans.gains` apply where each stat is used: attack and
//!   defence in `resolve_strikes`, the morale target and the desertion
//!   chance in `upkeep::soldier_upkeep`. Rank 0 adds nothing.

use bevy_ecs::prelude::*;
use sim_agents::CombatStats;
use sim_core::{
    Calendar, FactionModifiers, Position, RunState, SimEvent, SimEventKind, SimTick,
    SpawnId, Stat, StateHash, StateHasher,
};

use crate::components::{Captain, Follower};
use crate::data::Army;
use crate::orders::{faction_name, log};
use crate::progression::Champion;
use crate::progression::champion::{Feat, credit};

/// A soldier's service (SIM-36): on every soldier and captain. Hashed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Veteran {
    /// Battles survived, having struck or been struck.
    pub battles: u16,
    /// Veteran rank: 0 (recruit) up to the number of army.ron
    /// `veterans.thresholds`.
    pub rank: u8,
    /// Struck or was struck in the army's current battle.
    pub fought: bool,
}

impl StateHash for Veteran {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u16(self.battles);
        h.u8(self.rank);
        h.bool(self.fought);
    }
}

/// SIM-36, hourly (slot 1 of `progression::progress`, every preset): ends
/// the battles of armies that have landed or taken no blow for
/// `veterans.quiet_hours`, in captain SpawnId order. Span
/// `sim_military::end_battles`. See the module doc.
pub fn end_battles(world: &mut World) {
    let _span = tracing::info_span!("sim_military::end_battles").entered();
    let tick = world.resource::<SimTick>().0;
    let data = world.resource::<Army>().data.veterans.clone();
    let quiet = world.resource::<Calendar>().hours(data.quiet_hours).max(1);
    let mut q = world.query::<(Entity, &SpawnId, &Captain)>();
    let mut ended: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, c)| {
            c.battle_since.is_some() && tick.saturating_sub(c.last_engaged) >= quiet
        })
        .map(|(e, id, _)| (*id, e))
        .collect();
    if ended.is_empty() {
        return;
    }
    ended.sort_unstable();
    let in_run = world.resource::<RunState>().in_run();
    for (id, captain) in ended {
        let (faction, losses) = {
            let mut c = world.get_mut::<Captain>(captain).expect("a captain");
            c.battle_since = None;
            (c.faction, std::mem::take(&mut c.battle_losses))
        };
        let rate = world
            .resource::<FactionModifiers>()
            .mult(faction, Stat::VeteranRate);
        let members = members(world, captain);
        let mut fought = 0u32;
        let mut promoted: Vec<(u64, u8)> = Vec::new();
        let mut champion = false;
        for &(mid, e) in &members {
            champion |= world.get::<Champion>(e).is_some();
            if !world.get::<CombatStats>(e).is_some_and(|cs| cs.alive()) {
                continue;
            }
            let Some(mut v) = world.get_mut::<Veteran>(e) else {
                continue;
            };
            if !v.fought {
                continue;
            }
            v.fought = false;
            v.battles = v.battles.saturating_add(1);
            fought += 1;
            if in_run {
                let rank = data.rank_for(v.battles, rate).max(v.rank);
                if rank > v.rank {
                    v.rank = rank;
                    promoted.push((mid.0, rank));
                }
            }
        }
        let at = world.get::<Position>(captain).map_or_else(Default::default, |p| p.0);
        world.write_message(
            SimEvent::new(SimEventKind::Battle, faction)
                .subject(id.0)
                .other(fought as u64)
                .at(at)
                .amount(losses as i32),
        );
        for &(soldier, rank) in &promoted {
            world.write_message(
                SimEvent::new(SimEventKind::VeteranRank, faction)
                    .subject(soldier)
                    .at(at)
                    .amount(rank as i32),
            );
        }
        // SIM-35: a battle the champion's own army lived through.
        if champion {
            credit(world, faction, Feat::Battle);
        }
        let who = faction_name(world, faction);
        let ranks = if promoted.is_empty() {
            String::new()
        } else {
            format!(", {} rose in rank", promoted.len())
        };
        log(
            world,
            format!("{who}: a battle is over ({fought} fought, {losses} fell{ranks})"),
        );
    }
}

/// The captain and its followers, in SpawnId order.
fn members(world: &mut World, captain: Entity) -> Vec<(SpawnId, Entity)> {
    let mut q = world.query::<(Entity, &SpawnId, &Follower)>();
    let mut out: Vec<(SpawnId, Entity)> = q
        .iter(world)
        .filter(|(_, _, f)| f.captain == captain)
        .map(|(e, id, _)| (*id, e))
        .collect();
    if let Some(id) = world.get::<SpawnId>(captain) {
        out.push((*id, captain));
    }
    out.sort_unstable();
    out
}

/// The army `person` belongs to: its own if it is a captain, else its
/// captain's.
pub(crate) fn army_of(
    person: Entity,
    captains: &Query<&mut Captain>,
    followers: &Query<&Follower>,
) -> Option<Entity> {
    if captains.contains(person) {
        Some(person)
    } else {
        followers.get(person).ok().map(|f| f.captain)
    }
}

/// The rank effects (SIM-36) of a soldier's [`Veteran`]: `None` for a
/// recruit or someone without one.
pub(crate) fn rank_gain(army: &Army, v: Option<&Veteran>) -> Option<crate::data::RankGain> {
    v.and_then(|v| army.data.veterans.gain(v.rank)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{Casualties, resolve_strikes};
    use crate::index::FighterGrid;
    use crate::progression::champion::ChampionRank;
    use crate::progression::tests::run_app;
    use crate::spawn::{ArmySpec, spawn_army};
    use bevy_ecs::system::RunSystemOnce;
    use bevy_math::IVec2;
    use sim_agents::Strike;
    use sim_core::{Chronicle, EventFeed, FactionId, SimStepper};

    /// The captain and followers of `captain`, in SpawnId order.
    fn army(world: &mut World, captain: Entity) -> Vec<(SpawnId, Entity)> {
        members(world, captain)
    }

    /// One follower of a fresh `faction` army of one, spawned at `at`.
    fn lone_soldier(world: &mut World, faction: FactionId, at: IVec2) -> (Entity, Entity) {
        let cap = spawn_army(
            world,
            &ArmySpec {
                armed: true,
                ..ArmySpec::new(faction, at, 1)
            },
        );
        let man = army(world, cap)
            .into_iter()
            .map(|(_, e)| e)
            .find(|e| *e != cap)
            .expect("a follower");
        (cap, man)
    }

    /// SIM-36: a blow marks the striker and the struck as having fought and
    /// puts both armies in battle; a ranked striker adds its rank's attack
    /// and a ranked victim its defence, exactly; a kill names its faction.
    #[test]
    fn a_blow_marks_both_sides_and_ranks_change_the_damage() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        let start = world.resource::<crate::RunProgress>().start;
        let (cap_a, a) = lone_soldier(world, FactionId::PLAYER, start + IVec2::new(30, 0));
        let (cap_b, b) = lone_soldier(world, FactionId::RAIDERS, start + IVec2::new(-30, 0));
        let gains = world.resource::<Army>().data.veterans.gains.clone();
        let elite = gains.len() as u8;
        let g = gains[elite as usize - 1];
        let id_a = *world.get::<SpawnId>(a).unwrap();
        // One blow from a at b, with the given ranks; returns the damage.
        let blow = |world: &mut World, rank_a: u8, rank_b: u8, health: f32| -> f32 {
            world.get_mut::<Veteran>(a).unwrap().rank = rank_a;
            world.get_mut::<Veteran>(b).unwrap().rank = rank_b;
            world.get_mut::<CombatStats>(b).unwrap().health = health;
            *world.get_mut::<Strike>(a).unwrap() = Strike {
                target: Some(b),
                swing: true,
                bonus: 1.0,
                ..Default::default()
            };
            world.resource_mut::<FighterGrid>().fighters = vec![(id_a, a)];
            world.run_system_once(resolve_strikes).unwrap();
            health - world.get::<CombatStats>(b).unwrap().health
        };
        let d0 = blow(world, 0, 0, 100.0);
        for e in [a, b] {
            assert!(world.get::<Veteran>(e).unwrap().fought, "both fought");
        }
        for c in [cap_a, cap_b] {
            let c = world.get::<Captain>(c).unwrap();
            assert_eq!((c.battle_since, c.last_engaged), (Some(0), 0), "in battle");
        }
        let cs_a = *world.get::<CombatStats>(a).unwrap();
        let cs_b = *world.get::<CombatStats>(b).unwrap();
        let weapon = world.resource::<Army>().data.equipment.weapon_attack;
        let attack = cs_a.attack + weapon;
        let roll = (d0 + cs_b.defense) / attack;
        let d_attack = blow(world, elite, 0, 100.0);
        let d_defence = blow(world, 0, elite, 100.0);
        println!("damage {d0:.3}, elite striker {d_attack:.3}, elite victim {d_defence:.3}");
        assert!((d_attack - d0 - g.attack * roll).abs() < 1e-3, "attack gain");
        assert!((d0 - d_defence - g.defense).abs() < 1e-3, "defence gain");
        assert!(world.resource::<Casualties>().slain_by.is_empty());
        blow(world, 0, 0, 1.0);
        assert_eq!(
            world.resource::<Casualties>().slain_by,
            vec![(b, FactionId::PLAYER)]
        );
    }

    /// SIM-36, SIM-35: an hour after its last blow an army's battle ends.
    /// Those who fought count a battle and rank up by army.ron's
    /// thresholds (in a run), the army reports a `Battle` with its losses,
    /// and the champion's army earns its battle points. Outside a run the
    /// battles count but no one ranks up.
    #[test]
    fn a_battle_ends_after_a_quiet_hour_and_ranks_its_fighters() {
        for run in [true, false] {
            let (mut app, start) = run_app(42);
            let world = app.world_mut();
            if !run {
                world.resource_mut::<RunState>().config = None;
            }
            let data = world.resource::<Army>().data.veterans.clone();
            let champion = start.champion;
            let men = army(world, champion);
            let (twice, once, idle) = (men[1].1, men[2].1, men[3].1);
            {
                let mut c = world.get_mut::<Captain>(champion).unwrap();
                c.battle_since = Some(0);
                c.last_engaged = 0;
                c.battle_losses = 2;
            }
            world.get_mut::<Veteran>(champion).unwrap().fought = true;
            *world.get_mut::<Veteran>(twice).unwrap() = Veteran {
                battles: data.thresholds[1] - 1,
                rank: 1,
                fought: true,
            };
            world.get_mut::<Veteran>(once).unwrap().fought = true;
            let points = world.get::<ChampionRank>(champion).unwrap().points;
            app.step(100);
            let world = app.world_mut();
            assert!(
                world.get::<Captain>(champion).unwrap().battle_since.is_some(),
                "not over before an hour"
            );
            app.step(1);
            let world = app.world_mut();
            let c = world.get::<Captain>(champion).unwrap();
            assert_eq!((c.battle_since, c.battle_losses), (None, 0));
            let v = |w: &World, e| *w.get::<Veteran>(e).unwrap();
            let want = |battles: u16, rank: u8| Veteran {
                battles,
                rank: if run { rank } else { 0 },
                fought: false,
            };
            assert_eq!(v(world, once), want(1, 1));
            assert_eq!(v(world, champion), want(1, 1));
            let twice_rank = if run { 2 } else { 1 };
            assert_eq!(
                v(world, twice),
                Veteran {
                    battles: data.thresholds[1],
                    rank: twice_rank,
                    fought: false
                }
            );
            assert_eq!(v(world, idle), Veteran::default(), "never fought");
            let champion_id = world.get::<SpawnId>(champion).unwrap().0;
            let battle = world
                .resource::<Chronicle>()
                .entries()
                .iter()
                .find(|e| e.event.kind == SimEventKind::Battle)
                .copied()
                .expect("a Battle entry");
            assert_eq!(
                (
                    battle.tick,
                    battle.event.faction,
                    battle.event.subject,
                    battle.event.other,
                    battle.event.amount
                ),
                (100, FactionId::PLAYER, champion_id, 3, 2)
            );
            let ranked = world
                .resource::<EventFeed>()
                .iter()
                .filter(|e| e.event.kind == SimEventKind::VeteranRank)
                .count();
            assert_eq!(ranked, if run { 3 } else { 0 });
            let battle_points = world
                .resource::<crate::Progression>()
                .data
                .champion
                .points
                .battle;
            let now = world.get::<ChampionRank>(champion).unwrap().points;
            assert_eq!(now, points + if run { battle_points } else { 0 });
        }
    }

    /// SIM-36: ranks follow the thresholds, faster with a VeteranRate
    /// bonus, and each rank's gain is its own total.
    #[test]
    fn ranks_follow_battles_and_the_veteran_rate() {
        let d = crate::data::VeteranData {
            quiet_hours: 1.0,
            thresholds: vec![1, 3, 6],
            names: ["Recruit", "Soldier", "Veteran", "Elite"].map(String::from).to_vec(),
            gains: vec![
                crate::data::RankGain {
                    attack: 1.0,
                    defense: 0.5,
                    morale: 0.03,
                    desertion: 0.1,
                };
                3
            ],
        };
        assert!(d.validate().is_ok());
        let ranks: Vec<u8> = (0..8).map(|b| d.rank_for(b, 1.0)).collect();
        assert_eq!(ranks, vec![0, 1, 1, 2, 2, 2, 3, 3]);
        // "Hardened": 25% faster. 5 battles count as 6.25.
        let ranks: Vec<u8> = (0..8).map(|b| d.rank_for(b, 1.25)).collect();
        assert_eq!(ranks, vec![0, 1, 1, 2, 2, 3, 3, 3]);
        assert!(d.gain(0).is_none());
        assert_eq!(d.gain(3).map(|g| g.attack), Some(1.0));
        assert_eq!((d.name(0), d.name(3), d.name(9)), ("Recruit", "Elite", "?"));
        for bad in [
            crate::data::VeteranData {
                thresholds: vec![0, 3, 6],
                ..d.clone()
            },
            crate::data::VeteranData {
                thresholds: vec![1, 3, 3],
                ..d.clone()
            },
            crate::data::VeteranData {
                names: vec!["Recruit".into()],
                ..d.clone()
            },
            crate::data::VeteranData {
                quiet_hours: 0.0,
                ..d.clone()
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
        let mut bad = d.clone();
        bad.gains[0].desertion = 1.5;
        assert!(bad.validate().is_err());
    }
}
