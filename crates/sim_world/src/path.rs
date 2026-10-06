//! Deterministic tile pathfinding (A* with integer costs), shared by road
//! building and villager movement, plus a cache of found paths.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};
use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;

use crate::map::{Terrain, WorldMap};

/// Cost to enter a tile of each terrain (indexed by `Terrain as usize`),
/// per orthogonal step x10; `None` is impassable. Diagonal steps cost x14.
#[derive(Debug, Clone, PartialEq)]
pub struct TravelCosts {
    pub per_terrain: [Option<u32>; 8],
    /// Extra cost per unit of height difference (height is 0..1).
    pub slope_cost: f32,
}

impl TravelCosts {
    /// The cheapest passable terrain cost (keeps the A* heuristic admissible).
    pub fn min_cost(&self) -> u32 {
        self.per_terrain
            .iter()
            .flatten()
            .copied()
            .min()
            .unwrap_or(1)
            .max(1)
    }
}

/// Integer-cost A* from `from` to `to`, 8-connected, inside a box around the
/// endpoints (`margin` tiles; `None` = whole map). Costs are integers and
/// ties break on tile index, so the path is fully deterministic.
pub fn find_path(
    terrain: &[Terrain],
    height: &[f32],
    size: usize,
    from: IVec2,
    to: IVec2,
    costs: &TravelCosts,
    margin: Option<i32>,
) -> Option<Vec<IVec2>> {
    let (lo, hi) = match margin {
        Some(m) => (
            (from.min(to) - IVec2::splat(m)).max(IVec2::ZERO),
            (from.max(to) + IVec2::splat(m)).min(IVec2::splat(size as i32 - 1)),
        ),
        None => (IVec2::ZERO, IVec2::splat(size as i32 - 1)),
    };
    let w = (hi.x - lo.x + 1) as usize;
    let h = (hi.y - lo.y + 1) as usize;
    let local = |p: IVec2| (p.y - lo.y) as usize * w + (p.x - lo.x) as usize;
    let global = |p: IVec2| p.y as usize * size + p.x as usize;
    let unlocal = |i: usize| IVec2::new((i % w) as i32 + lo.x, (i / w) as i32 + lo.y);

    let min_step = costs.min_cost() as u64;
    let heuristic = |p: IVec2| {
        let d = (p - to).abs();
        let (a, b) = (d.x.max(d.y) as u64, d.x.min(d.y) as u64);
        (10 * a + 4 * b) * min_step
    };

    let mut g = vec![u64::MAX; w * h];
    let mut came = vec![u32::MAX; w * h];
    let mut heap = BinaryHeap::new();
    let start = local(from);
    g[start] = 0;
    heap.push(Reverse((heuristic(from), start)));

    while let Some(Reverse((_, i))) = heap.pop() {
        let p = unlocal(i);
        if p == to {
            let mut path = vec![p];
            let mut cur = i;
            while came[cur] != u32::MAX {
                cur = came[cur] as usize;
                path.push(unlocal(cur));
            }
            path.reverse();
            return Some(path);
        }
        let gi = g[i];
        for (dx, dy) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ] {
            let q = p + IVec2::new(dx, dy);
            if q.x < lo.x || q.y < lo.y || q.x > hi.x || q.y > hi.y {
                continue;
            }
            let Some(cost) = costs.per_terrain[terrain[global(q)] as usize] else {
                continue;
            };
            let step = if dx != 0 && dy != 0 { 14 } else { 10 };
            let climb = (height[global(q)] - height[global(p)]).abs() * costs.slope_cost;
            let ng = gi + (cost * step) as u64 + climb as u64;
            let j = local(q);
            if ng < g[j] {
                g[j] = ng;
                came[j] = i as u32;
                heap.push(Reverse((ng + heuristic(q), j)));
            }
        }
    }
    None
}

/// A path between two tiles, shared between agents travelling it.
pub type SharedPath = Arc<[IVec2]>;

/// Cache key: (from, to) tile coordinates.
type PathKey = ((i32, i32), (i32, i32));

/// Memoised paths keyed by (from, to). Paths are a pure function of the map
/// and the endpoints, so the cache is not simulation state; it is cleared
/// whenever any map chunk changes.
#[derive(Resource, Debug, Default)]
pub struct PathCache {
    paths: BTreeMap<PathKey, Option<SharedPath>>,
    map_revision: u64,
    pub searches: u64,
    pub hits: u64,
}

impl PathCache {
    /// Search margin around the endpoints, in tiles.
    pub const MARGIN: i32 = 48;

    pub fn get_or_find(
        &mut self,
        map: &WorldMap,
        costs: &TravelCosts,
        from: IVec2,
        to: IVec2,
    ) -> Option<SharedPath> {
        let revision = map.total_revision();
        if revision != self.map_revision {
            self.paths.clear();
            self.map_revision = revision;
        }
        let key = ((from.x, from.y), (to.x, to.y));
        if let Some(p) = self.paths.get(&key) {
            self.hits += 1;
            return p.clone();
        }
        self.searches += 1;
        let size = map.size() as usize;
        let found = find_path(
            map.terrain_slice(),
            map.height_slice(),
            size,
            from,
            to,
            costs,
            Some(Self::MARGIN),
        )
        .map(|p| SharedPath::from(p.into_boxed_slice()));
        self.paths.insert(key, found.clone());
        found
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grass_costs() -> TravelCosts {
        let mut per_terrain = [Some(10); 8];
        per_terrain[Terrain::Water as usize] = None;
        TravelCosts {
            per_terrain,
            slope_cost: 0.0,
        }
    }

    #[test]
    fn straight_line_on_open_ground() {
        let size = 16;
        let terrain = vec![Terrain::Grass; size * size];
        let height = vec![0.5; size * size];
        let p = find_path(
            &terrain,
            &height,
            size,
            IVec2::new(1, 1),
            IVec2::new(6, 1),
            &grass_costs(),
            None,
        )
        .unwrap();
        assert_eq!(p.len(), 6);
        assert!(p.iter().all(|t| t.y == 1));
    }

    #[test]
    fn start_equals_goal() {
        let size = 8;
        let terrain = vec![Terrain::Grass; size * size];
        let height = vec![0.5; size * size];
        let p = find_path(
            &terrain,
            &height,
            size,
            IVec2::new(3, 3),
            IVec2::new(3, 3),
            &grass_costs(),
            None,
        )
        .unwrap();
        assert_eq!(p, vec![IVec2::new(3, 3)]);
    }
}
