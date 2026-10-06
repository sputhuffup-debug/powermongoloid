//! In-house 2D gradient noise (ADR 0005). Permutation tables are shuffled by
//! a `SimSeed` stream, so terrain obeys the single-RNG rule (SIM-02).

use rand::RngExt;
use rand_chacha::ChaCha8Rng;

/// Classic improved Perlin noise in 2D, output roughly in -1..1.
#[derive(Clone)]
pub struct Perlin {
    perm: [u8; 512],
}

const GRADIENTS: [(f32, f32); 8] = [
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (
        std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ),
    (
        -std::f32::consts::FRAC_1_SQRT_2,
        std::f32::consts::FRAC_1_SQRT_2,
    ),
    (
        std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
    ),
    (
        -std::f32::consts::FRAC_1_SQRT_2,
        -std::f32::consts::FRAC_1_SQRT_2,
    ),
];

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Perlin {
    pub fn new(rng: &mut ChaCha8Rng) -> Self {
        let mut p: [u8; 256] = std::array::from_fn(|i| i as u8);
        for i in (1..256).rev() {
            let j = rng.random_range(0..=i);
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Self { perm }
    }

    #[inline]
    fn grad(&self, ix: i32, iy: i32, dx: f32, dy: f32) -> f32 {
        let h = self.perm[(self.perm[(ix & 255) as usize] as usize + (iy & 255) as usize) & 511];
        let (gx, gy) = GRADIENTS[(h & 7) as usize];
        gx * dx + gy * dy
    }

    pub fn get(&self, x: f32, y: f32) -> f32 {
        let x0 = x.floor();
        let y0 = y.floor();
        let (ix, iy) = (x0 as i32, y0 as i32);
        let (dx, dy) = (x - x0, y - y0);
        let n00 = self.grad(ix, iy, dx, dy);
        let n10 = self.grad(ix + 1, iy, dx - 1.0, dy);
        let n01 = self.grad(ix, iy + 1, dx, dy - 1.0);
        let n11 = self.grad(ix + 1, iy + 1, dx - 1.0, dy - 1.0);
        let (u, v) = (fade(dx), fade(dy));
        // Scale so the output spans roughly -1..1.
        lerp(lerp(n00, n10, u), lerp(n01, n11, u), v) * std::f32::consts::SQRT_2
    }

    /// Fractal Brownian motion, normalised to roughly -1..1.
    pub fn fbm(&self, x: f32, y: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for o in 0..octaves {
            // Offset each octave so lattice points don't line up.
            let off = o as f32 * 17.13;
            sum += amp * self.get(x * freq + off, y * freq - off);
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        if norm > 0.0 { sum / norm } else { 0.0 }
    }

    /// Ridged multifractal in 0..1: sharp crests where the noise crosses 0.
    pub fn ridged(&self, x: f32, y: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for o in 0..octaves {
            let off = o as f32 * 31.7;
            let r = 1.0 - self.get(x * freq + off, y * freq + off).abs();
            sum += amp * r * r;
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        if norm > 0.0 { sum / norm } else { 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::SimSeed;

    fn perlin(seed: u64) -> Perlin {
        Perlin::new(&mut SimSeed(seed).stream(1, 0))
    }

    #[test]
    fn deterministic_per_seed() {
        let (a, b, c) = (perlin(1), perlin(1), perlin(2));
        let pts = [(0.3, 0.7), (12.5, -3.25), (100.1, 55.9)];
        for (x, y) in pts {
            assert_eq!(a.get(x, y).to_bits(), b.get(x, y).to_bits());
        }
        assert!(pts.iter().any(|&(x, y)| a.get(x, y) != c.get(x, y)));
    }

    #[test]
    fn zero_on_lattice_and_bounded() {
        let p = perlin(9);
        assert_eq!(p.get(3.0, 4.0), 0.0);
        let mut max = 0.0f32;
        for i in 0..20_000 {
            let (x, y) = (i as f32 * 0.137, i as f32 * 0.071);
            let v = p.fbm(x, y, 6, 2.0, 0.5);
            assert!(v.is_finite());
            max = max.max(v.abs());
            let r = p.ridged(x, y, 5, 2.0, 0.5);
            assert!((0.0..=1.0).contains(&r));
        }
        assert!(max <= 1.05, "fbm out of range: {max}");
        assert!(max > 0.3, "fbm suspiciously flat: {max}");
    }
}
