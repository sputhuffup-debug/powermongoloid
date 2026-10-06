//! Physical goods (SIM-20): inventories carried by agents, stockpiles held
//! by buildings, and transfers between them.
//!
//! Many agents use the same storehouse in the same tick, so agents never
//! touch a stockpile directly. They post a [`Transfers`] order on
//! themselves; [`resolve_transfers`] applies all orders in
//! `(store SpawnId, agent SpawnId)` order, so the outcome never depends on
//! scheduling (ADR 0007).

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use sim_core::{SpawnId, StateHash, StateHasher};

use crate::data::{Economy, GoodId};

/// Goods held by a building.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Stockpile {
    pub goods: BTreeMap<GoodId, u32>,
    pub capacity: u32,
}

impl Stockpile {
    pub fn with_capacity(capacity: u32) -> Self {
        Self {
            goods: BTreeMap::new(),
            capacity,
        }
    }

    pub fn total(&self) -> u32 {
        self.goods.values().sum()
    }

    pub fn get(&self, g: GoodId) -> u32 {
        self.goods.get(&g).copied().unwrap_or(0)
    }

    pub fn room(&self) -> u32 {
        self.capacity.saturating_sub(self.total())
    }

    /// Adds up to `qty`, limited by capacity; returns the amount added.
    pub fn add(&mut self, g: GoodId, qty: u32) -> u32 {
        let n = qty.min(self.room());
        if n > 0 {
            *self.goods.entry(g).or_insert(0) += n;
        }
        n
    }

    /// Removes up to `qty`; returns the amount removed.
    pub fn take(&mut self, g: GoodId, qty: u32) -> u32 {
        let have = self.get(g);
        let n = qty.min(have);
        if n == have {
            self.goods.remove(&g);
        } else if n > 0 {
            *self.goods.get_mut(&g).expect("have > n > 0") -= n;
        }
        n
    }

    /// Total food value held.
    pub fn food_value(&self, econ: &Economy) -> f32 {
        self.goods
            .iter()
            .map(|(g, q)| econ.food_value(*g) * *q as f32)
            .sum()
    }

    pub fn has_food(&self, econ: &Economy) -> bool {
        self.goods.keys().any(|g| econ.is_food(*g))
    }
}

impl StateHash for Stockpile {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.capacity);
        h.u32(self.goods.len() as u32);
        for (g, q) in &self.goods {
            h.u8(g.0);
            h.u32(*q);
        }
    }
}

pub const INVENTORY_SLOTS: usize = 4;

/// Goods carried by an agent: up to four kinds, `capacity` units in all.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Inventory {
    pub slots: [(GoodId, u16); INVENTORY_SLOTS],
    pub capacity: u16,
}

impl Inventory {
    pub fn with_capacity(capacity: u16) -> Self {
        Self {
            slots: [(GoodId(0), 0); INVENTORY_SLOTS],
            capacity,
        }
    }

    pub fn total(&self) -> u16 {
        self.slots.iter().map(|s| s.1).sum()
    }

    pub fn room(&self) -> u16 {
        self.capacity.saturating_sub(self.total())
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    pub fn get(&self, g: GoodId) -> u16 {
        self.slots
            .iter()
            .filter(|s| s.0 == g && s.1 > 0)
            .map(|s| s.1)
            .sum()
    }

    /// Adds up to `qty` (limited by room and free slots); returns the amount.
    pub fn add(&mut self, g: GoodId, qty: u16) -> u16 {
        let n = qty.min(self.room());
        if n == 0 {
            return 0;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.0 == g && s.1 > 0) {
            slot.1 += n;
            return n;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.1 == 0) {
            *slot = (g, n);
            return n;
        }
        0
    }

    /// Adds `qty` regardless of capacity (food picked up to eat on the
    /// spot); still needs a slot. Returns the amount added.
    pub fn add_over_capacity(&mut self, g: GoodId, qty: u16) -> u16 {
        if qty == 0 {
            return 0;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.0 == g && s.1 > 0) {
            slot.1 += qty;
            return qty;
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.1 == 0) {
            *slot = (g, qty);
            return qty;
        }
        0
    }

    pub fn take(&mut self, g: GoodId, qty: u16) -> u16 {
        let mut left = qty;
        for slot in self.slots.iter_mut().filter(|s| s.0 == g) {
            let n = left.min(slot.1);
            slot.1 -= n;
            left -= n;
        }
        qty - left
    }

    /// Non-empty slots.
    pub fn contents(&self) -> impl Iterator<Item = (GoodId, u16)> + '_ {
        self.slots.iter().copied().filter(|s| s.1 > 0)
    }

    pub fn has_all(&self, needs: &[(GoodId, u16)]) -> bool {
        needs.iter().all(|(g, q)| self.get(*g) >= *q)
    }
}

impl StateHash for Inventory {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u16(self.capacity);
        // Canonical order, so slot layout doesn't matter.
        let mut c: Vec<(GoodId, u16)> = self.contents().collect();
        c.sort();
        for (g, q) in c {
            h.u8(g.0);
            h.u16(q);
        }
    }
}

/// One step of a transfer between an agent and a building.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferOp {
    /// Take up to `max` of one good.
    Take { good: GoodId, max: u16 },
    /// Take up to `max` units of food, most nourishing first.
    TakeFood { max: u16 },
    /// Take up to `max` units of food to eat right away, even if the
    /// villager's hands are full.
    TakeMeal { max: u16 },
    /// Take up to `max` units of anything, largest piles first.
    TakeAny { max: u16 },
    /// Put everything carried into the building (as much as fits).
    PutAll,
}

/// A transfer order an agent posts on itself; applied and cleared by
/// [`resolve_transfers`] in the same tick.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct Transfers {
    pub order: Option<TransferOrder>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransferOrder {
    pub store: Entity,
    pub ops: Vec<TransferOp>,
}

impl Transfers {
    pub fn post(&mut self, store: Entity, ops: Vec<TransferOp>) {
        self.order = Some(TransferOrder { store, ops });
    }
}

impl StateHash for Transfers {
    fn hash_state(&self, h: &mut StateHasher) {
        match &self.order {
            None => h.u8(0),
            Some(o) => {
                h.u8(1);
                h.entity(o.store);
                h.u32(o.ops.len() as u32);
                for op in &o.ops {
                    match *op {
                        TransferOp::Take { good, max } => {
                            h.u8(1);
                            h.u8(good.0);
                            h.u16(max);
                        }
                        TransferOp::TakeFood { max } => {
                            h.u8(2);
                            h.u16(max);
                        }
                        TransferOp::TakeAny { max } => {
                            h.u8(3);
                            h.u16(max);
                        }
                        TransferOp::TakeMeal { max } => {
                            h.u8(5);
                            h.u16(max);
                        }
                        TransferOp::PutAll => h.u8(4),
                    }
                }
            }
        }
    }
}

/// Applies one op between an inventory and a stockpile.
pub fn apply_op(op: TransferOp, inv: &mut Inventory, store: &mut Stockpile, econ: &Economy) {
    match op {
        TransferOp::Take { good, max } => {
            let want = max.min(inv.room());
            let got = store.take(good, want as u32) as u16;
            let added = inv.add(good, got);
            store.add(good, (got - added) as u32);
        }
        TransferOp::TakeFood { max } => {
            let mut left = max.min(inv.room());
            for good in econ.foods_by_value() {
                if left == 0 {
                    break;
                }
                let got = store.take(good, left as u32) as u16;
                let added = inv.add(good, got);
                store.add(good, (got - added) as u32);
                left -= added;
            }
        }
        TransferOp::TakeMeal { max } => {
            let mut left = max;
            for good in econ.foods_by_value() {
                if left == 0 {
                    break;
                }
                let got = store.take(good, left as u32) as u16;
                let added = inv.add_over_capacity(good, got);
                store.add(good, (got - added) as u32);
                left -= added;
            }
        }
        TransferOp::TakeAny { max } => {
            let mut left = max.min(inv.room());
            let mut piles: Vec<(GoodId, u32)> = store.goods.iter().map(|(g, q)| (*g, *q)).collect();
            piles.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            for (good, _) in piles {
                if left == 0 {
                    break;
                }
                let got = store.take(good, left as u32) as u16;
                let added = inv.add(good, got);
                store.add(good, (got - added) as u32);
                left -= added;
            }
        }
        TransferOp::PutAll => {
            let carried: Vec<(GoodId, u16)> = inv.contents().collect();
            for (good, qty) in carried {
                let put = store.add(good, qty as u32) as u16;
                inv.take(good, put);
            }
        }
    }
}

/// Act set, after agents post orders: apply every order in a stable order.
pub fn resolve_transfers(
    econ: Res<Economy>,
    ids: Query<&SpawnId>,
    mut agents: Query<(Entity, &SpawnId, &mut Inventory, &mut Transfers)>,
    mut stores: Query<&mut Stockpile>,
) {
    let _span = tracing::info_span!("sim_economy::resolve_transfers").entered();
    let mut pending: Vec<(u64, u64, Entity)> = agents
        .iter()
        .filter_map(|(e, id, _, t)| {
            let order = t.order.as_ref()?;
            let store_id = ids.get(order.store).map_or(u64::MAX, |s| s.0);
            Some((store_id, id.0, e))
        })
        .collect();
    if pending.is_empty() {
        return;
    }
    pending.sort_unstable();
    for (_, _, agent) in pending {
        let Ok((_, _, mut inv, mut transfers)) = agents.get_mut(agent) else {
            continue;
        };
        let Some(order) = transfers.order.take() else {
            continue;
        };
        if let Ok(mut store) = stores.get_mut(order.store) {
            for op in order.ops {
                apply_op(op, &mut inv, &mut store, &econ);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn econ() -> Economy {
        Economy::default_data()
    }

    #[test]
    fn inventory_respects_capacity_and_slots() {
        let mut inv = Inventory::with_capacity(10);
        assert_eq!(inv.add(GoodId(0), 6), 6);
        assert_eq!(inv.add(GoodId(1), 6), 4);
        assert_eq!(inv.total(), 10);
        assert_eq!(inv.take(GoodId(0), 9), 6);
        // Only four kinds fit.
        let mut inv = Inventory::with_capacity(100);
        for g in 0..4 {
            inv.add(GoodId(g), 1);
        }
        assert_eq!(inv.add(GoodId(7), 1), 0);
    }

    #[test]
    fn stockpile_add_take() {
        let mut s = Stockpile::with_capacity(10);
        assert_eq!(s.add(GoodId(2), 15), 10);
        assert_eq!(s.take(GoodId(2), 4), 4);
        assert_eq!(s.get(GoodId(2)), 6);
        assert_eq!(s.take(GoodId(2), 99), 6);
        assert!(s.goods.is_empty());
    }

    #[test]
    fn take_food_prefers_nourishing_goods() {
        let e = econ();
        let mut store = Stockpile::with_capacity(1000);
        let (grain, mutton) = (GoodId(0), GoodId(2));
        store.add(grain, 10);
        store.add(mutton, 3);
        let mut inv = Inventory::with_capacity(5);
        apply_op(TransferOp::TakeFood { max: 5 }, &mut inv, &mut store, &e);
        assert_eq!(inv.get(mutton), 3);
        assert_eq!(inv.get(grain), 2);
        assert_eq!(store.get(grain), 8);
    }

    #[test]
    fn take_meal_ignores_full_hands() {
        let e = econ();
        let mut store = Stockpile::with_capacity(100);
        store.add(GoodId(3), 5);
        let mut inv = Inventory::with_capacity(4);
        inv.add(GoodId(5), 4);
        apply_op(TransferOp::TakeFood { max: 2 }, &mut inv, &mut store, &e);
        assert_eq!(inv.get(GoodId(3)), 0, "no room to carry");
        apply_op(TransferOp::TakeMeal { max: 2 }, &mut inv, &mut store, &e);
        assert_eq!(inv.get(GoodId(3)), 2, "but enough to eat");
    }

    #[test]
    fn put_all_keeps_what_does_not_fit() {
        let e = econ();
        let mut store = Stockpile::with_capacity(3);
        let mut inv = Inventory::with_capacity(10);
        inv.add(GoodId(4), 5);
        apply_op(TransferOp::PutAll, &mut inv, &mut store, &e);
        assert_eq!(store.get(GoodId(4)), 3);
        assert_eq!(inv.get(GoodId(4)), 2);
    }

    #[test]
    fn resolver_serves_lower_spawn_ids_first() {
        use bevy_ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.insert_resource(econ());
        let mut store = Stockpile::with_capacity(100);
        store.add(GoodId(3), 5);
        let store = world.spawn((SpawnId(0), store)).id();
        // Spawn the later agent first: order must come from SpawnId.
        let late = world
            .spawn((
                SpawnId(9),
                Inventory::with_capacity(4),
                Transfers::default(),
            ))
            .id();
        let early = world
            .spawn((
                SpawnId(1),
                Inventory::with_capacity(4),
                Transfers::default(),
            ))
            .id();
        for a in [late, early] {
            world.get_mut::<Transfers>(a).unwrap().post(
                store,
                vec![TransferOp::Take {
                    good: GoodId(3),
                    max: 4,
                }],
            );
        }
        world.run_system_once(resolve_transfers).unwrap();
        assert_eq!(world.get::<Inventory>(early).unwrap().get(GoodId(3)), 4);
        assert_eq!(world.get::<Inventory>(late).unwrap().get(GoodId(3)), 1);
        assert!(world.get::<Transfers>(late).unwrap().order.is_none());
    }
}
