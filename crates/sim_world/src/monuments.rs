//! Monuments (GP-23): landmarks placed by world generation in the `run`
//! preset. This module holds the types; placement lives in
//! `worldgen/monuments.rs`, and holding, guards and bonuses in
//! `sim_military::monuments` (M4 step 4).

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use serde::{Deserialize, Serialize};
use sim_core::{NameId, StateHash, StateHasher};

use crate::map::RegionId;

/// The kinds of monument (GP-23). Append only: saves and the state hash
/// store the variant index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MonumentKind {
    /// Standing stones: morale.
    StandingStones = 0,
    /// A ruined fort: defence.
    RuinedFort = 1,
    /// A holy well: crop and fish yield.
    HolyWell = 2,
    /// A road marker: march speed.
    RoadMarker = 3,
    /// An ancient forge: smithing speed.
    AncientForge = 4,
    /// A market cross: trade value.
    MarketCross = 5,
    /// A wayshrine: loyalty drift (piety in M4b).
    Wayshrine = 6,
    /// A watchtower: threat-warning radius (sight in M5).
    Watchtower = 7,
}

impl MonumentKind {
    /// Number of kinds.
    pub const COUNT: usize = 8;

    /// Every kind, in id order.
    pub const ALL: [MonumentKind; Self::COUNT] = [
        MonumentKind::StandingStones,
        MonumentKind::RuinedFort,
        MonumentKind::HolyWell,
        MonumentKind::RoadMarker,
        MonumentKind::AncientForge,
        MonumentKind::MarketCross,
        MonumentKind::Wayshrine,
        MonumentKind::Watchtower,
    ];

    /// Index into per-kind arrays (the variant index).
    pub fn index(self) -> usize {
        self as usize
    }

    /// Lower-case display name ("standing stones"), for summaries and logs.
    pub fn name(self) -> &'static str {
        match self {
            MonumentKind::StandingStones => "standing stones",
            MonumentKind::RuinedFort => "ruined fort",
            MonumentKind::HolyWell => "holy well",
            MonumentKind::RoadMarker => "road marker",
            MonumentKind::AncientForge => "ancient forge",
            MonumentKind::MarketCross => "market cross",
            MonumentKind::Wayshrine => "wayshrine",
            MonumentKind::Watchtower => "watchtower",
        }
    }

    /// Title-case noun used in a monument's name ("Kestmoor Stones").
    pub fn title(self) -> &'static str {
        match self {
            MonumentKind::StandingStones => "Stones",
            MonumentKind::RuinedFort => "Fort",
            MonumentKind::HolyWell => "Well",
            MonumentKind::RoadMarker => "Marker",
            MonumentKind::AncientForge => "Forge",
            MonumentKind::MarketCross => "Cross",
            MonumentKind::Wayshrine => "Shrine",
            MonumentKind::Watchtower => "Watchtower",
        }
    }
}

/// A monument entity (GP-23), spawned by [`install_world`](crate::install_world)
/// after the settlements, with a `SpawnId` and a `Position`. Monuments never
/// move, so they have no `Mobile`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Monument {
    /// What it is, which decides its bonus.
    pub kind: MonumentKind,
    /// The region it stands in.
    pub region: RegionId,
    /// Its name in the `NameTable`.
    pub name: NameId,
}

impl StateHash for Monument {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.kind as u8);
        h.u8(self.region);
        h.u32(self.name.0);
    }
}

/// A monument chosen by world generation, before it is installed.
#[derive(Debug, Clone, PartialEq)]
pub struct MonumentSite {
    /// What it is.
    pub kind: MonumentKind,
    /// The tile it stands on.
    pub center: IVec2,
    /// The region of that tile.
    pub region: RegionId,
    /// Its generated name, e.g. "Kestmoor Stones".
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_is_listed_in_order() {
        for (i, k) in MonumentKind::ALL.iter().enumerate() {
            assert_eq!(k.index(), i);
            assert!(!k.name().is_empty() && !k.title().is_empty());
        }
    }
}
