//! Populating settlements at world creation: households, villagers, their
//! occupations and workplaces, and each settlement's buildings.
//!
//! - **Developed world** (M2): one region is populated; `villagers.ron`'s
//!   total is apportioned over its settlements by farmland.
//! - **Run island** (M4, GP-21): every settlement carries a drawn
//!   [`SettlementSize`] and is populated at exactly that many people.

use std::collections::{BTreeMap, BTreeSet};

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
use sim_world::{Mobile, RegionId, RoadNetwork, Settlement, SettlementSize, Terrain, WorldMap};

use crate::components::{
    Activity, CombatStats, DecisionTrace, Employment, Needs, NextDecision, Strike, Threat, Travel,
    Villager,
};
use crate::data::{RegionChoice, Villagers, YearRange};
use crate::food::FoodSecurity;
use crate::loyalty::{Loyalty, SettlementLoyalty};
use crate::standing::{Goodwill, Owner};

/// What population did, for diagnostics and tests.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct PopulationReport {
    /// Regions that were populated, in id order: the configured region in
    /// the developed world, every region with a sized settlement in a run.
    pub regions: Vec<RegionId>,
    /// (settlement, villagers) for each populated settlement.
    pub settlements: Vec<(Entity, u32)>,
    /// Workers moved to another occupation because their workplace
    /// couldn't be built (no coast, no forest, ...).
    pub reassigned: u32,
    /// (settlement, words drawn from its populate stream), in populate
    /// order: specialties change who works where, never how many draws are
    /// made (M4a step 9, BUILD_GUIDE G10; `populate_draws_are_unchanged`).
    pub draws: Vec<(SpawnId, u128)>,
    /// (settlement, its site counts) for each run settlement given a
    /// specialty: reachable tiles of every specialty row's site, and its
    /// road degree (diagnostics; `sim_headless --workshops` prints them).
    pub sites: Vec<(SpawnId, SiteCounts)>,
}

/// What populate counted at a run settlement's site to pick its
/// [`Specialty`] (SIM-62): reachable Upland, Coast and ForestEdge tiles
/// within the specialty rows' radius, and the roads that meet there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SiteCounts {
    /// Hills and mountain tiles.
    pub upland: u32,
    /// Beach or grass next to water.
    pub coast: u32,
    /// Forest next to open land.
    pub forest_edge: u32,
    /// Roads with an end at the settlement.
    pub roads: u32,
}

/// SIM-62 (M4a step 9): the specialty a site gives, from villagers.ron
/// `specialties` (the first matching row), with the occupation shares it
/// scales. `None` when no row matches (or the list is empty).
pub fn specialty_of<'a>(
    rules: &'a [crate::data::SpecialtyRule],
    map: &WorldMap,
    center: IVec2,
    roads: u32,
) -> Option<&'a crate::data::SpecialtyRule> {
    rules.iter().find(|r| {
        let tiles = r.site.map_or(0, |site| {
            sim_economy::site_tiles(map, center, r.radius, site)
        });
        r.matches(tiles, roads)
    })
}

/// Roads in `network` with an end at settlement `id`.
pub fn road_degree(network: Option<&RoadNetwork>, id: SpawnId) -> u32 {
    network.map_or(0, |n| {
        n.roads.iter().filter(|r| r.a == id || r.b == id).count() as u32
    })
}

/// What a run settlement's site makes it (SIM-62, M4a; LORDSHIP_DESIGN
/// section 8.4): read from its site at populate in a run (M4a step 9) and
/// shown on its label and panel. Append only: saves store the position.
#[derive(
    Component,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum Specialty {
    /// Upland ore: miners and smiths; arms.
    Mining,
    /// A crossroads: carters and bakers; hired men.
    Market,
    /// Forest edge: woodcutters; cheap works.
    Timber,
    /// Coast: fishers; a larder.
    Fishing,
    /// Everything else: farmers and bakers; a larder.
    Farming,
}

impl Specialty {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Specialty::Mining => "Mining",
            Specialty::Market => "Market",
            Specialty::Timber => "Timber",
            Specialty::Fishing => "Fishing",
            Specialty::Farming => "Farming",
        }
    }
}

impl sim_core::StateHash for Specialty {
    fn hash_state(&self, h: &mut sim_core::StateHasher) {
        h.u8(*self as u8);
    }
}

sim_core::save_serde!(Specialty);

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

/// M2: splits `total` people over `chosen` (one region), at least
/// `min_per_settlement` each, the rest by farmland: bigger farmland, bigger
/// village.
fn apportion_by_farmland(
    world: &World,
    chosen: &[(SpawnId, Entity, Settlement)],
    total: u32,
    min_per_settlement: u32,
) -> Vec<u32> {
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
    let floor = min_per_settlement.min(total / chosen.len() as u32);
    let extra = total - floor * chosen.len() as u32;
    apportion(extra, &weights)
        .into_iter()
        .map(|n| n + floor)
        .collect()
}

/// Populates the world, once, after world generation: every sized
/// settlement at its [`SettlementSize`] when any has one (the run island,
/// GP-21), otherwise the configured region (M2).
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
    let drawn: Vec<Option<u32>> = settlements
        .iter()
        .map(|(_, e, _)| world.get::<SettlementSize>(*e).map(|s| s.people))
        .collect();
    let mut report = PopulationReport::default();
    let sized = drawn.iter().any(Option::is_some);
    let (chosen, sizes) = if sized {
        // GP-21 (run island): every settlement at its drawn size, in SpawnId
        // order. The total in villagers.ron (and any override) is ignored.
        let (chosen, sizes): (Vec<_>, Vec<_>) = settlements
            .into_iter()
            .zip(drawn)
            .filter_map(|(s, n)| n.filter(|n| *n > 0).map(|n| (s, n)))
            .unzip();
        let regions: BTreeSet<RegionId> = chosen.iter().map(|(_, _, s)| s.region).collect();
        report.regions = regions.into_iter().collect();
        if chosen.is_empty() {
            world.insert_resource(report);
            return;
        }
        (chosen, sizes)
    } else {
        let region = choose_region(&settlements, pop.region);
        let chosen: Vec<(SpawnId, Entity, Settlement)> = settlements
            .into_iter()
            .filter(|(_, _, s)| Some(s.region) == region)
            .collect();
        report.regions = region.into_iter().collect();
        if chosen.is_empty() || pop.total == 0 {
            world.insert_resource(report);
            return;
        }
        let sizes = apportion_by_farmland(world, &chosen, pop.total, pop.min_per_settlement);
        (chosen, sizes)
    };

    let size_weights: Vec<f32> = hh.size_weights.iter().map(|w| *w as f32).collect();
    let size_total: f32 = size_weights.iter().sum();
    let shares: Vec<f32> = Occupation::ALL
        .iter()
        .map(|o| villagers.share(*o))
        .collect();
    // SIM-62 (M4a step 9): in a run, each settlement's specialty, read from
    // its site before anything is built. It scales the shares (so changes
    // how many work each trade and how many workplaces are built), never
    // the number of draws.
    let in_run = sized
        && world
            .get_resource::<sim_core::RunState>()
            .is_some_and(|r| r.in_run());
    let specialties: Vec<Option<(Specialty, Vec<f32>)>> = if in_run {
        let map = world.resource::<WorldMap>();
        let roads = world.get_resource::<RoadNetwork>();
        let rules = &villagers.data.specialties;
        let radius = rules.iter().map(|r| r.radius).max().unwrap_or(0);
        chosen
            .iter()
            .map(|(id, _, site)| {
                let degree = road_degree(roads, *id);
                let rule = specialty_of(rules, map, site.center, degree)?;
                let count = |rule| sim_economy::site_tiles(map, site.center, radius, rule);
                report.sites.push((
                    *id,
                    SiteCounts {
                        upland: count(sim_economy::SiteRule::Upland),
                        coast: count(sim_economy::SiteRule::Coast),
                        forest_edge: count(sim_economy::SiteRule::ForestEdge),
                        roads: degree,
                    },
                ));
                Some((rule.kind, rule.scale(&shares)))
            })
            .collect()
    } else {
        vec![None; chosen.len()]
    };
    let mut occupied = Occupied::default();
    // Every settlement starts independent (GP-05), its villagers at the
    // independent loyalty target (GP-06).
    let start_loyalty = villagers.data.loyalty.independent_target;

    for (((settlement_id, settlement, site), target), specialty) in
        chosen.iter().zip(&sizes).zip(&specialties)
    {
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
        let counts = apportion(workers, specialty.as_ref().map_or(&shares, |s| &s.1));
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
                            Allegiance(FactionId::INDEPENDENT),
                            DetailLevel { full: true },
                            Mobile,
                            civilian_stats(&villagers),
                            Strike::default(),
                            Loyalty(start_loyalty),
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
            Owner::default(),
            Goodwill::default(),
            SettlementLoyalty {
                mean: start_loyalty,
                min: start_loyalty,
                residents: residents_total,
                ..SettlementLoyalty::default()
            },
        ));
        if let Some((kind, _)) = specialty {
            world.entity_mut(*settlement).insert(*kind);
        }
        report.settlements.push((*settlement, residents_total));
        report.draws.push((*settlement_id, rng.get_word_pos()));
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
