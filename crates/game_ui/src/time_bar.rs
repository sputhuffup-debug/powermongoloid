//! GP-11: time controls (pause, 1x, 2x, 4x, 8x) and the clock (SIM-04).
//! Speed only scales Bevy's virtual time, which drives the fixed-tick
//! loop; it never touches simulation state, so replays are unaffected.
//! Commands issued while paused queue as messages and apply next tick.

use std::time::Duration;

use bevy::prelude::*;
use sim_core::{Calendar, SIM_TICK_HZ, SimTick, SimTimings};

use crate::style;

pub struct TimeBarPlugin;

impl Plugin for TimeBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_time_bar).add_systems(
            Update,
            (
                speed_keys.run_if(crate::playing),
                speed_buttons,
                update_time_bar,
                cap_catch_up,
            )
                .chain()
                .in_set(crate::UiSet::Hud),
        );
    }
}

pub const SPEEDS: [f32; 4] = [1.0, 2.0, 4.0, 8.0];

#[derive(Component)]
struct ClockText;

/// A speed button: `None` is pause.
#[derive(Component, Clone, Copy)]
struct SpeedButton(Option<f32>);

/// Most wall-clock time the fixed-tick loop may take in one frame. With
/// about 8 ms of rendering this keeps frames near 30 fps (NFR-05).
const SIM_FRAME_BUDGET: Duration = Duration::from_millis(25);

/// How much real time one frame may hand to the fixed-tick loop: as many
/// ticks as fit in `budget` at the current `tick_cost` (at least one), at
/// `speed` ticks of game time per tick of real time.
pub fn catch_up_cap(tick_cost: Duration, speed: f64, budget: Duration) -> Duration {
    let cost = tick_cost.as_secs_f64().max(1e-4);
    let ticks = (budget.as_secs_f64() / cost).floor().max(1.0);
    Duration::from_secs_f64(ticks / (SIM_TICK_HZ * speed.max(1e-3)))
}

/// NFR-05: when ticks get expensive (a battle of thousands) at high speed,
/// run the game slower than the chosen speed instead of freezing. Without
/// a cap, a slow frame owes more ticks, which make the next frame slower
/// still, until frames take seconds. Bevy drops virtual time beyond
/// `max_delta`; the sim only counts ticks, so replays are unaffected.
fn cap_catch_up(mut time: ResMut<Time<Virtual>>, timings: Res<SimTimings>) {
    let cap = catch_up_cap(
        timings.total_avg,
        time.relative_speed_f64(),
        SIM_FRAME_BUDGET,
    );
    if time.max_delta() != cap {
        time.set_max_delta(cap);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const MS: fn(u64) -> Duration = Duration::from_millis;

    #[test]
    fn cheap_ticks_leave_room_to_catch_up() {
        // 3 ms ticks: 8 fit in 25 ms; at 1x that is 0.8 s of real time.
        let cap = catch_up_cap(MS(3), 1.0, MS(25));
        assert_eq!(cap, Duration::from_secs_f64(0.8));
        // At 8x the same 8 ticks pass in an eighth of the time.
        assert_eq!(
            catch_up_cap(MS(3), 8.0, MS(25)),
            Duration::from_secs_f64(0.1)
        );
    }

    #[test]
    fn expensive_ticks_at_high_speed_get_one_tick_per_frame() {
        // The 2026-10-07 stress run: 36 ms ticks at 8x.
        let cap = catch_up_cap(MS(36), 8.0, MS(25));
        assert_eq!(cap, Duration::from_secs_f64(1.0 / 80.0));
    }

    #[test]
    fn no_timing_yet_still_allows_progress() {
        assert!(catch_up_cap(Duration::ZERO, 1.0, MS(25)) >= MS(100));
    }
}
