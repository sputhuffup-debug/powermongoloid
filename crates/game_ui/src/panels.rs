//! Player panels: the villager inspector (UI-03) and the settlement panel
//! (UI-02). Both read simulation state and show it as text.

use std::collections::BTreeMap;

use bevy::prelude::*;
use sim_agents::{
    ActionKind, Activity, CombatStats, DecisionTrace, DeedLog, Employment, Enlisted, FoodSecurity,
    Goodwill, LoyaltyTerms, Needs, Owner, SettlementLoyalty, Stage, Stance, Threat, Villager,
    Villagers,
};
use sim_core::{Calendar, FactionId, NameTable, SimTick, SpawnId};
use sim_economy::{
    Building, Economy, Occupation, SettlementEconomy, Stockpile, Workplace, WorkplaceKind,
};
use sim_military::{
    Army, Captain, Equipment, Follower, Influence, Progression, RequestBoard, RequestKind,
    RunProgress, Soldier, Tenure, Traits, Veteran,
};
use sim_world::Settlement;

use crate::selection::Selection;
use crate::style;

mod govern;
pub(crate) use govern::GovernView;
mod why;

pub struct PanelsPlugin;

impl Plugin for PanelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_panels).add_systems(
            Update,
            (villager_panel, settlement_panel).in_set(crate::UiSet::Hud),
        );
        govern::build(app);
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
            // Below the time bar and the run bar (M4).
            top: if top { Val::Px(146.0) } else { Val::Auto },
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
            govern::spawn(p);
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters.
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
    (progression, ranks): (
        Option<Res<Progression>>,
        Query<(Option<&Veteran>, Option<&Traits>)>,
    ),
    marks: Query<(
        Option<&sim_agents::Assignment>,
        Option<&sim_agents::Mustered>,
        Has<sim_agents::Rationed>,
    )>,
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
        // SIM-34, SIM-36: veteran rank and traits.
        if let Some(Ok((veteran, traits))) = selection.villager.map(|e| ranks.get(e)) {
            let mut extra = String::new();
            if let Some(v) = veteran {
                extra += &format!(
                    "Rank: {} ({} battles)\n",
                    army.data.veterans.name(v.rank),
                    v.battles
                );
            }
            if let (Some(t), Some(p)) = (traits, progression.as_ref())
                && !t.0.is_empty()
            {
                let names: Vec<&str> =
                    t.0.iter()
                        .filter_map(|&id| p.trait_data(id))
                        .map(|t| t.name.as_str())
                        .collect();
                extra += &format!("Traits: {}\n", names.join(", "));
            }
            if let Some(i) = text.0.find("\nHealth") {
                text.0.insert_str(i, &format!("\n{}", extra.trim_end()));
            }
        }
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
    // GP-34 (M4a): an edict moved, mustered or rationed him.
    if let Some(Ok((a, m, r))) = selection.villager.map(|e| marks.get(e))
        && let Some(line) = govern::mark_words(*occ, a, m, r, calendar.ticks_per_day)
    {
        s += &line;
        s += "\n";
    }
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
    standing: Query<(
        &SpawnId,
        Option<&Owner>,
        Option<&Goodwill>,
        Option<&SettlementLoyalty>,
    )>,
    // UI-11 (M4a): what the "Why they feel this way" section reads.
    (lordship, villagers_data, progression, run_progress): (
        Query<(
            &Owner,
            Option<&Tenure>,
            Option<&LoyaltyTerms>,
            Option<&DeedLog>,
            Option<&Influence>,
        )>,
        Option<Res<Villagers>>,
        Option<Res<Progression>>,
        Option<Res<RunProgress>>,
    ),
    (board, calendar, tick): (
        Option<Res<RequestBoard>>,
        Option<Res<Calendar>>,
        Option<Res<SimTick>>,
    ),
    stocks: Query<&Stockpile>,
    workplaces: Query<&Workplace>,
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
    s += &format!("Region {}", settlement.region);
    // GP-05, GP-06, GP-22: who holds it, how it feels about you, and what
    // it asks.
    if let Ok((id, owner, goodwill, loyalty)) = standing.get(sel) {
        let owner = owner.map_or(FactionId::INDEPENDENT, |o| o.faction);
        let owner_name = army
            .as_ref()
            .map_or("?".to_string(), |a| a.faction_name(owner).to_string());
        s += &format!(" | Owner: {owner_name}\n");
        if let Some(g) = goodwill {
            let g = g.get(FactionId::PLAYER);
            s += &format!(
                "Goodwill to you {} {:+.2}\n",
                style::bar((g + 1.0) / 2.0, 10),
                g
            );
        }
        if let Some(l) = loyalty
            && owner != FactionId::INDEPENDENT
        {
            s += &format!(
                "Loyalty {} mean {:.0}%, lowest {:.0}%",
                style::bar(l.mean, 10),
                l.mean * 100.0,
                l.min * 100.0
            );
            if let (Some(since), Some(cal)) = (l.below_since, calendar.as_ref()) {
                s += &format!(" | unrest since day {}", cal.at(since).day + 1);
            }
            s += "\n";
        }
        if let (Some(board), Some(cal), Some(tick)) =
            (board.as_ref(), calendar.as_ref(), tick.as_ref())
        {
            for r in board.open.iter().filter(|r| r.settlement == id.0) {
                let left = r.deadline.saturating_sub(tick.0) / cal.ticks_per_hour().max(1);
                let what = match r.kind {
                    RequestKind::BringFood { need, delivered } => {
                        format!("bring food ({delivered:.0}/{need:.0})")
                    }
                    k => k.name().to_string(),
                };
                let mut reward = Vec::new();
                if r.reward.food > 0.0 {
                    reward.push(format!("{:.0} food", r.reward.food));
                }
                for (g, q) in &r.reward.goods {
                    reward.push(format!("{q} {}", econ.good_name(*g)));
                }
                if r.reward.volunteers > 0 {
                    reward.push(format!("{} volunteers", r.reward.volunteers));
                }
                s += &format!(
                    "Asks you: {what}, {left}h left | reward: {}\n",
                    if reward.is_empty() {
                        "goodwill".to_string()
                    } else {
                        reward.join(", ")
                    }
                );
            }
        }
        // UI-11, SIM-60, SIM-61 (M4a): the town's own account of its mood.
        if let (Some(vd), Some(cal), Some(tick), Ok((held, tenure, terms, log, town_influence))) = (
            villagers_data.as_ref(),
            calendar.as_ref(),
            tick.as_ref(),
            lordship.get(sel),
        ) {
            let influence = town_influence.and_then(|i| {
                let p = progression.as_ref()?;
                Some(why::InfluenceFacts {
                    influence: i,
                    data: &p.data.influence,
                    invite_goodwill: p.data.invitations.goodwill,
                    seat: run_progress.as_ref().and_then(|r| r.rival_seat) == Some(id.0),
                })
            });
            let name = |f: FactionId| {
                army.as_ref()
                    .map_or("?".to_string(), |a| a.faction_name(f).to_string())
            };
            let why = why::section(&why::Why {
                owner: *held,
                goodwill,
                food_days: food.map(|f| f.days),
                loyalty,
                terms,
                log,
                tenure,
                data: &vd.data.loyalty,
                scar_fade_per_day: progression
                    .as_ref()
                    .map_or(0.0, |p| p.data.tenure.scar_fade_per_day),
                influence,
                tick: tick.0,
                ticks_per_day: cal.ticks_per_day,
                name: &name,
            });
            if !why.is_empty() {
                s += "\n";
                s += &why;
            }
        }
        s += "\n";
    } else {
        s += "\n\n";
    }
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
    // SIM-64: what waits at the smithies and the mine, beside the
    // storehouse: inputs on their way to a cycle, outputs on their way in.
    let shops: Vec<String> = se
        .workplaces
        .iter()
        .filter_map(|&e| Some((workplaces.get(e).ok()?, stocks.get(e).ok()?)))
        .filter(|(wp, _)| matches!(wp.kind, WorkplaceKind::Smithy | WorkplaceKind::Mine))
        .map(|(wp, stock)| workshop_line(&econ, wp, stock))
        .collect();
    if !shops.is_empty() {
        s += "\nWorkshops:\n";
        for line in shops {
            s += &format!("  {line}\n");
        }
    }
    text.0 = s;
}

/// One workshop's line on the settlement panel (SIM-64): its kind, what it
/// makes and its stock (no food: workers' leftover meals aren't news).
fn workshop_line(econ: &Economy, wp: &Workplace, stock: &Stockpile) -> String {
    let makes = wp
        .recipe
        .map(|r| econ.recipe(r))
        .and_then(|r| r.outputs.first())
        .map_or(String::new(), |(g, _)| format!(" ({})", econ.good_name(*g)));
    let goods: Vec<String> = stock
        .goods
        .iter()
        .filter(|(g, _)| !econ.is_food(**g))
        .map(|(g, q)| format!("{} {q}", econ.good_name(*g)))
        .collect();
    let goods = if goods.is_empty() {
        "empty".to_string()
    } else {
        goods.join(", ")
    };
    format!("{}{makes}: {goods}", capitalise(wp.kind.name()))
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

    /// SIM-64: a workshop's line names what it makes and its stock,
    /// leaving out food.
    #[test]
    fn workshop_lines_show_stock_without_food() {
        let econ = Economy::default_data();
        let weapons = econ.recipes_for(WorkplaceKind::Smithy)[1];
        let wp = Workplace {
            kind: WorkplaceKind::Smithy,
            recipe: Some(weapons),
            workers: Vec::new(),
        };
        let good = |n: &str| {
            (0..econ.good_count() as u8)
                .map(sim_economy::GoodId)
                .find(|g| econ.good_name(*g) == n)
                .unwrap()
        };
        let mut stock = Stockpile::with_capacity(200);
        assert_eq!(workshop_line(&econ, &wp, &stock), "Smithy (weapons): empty");
        stock.add(good("ore"), 12);
        stock.add(good("grain"), 3);
        stock.add(good("weapons"), 2);
        assert_eq!(
            workshop_line(&econ, &wp, &stock),
            "Smithy (weapons): ore 12, weapons 2"
        );
    }
}
