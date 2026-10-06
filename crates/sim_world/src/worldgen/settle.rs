//! Settlement placement: score candidate tiles, then place greedily with a
//! minimum spacing, relaxing it until at least the minimum count fits.

use bevy_math::IVec2;

use crate::map::Terrain;
use crate::settings::WorldGenSettings;

pub(crate) struct SiteInputs<'a> {
    pub size: usize,
    pub terrain: &'a [Terrain],
    pub fertility: &'a [u8],
    pub slope: &'a [f32],
    pub water_dist: &'a [u8],
    pub landmass: &'a [u32],
    pub main_landmass: u32,
}

/// Terrain a settlement centre may sit on.
pub fn suitable(t: Terrain) -> bool {
    matches!(t, Terrain::Grass | Terrain::Forest)
}

/// Maximum slope (height difference to a neighbour) for a settlement centre.
pub const MAX_SITE_SLOPE: f32 = 0.004;

fn score(inp: &SiteInputs, i: usize) -> Option<f32> {
    let t = inp.terrain[i];
    if !suitable(t) || inp.landmass[i] != inp.main_landmass {
        return None;
    }
    let (wd, slope) = (inp.water_dist[i], inp.slope[i]);
    if wd < 3 || slope > MAX_SITE_SLOPE {
        return None;
    }
    let water_bonus = match wd {
        0..=12 => 0.12,
        13..=30 => 0.05,
        _ => 0.0,
    };
    let forest_penalty = if t == Terrain::Forest { 0.1 } else { 0.0 };
    Some(inp.fertility[i] as f32 / 255.0 + water_bonus - slope * 50.0 - forest_penalty)
}

/// Picks settlement centres. Returns tile indices in placement order.
pub(crate) fn place(inp: &SiteInputs, s: &WorldGenSettings) -> Vec<usize> {
    let st = &s.settlements;
    let size = inp.size;
    let main_area = inp
        .landmass
        .iter()
        .filter(|&&l| l == inp.main_landmass)
        .count() as f32;
    // Spread settlements over the island: spacing from land area per site,
    // but never below the configured minimum.
    let mut spacing = s
        .scaled(st.min_spacing_tiles_at_2048)
        .max(0.75 * (main_area / st.target as f32).sqrt());

    // Best candidate per grid cell keeps the candidate list small.
    let cell = (s.scaled(st.min_spacing_tiles_at_2048) / 3.0).max(4.0) as usize;
    let cells_per_side = size.div_ceil(cell);
    let mut best: Vec<Option<(f32, usize)>> = vec![None; cells_per_side * cells_per_side];
    for i in 0..inp.terrain.len() {
        if let Some(sc) = score(inp, i) {
            let (x, y) = (i % size, i / size);
            let c = (y / cell) * cells_per_side + x / cell;
            // Strictly greater: ties keep the lowest index.
            if best[c].is_none_or(|(b, _)| sc > b) {
                best[c] = Some((sc, i));
            }
        }
    }
    let mut candidates: Vec<(f32, usize)> = best.into_iter().flatten().collect();
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

    let pos = |i: usize| IVec2::new((i % size) as i32, (i / size) as i32);
    for _ in 0..30 {
        let mut chosen: Vec<usize> = Vec::new();
        for &(_, i) in &candidates {
            if chosen.len() as u32 >= st.target {
                break;
            }
            let p = pos(i).as_vec2();
            if chosen
                .iter()
                .all(|&j| pos(j).as_vec2().distance(p) >= spacing)
            {
                chosen.push(i);
            }
        }
        if chosen.len() as u32 >= st.min || spacing < 4.0 {
            chosen.truncate(st.max as usize);
            return chosen;
        }
        spacing *= 0.9;
    }
    Vec::new()
}
