//! Height and moisture fields, and terrain classification from quantiles.

use std::collections::VecDeque;

use sim_core::SimSeed;
use sim_core::rng::rng_key;

use crate::map::Terrain;
use crate::noise::Perlin;
use crate::settings::WorldGenSettings;

/// Fills `out` (row-major, `size` x `size`) by calling `f(x, y)` for every
/// tile, splitting rows across `threads`. Each tile is independent, so the
/// result does not depend on the thread count.
pub(crate) fn par_fill<T: Send + Copy>(
    out: &mut [T],
    size: usize,
    threads: usize,
    f: impl Fn(usize, usize) -> T + Sync,
) {
    let rows_per_band = size.div_ceil(threads.max(1));
    std::thread::scope(|s| {
        for (band, slice) in out.chunks_mut(rows_per_band * size).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (i, v) in slice.iter_mut().enumerate() {
                    let idx = band * rows_per_band * size + i;
                    *v = f(idx % size, idx / size);
                }
            });
        }
    });
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Height in 0..1: warped fBm plus ridges, times a noisy island falloff.
pub(crate) fn height_field(seed: SimSeed, s: &WorldGenSettings, threads: usize) -> Vec<f32> {
    let hs = &s.height;
    let base = Perlin::new(&mut seed.stream(rng_key("sim_world::gen::height"), 0));
    let warp = Perlin::new(&mut seed.stream(rng_key("sim_world::gen::warp"), 0));
    let ridge = Perlin::new(&mut seed.stream(rng_key("sim_world::gen::ridge"), 0));
    let coast = Perlin::new(&mut seed.stream(rng_key("sim_world::gen::coast"), 0));
    let size = s.size as usize;
    let inv = 1.0 / s.size as f32;
    let mut out = vec![0.0f32; size * size];
    par_fill(&mut out, size, threads, |x, y| {
        let u = (x as f32 + 0.5) * inv;
        let v = (y as f32 + 0.5) * inv;
        let wx = warp.fbm(u * hs.warp_frequency, v * hs.warp_frequency, 3, 2.0, 0.5);
        let wy = warp.fbm(
            u * hs.warp_frequency + 5.2,
            v * hs.warp_frequency + 1.3,
            3,
            2.0,
            0.5,
        );
        let (uw, vw) = (u + wx * hs.warp_strength, v + wy * hs.warp_strength);
        let b = base.fbm(
            uw * hs.frequency,
            vw * hs.frequency,
            hs.octaves,
            hs.lacunarity,
            hs.gain,
        ) * 0.5
            + 0.5;
        let r = ridge.ridged(
            uw * hs.ridge_frequency,
            vw * hs.ridge_frequency,
            hs.ridge_octaves,
            hs.lacunarity,
            hs.gain,
        );
        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt()
            + coast.fbm(u * hs.coast_frequency, v * hs.coast_frequency, 4, 2.0, 0.5)
                * hs.coast_wobble;
        let falloff = 1.0 - smoothstep(hs.island_inner, hs.island_outer, d);
        ((b * (1.0 - hs.ridge_weight) + r * hs.ridge_weight) * falloff).clamp(0.0, 1.0)
    });
    out
}

/// Moisture in 0..1.
pub(crate) fn moisture_field(seed: SimSeed, s: &WorldGenSettings, threads: usize) -> Vec<f32> {
    let m = &s.moisture;
    let noise = Perlin::new(&mut seed.stream(rng_key("sim_world::gen::moisture"), 0));
    let size = s.size as usize;
    let inv = 1.0 / s.size as f32;
    let mut out = vec![0.0f32; size * size];
    par_fill(&mut out, size, threads, |x, y| {
        let (u, v) = ((x as f32 + 0.5) * inv, (y as f32 + 0.5) * inv);
        (noise.fbm(
            u * m.frequency,
            v * m.frequency,
            m.octaves,
            m.lacunarity,
            m.gain,
        ) * 0.5
            + 0.5)
            .clamp(0.0, 1.0)
    });
    out
}

/// The value below which fraction `q` of the selected values fall, from a
/// fine histogram (deterministic, no float sort). Values must be in 0..1.
pub(crate) fn quantile(values: &[f32], select: impl Fn(usize) -> bool, q: f32) -> f32 {
    const BINS: usize = 1 << 16;
    let mut hist = vec![0u32; BINS];
    let mut total = 0u64;
    for (i, &v) in values.iter().enumerate() {
        if select(i) {
            hist[((v.clamp(0.0, 1.0) * (BINS - 1) as f32) as usize).min(BINS - 1)] += 1;
            total += 1;
        }
    }
    let target = (q.clamp(0.0, 1.0) as f64 * total as f64) as u64;
    let mut acc = 0u64;
    for (b, &c) in hist.iter().enumerate() {
        acc += c as u64;
        if acc > target {
            return b as f32 / (BINS - 1) as f32;
        }
    }
    1.0
}

/// 4-neighbour breadth-first distance from every tile to the nearest
/// water tile, capped at 255.
pub(crate) fn water_distance(terrain: &[Terrain], size: usize) -> Vec<u8> {
    distance_to(terrain, size, |t| t == Terrain::Water)
}

/// 4-neighbour breadth-first distance (over any terrain) from every tile to
/// the nearest tile where `source` holds, capped at 255 (also when there is
/// no source).
pub(crate) fn distance_to(
    terrain: &[Terrain],
    size: usize,
    source: impl Fn(Terrain) -> bool,
) -> Vec<u8> {
    let mut dist = vec![u8::MAX; terrain.len()];
    let mut queue = VecDeque::new();
    for (i, t) in terrain.iter().enumerate() {
        if source(*t) {
            dist[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let d = dist[i];
        if d == u8::MAX - 1 {
            continue;
        }
        let (x, y) = (i % size, i / size);
        let mut visit = |j: usize| {
            if dist[j] == u8::MAX {
                dist[j] = d + 1;
                queue.push_back(j);
            }
        };
        if x > 0 {
            visit(i - 1);
        }
        if x + 1 < size {
            visit(i + 1);
        }
        if y > 0 {
            visit(i - size);
        }
        if y + 1 < size {
            visit(i + size);
        }
    }
    dist
}

/// Largest absolute height difference to a 4-neighbour.
pub(crate) fn slope_field(height: &[f32], size: usize, threads: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; height.len()];
    par_fill(&mut out, size, threads, |x, y| {
        let i = y * size + x;
        let h = height[i];
        let mut s = 0.0f32;
        if x > 0 {
            s = s.max((h - height[i - 1]).abs());
        }
        if x + 1 < size {
            s = s.max((h - height[i + 1]).abs());
        }
        if y > 0 {
            s = s.max((h - height[i - size]).abs());
        }
        if y + 1 < size {
            s = s.max((h - height[i + size]).abs());
        }
        s
    });
    out
}

pub(crate) struct Classified {
    pub terrain: Vec<Terrain>,
    pub sea_level: f32,
}

/// Assigns base terrain from height and moisture quantiles (SIM-01). Roads
/// and farmland are added later, around settlements.
pub(crate) fn classify(height: &[f32], moisture: &[f32], s: &WorldGenSettings) -> Classified {
    let size = s.size as usize;
    let t = &s.terrain;
    let sea_level = quantile(height, |_| true, t.water_fraction);
    let land = |i: usize| height[i] >= sea_level;
    let mountain = quantile(height, land, 1.0 - t.mountain_land_fraction);
    let hills = quantile(
        height,
        land,
        1.0 - t.mountain_land_fraction - t.hills_land_fraction,
    );
    let beach_max = quantile(height, land, t.beach_max_land_quantile);

    let mut terrain: Vec<Terrain> = height
        .iter()
        .map(|&h| {
            if h < sea_level {
                Terrain::Water
            } else if h >= mountain {
                Terrain::Mountain
            } else if h >= hills {
                Terrain::Hills
            } else {
                Terrain::Grass
            }
        })
        .collect();

    let dist = water_distance(&terrain, size);
    for i in 0..terrain.len() {
        if terrain[i] == Terrain::Grass && dist[i] as u32 <= t.beach_width && height[i] <= beach_max
        {
            terrain[i] = Terrain::Beach;
        }
    }

    let forest_line = quantile(
        moisture,
        |i| terrain[i] == Terrain::Grass,
        1.0 - t.forest_lowland_fraction,
    );
    for i in 0..terrain.len() {
        if terrain[i] == Terrain::Grass && moisture[i] >= forest_line {
            terrain[i] = Terrain::Forest;
        }
    }
    Classified { terrain, sea_level }
}

/// Fertility 0..255 from moisture, altitude and slope.
pub(crate) fn fertility(
    height: &[f32],
    moisture: &[f32],
    slope: &[f32],
    terrain: &[Terrain],
    sea_level: f32,
) -> Vec<u8> {
    let span = (1.0 - sea_level).max(1e-6);
    (0..height.len())
        .map(|i| {
            let alt = ((height[i] - sea_level) / span).clamp(0.0, 1.0);
            let base = 0.25 + 0.65 * moisture[i] - 0.45 * alt - slope[i] * 60.0;
            let mult = match terrain[i] {
                Terrain::Water | Terrain::Mountain => 0.0,
                Terrain::Beach => 0.3,
                Terrain::Hills => 0.5,
                Terrain::Forest => 0.9,
                Terrain::Grass | Terrain::Farmland | Terrain::Road => 1.0,
            };
            ((base * mult).clamp(0.0, 1.0) * 255.0) as u8
        })
        .collect()
}

/// Labels 8-connected land components; returns labels (u32::MAX for water)
/// and the label of the largest component.
pub(crate) fn landmasses(terrain: &[Terrain], size: usize) -> (Vec<u32>, u32) {
    let mut label = vec![u32::MAX; terrain.len()];
    let mut sizes: Vec<u64> = Vec::new();
    let mut stack = Vec::new();
    for start in 0..terrain.len() {
        if !terrain[start].is_land() || label[start] != u32::MAX {
            continue;
        }
        let id = sizes.len() as u32;
        let mut count = 0u64;
        label[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            count += 1;
            let (x, y) = ((i % size) as i64, (i / size) as i64);
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx, dy) == (0, 0)
                        || nx < 0
                        || ny < 0
                        || nx >= size as i64
                        || ny >= size as i64
                    {
                        continue;
                    }
                    let j = ny as usize * size + nx as usize;
                    if terrain[j].is_land() && label[j] == u32::MAX {
                        label[j] = id;
                        stack.push(j);
                    }
                }
            }
        }
        sizes.push(count);
    }
    // Largest; ties go to the lowest label.
    let largest = sizes
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0)))
        .map_or(u32::MAX, |(i, _)| i as u32);
    (label, largest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn par_fill_is_independent_of_thread_count() {
        let f = |x: usize, y: usize| (x * 31 + y * 7) as u32;
        let mut a = vec![0u32; 100 * 100];
        let mut b = vec![0u32; 100 * 100];
        par_fill(&mut a, 100, 1, f);
        par_fill(&mut b, 100, 7, f);
        assert_eq!(a, b);
        assert_eq!(a[3 * 100 + 2], 2 * 31 + 3 * 7);
    }

    #[test]
    fn quantile_matches_uniform_values() {
        let v: Vec<f32> = (0..10_000).map(|i| i as f32 / 10_000.0).collect();
        assert!((quantile(&v, |_| true, 0.25) - 0.25).abs() < 0.001);
        assert!((quantile(&v, |i| i % 2 == 0, 0.5) - 0.5).abs() < 0.001);
    }

    #[test]
    fn water_distance_counts_steps() {
        let mut t = vec![Terrain::Grass; 25];
        t[0] = Terrain::Water;
        let d = water_distance(&t, 5);
        assert_eq!(d[0], 0);
        assert_eq!(d[1], 1);
        assert_eq!(d[24], 8);
    }

    #[test]
    fn landmasses_finds_largest() {
        // Two islands in a 5x5: a 1-tile and a 4-tile one.
        let mut t = vec![Terrain::Water; 25];
        t[0] = Terrain::Grass;
        for i in [12, 13, 17, 18] {
            t[i] = Terrain::Grass;
        }
        let (label, largest) = landmasses(&t, 5);
        assert_eq!(label[12], largest);
        assert_ne!(label[0], largest);
        assert_eq!(label[1], u32::MAX);
    }
}
