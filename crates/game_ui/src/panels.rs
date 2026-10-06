//! Player panels: the villager inspector (UI-03) and the settlement panel
//! (UI-02). Both read simulation state and show it as text.

use std::collections::BTreeMap;

use bevy::prelude::*;
use sim_agents::{
    ActionKind, Activity, CombatStats, DecisionTrace, Employment, Enlisted, FoodSecurity, Needs,
    Stage, Stance, Threat, Villager, Villagers,
};
use sim_core::{Calendar, NameTable};
use sim_economy::{Building, Economy, Occupation, SettlementEconomy, Stockpile};
use sim_military::{Army, Captain, Equipment, Follower, Soldier};
use sim_world::Settlement;

use crate::selection::Selection;
use crate::style;

pub struct PanelsPlugin;

impl Plugin for PanelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_panels)
            .add_systems(Update, (villager_panel, settlement_panel));
    }
}

#[derive(Component)]
struct VillagerPanel;

#[derive(Component)]
struct VillagerText;

#[derive(Component)]
struct SettlementPanel;

#[derive(Component)]
struct SettlementText;

/// Both panels sit on the right, clear of the dev-only debug panel: the
/// villager inspector at the top, the settlement panel at the bottom.
fn panel(top: bool) -> (Node, BackgroundColor, Interaction, Visibility) {
    (
        Node {
            position_type: PositionType::Absolute,
            top: if top { Val::Px(56.0) } else { Val::Auto },
            bottom: if top { Val::Auto } else { Val::Px(10.0) },
            right: Val::Px(10.0),
            width: Val::Px(380.0),
            padding: UiRect::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(style::PANEL),
        Interaction::None,
        Visibility::Hidden,
    )
}

fn spawn_panels(mut commands: Commands) {
    commands
        .spawn((VillagerPanel, panel(true)))
        .with_children(|p| {
            p.spawn((
                VillagerText,
                Text::new(""),
                style::font(13.0),
                TextColor(style::TEXT),
            ));
        });
    commands
        .spawn((SettlementPanel, panel(false)))
        .with_children(|p| {
            p.spawn((
                SettlementText,
                Text::new(""),
                style::font(13.0),
                TextColor(style::TEXT),
            ));
        });
}

fn describe_action(kind: ActionKind, stage: Stage, target: Option<String>) -> String {
    let doing = kind.name();
    let at = target.map(|t| format!(" ({t})")).unwrap_or_default();
    match stage {
        Stage::Travel => format!(
            "walking to {}{at}",
            match kind {
                ActionKind::Eat => "eat",
                ActionKind::Fetch => "fetch food",
                ActionKind::Sleep | ActionKind::GoHome | ActionKind::Hide => "home",
                ActionKind::Flee => "safety",
                ActionKind::Fight => "the fight",
                ActionKind::Follow | ActionKind::Rout => "the army",
                ActionKind::Work => "work",
                ActionKind::Haul => "pick up goods",
                ActionKind::Idle => "stroll",
            }
        ),
        Stage::Perform => format!("{doing}{at}"),
        Stage::Deliver => format!("carrying goods{at}"),
        Stage::Done => format!("finished {doing}, deciding what next"),
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn villager_panel(
    selection: Res<Selection>,
    names: Option<Res<NameTable>>,
    calendar: Option<Res<Calendar>>,
    villagers_data: Option<Res<Villagers>>,
    econ: Option<Res<Economy>>,
    villagers: Query<(
        &Villager,
        &Occupation,
        &Employment,
        &Needs,
        &Activity,
        &DecisionTrace,
        &sim_economy::Inventory,
    )>,
    buildings: Query<&Building>,
    settlements: Query<&Settlement>,
    soldiers: Query<SoldierQuery, With<Enlisted>>,
    army: Option<Res<Army>>,
    mut panel: Query<&mut Visibility, With<VillagerPanel>>,
    mut text: Query<&mut Text, With<VillagerText>>,
) {
    let (Ok(mut vis), Ok(mut text)) = (panel.single_mut(), text.single_mut()) else {
        return;
    };
    let (Some(names), Some(calendar), Some(data), Some(econ), Some(army)) =
        (names, calendar, villagers_data, econ, army)
    else {
        return;
    };
    if let Some(Ok(soldier)) = selection.villager.map(|e| soldiers.get(e)) {
        *vis = Visibility::Inherited;
        text.0 = soldier_text(soldier, &names, &army, &settlements);
        return;
    }
    let Some(Ok((v, occ, job, needs, act, trace, inv))) =
        selection.villager.map(|e| villagers.get(e))
    else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    let years = v.age_days as u32 / calendar.days_per_year();
    let home = settlements
        .get(v.settlement)
        .map(|s| names.get(s.name).to_string())
        .unwrap_or_default();
    let workplace = job
        .workplace
        .and_then(|w| buildings.get(w).ok())
        .map(|b| b.kind.name().to_string())
        .unwrap_or_else(|| "none".into());
    let target = act
        .target
        .and_then(|t| buildings.get(t).ok())
        .map(|b| b.kind.name().to_string());
    let hunger_note = if needs.hunger >= data.data.needs.starving {
        " starving"
    } else if needs.hunger >= data.data.needs.hungry {
        " hungry"
    } else {
        ""
    };
    let carrying: Vec<String> = inv
        .contents()
        .map(|(g, q)| format!("{q} {}", econ.good_name(g)))
        .collect();

    let mut s = String::new();
    s += &format!("{}\n", names.get(v.name));
    s += &format!(
        "{} | {years} years | lives in {home}\n",
        capitalise(occ.name())
    );
    s += &format!("Works at: {workplace} | {} jobs done\n\n", job.cycles);
    s += &format!(
        "Hunger {} {:.0}%{hunger_note}\n",
        style::bar(needs.hunger, 12),
        needs.hunger * 100.0
    );
    s += &format!(
        "Rest   {} {:.0}%\n",
        style::bar(needs.rest, 12),
        needs.rest * 100.0
    );
    s += &format!(
        "Safety {} {:.0}%\n\n",
        style::bar(needs.safety, 12),
        needs.safety * 100.0
    );
    s += &format!("Now: {}\n", describe_action(act.kind, act.stage, target));
    if !carrying.is_empty() {
        s += &format!("Carrying: {}\n", carrying.join(", "));
    }
    s += "\nWhy (last decision):\n";
    for (i, o) in trace.options.iter().enumerate() {
        let marker = if i == 0 { ">" } else { " " };
        let factors: Vec<String> = o
            .factors
            .iter()
            .map(|(n, x)| format!("{n} {x:.2}"))
            .collect();
        s += &format!(
            "{marker} {:<13} {:.2}  {}\n",
            o.kind.name(),
            o.score,
            factors.join(", ")
        );
    }
    s += "\nMemories: none yet (arrive in M5)";
    text.0 = s;
}

type SoldierQuery = (
    Option<&'static Villager>,
    Option<&'static Soldier>,
    &'static CombatStats,
    &'static Needs,
    &'static Activity,
    &'static DecisionTrace,
    Option<&'static Equipment>,
    Option<&'static Follower>,
    Has<Captain>,
    &'static sim_core::Allegiance,
);

/// The inspector for a soldier or captain (UI-03 for armies).
#[allow(clippy::type_complexity)] // A query item.
fn soldier_text(
    (villager, soldier, cs, needs, act, trace, eq, follower, captain, allegiance): (
        Option<&Villager>,
        Option<&Soldier>,
        &CombatStats,
        &Needs,
        &Activity,
        &DecisionTrace,
        Option<&Equipment>,
        Option<&Follower>,
        bool,
        &sim_core::Allegiance,
    ),
    names: &NameTable,
    army: &Army,
    settlements: &Query<&Settlement>,
) -> String {
    let name = crate::army::person_name(names, villager, soldier);
    let role = if captain {
        "Captain"
    } else if follower.is_some() {
        "Follower"
    } else {
        "Soldier"
    };
    let mut s = format!(
        "{name}
{role} | {}
",
        army.faction_name(allegiance.0)
    );
    if let Some(v) = villager
        && let Ok(home) = settlements.get(v.settlement)
    {
        s += &format!(
            "From {}
",
            names.get(home.name)
        );
    }
    s += "
";
    s += &format!(
        "Health {} {:.0}/{:.0}
",
        style::bar(cs.health / cs.max_health.max(1.0), 12),
        cs.health.max(0.0),
        cs.max_health
    );
    s += &format!(
        "Morale {} {:.0}%
",
        style::bar(cs.morale, 12),
        cs.morale * 100.0
    );
    s += &format!(
        "Hunger {} {:.0}%
",
        style::bar(needs.hunger, 12),
        needs.hunger * 100.0
    );
    let kit = match eq {
        Some(e) if e.weapon && e.tools => "weapon and tools",
        Some(e) if e.weapon => "weapon",
        Some(e) if e.tools => "tools",
        _ => "bare hands",
    };
    s += &format!(
        "Armed with: {kit}

"
    );
    s += &format!(
        "Now: {}
",
        act.kind.name()
    );
    s += "
Why (last decision):
";
    for (i, o) in trace.options.iter().enumerate() {
        let marker = if i == 0 { ">" } else { " " };
        let factors: Vec<String> = o
            .factors
            .iter()
            .map(|(n, x)| format!("{n} {x:.2}"))
            .collect();
        s += &format!(
            "{marker} {:<13} {:.2}  {}\n",
            o.kind.name(),
            o.score,
            factors.join(", ")
        );
    }
    s
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters.
fn settlement_panel(
    selection: Res<Selection>,
    names: Option<Res<NameTable>>,
    econ: Option<Res<Economy>>,
    settlements: Query<(
        &Settlement,
        Option<&SettlementEconomy>,
        Option<&FoodSecurity>,
        Option<&Threat>,
    )>,
    stocks: Query<&Stockpile>,
    villagers: Query<(&Villager, &Occupation, &CombatStats, &Activity), Without<Enlisted>>,
    army: Option<Res<Army>>,
    mut panel: Query<&mut Visibility, With<SettlementPanel>>,
    mut text: Query<&mut Text, With<SettlementText>>,
) {
    let (Ok(mut vis), Ok(mut text)) = (panel.single_mut(), text.single_mut()) else {
        return;
    };
    let (Some(names), Some(econ)) = (names, econ) else {
        return;
    };
    let Some(Ok((settlement, se, food, threat))) = selection.settlement.map(|e| settlements.get(e))
    else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    let sel = selection.settlement.expect("checked above");
    let mut s = format!("{}\n", names.get(settlement.name));
    s += &format!(
        "Region {} | Owner: independent | Loyalty: n/a until M4\n\n",
        settlement.region
    );
    let Some(se) = se else {
        s += "Nobody is simulated here yet: one region is populated in M2;\nthe rest of the island arrives with LOD in M5.";
        text.0 = s;
        return;
    };
    let mut by_occ: BTreeMap<Occupation, u32> = BTreeMap::new();
    let (mut militia, mut scared) = (0, 0);
    for (v, occ, cs, act) in &villagers {
        if v.settlement == sel {
            *by_occ.entry(*occ).or_default() += 1;
            if matches!(cs.stance, Stance::Militia { .. }) {
                militia += 1;
            } else if matches!(act.kind, ActionKind::Flee | ActionKind::Hide)
                && act.stage != Stage::Done
            {
                scared += 1;
            }
        }
    }
    if let Some(t) = threat
        && (t.level > 0.0 || t.provoked_by.is_some())
    {
        s += &format!(
            "Threat {} {:.0}% | {scared} hiding or fleeing
",
            style::bar(t.level, 10),
            t.level * 100.0
        );
        if let (Some(f), Some(army)) = (t.provoked_by, army.as_ref()) {
            s += &format!(
                "Up in arms against {}: {militia} fighting
",
                army.faction_name(f)
            );
        }
        s += "
";
    }
    s += &format!("Population {}\n", se.population);
    for (occ, n) in &by_occ {
        s += &format!("  {:<11} {n}\n", occ.name());
    }
    if let Some(f) = food {
        s += &format!("\nFood security: {:.1} days\n", f.days);
    }
    if let Ok(store) = stocks.get(se.storehouse) {
        s += "\nStorehouse:\n";
        if store.goods.is_empty() {
            s += "  empty\n";
        }
        for (g, q) in &store.goods {
            s += &format!("  {:<8} {q}\n", econ.good_name(*g));
        }
    }
    text.0 = s;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_actions_readably() {
        assert_eq!(
            describe_action(ActionKind::Work, Stage::Travel, Some("farm".into())),
            "walking to work (farm)"
        );
        assert_eq!(
            describe_action(ActionKind::Sleep, Stage::Perform, None),
            "sleeping"
        );
        assert_eq!(capitalise("farmer"), "Farmer");
    }
}
