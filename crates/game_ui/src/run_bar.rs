//! The run bar (GP-21, GP-24, GP-25, GP-27, GP-28, GP-31), under the time bar
//! in a run: the day, your share of each region's people, the rival's
//! strength and the next step of its clock, your captain slots, your sphere
//! ("Sphere 7, warming 3", M4a) and the overlay; then the offers waiting
//! for you as buttons that open their cards, the fast-forward button
//! (GP-16) and "Abandon run".

use std::collections::BTreeSet;

use bevy::prelude::*;
use game_render::Overlay;
use sim_agents::Owner;
use sim_core::{Calendar, CommandKind, FactionId, NameTable, Offers, RunState, SimTick, SpawnId};
use sim_military::influence::InfluenceData;
use sim_military::{Captain, Follower, Influence, Progression, RivalState, RunProgress};
use sim_world::Settlement;

use crate::cards::{SentOffers, offer_title, spawn_cards};
use crate::feed::FastForward;
use crate::style;
use crate::{UiSet, playing};

pub struct RunBarPlugin;

impl Plugin for RunBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpenOffer>()
            .add_systems(Startup, spawn_run_bar)
            .add_systems(
                Update,
                close_offer
                    .run_if(game_render::epoch_changed)
                    .in_set(UiSet::Screens),
            )
            .add_systems(
                Update,
                (
                    update_run_bar,
                    offer_buttons,
                    bar_buttons.run_if(playing),
                    draw_card_popup,
                )
                    .chain()
                    .in_set(UiSet::Hud),
            );
    }
}

/// A new world (`WorldEpoch`): its offers are not the old world's.
fn close_offer(mut open: ResMut<OpenOffer>) {
    let _span = info_span!("game_ui::run_bar::close_offer").entered();
    open.offer = None;
    open.set_changed();
}

/// The offer whose cards are open, if any.
#[derive(Resource, Default, PartialEq)]
struct OpenOffer {
    offer: Option<u64>,
    root: Option<Entity>,
}

#[derive(Component)]
struct RunBar;

#[derive(Component)]
struct RunText;

/// Holds the offer buttons.
#[derive(Component)]
struct OfferRow;

#[derive(Component, Clone, Copy)]
struct OfferButton(u64);

#[derive(Component, Clone, Copy, PartialEq)]
enum BarButton {
    FastForward,
    Abandon,
    /// Closes the open cards.
    Later,
}

/// Text of the abandon button: armed after one click, asks again.
#[derive(Component)]
struct AbandonText;

fn spawn_run_bar(mut commands: Commands) {
    commands
        .spawn((
            RunBar,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(50.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-460.0)),
                width: Val::Px(920.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(style::PANEL),
            Interaction::None,
            Visibility::Hidden,
        ))
        .with_children(|bar| {
            bar.spawn((
                RunText,
                Text::new(""),
                style::font(13.0),
                TextColor(style::TEXT),
            ));
            bar.spawn(Node {
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    OfferRow,
                    Node {
                        column_gap: Val::Px(6.0),
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                style::button(row, BarButton::FastForward, ">> Next event", 13.0);
                row.spawn((
                    Button,
                    BarButton::Abandon,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(style::BUTTON),
                ))
                .with_children(|b| {
                    b.spawn((
                        AbandonText,
                        Text::new("Abandon run"),
                        style::font(13.0),
                        TextColor(style::TEXT),
                    ));
                });
            });
        });
}

/// GP-31, UI-12: how many towns lie inside your sphere (their influence net,
/// your share less the rival's, is at least `influence.sphere`, the towns
/// you hold included) and how many more are warming to you (at least
/// `influence.warming`, but not inside, among the towns that can warm: not
/// yours and not the rival's seat, as the day's `Deed::Influence` goes).
/// Each town is `(net, can_warm)`.
fn sphere_counts(
    data: &InfluenceData,
    towns: impl IntoIterator<Item = (f32, bool)>,
) -> (usize, usize) {
    let (mut sphere, mut warming) = (0, 0);
    for (net, can_warm) in towns {
        if net >= data.sphere {
            sphere += 1;
        } else if can_warm && net >= data.warming {
            warming += 1;
        }
    }
    (sphere, warming)
}

/// "5h" or "2d 3h" from a tick count.
fn span(ticks: u64, cal: &Calendar) -> String {
    let hours = ticks.div_ceil(cal.ticks_per_hour());
    if hours >= 24 {
        format!("{}d {}h", hours / 24, hours % 24)
    } else {
        format!("{hours}h")
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bevy system parameters.
fn update_run_bar(
    time: Res<Time<Real>>,
    run: Option<Res<RunState>>,
    tick: Option<Res<SimTick>>,
    cal: Option<Res<Calendar>>,
    progress: Option<Res<RunProgress>>,
    rival: Option<Res<RivalState>>,
    progression: Option<Res<Progression>>,
    overlay: Res<Overlay>,
    captains: Query<(Entity, &Captain)>,
    followers: Query<&Follower>,
    influence: Query<(&Influence, &Owner, &SpawnId)>,
    mut bar: Query<&mut Visibility, With<RunBar>>,
    mut text: Query<&mut Text, With<RunText>>,
    mut cooldown: Local<f32>,
) {
    let _span = info_span!("game_ui::run_bar::update").entered();
    let (Some(run), Some(tick), Some(cal), Some(progress), Some(rival)) =
        (run, tick, cal, progress, rival)
    else {
        return;
    };
    let (Ok(mut vis), Ok(mut text)) = (bar.single_mut(), text.single_mut()) else {
        return;
    };
    if !run.in_run() {
        vis.set_if_neq(Visibility::Hidden);
        return;
    }
    vis.set_if_neq(Visibility::Inherited);
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = 0.25;
    let day = cal.at(tick.0).day + 1;
    let shares: Vec<String> = progress
        .dominion
        .iter()
        .enumerate()
        .filter(|(_, r)| r.total > 0)
        .map(|(i, r)| format!("R{i} {:.0}%", r.share() * 100.0))
        .collect();
    let rival_captains: BTreeSet<Entity> = captains
        .iter()
        .filter(|(_, c)| c.faction == FactionId::RIVAL)
        .map(|(e, _)| e)
        .collect();
    let rival_men = rival_captains.len()
        + followers
            .iter()
            .filter(|f| rival_captains.contains(&f.captain))
            .count();
    let mine = captains
        .iter()
        .filter(|(_, c)| c.faction == FactionId::PLAYER)
        .count();
    let mut s = format!(
        "Day {day} | Your share: {}",
        if shares.is_empty() {
            "-".into()
        } else {
            shares.join("  ")
        }
    );
    if rival.seat.is_some() {
        s += &format!(
            " | Rival: {rival_men} men, seat +{:.0}% defence | levy in {}, raid in {}",
            rival.defence_bonus * 100.0,
            span(rival.next_levy.saturating_sub(tick.0), &cal),
            span(rival.next_raid.saturating_sub(tick.0), &cal)
        );
    } else {
        s += " | Rival: unseated";
    }
    s += &format!(
        "\nCaptains besides your champion: {}/{}",
        mine.saturating_sub(1),
        progress.captain_slots
    );
    // GP-31 (M4a): the sphere, from the towns' own influence.
    if let Some(data) = progression
        .as_ref()
        .map(|p| &p.data.influence)
        .filter(|d| d.enabled)
    {
        let seat = progress.rival_seat;
        let towns = influence.iter().map(|(inf, owner, id)| {
            let can_warm = owner.faction != FactionId::PLAYER && Some(id.0) != seat;
            (inf.net(), can_warm)
        });
        let (sphere, warming) = sphere_counts(data, towns);
        s += &format!(" | Sphere {sphere}, warming {warming}");
    }
    s += &format!(" | Overlay: {} (O)", overlay.mode.name());
    if let Some(legend) = overlay.mode.legend() {
        s += &format!(": {legend}");
    }
    text.0 = s;
}

/// Keeps one button per waiting offer.
#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn offer_buttons(
    mut commands: Commands,
    offers: Option<Res<Offers>>,
    sent: Res<SentOffers>,
    tick: Option<Res<SimTick>>,
    names: Option<Res<NameTable>>,
    settlements: Query<(&SpawnId, &Settlement)>,
    row: Query<Entity, With<OfferRow>>,
    epoch: Res<game_render::WorldEpoch>,
    mut shown: Local<Option<Vec<u64>>>,
) {
    let _span = info_span!("game_ui::run_bar::offer_buttons").entered();
    if epoch.is_changed() {
        // Offer ids restart per run: redraw the buttons.
        *shown = None;
    }
    let (Some(offers), Some(names), Ok(row)) = (offers, names, row.single()) else {
        return;
    };
    let tick = tick.map_or(0, |t| t.0);
    let waiting: Vec<&sim_core::Offer> = offers
        .for_faction(FactionId::PLAYER)
        .filter(|o| !sent.hides(o.id, tick))
        .collect();
    let ids: Vec<u64> = waiting.iter().map(|o| o.id).collect();
    if shown.as_ref() == Some(&ids) {
        return;
    }
    *shown = Some(ids);
    let place = |id: u64| {
        settlements
            .iter()
            .find(|(s, _)| s.0 == id)
            .map_or_else(|| "?".into(), |(_, s)| names.get(s.name).to_string())
    };
    commands.entity(row).despawn_children();
    commands.entity(row).with_children(|r| {
        if waiting.is_empty() {
            r.spawn((
                Text::new("No offers waiting"),
                style::font(13.0),
                TextColor(style::DIM),
            ));
        }
        for o in waiting {
            r.spawn((
                Button,
                OfferButton(o.id),
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(style::BUTTON_ACTIVE),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(offer_title(o, &place)),
                    style::font(13.0),
                    TextColor(style::TEXT),
                ));
            });
        }
    });
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn bar_buttons(
    offers: Query<(&Interaction, &OfferButton), Changed<Interaction>>,
    buttons: Query<(&Interaction, &BarButton), Changed<Interaction>>,
    time_real: Res<Time<Real>>,
    mut open: ResMut<OpenOffer>,
    mut ff: ResMut<FastForward>,
    mut time: ResMut<Time<Virtual>>,
    mut abandon: Query<&mut Text, With<AbandonText>>,
    mut armed_at: Local<Option<f32>>,
    mut out: MessageWriter<sim_core::SimCommand>,
) {
    let _span = info_span!("game_ui::run_bar::bar_buttons").entered();
    for (interaction, b) in &offers {
        if *interaction == Interaction::Pressed {
            open.offer = Some(b.0);
        }
    }
    let now = time_real.elapsed_secs();
    // An armed abandon disarms after a few seconds.
    if armed_at.is_some_and(|t| now - t > 6.0) {
        *armed_at = None;
        if let Ok(mut t) = abandon.single_mut() {
            t.0 = "Abandon run".into();
        }
    }
    for (interaction, b) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match b {
            BarButton::FastForward => {
                ff.0 = true;
                time.unpause();
                time.set_relative_speed(crate::time_bar::SPEEDS[3]);
            }
            BarButton::Abandon => {
                if armed_at.take().is_some() {
                    crate::army::send(&mut out, CommandKind::AbandonRun);
                    // The clock must run for the sim to take the command.
                    time.unpause();
                    if let Ok(mut t) = abandon.single_mut() {
                        t.0 = "Abandon run".into();
                    }
                } else {
                    *armed_at = Some(now);
                    if let Ok(mut t) = abandon.single_mut() {
                        t.0 = "Really abandon?".into();
                    }
                }
            }
            BarButton::Later => open.offer = None,
        }
    }
}

/// The open offer's cards, centred on screen; closes once the offer is
/// answered or gone.
#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn draw_card_popup(
    mut commands: Commands,
    mut open: ResMut<OpenOffer>,
    offers: Option<Res<Offers>>,
    sent: Res<SentOffers>,
    tick: Option<Res<SimTick>>,
    progression: Option<Res<Progression>>,
    names: Option<Res<NameTable>>,
    settlements: Query<(&SpawnId, &Settlement)>,
) {
    let _span = info_span!("game_ui::run_bar::draw_card_popup").entered();
    let (Some(offers), Some(progression), Some(names)) = (offers, progression, names) else {
        return;
    };
    let tick = tick.map_or(0, |t| t.0);
    let offer = open
        .offer
        .filter(|id| !sent.hides(*id, tick))
        .and_then(|id| offers.get(id).cloned());
    if offer.is_none() && open.offer.is_some() {
        open.offer = None;
    }
    if !open.is_changed() {
        return;
    }
    if let Some(root) = open.root.take() {
        commands.entity(root).despawn();
    }
    let Some(offer) = offer else { return };
    let place = |id: u64| {
        settlements
            .iter()
            .find(|(s, _)| s.0 == id)
            .map_or_else(|| "?".into(), |(_, s)| names.get(s.name).to_string())
    };
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(30.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-340.0)),
                width: Val::Px(680.0),
                padding: UiRect::all(Val::Px(14.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(style::PANEL),
            Interaction::None,
            GlobalZIndex(15),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(offer_title(&offer, &place)),
                style::font(20.0),
                TextColor(style::GOLD),
            ));
            spawn_cards(p, &offer, &progression, &place);
            style::button(p, BarButton::Later, "Later", 13.0);
        })
        .id();
    open.root = Some(root);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UI-12: the run bar's sphere counts the towns inside the sphere
    /// (yours included) and, apart from them, the towns warming to you:
    /// never your own (a fresh, restless holding at a net of 0.3 is yours
    /// already) nor the rival's seat.
    #[test]
    fn the_sphere_counts_towns_by_net() {
        let d = InfluenceData::default();
        assert_eq!((d.sphere, d.warming), (0.5, 0.2));
        assert_eq!(sphere_counts(&d, []), (0, 0));
        let free = |nets: &[f32]| nets.iter().map(|&n| (n, true)).collect::<Vec<_>>();
        assert_eq!(
            sphere_counts(&d, free(&[0.9, 0.5, 0.49, 0.2, 0.19, 0.0, -0.4, -0.9])),
            (2, 2),
            "0.5 is inside, 0.49 only warming, 0.2 warming, 0.19 not"
        );
        // A rival-leaning town is neither.
        assert_eq!(sphere_counts(&d, free(&[-0.7, -0.5, -0.2])), (0, 0));
        // Your own towns (and the seat) count inside the sphere, never as
        // warming.
        assert_eq!(
            sphere_counts(&d, [(0.3, false), (0.6, false), (0.3, true)]),
            (1, 1)
        );
        // The lines come from the data, not from here.
        let tight = InfluenceData {
            sphere: 0.8,
            warming: 0.6,
            ..d
        };
        assert_eq!(sphere_counts(&tight, free(&[0.85, 0.7, 0.5])), (1, 1));
    }

    #[test]
    fn spans_read_in_hours_and_days() {
        let cal = Calendar::default_data();
        let h = cal.ticks_per_hour();
        assert_eq!(span(h * 5, &cal), "5h");
        assert_eq!(span(h * 27, &cal), "1d 3h");
        assert_eq!(span(1, &cal), "1h");
    }
}
