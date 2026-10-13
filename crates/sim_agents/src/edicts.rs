//! Edicts: a run settlement's three dials and the marks they leave on its
//! villagers (GP-34, GP-17 part, M4a; LORDSHIP_DESIGN section 8.1,
//! ADR 0020).
//!
//! - [`Edicts`]: on every populated settlement of a run (inserted by
//!   sim_military's `setup_run`, every dial at its default). Set by the
//!   player's `Govern` command (validated in `orders::apply`) or a
//!   steward's habit, and applied by sim_military's hourly `govern::step`.
//!   Every default is an exact no-op.
//! - [`Rationed`], [`Mustered`], [`Assignment`]: on villagers moved,
//!   mustered or rationed by an edict.
//!
//! M4a step 1 lands the types (hashed and saved) and nothing writes them
//! yet: step 4 brings the dials to life. Per-town effects live here, read
//! at the point of use, never in `FactionModifiers` (ADR 0015).

use bevy_ecs::prelude::*;
use sim_core::{BuildKind, Dial, Dues, StateHash, StateHasher, Table, WorkFocus};
use sim_economy::Occupation;

/// One dial's bookkeeping in [`Edicts::dials`] (indexed by [`Dial`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DialState {
    /// The player set the current setting: the steward's habits leave the
    /// dial alone until it is released.
    pub by_player: bool,
    /// The tick the dial unlocks (0: unlocked). A change locks it for
    /// progression.ron `edicts.lock_days` from the hour it started.
    pub locked_until: u64,
    /// The tick the current setting started (its first wave), for the
    /// countdown badges.
    pub since: u64,
}

/// A run settlement's edicts (GP-34, M4a): the Work, Dues and Table dials,
/// changes waiting for the next hourly `govern::step`, each dial's lock,
/// the feast's clock and the building orders of the run. Hashed and saved.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct Edicts {
    /// The Work dial.
    pub work: WorkFocus,
    /// The Dues dial.
    pub dues: Dues,
    /// The Table dial.
    pub table: Table,
    /// A Work setting waiting for the next hourly step.
    pub pending_work: Option<WorkFocus>,
    /// A Dues setting waiting for the next hourly step.
    pub pending_dues: Option<Dues>,
    /// A Table setting waiting for the next hourly step.
    pub pending_table: Option<Table>,
    /// Per dial ([`Dial`] position): who set it, its lock and its start.
    pub dials: [DialState; 3],
    /// A feast runs until this tick (0: none).
    pub feast_until: u64,
    /// A feast's Table term lasts until this tick (0: none).
    pub feast_term_until: u64,
    /// No new feast before this tick.
    pub next_feast: u64,
    /// Buildings ordered this run, in order (at most two; SIM-63).
    pub builds: Vec<BuildKind>,
}

impl Edicts {
    /// The state of `dial`.
    pub fn dial(&self, dial: Dial) -> &DialState {
        &self.dials[dial as usize]
    }
}

impl StateHash for Edicts {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.work.tag());
        h.u8(self.dues.tag());
        h.u8(self.table.tag());
        h.u8(self.pending_work.map_or(u8::MAX, WorkFocus::tag));
        h.u8(self.pending_dues.map_or(u8::MAX, Dues::tag));
        h.u8(self.pending_table.map_or(u8::MAX, Table::tag));
        for d in &self.dials {
            h.bool(d.by_player);
            h.u64(d.locked_until);
            h.u64(d.since);
        }
        h.u64(self.feast_until);
        h.u64(self.feast_term_until);
        h.u64(self.next_feast);
        h.u32(self.builds.len() as u32);
        for b in &self.builds {
            h.u8(b.tag());
        }
    }
}

/// A villager living under the Ration edict (GP-34, M4a): hunger and the
/// work cycle slowed (progression.ron `edicts.ration`). Hashed and saved.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rationed;

impl StateHash for Rationed {
    fn hash_state(&self, _h: &mut StateHasher) {}
}

/// A villager mustered by the Levy edict, waiting on the green for a
/// captain to collect him (GP-34, M4a). Hashed and saved.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mustered {
    /// The tick he was mustered.
    pub since: u64,
}

impl StateHash for Mustered {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.since);
    }
}

/// A villager an edict moved to another trade (GP-34, M4a): where he came
/// from, so Balanced can move him back, and why, for the inspector ("Smith,
/// moved from farmer by the Forge edict, day 6"). Hashed and saved.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assignment {
    /// The trade he had before an edict moved him.
    pub from: Occupation,
    /// The Work setting that moved him.
    pub focus: WorkFocus,
    /// The tick he was moved.
    pub since: u64,
    /// The steward's habit set the dial, not the player.
    pub by_steward: bool,
}

impl StateHash for Assignment {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.from as u8);
        h.u8(self.focus.tag());
        h.u64(self.since);
        h.bool(self.by_steward);
    }
}

sim_core::save_fields!(DialState {
    by_player,
    locked_until,
    since
});
sim_core::save_fields!(Edicts {
    work,
    dues,
    table,
    pending_work,
    pending_dues,
    pending_table,
    dials,
    feast_until,
    feast_term_until,
    next_feast,
    builds
});
sim_core::save_unit!(Rationed);
sim_core::save_fields!(Mustered { since });
sim_core::save_fields!(Assignment {
    from,
    focus,
    since,
    by_steward
});

#[cfg(test)]
mod tests {
    use super::*;

    /// GP-34: every dial starts at its no-op default, unlocked and unset.
    #[test]
    fn edicts_start_at_their_defaults() {
        let e = Edicts::default();
        assert_eq!(
            (e.work, e.dues, e.table),
            (WorkFocus::Balanced, Dues::Light, Table::Normal)
        );
        for d in Dial::ALL {
            assert_eq!(*e.dial(d), DialState::default());
        }
        assert!(e.builds.is_empty());
    }
}
