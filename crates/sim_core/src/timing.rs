//! Wall-clock cost of each [`SimSet`] for the debug panel (UI-07).
//!
//! Diagnostics only: never read by simulation logic and not part of the state
//! hash, so using `Instant` here does not affect determinism.

use std::time::{Duration, Instant};

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;

use crate::schedule::SimSet;

#[derive(Resource, Debug, Default, Clone)]
pub struct SimTimings {
    /// Duration of each set in the last tick, indexed like [`SimSet::ALL`].
    pub per_set: [Duration; SimSet::ALL.len()],
    /// Total duration of the last tick.
    pub total: Duration,
    /// Exponential moving average of `total`, for a readable display.
    pub total_avg: Duration,
    last_mark: Option<Instant>,
    tick_start: Option<Instant>,
}

/// Marker `I` runs between set `I-1` and set `I`. Marker 0 starts the tick;
/// marker `N` (after the last set) ends it.
fn mark<const I: usize>(mut t: ResMut<SimTimings>) {
    let now = Instant::now();
    if I == 0 {
        t.tick_start = Some(now);
    } else if let Some(last) = t.last_mark {
        t.per_set[I - 1] = now - last;
    }
    if I == SimSet::ALL.len() {
        if let Some(start) = t.tick_start {
            t.total = now - start;
            t.total_avg = if t.total_avg.is_zero() {
                t.total
            } else {
                t.total_avg.mul_f64(0.9) + t.total.mul_f64(0.1)
            };
        }
    }
    t.last_mark = Some(now);
}

pub(crate) fn add_markers(app: &mut App) {
    use SimSet::*;
    app.add_systems(
        FixedUpdate,
        (
            mark::<0>.before(Input),
            mark::<1>.after(Input).before(Knowledge),
            mark::<2>.after(Knowledge).before(Decide),
            mark::<3>.after(Decide).before(Move),
            mark::<4>.after(Move).before(Act),
            mark::<5>.after(Act).before(Needs),
            mark::<6>.after(Needs).before(Aggregate),
            mark::<7>.after(Aggregate).before(Lod),
            mark::<8>.after(Lod).before(Bookkeeping),
            mark::<9>.after(Bookkeeping),
        ),
    );
}
