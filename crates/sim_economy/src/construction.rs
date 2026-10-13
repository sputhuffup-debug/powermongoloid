//! Building orders (SIM-63, M4a; LORDSHIP_DESIGN section 8.3, ADR 0020).
//!
//! A building a run settlement was ordered to build stands as a real
//! `Building` with a [`Construction`] on it: carters bring what it
//! [`needs`](Construction::needs) into its own stockpile, and once that
//! covers the needs the hours count down hourly (sim_military's
//! `govern::step`). At zero the needs are consumed and the component is
//! removed. M4a step 1 lands the type (hashed and saved); step 8 adds
//! `choose_site`, `start_building` and `advance`.

use bevy_ecs::prelude::*;
use sim_core::{StateHash, StateHasher};

use crate::data::GoodId;

/// A building site (SIM-63, M4a): what it still needs delivered and the
/// game hours of work left once it has it. Hashed and saved.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct Construction {
    /// Goods the site needs in its stockpile before work counts, in good
    /// order.
    pub needs: Vec<(GoodId, u32)>,
    /// Game hours of work left.
    pub hours_left: f32,
}

impl StateHash for Construction {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.needs.len() as u32);
        for (g, q) in &self.needs {
            h.u8(g.0);
            h.u32(*q);
        }
        h.f32(self.hours_left);
    }
}

sim_core::save_fields!(Construction { needs, hours_left });
