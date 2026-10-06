//! Procedurally generated stylized textures (ADR 0009). Generated at
//! startup from a fixed hash, so they look the same every run and need no
//! asset files (and nothing borrowed from other games). Hand-made art can
//! replace any of them later: materials only hold image handles.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Handles to the generated textures.
#[derive(Resource, Clone)]
pub struct ArtTextures {
    /// Grayscale painterly detail, multiplied over terrain vertex colours.
    pub terrain_detail: Handle<Image>,
    pub plaster: Handle<Image>,
    pub planks: Handle<Image>,
    pub shingles: Handle<Image>,
    pub stone: Handle<Image>,
    /// Neutral weave, tinted per occupation.
    pub cloth: Handle<Image>,
    /// White figure silhouette with alpha, tinted per occupation.
    pub silhouette: Handle<Image>,
}

/// Integer hash to 0..1 (no RNG: render-side, but still deterministic).
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
}

/// Tileable value noise with period `period` cells over a unit square.
fn tile_noise(u: f32, v: f32, period: i32, seed: u32) -> f32 {
    let (x, y) = (u * period as f32, v * period as f32);
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let w = |i: i32| i.rem_euclid(period);
    let a = hash(w(x0), w(y0), seed);
    let b = hash(w(x0 + 1), w(y0), seed);
    let c = hash(w(x0), w(y0 + 1), seed);
    let d = hash(w(x0 + 1), w(y0 + 1), seed);
    let top = a + (b - a) * sx;
    let bottom = c + (d - c) * sx;
    top + (bottom - top) * sy
}

fn fbm(u: f32, v: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut period = 4;
    for o in 0..5 {
        sum += amp * tile_noise(u, v, period, seed + o);
        amp *= 0.5;
        period *= 2;
    }
    sum / 0.97
}

fn make(size: u32, f: impl Fn(f32, f32) -> [f32; 4]) -> Image {
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (
                (x as f32 + 0.5) / size as f32,
                (y as f32 + 0.5) / size as f32,
            );
            for c in f(u, v) {
                data.push((c.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

fn gray(g: f32) -> [f32; 4] {
    [g, g, g, 1.0]
}

/// Painterly grayscale: soft blotches plus short directional strokes.
pub fn terrain_detail_image() -> Image {
    make(256, |u, v| {
        let blotch = fbm(u, v, 11);
        let stroke = tile_noise(u * 1.0 + v * 0.35, v * 0.2, 48, 17);
        gray(0.72 + 0.2 * blotch + 0.1 * stroke)
    })
}

fn plaster_image() -> Image {
    make(128, |u, v| {
        let n = fbm(u, v, 21);
        let timber = if (u * 4.0).fract() < 0.08 || (v * 3.0).fract() < 0.07 {
            0.55
        } else {
            1.0
        };
        let c = (0.82 + 0.16 * n) * timber;
        [c, c * 0.95, c * 0.84, 1.0]
    })
}

fn planks_image() -> Image {
    make(128, |u, v| {
        let row = (v * 8.0).floor();
        let seam = if (v * 8.0).fract() < 0.08 { 0.6 } else { 1.0 };
        let grain = tile_noise(u * 0.25, v, 64, 31 + row as u32);
        let c = (0.55 + 0.25 * grain) * seam;
        [c, c * 0.72, c * 0.48, 1.0]
    })
}

fn shingles_image() -> Image {
    make(128, |u, v| {
        let row = (v * 10.0).floor();
        let offset = if row as i32 % 2 == 0 { 0.0 } else { 0.5 / 6.0 };
        let gap = if ((u + offset) * 6.0).fract() < 0.06 || (v * 10.0).fract() < 0.1 {
            0.6
        } else {
            1.0
        };
        let n = hash(((u + offset) * 6.0) as i32, row as i32, 41);
        let c = (0.75 + 0.25 * n) * gap;
        [c * 0.62, c * 0.27, c * 0.2, 1.0]
    })
}

fn stone_image() -> Image {
    make(128, |u, v| {
        let n = fbm(u, v, 51);
        let crack = if tile_noise(u, v, 12, 53) > 0.82 {
            0.7
        } else {
            1.0
        };
        gray((0.38 + 0.22 * n) * crack)
    })
}

fn cloth_image() -> Image {
    make(64, |u, v| {
        let weave = if ((u * 32.0) as i32 + (v * 32.0) as i32) % 2 == 0 {
            0.95
        } else {
            0.85
        };
        // A darker belt across the middle of the figure.
        let belt = if (0.48..0.54).contains(&v) { 0.55 } else { 1.0 };
        gray(weave * belt)
    })
}

/// White silhouette: head and body, transparent elsewhere.
fn silhouette_image() -> Image {
    make(64, |u, v| {
        // v runs top to bottom in the image.
        let head = (u - 0.5).powi(2) + (v - 0.2).powi(2) < 0.012;
        let half_width = 0.12 + 0.1 * ((v - 0.32) / 0.66).clamp(0.0, 1.0);
        let body = (0.32..0.98).contains(&v) && (u - 0.5).abs() < half_width;
        if head || body {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        }
    })
}

pub(crate) fn generate(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(ArtTextures {
        terrain_detail: images.add(terrain_detail_image()),
        plaster: images.add(plaster_image()),
        planks: images.add(planks_image()),
        shingles: images.add(shingles_image()),
        stone: images.add(stone_image()),
        cloth: images.add(cloth_image()),
        silhouette: images.add(silhouette_image()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_tiles_seamlessly() {
        for v in [0.1, 0.5, 0.9] {
            let a = tile_noise(0.0, v, 8, 3);
            let b = tile_noise(1.0, v, 8, 3);
            assert!((a - b).abs() < 1e-5, "left and right edges must match");
        }
    }

    #[test]
    fn textures_are_deterministic() {
        let a = terrain_detail_image();
        let b = terrain_detail_image();
        assert_eq!(a.data, b.data);
    }
}
