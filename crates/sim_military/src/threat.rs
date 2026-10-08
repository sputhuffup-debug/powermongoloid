//! How frightening nearby armies are to each settlement (SIM-13). Villagers
//! read their settlement's `Threat` through the safety need. A settlement's
//! own owner's armies don't frighten it (GP-05), unless it has risen
//! against them (GP-06).

use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use sim_agents::{Owner, Threat};
use sim_core::SimTick;
use sim_world::Settlement;

use crate::data::Army;
use crate::index::ArmyIndex;

pub(crate) fn update_threats(
    tick: Res<SimTick>,
    army: Res<Army>,
    index: Res<ArmyIndex>,
    mut q: Query<(&Settlement, Option<&Owner>, &mut Threat)>,
) {
    let _span = tracing::info_span!("sim_military::update_threats").entered();
    let radius = army.data.threat.radius;
    for (s, owner, mut threat) in &mut q {
        let mut t = *threat;
        if t.provoked_by.is_some() && tick.0 > t.provoked_until {
            t.provoked_by = None;
        }
        let center = s.center.as_vec2() + Vec2::splat(0.5);
        let mut level = 0.0f32;
        for a in &index.armies {
            // A garrison guards its own settlement; it frightens it only
            // when the settlement rises against it.
            if owner.is_some_and(|o| o.faction == a.faction) && t.provoked_by != Some(a.faction) {
                continue;
            }
            let d = a.pos.distance(center);
            if d >= radius {
                continue;
            }
            // Full strength within 40% of the radius, fading to the edge.
            let near = ((radius - d) / (radius * 0.6)).clamp(0.0, 1.0);
            let mut l = army.threat_of(a.posture) * near;
            if t.provoked_by == Some(a.faction) {
                l = l.max(near.max(0.5));
            }
            if l > level {
                level = l;
                t.from = a.pos.floor().as_ivec2();
            }
        }
        t.level = level;
        if t != *threat {
            *threat = t;
        }
    }
}
