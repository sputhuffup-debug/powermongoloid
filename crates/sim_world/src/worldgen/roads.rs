//! Road network: which settlements to link, and deterministic A* routing.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

use bevy_math::IVec2;

use crate::map::Terrain;
use crate::settings::{RoadSettings, WorldGenSettings};

/// Pairs of settlement indices (a < b) to link: a minimum spanning tree so
/// every settlement is connected, plus each settlement's nearest neighbours
/// within range. Sorted shortest first so later roads reuse earlier ones.
pub(crate) fn links(centers: &[IVec2], s: &WorldGenSettings) -> Vec<(usize, usize)> {
    let n = centers.len();
    let dist = |a: usize, b: usize| centers[a].as_vec2().distance(centers[b].as_vec2());
    let mut edges = BTreeSet::new();

    // Prim's algorithm; ties go to the lowest index.
    if n > 1 {
        let mut in_tree = vec![false; n];
        let mut best = vec![(f32::INFINITY, 0usize); n];
        in_tree[0] = true;
        for j in 1..n {
            best[j] = (dist(0, j), 0);
        }
        for _ in 1..n {
            let next = (0..n)
                .filter(|&j| !in_tree[j])
                .min_by(|&a, &b| best[a].0.total_cmp(&best[b].0).then(a.cmp(&b)))
                .expect("a vertex remains");
            in_tree[next] = true;
            let parent = best[next].1;
            edges.insert((parent.min(next), parent.max(next)));
            for j in 0..n {
                if !in_tree[j] && dist(next, j) < best[j].0 {
                    best[j] = (dist(next, j), next);
                }
            }
        }
    }

    let max_link = s.scaled(s.roads.max_link_tiles_at_2048);
    for a in 0..n {
        let mut others: Vec<usize> = (0..n).filter(|&b| b != a).collect();
        others.sort_by(|&x, &y| dist(a, x).total_cmp(&dist(a, y)).then(x.cmp(&y)));
        for &b in others.iter().take(s.roads.nearest_neighbours as usize) {
            if dist(a, b) <= max_link {
                edges.insert((a.min(b), a.max(b)));
            }
        }
    }

    let mut edges: Vec<(usize, usize)> = edges.into_iter().collect();
    edges.sort_by(|&(a, b), &(c, d)| dist(a, b).total_cmp(&dist(c, d)).then((a, b).cmp(&(c, d))));
    edges
}

/// Cost to enter a tile, x10 for orthogonal steps; `None` is impassable.
fn tile_cost(t: Terrain, r: &RoadSettings) -> Option<u32> {
    Some(match t {
        Terrain::Water => return None,
        Terrain::Grass => r.cost_grass,
        Terrain::Farmland => r.cost_farmland,
        Terrain::Beach => r.cost_beach,
        Terrain::Forest => r.cost_forest,
        Terrain::Hills => r.cost_hills,
        Terrain::Mountain => r.cost_mountain,
        Terrain::Road => r.cost_road,
    })
}

/// Integer-cost A* from `from` to `to`, 8-connected, inside a box around the
/// endpoints (`margin` tiles; `None` = whole map). Costs are integers and
/// ties break on tile index, so the path is fully deterministic.
pub(crate) fn route(
    terrain: &[Terrain],
    height: &[f32],
    size: usize,
    from: IVec2,
    to: IVec2,
    r: &RoadSettings,
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

    let min_step = r.cost_road as u64;
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
            let Some(cost) = tile_cost(terrain[global(q)], r) else {
                continue;
            };
            let step = if dx != 0 && dy != 0 { 14 } else { 10 };
            let climb = (height[global(q)] - height[global(p)]).abs() * r.slope_cost;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> RoadSettings {
        crate::settings::WorldGenSettings::default_data().roads
    }

    #[test]
    fn routes_around_water() {
        // 9x9 grass with a water wall at x=4 except a gap at y=8.
        let size = 9;
        let mut terrain = vec![Terrain::Grass; size * size];
        for y in 0..8 {
            terrain[y * size + 4] = Terrain::Water;
        }
        let height = vec![0.5; size * size];
        let path = route(
            &terrain,
            &height,
            size,
            IVec2::new(0, 0),
            IVec2::new(8, 0),
            &settings(),
            None,
        )
        .expect("a path exists");
        assert_eq!(path.first(), Some(&IVec2::new(0, 0)));
        assert_eq!(path.last(), Some(&IVec2::new(8, 0)));
        assert!(
            path.iter()
                .all(|p| terrain[p.y as usize * size + p.x as usize] != Terrain::Water)
        );
        assert!(
            path.windows(2)
                .all(|w| (w[1] - w[0]).abs().max_element() == 1)
        );
    }

    #[test]
    fn no_path_across_the_sea() {
        let size = 5;
        let mut terrain = vec![Terrain::Grass; size * size];
        for y in 0..size {
            terrain[y * size + 2] = Terrain::Water;
        }
        let height = vec![0.5; size * size];
        assert!(
            route(
                &terrain,
                &height,
                size,
                IVec2::ZERO,
                IVec2::new(4, 0),
                &settings(),
                None
            )
            .is_none()
        );
    }
}
