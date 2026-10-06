//! Army data from `assets/data/army.ron`.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use sim_core::{Calendar, FactionId, Posture, StateHash, StateHasher};
use sim_economy::{Economy, GoodId};

/// `assets/data/army.ron`, embedded at compile time.
pub const DEFAULT_ARMY_RON: &str = include_str!("../../../assets/data/army.ron");

/// A faction's name and colour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactionDef {
    pub name: String,
    pub color: (f32, f32, f32),
}

/// Base fighting stats for a kind of soldier.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UnitStats {
    pub health: f32,
    pub attack: f32,
    pub defense: f32,
    pub morale: f32,
}

/// What arms add (SIM-32).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentData {
    pub weapon_attack: f32,
    pub tools_defense: f32,
}

/// Combat and morale numbers (`combat` in army.ron).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatData {
    pub engage_radius: f32,
    pub attack_range: f32,
    pub attack_cooldown: u64,
    pub roll_min: f32,
    pub roll_max: f32,
    pub rout_morale: f32,
    pub rally_morale: f32,
    pub casualty_morale: f32,
    pub casualty_morale_cap: f32,
    pub casualty_radius: f32,
    pub kill_morale: f32,
}

/// The captain's leadership aura.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeadershipData {
    pub radius: f32,
    pub morale_per_day: f32,
    pub attack_bonus: f32,
    pub captain_loss_morale: f32,
}

/// Food, morale, desertion and starvation (SIM-31).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpkeepData {
    pub hungry: f32,
    pub sated: f32,
    pub unfed_morale_per_day: f32,
    pub fed_morale_per_day: f32,
    pub desert_below: f32,
    pub desert_chance_per_day: f32,
    pub starving: f32,
    pub starve_damage_per_day: f32,
    pub forage_radius: f32,
    pub first_desertion_within_days: f32,
}

/// Loose formation and steering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormationData {
    pub spacing: f32,
    pub separation: f32,
    pub arrive_radius: f32,
    pub catch_up: f32,
}

/// How frightening an army is to settlements, by posture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreatData {
    pub radius: f32,
    pub passive: f32,
    pub neutral: f32,
    pub aggressive: f32,
}

/// A value per posture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeepDays {
    pub passive: f32,
    pub neutral: f32,
    pub aggressive: f32,
}

/// Who joins when an army recruits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecruitData {
    pub radius: f32,
    pub willing: KeepDays,
    pub hungry_village_bonus: f32,
    pub fed_army_bonus: f32,
}

/// The whole of army.ron, as written. See docs/dev/DATA.md.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmyData {
    pub factions: Vec<FactionDef>,
    pub soldier: UnitStats,
    pub captain: UnitStats,
    pub equipment: EquipmentData,
    pub combat: CombatData,
    pub leadership: LeadershipData,
    pub upkeep: UpkeepData,
    pub formation: FormationData,
    pub baggage_per_head: u32,
    pub carry: u16,
    pub max_captains: u32,
    pub recruit: RecruitData,
    pub threat: ThreatData,
    pub gather_keep_days: KeepDays,
    pub gather_target_days: f32,
    pub trade_keep_days: f32,
    pub trade_value: Vec<(String, f32)>,
}

/// Army data plus per-tick rates derived with the calendar.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Army {
    pub data: ArmyData,
    pub ticks_per_day: f32,
    /// Food value per unit, indexed by `GoodId`.
    pub trade_value: Vec<f32>,
}

impl Army {
    /// Parses army.ron text and resolves trade goods against the economy.
    pub fn from_ron(text: &str, calendar: &Calendar, econ: &Economy) -> Result<Self, String> {
        let data: ArmyData = ron::from_str(text).map_err(|e| e.to_string())?;
        if data.factions.len() < 2 {
            return Err("army: need at least the independent faction and one warlord".into());
        }
        let mut trade_value = vec![0.0; econ.good_count()];
        for (name, v) in &data.trade_value {
            let i = econ
                .data
                .goods
                .iter()
                .position(|g| &g.name == name)
                .ok_or_else(|| format!("army: unknown good {name:?} in trade_value"))?;
            trade_value[i] = *v;
        }
        Ok(Self {
            data,
            ticks_per_day: calendar.ticks_per_day as f32,
            trade_value,
        })
    }

    /// The shipped army data.
    pub fn default_data(calendar: &Calendar, econ: &Economy) -> Self {
        Self::from_ron(DEFAULT_ARMY_RON, calendar, econ).expect("assets/data/army.ron is valid")
    }

    /// Converts a per-day amount to per-tick.
    pub fn per_tick(&self, per_day: f32) -> f32 {
        per_day / self.ticks_per_day
    }

    /// A faction's display name.
    pub fn faction_name(&self, f: FactionId) -> &str {
        self.data
            .factions
            .get(f.0 as usize)
            .map_or("?", |d| d.name.as_str())
    }

    /// Warlord factions (everyone but the independents) are at war.
    pub fn at_war(&self, a: FactionId, b: FactionId) -> bool {
        a != b && a.0 != 0 && b.0 != 0
    }

    /// Threat an army of this posture poses at close range.
    pub fn threat_of(&self, posture: Posture) -> f32 {
        match posture {
            Posture::Passive => self.data.threat.passive,
            Posture::Neutral => self.data.threat.neutral,
            Posture::Aggressive => self.data.threat.aggressive,
        }
    }

    /// Days of food a village keeps when an army of this posture gathers.
    pub fn keep_days(&self, posture: Posture) -> f32 {
        match posture {
            Posture::Passive => self.data.gather_keep_days.passive,
            Posture::Neutral => self.data.gather_keep_days.neutral,
            Posture::Aggressive => self.data.gather_keep_days.aggressive,
        }
    }

    /// Chance an asked adult joins, by posture.
    pub fn willing(&self, posture: Posture) -> f32 {
        let w = &self.data.recruit.willing;
        match posture {
            Posture::Passive => w.passive,
            Posture::Neutral => w.neutral,
            Posture::Aggressive => w.aggressive,
        }
    }

    /// Baggage capacity for an army of `people` (followers and captain).
    pub fn baggage_capacity(&self, people: u32) -> u32 {
        self.data.baggage_per_head * people.max(10)
    }

    /// Loose-formation radius for an army of `n` followers (tiles).
    pub fn formation_radius(&self, n: u32) -> f32 {
        self.data.formation.spacing * (n as f32 + 1.5).sqrt()
    }

    /// Food value of one unit of `g` in barter.
    pub fn trade_value(&self, g: GoodId) -> f32 {
        self.trade_value.get(g.0 as usize).copied().unwrap_or(0.0)
    }
}

impl StateHash for Army {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(
            ron::to_string(&self.data)
                .expect("army data serialises")
                .as_bytes(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_data_loads() {
        let a = Army::default_data(&Calendar::default_data(), &Economy::default_data());
        assert!(a.at_war(FactionId(1), FactionId(2)));
        assert!(
            !a.at_war(FactionId(0), FactionId(1)),
            "independents aren't at war by default"
        );
        assert!(!a.at_war(FactionId(1), FactionId(1)));
        assert!(a.threat_of(Posture::Passive) < a.threat_of(Posture::Aggressive));
        assert!(a.keep_days(Posture::Aggressive) < a.keep_days(Posture::Passive));
        assert!(a.trade_value(GoodId(7)) > 0.0, "weapons trade for food");
    }
}
