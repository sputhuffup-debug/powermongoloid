//! Wild food (SIM-38): a landless band lives off the land with a `Forage`
//! order. Not to be confused with `orders::forage`, which is an army's
//! automatic gathering from the settlements it passes.
//!
//! - [`WildFood`]: the island cut into [`CELL`]-tile patches. A full patch
//!   holds progression.ron `forage.per_tile` food value per grass or forest
//!   tile; foraging empties it and it grows back daily ([`regrow`]).
//! - [`harvest`]: one harvest of a foraging band, called by the orders
//!   step every `forage.every_ticks` while the band holds still.
//!
//! Patches are visited in index order (row-major), so harvests are
//! deterministic; the food is the least nourishing food good, added to the
//! baggage with `Stockpile::add` (goods from nothing, like a farm's
//! harvest, not a move between two stockpiles).

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use rand::RngExt;
use sim_core::rng::rng_key;
use sim_core::{
    Calendar, FactionModifiers, Position, SimSeed, SimTick, SpawnId, Stat, StateHash, StateHasher,
};
use sim_economy::{Economy, Stockpile};
use sim_world::{Terrain, WorldMap};

use crate::components::{ArmyStats, Captain};
use crate::progression::{ForageData, Progression};

/// Edge of a wild-food patch, in tiles.
pub const CELL: u32 = 16;

/// The island's wild food (SIM-38): one patch per [`CELL`] x [`CELL`]
/// tiles, row-major. Empty (`per_side` 0) until built: at run setup, or by
/// the first `Forage` order elsewhere. Hashed.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct WildFood {
    /// Patch edge in tiles ([`CELL`]).
    pub cell: u32,
    /// Patches per side of the map.
    pub per_side: u32,
    /// Grass and forest tiles in each patch: its capacity is this times
    /// `forage.per_tile`.
    pub tiles: Vec<u16>,
    /// Food value left in each patch.
    pub level: Vec<f32>,
}

impl StateHash for WildFood {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.cell);
        h.u32(self.per_side);
        h.u32(self.level.len() as u32);
        for (t, l) in self.tiles.iter().zip(&self.level) {
            h.u16(*t);
            h.f32(*l);
        }
    }
}

impl WildFood {
    /// Full patches for `map`: each holds `per_tile` per grass or forest
    /// tile.
    pub fn build(map: &WorldMap, per_tile: f32) -> Self {
        let size = map.size();
        let per_side = size.div_ceil(CELL);
        let mut tiles = vec![0u16; (per_side * per_side) as usize];
        for (i, t) in map.terrain_slice().iter().enumerate() {
            if matches!(t, Terrain::Grass | Terrain::Forest) {
                let p = map.pos(i);
                let c = (p.y as u32 / CELL) * per_side + p.x as u32 / CELL;
                tiles[c as usize] += 1;
            }
        }
        let level = tiles.iter().map(|&n| n as f32 * per_tile).collect();
        Self {
            cell: CELL,
            per_side,
            tiles,
            level,
        }
    }

    /// Whether the patches exist yet.
    pub fn is_built(&self) -> bool {
        self.per_side > 0
    }

    /// A full patch's food value.
    pub fn capacity(&self, patch: usize, per_tile: f32) -> f32 {
        self.tiles.get(patch).map_or(0.0, |&n| n as f32 * per_tile)
    }

    /// Patches with any tile within `radius` of `at`, in index order.
    pub fn patches_within(&self, at: IVec2, radius: f32) -> Vec<usize> {
        if !self.is_built() {
            return Vec::new();
        }
        let cell = self.cell as i32;
        let last = self.per_side as i32 - 1;
        let r = radius.max(0.0);
        let lo = |v: i32| {
            ((v as f32 - r).floor() as i32)
                .div_euclid(cell)
                .clamp(0, last)
        };
        let hi = |v: i32| {
            ((v as f32 + r).ceil() as i32)
                .div_euclid(cell)
                .clamp(0, last)
        };
        let mut out = Vec::new();
        for cy in lo(at.y)..=hi(at.y) {
            for cx in lo(at.x)..=hi(at.x) {
                // Nearest point of the patch to `at`.
                let x0 = cx * cell;
                let y0 = cy * cell;
                let nx = at.x.clamp(x0, x0 + cell - 1);
                let ny = at.y.clamp(y0, y0 + cell - 1);
                if (IVec2::new(nx, ny) - at).as_vec2().length() <= r {
                    out.push((cy * self.per_side as i32 + cx) as usize);
                }
            }
        }
        out
    }

    /// Takes up to `want` food value from the patches within `radius` of
    /// `at`, emptying them in index order. Returns what was taken.
    pub fn take(&mut self, at: IVec2, radius: f32, want: f32) -> f32 {
        let mut taken = 0.0;
        for p in self.patches_within(at, radius) {
            if taken >= want {
                break;
            }
            let t = self.level[p].min(want - taken);
            if t > 0.0 {
                self.level[p] -= t;
                taken += t;
            }
        }
        taken
    }

    /// Food value left within `radius` of `at`.
    pub fn available(&self, at: IVec2, radius: f32) -> f32 {
        self.patches_within(at, radius)
            .into_iter()
            .map(|p| self.level[p])
            .sum()
    }

    /// One day's regrowth: every patch gains `fraction` of its capacity,
    /// up to full.
    pub fn grow(&mut self, per_tile: f32, fraction: f32) {
        for (l, &n) in self.level.iter_mut().zip(&self.tiles) {
            let cap = n as f32 * per_tile;
            if *l < cap {
                *l = (*l + cap * fraction).min(cap);
            }
        }
    }
}

/// Builds [`WildFood`] from the map if it isn't built yet (serial steps
/// only: run setup, the orders step).
pub fn ensure_built(world: &mut World) {
    if world.resource::<WildFood>().is_built() {
        return;
    }
    let per_tile = world.resource::<Progression>().data.forage.per_tile;
    let Some(map) = world.get_resource::<WorldMap>() else {
        return;
    };
    let wild = WildFood::build(map, per_tile);
    world.insert_resource(wild);
}

/// The progress driver's daily wild-food step (SIM-38): every patch grows
/// back `forage.regrow_per_day` of its capacity. Runs once the patches
/// exist, which in practice means in a run (built at setup); elsewhere only
/// after someone foraged. Span `sim_military::wild::regrow`.
pub fn regrow(world: &mut World) {
    let tick = world.resource::<SimTick>().0;
    if !world.resource::<Calendar>().on_day(tick) || !world.resource::<WildFood>().is_built() {
        return;
    }
    let _span = tracing::info_span!("sim_military::wild::regrow").entered();
    let f = world.resource::<Progression>().data.forage;
    world
        .resource_mut::<WildFood>()
        .grow(f.per_tile, f.regrow_per_day);
}

/// What one [`harvest`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Harvest {
    /// Food value added to the baggage (it may round to nothing).
    Gathered(f32),
    /// The baggage holds `gather_target_days` of food, or has no room.
    Full,
    /// Nothing is left within the radius.
    Exhausted,
}

/// One harvest by captain `e`'s band (SIM-38): `forage.per_member` per
/// member, times the faction's `ForageYield` modifier, from the patches
/// within `radius` of the captain, capped by what the baggage lacks for
/// `gather_target_days`. The food value is turned into units of the least
/// nourishing food with stochastic rounding (stream
/// `sim_military::wild ^ captain`, drawn only for a fraction) and added to
/// the baggage. Counted in `FactionStats::foraged`. Call it only from a
/// serial step (the orders step's `Forage` arm).
pub fn harvest(world: &mut World, e: Entity, radius: u16, tick: u64) -> Harvest {
    let Some(c) = world.get::<Captain>(e) else {
        return Harvest::Exhausted;
    };
    let faction = c.faction;
    let room_value = crate::orders::baggage_room(world, e);
    let room_units = world.get::<Stockpile>(e).map_or(0, |b| b.room());
    if room_value <= 0.0 || room_units == 0 {
        return Harvest::Full;
    }
    ensure_built(world);
    let econ = world.resource::<Economy>().clone();
    let Some(&food) = econ.foods_by_value().last() else {
        return Harvest::Exhausted;
    };
    let value = econ.food_value(food);
    let at = world.get::<Position>(e).map_or(IVec2::ZERO, |p| p.0);
    let data: ForageData = world.resource::<Progression>().data.forage;
    let mult = world
        .resource::<FactionModifiers>()
        .mult(faction, Stat::ForageYield)
        .max(0.0);
    let members = crate::orders::army_size(world, e) as f32;
    let r = radius as f32;
    if world.resource::<WildFood>().available(at, r) <= 0.0 {
        return Harvest::Exhausted;
    }
    let mut want = data.per_member * members;
    if mult > 0.0 {
        want = want.min(room_value / mult);
    }
    let taken = world.resource_mut::<WildFood>().take(at, r, want);
    let gathered = taken * mult;
    let exact = gathered / value;
    let mut units = exact.floor();
    let frac = exact - units;
    if frac > 0.0 {
        let id = world.get::<SpawnId>(e).map_or(0, |s| s.0);
        let mut rng = world
            .resource::<SimSeed>()
            .stream(rng_key("sim_military::wild") ^ id, tick);
        if rng.random::<f32>() < frac {
            units += 1.0;
        }
    }
    let units = (units as u32).min(room_units);
    let added = world
        .get_mut::<Stockpile>(e)
        .map_or(0, |mut b| b.add(food, units));
    let got = added as f32 * value;
    world.resource_mut::<ArmyStats>().of(faction).foraged += got;
    Harvest::Gathered(got)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SIM-38: harvests empty the patches within the radius in index
    /// order, leave the rest, and a day's regrowth refills a fraction, up
    /// to full.
    #[test]
    fn patches_deplete_and_regrow() {
        // 64 x 64 grass with a band of water down x = 16..32: patches of
        // 16 x 16, four per side.
        let mut map = WorldMap::flat(64, 16, Terrain::Grass);
        for y in 0..64 {
            for x in 16..32 {
                map.set_terrain(IVec2::new(x, y), Terrain::Water);
            }
        }
        map.set_terrain(IVec2::new(0, 0), Terrain::Forest);
        map.set_terrain(IVec2::new(1, 0), Terrain::Mountain);
        let per_tile = 0.01;
        let mut w = WildFood::build(&map, per_tile);
        assert_eq!((w.cell, w.per_side), (16, 4));
        assert_eq!(w.tiles[0], 255, "forest counts, mountain doesn't");
        assert_eq!(w.tiles[1], 0, "water");
        assert_eq!(w.tiles[2], 256);
        assert!((w.capacity(2, per_tile) - 2.56).abs() < 1e-5);
        assert_eq!(w.level[2], w.capacity(2, per_tile), "full at the start");

        // From (8, 8) with radius 12: patches 0, 1 (water), 4 and 5 (its
        // nearest tile, (16, 16), is 11.3 away).
        let at = IVec2::new(8, 8);
        assert_eq!(w.patches_within(at, 12.0), vec![0, 1, 4, 5]);
        assert_eq!(w.patches_within(at, 10.0), vec![0, 1, 4]);
        assert_eq!(w.patches_within(at, 7.0), vec![0], "only its own");
        let before = w.clone();
        let full0 = w.level[0];
        // Index order: patch 0 empties first, then 4.
        let got = w.take(at, 12.0, full0 + 1.0);
        assert!((got - (full0 + 1.0)).abs() < 1e-5);
        assert_eq!(w.level[0], 0.0);
        assert!((w.level[4] - (w.capacity(4, per_tile) - 1.0)).abs() < 1e-5);
        assert_eq!(w.level[5], before.level[5], "untouched");
        assert_eq!(w.level[2], before.level[2], "out of range");
        // Asking for more than is left takes what's there.
        let left = w.available(at, 12.0);
        assert!((w.take(at, 12.0, 100.0) - left).abs() < 1e-4);
        assert_eq!(w.available(at, 12.0), 0.0, "emptied");

        // A day's regrowth refills a tenth, up to full.
        w.grow(per_tile, 0.1);
        assert!((w.level[0] - w.capacity(0, per_tile) * 0.1).abs() < 1e-5);
        assert_eq!(w.level[1], 0.0, "water grows nothing");
        assert_eq!(w.level[2], before.level[2], "a full patch stays full");
        for _ in 0..20 {
            w.grow(per_tile, 0.1);
        }
        assert_eq!(w, before, "regrown to full");
    }
}
