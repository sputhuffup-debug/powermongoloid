//! Armies (UI-01, GP-02, GP-03): the captain panel, right-click orders and
//! a settlement order menu. Every order is sent as a `SimCommand` from the
//! player's faction; the UI never changes the simulation itself.
//!
//! Mouse: right-click ground = move there (Shift queues); right-click a
//! village = attack, recruit, gather food, trade, equip or move.
//! Keys (army selected): R return to camp, Backspace cancel orders,
//! P cycle posture.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use game_render::CameraRig;
use sim_agents::{ActionKind, Activity, CombatStats, Villager};
use sim_core::{CommandKind, FactionId, NameTable, OrderSpec, Posture, SimCommand, SpawnId};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_military::{Army, Captain, Equipment, Follower, MilitaryLog, Soldier, describe};
use sim_world::{Settlement, WorldMap};

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
                    right_click,
                    menu_buttons,
                    panel_buttons,
                    hotkeys,
                    draw_menu,
                    army_panel,
                )
                    .chain(),
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
    Cancel,
    Appoint,
}

/// The open settlement order menu, if any.
#[derive(Resource, Default)]
struct OrderMenu {
    open: Option<MenuTarget>,
    root: Option<Entity>,
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
    Garrison,
    Move,
}

impl MenuAction {
    const ALL: [MenuAction; 7] = [
        MenuAction::GatherFood,
        MenuAction::Recruit,
        MenuAction::Trade,
        MenuAction::Equip,
        MenuAction::Attack,
        MenuAction::Garrison,
        MenuAction::Move,
    ];

    fn label(self) -> &'static str {
        match self {
            MenuAction::Attack => "Attack",
            MenuAction::Recruit => "Recruit 10",
            MenuAction::GatherFood => "Gather food",
            MenuAction::Trade => "Trade",
            MenuAction::Equip => "Equip",
            // GP-05: refused unless you hold the settlement.
            MenuAction::Garrison => "Garrison",
            MenuAction::Move => "Move here",
        }
    }
}

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

fn send(out: &mut MessageWriter<SimCommand>, kind: CommandKind) {
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

fn draw_menu(mut commands: Commands, mut menu: ResMut<OrderMenu>) {
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
            for action in MenuAction::ALL {
                m.spawn((
                    Button,
                    MenuButton(action),
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(style::BUTTON),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(action.label()),
                        style::font(13.0),
                        TextColor(style::TEXT),
                    ));
                });
            }
        })
        .id();
    menu.root = Some(root);
}

fn menu_buttons(
    mouse: Res<ButtonInput<MouseButton>>,
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
        let order = match button.0 {
            MenuAction::Attack => OrderSpec::Attack { settlement: s },
            MenuAction::Recruit => OrderSpec::Recruit {
                settlement: s,
                count: 10,
            },
            MenuAction::GatherFood => OrderSpec::GatherFood { settlement: s },
            MenuAction::Trade => OrderSpec::Trade { settlement: s },
            MenuAction::Equip => OrderSpec::Equip { settlement: s },
            MenuAction::Garrison => OrderSpec::Garrison { settlement: s },
            MenuAction::Move => OrderSpec::MoveTo {
                x: t.tile.x,
                y: t.tile.y,
            },
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

fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    captains: Query<(&SpawnId, &Captain)>,
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters.
fn army_panel(
    selection: Res<Selection>,
    names: Option<Res<NameTable>>,
    army: Option<Res<Army>>,
    econ: Option<Res<Economy>>,
    villagers: Option<Res<sim_agents::Villagers>>,
    log: Option<Res<MilitaryLog>>,
    captains: Query<(
        &Captain,
        &CombatStats,
        Option<&Stockpile>,
        Option<&Villager>,
        Option<&Soldier>,
        Option<&Equipment>,
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
    let (Some(names), Some(army), Some(econ), Some(villagers), Some(log)) =
        (names, army, econ, villagers, log)
    else {
        return;
    };
    let Some(e) = selection.army else {
        *vis = Visibility::Hidden;
        return;
    };
    let Ok((c, cs, baggage, villager, soldier, cap_eq)) = captains.get(e) else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
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
    s += &format!("Men: {n} ({fighting} fighting, {routed} routed)\n");
    s += &format!("Morale {} {:.0}%\n", style::bar(morale, 12), morale * 100.0);
    s += &format!(
        "Food   {} {days:.1} days\n",
        style::bar((days / 5.0).min(1.0), 12)
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
    for (tick, line) in log.entries.iter().rev().take(4) {
        let day = tick / 2400 + 1;
        s += &format!("  day {day}: {line}\n");
    }
    if c.faction != FactionId::PLAYER {
        s += "(not yours: no orders)";
    } else {
        s += "Right-click: move / village orders (Shift queues)";
    }
    text.0 = s;
    for (b, mut bg) in &mut buttons {
        let active = matches!(b, PanelButton::Posture(p) if *p == c.posture);
        bg.0 = if active {
            style::BUTTON_ACTIVE
        } else {
            style::BUTTON
        };
    }
}
