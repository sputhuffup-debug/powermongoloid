//! `SpatialGrid`: moving entities bucketed in 8-tile cells, for neighbour and
//! picking queries. Rebuilt every tick in Bookkeeping, in SpawnId order.
//! Small cells keep crowd queries (separation, nearest enemy) cheap.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use sim_core::{Motion, Position, SpawnId, StateHash, StateHasher};

use crate::map::WorldMap;

/// Marks entities the grid tracks (villagers now; soldiers from M3).
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mobile;

impl StateHash for Mobile {
    fn hash_state(&self, _h: &mut StateHasher) {}
}

/// Derived from positions every tick, so not part of the state hash.
#[derive(Resource, Debug, Default)]
pub struct SpatialGrid {
    /// Cell edge in tiles.
    chunk_size: i32,
    per_side: i32,
    cells: Vec<Vec<(SpawnId, Entity, Vec2)>>,
    /// Cells holding anyone, so a rebuild touches only those.
    used: Vec<u32>,
}

impl SpatialGrid {
    fn cell_of(&self, p: Vec2) -> Option<usize> {
        if self.per_side == 0 {
            return None;
        }
        let c = (p / self.chunk_size as f32).floor().as_ivec2();
        (c.x >= 0 && c.y >= 0 && c.x < self.per_side && c.y < self.per_side)
            .then(|| (c.y * self.per_side + c.x) as usize)
    }

    /// Entities within `radius` tiles of `p`, nearest first (ties by SpawnId).
    pub fn within(&self, p: Vec2, radius: f32) -> Vec<(Entity, f32)> {
        let mut out: Vec<(SpawnId, Entity, f32)> = Vec::new();
        if self.per_side == 0 {
            return Vec::new();
        }
        let lo = ((p - radius) / self.chunk_size as f32)
            .floor()
            .as_ivec2()
            .max(IVec2::ZERO);
        let hi = ((p + radius) / self.chunk_size as f32)
            .floor()
            .as_ivec2()
            .min(IVec2::splat(self.per_side - 1));
        for cy in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                for (id, e, q) in &self.cells[(cy * self.per_side + cx) as usize] {
                    let d = q.distance(p);
                    if d <= radius {
                        out.push((*id, *e, d));
                    }
                }
            }
        }
        out.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
        out.into_iter().map(|(_, e, d)| (e, d)).collect()
    }

    /// Calls `f(entity, position, distance)` for every entity within
    /// `radius` of `p`, without allocating. The visiting order is fixed
    /// (cell by cell, SpawnId order within a cell), so sums over it are
    /// deterministic.
    pub fn for_each_within(&self, p: Vec2, radius: f32, mut f: impl FnMut(Entity, Vec2, f32)) {
        if self.per_side == 0 {
            return;
        }
        let lo = ((p - radius) / self.chunk_size as f32)
            .floor()
            .as_ivec2()
            .max(IVec2::ZERO);
        let hi = ((p + radius) / self.chunk_size as f32)
            .floor()
            .as_ivec2()
            .min(IVec2::splat(self.per_side - 1));
        let r2 = radius * radius;
        for cy in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                for (_, e, q) in &self.cells[(cy * self.per_side + cx) as usize] {
                    let d2 = q.distance_squared(p);
                    if d2 <= r2 {
                        f(*e, *q, d2.sqrt());
                    }
                }
            }
        }
    }

    /// Like [`Self::for_each_within`], but stops after checking `budget`
    /// entries, so dense crowds cost a bounded amount. The visiting order is
    /// fixed, so which entries are seen is deterministic.
    pub fn for_each_within_budget(
        &self,
        p: Vec2,
        radius: f32,
        mut budget: usize,
        mut f: impl FnMut(Entity, Vec2, f32),
    ) {
        if self.per_side == 0 {
            return;
        }
        let lo = ((p - radius) / self.chunk_size as f32)
            .floor()
            .as_ivec2()
            .max(IVec2::ZERO);
        let hi = ((p + radius) / self.chunk_size as f32)
            .floor()
            .as_ivec2()
            .min(IVec2::splat(self.per_side - 1));
        let r2 = radius * radius;
        for cy in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                for (_, e, q) in &self.cells[(cy * self.per_side + cx) as usize] {
                    if budget == 0 {
                        return;
                    }
                    budget -= 1;
                    let d2 = q.distance_squared(p);
                    if d2 <= r2 {
                        f(*e, *q, d2.sqrt());
                    }
                }
            }
        }
    }

    /// Entities in the grid.
    pub fn len(&self) -> usize {
        self.cells.iter().map(Vec::len).sum()
    }

    /// Whether the grid is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Grid cell edge in tiles.
const CELL: i32 = 8;

/// Continuous tile position of an entity (tile centre plus offset).
pub fn world_pos(pos: &Position, motion: Option<&Motion>) -> Vec2 {
    pos.0.as_vec2() + Vec2::splat(0.5) + motion.map_or(Vec2::ZERO, |m| m.offset)
}

pub(crate) fn rebuild_grid(
    map: Option<Res<WorldMap>>,
    mut grid: ResMut<SpatialGrid>,
    q: Query<(Entity, &SpawnId, &Position, Option<&Motion>), With<Mobile>>,
) {
    let _span = tracing::info_span!("sim_world::rebuild_grid").entered();
    let Some(map) = map else { return };
    let per_side = (map.size() as i32 + CELL - 1) / CELL;
    if grid.per_side != per_side {
        grid.per_side = per_side;
        grid.chunk_size = CELL;
        grid.cells = vec![Vec::new(); (per_side * per_side) as usize];
        grid.used.clear();
    }
    let mut used = std::mem::take(&mut grid.used);
    for &c in &used {
        grid.cells[c as usize].clear();
    }
    used.clear();
    for (e, id, pos, motion) in &q {
        let p = world_pos(pos, motion);
        if let Some(c) = grid.cell_of(p) {
            if grid.cells[c].is_empty() {
                used.push(c as u32);
            }
            grid.cells[c].push((*id, e, p));
        }
    }
    for &c in &used {
        grid.cells[c as usize].sort_unstable_by_key(|(id, ..)| *id);
    }
    grid.used = used;
}
