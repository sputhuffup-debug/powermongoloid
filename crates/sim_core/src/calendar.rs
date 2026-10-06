//! The game calendar (SIM-04): days and their phases, derived from
//! [`SimTick`](crate::SimTick) so there is no separate clock state to drift.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::hash::{StateHash, StateHasher};

/// `assets/data/calendar.ron`, embedded at compile time.
pub const DEFAULT_CALENDAR_RON: &str = include_str!("../../../assets/data/calendar.ron");

/// Phases of the day (SIM-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Phase {
    Morning,
    Work,
    Evening,
    Night,
}

impl Phase {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Phase::Morning => "Morning",
            Phase::Work => "Work",
            Phase::Evening => "Evening",
            Phase::Night => "Night",
        }
    }
}

/// A phase and its share of the day.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhaseSpan {
    pub phase: Phase,
    pub fraction: f32,
}

/// Day length, phases and seasons (calendar.ron). Hashed.
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calendar {
    pub ticks_per_day: u64,
    pub phases: Vec<PhaseSpan>,
    pub days_per_season: u32,
    pub seasons_per_year: u32,
}

/// What the calendar says about one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockReading {
    /// Day number, starting at 0.
    pub day: u64,
    pub tick_of_day: u64,
    /// 0..1 through the day.
    pub day_fraction: f32,
    pub phase: Phase,
    /// 0..1 through the current phase.
    pub phase_fraction: f32,
}

impl ClockReading {
    /// A 24-hour clock face for display. Each phase spans fixed hours:
    /// morning 06-08, work 08-17, evening 17-21, night 21-06.
    pub fn hours_minutes(&self) -> (u32, u32) {
        let (start, end) = match self.phase {
            Phase::Morning => (6.0, 8.0),
            Phase::Work => (8.0, 17.0),
            Phase::Evening => (17.0, 21.0),
            Phase::Night => (21.0, 30.0),
        };
        let hours = start + (end - start) * self.phase_fraction;
        let minutes = ((hours * 60.0) as u32) % (24 * 60);
        (minutes / 60, minutes % 60)
    }
}

impl Calendar {
    /// Parses and validates calendar.ron text.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        let c: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        c.validate()?;
        Ok(c)
    }

    /// The shipped calendar.
    pub fn default_data() -> Self {
        Self::from_ron(DEFAULT_CALENDAR_RON).expect("assets/data/calendar.ron is valid")
    }

    /// Checks for a positive day length and phases whose fractions are positive and sum to 1.
    pub fn validate(&self) -> Result<(), String> {
        if self.ticks_per_day == 0 || self.phases.is_empty() {
            return Err("calendar: need ticks_per_day > 0 and at least one phase".into());
        }
        let sum: f32 = self.phases.iter().map(|p| p.fraction).sum();
        if (sum - 1.0).abs() > 1e-4 || self.phases.iter().any(|p| p.fraction <= 0.0) {
            return Err(format!(
                "calendar: phase fractions must be positive and sum to 1 (got {sum})"
            ));
        }
        Ok(())
    }

    /// Ticks in a year.
    pub fn ticks_per_year(&self) -> u64 {
        self.ticks_per_day * (self.days_per_season * self.seasons_per_year) as u64
    }

    /// Days in a year.
    pub fn days_per_year(&self) -> u32 {
        self.days_per_season * self.seasons_per_year
    }

    /// Day, season, phase and time of day at `tick`.
    pub fn at(&self, tick: u64) -> ClockReading {
        let day = tick / self.ticks_per_day;
        let tick_of_day = tick % self.ticks_per_day;
        let day_fraction = tick_of_day as f32 / self.ticks_per_day as f32;
        let mut start = 0.0;
        for span in &self.phases {
            let end = start + span.fraction;
            if day_fraction < end || std::ptr::eq(span, self.phases.last().expect("non-empty")) {
                return ClockReading {
                    day,
                    tick_of_day,
                    day_fraction,
                    phase: span.phase,
                    phase_fraction: ((day_fraction - start) / span.fraction).clamp(0.0, 1.0),
                };
            }
            start = end;
        }
        unreachable!("the last phase always matches")
    }

    /// The first tick of the next occurrence of `phase` at or after `tick`.
    pub fn next_start_of(&self, phase: Phase, tick: u64) -> u64 {
        let mut start = 0.0;
        let mut offset = None;
        for span in &self.phases {
            if span.phase == phase {
                offset = Some((start * self.ticks_per_day as f32).round() as u64);
                break;
            }
            start += span.fraction;
        }
        let offset = offset.unwrap_or(0);
        let day_start = tick - tick % self.ticks_per_day;
        let candidate = day_start + offset;
        if candidate >= tick {
            candidate
        } else {
            candidate + self.ticks_per_day
        }
    }
}

impl StateHash for Calendar {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bytes(
            ron::to_string(self)
                .expect("calendar serialises")
                .as_bytes(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_day_is_four_minutes_at_ten_hz() {
        let c = Calendar::default_data();
        assert_eq!(c.ticks_per_day, 2400);
        assert_eq!(c.ticks_per_day as f64 / crate::SIM_TICK_HZ, 240.0);
    }

    #[test]
    fn phases_follow_in_order_and_wrap() {
        let c = Calendar::default_data();
        assert_eq!(c.at(0).phase, Phase::Morning);
        assert_eq!(c.at(359).phase, Phase::Morning);
        assert_eq!(c.at(360).phase, Phase::Work);
        assert_eq!(c.at(1440).phase, Phase::Evening);
        assert_eq!(c.at(1800).phase, Phase::Night);
        assert_eq!(c.at(2399).phase, Phase::Night);
        let next = c.at(2400);
        assert_eq!(
            (next.day, next.phase, next.tick_of_day),
            (1, Phase::Morning, 0)
        );
    }

    #[test]
    fn next_start_of_phase() {
        let c = Calendar::default_data();
        assert_eq!(c.next_start_of(Phase::Work, 0), 360);
        assert_eq!(c.next_start_of(Phase::Work, 360), 360);
        assert_eq!(c.next_start_of(Phase::Work, 361), 2400 + 360);
        assert_eq!(c.next_start_of(Phase::Morning, 2000), 2400);
    }

    #[test]
    fn clock_face_follows_phases() {
        let c = Calendar::default_data();
        assert_eq!(c.at(0).hours_minutes(), (6, 0));
        assert_eq!(c.at(360).hours_minutes(), (8, 0));
        assert_eq!(c.at(1440).hours_minutes(), (17, 0));
        assert_eq!(c.at(1800).hours_minutes(), (21, 0));
        assert_eq!(c.at(2399).hours_minutes().0, 5);
    }

    #[test]
    fn rejects_bad_fractions() {
        let mut c = Calendar::default_data();
        c.phases[0].fraction = 0.5;
        assert!(c.validate().is_err());
    }
}
