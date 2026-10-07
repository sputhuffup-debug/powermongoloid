//! Helpers shared by the scenario tests: M3 armies (commands, positions,
//! village lists) and M4 runs (`run_app`, offers, the champion, the
//! chronicle).
#![allow(dead_code)] // Each test crate uses a different subset.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use sim_agents::{CombatStats, Villagers};
use sim_core::{
    Chronicle, ChronicleEntry, CommandKind, FactionId, Motion, OrderSpec, Position, RunConfig,
    SimCommand, SpawnId,
};
use sim_economy::SettlementEconomy;
use sim_headless::{HeadlessConfig, headless_app};
use sim_military::{ArmyStats, Follower, nearest_passable};
use sim_world::{Settlement, WorldMap, world_pos};

pub use sim_headless::App;

/// Sends a player command the way the UI does: as a `SimCommand` message.
pub fn command(app: &mut App, issuer: u16, kind: CommandKind) {
    app.world_mut()
        .write_message(SimCommand::player(FactionId(issuer), kind));
}

pub fn order(app: &mut App, issuer: u16, captain: Entity, order: OrderSpec) {
    let id = spawn_id(app, captain);
    command(
        app,
        issuer,
        CommandKind::IssueOrder {
            captain: id,
            order,
            queue: false,
        },
    );
}

pub fn spawn_id(app: &App, e: Entity) -> u64 {
    app.world().get::<SpawnId>(e).expect("sim entity").0
}

pub fn pos_of(app: &App, e: Entity) -> Vec2 {
    let w = app.world();
    world_pos(w.get::<Position>(e).unwrap(), w.get::<Motion>(e))
}

/// Living followers of `captain`: (position, tile).
pub fn followers(app: &mut App, captain: Entity) -> Vec<(Vec2, IVec2)> {
    let world = app.world_mut();
    let mut q = world.query::<(&Follower, &Position, &Motion, &CombatStats)>();
    q.iter(world)
        .filter(|(f, _, _, cs)| f.captain == captain && cs.alive())
        .map(|(_, p, m, _)| (world_pos(p, Some(m)), p.0))
        .collect()
}

/// Populated settlements: (entity, SpawnId, centre, population), by SpawnId.
pub fn villages(app: &mut App) -> Vec<(Entity, u64, IVec2, u32)> {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &SpawnId, &Settlement, &SettlementEconomy)>();
    let mut v: Vec<_> = q
        .iter(world)
        .filter(|(.., se)| se.population > 0)
        .map(|(e, id, s, se)| (e, id.0, s.center, se.population))
        .collect();
    v.sort_unstable_by_key(|x| x.1);
    v
}

pub fn passable_near(app: &App, at: IVec2) -> Option<IVec2> {
    let world = app.world();
    let map = world.resource::<WorldMap>();
    let costs = &world.resource::<Villagers>().travel_costs;
    nearest_passable(map, costs, at)
}

/// The fraction-th quantile of `xs` (sorts in place).
pub fn quantile(xs: &mut [f32], fraction: f32) -> f32 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.sort_by(f32::total_cmp);
    let i = ((xs.len() - 1) as f32 * fraction).round() as usize;
    xs[i]
}

/// GP-21: a run on the run island with the default `RunConfig`, set up
/// before the first tick (band, champion, start boon offer).
pub fn run_app(seed: u64) -> App {
    headless_app(HeadlessConfig::run(seed, RunConfig::default()))
}

/// The id of the player's pending start boon offer (GP-27).
pub fn start_offer(app: &App) -> u64 {
    sim_headless::start_offer(app).expect("a pending start boon offer")
}

/// The player picks `choice` of `offer` (GP-27), as the UI does: a
/// recorded `ChooseOffer` command, applied on the next tick.
pub fn choose(app: &mut App, offer: u64, choice: u8) {
    command(
        app,
        FactionId::PLAYER.0,
        CommandKind::ChooseOffer { offer, choice },
    );
}

/// The player's champion (SIM-35).
pub fn champion(app: &mut App) -> Entity {
    sim_military::champion_of(app.world_mut(), FactionId::PLAYER).expect("a champion")
}

/// The chronicle so far. Tests read it (or the event feed), never
/// `SimEvent` messages, which live two ticks.
pub fn chronicle(app: &App) -> Vec<ChronicleEntry> {
    app.world().resource::<Chronicle>().entries().to_vec()
}

/// Commands refused for `faction` so far.
pub fn rejected(app: &App, faction: FactionId) -> u32 {
    app.world()
        .resource::<ArmyStats>()
        .get(faction)
        .rejected_commands
}
