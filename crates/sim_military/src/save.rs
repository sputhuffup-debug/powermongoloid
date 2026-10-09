//! Save and resume (GP-12, ADR 0016): how sim_military's state is saved.
//!
//! Every hashed component and resource is registered here, in the order it
//! is hashed ([`register`]); the data resources (army.ron, progression.ron,
//! director.ron) are saved as their RON text and resolved again on load.
//! The army index and the fighter grid are saved too (see
//! [`crate::index::save_index`]), as is the military log for the UI. The
//! per-tick scratch resources start empty after a load ([`after_load`]).

use bevy_app::App;
use bevy_ecs::prelude::*;
use sim_core::{AppSaveExt, Calendar, SaveError, SaveReader, SaveState, SaveWriter};
use sim_economy::Economy;

use crate::combat::Casualties;
use crate::components::{
    ArmyStats, Captain, Equipment, FactionStats, Factions, Follower, MilitaryLog, Order, Raider,
    Soldier,
};
use crate::data::Army;
use crate::director::{Director, DirectorData};
use crate::index::{ArmyIndex, FighterGrid};
use crate::monuments::{Guard, Holder, NoUpkeep};
use crate::orders::CommandInbox;
use crate::progression::{
    Champion, ChampionRank, Progression, RegionTally, RunProgress, StartTerrain, Traits,
};
use crate::requests::{Request, RequestBoard, RequestKind, Reward, Watch};
use crate::rival::{RivalBand, RivalState};
use crate::upkeep::Deserters;
use crate::veterans::Veteran;
use crate::wild::WildFood;

sim_core::save_fields!(Order {
    spec,
    step,
    since,
    origin,
    held_since
});
sim_core::save_fields!(Captain {
    faction,
    posture,
    orders,
    goal,
    window,
    rally,
    rally_window,
    last_forage,
    battle_since,
    last_engaged,
    battle_losses,
    battle_foes,
    battle_kills,
    battle_rout
});
sim_core::save_fields!(Follower {
    captain,
    slot,
    deserting
});
sim_core::save_fields!(Equipment { weapon, tools });
sim_core::save_fields!(Soldier { name });
sim_core::save_fields!(Raider {
    next_think,
    last_target,
    target
});
sim_core::save_fields!(Factions { seats });
sim_core::save_fields!(FactionStats {
    recruited,
    deserted,
    lost,
    kills,
    villagers_killed,
    seized,
    bought,
    given,
    provocations,
    starvation_damage,
    first_desertion,
    first_starvation,
    rejected_commands,
    conquests,
    supplied,
    donated,
    foraged
});
sim_core::save_fields!(ArmyStats { factions });
sim_core::save_unit!(Champion, NoUpkeep);
sim_core::save_newtype!(Traits);
sim_core::save_serde!(StartTerrain);
sim_core::save_fields!(RegionTally {
    total,
    held_by_player,
    flags
});
sim_core::save_fields!(RunProgress {
    start,
    terrain,
    start_region,
    rival_seat,
    seat_center,
    rival_region,
    band,
    modifiers,
    boons,
    succession_used,
    invite_cooldowns,
    drifters,
    dominion,
    captain_slots
});
sim_core::save_fields!(WildFood {
    cell,
    per_side,
    tiles,
    level
});
sim_core::save_fields!(Reward {
    food,
    goods,
    volunteers,
    goodwill
});
sim_core::save_fields!(Request {
    id,
    settlement,
    kind,
    posted,
    deadline,
    reward
});
sim_core::save_fields!(Watch {
    settlement,
    low_food,
    quiet_until,
    camp_quiet_until
});
sim_core::save_fields!(RequestBoard {
    next_id,
    open,
    watch,
    posted,
    done,
    failed
});
sim_core::save_fields!(Holder {
    faction,
    since,
    contender,
    last_presence
});
sim_core::save_fields!(Guard { monument });
sim_core::save_fields!(Veteran {
    battles,
    rank,
    fought
});
sim_core::save_fields!(ChampionRank { rank, points });
sim_core::save_fields!(RivalState {
    seat,
    seat_center,
    defence_bonus,
    seat_radius,
    next_levy,
    next_raid,
    raids,
    levied
});
sim_core::save_fields!(RivalBand { target, next_think });
sim_core::save_fields!(Director {
    tension,
    last_spawn,
    next_check,
    first_event,
    bands
});
sim_core::save_fields!(MilitaryLog { entries, seq });

impl SaveState for RequestKind {
    fn save(&self, w: &mut SaveWriter) {
        w.u8(self.tag());
        match *self {
            RequestKind::DriveOff { since } => w.u64(since),
            RequestKind::BringFood { need, delivered } => {
                w.f32(need);
                w.f32(delivered);
            }
            RequestKind::ClearCamp { monument } => w.u64(monument),
        }
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        Ok(match r.u8()? {
            0 => RequestKind::DriveOff { since: r.u64()? },
            1 => RequestKind::BringFood {
                need: r.f32()?,
                delivered: r.f32()?,
            },
            2 => RequestKind::ClearCamp { monument: r.u64()? },
            t => return Err(SaveError::Corrupt(format!("bad request kind {t}"))),
        })
    }
}

/// Saved as its RON text; resolved against the loaded calendar and economy.
impl SaveState for DirectorData {
    fn save(&self, w: &mut SaveWriter) {
        w.ron(self);
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        DirectorData::from_ron(&r.str()?).map_err(SaveError::Mismatch)
    }
}

/// Saved as its RON text, validated on load.
impl SaveState for Progression {
    fn save(&self, w: &mut SaveWriter) {
        w.ron(&self.data);
    }
    fn load(r: &mut SaveReader) -> Result<Self, SaveError> {
        Progression::from_ron(&r.str()?).map_err(SaveError::Mismatch)
    }
}

fn save_army(world: &World, w: &mut SaveWriter) {
    w.ron(&world.resource::<Army>().data);
}

fn load_army(world: &mut World, r: &mut SaveReader) -> Result<(), SaveError> {
    let text = r.str()?;
    let army = Army::from_ron(
        &text,
        world.resource::<Calendar>(),
        world.resource::<Economy>(),
    )
    .map_err(SaveError::Mismatch)?;
    world.insert_resource(army);
    Ok(())
}

/// Registers sim_military's saved types (GP-12): every hashed type, in
/// hashing order, then the index, grid and log.
pub(crate) fn register(app: &mut App) {
    app.register_saved_resource_with::<Army>(save_army, load_army)
        .register_saved_resource::<Factions>()
        .register_saved_resource::<ArmyStats>()
        .register_saved::<Captain>()
        .register_saved::<Follower>()
        .register_saved::<Equipment>()
        .register_saved::<Soldier>()
        .register_saved::<Raider>()
        .register_saved_resource::<Progression>()
        .register_saved_resource::<RunProgress>()
        .register_saved::<Champion>()
        .register_saved::<Traits>()
        .register_saved_resource::<WildFood>()
        .register_saved_resource::<RequestBoard>()
        .register_saved::<Holder>()
        .register_saved::<Guard>()
        .register_saved::<NoUpkeep>()
        .register_saved::<Veteran>()
        .register_saved::<ChampionRank>()
        .register_saved_resource::<RivalState>()
        .register_saved::<RivalBand>()
        .register_saved_resource::<DirectorData>()
        .register_saved_resource::<Director>()
        .register_saved_resource::<MilitaryLog>()
        .register_saved_resource_with::<ArmyIndex>(
            crate::index::save_index,
            crate::index::load_index,
        )
        .register_saved_resource_with::<FighterGrid>(
            crate::index::save_grid,
            crate::index::load_grid,
        )
        .on_load(after_load);
}

/// GP-12, after a load: this tick's scratch (inbox, casualties, deserters)
/// starts empty, as it is between ticks.
fn after_load(world: &mut World) {
    world.insert_resource(CommandInbox::default());
    world.insert_resource(Casualties::default());
    world.insert_resource(Deserters::default());
}
