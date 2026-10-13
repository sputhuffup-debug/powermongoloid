//! The settlement panel's edict dials (GP-34, UI-11; M4a step 4, ADR 0020):
//! WORK / DUES / TABLE rows at a town you rule, each option a button with a
//! one-line forecast; the label badges and the inspector's "moved by the
//! Forge edict" line come from the same view.
//!
//! The forecasts need `&mut World` (they run queries), so [`refresh`] is an
//! exclusive system that runs at most twice a second and copies words into
//! the UI-side [`GovernView`]; everything else only reads that. Nothing here
//! writes sim state: a click sends a `Govern` command (`army::send`).

use std::collections::BTreeMap;

use bevy::prelude::*;
use sim_agents::{Assignment, Edicts, Mustered, Owner, Rationed, Villager};
use sim_core::{
    Calendar, CommandKind, Dial, Dues, FactionId, GovernAct, NameTable, SimTick, SpawnId, Table,
    WorkFocus,
};
use sim_economy::Occupation;
use sim_military::govern::{
    DuesForecast, TableForecast, WorkForecast, dues_forecast, lock_left, refusal, table_forecast,
    work_forecast,
};
use sim_military::{Progression, Soldier};

use super::capitalise;
use crate::selection::Selection;
use crate::style;

/// Seconds between refreshes of the view.
const EVERY: f32 = 0.5;

pub(super) fn build(app: &mut App) {
    app.init_resource::<GovernView>().add_systems(
        Update,
        (refresh, show, click)
            .chain()
            .in_set(crate::UiSet::Hud)
            .before(super::settlement_panel),
    );
}

/// One option of a dial, as the view copied it out of the sim.
#[derive(Debug, Clone, Default)]
pub(crate) struct OptionView {
    /// The setting's name.
    pub name: &'static str,
    /// The one-line forecast.
    pub forecast: String,
    /// Why it can't be chosen now.
    pub refusal: Option<&'static str>,
    /// It is the setting now.
    pub current: bool,
    /// It is ordered and takes effect at the next hour.
    pub pending: bool,
}

/// One dial of the selected town.
#[derive(Debug, Clone, Default)]
pub(crate) struct DialView {
    /// "WORK", "DUES" or "TABLE".
    pub title: &'static str,
    /// The day it unlocks (1-based), while locked.
    pub locked_until_day: Option<u64>,
    /// Its settings.
    pub options: Vec<OptionView>,
}

/// What the panel, the labels and the inspector read (UI-side only).
#[derive(Resource, Default)]
pub(crate) struct GovernView {
    /// The selected town's SpawnId, when it is yours and has dials.
    town: Option<u64>,
    /// Its three dials.
    dials: Vec<DialView>,
    /// Label badge lines for towns whose dials are off their defaults.
    pub badges: BTreeMap<Entity, Vec<String>>,
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A loyalty term in words.
fn loyalty_words(term: f32) -> String {
    if term.abs() < 0.005 {
        "no loyalty cost".to_string()
    } else {
        format!("loyalty {term:+.2}")
    }
}

/// The Work dial's forecast line.
pub(crate) fn work_words(focus: WorkFocus, f: &WorkForecast) -> String {
    let name = focus.name();
    if f.total == 0 {
        return format!("{name}: nobody to move");
    }
    let mut s = if f.dawns == 0 {
        format!("{name}: {} now", f.total)
    } else {
        format!(
            "{name}: {} now, {} in all over {}",
            f.first,
            f.total,
            plural(f.dawns, "dawn", "dawns")
        )
    };
    if f.farmers == 1 {
        s += "; 1 farmer leaves the fields";
    } else if f.farmers > 1 {
        s += &format!("; {} farmers leave the fields", f.farmers);
    }
    s
}

/// The Dues dial's forecast line. `collector` names the captain who would
/// collect; `reach` is edicts.reach.
pub(crate) fn dues_words(
    dues: Dues,
    f: &DuesForecast,
    collector: Option<&str>,
    reach: f32,
) -> String {
    let loyalty = loyalty_words(f.term);
    let nobody = format!("no army within {reach:.0} tiles");
    match dues {
        Dues::Light => format!("Light: nothing asked; {loyalty}"),
        Dues::Tithe => match collector {
            Some(who) => format!(
                "Tithe: {:.1} food and {} arms to {who} now; {loyalty}",
                f.food, f.arms
            ),
            None => format!(
                "Tithe: {:.1} food and {} arms to give; {nobody}; {loyalty}",
                f.food, f.arms
            ),
        },
        Dues::Levy => {
            let mut s = format!(
                "Levy: {} now, +{} each dawn ({} at most); {loyalty}",
                plural(f.men_per_day, "man", "men"),
                f.men_per_day,
                f.men_cap
            );
            if collector.is_none() {
                s += &format!("; {nobody}");
            }
            s
        }
    }
}

/// The Table dial's forecast line. `work` is the ration's work factor.
pub(crate) fn table_words(table: Table, f: &TableForecast, work: f32) -> String {
    match table {
        Table::Normal => "Normal: no ration, no feast".to_string(),
        Table::Ration => format!(
            "Ration: eat {:.0}% less, work {:.0}% slower; {}",
            (1.0 - f.hunger) * 100.0,
            (1.0 - work) * 100.0,
            loyalty_words(f.term)
        ),
        Table::Feast => match f.refused {
            Some(why) => format!("Feast: {why}"),
            None => {
                let food = if (f.food_days - 0.5).abs() < 0.05 {
                    "half a day's".to_string()
                } else {
                    format!("{:.1} days'", f.food_days)
                };
                format!("Feast: {food} food; {:+.2} after", f.term)
            }
        },
    }
}

/// The label badge lines for a town's dials (empty at the defaults).
/// `first_wave` is whether the Work dial's first wave is still running and
/// `dawns` the dawns it has left.
pub(crate) fn badge_lines(
    ed: &Edicts,
    tick: u64,
    ticks_per_day: u64,
    work_dawns: u32,
) -> Vec<String> {
    let tpd = ticks_per_day.max(1);
    let first_wave = |dial: Dial| {
        let since = ed.dial(dial).since;
        since > 0 && tick < (since / tpd + 1) * tpd
    };
    let mut out = Vec::new();
    if ed.work != WorkFocus::Balanced {
        let name = ed.work.name();
        out.push(if first_wave(Dial::Work) && work_dawns > 0 {
            format!(
                "{name}: first wave now, ~{} to finish",
                plural(work_dawns, "dawn", "dawns")
            )
        } else if first_wave(Dial::Work) {
            format!("{name}: first wave now")
        } else {
            name.to_string()
        });
    }
    if ed.dues != Dues::Light {
        let name = ed.dues.name();
        out.push(if ed.dues == Dues::Levy && first_wave(Dial::Dues) {
            format!("{name}: first muster now")
        } else {
            name.to_string()
        });
    }
    if ed.table != Table::Normal {
        out.push(ed.table.name().to_string());
    }
    out
}

/// The inspector's line about a villager an edict touched (None if none).
pub(crate) fn mark_words(
    occ: Occupation,
    assignment: Option<&Assignment>,
    mustered: Option<&Mustered>,
    rationed: bool,
    ticks_per_day: u64,
) -> Option<String> {
    let day = |t: u64| t / ticks_per_day.max(1) + 1;
    let mut parts = Vec::new();
    if let Some(a) = assignment {
        parts.push(format!(
            "{}, moved from {} by {} {} edict, day {}",
            capitalise(occ.name()),
            a.from.name(),
            if a.by_steward { "the steward's" } else { "the" },
            a.focus.name(),
            day(a.since)
        ));
    }
    if let Some(m) = mustered {
        parts.push(format!("Mustered on the green since day {}", day(m.since)));
    }
    if rationed {
        parts.push("On short rations by the Ration edict".to_string());
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

/// The act a button of `dial` at position `i` sends.
fn act_of(dial: Dial, i: usize) -> Option<GovernAct> {
    Some(match dial {
        Dial::Work => GovernAct::Work(*WorkFocus::ALL.get(i)?),
        Dial::Dues => GovernAct::Dues(*Dues::ALL.get(i)?),
        Dial::Table => GovernAct::Table(*Table::ALL.get(i)?),
    })
}

fn clear(view: &mut GovernView) {
    view.town = None;
    view.dials.clear();
    view.badges.clear();
}

/// Rebuilds the view from the sim, at most twice a second (exclusive: the
/// forecasts run queries).
fn refresh(world: &mut World, mut last: Local<(f32, Option<Entity>)>) {
    let _span = info_span!("game_ui::govern::refresh").entered();
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |t| t.elapsed_secs());
    let sel = world.get_resource::<Selection>().and_then(|s| s.settlement);
    if now >= last.0 && now - last.0 < EVERY && last.1 == sel {
        return;
    }
    *last = (now, sel);
    let ready = world.contains_resource::<SimTick>()
        && world.contains_resource::<Calendar>()
        && world.contains_resource::<Progression>()
        && world.contains_resource::<NameTable>();
    let mut view = std::mem::take(&mut *world.resource_mut::<GovernView>());
    clear(&mut view);
    if ready {
        fill(world, sel, &mut view);
    }
    *world.resource_mut::<GovernView>() = view;
}

fn fill(world: &mut World, sel: Option<Entity>, view: &mut GovernView) {
    let tick = world.resource::<SimTick>().0;
    let tpd = world.resource::<Calendar>().ticks_per_day;
    let data = world.resource::<Progression>().data.edicts;
    let mut q = world.query::<(Entity, &Edicts, &Owner)>();
    let towns: Vec<(Entity, Edicts)> = q
        .iter(world)
        .filter(|(_, _, o)| o.faction == FactionId::PLAYER)
        .map(|(e, ed, _)| (e, ed.clone()))
        .collect();
    for (s, ed) in &towns {
        if ed.is_default() {
            continue;
        }
        let dawns = if ed.work != WorkFocus::Balanced {
            work_forecast(world, *s, ed.work).dawns
        } else {
            0
        };
        view.badges.insert(*s, badge_lines(ed, tick, tpd, dawns));
    }
    let Some((s, ed)) = sel.and_then(|s| towns.iter().find(|(e, _)| *e == s)) else {
        return;
    };
    let Some(id) = world.get::<SpawnId>(*s).map(|i| i.0) else {
        return;
    };
    let me = FactionId::PLAYER;
    let locked = |world: &World, dial: Dial| {
        (lock_left(world, *s, dial) > 0)
            .then(|| (tick + lock_left(world, *s, dial)) / tpd.max(1) + 1)
    };
    let mut dials = Vec::new();

    let options = WorkFocus::ALL
        .iter()
        .map(|&f| OptionView {
            name: f.name(),
            forecast: work_words(f, &work_forecast(world, *s, f)),
            refusal: refusal(world, me, *s, GovernAct::Work(f)),
            current: ed.work == f && ed.pending_work.is_none(),
            pending: ed.pending_work == Some(f),
        })
        .collect();
    dials.push(DialView {
        title: "WORK",
        locked_until_day: locked(world, Dial::Work),
        options,
    });

    let collector_of = |world: &mut World, c: Option<u64>| -> Option<String> {
        let c = c?;
        let mut q = world.query::<(&SpawnId, Option<&Villager>, Option<&Soldier>)>();
        let names = world.resource::<NameTable>();
        q.iter(world)
            .find(|(id, ..)| id.0 == c)
            .map(|(_, v, sd)| format!("captain {}", crate::army::person_name(names, v, sd)))
    };
    let mut options = Vec::new();
    for d in Dues::ALL {
        let f = dues_forecast(world, *s, d);
        let who = collector_of(world, f.collector);
        options.push(OptionView {
            name: d.name(),
            forecast: dues_words(d, &f, who.as_deref(), data.reach),
            refusal: refusal(world, me, *s, GovernAct::Dues(d)),
            current: ed.dues == d && ed.pending_dues.is_none(),
            pending: ed.pending_dues == Some(d),
        });
    }
    dials.push(DialView {
        title: "DUES",
        locked_until_day: locked(world, Dial::Dues),
        options,
    });

    let options = Table::ALL
        .iter()
        .map(|&t| OptionView {
            name: t.name(),
            forecast: table_words(t, &table_forecast(world, *s, t), data.ration.work),
            refusal: refusal(world, me, *s, GovernAct::Table(t)),
            current: ed.table == t && ed.pending_table.is_none(),
            pending: ed.pending_table == Some(t),
        })
        .collect();
    dials.push(DialView {
        title: "TABLE",
        locked_until_day: locked(world, Dial::Table),
        options,
    });
    view.town = Some(id);
    view.dials = dials;
}

#[derive(Component)]
pub(super) struct GovernRoot;

#[derive(Component)]
struct OptButton(Dial, usize);

#[derive(Component)]
struct OptText(Dial, usize);

#[derive(Component)]
struct HeaderText(Dial);

#[derive(Component)]
struct ForecastText(Dial);

/// Spawns the dial rows under the settlement panel.
pub(super) fn spawn(p: &mut ChildSpawnerCommands) {
    p.spawn((
        GovernRoot,
        Node {
            flex_direction: FlexDirection::Column,
            margin: UiRect::top(Val::Px(8.0)),
            row_gap: Val::Px(2.0),
            display: Display::None,
            ..default()
        },
    ))
    .with_children(|root| {
        for (dial, names) in [
            (Dial::Work, WorkFocus::ALL.map(|f| f.name()).to_vec()),
            (Dial::Dues, Dues::ALL.map(|f| f.name()).to_vec()),
            (Dial::Table, Table::ALL.map(|f| f.name()).to_vec()),
        ] {
            root.spawn((
                HeaderText(dial),
                Text::new(""),
                style::font(12.0),
                TextColor(style::GOLD),
            ));
            root.spawn(Node {
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                for (i, name) in names.into_iter().enumerate() {
                    row.spawn((
                        Button,
                        OptButton(dial, i),
                        Node {
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(style::BUTTON),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            OptText(dial, i),
                            Text::new(name),
                            style::font(12.0),
                            TextColor(style::TEXT),
                        ));
                    });
                }
            });
            root.spawn((
                ForecastText(dial),
                Text::new(""),
                style::font(11.0),
                TextColor(style::DIM),
            ));
        }
    });
}

/// Draws the view: the buttons' marks and colours, the headers and the
/// hovered (else current) option's forecast.
#[allow(clippy::type_complexity)] // Bevy queries.
fn show(
    view: Res<GovernView>,
    mut root: Query<&mut Node, With<GovernRoot>>,
    mut buttons: Query<(&OptButton, &Interaction, &mut BackgroundColor)>,
    mut labels: Query<
        (&OptText, &mut Text, &mut TextColor),
        (Without<HeaderText>, Without<ForecastText>),
    >,
    mut headers: Query<(&HeaderText, &mut Text), (Without<OptText>, Without<ForecastText>)>,
    mut forecasts: Query<(&ForecastText, &mut Text), (Without<OptText>, Without<HeaderText>)>,
) {
    let Ok(mut node) = root.single_mut() else {
        return;
    };
    let want = if view.town.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if node.display != want {
        node.display = want;
    }
    if view.town.is_none() {
        return;
    }
    let dial_view = |d: Dial| view.dials.get(d as usize);
    for (b, interaction, mut bg) in &mut buttons {
        let Some(o) = dial_view(b.0).and_then(|d| d.options.get(b.1)) else {
            continue;
        };
        let hot = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        let color = if o.current {
            style::BUTTON_ACTIVE
        } else if o.refusal.is_some() {
            style::BUTTON_OFF
        } else if hot {
            style::BUTTON_ACTIVE.with_alpha(0.6)
        } else {
            style::BUTTON
        };
        bg.set_if_neq(BackgroundColor(color));
    }
    for (l, mut text, mut color) in &mut labels {
        let Some(o) = dial_view(l.0).and_then(|d| d.options.get(l.1)) else {
            continue;
        };
        let s = if o.current {
            format!("[{}]", o.name)
        } else if o.pending {
            format!("({})", o.name)
        } else {
            o.name.to_string()
        };
        if text.0 != s {
            text.0 = s;
        }
        let c = if o.refusal.is_some() && !o.current && !o.pending {
            style::DIM
        } else {
            style::TEXT
        };
        color.set_if_neq(TextColor(c));
    }
    for (h, mut text) in &mut headers {
        let Some(d) = dial_view(h.0) else {
            continue;
        };
        let now = d.options.iter().find(|o| o.current).map_or("", |o| o.name);
        let mut s = format!("{}: {now}", d.title);
        if let Some(day) = d.locked_until_day {
            s += &format!("  (locked until day {day})");
        }
        if text.0 != s {
            text.0 = s;
        }
    }
    for (f, mut text) in &mut forecasts {
        let Some(d) = dial_view(f.0) else {
            continue;
        };
        let hovered = buttons
            .iter()
            .find(|(b, i, _)| {
                b.0 == f.0 && matches!(i, Interaction::Hovered | Interaction::Pressed)
            })
            .map(|(b, ..)| b.1);
        let o = hovered
            .and_then(|i| d.options.get(i))
            .or_else(|| d.options.iter().find(|o| o.pending))
            .or_else(|| d.options.iter().find(|o| o.current));
        let mut s = o.map_or(String::new(), |o| {
            let mut s = o.forecast.clone();
            if o.pending {
                s += " (takes effect next hour)";
            } else if !o.current
                && let Some(why) = o.refusal
                && !o.forecast.contains(why)
            {
                s += &format!(" [{why}]");
            }
            s
        });
        if let Some(day) = d.locked_until_day
            && hovered.is_none()
        {
            s += &format!(" (locked until day {day})");
        }
        if text.0 != s {
            text.0 = s;
        }
    }
}

/// A press on an option sends `Govern` (the sim validates it again).
fn click(
    view: Res<GovernView>,
    buttons: Query<(&Interaction, &OptButton), Changed<Interaction>>,
    mut out: MessageWriter<sim_core::SimCommand>,
) {
    let Some(town) = view.town else {
        return;
    };
    for (interaction, b) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(o) = view
            .dials
            .get(b.0 as usize)
            .and_then(|d| d.options.get(b.1))
        else {
            continue;
        };
        if o.current || o.pending || o.refusal.is_some() {
            continue;
        }
        if let Some(act) = act_of(b.0, b.1) {
            crate::army::send(
                &mut out,
                CommandKind::Govern {
                    settlement: town,
                    act,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_agents::DialState;

    /// GP-34: the forecast lines name what each setting costs.
    #[test]
    fn forecast_words_name_the_cost() {
        let w = WorkForecast {
            first: 3,
            total: 9,
            farmers: 2,
            dawns: 2,
        };
        assert_eq!(
            work_words(WorkFocus::Forge, &w),
            "Forge: 3 now, 9 in all over 2 dawns; 2 farmers leave the fields"
        );
        let none = WorkForecast::default();
        assert_eq!(work_words(WorkFocus::Walls, &none), "Walls: nobody to move");

        let tithe = DuesForecast {
            term: -0.05,
            food: 12.0,
            arms: 4,
            collector: Some(812),
            ..Default::default()
        };
        assert_eq!(
            dues_words(Dues::Tithe, &tithe, Some("captain Aldo"), 60.0),
            "Tithe: 12.0 food and 4 arms to captain Aldo now; loyalty -0.05"
        );
        assert_eq!(
            dues_words(Dues::Tithe, &tithe, None, 60.0),
            "Tithe: 12.0 food and 4 arms to give; no army within 60 tiles; loyalty -0.05"
        );
        let levy = DuesForecast {
            term: -0.08,
            men_per_day: 2,
            men_cap: 10,
            ..Default::default()
        };
        assert_eq!(
            dues_words(Dues::Levy, &levy, Some("captain Aldo"), 60.0),
            "Levy: 2 men now, +2 each dawn (10 at most); loyalty -0.08"
        );
        assert_eq!(
            dues_words(Dues::Light, &DuesForecast::default(), None, 60.0),
            "Light: nothing asked; no loyalty cost"
        );

        let ration = TableForecast {
            term: -0.10,
            hunger: 0.75,
            ..Default::default()
        };
        assert_eq!(
            table_words(Table::Ration, &ration, 0.9),
            "Ration: eat 25% less, work 10% slower; loyalty -0.10"
        );
        let feast = TableForecast {
            term: 0.08,
            hunger: 1.0,
            food_days: 0.5,
            refused: None,
        };
        assert_eq!(
            table_words(Table::Feast, &feast, 1.0),
            "Feast: half a day's food; +0.08 after"
        );
        let refused = TableForecast {
            refused: Some("too little food for a feast"),
            ..feast
        };
        assert_eq!(
            table_words(Table::Feast, &refused, 1.0),
            "Feast: too little food for a feast"
        );
    }

    /// The badge shows a non-default dial and counts the first wave down.
    #[test]
    fn badges_count_the_first_wave_down() {
        let tpd = 2400;
        let mut ed = Edicts::default();
        assert!(badge_lines(&ed, 100, tpd, 0).is_empty());
        ed.work = WorkFocus::Forge;
        ed.dials[Dial::Work as usize] = DialState {
            by_player: true,
            locked_until: 3 * tpd,
            since: tpd + 100,
        };
        assert_eq!(
            badge_lines(&ed, tpd + 200, tpd, 2),
            vec!["Forge: first wave now, ~2 dawns to finish"]
        );
        // After the next dawn the first wave is over.
        assert_eq!(badge_lines(&ed, 2 * tpd, tpd, 1), vec!["Forge"]);
        ed.table = Table::Ration;
        assert_eq!(badge_lines(&ed, 2 * tpd, tpd, 1), vec!["Forge", "Ration"]);
    }

    /// The inspector names the edict that moved a villager.
    #[test]
    fn marks_name_the_edict() {
        let a = Assignment {
            from: Occupation::Farmer,
            focus: WorkFocus::Forge,
            since: 5 * 2400 + 10,
            by_steward: false,
        };
        let s = mark_words(Occupation::Smith, Some(&a), None, false, 2400).unwrap();
        assert_eq!(s, "Smith, moved from farmer by the Forge edict, day 6");
        let by = Assignment {
            by_steward: true,
            ..a
        };
        assert!(
            mark_words(Occupation::Smith, Some(&by), None, false, 2400)
                .unwrap()
                .contains("by the steward's Forge edict")
        );
        let m = Mustered { since: 2400 };
        assert_eq!(
            mark_words(Occupation::Farmer, None, Some(&m), false, 2400).unwrap(),
            "Mustered on the green since day 2"
        );
        assert!(mark_words(Occupation::Farmer, None, None, false, 2400).is_none());
    }
}
