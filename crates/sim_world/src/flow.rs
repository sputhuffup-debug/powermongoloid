//! Flow fields for crowds (SIM-30): an integer Dijkstra field toward one
//! goal tile over a window of the map. Every member of an army descends the
//! same field, so a crowd flows around lakes and mountains without a path
//! search per soldier. Fields are pure functions of the map, the goal and
//! the window, so the cache is not simulation state.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};

use crate::map::WorldMap;
use crate::path::TravelCosts;

const NEIGHBOURS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Cost to reach the goal from each tile of a window; `u32::MAX` where the
/// goal can't be reached.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowField {
    pub goal: IVec2,
    pub lo: IVec2,
    pub hi: IVec2,
    width: i32,
    cost: Vec<u32>,
}

impl FlowField {
    /// Builds the field over the window `lo..=hi` (clamped to the map).
    pub fn build(map: &WorldMap, costs: &TravelCosts, goal: IVec2, lo: IVec2, hi: IVec2) -> Self {
        let _span = tracing::info_span!("sim_world::flow::build").entered();
        let size = map.size() as i32;
        let lo = lo.max(IVec2::ZERO);
        let hi = hi.min(IVec2::splat(size - 1));
        let width = hi.x - lo.x + 1;
        let height = hi.y - lo.y + 1;
        let mut field = Self {
            goal,
            lo,
            hi,
            width,
            cost: vec![u32::MAX; (width.max(0) * height.max(0)) as usize],
        };
        let Some(start) = field.local(goal) else {
            return field;
        };
        let terrain = map.terrain_slice();
        let heights = map.height_slice();
        let global = |p: IVec2| (p.y * size + p.x) as usize;
        let mut heap = BinaryHeap::new();
        field.cost[start] = 0;
        heap.push(Reverse((0u32, start as u32)));
        while let Some(Reverse((c, i))) = heap.pop() {
            let i = i as usize;
            if c > field.cost[i] {
                continue;
            }
            let p = field.unlocal(i);
            // Costs are symmetric enough for crowds: the cost of stepping
            // from q into p, so descending the field walks the cheap way.
            let Some(enter) = costs.per_terrain[terrain[global(p)] as usize] else {
                continue;
            };
            for (dx, dy) in NEIGHBOURS {
                let q = p + IVec2::new(dx, dy);
                let Some(j) = field.local(q) else { continue };
                if costs.per_terrain[terrain[global(q)] as usize].is_none() {
                    continue;
                }
                // No cutting corners past impassable tiles.
                if dx != 0
                    && dy != 0
                    && (costs.per_terrain[terrain[global(IVec2::new(q.x, p.y))] as usize].is_none()
                        || costs.per_terrain[terrain[global(IVec2::new(p.x, q.y))] as usize]
                            .is_none())
                {
                    continue;
                }
                let step = if dx != 0 && dy != 0 { 14 } else { 10 };
                let climb = (heights[global(q)] - heights[global(p)]).abs() * costs.slope_cost;
                let nc = c + enter * step + climb as u32;
                if nc < field.cost[j] {
                    field.cost[j] = nc;
                    heap.push(Reverse((nc, j as u32)));
                }
            }
        }
        field
    }

    fn local(&self, p: IVec2) -> Option<usize> {
        (p.x >= self.lo.x && p.y >= self.lo.y && p.x <= self.hi.x && p.y <= self.hi.y)
            .then(|| ((p.y - self.lo.y) * self.width + (p.x - self.lo.x)) as usize)
    }

    fn unlocal(&self, i: usize) -> IVec2 {
        IVec2::new(i as i32 % self.width, i as i32 / self.width) + self.lo
    }

    /// Cost to the goal from `p`, if `p` is in the window and reaches it.
    pub fn cost(&self, p: IVec2) -> Option<u32> {
        self.local(p)
            .map(|i| self.cost[i])
            .filter(|c| *c != u32::MAX)
    }

    pub fn contains(&self, p: IVec2) -> bool {
        self.local(p).is_some()
    }

    /// The cheapest neighbouring tile toward the goal (ties: fixed
    /// neighbour order), or `None` at the goal or off the field.
    pub fn next_tile(&self, p: IVec2) -> Option<IVec2> {
        let here = self.cost(p)?;
        let mut best: Option<(u32, IVec2)> = None;
        for (dx, dy) in NEIGHBOURS {
            let q = p + IVec2::new(dx, dy);
            if let Some(c) = self.cost(q)
                && c < here
                && best.is_none_or(|(b, _)| c < b)
            {
                best = Some((c, q));
            }
        }
        best.map(|(_, q)| q)
    }

    /// Unit direction toward the goal from a continuous position.
    pub fn direction(&self, p: Vec2) -> Option<Vec2> {
        let tile = p.floor().as_ivec2();
        let next = self.next_tile(tile)?;
        let d = next.as_vec2() + Vec2::splat(0.5) - p;
        (d.length_squared() > 1e-6).then(|| d.normalize())
    }
}

/// Cache key: goal and window corners.
pub type FlowKey = (IVec2, IVec2, IVec2);

/// Built fields by (goal, window). Cleared when the map changes; when full,
/// the least recently used field goes. A field missing from the cache is
/// rebuilt identically, so the cache never changes outcomes.
#[derive(Resource, Debug, Default)]
pub struct FlowCache {
    fields: BTreeMap<(i32, i32, i32, i32, i32, i32), (Arc<FlowField>, u64)>,
    map_revision: u64,
    uses: u64,
    pub builds: u64,
}

impl FlowCache {
    const MAX: usize = 64;

    pub fn get_or_build(
        &mut self,
        map: &WorldMap,
        costs: &TravelCosts,
        key: FlowKey,
    ) -> Arc<FlowField> {
        let revision = map.total_revision();
        if revision != self.map_revision {
            self.fields.clear();
            self.map_revision = revision;
        }
        let (g, lo, hi) = key;
        let k = (g.x, g.y, lo.x, lo.y, hi.x, hi.y);
        self.uses += 1;
        if let Some((f, used)) = self.fields.get_mut(&k) {
            *used = self.uses;
            return f.clone();
        }
        if self.fields.len() >= Self::MAX
            && let Some(oldest) = self
                .fields
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(k, _)| *k)
        {
            self.fields.remove(&oldest);
        }
        self.builds += 1;
        let f = Arc::new(FlowField::build(map, costs, g, lo, hi));
        self.fields.insert(k, (f.clone(), self.uses));
        f
    }

    /// A field already built for `key`, if cached.
    pub fn get(&self, key: FlowKey) -> Option<Arc<FlowField>> {
        let (g, lo, hi) = key;
        self.fields
            .get(&(g.x, g.y, lo.x, lo.y, hi.x, hi.y))
            .map(|(f, _)| f.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Terrain;

    fn costs() -> TravelCosts {
        let mut per_terrain = [Some(10); 8];
        per_terrain[Terrain::Water as usize] = None;
        TravelCosts {
            per_terrain,
            slope_cost: 0.0,
        }
    }

    /// A 32x32 grass map with a wall of water at x = 16 for y < 28.
    fn walled() -> WorldMap {
        let mut map = WorldMap::flat(32, 8, Terrain::Grass);
        for y in 0..28 {
            map.set_terrain(IVec2::new(16, y), Terrain::Water);
        }
        map
    }

    #[test]
    fn descending_the_field_goes_around_water() {
        let map = walled();
        let goal = IVec2::new(28, 4);
        let f = FlowField::build(&map, &costs(), goal, IVec2::ZERO, IVec2::splat(31));
        let mut p = IVec2::new(4, 4);
        let mut steps = 0;
        while p != goal {
            p = f.next_tile(p).expect("reachable");
            assert_ne!(map.terrain(p), Terrain::Water);
            steps += 1;
            assert!(steps < 200);
        }
        assert!(steps > 40, "must detour around the wall, took {steps}");
        assert!(f.cost(IVec2::new(16, 4)).is_none(), "water has no cost");
    }

    #[test]
    fn outside_the_window_there_is_no_field() {
        let map = walled();
        let f = FlowField::build(
            &map,
            &costs(),
            IVec2::new(5, 5),
            IVec2::ZERO,
            IVec2::splat(10),
        );
        assert!(f.cost(IVec2::new(20, 20)).is_none());
        assert!(f.next_tile(IVec2::new(5, 5)).is_none(), "at the goal");
    }

    #[test]
    fn cache_returns_the_same_field_and_clears_on_map_change() {
        let mut map = walled();
        let mut cache = FlowCache::default();
        let key = (IVec2::new(28, 4), IVec2::ZERO, IVec2::splat(31));
        let a = cache.get_or_build(&map, &costs(), key);
        let b = cache.get_or_build(&map, &costs(), key);
        assert!(Arc::ptr_eq(&a, &b));
        map.set_terrain(IVec2::new(2, 2), Terrain::Water);
        let c = cache.get_or_build(&map, &costs(), key);
        assert!(!Arc::ptr_eq(&a, &c));
        assert_eq!(cache.builds, 2);
    }
}
