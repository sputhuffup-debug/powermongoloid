//! Economy data from `assets/data/economy.ron`, resolved into ids.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_core::{Stat, StateHash, StateHasher};

/// `assets/data/economy.ron`, embedded at compile time.
pub const DEFAULT_ECONOMY_RON: &str = include_str!("../../../assets/data/economy.ron");

/// A good: an index into `economy.ron`'s goods list (see [`Economy`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct GoodId(pub u8);

/// Index into [`Economy::recipes`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecipeId(pub u16);

/// Occupations at launch (SIM-11).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Component,
)]
pub enum Occupation {
    Farmer,
    Fisher,
    Shepherd,
    Woodcutter,
    Miner,
    Smith,
    Baker,
    Carter,
    Idle,
}

impl Occupation {
    /// Every occupation, in id order.
    pub const ALL: [Occupation; 9] = [
        Occupation::Farmer,
        Occupation::Fisher,
        Occupation::Shepherd,
        Occupation::Woodcutter,
        Occupation::Miner,
        Occupation::Smith,
        Occupation::Baker,
        Occupation::Carter,
        Occupation::Idle,
    ];

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Occupation::Farmer => "farmer",
            Occupation::Fisher => "fisher",
            Occupation::Shepherd => "shepherd",
            Occupation::Woodcutter => "woodcutter",
            Occupation::Miner => "miner",
            Occupation::Smith => "smith",
            Occupation::Baker => "baker",
            Occupation::Carter => "carter",
            Occupation::Idle => "idle",
        }
    }

    /// The workplace this occupation works at, if any.
    pub fn workplace(self) -> Option<WorkplaceKind> {
        Some(match self {
            Occupation::Farmer => WorkplaceKind::Farm,
            Occupation::Fisher => WorkplaceKind::FishingSpot,
            Occupation::Shepherd => WorkplaceKind::Pasture,
            Occupation::Woodcutter => WorkplaceKind::WoodcutterCamp,
            Occupation::Miner => WorkplaceKind::Mine,
            Occupation::Smith => WorkplaceKind::Smithy,
            Occupation::Baker => WorkplaceKind::Bakery,
            Occupation::Carter => WorkplaceKind::Storehouse,
            Occupation::Idle => return None,
        })
    }
}

impl StateHash for Occupation {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(*self as u8);
    }
}

/// Kinds of workplace (economy.ron `workplaces`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum WorkplaceKind {
    Farm,
    FishingSpot,
    Pasture,
    WoodcutterCamp,
    Mine,
    Smithy,
    Bakery,
    Storehouse,
}

impl WorkplaceKind {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            WorkplaceKind::Farm => "farm",
            WorkplaceKind::FishingSpot => "fishing spot",
            WorkplaceKind::Pasture => "pasture",
            WorkplaceKind::WoodcutterCamp => "woodcutter's camp",
            WorkplaceKind::Mine => "mine",
            WorkplaceKind::Smithy => "smithy",
            WorkplaceKind::Bakery => "bakery",
            WorkplaceKind::Storehouse => "storehouse",
        }
    }
}

/// Where a workplace may stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SiteRule {
    /// Farmland tiles.
    Farmland,
    /// Open grass.
    Grass,
    /// Land (beach or grass) next to water.
    Coast,
    /// Forest next to open land.
    ForestEdge,
    /// Hills or mountain.
    Upland,
    /// Forest, cleared for the purpose (a last resort for farms).
    Clearing,
    /// Any open land near the centre.
    Centre,
}

/// A good as written in economy.ron.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoodDef {
    pub name: String,
    pub food: f32,
}

/// A recipe as written in economy.ron (goods by name).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeDef {
    pub name: String,
    pub workplace: WorkplaceKind,
    pub worker: Occupation,
    pub inputs: Vec<(String, u16)>,
    pub outputs: Vec<(String, u16)>,
    pub duration: u32,
    /// The faction modifier that applies to this recipe (GP-23), read
    /// where villagers work (sim_agents `act::perform`): `Some(FoodYield)`
    /// scales its outputs, `Some(SmithSpeed)` shortens its duration. `None`
    /// (the default) for none. Other stats are refused.
    #[serde(default)]
    pub modifier: Option<Stat>,
}

/// Workplace placement and staffing, from economy.ron.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkplaceDef {
    pub kind: WorkplaceKind,
    pub slots: u32,
    pub radius: i32,
    pub spacing: i32,
    pub sites: Vec<SiteRule>,
}

/// The file as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomyData {
    pub goods: Vec<GoodDef>,
    pub recipes: Vec<RecipeDef>,
    pub workplaces: Vec<WorkplaceDef>,
    pub workplace_capacity: u32,
    pub storehouse_capacity: u32,
    pub home_capacity: u32,
    pub carry: u16,
    pub cart_carry: u16,
    pub initial_stock_per_villager: Vec<(String, u32)>,
    pub initial_home_food_per_resident: u32,
}

/// A recipe with goods resolved to ids.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub name: String,
    pub workplace: WorkplaceKind,
    pub worker: Occupation,
    pub inputs: Vec<(GoodId, u16)>,
    pub outputs: Vec<(GoodId, u16)>,
    pub duration: u32,
    /// See [`RecipeDef::modifier`].
    pub modifier: Option<Stat>,
}

/// Resolved economy data, a hashed resource.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Economy {
    pub data: EconomyData,
    pub recipes: Vec<Recipe>,
    pub initial_stock: Vec<(GoodId, u32)>,
}

impl Economy {
    /// Parses economy.ron text and resolves names to ids.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        let data: EconomyData = ron::from_str(text).map_err(|e| e.to_string())?;
        Self::resolve(data)
    }

    /// The shipped economy.
    pub fn default_data() -> Self {
        Self::from_ron(DEFAULT_ECONOMY_RON).expect("assets/data/economy.ron is valid")
    }

    fn resolve(data: EconomyData) -> Result<Self, String> {
        if data.goods.is_empty() || data.goods.len() > u8::MAX as usize {
            return Err("economy: need 1..=255 goods".into());
        }
        let good = |name: &str| -> Result<GoodId, String> {
            data.goods
                .iter()
                .position(|g| g.name == name)
                .map(|i| GoodId(i as u8))
                .ok_or_else(|| format!("economy: unknown good {name:?}"))
        };
        let resolve_list = |list: &[(String, u16)]| -> Result<Vec<(GoodId, u16)>, String> {
            list.iter().map(|(n, q)| Ok((good(n)?, *q))).collect()
        };
        let mut recipes = Vec::new();
        for r in &data.recipes {
            if r.duration == 0 || r.outputs.is_empty() {
                return Err(format!(
                    "economy: recipe {} needs outputs and a duration",
                    r.name
                ));
            }
            if r.worker.workplace() != Some(r.workplace) {
                return Err(format!(
                    "economy: recipe {}: a {} doesn't work at a {}",
                    r.name,
                    r.worker.name(),
                    r.workplace.name()
                ));
            }
            if !matches!(r.modifier, None | Some(Stat::FoodYield | Stat::SmithSpeed)) {
                return Err(format!(
                    "economy: recipe {}: modifier must be FoodYield or SmithSpeed",
                    r.name
                ));
            }
            recipes.push(Recipe {
                name: r.name.clone(),
                workplace: r.workplace,
                worker: r.worker,
                inputs: resolve_list(&r.inputs)?,
                outputs: resolve_list(&r.outputs)?,
                duration: r.duration,
                modifier: r.modifier,
            });
        }
        for kind in [
            WorkplaceKind::Farm,
            WorkplaceKind::FishingSpot,
            WorkplaceKind::Pasture,
            WorkplaceKind::WoodcutterCamp,
            WorkplaceKind::Mine,
            WorkplaceKind::Smithy,
            WorkplaceKind::Bakery,
            WorkplaceKind::Storehouse,
        ] {
            if !data.workplaces.iter().any(|w| w.kind == kind) {
                return Err(format!("economy: no workplace entry for {}", kind.name()));
            }
        }
        let initial_stock = data
            .initial_stock_per_villager
            .iter()
            .map(|(n, q)| Ok((good(n)?, *q)))
            .collect::<Result<_, String>>()?;
        Ok(Self {
            data,
            recipes,
            initial_stock,
        })
    }

    /// Number of goods.
    pub fn good_count(&self) -> usize {
        self.data.goods.len()
    }

    /// The good's name.
    pub fn good_name(&self, g: GoodId) -> &str {
        self.data
            .goods
            .get(g.0 as usize)
            .map_or("?", |d| d.name.as_str())
    }

    /// Hunger one unit removes (0 if not food).
    pub fn food_value(&self, g: GoodId) -> f32 {
        self.data.goods.get(g.0 as usize).map_or(0.0, |d| d.food)
    }

    /// Whether the good is food.
    pub fn is_food(&self, g: GoodId) -> bool {
        self.food_value(g) > 0.0
    }

    /// Food goods, most nourishing first (eating preference).
    pub fn foods_by_value(&self) -> Vec<GoodId> {
        let mut foods: Vec<GoodId> = (0..self.good_count() as u8)
            .map(GoodId)
            .filter(|g| self.is_food(*g))
            .collect();
        foods.sort_by(|a, b| {
            self.food_value(*b)
                .total_cmp(&self.food_value(*a))
                .then(a.cmp(b))
        });
        foods
    }

    /// Placement and staffing for a workplace kind.
    pub fn workplace(&self, kind: WorkplaceKind) -> &WorkplaceDef {
        self.data
            .workplaces
            .iter()
            .find(|w| w.kind == kind)
            .expect("validated: every kind has an entry")
    }

    /// Recipes made at a workplace kind, in file order.
    pub fn recipes_for(&self, kind: WorkplaceKind) -> Vec<RecipeId> {
        self.recipes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.workplace == kind)
            .map(|(i, _)| RecipeId(i as u16))
            .collect()
    }

    /// A resolved recipe by id.
    pub fn recipe(&self, id: RecipeId) -> &Recipe {
        &self.recipes[id.0 as usize]
    }
}

impl StateHash for Economy {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(
            ron::to_string(&self.data)
                .expect("economy data serialises")
                .as_bytes(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_data_resolves() {
        let e = Economy::default_data();
        assert_eq!(e.good_count(), 8, "SIM-20 lists eight goods");
        for name in [
            "grain", "fish", "mutton", "bread", "wood", "ore", "tools", "weapons",
        ] {
            assert!(
                e.data.goods.iter().any(|g| g.name == name),
                "missing {name}"
            );
        }
        assert_eq!(e.recipes_for(WorkplaceKind::Smithy).len(), 2);
        let foods = e.foods_by_value();
        assert_eq!(e.good_name(foods[0]), "mutton");
        assert!(!e.is_food(GoodId(4)));
    }

    #[test]
    fn unknown_goods_are_rejected() {
        let text = DEFAULT_ECONOMY_RON.replace("(\"bread\", 4)", "(\"cake\", 4)");
        assert!(Economy::from_ron(&text).unwrap_err().contains("cake"));
    }

    /// GP-23: farming and fishing scale with FoodYield, smithing with
    /// SmithSpeed; a recipe without the field has none, and any other stat
    /// is refused.
    #[test]
    fn recipe_modifiers_are_food_yield_or_smith_speed() {
        let e = Economy::default_data();
        let of = |name: &str| e.recipes.iter().find(|r| r.name == name).unwrap().modifier;
        assert_eq!(of("farming"), Some(Stat::FoodYield));
        assert_eq!(of("fishing"), Some(Stat::FoodYield));
        assert_eq!(of("toolmaking"), Some(Stat::SmithSpeed));
        assert_eq!(of("baking"), None);
        let text = DEFAULT_ECONOMY_RON.replace("Some(SmithSpeed)", "Some(Morale)");
        assert!(Economy::from_ron(&text).unwrap_err().contains("modifier"));
    }

    #[test]
    fn mismatched_worker_is_rejected() {
        let text = DEFAULT_ECONOMY_RON.replace(
            "workplace: Bakery, worker: Baker",
            "workplace: Bakery, worker: Miner",
        );
        assert!(Economy::from_ron(&text).is_err());
    }
}
