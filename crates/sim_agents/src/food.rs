//! Food security (UI-02, SIM-24): days of food a settlement holds at its
//! current population's rate of eating.

use bevy_ecs::prelude::*;
use sim_core::{StateHash, StateHasher};
use sim_economy::{Economy, SettlementEconomy, Stockpile};

use crate::data::Villagers;

/// Food security on a settlement (UI-02, SIM-24).
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct FoodSecurity {
    /// Days of food in the storehouse and homes.
    pub days: f32,
    /// Total food value (hunger it can remove).
    pub food_value: f32,
}

sim_core::save_fields!(FoodSecurity { days, food_value });

impl StateHash for FoodSecurity {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.days);
        h.f32(self.food_value);
    }
}

pub(crate) fn update_food_security(
    econ: Res<Economy>,
    villagers: Res<Villagers>,
    stocks: Query<&Stockpile>,
    mut settlements: Query<(&SettlementEconomy, &mut FoodSecurity)>,
) {
    let _span = tracing::info_span!("sim_agents::update_food_security").entered();
    for (se, mut fs) in &mut settlements {
        let value: f32 = std::iter::once(se.storehouse)
            .chain(se.homes.iter().copied())
            .filter_map(|e| stocks.get(e).ok())
            .map(|s| s.food_value(&econ))
            .sum();
        let per_day = villagers.data.needs.hunger_per_day * se.population.max(1) as f32;
        fs.food_value = value;
        fs.days = value / per_day;
    }
}
