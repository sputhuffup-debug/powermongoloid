//! Settlement layout: houses and workplaces placed on suitable land around
//! a settlement, deterministically (sorted by distance, then tile index).

use std::collections::{BTreeMap, BTreeSet};

/// Tiles already built on, shared across settlements so sites never overlap.
#[derive(Debug, Clone, Default)]
pub struct Occupied(BTreeSet<(i32, i32)>);

impl Occupied {
    /// Whether tile `p` is built on.
    pub fn contains(&self, p: &IVec2) -> bool {
        self.0.contains(&(p.x, p.y))
    }
    /// Marks `p` built on; false if it already was.
    pub fn insert(&mut self, p: IVec2) -> bool {
        self.0.insert((p.x, p.y))
    }
}

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::{NextSpawnId, Position, StateHash, StateHasher};
use sim_world::{Terrain, WorldMap};

use crate::data::{Economy, RecipeId, SiteRule, WorkplaceKind};
use crate::goods::Stockpile;

/// What a building is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingKind {
    House,
    Workplace(WorkplaceKind),
}

impl BuildingKind {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            BuildingKind::House => "house",
            BuildingKind::Workplace(k) => k.name(),
        }
    }
}

impl sim_core::SaveState for BuildingKind {
    fn save(&self, w: &mut sim_core::SaveWriter) {
        match *self {
            BuildingKind::House => w.u8(0),
            BuildingKind::Workplace(k) => {
                w.u8(1);
                w.serde(&k);
            }
        }
    }
    fn load(r: &mut sim_core::SaveReader) -> Result<Self, sim_core::SaveError> {
        Ok(match r.u8()? {
            0 => BuildingKind::House,
            1 => BuildingKind::Workplace(r.serde()?),
            t => {
                return Err(sim_core::SaveError::Corrupt(format!(
                    "bad building kind {t}"
                )));
            }
        })
    }
}

// GP-12: saved field by field; entities as their SpawnIds.
sim_core::save_fields!(Building {
    kind,
    settlement,
    tile
});
sim_core::save_fields!(Workplace {
    kind,
    recipe,
    workers
});
sim_core::save_fields!(Home { residents });
sim_core::save_fields!(SettlementEconomy {
    storehouse,
    homes,
    workplaces,
    population
});

/// A building: what it is, whose it is and where.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Building {
    pub kind: BuildingKind,
    pub settlement: Entity,
    pub tile: IVec2,
}

impl StateHash for Building {
    fn hash_state(&self, h: &mut StateHasher) {
        match self.kind {
            BuildingKind::House => h.u8(0),
            BuildingKind::Workplace(k) => {
                h.u8(1);
                h.u8(k as u8);
            }
        }
        h.entity(self.settlement);
        h.ivec2(self.tile);
    }
}

/// A workplace (SEED.md "Core data model"): its recipe and workers.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Workplace {
    pub kind: WorkplaceKind,
    pub recipe: Option<RecipeId>,
    pub workers: Vec<Entity>,
}

impl StateHash for Workplace {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.kind as u8);
        h.u16(self.recipe.map_or(u16::MAX, |r| r.0));
        h.u32(self.workers.len() as u32);
        for w in &self.workers {
            h.entity(*w);
        }
    }
}

/// A house and who lives there.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct Home {
    pub residents: Vec<Entity>,
}

impl StateHash for Home {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.residents.len() as u32);
        for r in &self.residents {
            h.entity(*r);
        }
    }
}

/// The economic side of a settlement: its buildings and population.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SettlementEconomy {
    pub storehouse: Entity,
    pub homes: Vec<Entity>,
    pub workplaces: Vec<Entity>,
    pub population: u32,
}

impl StateHash for SettlementEconomy {
    fn hash_state(&self, h: &mut StateHasher) {
        h.entity(self.storehouse);
        h.u32(self.homes.len() as u32);
        for e in &self.homes {
            h.entity(*e);
        }
        h.u32(self.workplaces.len() as u32);
        for e in &self.workplaces {
            h.entity(*e);
        }
        h.u32(self.population);
    }
}

/// The layout's recipe for the `i`-th workplace of a kind whose recipes
/// are `recipes` (`Economy::recipes_for`): workplaces with several recipes
/// (the smithy) alternate between them in file order. A recipe flipped with
/// [`set_recipe`] goes back to this one when its edict ends (step 4).
pub fn layout_recipe(recipes: &[RecipeId], i: usize) -> Option<RecipeId> {
    (!recipes.is_empty()).then(|| recipes[i % recipes.len()])
}

/// Sets a workplace's recipe (SIM-64; the Forge edict and ordered smithies
/// call it). Refused, changing nothing, if the recipe is made at another
/// kind of workplace. Any cycle under way finishes with the old recipe's
/// outputs; a worker's next cycle takes the new one's inputs.
pub fn set_recipe(
    econ: &Economy,
    workplace: &mut Workplace,
    recipe: RecipeId,
) -> Result<(), String> {
    let r = econ
        .recipes
        .get(recipe.0 as usize)
        .ok_or_else(|| format!("no recipe #{}", recipe.0))?;
    if r.workplace != workplace.kind {
        return Err(format!(
            "{} is made at a {}, not a {}",
            r.name,
            r.workplace.name(),
            workplace.kind.name()
        ));
    }
    workplace.recipe = Some(recipe);
    Ok(())
}

/// What to build for one settlement.
#[derive(Debug, Clone, Default)]
pub struct LayoutRequest {
    pub houses: u32,
    /// Workers needing a place, per workplace kind (excluding the storehouse).
    pub workers: BTreeMap<WorkplaceKind, u32>,
}

/// What was built.
#[derive(Debug, Clone)]
pub struct Layout {
    pub storehouse: Entity,
    pub houses: Vec<Entity>,
    pub workplaces: BTreeMap<WorkplaceKind, Vec<Entity>>,
}

/// Radius (tiles) around the centre that stays open as the village green.
const GREEN_RADIUS: i32 = 2;
const HOUSE_RADIUS: i32 = 50;

fn open_land(t: Terrain) -> bool {
    matches!(
        t,
        Terrain::Grass | Terrain::Farmland | Terrain::Forest | Terrain::Beach
    )
}

fn site_ok(map: &WorldMap, p: IVec2, rule: SiteRule) -> bool {
    let t = map.terrain(p);
    let neighbours = || {
        (-1..=1)
            .flat_map(move |dy| (-1..=1).map(move |dx| p + IVec2::new(dx, dy)))
            .filter(move |q| *q != p && map.in_bounds(*q))
    };
    match rule {
        SiteRule::Farmland => t == Terrain::Farmland,
        SiteRule::Grass => t == Terrain::Grass,
        SiteRule::Coast => {
            matches!(t, Terrain::Beach | Terrain::Grass)
                && neighbours().any(|q| map.terrain(q) == Terrain::Water)
        }
        SiteRule::ForestEdge => {
            t == Terrain::Forest
                && neighbours()
                    .any(|q| matches!(map.terrain(q), Terrain::Grass | Terrain::Farmland))
        }
        SiteRule::Upland => matches!(t, Terrain::Hills | Terrain::Mountain),
        SiteRule::Clearing => t == Terrain::Forest,
        SiteRule::Centre => matches!(t, Terrain::Grass | Terrain::Farmland | Terrain::Forest),
    }
}

/// Land tiles reachable on foot (8-connected, no water) from `center`,
/// within `radius` tiles. Buildings go only where villagers can walk to.
struct Reachable {
    origin: IVec2,
    side: i32,
    mask: Vec<bool>,
}

impl Reachable {
    fn new(map: &WorldMap, center: IVec2, radius: i32) -> Self {
        let side = 2 * radius + 1;
        let origin = center - IVec2::splat(radius);
        let mut mask = vec![false; (side * side) as usize];
        let idx = |p: IVec2| {
            let l = p - origin;
            (l.x >= 0 && l.y >= 0 && l.x < side && l.y < side).then(|| (l.y * side + l.x) as usize)
        };
        let mut stack = Vec::new();
        if let Some(i) = idx(center)
            && map.in_bounds(center)
            && map.terrain(center) != Terrain::Water
        {
            mask[i] = true;
            stack.push(center);
        }
        while let Some(p) = stack.pop() {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let q = p + IVec2::new(dx, dy);
                    if let Some(i) = idx(q)
                        && !mask[i]
                        && map.in_bounds(q)
                        && map.terrain(q) != Terrain::Water
                    {
                        mask[i] = true;
                        stack.push(q);
                    }
                }
            }
        }
        Self { origin, side, mask }
    }

    fn contains(&self, p: IVec2) -> bool {
        let l = p - self.origin;
        l.x >= 0
            && l.y >= 0
            && l.x < self.side
            && l.y < self.side
            && self.mask[(l.y * self.side + l.x) as usize]
    }
}

/// Furthest any building goes from the centre.
const MAX_RADIUS: i32 = 64;

/// Tiles within `radius` of `center`, nearest first (ties by tile index).
fn ring(map: &WorldMap, center: IVec2, min_r: i32, radius: i32) -> Vec<IVec2> {
    let mut tiles = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let d2 = dx * dx + dy * dy;
            let p = center + IVec2::new(dx, dy);
            if d2 > min_r * min_r && d2 <= radius * radius && map.in_bounds(p) {
                tiles.push((d2, map.index(p), p));
            }
        }
    }
    tiles.sort_unstable_by_key(|(d2, i, _)| (*d2, *i));
    tiles.into_iter().map(|(_, _, p)| p).collect()
}

/// Picks up to `count` sites for one workplace kind.
fn pick_sites(
    map: &WorldMap,
    center: IVec2,
    count: u32,
    def: &crate::data::WorkplaceDef,
    occupied: &Occupied,
    reachable: &Reachable,
) -> Vec<IVec2> {
    let (radius, spacing) = (def.radius.min(MAX_RADIUS), def.spacing);
    let mut chosen: Vec<IVec2> = Vec::new();
    let candidates = ring(map, center, GREEN_RADIUS, radius);
    for rule in &def.sites {
        for &p in &candidates {
            if chosen.len() as u32 >= count {
                return chosen;
            }
            if occupied.contains(&p) || !reachable.contains(p) || !site_ok(map, p, *rule) {
                continue;
            }
            if chosen
                .iter()
                .all(|c| (c.x - p.x).abs().max((c.y - p.y).abs()) >= spacing)
            {
                chosen.push(p);
            }
        }
    }
    chosen
}

fn spawn_building(world: &mut World, bundle: impl Bundle) -> Entity {
    let id = world.resource_mut::<NextSpawnId>().allocate();
    world.spawn((id, bundle)).id()
}

/// Builds a settlement's storehouse, bakery, smithy, houses and outlying
/// workplaces. `occupied` is shared across settlements so sites never
/// overlap. Returns fewer workplaces than requested when the land runs out.
pub fn build_settlement(
    world: &mut World,
    settlement: Entity,
    center: IVec2,
    req: &LayoutRequest,
    occupied: &mut Occupied,
) -> Layout {
    let econ = world.resource::<Economy>().clone();
    let map = world.resource::<WorldMap>().clone();
    let mut workplaces: BTreeMap<WorkplaceKind, Vec<Entity>> = BTreeMap::new();
    let reachable = Reachable::new(&map, center, MAX_RADIUS);

    let place = |world: &mut World,
                 kind: WorkplaceKind,
                 count: u32,
                 occupied: &mut Occupied|
     -> Vec<Entity> {
        let def = econ.workplace(kind);
        let sites = pick_sites(&map, center, count, def, occupied, &reachable);
        let recipes = econ.recipes_for(kind);
        let capacity = if kind == WorkplaceKind::Storehouse {
            econ.data.storehouse_capacity
        } else {
            econ.data.workplace_capacity
        };
        sites
            .into_iter()
            .enumerate()
            .map(|(i, tile)| {
                occupied.insert(tile);
                // Workplaces with several recipes (the smithy) alternate.
                let recipe = layout_recipe(&recipes, i);
                spawn_building(
                    world,
                    (
                        Building {
                            kind: BuildingKind::Workplace(kind),
                            settlement,
                            tile,
                        },
                        Position(tile),
                        Stockpile::with_capacity(capacity),
                        Workplace {
                            kind,
                            recipe,
                            workers: Vec::new(),
                        },
                    ),
                )
            })
            .collect()
    };

    // The storehouse goes nearest the centre; the green stays open.
    let storehouse = place(world, WorkplaceKind::Storehouse, 1, occupied)
        .first()
        .copied()
        .unwrap_or_else(|| {
            // No open land near the centre at all: put it on the centre.
            occupied.insert(center);
            spawn_building(
                world,
                (
                    Building {
                        kind: BuildingKind::Workplace(WorkplaceKind::Storehouse),
                        settlement,
                        tile: center,
                    },
                    Position(center),
                    Stockpile::with_capacity(econ.data.storehouse_capacity),
                    Workplace {
                        kind: WorkplaceKind::Storehouse,
                        recipe: None,
                        workers: Vec::new(),
                    },
                ),
            )
        });
    workplaces.insert(WorkplaceKind::Storehouse, vec![storehouse]);

    // Workshops near the centre, then houses, then outlying workplaces.
    let slots = |kind: WorkplaceKind| econ.workplace(kind).slots.max(1);
    for kind in [WorkplaceKind::Bakery, WorkplaceKind::Smithy] {
        let workers = req.workers.get(&kind).copied().unwrap_or(0);
        if workers > 0 {
            let built = place(world, kind, workers.div_ceil(slots(kind)), occupied);
            workplaces.insert(kind, built);
        }
    }

    let mut houses = Vec::new();
    for p in ring(&map, center, GREEN_RADIUS + 1, HOUSE_RADIUS) {
        if houses.len() as u32 >= req.houses {
            break;
        }
        // Every other tile both ways leaves lanes between houses.
        if p.x % 2 != 0
            || p.y % 2 != 0
            || occupied.contains(&p)
            || !reachable.contains(p)
            || !open_land(map.terrain(p))
        {
            continue;
        }
        occupied.insert(p);
        houses.push(spawn_building(
            world,
            (
                Building {
                    kind: BuildingKind::House,
                    settlement,
                    tile: p,
                },
                Position(p),
                Stockpile::with_capacity(econ.data.home_capacity),
                Home::default(),
            ),
        ));
    }

    for kind in [
        WorkplaceKind::Farm,
        WorkplaceKind::FishingSpot,
        WorkplaceKind::Pasture,
        WorkplaceKind::WoodcutterCamp,
        WorkplaceKind::Mine,
    ] {
        let workers = req.workers.get(&kind).copied().unwrap_or(0);
        if workers > 0 {
            let built = place(world, kind, workers.div_ceil(slots(kind)), occupied);
            workplaces.insert(kind, built);
        }
    }

    Layout {
        storehouse,
        houses,
        workplaces,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SIM-64: a smithy's recipe can be flipped to either smithing recipe,
    /// never to one made elsewhere; the layout alternates in file order.
    #[test]
    fn set_recipe_checks_the_workplace() {
        let econ = Economy::default_data();
        let smithing = econ.recipes_for(WorkplaceKind::Smithy);
        assert_eq!(layout_recipe(&smithing, 0), Some(smithing[0]));
        assert_eq!(layout_recipe(&smithing, 1), Some(smithing[1]));
        assert_eq!(layout_recipe(&smithing, 2), Some(smithing[0]));
        assert_eq!(layout_recipe(&[], 3), None);
        let mut wp = Workplace {
            kind: WorkplaceKind::Smithy,
            recipe: Some(smithing[0]),
            workers: Vec::new(),
        };
        set_recipe(&econ, &mut wp, smithing[1]).unwrap();
        assert_eq!(wp.recipe, Some(smithing[1]));
        let baking = econ.recipes_for(WorkplaceKind::Bakery)[0];
        assert!(set_recipe(&econ, &mut wp, baking).is_err());
        assert!(set_recipe(&econ, &mut wp, RecipeId(999)).is_err());
        assert_eq!(wp.recipe, Some(smithing[1]), "a refusal changes nothing");
    }

    #[test]
    fn ring_is_sorted_nearest_first() {
        let map = sim_world::generate_world(
            1,
            &sim_world::WorldGenSettings::default_data().with_size(128),
            1,
        )
        .map;
        let r = ring(&map, IVec2::new(64, 64), 0, 3);
        let d = |p: &IVec2| (p.x - 64).pow(2) + (p.y - 64).pow(2);
        assert!(r.windows(2).all(|w| d(&w[0]) <= d(&w[1])));
        assert!(
            !r.contains(&IVec2::new(64, 64)),
            "centre excluded when min_r = 0"
        );
    }
}
