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
//! - [`GoodwillLedger`] and [`apply_goodwill`]: deeds (sim_military's
//!   trades, seizures, killings, answered requests, the "Known" boon) post
//!   [`GoodwillChange`]s; once a tick, in `AgentSet::Standing`, the ledger
//!   is sorted, each change and a `goodwill.spread_fraction` of it for every
//!   settlement within `goodwill.spread_radius` are summed, and the sums are
//!   applied, clamped (GP-22).
//!
//! Both components sit on every populated settlement from the start (all
//! presets), next to `SettlementEconomy`, so the state hash covers them
//! from tick 0.

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::{Allegiance, FactionId, SimTick, SpawnId, StateHash, StateHasher};
use sim_world::Settlement;

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

// GP-12: saved field by field (Goodwill's list stays private).
sim_core::save_fields!(Owner { faction, since });
sim_core::save_fields!(Goodwill { by_faction });

/// The deed behind a [`GoodwillChange`] (GP-22). Its delta comes from
/// progression.ron `goodwill_deeds` (sim_military), or from the boon's own
/// effect for [`Deed::Boon`]. Append only: saves and the hash store the tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Deed {
    /// A fair trade: bought food with goods (Trade, passive gathering).
    Trade = 0,
    /// Arms paid for in food (passive Equip).
    Equip = 1,
    /// Food taken from the village's reserve (neutral gathering, hungry).
    Seize = 2,
    /// Food plundered (aggressive gathering).
    Plunder = 3,
    /// The settlement was provoked into arms.
    Provoke = 4,
    /// One of the settlement's militia was killed.
    Killing = 5,
    /// A request of the settlement was answered (M4 step 3).
    Request = 6,
    /// A boon's goodwill (the "Known" start boon).
    Boon = 7,
}

/// One change of a settlement's goodwill toward a faction, waiting in the
/// [`GoodwillLedger`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoodwillChange {
    /// The settlement, by SpawnId.
    pub settlement: u64,
    /// The faction whose deed it was.
    pub faction: FactionId,
    /// The change at the settlement itself (spread adds a fraction of it
    /// to its neighbours).
    pub delta: f32,
    /// The deed.
    pub cause: Deed,
}

impl sim_core::SaveState for Deed {
    fn save(&self, w: &mut sim_core::SaveWriter) {
        w.u8(*self as u8);
    }
    fn load(r: &mut sim_core::SaveReader) -> Result<Self, sim_core::SaveError> {
        const ALL: [Deed; 8] = [
            Deed::Trade,
            Deed::Equip,
            Deed::Seize,
            Deed::Plunder,
            Deed::Provoke,
            Deed::Killing,
            Deed::Request,
            Deed::Boon,
        ];
        let t = r.u8()?;
        ALL.get(t as usize)
            .copied()
            .ok_or_else(|| sim_core::SaveError::Corrupt(format!("bad deed tag {t}")))
    }
}

sim_core::save_fields!(GoodwillChange {
    settlement,
    faction,
    delta,
    cause
});
sim_core::save_newtype!(GoodwillLedger);

impl GoodwillChange {
    /// The canonical order: settlement, faction, deed, then the delta's bits.
    fn key(&self) -> (u64, FactionId, Deed, u32) {
        (
            self.settlement,
            self.faction,
            self.cause,
            self.delta.to_bits(),
        )
    }
}

/// Goodwill changes posted this tick, applied (and cleared) by
/// [`apply_goodwill`] in `AgentSet::Standing` (GP-22). Writers push from
/// serial steps; the order they push in doesn't matter, because the ledger
/// is sorted before it is applied. Hashed: a change posted after
/// `AgentSet::Standing` (the hourly progress driver) waits a tick.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct GoodwillLedger(pub Vec<GoodwillChange>);

impl GoodwillLedger {
    /// Posts a change of `delta` in `settlement`'s goodwill toward
    /// `faction`. A zero or non-finite delta is dropped.
    pub fn push(&mut self, settlement: u64, faction: FactionId, delta: f32, cause: Deed) {
        if delta != 0.0 && delta.is_finite() {
            self.0.push(GoodwillChange {
                settlement,
                faction,
                delta,
                cause,
            });
        }
    }

    /// Whether nothing is waiting.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl StateHash for GoodwillLedger {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.0.len() as u32);
        for c in &self.0 {
            h.u64(c.settlement);
            h.u16(c.faction.0);
            h.f32(c.delta);
            h.u8(c.cause as u8);
        }
    }
}

/// Run condition for [`apply_goodwill`]: something is in the ledger.
pub fn ledger_nonempty(ledger: Res<GoodwillLedger>) -> bool {
    !ledger.is_empty()
}

/// The summed goodwill changes of `changes`, as (index into `settlements`,
/// faction, total), sorted by index then faction (GP-22).
///
/// `settlements` is (SpawnId, centre), sorted by SpawnId. Each change
/// counts in full at its own settlement and `fraction` of it at every other
/// settlement whose centre is within `radius` of its centre. `changes` is
/// sorted into its canonical order first and settlements are visited in
/// SpawnId order, so the sums don't depend on the order anything was pushed
/// or spawned in. A change naming no listed settlement is dropped.
pub fn spread_totals(
    changes: &mut [GoodwillChange],
    settlements: &[(SpawnId, IVec2)],
    radius: f32,
    fraction: f32,
) -> Vec<(usize, FactionId, f32)> {
    changes.sort_by_key(|c| c.key());
    let r2 = radius * radius;
    let at: Vec<Option<IVec2>> = changes
        .iter()
        .map(|c| {
            settlements
                .binary_search_by_key(&c.settlement, |(id, _)| id.0)
                .ok()
                .map(|i| settlements[i].1)
        })
        .collect();
    let mut out: Vec<(usize, FactionId, f32)> = Vec::new();
    for (i, &(id, centre)) in settlements.iter().enumerate() {
        let first = out.len();
        for (c, from) in changes.iter().zip(&at) {
            let Some(from) = *from else {
                continue;
            };
            let d = if c.settlement == id.0 {
                c.delta
            } else if fraction > 0.0 && (from - centre).as_vec2().length_squared() <= r2 {
                c.delta * fraction
            } else {
                continue;
            };
            match out[first..].iter_mut().find(|(_, f, _)| *f == c.faction) {
                Some(entry) => entry.2 += d,
                None => out.push((i, c.faction, d)),
            }
        }
        out[first..].sort_by_key(|(_, f, _)| *f);
    }
    out
}

/// `AgentSet::Standing`, first, when the ledger has entries (GP-22): sums
/// the ledger's changes and their spread ([`spread_totals`],
/// villagers.ron `goodwill`) per settlement and faction, adds each sum to
/// the settlement's [`Goodwill`] (clamped to -1..1), and clears the ledger.
pub fn apply_goodwill(
    villagers: Res<Villagers>,
    mut ledger: ResMut<GoodwillLedger>,
    mut q: Query<(&SpawnId, &Settlement, &mut Goodwill)>,
) {
    let _span = tracing::info_span!("sim_agents::apply_goodwill").entered();
    let mut settlements: Vec<(SpawnId, IVec2)> =
        q.iter().map(|(id, s, _)| (*id, s.center)).collect();
    settlements.sort_unstable_by_key(|(id, _)| *id);
    let g = &villagers.data.goodwill;
    let totals = spread_totals(
        &mut ledger.0,
        &settlements,
        g.spread_radius,
        g.spread_fraction,
    );
    ledger.0.clear();
    let mut by_id: Vec<(SpawnId, Mut<Goodwill>)> =
        q.iter_mut().map(|(id, _, gw)| (*id, gw)).collect();
    by_id.sort_unstable_by_key(|(id, _)| *id);
    for (i, faction, delta) in totals {
        // `settlements` and `by_id` list the same entities in the same order.
        by_id[i].1.add(faction, delta);
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

    /// Settlements 0..4 (SpawnIds) at x = 0, 100, 140, 400 and a field (no
    /// Goodwill) at x = 50, spawned in `order`; the ledger gets `changes` in
    /// the given order. Returns each settlement's goodwill after
    /// `apply_goodwill`, by SpawnId.
    fn spread_world(
        order: &[u64],
        changes: &[GoodwillChange],
    ) -> Vec<(u64, Vec<(FactionId, f32)>)> {
        use bevy_ecs::system::RunSystemOnce;
        let mut world = World::new();
        let villagers = Villagers::default_data(&Calendar::default_data());
        let g = &villagers.data.goodwill;
        assert_eq!(
            (g.spread_radius, g.spread_fraction),
            (150.0, 0.25),
            "test assumes"
        );
        world.insert_resource(villagers);
        let xs = [0, 100, 140, 400, 50];
        for &id in order {
            let mut e = world.spawn((
                SpawnId(id),
                Settlement {
                    name: sim_core::NameId(id as u32),
                    region: 0,
                    center: IVec2::new(xs[id as usize], 0),
                },
            ));
            if id != 4 {
                e.insert(Goodwill::default());
            }
        }
        let mut ledger = GoodwillLedger::default();
        for c in changes {
            ledger.push(c.settlement, c.faction, c.delta, c.cause);
        }
        world.insert_resource(ledger);
        world.run_system_once(apply_goodwill).unwrap();
        assert!(world.resource::<GoodwillLedger>().is_empty(), "cleared");
        let mut out: Vec<(u64, Vec<(FactionId, f32)>)> = world
            .query::<(&SpawnId, &Goodwill)>()
            .iter(&world)
            .map(|(id, g)| (id.0, g.entries().to_vec()))
            .collect();
        out.sort_by_key(|(id, _)| *id);
        out
    }

    /// GP-22: the ledger's sums don't depend on the order changes were
    /// pushed or settlements spawned in; a change counts in full where it
    /// happened and a fraction within the radius; sums are clamped.
    #[test]
    fn spread_is_order_independent() {
        let change = |settlement, faction, delta, cause| GoodwillChange {
            settlement,
            faction,
            delta,
            cause,
        };
        let changes = [
            change(0, FactionId::PLAYER, 0.4, Deed::Boon),
            change(1, FactionId::PLAYER, -0.1, Deed::Seize),
            change(0, FactionId::RAIDERS, -0.3, Deed::Plunder),
            change(2, FactionId::PLAYER, 0.7, Deed::Request),
            change(2, FactionId::PLAYER, 0.7, Deed::Request),
            change(9, FactionId::PLAYER, 1.0, Deed::Trade), // no such settlement
            change(3, FactionId::RIVAL, 0.0, Deed::Trade),  // dropped
        ];
        let mut reversed = changes;
        reversed.reverse();
        let a = spread_world(&[0, 1, 2, 3, 4], &changes);
        let b = spread_world(&[4, 3, 2, 1, 0], &reversed);
        let c = spread_world(
            &[2, 0, 4, 3, 1],
            &[
                changes[3], changes[0], changes[5], changes[1], changes[4], changes[2],
            ],
        );
        assert_eq!(a, b, "push and spawn order don't matter");
        assert_eq!(a, c);

        let near = |x: f32, y: f32| (x - y).abs() < 1e-5;
        let get = |id: u64, f: FactionId| {
            a.iter()
                .find(|(s, _)| *s == id)
                .and_then(|(_, e)| e.iter().find(|(g, _)| *g == f))
                .map_or(0.0, |(_, v)| *v)
        };
        // 0 (x 0): its own +0.4, a quarter of 1's -0.1 (100 away); 2 is 140
        // away: a quarter of 1.4.
        assert!(near(get(0, FactionId::PLAYER), 0.4 - 0.025 + 0.35));
        assert!(near(get(0, FactionId::RAIDERS), -0.3));
        // 1 (x 100): -0.1, +0.1 from 0, +0.35 from 2 (40 away).
        assert!(near(get(1, FactionId::PLAYER), -0.1 + 0.1 + 0.35));
        assert!(near(get(1, FactionId::RAIDERS), -0.075));
        // 2 (x 140): 1.4 + 0.1 - 0.025, clamped.
        assert_eq!(get(2, FactionId::PLAYER), 1.0, "clamped");
        // 3 (x 400) is out of everyone's reach; zero deltas add nothing.
        assert_eq!(a.iter().find(|(s, _)| *s == 3).unwrap().1, vec![]);
        assert_eq!(a.len(), 4, "the field has no goodwill");
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
