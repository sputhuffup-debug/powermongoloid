//! GP-11: time controls (pause, 1x, 2x, 4x, 8x) and the clock (SIM-04).
//! Speed only scales Bevy's virtual time, which drives the fixed-tick
//! loop; it never touches simulation state, so replays are unaffected.
//! Commands issued while paused queue as messages and apply next tick.

use std::time::Duration;

use bevy::prelude::*;
use sim_core::{Calendar, SimTick};

use crate::style;

pub struct TimeBarPlugin;

impl Plugin for TimeBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (allow_catch_up, spawn_time_bar))
            .add_systems(Update, (speed_keys, speed_buttons, update_time_bar).chain());
    }
}

pub const SPEEDS: [f32; 4] = [1.0, 2.0, 4.0, 8.0];

#[derive(Component)]
struct ClockText;

/// A speed button: `None` is pause.
#[derive(Component, Clone, Copy)]
struct SpeedButton(Option<f32>);

/// At 8x, a slow frame can hold many ticks' worth of time; raise the
/// virtual clock's per-frame cap so those ticks still run instead of being
/// dropped.
fn allow_catch_up(mut time: ResMut<Time<Virtual>>) {
    time.set_max_delta(Duration::from_secs(2));
}

fn spawn_time_bar(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-190.0)),
                width: Val::Px(380.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(style::PANEL),
            Interaction::None,
        ))
        .with_children(|bar| {
            bar.spawn((
                ClockText,
                Text::new("Day 1"),
                style::font(14.0),
                TextColor(style::TEXT),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ));
            for speed in [None, Some(1.0), Some(2.0), Some(4.0), Some(8.0)] {
                let label = match speed {
                    None => "II".to_string(),
                    Some(s) => format!("{s}x"),
                };
                bar.spawn((
                    Button,
                    SpeedButton(speed),
                    Node {
                        padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
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

fn set_speed(time: &mut Time<Virtual>, speed: Option<f32>) {
    match speed {
        None => time.pause(),
        Some(s) => {
            time.unpause();
            time.set_relative_speed(s);
        }
    }
}

/// Space toggles pause; 1-4 pick 1x, 2x, 4x, 8x.
fn speed_keys(keys: Res<ButtonInput<KeyCode>>, mut time: ResMut<Time<Virtual>>) {
    if keys.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
    for (key, speed) in [
        (KeyCode::Digit1, SPEEDS[0]),
        (KeyCode::Digit2, SPEEDS[1]),
        (KeyCode::Digit3, SPEEDS[2]),
        (KeyCode::Digit4, SPEEDS[3]),
    ] {
        if keys.just_pressed(key) {
            set_speed(&mut time, Some(speed));
        }
    }
}

fn speed_buttons(
    mut time: ResMut<Time<Virtual>>,
    buttons: Query<(&Interaction, &SpeedButton), Changed<Interaction>>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            set_speed(&mut time, button.0);
        }
    }
}

fn update_time_bar(
    time: Res<Time<Virtual>>,
    tick: Option<Res<SimTick>>,
    calendar: Option<Res<Calendar>>,
    mut clock: Query<&mut Text, With<ClockText>>,
    mut buttons: Query<(&SpeedButton, &mut BackgroundColor)>,
) {
    let (Some(tick), Some(calendar)) = (tick, calendar) else {
        return;
    };
    let r = calendar.at(tick.0);
    let (h, m) = r.hours_minutes();
    if let Ok(mut text) = clock.single_mut() {
        text.0 = format!("Day {} | {} | {:02}:{:02}", r.day + 1, r.phase.name(), h, m);
    }
    let current = if time.is_paused() {
        None
    } else {
        Some(time.relative_speed())
    };
    for (button, mut bg) in &mut buttons {
        let active = match (button.0, current) {
            (None, None) => true,
            (Some(a), Some(b)) => (a - b).abs() < 1e-3,
            _ => false,
        };
        bg.0 = if active {
            style::BUTTON_ACTIVE
        } else {
            style::BUTTON
        };
    }
}
