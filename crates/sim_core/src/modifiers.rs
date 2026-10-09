//! Per-faction stat modifiers (GP-23, GP-24, SIM-34): the summed bonuses from
//! held monuments, boons, the champion's traits and run modifiers.
//!
//! [`FactionModifiers`] lives in sim_core so that every sim crate can read it
//! (villagers' yields and loyalty in sim_agents, combat and marching in
//! sim_military). Only sim_military writes it, rebuilding it once per game
//! hour. Consumers read [`FactionModifiers::mult`] or
//! [`FactionModifiers::value`] at the point of use and never fold a modifier
//! into shared data. With no modifiers every multiplier is exactly 1.0, so
//! `x * mult` changes nothing (M4_PLAN default 6).

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::FactionId;
use crate::hash::{StateHash, StateHasher};

/// A stat a modifier can change. Append only (data files and saves name it).
/// Most are fractions added to 1.0 (+0.01 is +1%); `ThreatRadius` and
/// `CaptainSlots` are flat amounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Stat {
    /// Soldiers' morale target.
    Morale,
    /// Damage taken is divided by this.
    Defence,
    /// Damage dealt.
    Attack,
    /// Crop and fish yield in owned settlements.
    FoodYield,
    /// Army march speed.
    MarchSpeed,
    /// Smithing speed.
    SmithSpeed,
    /// The value of goods traded.
    TradeValue,
    /// Villagers' loyalty drift rate.
    LoyaltyDrift,
    /// Flat tiles added to owned settlements' threat-warning radius.
    ThreatRadius,
    /// Flat captain slots.
    CaptainSlots,
    /// How fast soldiers become veterans: battles count this much more
    /// toward a veteran rank (SIM-36).
    VeteranRate,
    /// Soldiers' chance to desert (scaled by the multiplier).
    Desertion,
    /// Wild foraging yield.
    ForageYield,
    /// Recruiting success: each villager's chance to join a `Recruit`
    /// (scaled by the multiplier).
    RecruitChance,
}

impl Stat {
    /// Number of stats.
    pub const COUNT: usize = 14;

    /// Every stat, in discriminant order.
    pub const ALL: [Stat; Stat::COUNT] = [
        Stat::Morale,
        Stat::Defence,
        Stat::Attack,
        Stat::FoodYield,
        Stat::MarchSpeed,
        Stat::SmithSpeed,
        Stat::TradeValue,
        Stat::LoyaltyDrift,
        Stat::ThreatRadius,
        Stat::CaptainSlots,
        Stat::VeteranRate,
        Stat::Desertion,
        Stat::ForageYield,
        Stat::RecruitChance,
    ];

    /// Position in a row of [`FactionModifiers`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Stat::Morale => "morale",
            Stat::Defence => "defence",
            Stat::Attack => "attack",
            Stat::FoodYield => "food yield",
            Stat::MarchSpeed => "march speed",
            Stat::SmithSpeed => "smithing speed",
            Stat::TradeValue => "trade value",
            Stat::LoyaltyDrift => "loyalty drift",
            Stat::ThreatRadius => "threat radius",
            Stat::CaptainSlots => "captain slots",
            Stat::VeteranRate => "veteran rate",
            Stat::Desertion => "desertion",
            Stat::ForageYield => "forage yield",
            Stat::RecruitChance => "recruit chance",
        }
    }
}

/// Where a modifier came from, for the UI's breakdown. Append only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModifierSource {
    /// Every held monument of one kind (the capped sum). `kind` is the
    /// `MonumentKind` discriminant (sim_world).
    Monuments { kind: u8 },
    /// Holding every monument of `kind` in `region`.
    SetBonus { kind: u8, region: u8 },
    /// Boon `card` (progression.ron).
    Boon { card: u16 },
    /// Trait `id` (progression.ron).
    Trait { id: u16 },
    /// Run modifier `card` (progression.ron).
    RunModifier { card: u16 },
}

impl StateHash for ModifierSource {
    fn hash_state(&self, h: &mut StateHasher) {
        match *self {
            ModifierSource::Monuments { kind } => {
                h.u8(0);
                h.u8(kind);
            }
            ModifierSource::SetBonus { kind, region } => {
                h.u8(1);
                h.u8(kind);
                h.u8(region);
            }
            ModifierSource::Boon { card } => {
                h.u8(2);
                h.u16(card);
            }
            ModifierSource::Trait { id } => {
                h.u8(3);
                h.u16(id);
            }
            ModifierSource::RunModifier { card } => {
                h.u8(4);
                h.u16(card);
            }
        }
    }
}

/// One contribution to [`FactionModifiers`], kept for the UI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModifierEntry {
    /// The faction it applies to; `None` for the world row (everyone).
    pub faction: Option<FactionId>,
    /// The stat changed.
    pub stat: Stat,
    /// Where it came from.
    pub source: ModifierSource,
    /// The amount added.
    pub value: f32,
}

/// Summed stat modifiers per faction, plus a world row that applies to every
/// faction (run modifiers such as a lean year). Hashed. Rebuilt hourly by
/// sim_military; everything else only reads it.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct FactionModifiers {
    /// Indexed by `FactionId`, then by [`Stat::index`]. Missing rows are zero.
    pub by_faction: Vec<[f32; Stat::COUNT]>,
    /// Added to every faction's row.
    pub world: [f32; Stat::COUNT],
    /// Every contribution, in the order added.
    pub breakdown: Vec<ModifierEntry>,
}

impl FactionModifiers {
    /// The summed modifier to `stat` for `faction` (0.0 if none).
    pub fn value(&self, faction: FactionId, stat: Stat) -> f32 {
        let own = self
            .by_faction
            .get(faction.0 as usize)
            .map_or(0.0, |row| row[stat.index()]);
        own + self.world[stat.index()]
    }

    /// `1.0 + value`: the multiplier for a fractional stat. Exactly 1.0 with
    /// no modifiers.
    pub fn mult(&self, faction: FactionId, stat: Stat) -> f32 {
        1.0 + self.value(faction, stat)
    }

    /// Removes every modifier (the start of a rebuild).
    pub fn clear(&mut self) {
        self.by_faction.clear();
        self.world = [0.0; Stat::COUNT];
        self.breakdown.clear();
    }

    /// Adds `value` to `stat` for `faction`.
    pub fn add(&mut self, faction: FactionId, stat: Stat, source: ModifierSource, value: f32) {
        let i = faction.0 as usize;
        if self.by_faction.len() <= i {
            self.by_faction.resize(i + 1, [0.0; Stat::COUNT]);
        }
        self.by_faction[i][stat.index()] += value;
        self.breakdown.push(ModifierEntry {
            faction: Some(faction),
            stat,
            source,
            value,
        });
    }

    /// Adds `value` to `stat` for every faction (the world row).
    pub fn add_world(&mut self, stat: Stat, source: ModifierSource, value: f32) {
        self.world[stat.index()] += value;
        self.breakdown.push(ModifierEntry {
            faction: None,
            stat,
            source,
            value,
        });
    }
}

impl StateHash for FactionModifiers {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.by_faction.len() as u32);
        for row in &self.by_faction {
            for &v in row {
                h.f32(v);
            }
        }
        for &v in &self.world {
            h.f32(v);
        }
        h.u32(self.breakdown.len() as u32);
        for e in &self.breakdown {
            match e.faction {
                Some(f) => {
                    h.u8(1);
                    h.u16(f.0);
                }
                None => h.u8(0),
            }
            h.u8(e.stat as u8);
            e.source.hash_state(h);
            h.f32(e.value);
        }
    }
}

crate::save_serde!(Stat, ModifierSource);
crate::save_fields!(ModifierEntry {
    faction,
    stat,
    source,
    value
});
crate::save_fields!(FactionModifiers {
    by_faction,
    world,
    breakdown
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_modifiers_is_an_exact_no_op() {
        let m = FactionModifiers::default();
        for s in Stat::ALL {
            assert_eq!(m.mult(FactionId::PLAYER, s), 1.0);
            assert_eq!(m.value(FactionId(40), s), 0.0);
        }
        assert_eq!(37.5 * m.mult(FactionId::RIVAL, Stat::Morale), 37.5);
    }

    #[test]
    fn faction_and_world_rows_add_up() {
        let mut m = FactionModifiers::default();
        m.add(
            FactionId::PLAYER,
            Stat::Morale,
            ModifierSource::Monuments { kind: 0 },
            0.03,
        );
        m.add_world(
            Stat::FoodYield,
            ModifierSource::RunModifier { card: 0 },
            -0.15,
        );
        assert_eq!(m.value(FactionId::PLAYER, Stat::Morale), 0.03);
        assert_eq!(m.value(FactionId::RAIDERS, Stat::Morale), 0.0);
        assert!((m.mult(FactionId::RAIDERS, Stat::FoodYield) - 0.85).abs() < 1e-6);
        assert_eq!(m.breakdown.len(), 2);
        m.clear();
        assert_eq!(m, FactionModifiers::default());
    }

    #[test]
    fn every_stat_is_listed_in_order() {
        for (i, s) in Stat::ALL.iter().enumerate() {
            assert_eq!(s.index(), i, "{}", s.name());
        }
    }
}
