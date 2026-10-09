//! The event feed (UI-05, bottom-left): the latest `EventFeed` entries,
//! read by `seq` cursor and written as plain sentences. Clicking a line
//! moves the camera there.
//!
//! In a run the feed also pauses the game: on the kinds the director's
//! personality lists in `auto_pause` (director.ron), and on the next
//! notable event while fast-forwarding ([`FastForward`], the run bar's
//! ">>" button; GP-16).
//!
//! TODO(M4 step 9): swap [`describe`] for `sim_military::chronicle::describe`
//! and merge `MilitaryLog` lines by their `seq` once step 9 adds it.

use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;
use game_render::CameraRig;
use sim_core::{
    Calendar, Cause, EventFeed, FactionId, NameTable, RunState, SimEvent, SimEventKind, SpawnId,
};
use sim_military::{Army, DirectorData, Progression};
use sim_world::{Monument, MonumentKind, Settlement};

use crate::style;
use crate::{UiSet, playing};

pub struct FeedPlugin;

impl Plugin for FeedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FastForward>()
            .init_resource::<FeedTargets>()
            .add_systems(Startup, spawn_feed)
            .add_systems(
                Update,
                (read_feed, (click_feed).run_if(playing))
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

#[derive(Default)]
struct FeedState {
    cursor: u64,
    lines: VecDeque<(String, IVec2)>,
}

fn spawn_feed(mut commands: Commands) {
    commands
        .spawn((
            FeedRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                bottom: Val::Px(10.0),
                width: Val::Px(420.0),
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

/// Who a faction is, from the player's side.
fn who(f: FactionId, army: &Army) -> String {
    if f == FactionId::PLAYER {
        "You".into()
    } else {
        format!("The {}", army.faction_name(f).to_lowercase())
    }
}

/// Whose: "Your" or "The rival's".
fn whose(f: FactionId, army: &Army) -> String {
    if f == FactionId::PLAYER {
        "Your".into()
    } else {
        format!("{}'s", who(f, army))
    }
}

/// Plain words for a cause.
pub fn cause_text(c: Cause, army: &Army) -> String {
    match c {
        Cause::None => "no cause".into(),
        Cause::Player => "by your choice".into(),
        Cause::Request { .. } => "a request".into(),
        Cause::Conquest { .. } => "conquest".into(),
        Cause::Invitation => "an invitation".into(),
        Cause::Starvation => "starvation".into(),
        Cause::Plunder => "plunder".into(),
        Cause::Revolt => "a revolt".into(),
        Cause::Raid { .. } => "a raid".into(),
        Cause::Hold => "held long enough".into(),
        Cause::Boon { .. } => "a boon".into(),
        Cause::Death { faction } => {
            format!("killed by {}", army.faction_name(faction).to_lowercase())
        }
    }
}

/// One feed line for an event. `name` names a settlement or monument by
/// SpawnId.
pub fn describe(
    e: &SimEvent,
    army: &Army,
    progression: &Progression,
    name: &dyn Fn(u64) -> String,
) -> Option<String> {
    let f = who(e.faction, army);
    let request = |tag: i32| match tag {
        0 => "drive off raiders",
        1 => "bring food",
        _ => "clear the camp",
    };
    let kind = |k: i32| {
        MonumentKind::ALL
            .get(k as usize)
            .map_or("monument", |k| k.name())
    };
    Some(match e.kind {
        SimEventKind::RunStarted => "The run begins.".into(),
        SimEventKind::OfferPosted if e.faction == FactionId::PLAYER => {
            "A new offer awaits you (run bar).".into()
        }
        SimEventKind::OfferPosted | SimEventKind::OfferChosen => return None,
        SimEventKind::MonumentTaken => {
            format!("{f} took the {} {}.", kind(e.amount), name(e.subject))
        }
        SimEventKind::MonumentLost => format!("{f} lost {}.", name(e.subject)),
        SimEventKind::RequestPosted => {
            format!("{} asks you to {}.", name(e.other), request(e.amount))
        }
        SimEventKind::RequestDone => format!("You answered {}'s request.", name(e.other)),
        SimEventKind::RequestFailed => format!("{}'s request went unanswered.", name(e.other)),
        SimEventKind::Conquest => format!("{f} took {} ({} people).", name(e.subject), e.amount),
        SimEventKind::Invited => format!("{} invited {} in.", name(e.subject), f.to_lowercase()),
        SimEventKind::Revolt => format!("{} rose against {}.", name(e.subject), f.to_lowercase()),
        SimEventKind::RankUp => format!(
            "{} champion reached rank {}.",
            whose(e.faction, army),
            e.amount
        ),
        SimEventKind::TraitGained => format!(
            "{f} gained a trait: {}.",
            progression
                .trait_data(e.amount as u16)
                .map_or("?", |t| t.name.as_str())
        ),
        SimEventKind::VeteranRank => format!(
            "A soldier became {}.",
            army.data.veterans.name(e.amount as u8).to_lowercase()
        ),
        SimEventKind::ChampionFell => format!(
            "{} champion fell ({}).",
            whose(e.faction, army),
            cause_text(e.cause, army)
        ),
        SimEventKind::Succession => format!("{f} raised a new champion."),
        SimEventKind::RivalLevy => format!("The rival levied {} men.", e.amount),
        SimEventKind::RivalRaid => format!("The rival sent {} raiders out.", e.amount),
        SimEventKind::RaidersArrived => {
            if e.other == 0 {
                format!("{} raiders appeared.", e.amount)
            } else {
                format!(
                    "{} raiders appeared, bound for {}.",
                    e.amount,
                    name(e.other)
                )
            }
        }
        SimEventKind::Battle => format!("{f} fought a battle: {} dead.", e.amount),
        SimEventKind::DriftersJoined => {
            format!("{} drifters from {} joined you.", e.amount, name(e.other))
        }
        SimEventKind::RunEnded => "The run is over.".into(),
    })
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn read_feed(
    feed: Option<Res<EventFeed>>,
    army: Option<Res<Army>>,
    progression: Option<Res<Progression>>,
    director: Option<Res<DirectorData>>,
    run: Option<Res<RunState>>,
    calendar: Option<Res<Calendar>>,
    names: Option<Res<NameTable>>,
    places: Query<(&SpawnId, Option<&Settlement>, Option<&Monument>)>,
    mut ff: ResMut<FastForward>,
    mut time: ResMut<Time<Virtual>>,
    mut state: Local<FeedState>,
    mut root: Query<&mut Visibility, With<FeedRoot>>,
    mut lines: Query<(&FeedLine, &mut Text)>,
    mut targets: ResMut<FeedTargets>,
) {
    let _span = info_span!("game_ui::feed::read_feed").entered();
    let (
        Some(feed),
        Some(army),
        Some(progression),
        Some(director),
        Some(run),
        Some(cal),
        Some(names),
    ) = (feed, army, progression, director, run, calendar, names)
    else {
        return;
    };
    if feed.next_seq() <= state.cursor {
        return;
    }
    let mut by_id: BTreeMap<u64, String> = BTreeMap::new();
    for (id, s, m) in &places {
        if let Some(s) = s {
            by_id.insert(id.0, names.get(s.name).to_string());
        } else if let Some(m) = m {
            by_id.insert(id.0, names.get(m.name).to_string());
        }
    }
    let name = |id: u64| {
        by_id
            .get(&id)
            .cloned()
            .unwrap_or_else(|| "somewhere".into())
    };
    let pause_on: Vec<SimEventKind> = run
        .config
        .and_then(|c| director.personality(c.difficulty))
        .map(|p| p.auto_pause.clone())
        .unwrap_or_default();
    let mut veterans = 0;
    let mut pause = false;
    for entry in feed.since(state.cursor) {
        let e = &entry.event;
        let mine = e.faction == FactionId::PLAYER || e.kind != SimEventKind::OfferPosted;
        if run.in_run() && mine && pause_on.contains(&e.kind) {
            pause = true;
        }
        if ff.0 && e.kind.notable() {
            pause = true;
        }
        // A big battle ranks up dozens at once: one line for them all.
        if e.kind == SimEventKind::VeteranRank {
            veterans += 1;
            continue;
        }
        if let Some(text) = describe(e, &army, &progression, &name) {
            let day = cal.at(entry.tick).day + 1;
            state.lines.push_back((format!("Day {day}: {text}"), e.at));
        }
    }
    if veterans > 0 {
        state
            .lines
            .push_back((format!("{veterans} soldiers rose in rank."), IVec2::ZERO));
    }
    while state.lines.len() > LINES {
        state.lines.pop_front();
    }
    state.cursor = feed.next_seq();
    if pause && !time.is_paused() {
        info!("feed: paused on an event");
        time.pause();
        ff.0 = false;
    }
    if let Ok(mut v) = root.single_mut() {
        v.set_if_neq(if state.lines.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    let shown: Vec<&(String, IVec2)> = state.lines.iter().rev().collect();
    for (line, mut text) in &mut lines {
        text.0 = shown.get(line.0).map_or(String::new(), |l| l.0.clone());
    }
    targets.0 = shown.iter().map(|l| l.1).collect();
}

/// Where each shown line points, newest first (UI-side only).
#[derive(Resource, Default)]
struct FeedTargets(Vec<IVec2>);

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
        let at = targets.0.get(line.0).copied();
        if let (Some(at), Ok(mut rig)) = (at, rigs.single_mut())
            && at != IVec2::ZERO
        {
            rig.target_focus = at.as_vec2() + 0.5;
            rig.target_distance = rig.target_distance.min(140.0);
        }
    }
}
