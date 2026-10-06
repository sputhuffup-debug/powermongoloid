//! Goods, inventories, recipes and stockpiles (SIM-20, SIM-21); trade
//! between settlements arrives in M4.
//!
//! Never depends on rendering, windowing, audio or asset crates.

pub mod data;
pub mod goods;
pub mod layout;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use sim_core::{AppHashExt, SimSet};

pub use data::{Economy, GoodId, Occupation, RecipeId, WorkplaceKind};
pub use goods::{Inventory, Stockpile, TransferOp, Transfers};
pub use layout::{Building, BuildingKind, Home, Occupied, SettlementEconomy, Workplace};

/// Ordering inside [`SimSet::Act`]: agents post transfer orders, then the
/// resolver applies them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EconomySet {
    /// Agent systems that post [`Transfers`] orders go here.
    Post,
    Resolve,
}

pub struct EconomyPlugin;

impl Plugin for EconomyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Economy::default_data())
            .configure_sets(
                FixedUpdate,
                (EconomySet::Post, EconomySet::Resolve)
                    .chain()
                    .in_set(SimSet::Act),
            )
            .add_systems(
                FixedUpdate,
                goods::resolve_transfers.in_set(EconomySet::Resolve),
            )
            .register_hashed_resource::<Economy>()
            .register_hashed::<Stockpile>()
            .register_hashed::<Inventory>()
            .register_hashed::<Transfers>()
            .register_hashed::<Building>()
            .register_hashed::<Workplace>()
            .register_hashed::<Home>()
            .register_hashed::<SettlementEconomy>()
            .register_hashed::<Occupation>();
    }
}
