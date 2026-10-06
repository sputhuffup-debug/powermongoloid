//! Populating one region at world creation: households, villagers, their
//! occupations and workplaces, and each settlement's buildings.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use rand::RngExt;
use rand_chacha::ChaCha8Rng;
use sim_core::rng::rng_key;
use sim_core::{
    Allegiance, Calendar, DetailLevel, FactionId, Motion, NameTable, NextSpawnId, Position,
    SimSeed, SpawnId,
};
use sim_economy::layout::{LayoutRequest, build_settlement};
use sim_economy::{
    Economy, Home, Inventory, Occupation, Occupied, SettlementEconomy, Stockpile, Transfers,
    Workplace, WorkplaceKind,
};
use sim_world::{Mobile, RegionId, Settlement, Terrain, WorldMap};

use crate::components::{
    Activity, CombatStats, DecisionTrace, Employment, Needs, NextDecision, Strike, Threat, Travel,
    Villager,
};
use crate::data::{RegionChoice, Villagers, YearRange};
use crate::food::FoodSecurity;

/// What population did, for diagnostics and tests.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PopulationReport {
    pub region: Option<RegionId>,
    /// (settlement, villagers) for each populated settlement.
    pub settlements: Vec<(Entity, u32)>,
    /// Workers moved to another occupation because their workplace
    /// couldn't be built (no coast, no forest, ...).
    pub reassigned: u32,
}

struct Member {
    age_days: u16,
    working: bool,
}

fn years_to_days(rng: &mut ChaCha8Rng, r: YearRange, days_per_year: u32) -> u16 {
    let years = rng.random_range(r.min..=r.max.max(r.min));
    let day = rng.random_range(0..days_per_year);
    (years * days_per_year + day).min(u16::MAX as u32) as u16
}

/// Splits `total` by weights with the largest-remainder method.
pub fn apportion(total: u32, weights: &[f32]) -> Vec<u32> {
    let sum: f32 = weights.iter().sum();
    if sum <= 0.0 || weights.is_empty() {
        return vec![0; weights.len()];
    }
    let exact: Vec<f32> = weights.iter().map(|w| w / sum * total as f32).collect();
    let mut out: Vec<u32> = exact.iter().map(|e| e.floor() as u32).collect();
    let mut rest = total - out.iter().sum::<u32>();
    let mut order: Vec<usize> = (0..weights.len()).collect();
    order.sort_by(|&a, &b| {
        let ra = exact[a] - exact[a].floor();
        let rb = exact[b] - exact[b].floor();
        rb.total_cmp(&ra).then(a.cmp(&b))
    });
    for i in order {
        if rest == 0 {
            break;
        }
        out[i] += 1;
        rest -= 1;
    }
    out
}

fn choose_region(
    settlements: &[(SpawnId, Entity, Settlement)],
    choice: RegionChoice,
) -> Option<RegionId> {
    match choice {
        RegionChoice::Index(r) => Some(r),
        RegionChoice::Auto => {
            let mut counts: BTreeMap<RegionId, u32> = BTreeMap::new();
            for (_, _, s) in settlements {
                *counts.entry(s.region).or_default() += 1;
            }
            // Most settlements; ties go to the lowest region id.
            counts
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(r, _)| r)
        }
    }
}

/// A civilian's fighting stats from `villagers.ron`.
pub fn civilian_stats(villagers: &Villagers) -> CombatStats {
    let c = &villagers.data.combat;
    CombatStats {
        health: c.health,
        max_health: c.health,
        attack: c.attack,
        defense: c.defense,
        morale: c.morale,
        stance: crate::components::Stance::Civilian,
    }
}

fn spawn(world: &mut World, bundle: impl Bundle) -> Entity {
    let id = world.resource_mut::<NextSpawnId>().allocate();
    world.spawn((id, bundle)).id()
}

/// Populates the configured region. Runs once, after world generation.
pub fn populate(world: &mut World) {
    let _span = tracing::info_span!("sim_agents::populate").entered();
    let villagers = world.resource::<Villagers>().clone();
    let econ = world.resource::<Economy>().clone();
    let calendar = world.resource::<Calendar>().clone();
    let seed = *world.resource::<SimSeed>();
    let days_per_year = calendar.days_per_year();
    let hh = &villagers.data.households;
    let pop = &villagers.data.population;

    let mut settlements: Vec<(SpawnId, Entity, Settlement)> = world
        .query::<(&SpawnId, Entity, &Settlement)>()
        .iter(world)
        .map(|(id, e, s)| (*id, e, *s))
        .collect();
    settlements.sort_unstable_by_key(|(id, ..)| *id);
    let region = choose_region(&settlements, pop.region);
    let chosen: Vec<(SpawnId, Entity, Settlement)> = settlements
        .into_iter()
        .filter(|(_, _, s)| Some(s.region) == region)
        .collect();
    let mut report = PopulationReport {
        region,
        ..Default::default()
    };
    if chosen.is_empty() || pop.total == 0 {
        world.insert_resource(report);
        return;
    }

    // Bigger farmland, bigger village.
    let weights: Vec<f32> = {
        let map = world.resource::<WorldMap>();
        chosen
            .iter()
            .map(|(_, _, s)| {
                let mut farmland = 0u32;
                for dy in -20..=20 {
                    for dx in -20..=20 {
                        let p = s.center + IVec2::new(dx, dy);
                        if map.in_bounds(p) && map.terrain(p) == Terrain::Farmland {
                            farmland += 1;
                        }
                    }
                }
                farmland as f32 + 50.0
            })
            .collect()
    };
    let floor = pop.min_per_settlement.min(pop.total / chosen.len() as u32);
    let extra = pop.total - floor * chosen.len() as u32;
    let sizes: Vec<u32> = apportion(extra, &weights)
        .into_iter()
        .map(|n| n + floor)
        .collect();

    let size_weights: Vec<f32> = hh.size_weights.iter().map(|w| *w as f32).collect();
    let size_total: f32 = size_weights.iter().sum();
    let shares: Vec<f32> = Occupation::ALL
        .iter()
        .map(|o| villagers.share(*o))
        .collect();
    let mut occupied = Occupied::default();

    for ((settlement_id, settlement, site), target) in chosen.iter().zip(&sizes) {
        let mut rng = seed.stream(rng_key("sim_agents::populate"), settlement_id.0);

        // Households.
        let mut households: Vec<Vec<Member>> = Vec::new();
        let mut placed = 0u32;
        while placed < *target {
            let mut pick = rng.random_range(0.0..size_total);
            let mut size = size_weights.len();
            for (i, w) in size_weights.iter().enumerate() {
                if pick < *w {
                    size = i + 1;
                    break;
                }
                pick -= w;
            }
            let size = (size as u32).min(target - placed) as usize;
            let mut members = Vec::with_capacity(size);
            let adults = if size >= 2 { 2 } else { 1 };
            for m in 0..size {
                let range = if m < adults {
                    if rng.random_range(0.0..1.0) < hh.elder_chance {
                        hh.elder_age
                    } else {
                        hh.adult_age
                    }
                } else {
                    hh.child_age
                };
                let age_days = years_to_days(&mut rng, range, days_per_year);
                let years = age_days as u32 / days_per_year;
                members.push(Member {
                    age_days,
                    working: years >= hh.working_age.min && years <= hh.working_age.max,
                });
            }
            placed += size as u32;
            households.push(members);
        }

        // Occupations for working-age villagers, shuffled.
        let workers = households.iter().flatten().filter(|m| m.working).count() as u32;
        let counts = apportion(workers, &shares);
        let mut jobs: Vec<Occupation> = Occupation::ALL
            .iter()
            .zip(&counts)
            .flat_map(|(o, n)| std::iter::repeat_n(*o, *n as usize))
            .collect();
        for i in (1..jobs.len()).rev() {
            let j = rng.random_range(0..=i);
            jobs.swap(i, j);
        }

        let mut req = LayoutRequest {
            houses: households.len() as u32,
            workers: BTreeMap::new(),
        };
        for occ in &jobs {
            if let Some(kind) = occ.workplace()
                && kind != WorkplaceKind::Storehouse
            {
                *req.workers.entry(kind).or_default() += 1;
            }
        }
        let layout = build_settlement(world, *settlement, site.center, &req, &mut occupied);

        // Workers whose workplaces couldn't all be built become farmers
        // (if farms have room) or idle.
        let capacity = |kind: WorkplaceKind| -> u32 {
            let slots = econ.workplace(kind).slots;
            if kind == WorkplaceKind::Storehouse {
                u32::MAX
            } else {
                layout.workplaces.get(&kind).map_or(0, |w| w.len() as u32) * slots
            }
        };
        let mut used: BTreeMap<WorkplaceKind, u32> = BTreeMap::new();
        for job in jobs.iter_mut() {
            let Some(kind) = job.workplace() else {
                continue;
            };
            let n = used.entry(kind).or_default();
            if *n < capacity(kind) {
                *n += 1;
                continue;
            }
            report.reassigned += 1;
            let farm = used.entry(WorkplaceKind::Farm).or_default();
            if *farm < capacity(WorkplaceKind::Farm) {
                *farm += 1;
                *job = Occupation::Farmer;
            } else {
                *job = Occupation::Idle;
            }
        }

        // Villagers.
        let mut residents_total = 0u32;
        let mut job_iter = jobs.into_iter();
        let mut workers_by_kind: BTreeMap<WorkplaceKind, Vec<(Entity, IVec2)>> = BTreeMap::new();
        let mut homes = Vec::new();
        for (members, house) in households.iter().zip(
            layout
                .houses
                .iter()
                .copied()
                .chain(std::iter::repeat(layout.storehouse)),
        ) {
            let home_tile = world.get::<Position>(house).map_or(site.center, |p| p.0);
            let family = &villagers.data.names.family
                [rng.random_range(0..villagers.data.names.family.len())];
            let mut residents = Vec::new();
            for m in members {
                let given = &villagers.data.names.given
                    [rng.random_range(0..villagers.data.names.given.len())];
                let name = world
                    .resource_mut::<NameTable>()
                    .push(format!("{given} {family}"));
                let occupation = if m.working {
                    job_iter.next().unwrap_or(Occupation::Idle)
                } else {
                    Occupation::Idle
                };
                let carry = if occupation == Occupation::Carter {
                    econ.data.cart_carry
                } else {
                    econ.data.carry
                };
                let decide_at =
                    world.resource::<NextSpawnId>().0 % villagers.data.ai.decision_interval;
                let e = spawn(
                    world,
                    (
                        (
                            Villager {
                                name,
                                age_days: m.age_days,
                                home: house,
                                settlement: *settlement,
                            },
                            occupation,
                            Employment::default(),
                            Needs {
                                // Staggered, so meals don't all fall at once.
                                hunger: rng.random_range(0.0..0.35),
                                rest: rng.random_range(0.8..1.0),
                                ..Needs::default()
                            },
                            Activity::default(),
                            Travel::default(),
                            NextDecision(decide_at),
                            DecisionTrace::default(),
                        ),
                        (
                            Inventory::with_capacity(carry),
                            Transfers::default(),
                            Position(home_tile),
                            Motion::default(),
                            Allegiance(FactionId(0)),
                            DetailLevel { full: true },
                            Mobile,
                            civilian_stats(&villagers),
                            Strike::default(),
                        ),
                    ),
                );
                if let Some(kind) = occupation.workplace() {
                    workers_by_kind
                        .entry(kind)
                        .or_default()
                        .push((e, home_tile));
                }
                residents.push(e);
            }
            residents_total += residents.len() as u32;
            if let Some(mut home) = world.get_mut::<Home>(house) {
                home.residents.extend(residents.iter().copied());
                if !homes.contains(&house) {
                    homes.push(house);
                }
            }
            if let Some(mut stock) = world.get_mut::<Stockpile>(house) {
                let grain = econ.foods_by_value().last().copied().unwrap_or_default();
                stock.add(
                    grain,
                    econ.data.initial_home_food_per_resident * members.len() as u32,
                );
            }
        }

        // Each worker goes to the nearest workplace of their kind with room.
        for (kind, workers) in workers_by_kind {
            let sites: Vec<(Entity, IVec2)> = layout
                .workplaces
                .get(&kind)
                .map(|w| {
                    w.iter()
                        .map(|e| (*e, world.get::<Position>(*e).map_or(IVec2::ZERO, |p| p.0)))
                        .collect()
                })
                .unwrap_or_default();
            let slots = if kind == WorkplaceKind::Storehouse {
                u32::MAX
            } else {
                econ.workplace(kind).slots
            };
            let mut filled = vec![0u32; sites.len()];
            for (worker, home) in workers {
                let best = (0..sites.len())
                    .filter(|&i| filled[i] < slots)
                    .min_by_key(|&i| ((sites[i].1 - home).length_squared(), i));
                if let Some(i) = best {
                    filled[i] += 1;
                    world
                        .get_mut::<Employment>(worker)
                        .expect("villager")
                        .workplace = Some(sites[i].0);
                    world
                        .get_mut::<Workplace>(sites[i].0)
                        .expect("workplace")
                        .workers
                        .push(worker);
                }
            }
        }

        // Starting stock in the storehouse.
        if let Some(mut store) = world.get_mut::<Stockpile>(layout.storehouse) {
            for (good, per) in &econ.initial_stock {
                store.add(*good, per * residents_total);
            }
        }

        let mut workplaces: Vec<Entity> = layout.workplaces.values().flatten().copied().collect();
        workplaces.sort_unstable_by_key(|e| world.get::<SpawnId>(*e).map_or(u64::MAX, |s| s.0));
        world.entity_mut(*settlement).insert((
            SettlementEconomy {
                storehouse: layout.storehouse,
                homes,
                workplaces,
                population: residents_total,
            },
            FoodSecurity::default(),
            Threat::default(),
        ));
        report.settlements.push((*settlement, residents_total));
    }
    world.insert_resource(report);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apportion_sums_exactly() {
        let v = apportion(100, &[0.4, 0.09, 0.09, 0.08, 0.05, 0.04, 0.07, 0.12, 0.06]);
        assert_eq!(v.iter().sum::<u32>(), 100);
        assert_eq!(v[0], 40);
        assert_eq!(apportion(7, &[1.0, 1.0, 1.0]).iter().sum::<u32>(), 7);
        assert_eq!(apportion(5, &[0.0, 0.0]), vec![0, 0]);
    }
}
