//! The economy: goods, recipes, workplaces, stockpiles and inventories
//! (SIM-20, SIM-21), buildings and settlement layout. Trade between
//! settlements arrives in M4b.
//!
//! # Main pieces
//! - [`Economy`]: goods, recipes and workplaces from `economy.ron`.
//! - [`Stockpile`] on buildings (and on captains, as baggage) and
//!   [`Inventory`] on people.
//! - [`Transfers`] and [`goods::resolve_transfers`]: agents post goods orders
//!   on themselves in [`EconomySet::Post`], and the resolver applies them in
//!   SpawnId order in [`EconomySet::Resolve`]. This is the only way an agent
//!   touches a building's stock (ADR 0007).
//! - [`Building`], [`Workplace`], [`Home`], [`SettlementEconomy`], and
//!   [`layout::build_settlement`], which places them.
//! - [`Construction`]: a building site ordered in a run (SIM-63, M4a).
//!
//! Never depends on rendering, windowing, audio or asset crates.

pub mod construction;
pub mod data;
pub mod goods;
pub mod layout;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use sim_core::{AppHashExt, AppSaveExt, SimSet};

pub use construction::Construction;
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

/// Goods, buildings and the transfer resolver.
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
            .register_hashed::<Occupation>()
            // M4a: a building site (run only).
            .register_hashed::<Construction>()
            // GP-12: every hashed type is saved.
            .register_saved_resource::<Economy>()
            .register_saved::<Stockpile>()
            .register_saved::<Inventory>()
            .register_saved::<Transfers>()
            .register_saved::<Building>()
            .register_saved::<Workplace>()
            .register_saved::<Home>()
            .register_saved::<SettlementEconomy>()
            .register_saved::<Occupation>()
            .register_saved::<Construction>();
    }
}
