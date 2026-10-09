//! How frightening nearby armies are to each settlement (SIM-13). Villagers
//! read their settlement's `Threat` through the safety need. A settlement's
//! own owner's armies don't frighten it (GP-05), unless it has risen
//! against them (GP-06). The radius is army.ron `threat.radius` plus the
//! owner's `ThreatRadius` modifier (watchtowers, GP-23).

use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use sim_agents::{Owner, Threat};
use sim_core::{FactionId, FactionModifiers, SimTick, Stat};
use sim_world::Settlement;

use crate::data::Army;
use crate::index::ArmyIndex;

pub(crate) fn update_threats(
    tick: Res<SimTick>,
    army: Res<Army>,
    index: Res<ArmyIndex>,
    mods: Res<FactionModifiers>,
    mut q: Query<(&Settlement, Option<&Owner>, &mut Threat)>,
) {
    let _span = tracing::info_span!("sim_military::update_threats").entered();
    for (s, owner, mut threat) in &mut q {
        // GP-23: watchtowers widen the warning radius of their holder's
        // settlements (flat tiles; nothing without modifiers).
        let owner_faction = owner.map_or(FactionId::INDEPENDENT, |o| o.faction);
        let radius = army.data.threat.radius + mods.value(owner_faction, Stat::ThreatRadius);
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

#[cfg(test)]
mod tests {
    use crate::conquest::tests::settlements;
    use crate::conquest::transfer;
    use crate::progression::tests::run_app;
    use crate::spawn::{ArmySpec, spawn_army};
    use bevy_ecs::prelude::*;
    use sim_agents::Threat;
    use sim_core::{Cause, FactionId, Posture, SimStepper, SimTick};

    fn level(app: &bevy_app::App, s: Entity) -> f32 {
        app.world().get::<Threat>(s).unwrap().level
    }

    /// GP-05, GP-06, SIM-13: a settlement's own owner's army, even an
    /// aggressive one at its centre, adds no threat, so a garrison doesn't
    /// keep its villagers hiding; once the settlement rises against its
    /// owner, that army frightens it again, and another faction's army
    /// always does.
    #[test]
    fn an_owners_army_frightens_only_a_risen_village() {
        let (mut app, _) = run_app(42);
        let (_, a, ac) = settlements(app.world_mut())[0];
        let world = app.world_mut();
        transfer(world, a, FactionId::PLAYER, Cause::None).unwrap();
        let spec = ArmySpec {
            posture: Posture::Aggressive,
            ..ArmySpec::new(FactionId::PLAYER, ac, 5)
        };
        spawn_army(world, &spec);
        app.step(1);
        assert_eq!(level(&app, a), 0.0, "the garrison frightens its village");

        let world = app.world_mut();
        let tick = world.resource::<SimTick>().0;
        let mut threat = world.get_mut::<Threat>(a).unwrap();
        threat.provoked_by = Some(FactionId::PLAYER);
        threat.provoked_until = tick + 1_000;
        app.step(1);
        assert!(level(&app, a) >= 0.5, "a risen village fears its owner");

        let world = app.world_mut();
        let mut threat = world.get_mut::<Threat>(a).unwrap();
        threat.provoked_by = None;
        threat.provoked_until = 0;
        let spec = ArmySpec {
            posture: Posture::Aggressive,
            ..ArmySpec::new(FactionId::RAIDERS, ac, 5)
        };
        spawn_army(world, &spec);
        app.step(1);
        assert!(level(&app, a) > 0.0, "raiders at the gate frighten nobody");
    }
}
