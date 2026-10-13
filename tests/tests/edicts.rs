//! M4a step 4 scenario `edicts` (GP-34, GP-17 part; ADR 0020,
//! LORDSHIP_DESIGN section 8.1): a town you rule does what you say. The
//! player sets a dial with `CommandKind::Govern`; the next hourly
//! `govern::step` (progress slot 15) applies it, locks it for two days and
//! sends the first wave, later waves go at each dawn; tithes and levies
//! reach armies in `orders::lands`.
//!
//! - `the_first_wave_moves_within_the_hour`: Forge set just after an hour
//!   is applied by the next hour step, with the dial's `since`, an
//!   `EdictSet` in the feed and the first moved villagers.
//! - `forge_moves_smiths_and_flips_recipes`: every smithy runs weapons from
//!   the first hour, forge trades rise wave by wave within the caps, and
//!   Balanced (after the lock) restores the layout and sends the moved back.
//! - `fields_moves_workers_to_food`: Fields moves its donors to food.
//! - `walls_raises_haulers_and_the_watch`: carters, the 1.25x threat radius
//!   and the 1.5x build speed.
//! - `a_tithe_reaches_an_army_within_reach`: food, arms and a Tribute for a
//!   captain within 60 tiles; nothing beyond.
//! - `a_levy_musters_and_joins`: 2% muster on the green and wait, then join
//!   the captain who comes; the population drops.
//! - `rationing_slows_eating_by_a_quarter`: hunger at 0.75x, honest days.
//! - `a_feast_gathers_and_pays_if_fed`: the meal, the green, the Festival
//!   deed and the Table term; refused when the town is short of food.
//! - `dials_lock`: two days of lock, "not yet" for the later acts.
//! - `defaults_are_no_ops`: a run with the dials at their defaults hashes as
//!   one with no edicts at all.
//!
//! Each test prints the numbers it checks.

mod common;

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use common::*;
use sim_agents::{
    ActionKind, Activity, Assignment, Edicts, Enlisted, FoodSecurity, Goodwill, Loyalty,
    LoyaltyTerm, LoyaltyTerms, Mustered, Needs, Rationed, SettlementLoyalty, Threat, Villager,
    Villagers,
};
use sim_core::{
    Calendar, Cause, CommandKind, Dial, Dues, EventFeed, FactionId, GovernAct, Posture,
    SimEventKind, SimStepper, SimTick, SpawnId, Table, WorkFocus,
};
use sim_economy::{
    Economy, GoodId, Occupation, RecipeId, SettlementEconomy, Stockpile, Workplace, WorkplaceKind,
    layout_recipe,
};
use sim_military::{
    Army, ArmySpec, Captain, Equipment, Follower, Progression, RunProgress, govern, spawn_army,
    transfer,
};

const SEED: u64 = 42;

/// A town picked for a scenario.
#[derive(Debug, Clone, Copy)]
struct Town {
    e: Entity,
    id: u64,
    centre: IVec2,
}

fn tick(app: &App) -> u64 {
    app.world().resource::<SimTick>().0
}

fn day(app: &App) -> u64 {
    app.world().resource::<Calendar>().ticks_per_day
}

fn step_to(app: &mut App, t: u64) {
    let now = tick(app);
    assert!(t >= now, "already past tick {t} (at {now})");
    app.step(t - now);
}

/// Steps to the next tick that is `offset` into an hour (hours are 100
/// ticks), at least one tick on.
fn step_to_hour_offset(app: &mut App, offset: u64) {
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    let now = tick(app);
    let mut t = now - now % hour + offset;
    if t <= now {
        t += hour;
    }
    step_to(app, t);
}

/// Steps past the next hour tick (the hourly step has run, and so has the
/// lands pass of the tick after it).
fn step_past_hour(app: &mut App) {
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    let t = (tick(app) / hour + 1) * hour;
    step_to(app, t + 3);
}

/// Steps to the next day tick (a dawn), then one tick on.
fn step_past_dawn(app: &mut App) {
    let d = day(app);
    let t = (tick(app) / d + 1) * d;
    step_to(app, t + 3);
}

/// A fresh run of the seed, director silenced.
fn fresh() -> App {
    let mut app = run_app(SEED);
    quiet_director(&mut app);
    app
}

fn smithies(app: &App, s: Entity) -> Vec<Entity> {
    let w = app.world();
    w.get::<SettlementEconomy>(s)
        .unwrap()
        .workplaces
        .iter()
        .copied()
        .filter(|&e| w.get::<Workplace>(e).unwrap().kind == WorkplaceKind::Smithy)
        .collect()
}

/// The first populated free town in SpawnId order that is not the rival's
/// seat, has at least `people` people and `smithies` smithies, and is
/// farther than `away` tiles from every player captain (so the start band
/// is nobody's collector).
fn pick(app: &mut App, people: u32, min_smithies: usize, away: f32) -> Town {
    let seat = app.world().resource::<RunProgress>().rival_seat;
    let mut q = app.world_mut().query::<(&Captain, &sim_core::Position)>();
    let band: Vec<Vec2> = q
        .iter(app.world())
        .filter(|(c, _)| c.faction == FactionId::PLAYER)
        .map(|(_, p)| p.0.as_vec2())
        .collect();
    for (e, id, centre, pop) in villages(app) {
        if Some(id) == seat
            || pop < people
            || owner_of(app, e) != FactionId::INDEPENDENT
            || smithies(app, e).len() < min_smithies
            || band.iter().any(|b| b.distance(centre.as_vec2()) <= away)
        {
            continue;
        }
        return Town { e, id, centre };
    }
    panic!("no town with {people} people, {min_smithies} smithies, {away} tiles from the band");
}

/// Hands `t` to the player and makes its people loyal (a conquest's scar
/// must not decide a scenario about something else).
fn take(app: &mut App, t: Town, cause: Cause) {
    transfer(app.world_mut(), t.e, FactionId::PLAYER, cause).expect("populated");
    let w = app.world_mut();
    let mut q = w.query::<(&Villager, &mut Loyalty)>();
    for (v, mut l) in q.iter_mut(w) {
        if v.settlement == t.e {
            l.0 = 0.9;
        }
    }
    w.get_mut::<SettlementLoyalty>(t.e).unwrap().mean = 0.9;
}

fn best_food(app: &App) -> GoodId {
    *app.world()
        .resource::<Economy>()
        .foods_by_value()
        .last()
        .expect("a food")
}

fn daily_need(app: &App, people: u32) -> f32 {
    app.world()
        .resource::<Villagers>()
        .data
        .needs
        .hunger_per_day
        * people as f32
}

fn people(app: &App, t: Town) -> u32 {
    app.world()
        .get::<SettlementEconomy>(t.e)
        .unwrap()
        .population
}

/// Puts `days` of the town's need into its storehouse (or, when `store` is
/// false, its homes), one food at a time.
fn stock_food(app: &mut App, t: Town, days: f32, store: bool) {
    let units_value = days * daily_need(app, people(app, t));
    let food = best_food(app);
    let value = app.world().resource::<Economy>().food_value(food);
    let units = (units_value / value).ceil() as u32;
    let se = app.world().get::<SettlementEconomy>(t.e).unwrap().clone();
    let places: Vec<Entity> = if store { vec![se.storehouse] } else { se.homes };
    let mut placed = 0;
    'fill: while placed < units {
        let before = placed;
        for &p in &places {
            if placed == units {
                break 'fill;
            }
            if let Some(mut st) = app.world_mut().get_mut::<Stockpile>(p) {
                placed += st.add(food, 1);
            }
        }
        assert!(placed > before, "no room for food");
    }
}

fn good(app: &App, name: &str) -> GoodId {
    let econ = app.world().resource::<Economy>();
    GoodId(
        econ.data
            .goods
            .iter()
            .position(|g| g.name == name)
            .unwrap_or_else(|| panic!("no good {name}")) as u8,
    )
}

fn store_of(app: &App, t: Town) -> Entity {
    app.world()
        .get::<SettlementEconomy>(t.e)
        .unwrap()
        .storehouse
}

fn stock_of(app: &App, e: Entity, g: GoodId) -> u32 {
    app.world().get::<Stockpile>(e).unwrap().get(g)
}

fn govern_cmd(app: &mut App, t: Town, act: GovernAct) {
    command(
        app,
        FactionId::PLAYER.0,
        CommandKind::Govern {
            settlement: t.id,
            act,
        },
    );
}

fn edicts(app: &App, t: Town) -> Edicts {
    app.world().get::<Edicts>(t.e).expect("a run town").clone()
}

/// Residents of `t` (not enlisted) with their trade.
fn trades(app: &mut App, t: Town) -> Vec<(Entity, Occupation)> {
    let w = app.world_mut();
    let mut q = w.query_filtered::<(Entity, &SpawnId, &Villager, &Occupation), Without<Enlisted>>();
    let mut v: Vec<_> = q
        .iter(w)
        .filter(|(_, _, vil, _)| vil.settlement == t.e)
        .map(|(e, id, _, o)| (*id, e, *o))
        .collect();
    v.sort_unstable_by_key(|x| x.0);
    v.into_iter().map(|(_, e, o)| (e, o)).collect()
}

fn count_trades(app: &mut App, t: Town, f: impl Fn(Occupation) -> bool) -> u32 {
    trades(app, t).iter().filter(|(_, o)| f(*o)).count() as u32
}

fn is_forge(o: Occupation) -> bool {
    matches!(
        o,
        Occupation::Smith | Occupation::Miner | Occupation::Woodcutter
    )
}

fn is_food(o: Occupation) -> bool {
    matches!(
        o,
        Occupation::Farmer | Occupation::Fisher | Occupation::Shepherd | Occupation::Baker
    )
}

/// Residents of `t` carrying an `Assignment`, in SpawnId order.
fn assigned(app: &mut App, t: Town) -> Vec<(Entity, Assignment)> {
    let w = app.world_mut();
    let mut q = w.query::<(Entity, &SpawnId, &Villager, &Assignment)>();
    let mut v: Vec<_> = q
        .iter(w)
        .filter(|(_, _, vil, _)| vil.settlement == t.e)
        .map(|(e, id, _, a)| (*id, e, *a))
        .collect();
    v.sort_unstable_by_key(|x| x.0);
    v.into_iter().map(|(_, e, a)| (e, a)).collect()
}

/// The most a wave moves at a town of `pop` people (workers are fewer).
fn wave_cap(app: &App, pop: u32) -> u32 {
    let data = app.world().resource::<Progression>().data.edicts;
    govern::wave_size(&data, pop)
}

/// The weaponsmithing recipe.
fn weapon_recipe(app: &App) -> RecipeId {
    let econ = app.world().resource::<Economy>();
    let weapons = econ
        .data
        .goods
        .iter()
        .position(|g| g.name == "weapons")
        .expect("weapons");
    econ.recipes_for(WorkplaceKind::Smithy)
        .into_iter()
        .find(|r| {
            econ.recipe(*r)
                .outputs
                .iter()
                .any(|(g, _)| g.0 as usize == weapons)
        })
        .expect("a weapons recipe")
}

fn recipes_of(app: &App, smithies: &[Entity]) -> Vec<Option<RecipeId>> {
    smithies
        .iter()
        .map(|&e| app.world().get::<Workplace>(e).unwrap().recipe)
        .collect()
}

fn layout_recipes(app: &App, n: usize) -> Vec<Option<RecipeId>> {
    let recipes = app
        .world()
        .resource::<Economy>()
        .recipes_for(WorkplaceKind::Smithy);
    (0..n).map(|i| layout_recipe(&recipes, i)).collect()
}

/// Feed entries of `kind` about settlement `subject`, oldest first.
fn feed(app: &App, kind: SimEventKind, subject: u64) -> Vec<sim_core::FeedEntry> {
    app.world()
        .resource::<EventFeed>()
        .iter()
        .filter(|f| f.event.kind == kind && f.event.subject == subject)
        .cloned()
        .collect()
}

/// A scenario town at full tilt: free, 60+ people, a smithy, ample food.
fn forge_town(app: &mut App) -> Town {
    let t = pick(app, 60, 2, 0.0);
    take(app, t, Cause::Invitation);
    stock_food(app, t, 4.0, true);
    t
}

/// GP-34: Forge set just after an hour tick is applied by the next hourly
/// step. The dial turns at that hour (`since`, the lock two days on), an
/// `EdictSet` for the Work dial reaches the feed, and the first wave has
/// moved villagers who carry an `Assignment` naming Forge, no more than a
/// wave allows.
#[test]
fn the_first_wave_moves_within_the_hour() {
    let mut app = fresh();
    let t = forge_town(&mut app);
    step_to_hour_offset(&mut app, 2);
    let sent = tick(&app);
    let before = count_trades(&mut app, t, is_forge);
    govern_cmd(&mut app, t, GovernAct::Work(WorkFocus::Forge));
    app.step(1);
    assert_eq!(edicts(&app, t).pending_work, Some(WorkFocus::Forge));
    assert_eq!(
        edicts(&app, t).work,
        WorkFocus::Balanced,
        "not before the hour"
    );
    step_to(&mut app, sent + 200);
    let e = edicts(&app, t);
    let since = e.dial(Dial::Work).since;
    let hour = app.world().resource::<Calendar>().ticks_per_hour();
    println!(
        "town #{} ({} people): Forge sent at tick {sent}, applied at {since}, locked until {}",
        t.id,
        people(&app, t),
        e.dial(Dial::Work).locked_until
    );
    assert_eq!(e.work, WorkFocus::Forge);
    assert!(e.dial(Dial::Work).by_player);
    assert!(
        since > sent && since <= sent + hour,
        "applied at the next hour"
    );
    assert_eq!(since % hour, 0);
    assert_eq!(
        e.dial(Dial::Work).locked_until,
        since + 2 * day(&app),
        "two days of lock from that hour"
    );
    let told = feed(&app, SimEventKind::EdictSet, t.id);
    assert_eq!(told.len(), 1, "one EdictSet: {told:?}");
    assert_eq!(told[0].tick, since);
    assert_eq!(told[0].event.other, Dial::Work.tag() as u64);
    assert_eq!(told[0].event.amount, WorkFocus::Forge.tag() as i32);
    assert_eq!(told[0].event.cause, Cause::Player);

    let moved = assigned(&mut app, t);
    let after = count_trades(&mut app, t, is_forge);
    println!(
        "  {} moved in the first wave (cap {}), forge trades {before} -> {after}",
        moved.len(),
        wave_cap(&app, people(&app, t))
    );
    assert!(!moved.is_empty(), "the first wave moves someone");
    assert!(moved.len() as u32 <= wave_cap(&app, people(&app, t)));
    for (_, a) in &moved {
        assert_eq!(a.focus, WorkFocus::Forge);
        assert_eq!(a.since, since, "all in the hour's wave");
        assert!(!a.by_steward);
    }
    assert!(after > before, "forge trades rose");
}

/// GP-34: Forge flips every smithy to weaponsmithing in the first hour,
/// then forge trades (Smith, Miner, Woodcutter) rise wave by wave at each
/// dawn within the wave cap and the 20% cap. After the lock, Balanced puts
/// the smithies back on the layout's alternation and the assigned drift
/// back to their trades at the next waves.
#[test]
fn forge_moves_smiths_and_flips_recipes() {
    let mut app = fresh();
    let t = forge_town(&mut app);
    let smiths = smithies(&app, t.e);
    let weapons = weapon_recipe(&app);
    let layout = layout_recipes(&app, smiths.len());
    println!(
        "town #{}: {} smithies, layout {layout:?}",
        t.id,
        smiths.len()
    );
    assert_eq!(recipes_of(&app, &smiths), layout, "the layout to begin");
    assert!(
        layout.iter().any(|r| *r != Some(weapons)),
        "at least one smithy makes something else to begin with"
    );
    let base = count_trades(&mut app, t, is_forge);
    let workers = count_trades(&mut app, t, |_| true);
    // Free places at the smithies, the mine and the woodcutters' camp.
    let room: u32 = {
        let w = app.world();
        let econ = w.resource::<Economy>();
        w.get::<SettlementEconomy>(t.e)
            .unwrap()
            .workplaces
            .iter()
            .filter_map(|e| w.get::<Workplace>(*e))
            .filter(|wp| {
                matches!(
                    wp.kind,
                    WorkplaceKind::Smithy | WorkplaceKind::Mine | WorkplaceKind::WoodcutterCamp
                )
            })
            .map(|wp| {
                econ.workplace(wp.kind)
                    .slots
                    .saturating_sub(wp.workers.len() as u32)
            })
            .sum()
    };

    step_to_hour_offset(&mut app, 2);
    govern_cmd(&mut app, t, GovernAct::Work(WorkFocus::Forge));
    step_past_hour(&mut app);
    let applied = edicts(&app, t).dial(Dial::Work).since;
    assert!(applied > 0);
    assert_eq!(
        recipes_of(&app, &smiths),
        vec![Some(weapons); smiths.len()],
        "every smithy makes weapons from the first hour"
    );
    let mut series = vec![count_trades(&mut app, t, is_forge)];
    for _ in 0..3 {
        step_past_dawn(&mut app);
        series.push(count_trades(&mut app, t, is_forge));
    }
    println!("  forge trades (base {base}): first hour and three dawns {series:?}");
    assert!(series[0] > base, "the first wave moved smiths");
    assert!(series.windows(2).all(|w| w[1] >= w[0]), "never fewer");
    let cap = (workers as f32 * 0.2).floor() as u32;
    let gain = *series.last().unwrap() - base;
    println!(
        "  room for {room} more forge hands, 20% cap leaves {}; gained {gain}",
        cap.saturating_sub(base)
    );
    assert_eq!(
        gain,
        room.min(cap.saturating_sub(base)),
        "forge hands rise until the places or the 20% cap run out"
    );
    // Each wave stays within its cap.
    let mut by_wave = std::collections::BTreeMap::<u64, u32>::new();
    for (_, a) in assigned(&mut app, t) {
        *by_wave.entry(a.since).or_default() += 1;
    }
    println!("  moves per wave tick: {by_wave:?}");
    let cap = wave_cap(&app, people(&app, t));
    assert!(by_wave.values().all(|&n| n <= cap), "waves within {cap}");
    assert_eq!(
        recipes_of(&app, &smiths),
        vec![Some(weapons); smiths.len()],
        "still weapons"
    );
    let peak = assigned(&mut app, t).len();

    // After the lock, Balanced.
    let unlock = edicts(&app, t).dial(Dial::Work).locked_until;
    if tick(&app) <= unlock {
        step_to(&mut app, unlock + 1);
    }
    let before_rejects = rejected(&app, FactionId::PLAYER);
    govern_cmd(&mut app, t, GovernAct::Work(WorkFocus::Balanced));
    step_past_hour(&mut app);
    assert_eq!(
        rejected(&app, FactionId::PLAYER),
        before_rejects,
        "accepted"
    );
    assert_eq!(edicts(&app, t).work, WorkFocus::Balanced);
    assert_eq!(
        recipes_of(&app, &smiths),
        layout,
        "back on the layout's alternation"
    );
    let mut left = vec![assigned(&mut app, t).len()];
    for _ in 0..3 {
        step_past_dawn(&mut app);
        left.push(assigned(&mut app, t).len());
    }
    let origins: Vec<Occupation> = assigned(&mut app, t).iter().map(|(_, a)| a.from).collect();
    println!(
        "  assigned {peak} at the peak, then after Balanced {left:?}; still assigned from {origins:?}"
    );
    assert!(left.windows(2).all(|w| w[1] <= w[0]), "never more");
    assert!(*left.last().unwrap() < peak, "the assigned drift back");
}

/// GP-34: Fields moves its donors (idle adults, then the extra forge hands
/// and the carters beyond 8%) to farms, fishing spots, pastures and the
/// bakery, wave by wave, never above 85% of the workers in food. Every
/// moved villager carries `Assignment { focus: Fields }` and a food trade;
/// his `from` is one of the donor trades.
#[test]
fn fields_moves_workers_to_food() {
    let mut app = fresh();
    let t = pick(&mut app, 60, 1, 0.0);
    take(&mut app, t, Cause::Invitation);
    stock_food(&mut app, t, 4.0, true);
    // The default layout has few idle adults; free a handful of farmers
    // and smiths into idleness so there are donors (the scenario's setup,
    // through the same API the edicts use).
    let mut freed = 0;
    for (e, o) in trades(&mut app, t) {
        if freed < 6
            && matches!(
                o,
                Occupation::Farmer | Occupation::Smith | Occupation::Miner
            )
            && sim_agents::jobs::reassign(app.world_mut(), e, Occupation::Idle)
        {
            freed += 1;
        }
    }
    let idle0 = count_trades(&mut app, t, |o| o == Occupation::Idle);
    let food0 = count_trades(&mut app, t, is_food);
    let carters0 = count_trades(&mut app, t, |o| o == Occupation::Carter);
    let forge0 = count_trades(&mut app, t, is_forge);
    println!(
        "town #{}: idle {idle0}, food {food0}, carters {carters0}, forge {forge0}",
        t.id
    );
    assert!(idle0 >= 6);

    step_to_hour_offset(&mut app, 2);
    govern_cmd(&mut app, t, GovernAct::Work(WorkFocus::Fields));
    step_past_hour(&mut app);
    let first = count_trades(&mut app, t, is_food);
    step_past_dawn(&mut app);
    step_past_dawn(&mut app);
    let later = count_trades(&mut app, t, is_food);
    let idle1 = count_trades(&mut app, t, |o| o == Occupation::Idle);
    println!(
        "  food trades {food0} -> {first} (first hour) -> {later} (two dawns), idle {idle0} -> {idle1}"
    );
    assert!(first > food0, "the first wave moved someone to food");
    assert!(later >= first && later > food0);
    assert!(idle1 < idle0, "the donors were the idle");
    let workers = count_trades(&mut app, t, |_| true);
    assert!(later as f32 <= 0.85 * workers as f32, "within the 85% cap");
    let moved = assigned(&mut app, t);
    let cap = wave_cap(&app, people(&app, t));
    let mut by_wave = std::collections::BTreeMap::<u64, u32>::new();
    for (e, a) in &moved {
        assert_eq!(a.focus, WorkFocus::Fields);
        assert!(is_food(*app.world().get::<Occupation>(*e).unwrap()));
        assert!(
            matches!(a.from, Occupation::Idle | Occupation::Carter) || is_forge(a.from),
            "donor trade {:?}",
            a.from
        );
        *by_wave.entry(a.since).or_default() += 1;
    }
    println!("  moves per wave tick: {by_wave:?}");
    assert!(by_wave.values().all(|&n| n <= cap));
}

/// GP-34: Walls raises carters (idle, then farmers while the town has a
/// day of food), up to six in all, for the building sites; the town's
/// build speed reads 1.5 (1.0 otherwise); and its threat radius is 1.25x
/// (45 -> 56.25): an army 50 tiles off frightens a walled town and not the
/// same town unwalled.
#[test]
fn walls_raises_haulers_and_the_watch() {
    let mut app = fresh();
    let t = pick(&mut app, 60, 1, 0.0);
    take(&mut app, t, Cause::Invitation);
    stock_food(&mut app, t, 4.0, true);
    let data = app.world().resource::<Progression>().data.edicts;
    assert_eq!(govern::build_speed(app.world(), t.e), 1.0);

    // An army of the rival's, 50 tiles from the centre.
    let radius = app.world().resource::<Army>().data.threat.radius;
    let want = t.centre + IVec2::new(51, 0);
    let at = passable_near(&app, want).expect("a place to stand");
    let foe = spawn_army(
        app.world_mut(),
        &ArmySpec {
            posture: Posture::Aggressive,
            ..ArmySpec::new(FactionId::RIVAL, at, 5)
        },
    );
    app.step(2);
    let d = pos_of(&app, foe).distance(t.centre.as_vec2() + Vec2::splat(0.5));
    let level = |app: &App| app.world().get::<Threat>(t.e).unwrap().level;
    println!(
        "foe {d:.1} tiles off; radius {radius} (walled {}); level before {}",
        radius * data.walls_threat,
        level(&app)
    );
    assert!(
        d > radius && d < radius * data.walls_threat,
        "the foe stands between the two radii: {d}"
    );
    assert_eq!(
        level(&app),
        0.0,
        "unwalled, a foe at {d:.1} is beyond the radius"
    );

    let carters0 = count_trades(&mut app, t, |o| o == Occupation::Carter);
    step_to_hour_offset(&mut app, 2);
    govern_cmd(&mut app, t, GovernAct::Work(WorkFocus::Walls));
    step_past_hour(&mut app);
    for _ in 0..4 {
        step_past_dawn(&mut app);
    }
    let carters1 = count_trades(&mut app, t, |o| o == Occupation::Carter);
    let raised = assigned(&mut app, t)
        .iter()
        .filter(|(_, a)| a.focus == WorkFocus::Walls)
        .count();
    println!(
        "  carters {carters0} -> {carters1}, {raised} raised; level now {}",
        level(&app)
    );
    assert!(carters1 > carters0, "the walls need haulers");
    assert!(raised as u32 <= data.walls_carters, "at most six");
    assert_eq!(govern::build_speed(app.world(), t.e), data.walls_build);
    assert_eq!(data.walls_build, 1.5);
    // The same foe (it has not moved far) now frightens the walled town.
    let d = pos_of(&app, foe).distance(t.centre.as_vec2() + Vec2::splat(0.5));
    let near =
        ((radius * data.walls_threat - d) / (radius * data.walls_threat * 0.6)).clamp(0.0, 1.0);
    let expect = app
        .world()
        .resource::<Army>()
        .threat_of(Posture::Aggressive)
        * near;
    println!("  foe now {d:.1} tiles off; expected level {expect:.3}");
    if d < radius * data.walls_threat && d > radius {
        assert!(level(&app) > 0.0, "the walled watch sees it");
        assert!((level(&app) - expect).abs() < 0.05);
    }
}

/// A scenario for the tithe: the first quiet town with the player's
/// captain `distance` tiles from its centre. Returns the app, the town
/// and the captain.
fn tithe_run(distance: i32) -> (App, Town, Entity) {
    let mut app = fresh();
    let t = pick(&mut app, 60, 1, 70.0);
    take(&mut app, t, Cause::Invitation);
    stock_food(&mut app, t, 4.0, true);
    let weapons = good(&app, "weapons");
    let tools = good(&app, "tools");
    let store = store_of(&app, t);
    {
        let mut st = app.world_mut().get_mut::<Stockpile>(store).unwrap();
        st.add(weapons, 6);
        st.add(tools, 4);
    }
    let at = passable_near(&app, t.centre + IVec2::new(distance, 0)).expect("ground");
    let cap = spawn_army(
        app.world_mut(),
        &ArmySpec {
            food_days: 0.5,
            ..ArmySpec::new(FactionId::PLAYER, at, 6)
        },
    );
    (app, t, cap)
}

/// GP-34: a Tithe feeds and arms the nearest player captain within 60
/// tiles with baggage room: the food beyond half a day of the town's need
/// and the storehouse's weapons and tools, the soldiers armed from the
/// baggage, one `Tribute` naming the captain. A captain 80 tiles off gets
/// nothing, no `Tribute`, and the arms stay in the storehouse.
#[test]
fn a_tithe_reaches_an_army_within_reach() {
    let (mut app, t, cap) = tithe_run(30);
    let weapons = good(&app, "weapons");
    let tools = good(&app, "tools");
    let store = store_of(&app, t);
    let capid = spawn_id(&app, cap);
    let econ = app.world().resource::<Economy>().clone();
    let food_of = |app: &App, e: Entity| app.world().get::<Stockpile>(e).unwrap().food_value(&econ);
    let reach = pos_of(&app, cap).distance(t.centre.as_vec2());
    println!("tithe: captain {reach:.1} tiles off");
    assert!(reach <= 60.0);
    let bag0 = food_of(&app, cap);
    let arms0 = (stock_of(&app, store, weapons), stock_of(&app, store, tools));
    let store0 = food_of(&app, store);
    step_to_hour_offset(&mut app, 2);
    govern_cmd(&mut app, t, GovernAct::Dues(Dues::Tithe));
    step_past_hour(&mut app);
    let bag1 = food_of(&app, cap);
    let arms1 = (stock_of(&app, store, weapons), stock_of(&app, store, tools));
    let armed = {
        let w = app.world_mut();
        let mut q = w.query::<(&Follower, &Equipment)>();
        q.iter(w)
            .filter(|(f, eq)| f.captain == cap && eq.weapon)
            .count()
    };
    println!(
        "  food: baggage {bag0:.0} -> {bag1:.0}, store {store0:.0} -> {:.0}; arms in store {arms0:?} -> {arms1:?}; {armed} soldiers armed",
        food_of(&app, store)
    );
    assert!(bag1 > bag0 + 1.0, "the baggage was fed");
    assert!(food_of(&app, store) < store0, "from the storehouse");
    assert_eq!(arms1, (0, 0), "the storehouse's arms went with it");
    assert!(armed >= 1, "followers armed from the baggage");
    let tribute = feed(&app, SimEventKind::Tribute, capid);
    println!(
        "  Tribute: {:?}",
        tribute.iter().map(|f| f.event).collect::<Vec<_>>()
    );
    assert_eq!(tribute.len(), 1);
    assert_eq!(tribute[0].event.faction, FactionId::PLAYER);
    assert_eq!(tribute[0].event.amount, 0, "no men");
    assert!(tribute[0].event.other >> 32 > 0, "arms counted");
    assert!(tribute[0].event.other & 0xffff_ffff > 0, "food counted");
    assert_eq!(edicts(&app, t).dues, Dues::Tithe, "the Tithe stands");
    // It was the hour the dial turned.
    assert_eq!(tribute[0].tick, edicts(&app, t).dial(Dial::Dues).since + 1);

    // Out of reach.
    let (mut far, ft, fcap) = tithe_run(80);
    let fstore = store_of(&far, ft);
    let reach = pos_of(&far, fcap).distance(ft.centre.as_vec2());
    println!("  far: captain {reach:.1} tiles off");
    assert!(reach > 60.0);
    let far_arms0 = (
        stock_of(&far, fstore, weapons),
        stock_of(&far, fstore, tools),
    );
    step_to_hour_offset(&mut far, 2);
    govern_cmd(&mut far, ft, GovernAct::Dues(Dues::Tithe));
    step_past_dawn(&mut far);
    let far_arms1 = (
        stock_of(&far, fstore, weapons),
        stock_of(&far, fstore, tools),
    );
    println!("  arms in the far store {far_arms0:?} -> {far_arms1:?}");
    assert_eq!(edicts(&far, ft).dues, Dues::Tithe);
    // (The town's smithies go on making arms: only a gain is allowed.)
    assert!(
        far_arms1.0 >= far_arms0.0 && far_arms1.1 >= far_arms0.1,
        "nothing beyond 60 tiles"
    );
    assert!(
        feed(&far, SimEventKind::Tribute, spawn_id(&far, fcap)).is_empty(),
        "no Tribute for a captain out of reach"
    );
}

/// GP-34: a Levy marks 2% of the people (at least one) `Mustered` in the
/// hour it is set, they never work and wait on the green, a dawn musters
/// another 2% (10% at most waiting), and when a player captain comes
/// within 60 tiles the next lands pass enlists them all as his followers:
/// the town's population drops, the marks go, a `Tribute` counts the men.
/// A conquered town's Levy term is the harsh -0.12.
#[test]
fn a_levy_musters_and_joins() {
    let mut app = fresh();
    let t = pick(&mut app, 60, 1, 70.0);
    take(&mut app, t, Cause::Conquest { captain: 1 });
    stock_food(&mut app, t, 4.0, true);
    let pop = people(&app, t);
    let data = app.world().resource::<Progression>().data.edicts;
    let (per, cap) = govern::levy_sizes(&data.levy, pop);
    let waiting = |app: &mut App| -> Vec<Entity> {
        let w = app.world_mut();
        let mut q = w.query::<(Entity, &SpawnId, &Villager, &Mustered)>();
        let mut v: Vec<_> = q
            .iter(w)
            .filter(|(_, _, vil, _)| vil.settlement == t.e)
            .map(|(e, id, ..)| (*id, e))
            .collect();
        v.sort_unstable();
        v.into_iter().map(|(_, e)| e).collect()
    };
    step_to_hour_offset(&mut app, 2);
    govern_cmd(&mut app, t, GovernAct::Dues(Dues::Levy));
    step_past_hour(&mut app);
    let first = waiting(&mut app);
    println!(
        "town #{} ({pop} people): a wave is {per}, at most {cap} waiting; mustered {}",
        t.id,
        first.len()
    );
    assert_eq!(first.len() as u32, per, "2% of the people, at least one");
    assert!(per >= 1);

    // They wait: no collector within 60 tiles.
    app.step(300);
    assert_eq!(waiting(&mut app), first, "still on the green");
    for &e in &first {
        let a = app.world().get::<Activity>(e).unwrap();
        assert_ne!(a.kind, ActionKind::Work, "a mustered man does not work");
        assert_ne!(a.kind, ActionKind::Haul, "nor haul");
        assert!(app.world().get::<Enlisted>(e).is_none());
    }
    // The Dues term: harsh at a conquered town, written by the hourly step.
    app.step(100);
    let term = app
        .world()
        .get::<LoyaltyTerms>(t.e)
        .unwrap()
        .get(LoyaltyTerm::Dues);
    println!("  Dues term at a conquered town: {term}");
    assert_eq!(term, data.levy.harsh_term);

    // The next dawn musters more.
    step_past_dawn(&mut app);
    let second = waiting(&mut app);
    println!("  after a dawn: {} waiting (cap {cap})", second.len());
    assert_eq!(second.len() as u32, (2 * per).min(cap));
    assert!(first.iter().all(|e| second.contains(e)));

    // A captain comes within reach: the next lands pass (the next dawn's
    // hour step, then the tick after) enlists the waiting.
    let at = passable_near(&app, t.centre + IVec2::new(25, 0)).expect("ground");
    let band = spawn_army(app.world_mut(), &ArmySpec::new(FactionId::PLAYER, at, 4));
    let bandid = spawn_id(&app, band);
    let pop0 = people(&app, t);
    step_past_dawn(&mut app);
    let joined: Vec<Entity> = second
        .iter()
        .copied()
        .filter(|&e| {
            app.world()
                .get::<Follower>(e)
                .is_some_and(|f| f.captain == band)
        })
        .collect();
    let third = waiting(&mut app);
    println!(
        "  a captain at 25 tiles: {} of {} joined; population {pop0} -> {}; {} waiting",
        joined.len(),
        second.len(),
        people(&app, t),
        third.len()
    );
    // The dawn's own muster (another wave) is collected in the same pass.
    let men = second.len() as u32 + per;
    assert_eq!(joined.len(), second.len(), "every one who waited joined");
    assert!(
        joined
            .iter()
            .all(|&e| app.world().get::<Mustered>(e).is_none())
    );
    assert_eq!(
        people(&app, t),
        pop0 - men,
        "the town lost the dawn's men too"
    );
    assert!(third.is_empty(), "none left waiting after the pass");
    let tribute = feed(&app, SimEventKind::Tribute, bandid);
    assert_eq!(tribute.len(), 1);
    assert_eq!(tribute[0].event.amount as u32, men);
}

/// GP-34: the Ration scales hunger by 0.75 for the rationed town's
/// residents (a tick at a time, for those who didn't eat that tick), and
/// leaves another town's alone; the town's `FoodSecurity` counts its need
/// at 0.75 so its days stay honest: food value over `hunger_per_day x
/// people x 0.75`.
#[test]
fn rationing_slows_eating_by_a_quarter() {
    let mut app = fresh();
    let t = pick(&mut app, 60, 1, 0.0);
    take(&mut app, t, Cause::Invitation);
    stock_food(&mut app, t, 3.0, true);
    // A control town, unrationed.
    let other = {
        let towns = villages(&mut app);
        let (e, id, centre, _) = *towns
            .iter()
            .find(|(e, _, _, pop)| *e != t.e && *pop >= 40)
            .unwrap();
        Town { e, id, centre }
    };
    step_to_hour_offset(&mut app, 2);
    govern_cmd(&mut app, t, GovernAct::Table(Table::Ration));
    step_past_hour(&mut app);
    let r = *app
        .world()
        .get::<Rationed>(t.e)
        .expect("the settlement is rationed");
    assert_eq!((r.hunger, r.work), (0.75, 1.1));
    let marked = trades(&mut app, t)
        .iter()
        .filter(|(e, _)| app.world().get::<Rationed>(*e).is_some())
        .count();
    println!(
        "town #{}: {marked} of {} residents rationed",
        t.id,
        trades(&mut app, t).len()
    );
    assert_eq!(marked, trades(&mut app, t).len(), "every resident");
    assert!(app.world().get::<Rationed>(other.e).is_none());

    let per_tick = app.world().resource::<Villagers>().hunger_per_tick;
    let snapshot = |app: &mut App, s: Town| -> Vec<(Entity, f32, u32)> {
        let w = app.world_mut();
        let mut q = w.query_filtered::<(Entity, &Villager, &Needs), Without<Enlisted>>();
        q.iter(w)
            .filter(|(_, v, _)| v.settlement == s.e)
            .map(|(e, _, n)| (e, n.hunger, n.meals))
            .collect()
    };
    // Per tick: those who did not eat gain exactly their rate.
    let (mut rationed_ratio, mut plain_ratio) = (Vec::new(), Vec::new());
    for _ in 0..20 {
        let a = snapshot(&mut app, t);
        let b = snapshot(&mut app, other);
        app.step(1);
        for (s, before, out) in [(t, a, &mut rationed_ratio), (other, b, &mut plain_ratio)] {
            let after = snapshot(&mut app, s);
            for ((e, h0, m0), (e1, h1, m1)) in before.into_iter().zip(after) {
                if e == e1 && m0 == m1 && h0 < 0.99 {
                    out.push((h1 - h0) / per_tick);
                }
            }
        }
    }
    println!(
        "  hunger per tick over rate: rationed {} samples (min {:.4}, max {:.4}), plain {} (min {:.4}, max {:.4})",
        rationed_ratio.len(),
        rationed_ratio.iter().cloned().fold(f32::MAX, f32::min),
        rationed_ratio.iter().cloned().fold(f32::MIN, f32::max),
        plain_ratio.len(),
        plain_ratio.iter().cloned().fold(f32::MAX, f32::min),
        plain_ratio.iter().cloned().fold(f32::MIN, f32::max),
    );
    assert!(rationed_ratio.len() > 100 && plain_ratio.len() > 100);
    assert!(rationed_ratio.iter().all(|r| (r - 0.75).abs() < 1e-3));
    assert!(plain_ratio.iter().all(|r| (r - 1.0).abs() < 1e-3));

    // Honest days.
    let fs = *app.world().get::<FoodSecurity>(t.e).unwrap();
    let pop = people(&app, t);
    let honest = fs.food_value / (daily_need(&app, pop) * 0.75);
    println!(
        "  food security {:.3} days; food value / (need x 0.75) = {honest:.3}",
        fs.days
    );
    assert!((fs.days - honest).abs() < 1e-3 * honest.max(1.0));
    // The Table term while rationed.
    app.step(100);
    let term = app
        .world()
        .get::<LoyaltyTerms>(t.e)
        .unwrap()
        .get(LoyaltyTerm::Table);
    let want = app
        .world()
        .resource::<Progression>()
        .data
        .edicts
        .ration
        .term;
    println!("  Table term while rationed {term} (data {want})");
    assert_eq!(term, want);
}

/// One feast scenario: the town, fed, with the Feast set (or not, for the
/// control) in the morning. Returns the app and the town.
fn feast_run(feast: bool) -> (App, Town) {
    let mut app = fresh();
    let t = pick(&mut app, 60, 1, 0.0);
    take(&mut app, t, Cause::Invitation);
    stock_food(&mut app, t, 4.0, true);
    step_to_hour_offset(&mut app, 2);
    if feast {
        govern_cmd(&mut app, t, GovernAct::Table(Table::Feast));
    }
    (app, t)
}

/// Villagers of `t` within `r` tiles of its centre, and the mean distance
/// of all its residents (not enlisted, awake or not).
fn near_centre(app: &mut App, t: Town, r: f32) -> (usize, f32) {
    let list = trades(app, t);
    let c = t.centre.as_vec2();
    let ds: Vec<f32> = list
        .iter()
        .map(|(e, _)| pos_of(app, *e).distance(c))
        .collect();
    let near = ds.iter().filter(|d| **d <= r).count();
    (near, ds.iter().sum::<f32>() / ds.len() as f32)
}

/// GP-34: a Feast with ample food eats half a day of the town's need from
/// its storehouse at the hour it starts; for the next day the town works at
/// 0.3 and gathers on the green (more villagers stand within 8 tiles of the
/// centre in the middle of the working day than in an untreated twin run);
/// at its end a `Festival` deed raises the player's goodwill there (and the
/// Table dial goes back to Normal), the Table term is +0.08 for five days
/// and the next feast waits six.
#[test]
fn a_feast_gathers_and_pays_if_fed() {
    let (mut app, t) = feast_run(true);
    let (mut control, _) = feast_run(false);
    let pop = people(&app, t);
    let need = daily_need(&app, pop);
    let econ = app.world().resource::<Economy>().clone();
    let store = store_of(&app, t);
    let food = |app: &App| {
        app.world()
            .get::<Stockpile>(store)
            .unwrap()
            .food_value(&econ)
    };
    let start = tick(&app);
    // Up to the hour step before it applies, then the hour itself.
    step_to(&mut app, start + 97);
    step_to(&mut control, start + 97);
    let before = food(&app);
    let before_c = food(&control);
    step_to(&mut app, start + 99);
    step_to(&mut control, start + 99);
    let eaten = (before - food(&app)) - (before_c - food(&control));
    println!(
        "town #{} ({pop} people, need {need:.0}/day): the feast ate {eaten:.0} more than the control ({:.2} days)",
        t.id,
        eaten / need
    );
    assert!(
        (eaten / need - 0.5).abs() < 0.1,
        "half a day of food from the storehouse"
    );
    let e = edicts(&app, t);
    assert_eq!(e.table, Table::Feast);
    assert_eq!(e.feast_until, e.dial(Dial::Table).since + 2400);
    assert_eq!(e.feast_work, 0.3);

    // The green: sampled in the middle of the working day.
    let sample = (tick(&app) / day(&app)) * day(&app) + 800;
    step_to(&mut app, sample);
    step_to(&mut control, sample);
    let (feasting, fd) = near_centre(&mut app, t, 8.0);
    let (idle, id_) = near_centre(&mut control, t, 8.0);
    println!(
        "  at tick {sample}: within 8 tiles of the centre {feasting} against {idle} untreated; mean distance {fd:.1} against {id_:.1}"
    );
    assert!(feasting > idle, "the town gathers");
    assert!(fd < id_, "and stands closer in");

    // The end.
    let goodwill = |app: &App| {
        app.world()
            .get::<Goodwill>(t.e)
            .unwrap()
            .get(FactionId::PLAYER)
    };
    let end = e.feast_until;
    step_to(&mut app, end + 3);
    step_to(&mut control, end + 3);
    let e = edicts(&app, t);
    let (g, gc) = (goodwill(&app), goodwill(&control));
    let term = app
        .world()
        .get::<LoyaltyTerms>(t.e)
        .unwrap()
        .get(LoyaltyTerm::Table);
    println!(
        "  feast over: goodwill toward the player {g:.3} against {gc:.3}; Table term {term}; next feast at {} (ended {end})",
        e.next_feast
    );
    assert_eq!(e.table, Table::Normal);
    assert_eq!(e.feast_until, 0);
    assert!(g > gc, "word travels: goodwill rose");
    assert_eq!(e.feast_term_until, end + 5 * 2400);
    assert_eq!(e.next_feast, end + 6 * 2400);
    assert_eq!(term, 0.08, "the afterglow");
    let neighbour = villages(&mut app)
        .into_iter()
        .filter(|(n, ..)| *n != t.e)
        .find(|(n, ..)| {
            app.world()
                .get::<Goodwill>(*n)
                .unwrap()
                .get(FactionId::PLAYER)
                > control
                    .world()
                    .get::<Goodwill>(*n)
                    .unwrap()
                    .get(FactionId::PLAYER)
        });
    println!("  a neighbour that heard: {:?}", neighbour.map(|n| n.1));
    assert!(neighbour.is_some(), "the goodwill spreads to a neighbour");

    // Too little food: refused with its reason; and nothing changes.
    let mut poor = fresh();
    let pt = pick(&mut poor, 60, 1, 0.0);
    take(&mut poor, pt, Cause::Invitation);
    {
        // Strip the town of food: storehouse and homes.
        let se = poor.world().get::<SettlementEconomy>(pt.e).unwrap().clone();
        let food = poor.world().resource::<Economy>().foods_by_value();
        for p in std::iter::once(se.storehouse).chain(se.homes.iter().copied()) {
            let mut st = poor.world_mut().get_mut::<Stockpile>(p).unwrap();
            for &f in &food {
                let n = st.get(f);
                st.take(f, n);
            }
        }
    }
    poor.step(2);
    let why = govern::refusal(
        poor.world(),
        FactionId::PLAYER,
        pt.e,
        GovernAct::Table(Table::Feast),
    );
    println!("  a poor town's feast: {why:?}");
    assert_eq!(why, Some("too little food for a feast"));
    let before = rejected(&poor, FactionId::PLAYER);
    govern_cmd(&mut poor, pt, GovernAct::Table(Table::Feast));
    poor.step(1);
    assert_eq!(rejected(&poor, FactionId::PLAYER), before + 1);
    assert_eq!(edicts(&poor, pt).pending_table, None);
}

/// GP-34: a dial's change locks it for two days (counted in
/// `rejected_commands` when asked again, accepted once the lock ends), the
/// others stay free, a setting already set is refused, a town that is not
/// yours is refused, and building orders and stewards are "not yet".
#[test]
fn dials_lock() {
    let mut app = fresh();
    let t = pick(&mut app, 60, 1, 0.0);
    take(&mut app, t, Cause::Invitation);
    stock_food(&mut app, t, 4.0, true);
    let p = FactionId::PLAYER;
    step_to_hour_offset(&mut app, 2);
    govern_cmd(&mut app, t, GovernAct::Work(WorkFocus::Fields));
    step_past_hour(&mut app);
    let e = edicts(&app, t);
    let unlock = e.dial(Dial::Work).locked_until;
    assert_eq!(e.work, WorkFocus::Fields);
    assert_eq!(rejected(&app, p), 0);

    let why = |app: &App, s: Entity, act| govern::refusal(app.world(), p, s, act);
    assert_eq!(
        why(&app, t.e, GovernAct::Work(WorkFocus::Forge)),
        Some("locked")
    );
    // Asked: counted.
    govern_cmd(&mut app, t, GovernAct::Work(WorkFocus::Forge));
    app.step(1);
    println!(
        "locked until {unlock}; a second Work order at {}: rejected {}",
        tick(&app),
        rejected(&app, p)
    );
    assert_eq!(rejected(&app, p), 1);
    assert_eq!(edicts(&app, t).pending_work, None);
    // The other dials are free.
    govern_cmd(&mut app, t, GovernAct::Dues(Dues::Tithe));
    app.step(1);
    assert_eq!(rejected(&app, p), 1, "Dues is a different dial");
    assert_eq!(edicts(&app, t).pending_dues, Some(Dues::Tithe));
    // Not yet.
    for act in [
        GovernAct::Build(sim_core::BuildKind::Smithy),
        GovernAct::Steward { follower: 1 },
        GovernAct::Release(Dial::Work),
    ] {
        assert_eq!(why(&app, t.e, act), Some("not yet"), "{act:?}");
        govern_cmd(&mut app, t, act);
    }
    app.step(1);
    assert_eq!(rejected(&app, p), 4, "three more refused");
    // Not yours.
    let (other, ..) = *villages(&mut app)
        .iter()
        .find(|(e, ..)| *e != t.e && owner_of(&app, *e) == FactionId::INDEPENDENT)
        .unwrap();
    assert_eq!(
        why(&app, other, GovernAct::Work(WorkFocus::Forge)),
        Some("not your settlement")
    );

    // After the lock, accepted.
    step_to(&mut app, unlock + 1);
    assert_eq!(why(&app, t.e, GovernAct::Work(WorkFocus::Forge)), None);
    let r = rejected(&app, p);
    govern_cmd(&mut app, t, GovernAct::Work(WorkFocus::Forge));
    app.step(2);
    assert_eq!(rejected(&app, p), r, "accepted");
    assert_eq!(edicts(&app, t).pending_work, Some(WorkFocus::Forge));
    step_past_hour(&mut app);
    let e = edicts(&app, t);
    println!(
        "  after the lock: Work {:?}, since {}, locked until {}",
        e.work,
        e.dial(Dial::Work).since,
        e.dial(Dial::Work).locked_until
    );
    assert_eq!(e.work, WorkFocus::Forge);
    assert_eq!(
        e.dial(Dial::Work).locked_until,
        e.dial(Dial::Work).since + 2 * day(&app)
    );
    assert_eq!(
        why(&app, t.e, GovernAct::Work(WorkFocus::Forge)),
        Some("locked")
    );
}

/// GP-34: the dials at their defaults are exact no-ops. Two identical runs
/// of seed 42 with a player town, one with the `Edicts` components
/// removed, play through a dawn; with `Edicts::default()` put back, their
/// state hashes are equal, and no one anywhere is `Rationed`, `Mustered`
/// or carrying an `Assignment`.
#[test]
fn defaults_are_no_ops() {
    let mut with = fresh();
    let mut without = fresh();
    let t = pick(&mut with, 60, 1, 0.0);
    let t2 = pick(&mut without, 60, 1, 0.0);
    assert_eq!(t.id, t2.id);
    take(&mut with, t, Cause::Invitation);
    take(&mut without, t2, Cause::Invitation);
    let holders: Vec<Entity> = {
        let w = without.world_mut();
        let mut q = w.query_filtered::<Entity, With<Edicts>>();
        q.iter(w).collect()
    };
    println!("{} settlements carry Edicts", holders.len());
    assert!(!holders.is_empty());
    for &e in &holders {
        without.world_mut().entity_mut(e).remove::<Edicts>();
    }
    let ticks = day(&with) + 300;
    with.step(ticks);
    without.step(ticks);
    for &e in &holders {
        without.world_mut().entity_mut(e).insert(Edicts::default());
    }
    let a = sim_core::compute_state_hash(with.world_mut());
    let b = sim_core::compute_state_hash(without.world_mut());
    println!("after {ticks} ticks: hashes {a:016x} and {b:016x}");
    assert_eq!(a, b, "defaults changed the run");
    for app in [&mut with, &mut without] {
        let w = app.world_mut();
        assert_eq!(w.query::<&Rationed>().iter(w).count(), 0);
        assert_eq!(w.query::<&Mustered>().iter(w).count(), 0);
        assert_eq!(w.query::<&Assignment>().iter(w).count(), 0);
        let mut q = w.query::<&Edicts>();
        assert!(q.iter(w).all(|e| e.is_default()));
    }
}
