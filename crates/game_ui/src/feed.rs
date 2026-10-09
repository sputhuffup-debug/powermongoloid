//! The event feed (UI-05, bottom-left): the latest `EventFeed` entries,
//! read by `seq` cursor and told in the chronicle's words
//! (`sim_military::describe_entries`: the day, who did what, why, and a
//! battle's report), merged by tick with the military log's lines about
//! your warband (`MilitaryLog::since`, M3's log folded in). Each entry is
//! described once, when it arrives, and the line is kept. Clicking a line
//! moves the camera to the event.
//!
//! In a run the feed also pauses the game: on the kinds the director's
//! personality lists in `auto_pause` (director.ron), and on the next
//! notable event while fast-forwarding ([`FastForward`], the run bar's
//! ">>" button; GP-16).
//!
//! When the world is replaced (`WorldEpoch`: a load or a new run) the
//! feed starts again from the loaded feed's last lines, without pausing
//! on them, and fast-forward stops.

use std::collections::VecDeque;

use bevy::prelude::*;
use game_render::{CameraRig, WorldEpoch};
use sim_core::{
    Calendar, ChronicleEntry, EventFeed, FactionId, FeedEntry, Offers, RunState, SimEvent,
    SimEventKind,
};
use sim_military::{Army, DirectorData, MilitaryLog, Progression};

use crate::cards::SentOffers;
use crate::style;
use crate::{UiSet, playing};

pub struct FeedPlugin;

impl Plugin for FeedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FastForward>()
            .init_resource::<FeedTargets>()
            .init_resource::<FeedLines>()
            .add_systems(Startup, spawn_feed)
            .add_systems(
                Update,
                (
                    read_feed,
                    show_feed.run_if(resource_changed::<FeedLines>),
                    click_feed.run_if(playing),
                )
                    .chain()
                    .in_set(UiSet::Hud),
            );
    }
}

/// Fast-forward (GP-16): run at the top speed until the next notable event,
/// then pause.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FastForward(pub bool);

/// Lines shown.
const LINES: usize = 8;

#[derive(Component)]
struct FeedRoot;

/// One line: where clicking it moves the camera.
#[derive(Component)]
struct FeedLine(usize);

/// One feed line: its text, where clicking it goes, and whether it came
/// from the military log (drawn dimmer).
#[derive(Debug, Clone, PartialEq)]
struct Line {
    text: String,
    at: Option<IVec2>,
    log: bool,
}

/// The feed's lines, oldest first (UI-side only).
#[derive(Resource, Default)]
struct FeedLines(VecDeque<Line>);

/// Where `read_feed` has read up to.
#[derive(Default)]
struct Cursors {
    /// The `WorldEpoch` read from (`None` before the first frame).
    epoch: Option<u32>,
    /// The next `EventFeed` seq to read.
    events: u64,
    /// The next `MilitaryLog` seq to read.
    log: u64,
}

fn spawn_feed(mut commands: Commands) {
    commands
        .spawn((
            FeedRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(10.0),
                width: Val::Px(520.0),
                padding: UiRect::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(style::PANEL),
            Interaction::None,
            Visibility::Hidden,
        ))
        .with_children(|p| {
            for i in 0..LINES {
                p.spawn((
                    Button,
                    FeedLine(i),
                    Node::default(),
                    Text::new(""),
                    style::font(12.0),
                    TextColor(style::TEXT),
                ));
            }
        });
}

/// Whether an event gets a line (it may still pause the game): offers only
/// while they wait for you, and only your own offers answered and soldiers
/// ranked up (a big battle elsewhere ranks up whole armies).
fn worth_a_line(e: &SimEvent, waiting_offer: bool) -> bool {
    match e.kind {
        SimEventKind::OfferPosted => waiting_offer,
        SimEventKind::OfferChosen | SimEventKind::VeteranRank => e.faction == FactionId::PLAYER,
        _ => true,
    }
}

/// Military log lines the feed shows: those naming your warband
/// (`player`, army.ron's faction name), except the ones an event already
/// tells (offers chosen, the run abandoned, the champion's rank or fall,
/// requests answered, invitations, revolts, drifters joining, and `took
/// <place>`: conquests and monuments). Logistics (`took food from ...`,
/// `took 5 arms from ...`), refused orders, recruits, desertions, risings
/// and attacks stay.
fn log_line_shown(text: &str, player: &str) -> bool {
    const TOLD: [&str; 8] = [
        " chose ",
        " abandoned the run",
        ": the champion ",
        " answered ",
        " invites ",
        " invited ",
        " threw off ",
        " drifted in to join ",
    ];
    if player.is_empty() || !text.contains(player) || TOLD.iter().any(|t| text.contains(t)) {
        return false;
    }
    match text
        .strip_prefix(player)
        .and_then(|r| r.strip_prefix(" took "))
    {
        Some(rest) => rest.contains(" from "),
        None => true,
    }
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// Reads new `EventFeed` entries and `MilitaryLog` lines, describes them
/// once and merges them by tick into [`FeedLines`]; pauses the game on
/// the director's `auto_pause` kinds and ends fast-forward on a notable
/// event. Exclusive: the chronicle's sentences read names from the world.
fn read_feed(world: &mut World, mut cur: Local<Cursors>) {
    let _span = info_span!("game_ui::feed::read_feed").entered();
    let ready = world.contains_resource::<EventFeed>()
        && world.contains_resource::<MilitaryLog>()
        && world.contains_resource::<Army>()
        && world.contains_resource::<Progression>()
        && world.contains_resource::<Calendar>()
        && world.contains_resource::<sim_core::NameTable>()
        && world.contains_resource::<sim_core::Chronicle>();
    if !ready {
        return;
    }
    let epoch = world.get_resource::<WorldEpoch>().map_or(0, |e| e.0);
    let next_event = world.resource::<EventFeed>().next_seq();
    let next_log = world.resource::<MilitaryLog>().seq;
    let mut quiet = false;
    if cur.epoch != Some(epoch) {
        if cur.epoch.is_some() {
            // A new world: show its last lines, but don't pause on them.
            quiet = true;
            cur.events = next_event.saturating_sub(LINES as u64);
            cur.log = next_log.saturating_sub(LINES as u64);
            world.resource_mut::<FeedLines>().0.clear();
            world.resource_mut::<FastForward>().0 = false;
        }
        cur.epoch = Some(epoch);
    }
    if next_event <= cur.events && next_log <= cur.log {
        return;
    }
    let entries: Vec<FeedEntry> = world
        .resource::<EventFeed>()
        .since(cur.events)
        .copied()
        .collect();
    let player = world
        .resource::<Army>()
        .faction_name(FactionId::PLAYER)
        .to_string();
    let logged: Vec<(u64, String)> = world
        .resource::<MilitaryLog>()
        .since(cur.log)
        .filter(|(_, _, text)| log_line_shown(text, &player))
        .map(|(_, tick, text)| (tick, text.to_string()))
        .collect();
    cur.events = next_event;
    cur.log = next_log;

    let run = world
        .get_resource::<RunState>()
        .copied()
        .unwrap_or_default();
    let pause_on: Vec<SimEventKind> = run
        .config
        .and_then(|c| {
            world
                .get_resource::<DirectorData>()?
                .personality(c.difficulty)
                .map(|p| p.auto_pause.clone())
        })
        .unwrap_or_default();
    let ff = world.resource::<FastForward>().0;
    let mut pause = false;
    let mut told: Vec<ChronicleEntry> = Vec::new();
    {
        let sent = &world.resource::<SentOffers>().0;
        let offers = world.get_resource::<Offers>();
        for entry in &entries {
            let e = &entry.event;
            // Pause on an offer only while it still waits for you (the start
            // boon is posted before tick 0 and answered on the new-game screen).
            let waiting = e.kind == SimEventKind::OfferPosted
                && e.faction == FactionId::PLAYER
                && !sent.contains(&e.subject)
                && offers.is_some_and(|o| o.get(e.subject).is_some());
            let counts = e.kind != SimEventKind::OfferPosted || waiting;
            if !quiet && run.in_run() && counts && pause_on.contains(&e.kind) {
                pause = true;
            }
            if !quiet && ff && e.kind.notable() {
                pause = true;
            }
            if worth_a_line(e, waiting) {
                told.push(ChronicleEntry {
                    tick: entry.tick,
                    event: *e,
                });
            }
        }
    }
    let texts = sim_military::describe_entries(world, &told);
    // Merge by tick; within a tick, events before the log's lines.
    let mut new: Vec<(u64, u8, Line)> = told
        .iter()
        .zip(texts)
        .map(|(e, text)| {
            let at = (e.event.at != IVec2::ZERO).then_some(e.event.at);
            (
                e.tick,
                0,
                Line {
                    text,
                    at,
                    log: false,
                },
            )
        })
        .collect();
    let cal = world.resource::<Calendar>();
    new.extend(logged.into_iter().map(|(tick, text)| {
        let day = cal.at(tick).day + 1;
        let text = format!("Day {day}: {}.", upper_first(&text));
        (
            tick,
            1,
            Line {
                text,
                at: None,
                log: true,
            },
        )
    }));
    new.sort_by_key(|(tick, order, _)| (*tick, *order));
    if !new.is_empty() {
        let mut lines = world.resource_mut::<FeedLines>();
        lines.0.extend(new.into_iter().map(|(.., l)| l));
        while lines.0.len() > LINES {
            lines.0.pop_front();
        }
    }
    if pause {
        let mut time = world.resource_mut::<Time<Virtual>>();
        if !time.is_paused() {
            info!("feed: paused on an event");
            time.pause();
        }
        world.resource_mut::<FastForward>().0 = false;
    }
}

/// Draws [`FeedLines`], newest first.
fn show_feed(
    lines: Res<FeedLines>,
    mut targets: ResMut<FeedTargets>,
    mut root: Query<&mut Visibility, With<FeedRoot>>,
    mut texts: Query<(&FeedLine, &mut Text, &mut TextColor)>,
) {
    let _span = info_span!("game_ui::feed::show_feed").entered();
    if let Ok(mut v) = root.single_mut() {
        v.set_if_neq(if lines.0.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    let shown: Vec<&Line> = lines.0.iter().rev().collect();
    for (line, mut text, mut color) in &mut texts {
        let l = shown.get(line.0);
        text.0 = l.map_or(String::new(), |l| l.text.clone());
        color.0 = if l.is_some_and(|l| l.log) {
            style::DIM
        } else {
            style::TEXT
        };
    }
    targets.0 = shown.iter().map(|l| l.at).collect();
}

/// Where each shown line points, newest first (UI-side only).
#[derive(Resource, Default)]
struct FeedTargets(Vec<Option<IVec2>>);

fn click_feed(
    lines: Query<(&Interaction, &FeedLine), Changed<Interaction>>,
    targets: Res<FeedTargets>,
    mut rigs: Query<&mut CameraRig>,
) {
    let _span = info_span!("game_ui::feed::click_feed").entered();
    for (interaction, line) in &lines {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let at = targets.0.get(line.0).copied().flatten();
        if let (Some(at), Ok(mut rig)) = (at, rigs.single_mut()) {
            rig.target_focus = at.as_vec2() + 0.5;
            rig.target_distance = rig.target_distance.min(140.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "Your warband";

    /// UI-05: the military log's lines about your armies join the feed,
    /// except those an event already tells.
    #[test]
    fn log_lines_about_your_warband_join_the_feed_once() {
        for shown in [
            "Your warband bought food from Ashford",
            "Your warband took food from Ashford",
            "Your warband took 5 arms from Ashford",
            "Your warband: order refused (no such settlement)",
            "Your warband: 3 deserted",
            "Ashford takes up arms against Your warband",
            "Ashford rises against Your warband",
            "Your warband broke the defenders of Ashford",
        ] {
            assert!(log_line_shown(shown, ME), "{shown}");
        }
        for hidden in [
            "Your warband took Ashford",
            "Your warband took Pellcombe Well",
            "Your warband chose Well fed",
            "Your warband: the champion reached rank 2",
            "Your warband answered Ashford: bring food",
            "Ashford invites Your warband to rule it",
            "Ashford threw off Your warband and is free",
            "2 of Ashford drifted in to join Your warband",
            "Rival bought food from Ashford",
            "The rival levied 12 men at its seat",
        ] {
            assert!(!log_line_shown(hidden, ME), "{hidden}");
        }
        assert!(!log_line_shown("anything", ""));
    }

    #[test]
    fn other_factions_offers_and_ranks_get_no_line() {
        let mine = SimEvent::new(SimEventKind::VeteranRank, FactionId::PLAYER);
        let theirs = SimEvent::new(SimEventKind::VeteranRank, FactionId::RIVAL);
        assert!(worth_a_line(&mine, false));
        assert!(!worth_a_line(&theirs, false));
        let offer = SimEvent::new(SimEventKind::OfferPosted, FactionId::PLAYER);
        assert!(worth_a_line(&offer, true));
        assert!(!worth_a_line(&offer, false));
        let battle = SimEvent::new(SimEventKind::Battle, FactionId::RIVAL);
        assert!(worth_a_line(&battle, false));
    }
}
