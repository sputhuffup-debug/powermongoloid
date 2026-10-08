//! Who holds each settlement and how it regards each faction (GP-05, GP-06,
//! GP-22; ADR 0014).
//!
//! - [`Owner`]: the faction holding a settlement, and since when.
//! - [`Goodwill`]: the settlement's standing with each faction, in -1..1.
//!   It stays at zero until M4 step 3's ledger fills it; goodwill toward
//!   the owner is the "treatment" term of loyalty ([`crate::loyalty`]).
//! - [`set_owner`]: the one way a settlement changes hands. sim_military
//!   calls it for conquest, invitation and revolt.
//!
//! Both components sit on every populated settlement from the start (all
//! presets), next to `SettlementEconomy`, so the state hash covers them
//! from tick 0.

use bevy_ecs::prelude::*;
use sim_core::{Allegiance, FactionId, SimTick, SpawnId, StateHash, StateHasher};

use crate::components::{Enlisted, Threat, Villager};
use crate::data::Villagers;
use crate::loyalty::{Loyalty, SettlementLoyalty};

/// Who holds a settlement (GP-05). On every populated settlement;
/// `FactionId::INDEPENDENT` (0) for a free one. Villagers' [`Allegiance`]
/// follows it, through [`set_owner`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner {
    /// The holder.
    pub faction: FactionId,
    /// The tick the holder took the settlement (0 at the world's start).
    pub since: u64,
}

impl Default for Owner {
    fn default() -> Self {
        Self {
            faction: FactionId::INDEPENDENT,
            since: 0,
        }
    }
}

impl StateHash for Owner {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u16(self.faction.0);
        h.u64(self.since);
    }
}

/// A settlement's standing with each faction (GP-22), each in -1..1, 0 for
/// a faction it has never dealt with. Kept sorted by faction, so it hashes
/// and iterates in a fixed order. On every populated settlement.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct Goodwill {
    /// (faction, goodwill), sorted by faction, one entry per faction.
    by_faction: Vec<(FactionId, f32)>,
}

impl Goodwill {
    /// Goodwill toward `faction` (0.0 if none recorded).
    pub fn get(&self, faction: FactionId) -> f32 {
        self.by_faction
            .binary_search_by_key(&faction, |(f, _)| *f)
            .map_or(0.0, |i| self.by_faction[i].1)
    }

    /// Adds `delta` to the goodwill toward `faction`, clamped to -1..1, and
    /// returns the new value. A zero or non-finite delta changes nothing.
    pub fn add(&mut self, faction: FactionId, delta: f32) -> f32 {
        if delta == 0.0 || !delta.is_finite() {
            return self.get(faction);
        }
        let v = (self.get(faction) + delta).clamp(-1.0, 1.0);
        self.set(faction, v);
        v
    }

    /// Sets the goodwill toward `faction`, clamped to -1..1 (a non-finite
    /// value is ignored).
    pub fn set(&mut self, faction: FactionId, value: f32) {
        if !value.is_finite() {
            return;
        }
        let value = value.clamp(-1.0, 1.0);
        match self.by_faction.binary_search_by_key(&faction, |(f, _)| *f) {
            Ok(i) => self.by_faction[i].1 = value,
            Err(i) => self.by_faction.insert(i, (faction, value)),
        }
    }

    /// Every recorded (faction, goodwill), sorted by faction.
    pub fn entries(&self) -> &[(FactionId, f32)] {
        &self.by_faction
    }
}

impl StateHash for Goodwill {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.by_faction.len() as u32);
        for (f, g) in &self.by_faction {
            h.u16(f.0);
            h.f32(*g);
        }
    }
}

/// What [`set_owner`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerChange {
    /// The settlement's owner before the change.
    pub previous: FactionId,
    /// Residents (not enlisted) whose allegiance and loyalty were set.
    pub residents: u32,
}

/// The loyalty a settlement's residents start with under `to` when the
/// caller gives none: `(goodwill(to) + 1) / 2`, or `loyalty.independent_target`
/// for `FactionId::INDEPENDENT`.
pub fn default_seed(world: &World, settlement: Entity, to: FactionId) -> f32 {
    if to == FactionId::INDEPENDENT {
        return world
            .resource::<Villagers>()
            .data
            .loyalty
            .independent_target;
    }
    let g = world.get::<Goodwill>(settlement).map_or(0.0, |g| g.get(to));
    (g + 1.0) / 2.0
}

/// Hands `settlement` to `to` (GP-05, GP-06): the one way a settlement
/// changes owner. Call it from serial steps only (conquest, invitation,
/// revolt, run setup).
///
/// - Sets [`Owner`] to `to`, since the current tick.
/// - Gives every resident villager who is not [`Enlisted`], in SpawnId
///   order, `Allegiance(to)` and a [`Loyalty`] of `seed_loyalty` (clamped
///   to 0..1), or [`default_seed`] when it is `None`.
/// - Clears the settlement's provocation (`Threat::provoked_by` and
///   `provoked_until`): its militia stands down.
/// - Restarts [`SettlementLoyalty`]: mean and min become the seed,
///   `below_since` and `revolt_since` are cleared.
///
/// Returns `None`, changing nothing, if `settlement` isn't a populated
/// settlement (it has no `Owner`). Emitting events is the caller's job.
pub fn set_owner(
    world: &mut World,
    settlement: Entity,
    to: FactionId,
    seed_loyalty: Option<f32>,
) -> Option<OwnerChange> {
    let _span = tracing::info_span!("sim_agents::set_owner").entered();
    let previous = world.get::<Owner>(settlement)?.faction;
    let tick = world.resource::<SimTick>().0;
    let seed = seed_loyalty
        .filter(|v| v.is_finite())
        .unwrap_or_else(|| default_seed(world, settlement, to))
        .clamp(0.0, 1.0);

    let mut residents: Vec<(SpawnId, Entity)> = world
        .query_filtered::<(Entity, &SpawnId, &Villager), Without<Enlisted>>()
        .iter(world)
        .filter(|(_, _, v)| v.settlement == settlement)
        .map(|(e, id, _)| (*id, e))
        .collect();
    residents.sort_unstable();
    for (_, e) in &residents {
        let mut person = world.entity_mut(*e);
        if let Some(mut a) = person.get_mut::<Allegiance>() {
            a.0 = to;
        }
        if let Some(mut l) = person.get_mut::<Loyalty>() {
            l.0 = seed;
        }
    }

    let n = residents.len() as u32;
    let mut s = world.entity_mut(settlement);
    s.insert(Owner {
        faction: to,
        since: tick,
    });
    if let Some(mut threat) = s.get_mut::<Threat>() {
        threat.provoked_by = None;
        threat.provoked_until = 0;
    }
    if let Some(mut sl) = s.get_mut::<SettlementLoyalty>() {
        let level = if n > 0 { seed } else { 0.0 };
        *sl = SettlementLoyalty {
            mean: level,
            min: level,
            residents: n,
            below_since: None,
            revolt_since: None,
        };
    }
    Some(OwnerChange {
        previous,
        residents: n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::Calendar;

    #[test]
    fn goodwill_stays_sorted_and_clamped() {
        let mut g = Goodwill::default();
        assert_eq!(g.get(FactionId::PLAYER), 0.0);
        assert_eq!(g.add(FactionId::RIVAL, 0.4), 0.4);
        assert_eq!(g.add(FactionId::PLAYER, -0.7), -0.7);
        assert_eq!(g.add(FactionId::PLAYER, -0.7), -1.0, "clamped at -1");
        assert_eq!(g.add(FactionId::RAIDERS, 0.0), 0.0);
        assert_eq!(g.add(FactionId::RAIDERS, f32::NAN), 0.0);
        g.set(FactionId::BEASTS, 3.0);
        assert_eq!(
            g.entries(),
            &[
                (FactionId::PLAYER, -1.0),
                (FactionId::RIVAL, 0.4),
                (FactionId::BEASTS, 1.0)
            ],
            "sorted by faction; zero deltas add no entry"
        );
    }

    /// A settlement with residents `ids` (SpawnIds), spawned in the given
    /// entity order. `enlisted` and `elsewhere` pick out the soldier and the
    /// neighbour. Returns the world and the settlement.
    fn village(ids: &[u64], enlisted: u64, elsewhere: u64) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(SimTick(4_321));
        world.insert_resource(Villagers::default_data(&Calendar::default_data()));
        let start = SettlementLoyalty {
            mean: 0.6,
            min: 0.6,
            residents: 3,
            below_since: Some(100),
            revolt_since: Some(200),
        };
        let threat = Threat {
            level: 0.5,
            provoked_by: Some(FactionId::PLAYER),
            provoked_until: 9_000,
            ..Threat::default()
        };
        let mut goodwill = Goodwill::default();
        goodwill.set(FactionId::PLAYER, 0.5);
        let s = world
            .spawn((SpawnId(0), Owner::default(), goodwill, start, threat))
            .id();
        let other = world
            .spawn((SpawnId(1), Owner::default(), Goodwill::default()))
            .id();
        for &id in ids {
            let home = if id == elsewhere { other } else { s };
            let mut e = world.spawn((
                SpawnId(id),
                Villager {
                    name: sim_core::NameId(id as u32),
                    age_days: 9_000,
                    home,
                    settlement: home,
                },
                Allegiance(FactionId::INDEPENDENT),
                Loyalty(0.6),
            ));
            if id == enlisted {
                e.insert((Enlisted, Allegiance(FactionId::RAIDERS)));
            }
        }
        (world, s)
    }

    fn people(world: &mut World) -> Vec<(u64, u16, f32)> {
        let mut v: Vec<(u64, u16, f32)> = world
            .query::<(&SpawnId, &Allegiance, &Loyalty)>()
            .iter(world)
            .map(|(id, a, l)| (id.0, a.0.0, l.0))
            .collect();
        v.sort_by_key(|p| p.0);
        v
    }

    /// GP-05, GP-06: only the settlement's own, unenlisted villagers change
    /// sides, whatever order the entities were spawned in.
    #[test]
    fn set_owner_hands_over_residents_in_spawn_id_order() {
        let ids = [10, 11, 12, 13, 14];
        let (mut a, sa) = village(&ids, 12, 14);
        let mut reversed = ids;
        reversed.reverse();
        let (mut b, sb) = village(&reversed, 12, 14);

        let change = set_owner(&mut a, sa, FactionId::PLAYER, None).expect("populated");
        assert_eq!(
            change,
            OwnerChange {
                previous: FactionId::INDEPENDENT,
                residents: 3,
            }
        );
        assert_eq!(set_owner(&mut b, sb, FactionId::PLAYER, None), Some(change));
        let pa = people(&mut a);
        assert_eq!(pa, people(&mut b), "spawn order doesn't matter");
        // Seeded from goodwill 0.5 toward the player: (0.5 + 1) / 2.
        assert_eq!(
            pa,
            vec![
                (10, 1, 0.75),
                (11, 1, 0.75),
                (12, 2, 0.6), // enlisted: untouched
                (13, 1, 0.75),
                (14, 0, 0.6), // lives elsewhere: untouched
            ]
        );

        let s = a.entity(sa);
        assert_eq!(
            *s.get::<Owner>().unwrap(),
            Owner {
                faction: FactionId::PLAYER,
                since: 4_321,
            }
        );
        let t = s.get::<Threat>().unwrap();
        assert_eq!((t.provoked_by, t.provoked_until), (None, 0));
        assert_eq!(t.level, 0.5, "the threat itself is the army's business");
        assert_eq!(
            *s.get::<SettlementLoyalty>().unwrap(),
            SettlementLoyalty {
                mean: 0.75,
                min: 0.75,
                residents: 3,
                below_since: None,
                revolt_since: None,
            }
        );

        // A revolt back to independence with an explicit seed.
        let back = set_owner(&mut a, sa, FactionId::INDEPENDENT, Some(1.5)).unwrap();
        assert_eq!(back.previous, FactionId::PLAYER);
        assert_eq!(people(&mut a)[0], (10, 0, 1.0), "seed clamped to 0..1");
        // Without a seed, independence aims at loyalty.independent_target.
        set_owner(&mut a, sa, FactionId::INDEPENDENT, None).unwrap();
        let target = a.resource::<Villagers>().data.loyalty.independent_target;
        assert_eq!(people(&mut a)[1], (11, 0, target));
    }

    #[test]
    fn set_owner_ignores_what_is_not_a_populated_settlement() {
        let (mut world, _) = village(&[10], 0, 0);
        let field = world.spawn(SpawnId(99)).id();
        assert_eq!(set_owner(&mut world, field, FactionId::PLAYER, None), None);
        assert!(world.get::<Owner>(field).is_none());
        assert_eq!(people(&mut world), vec![(10, 0, 0.6)]);
    }
}
