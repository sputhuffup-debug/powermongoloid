//! M4a lane B's smithy and edict balance census (docs/m4a/CENSUS.md): what
//! a forge town makes with and without Forge, what Forge and Fields cost in
//! food, what a Tithe hands an army within reach and how big a Levy is, on
//! the run islands of seeds 1 and 7. Ignored: it measures, it asserts
//! nothing but that the runs finish. Run it in release:
//!
//! `cargo test --release -p integration_tests --test census -- --ignored --nocapture`
//!
//! Every independent town but the rival's is handed to the player at tick
//! 0 as an invitation (no fallout, no scar) with its people at loyalty 0.9,
//! the director silenced (`CENSUS_SPECIALTIES=0` lays the island out
//! without specialties, as before step 9); then each variant sets its
//! dials and runs `DAYS` (six) days and an hour:
//!
//! - **owned**: dials at their defaults (the "without Forge" line);
//! - **forge**: Work at Forge everywhere;
//! - **fields**: Work at Fields everywhere;
//! - **tithe**: Forge and Tithe, a player captain with 10 unarmed men
//!   parked 20 tiles from each of the four best forge towns;
//! - **levy**: Levy at the four biggest towns, a captain parked at each.

mod common;

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use common::*;
use sim_agents::{Edicts, Enlisted, FoodSecurity, Loyalty, Needs, SettlementLoyalty, Villager};
use sim_core::{
    Calendar, Cause, Dues, EventFeed, FactionId, SimEventKind, SimStepper, SimTick, WorkFocus,
};
use sim_economy::{
    Economy, GoodId, Inventory, Occupation, SettlementEconomy, Stockpile, Workplace, WorkplaceKind,
};
use sim_military::{ArmySpec, RunProgress, spawn_army, transfer};

/// Days each variant runs.
const DAYS: u64 = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Variant {
    Owned,
    Forge,
    Fields,
    Tithe,
    Levy,
}

/// One town's numbers at a moment.
#[derive(Debug, Clone, Default)]
struct Snap {
    people: u32,
    weapons: u32,
    ore: u32,
    food: f32,
    days: f32,
    hunger: f32,
    starving: u32,
    forge_trades: u32,
    food_trades: u32,
    owner: u16,
}

struct Town {
    e: Entity,
    id: u64,
    centre: IVec2,
    smithies: usize,
    mines: usize,
    specialty: String,
}

fn good(app: &App, name: &str) -> GoodId {
    let econ = app.world().resource::<Economy>();
    GoodId(
        econ.data
            .goods
            .iter()
            .position(|g| g.name == name)
            .expect("good") as u8,
    )
}

fn kinds(app: &App, s: Entity, kind: WorkplaceKind) -> usize {
    let w = app.world();
    w.get::<SettlementEconomy>(s)
        .unwrap()
        .workplaces
        .iter()
        .filter(|&&e| w.get::<Workplace>(e).unwrap().kind == kind)
        .count()
}

fn snap(app: &mut App, t: &Town) -> Snap {
    let weapons = good(app, "weapons");
    let ore_g = good(app, "ore");
    let se = app.world().get::<SettlementEconomy>(t.e).unwrap().clone();
    let (mut n, mut ore) = (0u32, 0u32);
    for b in std::iter::once(se.storehouse)
        .chain(se.homes.iter().copied())
        .chain(se.workplaces.iter().copied())
    {
        n += app
            .world()
            .get::<Stockpile>(b)
            .map_or(0, |s| s.get(weapons));
        ore += app.world().get::<Stockpile>(b).map_or(0, |s| s.get(ore_g));
    }
    let fs = app
        .world()
        .get::<FoodSecurity>(t.e)
        .cloned()
        .unwrap_or_default();
    let w = app.world_mut();
    let mut q =
        w.query_filtered::<(&Villager, &Needs, &Occupation, &Inventory), Without<Enlisted>>();
    let (mut hunger, mut starving, mut forge, mut food, mut people) = (0.0, 0, 0, 0, 0u32);
    for (v, needs, o, inv) in q.iter(w) {
        if v.settlement != t.e {
            continue;
        }
        people += 1;
        hunger += needs.hunger;
        if needs.hunger >= 0.9 {
            starving += 1;
        }
        n += inv.get(weapons) as u32;
        match o {
            Occupation::Smith | Occupation::Miner | Occupation::Woodcutter => forge += 1,
            Occupation::Farmer | Occupation::Fisher | Occupation::Shepherd | Occupation::Baker => {
                food += 1
            }
            _ => {}
        }
    }
    Snap {
        people,
        weapons: n,
        ore,
        food: fs.food_value,
        days: fs.days,
        hunger: hunger / people.max(1) as f32,
        starving,
        forge_trades: forge,
        food_trades: food,
        owner: owner_of(app, t.e).0,
    }
}

fn towns(app: &mut App) -> Vec<Town> {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let mut out = Vec::new();
    for (e, id, centre, _) in villages(app) {
        if Some(id) == seat || owner_of(app, e) != FactionId::INDEPENDENT {
            continue;
        }
        let specialty = app
            .world()
            .get::<sim_agents::Specialty>(e)
            .map_or("-", |s| s.name())
            .to_string();
        out.push(Town {
            e,
            id,
            centre,
            smithies: kinds(app, e, WorkplaceKind::Smithy),
            mines: kinds(app, e, WorkplaceKind::Mine),
            specialty,
        });
    }
    out
}

fn take_all(app: &mut App, towns: &[Town]) {
    for t in towns {
        transfer(app.world_mut(), t.e, FactionId::PLAYER, Cause::Invitation).expect("populated");
    }
    let set: Vec<Entity> = towns.iter().map(|t| t.e).collect();
    let w = app.world_mut();
    let mut q = w.query::<(&Villager, &mut Loyalty)>();
    for (v, mut l) in q.iter_mut(w) {
        if set.contains(&v.settlement) {
            l.0 = 0.9;
        }
    }
    for t in towns {
        w.get_mut::<SettlementLoyalty>(t.e).unwrap().mean = 0.9;
    }
}

fn park(app: &mut App, t: &Town) -> Entity {
    let at = passable_near(app, t.centre + IVec2::new(20, 0)).expect("land");
    spawn_army(
        app.world_mut(),
        &ArmySpec {
            food_days: 1.0,
            ..ArmySpec::new(FactionId::PLAYER, at, 10)
        },
    )
}

struct Outcome {
    /// Per town: snapshots at tick 1 and at each dawn + 1h.
    snaps: BTreeMap<u64, Vec<Snap>>,
    /// Per captain SpawnId: (food value, arms, men) by day.
    tribute: BTreeMap<u64, Vec<(u64, u64, i32)>>,
    /// Captains parked, by town id.
    parked: BTreeMap<u64, u64>,
    /// Mustered waiting at each dawn, by town.
    mustered: BTreeMap<u64, Vec<u32>>,
}

/// The run app; `CENSUS_SPECIALTIES=0` lays the island out as before step 9.
fn app(seed: u64) -> App {
    if std::env::var("CENSUS_SPECIALTIES").is_ok_and(|v| v == "0") {
        run_app_without_specialties(seed)
    } else {
        run_app(seed)
    }
}

fn run(seed: u64, variant: Variant, picks: &[u64]) -> Outcome {
    let mut app = app(seed);
    quiet_director(&mut app);
    let towns = towns(&mut app);
    take_all(&mut app, &towns);
    let mut parked = BTreeMap::new();
    for t in &towns {
        let mut ed = app.world_mut().get_mut::<Edicts>(t.e).unwrap();
        match variant {
            Variant::Owned => {}
            Variant::Forge => ed.order_work(WorkFocus::Forge, true),
            Variant::Fields => ed.order_work(WorkFocus::Fields, true),
            Variant::Tithe => {
                ed.order_work(WorkFocus::Forge, true);
                if picks.contains(&t.id) {
                    ed.order_dues(Dues::Tithe, true);
                }
            }
            Variant::Levy => {
                if picks.contains(&t.id) {
                    ed.order_dues(Dues::Levy, true);
                }
            }
        }
        if matches!(variant, Variant::Tithe | Variant::Levy) && picks.contains(&t.id) {
            let c = park(&mut app, t);
            parked.insert(t.id, spawn_id(&app, c));
        }
    }
    let day = app.world().resource::<Calendar>().ticks_per_day;
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    let mut snaps: BTreeMap<u64, Vec<Snap>> = BTreeMap::new();
    let mut mustered: BTreeMap<u64, Vec<u32>> = BTreeMap::new();
    app.step(1);
    for t in &towns {
        snaps.entry(t.id).or_default().push(snap(&mut app, t));
    }
    for d in 1..=DAYS {
        let now = app.world().resource::<SimTick>().0;
        // Just before the dawn: what is waiting on the green.
        app.step(d * day - 1 - now);
        for t in &towns {
            if picks.contains(&t.id) {
                let n = sim_military::govern::mustered(app.world_mut(), t.e).len() as u32;
                mustered.entry(t.id).or_default().push(n);
            }
        }
        app.step(hour + 1);
        for t in &towns {
            snaps.entry(t.id).or_default().push(snap(&mut app, t));
        }
    }
    let mut tribute: BTreeMap<u64, Vec<(u64, u64, i32)>> = BTreeMap::new();
    for f in app.world().resource::<EventFeed>().iter() {
        if f.event.kind == SimEventKind::Tribute {
            tribute.entry(f.event.subject).or_default().push((
                f.event.other & 0xffff_ffff,
                f.event.other >> 32,
                f.event.amount,
            ));
        }
    }
    Outcome {
        snaps,
        tribute,
        parked,
        mustered,
    }
}

#[test]
#[ignore = "a census: run in release with --nocapture"]
fn edict_census() {
    for seed in [1u64, 7] {
        let mut app = app(seed);
        let list = towns(&mut app);
        drop(app);
        let owned = run(seed, Variant::Owned, &[]);
        let forge = run(seed, Variant::Forge, &[]);
        let fields = run(seed, Variant::Fields, &[]);
        // The four best forge towns, and the four biggest.
        let made = |o: &Outcome, id: u64| {
            let s = &o.snaps[&id];
            s[L].weapons as i64 - s[0].weapons as i64
        };
        let mut best: Vec<u64> = list.iter().map(|t| t.id).collect();
        best.sort_by_key(|id| (-made(&forge, *id), *id));
        best.truncate(4);
        let mut big: Vec<(u32, u64)> = list
            .iter()
            .map(|t| (owned.snaps[&t.id][0].people, t.id))
            .collect();
        big.sort_by_key(|(p, id)| (std::cmp::Reverse(*p), *id));
        let big: Vec<u64> = big.into_iter().take(4).map(|(_, id)| id).collect();
        let tithe = run(seed, Variant::Tithe, &best);
        let levy = run(seed, Variant::Levy, &big);

        const L: usize = DAYS as usize;
        println!("\n== seed {seed}: {} towns, {DAYS} days ==", list.len());
        println!(
            "town  spec     ppl smy mine | weapons by the end: owned forge fields | food value d0 -> end (days at end), hunger end: owned | forge | fields | forge trades owned->forge, food trades owned->fields | owners at end"
        );
        let (mut tw_o, mut tw_f) = (0i64, 0i64);
        for t in &list {
            let (o, f, fl) = (
                &owned.snaps[&t.id],
                &forge.snaps[&t.id],
                &fields.snaps[&t.id],
            );
            tw_o += made(&owned, t.id);
            tw_f += made(&forge, t.id);
            println!(
                "#{:<4} {:<8} {:>3} {:>3} {:>4} | {:>5} {:>5} {:>6} | {:>5.0}->{:>5.0} ({:>4.1}) h{:.2} | {:>5.0} ({:>4.1}) h{:.2} | {:>5.0} ({:>4.1}) h{:.2} | {}->{}, {}->{} | {} {} {}",
                t.id,
                t.specialty,
                o[0].people,
                t.smithies,
                t.mines,
                made(&owned, t.id),
                made(&forge, t.id),
                made(&fields, t.id),
                o[0].food,
                o[L].food,
                o[L].days,
                o[L].hunger,
                f[L].food,
                f[L].days,
                f[L].hunger,
                fl[L].food,
                fl[L].days,
                fl[L].hunger,
                o[L].forge_trades,
                f[L].forge_trades,
                o[L].food_trades,
                fl[L].food_trades,
                o[L].owner,
                f[L].owner,
                fl[L].owner,
            );
        }
        println!("forge, by day (weapons made that day / ore left in the town at the dawn):");
        for t in &list {
            let f = &forge.snaps[&t.id];
            let per: Vec<String> = (1..=L)
                .map(|d| {
                    format!(
                        "{}/{}",
                        f[d].weapons as i64 - f[d - 1].weapons as i64,
                        f[d].ore
                    )
                })
                .collect();
            println!("  #{:<4} {:<8} {}", t.id, t.specialty, per.join(" "));
        }
        let total = |o: &Outcome, i: usize, f: fn(&Snap) -> f32| -> f32 {
            o.snaps.values().map(|s| f(&s[i])).sum()
        };
        let starving = |o: &Outcome| o.snaps.values().map(|s| s[L].starving).sum::<u32>();
        for (name, o) in [("owned", &owned), ("forge", &forge), ("fields", &fields)] {
            println!(
                "{name:>6}: food value d0 {:.0} -> end {:.0}; mean hunger end {:.3}; starving (>=0.9) end {}",
                total(o, 0, |s| s.food),
                total(o, 3, |s| s.food),
                total(o, 3, |s| s.hunger) / o.snaps.len() as f32,
                starving(o)
            );
        }
        println!("weapons by the end, island: owned {tw_o}, forge {tw_f}");
        println!("-- tithe (Forge + Tithe, a captain of 10 parked 20 tiles off) --");
        for id in &best {
            let cap = tithe.parked[id];
            let days = tithe.tribute.get(&cap).cloned().unwrap_or_default();
            let s = &tithe.snaps[id];
            println!(
                "#{id}: made under forge {}, tribute lines (food value, arms, men): {:?}; town food end {:.0} ({:.1} days), hunger {:.2}",
                made(&forge, *id),
                days,
                s[L].food,
                s[L].days,
                s[L].hunger
            );
        }
        println!("-- levy (a captain parked at each) --");
        for id in &big {
            let cap = levy.parked[id];
            let days = levy.tribute.get(&cap).cloned().unwrap_or_default();
            let men: i32 = days.iter().map(|d| d.2).sum();
            println!(
                "#{id}: {} people; mustered before each dawn {:?}; men sent {men} in {} lines; people end {}",
                levy.snaps[id][0].people,
                levy.mustered.get(id).cloned().unwrap_or_default(),
                days.len(),
                levy.snaps[id][L].people
            );
        }
    }
}
