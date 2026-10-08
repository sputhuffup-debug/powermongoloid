//! Villagers' loyalty to their settlement's owner (GP-06, SIM-24; ADR 0014).
//!
//! - [`Loyalty`] (0..1) on every villager, starting at
//!   `loyalty.independent_target` (villagers.ron).
//! - [`SettlementLoyalty`] on every populated settlement: the residents'
//!   mean and minimum, and how long the mean has been below
//!   `loyalty.revolt_threshold` (the revolt clock sim_military reads).
//! - Once a day, in [`AgentSet::Standing`](crate::AgentSet::Standing),
//!   [`drift_loyalty`] moves each villager toward [`loyalty_target`] and
//!   [`aggregate_loyalty`] refreshes the settlement summaries.
//!
//! Enlisted villagers keep the loyalty they had when they left: they are
//! neither drifted nor counted.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use sim_core::{
    Calendar, FactionId, FactionModifiers, SimTick, SpawnId, Stat, StateHash, StateHasher,
};

use crate::components::{Enlisted, Villager};
use crate::data::{LoyaltyData, Villagers};
use crate::food::FoodSecurity;
use crate::standing::{Goodwill, Owner};

/// A villager's loyalty to its settlement's owner, 0 (ready to rise) .. 1
/// (devoted) (GP-06). On every villager. Set by
/// [`set_owner`](crate::standing::set_owner) when the settlement changes
/// hands, then drifted daily.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Loyalty(pub f32);

impl StateHash for Loyalty {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.0);
    }
}

/// A settlement's loyalty, summarised daily from its residents who aren't
/// enlisted (GP-06, UI-02). On every populated settlement.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct SettlementLoyalty {
    /// Mean loyalty of the residents (0 with none).
    pub mean: f32,
    /// Lowest loyalty among them (0 with none).
    pub min: f32,
    /// Residents counted (not enlisted).
    pub residents: u32,
    /// The day tick since which `mean` has been below
    /// `loyalty.revolt_threshold` under the current owner. `None` while it
    /// isn't, and always for an independent or empty settlement. Restarts
    /// when the owner changes.
    pub below_since: Option<u64>,
    /// When a revolt began, for sim_military's revolt step (it sets and
    /// clears this). Cleared when the owner changes.
    pub revolt_since: Option<u64>,
}

fn hash_tick(h: &mut StateHasher, t: Option<u64>) {
    match t {
        Some(t) => {
            h.u8(1);
            h.u64(t);
        }
        None => h.u8(0),
    }
}

impl StateHash for SettlementLoyalty {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.mean);
        h.f32(self.min);
        h.u32(self.residents);
        hash_tick(h, self.below_since);
        hash_tick(h, self.revolt_since);
    }
}

/// The loyalty a settlement's villagers drift toward (GP-06, SIM-24).
///
/// Independent (`owner.faction` 0): `independent_target`. Under an owner:
/// the weighted mean of
/// - food security: `food_days / data.food_days`, capped at 1;
/// - treatment: goodwill toward the owner as `(goodwill + 1) / 2`;
/// - time under the owner: days since `owner.since` over `settle_days`,
///   capped at 1.
pub fn loyalty_target(
    data: &LoyaltyData,
    owner: Owner,
    goodwill: f32,
    food_days: f32,
    tick: u64,
    ticks_per_day: u64,
) -> f32 {
    if owner.faction == FactionId::INDEPENDENT {
        return data.independent_target;
    }
    let food = (food_days / data.food_days).clamp(0.0, 1.0);
    let treatment = ((goodwill + 1.0) / 2.0).clamp(0.0, 1.0);
    let days = tick.saturating_sub(owner.since) as f32 / ticks_per_day.max(1) as f32;
    let time = (days / data.settle_days).clamp(0.0, 1.0);
    let weights = data.w_food + data.w_goodwill + data.w_time;
    (data.w_food * food + data.w_goodwill * treatment + data.w_time * time) / weights
}

/// One day of drift: `loyalty` closes `rate` (clamped to 0..1) of the gap to
/// `target`. At the target it stays exactly where it is.
pub fn drift_toward(loyalty: f32, target: f32, rate: f32) -> f32 {
    let rate = rate.clamp(0.0, 1.0);
    (loyalty + (target - loyalty) * rate).clamp(0.0, 1.0)
}

/// Daily (GP-06, SIM-24): every villager who isn't enlisted drifts toward
/// its settlement's [`loyalty_target`], at `loyalty.drift_per_day` times
/// the owner's `LoyaltyDrift` modifier. Parallel: each villager writes only
/// its own `Loyalty` and reads settlement components.
pub fn drift_loyalty(
    tick: Res<SimTick>,
    calendar: Res<Calendar>,
    villagers: Res<Villagers>,
    mods: Res<FactionModifiers>,
    settlements: Query<(&Owner, &Goodwill, &FoodSecurity)>,
    mut people: Query<(&Villager, &mut Loyalty), Without<Enlisted>>,
) {
    let _span = tracing::info_span!("sim_agents::drift_loyalty").entered();
    let data = &villagers.data.loyalty;
    let (tick, tpd) = (tick.0, calendar.ticks_per_day);
    people.par_iter_mut().for_each(|(v, mut loyalty)| {
        let Ok((owner, goodwill, food)) = settlements.get(v.settlement) else {
            return;
        };
        let target = loyalty_target(
            data,
            *owner,
            goodwill.get(owner.faction),
            food.days,
            tick,
            tpd,
        );
        let rate = data.drift_per_day * mods.mult(owner.faction, Stat::LoyaltyDrift);
        let next = drift_toward(loyalty.0, target, rate);
        if next != loyalty.0 {
            loyalty.0 = next;
        }
    });
}

#[derive(Default)]
struct Tally {
    sum: f64,
    min: f32,
    n: u32,
}

/// Daily, after [`drift_loyalty`] (GP-06): each settlement's
/// [`SettlementLoyalty`] from its residents, summed in SpawnId order. Sets
/// `below_since` when an owned settlement's mean falls below
/// `loyalty.revolt_threshold`, and clears it when the mean recovers.
pub fn aggregate_loyalty(
    tick: Res<SimTick>,
    villagers: Res<Villagers>,
    people: Query<(&SpawnId, &Villager, &Loyalty), Without<Enlisted>>,
    mut settlements: Query<(Entity, &Owner, &mut SettlementLoyalty)>,
) {
    let _span = tracing::info_span!("sim_agents::aggregate_loyalty").entered();
    let threshold = villagers.data.loyalty.revolt_threshold;
    let mut list: Vec<(SpawnId, Entity, f32)> = people
        .iter()
        .map(|(id, v, l)| (*id, v.settlement, l.0))
        .collect();
    list.sort_unstable_by_key(|(id, ..)| *id);
    // Each settlement's sum runs in its residents' SpawnId order.
    let mut tallies: BTreeMap<Entity, Tally> = BTreeMap::new();
    for (_, s, l) in list {
        let t = tallies.entry(s).or_insert(Tally {
            min: f32::INFINITY,
            ..Tally::default()
        });
        t.sum += l as f64;
        t.min = t.min.min(l);
        t.n += 1;
    }
    for (e, owner, mut sl) in &mut settlements {
        let (mean, min, n) = match tallies.get(&e) {
            Some(t) if t.n > 0 => ((t.sum / t.n as f64) as f32, t.min, t.n),
            _ => (0.0, 0.0, 0),
        };
        let below = n > 0 && owner.faction != FactionId::INDEPENDENT && mean < threshold;
        let below_since = if below {
            Some(sl.below_since.unwrap_or(tick.0))
        } else {
            None
        };
        let next = SettlementLoyalty {
            mean,
            min,
            residents: n,
            below_since,
            revolt_since: sl.revolt_since,
        };
        if *sl != next {
            *sl = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, TaskPoolPlugin};
    use bevy_ecs::system::RunSystemOnce;

    fn data() -> LoyaltyData {
        Villagers::default_data(&Calendar::default_data())
            .data
            .loyalty
    }

    fn owned(since: u64) -> Owner {
        Owner {
            faction: FactionId::PLAYER,
            since,
        }
    }

    /// SIM-24, GP-06: food, goodwill and time each pull the target up.
    #[test]
    fn the_target_weighs_food_goodwill_and_time() {
        let d = data();
        let tpd = 2_400;
        let independent = loyalty_target(&d, Owner::default(), -1.0, 0.0, 0, tpd);
        assert_eq!(independent, d.independent_target);

        let starved = loyalty_target(&d, owned(0), 0.0, 0.0, 0, tpd);
        let fed = loyalty_target(&d, owned(0), 0.0, d.food_days, 0, tpd);
        let overfed = loyalty_target(&d, owned(0), 0.0, 10.0 * d.food_days, 0, tpd);
        let liked = loyalty_target(&d, owned(0), 1.0, 0.0, 0, tpd);
        let settled = loyalty_target(&d, owned(0), 0.0, 0.0, 100 * tpd, tpd);
        let weights = d.w_food + d.w_goodwill + d.w_time;
        assert!((starved - 0.5 * d.w_goodwill / weights).abs() < 1e-6);
        assert!((fed - starved - d.w_food / weights).abs() < 1e-6);
        assert_eq!(overfed, fed, "food security is capped at food_days");
        assert!((liked - starved - 0.5 * d.w_goodwill / weights).abs() < 1e-6);
        assert!((settled - starved - d.w_time / weights).abs() < 1e-6);
        let everything = loyalty_target(&d, owned(0), 1.0, d.food_days, 100 * tpd, tpd);
        assert!((everything - 1.0).abs() < 1e-6);
        // The tuning the conquest scenario relies on: a starved conquest
        // aims below the revolt threshold, a fed one above its seed.
        assert!(starved < d.revolt_threshold, "starved {starved}");
        assert!(fed > 0.5, "fed {fed}");
    }

    #[test]
    fn drift_closes_part_of_the_gap() {
        assert_eq!(drift_toward(0.5, 0.1, 0.25), 0.4);
        assert_eq!(drift_toward(0.5, 0.9, 1.5), 0.9, "rate clamped to 1");
        assert_eq!(drift_toward(0.5, 0.9, -1.0), 0.5);
        assert_eq!(
            drift_toward(0.6, 0.6, 0.4),
            0.6,
            "exactly still at the target"
        );
    }

    struct Village {
        settlement: Entity,
        people: Vec<Entity>,
        soldier: Entity,
    }

    /// A settlement with four residents at loyalty 0.5 plus an enlisted one.
    fn village(world: &mut World, first: u64, owner: Owner, food_days: f32) -> Village {
        let settlement = world
            .spawn((
                SpawnId(first),
                owner,
                Goodwill::default(),
                FoodSecurity {
                    days: food_days,
                    food_value: 0.0,
                },
                SettlementLoyalty::default(),
            ))
            .id();
        let person = |world: &mut World, id: u64| {
            world
                .spawn((
                    SpawnId(id),
                    Villager {
                        name: sim_core::NameId(id as u32),
                        age_days: 9_000,
                        home: settlement,
                        settlement,
                    },
                    Loyalty(0.5),
                ))
                .id()
        };
        let people = (1..=4).map(|i| person(world, first + i)).collect();
        let soldier = person(world, first + 5);
        world.entity_mut(soldier).insert(Enlisted);
        Village {
            settlement,
            people,
            soldier,
        }
    }

    fn day(world: &mut World, n: u64) {
        let tpd = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = n * tpd;
        world.run_system_once(drift_loyalty).unwrap();
        world.run_system_once(aggregate_loyalty).unwrap();
    }

    fn loyalty(world: &World, e: Entity) -> f32 {
        world.get::<Loyalty>(e).unwrap().0
    }

    /// GP-06, SIM-24: a starved conquest loses loyalty day by day until its
    /// mean is below the revolt threshold, a fed one gains it, and an
    /// independent village settles at its target. Enlisted villagers are
    /// neither drifted nor counted.
    #[test]
    fn loyalty_drifts_toward_its_target_each_day() {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        let world = app.world_mut();
        let calendar = Calendar::default_data();
        let d = data();
        world.insert_resource(Villagers::default_data(&calendar));
        world.insert_resource(calendar);
        world.insert_resource(SimTick(0));
        world.insert_resource(FactionModifiers::default());
        let starved = village(world, 0, owned(0), 0.0);
        let fed = village(world, 10, owned(0), 2.0 * d.food_days);
        let free = village(world, 20, Owner::default(), 0.0);

        // (Later the growing time term slowly lifts the target again, but a
        // starved village never gets back to the threshold on time alone.)
        let mut last = 0.5;
        let mut fell_below = None;
        for n in 0..4 {
            day(world, n);
            let now = loyalty(world, starved.people[0]);
            assert!(now < last, "day {n}: {now} after {last}");
            for p in &starved.people {
                assert_eq!(loyalty(world, *p), now, "one settlement, one target");
            }
            let sl = *world.get::<SettlementLoyalty>(starved.settlement).unwrap();
            assert_eq!((sl.mean, sl.min, sl.residents), (now, now, 4));
            if now < d.revolt_threshold {
                let since = *fell_below.get_or_insert(n * 2_400);
                assert_eq!(sl.below_since, Some(since), "the clock keeps its start");
            } else {
                assert_eq!(sl.below_since, None);
            }
            last = now;
        }
        assert!(
            fell_below.is_some(),
            "a starved conquest falls below the threshold"
        );
        assert_eq!(loyalty(world, starved.soldier), 0.5, "enlisted: untouched");
        assert!(loyalty(world, fed.people[0]) > 0.5);
        let sl = *world.get::<SettlementLoyalty>(fed.settlement).unwrap();
        assert_eq!(sl.below_since, None);
        // Independence: from 0.5 toward independent_target, never "below".
        let f = loyalty(world, free.people[0]);
        assert!(f > 0.5 && f <= d.independent_target, "{f}");
        let sl = *world.get::<SettlementLoyalty>(free.settlement).unwrap();
        assert_eq!(sl.below_since, None);

        // Feeding the starved village lifts it back above the threshold, and
        // the clock clears.
        world
            .get_mut::<FoodSecurity>(starved.settlement)
            .unwrap()
            .days = 2.0 * d.food_days;
        for n in 6..12 {
            day(world, n);
        }
        let sl = *world.get::<SettlementLoyalty>(starved.settlement).unwrap();
        assert!(sl.mean > d.revolt_threshold, "{}", sl.mean);
        assert_eq!(sl.below_since, None);
    }

    /// GP-06: the owner's `LoyaltyDrift` modifier scales the daily rate.
    #[test]
    fn the_loyalty_drift_modifier_speeds_drift() {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        let world = app.world_mut();
        let calendar = Calendar::default_data();
        world.insert_resource(Villagers::default_data(&calendar));
        world.insert_resource(calendar);
        world.insert_resource(SimTick(0));
        let mut mods = FactionModifiers::default();
        mods.add(
            FactionId::PLAYER,
            Stat::LoyaltyDrift,
            sim_core::ModifierSource::Monuments { kind: 6 },
            0.5,
        );
        world.insert_resource(mods);
        let fast = village(world, 0, owned(0), 0.0);
        let slow = village(
            world,
            10,
            Owner {
                faction: FactionId::RIVAL,
                since: 0,
            },
            0.0,
        );
        day(world, 0);
        let d = data();
        let target = loyalty_target(&d, owned(0), 0.0, 0.0, 0, 2_400);
        let expect = |rate: f32| 0.5 + (target - 0.5) * rate;
        assert_eq!(
            loyalty(world, fast.people[0]),
            expect(d.drift_per_day * 1.5)
        );
        assert_eq!(loyalty(world, slow.people[0]), expect(d.drift_per_day));
    }
}
