//! Armies (UI-01, GP-02, GP-03): the captain panel, right-click orders and
//! a settlement order menu. Every order is sent as a `SimCommand` from the
//! player's faction; the UI never changes the simulation itself.
//!
//! Mouse: right-click ground = move there (Shift queues); right-click a
//! village = attack, recruit, gather food, trade, equip, give food,
//! garrison, move or (towns not yours, in a run) demand submission.
//! Keys (army selected): R return to camp, F forage where the army stands,
//! Backspace cancel orders, P cycle posture.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_render::CameraRig;
use sim_agents::{ActionKind, Activity, CombatStats, Villager};
use sim_core::{
    Calendar, CommandKind, FactionId, NameTable, OrderSpec, Posture, RunState, SimCommand, SpawnId,
};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_military::{
    Army, Captain, Champion, ChampionRank, Equipment, Follower, MilitaryLog, Progression,
    RunProgress, Soldier, Traits, describe, forages_below,
};
use sim_world::{Settlement, WorldMap};

use crate::panels::take::{self, TakeView};
use crate::selection::{Selection, ground_hit};
use crate::style;

pub struct ArmyUiPlugin;

impl Plugin for ArmyUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrderMenu>()
            .add_systems(Startup, spawn_army_panel)
            .add_systems(
                Update,
                (
                    right_click.run_if(crate::playing),
                    menu_buttons,
                    menu_labels,
                    panel_buttons,
                    hotkeys.run_if(crate::playing),
                    draw_menu,
                    army_panel,
                )
                    .chain()
                    .in_set(crate::UiSet::Hud),
            );
    }
}

/// Names a person: villager or soldier.
pub fn person_name(
    names: &NameTable,
    villager: Option<&Villager>,
    soldier: Option<&Soldier>,
) -> String {
    match (villager, soldier) {
        (Some(v), _) => names.get(v.name).to_string(),
        (None, Some(s)) => names.get(s.name).to_string(),
        _ => "?".into(),
    }
}

#[derive(Component)]
struct ArmyPanel;

#[derive(Component)]
struct ArmyText;

#[derive(Component, Clone, Copy, PartialEq)]
enum PanelButton {
    Posture(Posture),
    Return,
    Forage,
    Cancel,
    Appoint,
}

/// The open settlement order menu, if any.
#[derive(Resource, Default)]
pub(crate) struct OrderMenu {
    open: Option<MenuTarget>,
    root: Option<Entity>,
}

impl OrderMenu {
    /// The open menu's (captain, settlement) SpawnIds, if one is open.
    pub(crate) fn target(&self) -> Option<(u64, u64)> {
        self.open.map(|t| (t.captain, t.settlement))
    }
}

#[derive(Clone, Copy, PartialEq)]
struct MenuTarget {
    captain: u64,
    settlement: u64,
    tile: IVec2,
    at: Vec2,
    queue: bool,
}

#[derive(Component, Clone, Copy)]
struct MenuButton(MenuAction);

#[derive(Clone, Copy, PartialEq)]
enum MenuAction {
    Attack,
    Recruit,
    GatherFood,
    Trade,
    Equip,
    Give,
    Garrison,
    Move,
    /// GP-32 (M4a): ask a town to kneel.
    Demand,
}

impl MenuAction {
    const ALL: [MenuAction; 9] = [
        MenuAction::GatherFood,
        MenuAction::Recruit,
        MenuAction::Trade,
        MenuAction::Equip,
        MenuAction::Give,
        MenuAction::Attack,
        MenuAction::Garrison,
        MenuAction::Move,
        MenuAction::Demand,
    ];

    fn label(self) -> &'static str {
        match self {
            MenuAction::Attack => "Attack",
            MenuAction::Recruit => "Recruit 10",
            MenuAction::GatherFood => "Gather food",
            MenuAction::Trade => "Trade",
            MenuAction::Equip => "Equip",
            // GP-22: food beyond the army's own few days.
            MenuAction::Give => "Give food",
            // GP-05: refused unless you hold the settlement.
            MenuAction::Garrison => "Garrison",
            MenuAction::Move => "Move here",
            MenuAction::Demand => "Demand submission",
        }
    }

    /// The label with a town's facts in hand (M4a step 5): "Gift food" and
    /// "Demand submission" carry their forecasts; `None` hides the button.
    fn text(self, facts: Option<&take::Facts>) -> Option<String> {
        match self {
            MenuAction::Give => Some(take::gift_label(facts)),
            MenuAction::Demand => take::demand_label(facts),
            other => Some(other.label().to_string()),
        }
    }
}

/// The text of a menu button (so [`menu_labels`] can find it).
#[derive(Component)]
struct MenuLabel(MenuAction);

fn spawn_army_panel(mut commands: Commands) {
    commands
        .spawn((
            ArmyPanel,
            Node {
                // Bottom centre: clear of the inspector and settlement panel
                // (right) and the dev debug panel (left).
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-195.0)),
                width: Val::Px(390.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(style::PANEL),
            Interaction::None,
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                ArmyText,
                Text::new(""),
                style::font(13.0),
                TextColor(style::TEXT),
            ));
            for row in [
                vec![
                    PanelButton::Posture(Posture::Passive),
                    PanelButton::Posture(Posture::Neutral),
                    PanelButton::Posture(Posture::Aggressive),
                ],
                vec![
                    PanelButton::Return,
                    PanelButton::Forage,
                    PanelButton::Cancel,
                    PanelButton::Appoint,
                ],
            ] {
                p.spawn(Node {
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|r| {
                    for b in row {
                        let label = match b {
                            PanelButton::Posture(p) => capitalise(p.name()),
                            PanelButton::Return => "Return (R)".into(),
                            PanelButton::Forage => "Forage (F)".into(),
                            PanelButton::Cancel => "Cancel".into(),
                            PanelButton::Appoint => "Appoint captain".into(),
                        };
                        r.spawn((
                            Button,
                            b,
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(style::BUTTON),
                        ))
                        .with_children(|b| {
                            b.spawn((Text::new(label), style::font(13.0), TextColor(style::TEXT)));
                        });
                    }
                });
            }
        });
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The selected army's captain, if it is the player's: (entity, SpawnId).
fn own_army(
    selection: &Selection,
    captains: &Query<(&SpawnId, &Captain)>,
) -> Option<(Entity, u64)> {
    let e = selection.army?;
    let (id, c) = captains.get(e).ok()?;
    (c.faction == FactionId::PLAYER).then_some((e, id.0))
}

/// Sends a player command (the one path every UI order takes).
pub(crate) fn send(out: &mut MessageWriter<SimCommand>, kind: CommandKind) {
    info!("order: {kind:?}");
    out.write(SimCommand::player(FactionId::PLAYER, kind));
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn right_click(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    map: Option<Res<WorldMap>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<CameraRig>>,
    captains: Query<(&SpawnId, &Captain)>,
    settlements: Query<(&SpawnId, &Settlement), With<SettlementEconomy>>,
    mut menu: ResMut<OrderMenu>,
    mut out: MessageWriter<SimCommand>,
) {
    if mouse.just_pressed(MouseButton::Left) || keys.just_pressed(KeyCode::Escape) {
        // Let a button press on the menu land first (menu_buttons runs
        // next); it closes the menu itself.
        if keys.just_pressed(KeyCode::Escape) {
            menu.open = None;
        }
    }
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    menu.open = None;
    let Some((_, captain)) = own_army(&selection, &captains) else {
        return;
    };
    let (Some(map), Ok(window), Ok((cam, cam_gt))) = (map, window.single(), camera.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Some(hit) = cam
        .viewport_to_world(cam_gt, cursor)
        .ok()
        .and_then(|ray| ground_hit(&map, ray))
    else {
        return;
    };
    let tile = hit.floor().as_ivec2();
    let queue = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let village = settlements
        .iter()
        .map(|(id, s)| (id.0, s.center.as_vec2().distance(hit)))
        .filter(|(_, d)| *d < 18.0)
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    match village {
        Some((settlement, _)) => {
            menu.open = Some(MenuTarget {
                captain,
                settlement,
                tile,
                at: cursor,
                queue,
            });
        }
        None => send(
            &mut out,
            CommandKind::IssueOrder {
                captain,
                order: OrderSpec::MoveTo {
                    x: tile.x,
                    y: tile.y,
                },
                queue,
            },
        ),
    }
}

fn draw_menu(mut commands: Commands, mut menu: ResMut<OrderMenu>, take: Res<TakeView>) {
    if !menu.is_changed() {
        return;
    }
    if let Some(root) = menu.root.take() {
        commands.entity(root).despawn();
    }
    let Some(target) = menu.open else {
        return;
    };
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(target.at.x),
                top: Val::Px(target.at.y),
                padding: UiRect::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(style::PANEL),
            Interaction::None,
            GlobalZIndex(10),
        ))
        .with_children(|m| {
            let facts = take.get(target.settlement);
            for action in MenuAction::ALL {
                // Hidden until its facts say it applies (`menu_labels`).
                let text = action.text(facts);
                m.spawn((
                    Button,
                    MenuButton(action),
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        display: if text.is_some() {
                            Display::Flex
                        } else {
                            Display::None
                        },
                        ..default()
                    },
                    BackgroundColor(style::BUTTON),
                ))
                .with_children(|b| {
                    b.spawn((
                        MenuLabel(action),
                        Text::new(text.unwrap_or_default()),
                        style::font(13.0),
                        TextColor(style::TEXT),
                    ));
                });
            }
        })
        .id();
    menu.root = Some(root);
}

/// Keeps the open menu's labels current (M4a step 5): "Gift food (+0.08
/// goodwill)", "Demand submission (need 11 within 40, have 8)", greyed with
/// its reason when refused, hidden where it doesn't apply.
fn menu_labels(
    menu: Res<OrderMenu>,
    take: Res<TakeView>,
    mut buttons: Query<(&MenuButton, &mut Node, &mut BackgroundColor)>,
    mut texts: Query<(&MenuLabel, &mut Text, &mut TextColor)>,
) {
    let _span = info_span!("game_ui::army::menu_labels").entered();
    let Some(t) = menu.open else {
        return;
    };
    let facts = take.get(t.settlement);
    for (button, mut node, mut bg) in &mut buttons {
        let shown = if button.0.text(facts).is_some() {
            Display::Flex
        } else {
            Display::None
        };
        node.set_if_neq(Node {
            display: shown,
            ..node.clone()
        });
        let off = button.0 == MenuAction::Demand && !take.demand_open(t.settlement);
        let colour = if off {
            style::BUTTON_OFF
        } else {
            style::BUTTON
        };
        if bg.0 != colour {
            bg.0 = colour;
        }
    }
    for (label, mut text, mut colour) in &mut texts {
        let new = label.0.text(facts).unwrap_or_default();
        if text.0 != new {
            text.0 = new;
        }
        let off = label.0 == MenuAction::Demand && !take.demand_open(t.settlement);
        let c = if off { style::DIM } else { style::TEXT };
        if colour.0 != c {
            colour.0 = c;
        }
    }
}

fn menu_buttons(
    mouse: Res<ButtonInput<MouseButton>>,
    take: Res<TakeView>,
    buttons: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut menu: ResMut<OrderMenu>,
    mut out: MessageWriter<SimCommand>,
) {
    let Some(t) = menu.open else {
        return;
    };
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let s = t.settlement;
        // A greyed demand does nothing (the sim would refuse it too).
        if button.0 == MenuAction::Demand && !take.demand_open(s) {
            return;
        }
        let order = match button.0 {
            MenuAction::Attack => OrderSpec::Attack { settlement: s },
            MenuAction::Recruit => OrderSpec::Recruit {
                settlement: s,
                count: 10,
            },
            MenuAction::GatherFood => OrderSpec::GatherFood { settlement: s },
            MenuAction::Trade => OrderSpec::Trade { settlement: s },
            MenuAction::Equip => OrderSpec::Equip { settlement: s },
            MenuAction::Give => OrderSpec::Give { settlement: s },
            MenuAction::Garrison => OrderSpec::Garrison { settlement: s },
            MenuAction::Move => OrderSpec::MoveTo {
                x: t.tile.x,
                y: t.tile.y,
            },
            MenuAction::Demand => OrderSpec::Demand { settlement: s },
        };
        send(
            &mut out,
            CommandKind::IssueOrder {
                captain: t.captain,
                order,
                queue: t.queue,
            },
        );
        menu.open = None;
        return;
    }
    // A left click anywhere else closes the menu.
    if mouse.just_released(MouseButton::Left) {
        menu.open = None;
    }
}

fn posture_after(p: Posture) -> Posture {
    match p {
        Posture::Passive => Posture::Neutral,
        Posture::Neutral => Posture::Aggressive,
        Posture::Aggressive => Posture::Passive,
    }
}

fn panel_buttons(
    selection: Res<Selection>,
    captains: Query<(&SpawnId, &Captain)>,
    followers: Query<(&SpawnId, &Follower)>,
    buttons: Query<(&Interaction, &PanelButton), Changed<Interaction>>,
    progression: Res<Progression>,
    mut out: MessageWriter<SimCommand>,
) {
    let Some((captain_e, captain)) = own_army(&selection, &captains) else {
        return;
    };
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *button {
            PanelButton::Posture(posture) => {
                send(&mut out, CommandKind::SetPosture { captain, posture });
            }
            PanelButton::Return => send(
                &mut out,
                CommandKind::IssueOrder {
                    captain,
                    order: OrderSpec::Return,
                    queue: false,
                },
            ),
            PanelButton::Forage => send(&mut out, forage(captain, &progression)),
            PanelButton::Cancel => send(&mut out, CommandKind::CancelOrders { captain }),
            PanelButton::Appoint => {
                // The selected person, if they follow this captain.
                if let Some(Ok((id, f))) = selection.villager.map(|v| followers.get(v))
                    && f.captain == captain_e
                {
                    send(&mut out, CommandKind::AppointCaptain { follower: id.0 });
                } else {
                    info!("appoint: select one of this captain's followers first");
                }
            }
        }
    }
}

/// SIM-38: forage where the army stands, within progression.ron
/// `forage.default_radius`.
fn forage(captain: u64, progression: &Progression) -> CommandKind {
    CommandKind::IssueOrder {
        captain,
        order: OrderSpec::Forage {
            radius: progression.data.forage.default_radius,
        },
        queue: false,
    }
}

fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    captains: Query<(&SpawnId, &Captain)>,
    progression: Res<Progression>,
    mut out: MessageWriter<SimCommand>,
) {
    let Some((e, captain)) = own_army(&selection, &captains) else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyR) {
        send(
            &mut out,
            CommandKind::IssueOrder {
                captain,
                order: OrderSpec::Return,
                queue: false,
            },
        );
    }
    if keys.just_pressed(KeyCode::KeyF) {
        send(&mut out, forage(captain, &progression));
    }
    if keys.just_pressed(KeyCode::Backspace) {
        send(&mut out, CommandKind::CancelOrders { captain });
    }
    if keys.just_pressed(KeyCode::KeyP)
        && let Ok((_, c)) = captains.get(e)
    {
        send(
            &mut out,
            CommandKind::SetPosture {
                captain,
                posture: posture_after(c.posture),
            },
        );
    }
}

type MemberQuery = (
    &'static Follower,
    &'static CombatStats,
    &'static Activity,
    Option<&'static Equipment>,
);

/// The last `n` military log lines that name `faction_name`, newest
/// first: the captain panel's "Recent" (UI-01, ADR 0018), so other
/// factions' dealings don't push the player's own lines out.
fn recent_lines<'a>(
    log: &'a MilitaryLog,
    faction_name: &'a str,
    n: usize,
) -> impl Iterator<Item = (u64, &'a str)> + 'a {
    log.entries
        .iter()
        .rev()
        .filter(move |(_, l)| l.contains(faction_name))
        .take(n)
        .map(|(t, l)| (*t, l.as_str()))
}

/// What the captain panel adds after Food (GP-03, ADR 0018): whether, and
/// how, the army helps itself at villages it passes without an order
/// ([`forages_below`]: neutral takes a day's food, passive buys, aggressive
/// plunders); nothing for an army that never eats (a guard camp, a band
/// for hire). The panel's one decimal would otherwise hide the line.
fn food_note(posture: Posture, days: f32, never_eats: bool) -> &'static str {
    if never_eats || days >= forages_below(posture) {
        return "";
    }
    match posture {
        Posture::Passive => " (hungry: buys food)",
        Posture::Neutral => " (hungry: helps itself)",
        Posture::Aggressive => " (plunders villages it passes)",
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters.
fn army_panel(
    selection: Res<Selection>,
    names: Option<Res<NameTable>>,
    army: Option<Res<Army>>,
    econ: Option<Res<Economy>>,
    villagers: Option<Res<sim_agents::Villagers>>,
    log: Option<Res<MilitaryLog>>,
    calendar: Option<Res<Calendar>>,
    (run, progress, progression): (
        Option<Res<RunState>>,
        Option<Res<RunProgress>>,
        Option<Res<Progression>>,
    ),
    champions: Query<(Option<&ChampionRank>, Option<&Traits>), With<Champion>>,
    captains: Query<(
        &Captain,
        &CombatStats,
        Option<&Stockpile>,
        Option<&Villager>,
        Option<&Soldier>,
        Option<&Equipment>,
        Has<sim_military::NoUpkeep>,
    )>,
    members: Query<MemberQuery>,
    settlements: Query<(&SpawnId, &Settlement)>,
    mut panel: Query<&mut Visibility, With<ArmyPanel>>,
    mut text: Query<&mut Text, With<ArmyText>>,
    mut buttons: Query<(&PanelButton, &mut BackgroundColor)>,
) {
    let (Ok(mut vis), Ok(mut text)) = (panel.single_mut(), text.single_mut()) else {
        return;
    };
    let (
        Some(names),
        Some(army),
        Some(econ),
        Some(villagers),
        Some(log),
        Some(calendar),
        Some(run),
        Some(progress),
        Some(progression),
    ) = (
        names,
        army,
        econ,
        villagers,
        log,
        calendar,
        run,
        progress,
        progression,
    )
    else {
        return;
    };
    let Some(e) = selection.army else {
        *vis = Visibility::Hidden;
        return;
    };
    let Ok((c, cs, baggage, villager, soldier, cap_eq, never_eats)) = captains.get(e) else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    let captain_count = captains
        .iter()
        .filter(|(c, ..)| c.faction == FactionId::PLAYER)
        .count();
    let (mut n, mut fighting, mut routed, mut morale, mut weapons, mut tools) =
        (0u32, 0u32, 0u32, cs.morale, 0u32, 0u32);
    if let Some(eq) = cap_eq {
        weapons += eq.weapon as u32;
        tools += eq.tools as u32;
    }
    for (f, s, act, eq) in &members {
        if f.captain != e {
            continue;
        }
        n += 1;
        morale += s.morale;
        match act.kind {
            ActionKind::Fight => fighting += 1,
            ActionKind::Rout => routed += 1,
            _ => {}
        }
        if let Some(eq) = eq {
            weapons += eq.weapon as u32;
            tools += eq.tools as u32;
        }
    }
    let people = n + 1;
    let morale = morale / people as f32;
    let food = baggage.map_or(0.0, |b| b.food_value(&econ));
    let days = food / (villagers.data.needs.hunger_per_day * people as f32);
    let settlement_name = |id: u64| {
        settlements
            .iter()
            .find(|(s, _)| s.0 == id)
            .map_or_else(|| "?".into(), |(_, s)| names.get(s.name).to_string())
    };
    let order_line = |spec: &OrderSpec| match *spec {
        OrderSpec::Recruit { settlement, .. }
        | OrderSpec::GatherFood { settlement }
        | OrderSpec::Equip { settlement }
        | OrderSpec::Attack { settlement }
        | OrderSpec::Trade { settlement }
        | OrderSpec::Give { settlement }
        | OrderSpec::Garrison { settlement } => {
            format!("{} at {}", describe(spec), settlement_name(settlement))
        }
        _ => describe(spec),
    };
    let mut s = format!(
        "{} | Captain {}\n",
        army.faction_name(c.faction),
        person_name(&names, villager, soldier)
    );
    s += &format!("Posture: {}\n", c.posture.name());
    // SIM-34, SIM-35: the champion's rank and traits.
    if let Ok((rank, traits)) = champions.get(e) {
        let ranks = &progression.data.champion.ranks;
        let (r, points) = rank.map_or((0, 0), |r| (r.rank, r.points));
        let next = ranks
            .get(r as usize)
            .map_or_else(|| "top rank".to_string(), |n| format!("next at {n}"));
        let traits: Vec<&str> = traits
            .map(|t| {
                t.0.iter()
                    .filter_map(|&id| progression.trait_data(id))
                    .map(|t| t.name.as_str())
                    .collect()
            })
            .unwrap_or_default();
        s += &format!(
            "Champion, rank {r} ({points} points, {next}) | Traits: {}\n",
            if traits.is_empty() {
                "none".to_string()
            } else {
                traits.join(", ")
            }
        );
    }
    // GP-25: captains beyond the champion's army need slots.
    let slots_full = run.in_run() && c.faction == FactionId::PLAYER && {
        let used = captain_count.saturating_sub(1) as u32;
        s += &format!("Captain slots: {used}/{} used\n", progress.captain_slots);
        used >= progress.captain_slots
    };
    s += &format!("Men: {n} ({fighting} fighting, {routed} routed)\n");
    s += &format!("Morale {} {:.0}%\n", style::bar(morale, 12), morale * 100.0);
    s += &format!(
        "Food   {} {days:.1} days{}\n",
        style::bar((days / 5.0).min(1.0), 12),
        food_note(c.posture, days, never_eats)
    );
    s += &format!("Armed: {weapons}/{people} weapons, {tools}/{people} tools\n");
    s += "\nOrders:\n";
    if c.orders.is_empty() {
        s += "  none: holding\n";
    }
    for o in &c.orders {
        s += &format!("  {}\n", order_line(&o.spec));
    }
    s += "\nRecent:\n";
    for (tick, line) in recent_lines(&log, army.faction_name(c.faction), 4) {
        let day = calendar.at(tick).day + 1;
        s += &format!("  day {day}: {line}\n");
    }
    if c.faction == FactionId::INDEPENDENT && never_eats {
        // GP-27: a band for hire (the Sellswords run modifier).
        s += "Sellswords for hire: see the run bar\n(not yours: no orders)";
    } else if c.faction != FactionId::PLAYER {
        s += "(not yours: no orders)";
    } else {
        s += "Right-click: move / village orders (Shift queues)";
    }
    text.0 = s;
    for (b, mut bg) in &mut buttons {
        let active = matches!(b, PanelButton::Posture(p) if *p == c.posture);
        bg.0 = if active {
            style::BUTTON_ACTIVE
        } else if *b == PanelButton::Appoint && slots_full {
            style::BUTTON_OFF
        } else {
            style::BUTTON
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UI-01 (ADR 0018): the captain panel's "Recent" keeps the player's
    /// own lines, newest first, however many other factions' lines came
    /// after them.
    #[test]
    fn recent_lines_keep_your_own() {
        let mut log = MilitaryLog::default();
        log.push(1, "Your warband: captain 7 ordered to gather food");
        log.push(2, "Ashford shared food with Your warband (+2.1 days)");
        for t in 3..10 {
            log.push(t, "Raiders plundered Northwell (+0.8 days)");
        }
        log.push(10, "Rival bought food from Ashford (+1.0 days)");
        let mine: Vec<(u64, &str)> = recent_lines(&log, "Your warband", 4).collect();
        assert_eq!(
            mine,
            vec![
                (2, "Ashford shared food with Your warband (+2.1 days)"),
                (1, "Your warband: captain 7 ordered to gather food"),
            ]
        );
        let raiders: Vec<u64> = recent_lines(&log, "Raiders", 4).map(|l| l.0).collect();
        assert_eq!(raiders, vec![9, 8, 7, 6]);
        assert_eq!(recent_lines(&log, "Your warband", 0).count(), 0);
    }

    /// GP-03 (ADR 0018): the Food line says how a hungry army helps
    /// itself, by posture and at that posture's threshold, and nothing for
    /// an army that never eats.
    #[test]
    fn the_food_note_follows_posture() {
        assert_eq!(
            food_note(Posture::Neutral, 0.9, false),
            " (hungry: helps itself)"
        );
        assert_eq!(food_note(Posture::Neutral, 1.0, false), "");
        assert_eq!(
            food_note(Posture::Passive, 0.5, false),
            " (hungry: buys food)"
        );
        assert_eq!(food_note(Posture::Passive, 1.5, false), "");
        assert_eq!(
            food_note(Posture::Aggressive, 2.5, false),
            " (plunders villages it passes)"
        );
        assert_eq!(food_note(Posture::Aggressive, 3.0, false), "");
        // A band for hire or a guard camp: no food, never eats, no note.
        for p in [Posture::Passive, Posture::Neutral, Posture::Aggressive] {
            assert_eq!(food_note(p, 0.0, true), "");
        }
    }
}
