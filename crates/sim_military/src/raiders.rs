//! The scripted hostile band (M3 stand-in for rival warlords, which arrive
//! with GP-09 in M5). It decides from the sim state and issues ordinary
//! AI-origin commands, so it uses the same order path as the player and
//! replays without being recorded.

use bevy_ecs::prelude::*;
use sim_core::{
    CommandKind, FactionId, OrderSpec, Position, RunState, SimCommand, SimTick, SpawnId,
};
use sim_economy::SettlementEconomy;
use sim_world::Settlement;

use crate::components::{Captain, Raider};
use crate::data::Army;
use crate::index::ArmyIndex;
use crate::monuments::Guard;

/// How often a raider reconsiders (ticks).
const THINK_EVERY: u64 = 300;
/// Raiders with less food than this (days) raid a village first.
const RAID_BELOW_DAYS: f32 = 2.0;

/// Runs in `SimInputSet::Ai`, after scripted and player input, so its
/// commands always arrive in the same order (guide G2). In a run, raiders
/// hunt only the player's armies (guide D2), and never a monument's guard
/// camp (GP-23).
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Bevy system parameters.
pub(crate) fn think(
    tick: Res<SimTick>,
    run: Res<RunState>,
    army: Res<Army>,
    index: Res<ArmyIndex>,
    villagers: Res<sim_agents::Villagers>,
    mut raiders: Query<(Entity, &SpawnId, &Captain, &Position, &mut Raider)>,
    captains: Query<(&SpawnId, &Captain, &Position), (Without<Raider>, Without<Guard>)>,
    settlements: Query<(&SpawnId, &Settlement, &SettlementEconomy)>,
    mut out: MessageWriter<SimCommand>,
) {
    let _span = tracing::info_span!("sim_military::raiders").entered();
    let mut ordered: Vec<_> = raiders.iter_mut().collect();
    ordered.sort_unstable_by_key(|(_, id, ..)| **id);
    for (e, id, captain, pos, mut raider) in ordered {
        if tick.0 < raider.next_think {
            continue;
        }
        raider.next_think = tick.0 + THINK_EVERY;
        let here = pos.0.as_vec2();
        // Food first; once fed, hunt the nearest enemy army.
        let days = index.get(e).map_or(0.0, |a| {
            a.food / (villagers.data.needs.hunger_per_day * (a.followers + 1) as f32).max(1e-3)
        });
        let raiding = matches!(
            captain.orders.front().map(|o| o.spec),
            Some(OrderSpec::GatherFood { .. })
        );
        if raiding {
            continue;
        }
        let prey = captains
            .iter()
            .filter(|(_, c, _)| {
                army.at_war(c.faction, captain.faction)
                    && (!run.in_run() || c.faction == FactionId::PLAYER)
            })
            .map(|(cid, _, p)| (p.0.as_vec2().distance(here), *cid, p.0))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let order = if let Some((_, _, at)) = prey.filter(|_| days >= RAID_BELOW_DAYS) {
            OrderSpec::MoveTo { x: at.x, y: at.y }
        } else {
            // Otherwise raid the nearest village not raided last.
            let Some((_, sid)) = settlements
                .iter()
                .filter(|(sid, _, se)| se.population > 0 && Some(sid.0) != raider.last_target)
                .map(|(sid, s, _)| (s.center.as_vec2().distance(here), sid.0))
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            else {
                continue;
            };
            raider.last_target = Some(sid);
            OrderSpec::GatherFood { settlement: sid }
        };
        out.write(SimCommand::ai(
            captain.faction,
            CommandKind::IssueOrder {
                captain: id.0,
                order,
                queue: false,
            },
        ));
    }
}
