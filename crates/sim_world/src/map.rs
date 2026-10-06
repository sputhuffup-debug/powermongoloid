//! The tile map: flat row-major arrays, chunked for rendering and hashing.

use std::collections::BTreeSet;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::hash::fnv1a_64;
use sim_core::{StateHash, StateHasher};

/// Terrain types (SIM-01). Stored as one byte per tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum Terrain {
    #[default]
    Water = 0,
    Beach = 1,
    Grass = 2,
    Farmland = 3,
    Forest = 4,
    Hills = 5,
    Mountain = 6,
    Road = 7,
}

impl Terrain {
    pub const ALL: [Terrain; 8] = [
        Terrain::Water,
        Terrain::Beach,
        Terrain::Grass,
        Terrain::Farmland,
        Terrain::Forest,
        Terrain::Hills,
        Terrain::Mountain,
        Terrain::Road,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Terrain::Water => "water",
            Terrain::Beach => "beach",
            Terrain::Grass => "grass",
            Terrain::Farmland => "farmland",
            Terrain::Forest => "forest",
            Terrain::Hills => "hills",
            Terrain::Mountain => "mountain",
            Terrain::Road => "road",
        }
    }

    pub fn is_land(self) -> bool {
        self != Terrain::Water
    }
}

/// Region index per tile; [`NO_REGION`] for water and unreachable land.
pub type RegionId = u8;
pub const NO_REGION: RegionId = u8::MAX;

/// Terrain stored as flat arrays (SEED.md "Core data model"), with
/// `chunk_size` x `chunk_size` chunks.
///
/// Fields are private: tiles change only through [`WorldMap::set_terrain`],
/// which bumps the chunk's revision (so the renderer rebuilds only changed
/// chunks, RND-01) and marks its hash digest dirty.
#[derive(Resource, Debug, Clone)]
pub struct WorldMap {
    size: u32,
    chunk_size: u32,
    /// Normalised height 0..1. Tiles below `sea_level` are water.
    height: Vec<f32>,
    terrain: Vec<Terrain>,
    /// 0..255.
    fertility: Vec<u8>,
    region: Vec<RegionId>,
    sea_level: f32,
    chunk_revision: Vec<u32>,
    chunk_digest: Vec<u64>,
    dirty: BTreeSet<u32>,
}

impl WorldMap {
    pub(crate) fn from_parts(
        size: u32,
        chunk_size: u32,
        height: Vec<f32>,
        terrain: Vec<Terrain>,
        fertility: Vec<u8>,
        region: Vec<RegionId>,
        sea_level: f32,
    ) -> Self {
        let n = (size * size) as usize;
        assert!(
            height.len() == n && terrain.len() == n && fertility.len() == n && region.len() == n
        );
        assert!(size.is_multiple_of(chunk_size));
        let chunks = (size / chunk_size).pow(2) as usize;
        let mut map = Self {
            size,
            chunk_size,
            height,
            terrain,
            fertility,
            region,
            sea_level,
            chunk_revision: vec![0; chunks],
            chunk_digest: vec![0; chunks],
            dirty: BTreeSet::new(),
        };
        for c in 0..chunks {
            map.chunk_digest[c] = map.digest_chunk(c as u32);
        }
        map
    }

    pub fn size(&self) -> u32 {
        self.size
    }
    pub fn chunk_size(&self) -> u32 {
        self.chunk_size
    }
    /// Chunks per side.
    pub fn chunks_per_side(&self) -> u32 {
        self.size / self.chunk_size
    }
    pub fn sea_level(&self) -> f32 {
        self.sea_level
    }

    pub fn in_bounds(&self, p: IVec2) -> bool {
        p.x >= 0 && p.y >= 0 && (p.x as u32) < self.size && (p.y as u32) < self.size
    }

    #[inline]
    pub fn index(&self, p: IVec2) -> usize {
        debug_assert!(self.in_bounds(p));
        p.y as usize * self.size as usize + p.x as usize
    }

    #[inline]
    pub fn pos(&self, index: usize) -> IVec2 {
        IVec2::new(
            (index % self.size as usize) as i32,
            (index / self.size as usize) as i32,
        )
    }

    pub fn terrain(&self, p: IVec2) -> Terrain {
        self.terrain[self.index(p)]
    }
    pub fn height(&self, p: IVec2) -> f32 {
        self.height[self.index(p)]
    }
    pub fn fertility(&self, p: IVec2) -> u8 {
        self.fertility[self.index(p)]
    }
    pub fn region(&self, p: IVec2) -> RegionId {
        self.region[self.index(p)]
    }

    /// Height clamped to sea level: what the surface looks like.
    pub fn surface_height(&self, p: IVec2) -> f32 {
        self.height(p).max(self.sea_level)
    }

    pub fn terrain_slice(&self) -> &[Terrain] {
        &self.terrain
    }
    pub fn height_slice(&self) -> &[f32] {
        &self.height
    }

    /// The chunk containing tile `p`.
    pub fn chunk_of(&self, p: IVec2) -> u32 {
        let cps = self.chunks_per_side();
        (p.y as u32 / self.chunk_size) * cps + p.x as u32 / self.chunk_size
    }

    /// The tile coordinate of a chunk's lower corner.
    pub fn chunk_origin(&self, chunk: u32) -> IVec2 {
        let cps = self.chunks_per_side();
        IVec2::new(
            ((chunk % cps) * self.chunk_size) as i32,
            ((chunk / cps) * self.chunk_size) as i32,
        )
    }

    /// Sum of all chunk revisions: changes whenever any tile changes.
    pub fn total_revision(&self) -> u64 {
        self.chunk_revision.iter().map(|r| *r as u64).sum()
    }

    /// Incremented whenever any tile in the chunk changes.
    pub fn chunk_revision(&self, chunk: u32) -> u32 {
        self.chunk_revision[chunk as usize]
    }

    /// Changes one tile's terrain. No-op if unchanged.
    pub fn set_terrain(&mut self, p: IVec2, t: Terrain) {
        let i = self.index(p);
        if self.terrain[i] == t {
            return;
        }
        self.terrain[i] = t;
        let c = self.chunk_of(p);
        self.chunk_revision[c as usize] = self.chunk_revision[c as usize].wrapping_add(1);
        self.dirty.insert(c);
    }

    /// Recomputes hash digests of chunks changed since the last refresh.
    pub fn refresh_digests(&mut self) {
        let dirty = std::mem::take(&mut self.dirty);
        for c in dirty {
            self.chunk_digest[c as usize] = self.digest_chunk(c);
        }
    }

    fn digest_chunk(&self, chunk: u32) -> u64 {
        let origin = self.chunk_origin(chunk);
        let cs = self.chunk_size as usize;
        let mut bytes = Vec::with_capacity(cs * cs * 7);
        for y in 0..cs {
            let row = self.index(origin + IVec2::new(0, y as i32));
            for i in row..row + cs {
                bytes.extend_from_slice(&self.height[i].to_bits().to_le_bytes());
                bytes.push(self.terrain[i] as u8);
                bytes.push(self.fertility[i]);
                bytes.push(self.region[i]);
            }
        }
        fnv1a_64(&bytes)
    }

    /// A digest of the whole map (the same value the state hash folds in).
    pub fn digest(&self) -> u64 {
        let ids = Default::default();
        let mut h = StateHasher::new(&ids);
        self.hash_state(&mut h);
        h.finish()
    }

    /// Number of tiles of each terrain type, indexed by `Terrain as usize`.
    pub fn terrain_counts(&self) -> [u64; 8] {
        let mut counts = [0u64; 8];
        for &t in &self.terrain {
            counts[t as usize] += 1;
        }
        counts
    }
}

impl StateHash for WorldMap {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.size);
        h.u32(self.chunk_size);
        h.f32(self.sea_level);
        for c in 0..self.chunk_digest.len() as u32 {
            // Dirty chunks are digested fresh so the hash is always current.
            let d = if self.dirty.contains(&c) {
                self.digest_chunk(c)
            } else {
                self.chunk_digest[c as usize]
            };
            h.u64(d);
        }
    }
}

/// Bookkeeping: refresh digests of chunks changed this tick.
pub(crate) fn refresh_chunk_digests(map: Option<ResMut<WorldMap>>) {
    let _span = tracing::info_span!("sim_world::refresh_chunk_digests").entered();
    if let Some(mut map) = map
        && !map.dirty.is_empty()
    {
        map.refresh_digests();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(size: u32, chunk: u32) -> WorldMap {
        let n = (size * size) as usize;
        WorldMap::from_parts(
            size,
            chunk,
            vec![0.5; n],
            vec![Terrain::Grass; n],
            vec![100; n],
            vec![0; n],
            0.3,
        )
    }

    fn hash(map: &WorldMap) -> u64 {
        let ids = Default::default();
        let mut h = StateHasher::new(&ids);
        map.hash_state(&mut h);
        h.finish()
    }

    #[test]
    fn set_terrain_bumps_only_its_chunk() {
        let mut map = flat(128, 64);
        assert_eq!(map.chunks_per_side(), 2);
        let p = IVec2::new(70, 10); // chunk 1
        map.set_terrain(p, Terrain::Road);
        assert_eq!(map.chunk_of(p), 1);
        let revs: Vec<u32> = (0..4).map(|c| map.chunk_revision(c)).collect();
        assert_eq!(revs, [0, 1, 0, 0]);
        // Setting the same value again is not a change.
        map.set_terrain(p, Terrain::Road);
        assert_eq!(map.chunk_revision(1), 1);
    }

    #[test]
    fn hash_tracks_changes_before_and_after_refresh() {
        let mut map = flat(128, 64);
        let before = hash(&map);
        map.set_terrain(IVec2::new(1, 1), Terrain::Forest);
        let dirty = hash(&map);
        assert_ne!(before, dirty);
        map.refresh_digests();
        assert_eq!(dirty, hash(&map), "refresh must not change the hash");
        map.set_terrain(IVec2::new(1, 1), Terrain::Grass);
        map.refresh_digests();
        assert_eq!(before, hash(&map));
    }

    #[test]
    fn index_and_pos_round_trip() {
        let map = flat(128, 64);
        for p in [IVec2::new(0, 0), IVec2::new(127, 0), IVec2::new(5, 99)] {
            assert_eq!(map.pos(map.index(p)), p);
        }
        assert_eq!(map.chunk_origin(3), IVec2::new(64, 64));
    }
}
