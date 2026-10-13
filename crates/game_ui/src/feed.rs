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

use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;
use game_render::{CameraRig, WorldEpoch};
use sim_agents::{LoyaltyTerm, LoyaltyTerms, Owner, SettlementLoyalty, Villagers};
use sim_core::{
    Calendar, ChronicleEntry, EventFeed, FactionId, FeedEntry, NameTable, Offers, RunState,
    SimEvent, SimEventKind, SimTick, SpawnId,
};
use sim_military::{Army, DirectorData, MilitaryLog, Progression, Tenure};
use sim_world::Settlement;

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
                    loyalty_holds,
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

/// GP-34 (M4a): the day's tribute from your lands as one Dawn line, not a
/// line a captain: `(food, arms, men)` per receiving band.
fn dawn_line(day: u64, bands: &[(u64, u64, i32)]) -> Option<String> {
    if bands.is_empty() {
        return None;
    }
    let (food, arms, men) = bands.iter().fold((0, 0, 0), |a, b| {
        (a.0 + b.0, a.1 + b.1, a.2 + i64::from(b.2))
    });
    let to = if bands.len() == 1 {
        "1 band".to_string()
    } else {
        format!("{} bands", bands.len())
    };
    Some(format!(
        "Day {day}, dawn: your lands sent {food} food, {arms} arms and {men} men to {to}."
    ))
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
    // Tribute events, grouped into one Dawn line per day below.
    let mut tributes: Vec<FeedEntry> = Vec::new();
    {
        let sent = world.resource::<SentOffers>();
        let tick = world.get_resource::<SimTick>().map_or(0, |t| t.0);
        let offers = world.get_resource::<Offers>();
        for entry in &entries {
            let e = &entry.event;
            // Pause on an offer only while it still waits for you (the start
            // boon is posted before tick 0 and answered on the new-game screen).
            let waiting = e.kind == SimEventKind::OfferPosted
                && e.faction == FactionId::PLAYER
                && !sent.hides(e.subject, tick)
                && offers.is_some_and(|o| o.get(e.subject).is_some());
            let counts = e.kind != SimEventKind::OfferPosted || waiting;
            if !quiet && run.in_run() && counts && pause_on.contains(&e.kind) {
                pause = true;
            }
            if !quiet && ff && e.kind.notable() {
                pause = true;
            }
            if e.kind == SimEventKind::Tribute && e.faction == FactionId::PLAYER {
                tributes.push(*entry);
            } else if worth_a_line(e, waiting) {
                told.push(ChronicleEntry {
                    tick: entry.tick,
                    event: *e,
                });
            }
        }
    }
    let mut texts = sim_military::describe_entries(world, &told);
    // GP-33 (M4a step 6): a vow gives the hour it comes due, not a span.
    let cal = world.resource::<Calendar>();
    for (e, text) in told.iter().zip(texts.iter_mut()) {
        if e.event.kind == SimEventKind::ReprisalVowed {
            let due = crate::fallout::when(e.event.other, cal);
            *text = crate::fallout::vow_feed_text(text, e.event.amount, &due);
        }
    }
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
    type Day = (u64, u64, Vec<(u64, u64, i32)>, IVec2);
    let mut days: Vec<Day> = Vec::new();
    for t in &tributes {
        let day = cal.at(t.tick).day + 1;
        let band = (
            t.event.other & 0xffff_ffff,
            t.event.other >> 32,
            t.event.amount,
        );
        match days.last_mut() {
            Some(d) if d.0 == day => d.2.push(band),
            _ => days.push((day, t.tick, vec![band], t.event.at)),
        }
    }
    for (day, tick, bands, at) in days {
        if let Some(text) = dawn_line(day, &bands) {
            new.push((
                tick,
                0,
                Line {
                    text,
                    at: (at != IVec2::ZERO).then_some(at),
                    log: false,
                },
            ));
        }
    }
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

/// GP-32 (M4a step 5): the term that holds a town above the revolt `line`,
/// if one does: Awe or Bond, positive, whose removal alone would leave the
/// town's mean loyalty under the line while the mean itself is not. The
/// larger wins.
fn holding_term(mean: f32, terms: &LoyaltyTerms, line: f32) -> Option<(LoyaltyTerm, f32)> {
    if mean < line {
        return None;
    }
    [LoyaltyTerm::Awe, LoyaltyTerm::Bond]
        .into_iter()
        .map(|t| (t, terms.get(t)))
        .filter(|(_, v)| *v > 0.0 && mean - v < line)
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// "Gorsefield stays loyal: awe of your army, +0.10": what holds a town of
/// yours above the revolt line. `kept` is the promises of defence kept
/// there, for the bond's wording.
fn holds_line(town: &str, term: LoyaltyTerm, v: f32, kept: u8) -> String {
    let why = match term {
        LoyaltyTerm::Bond if kept > 0 => "the bond you kept",
        LoyaltyTerm::Bond => "the bond you swore",
        _ => "awe of your army",
    };
    format!("{town} stays loyal: {why}, {v:+.2}")
}

/// Once a town a day at most, a line when Awe or Bond holds a town of yours
/// above the revolt line ([`holding_term`]). Reads only; what it has told
/// (a town and the day) is UI-side, forgotten with the world.
#[allow(clippy::type_complexity)] // Bevy system parameters.
fn loyalty_holds(
    mut lines: ResMut<FeedLines>,
    (calendar, tick, villagers, names, epoch): (
        Option<Res<Calendar>>,
        Option<Res<SimTick>>,
        Option<Res<Villagers>>,
        Option<Res<NameTable>>,
        Option<Res<WorldEpoch>>,
    ),
    towns: Query<(
        &SpawnId,
        &Settlement,
        &Owner,
        &SettlementLoyalty,
        &LoyaltyTerms,
        Option<&Tenure>,
    )>,
    mut told: Local<(Option<u32>, BTreeMap<u64, u64>)>,
) {
    let _span = info_span!("game_ui::feed::loyalty_holds").entered();
    let (Some(cal), Some(tick), Some(vd), Some(names)) = (calendar, tick, villagers, names) else {
        return;
    };
    let epoch = epoch.map_or(0, |e| e.0);
    if told.0 != Some(epoch) {
        *told = (Some(epoch), BTreeMap::new());
    }
    let day = cal.at(tick.0).day;
    let line = vd.data.loyalty.revolt_threshold;
    let mut found: Vec<(u64, Line)> = towns
        .iter()
        .filter(|(id, _, owner, ..)| {
            owner.faction == FactionId::PLAYER && told.1.get(&id.0) != Some(&day)
        })
        .filter_map(|(id, s, _, loyalty, terms, tenure)| {
            if loyalty.residents == 0 {
                return None;
            }
            let (term, v) = holding_term(loyalty.mean, terms, line)?;
            let kept = tenure.map_or(0, |t| t.kept);
            Some((
                id.0,
                Line {
                    text: holds_line(names.get(s.name), term, v, kept),
                    at: Some(s.center),
                    log: false,
                },
            ))
        })
        .collect();
    if found.is_empty() {
        return;
    }
    found.sort_by_key(|(id, _)| *id);
    for (id, l) in found {
        told.1.insert(id, day);
        lines.0.push_back(l);
    }
    while lines.0.len() > LINES {
        lines.0.pop_front();
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

    /// GP-34: a day's tribute is one Dawn line.
    #[test]
    fn a_days_tribute_is_one_dawn_line() {
        assert_eq!(dawn_line(7, &[]), None);
        assert_eq!(
            dawn_line(7, &[(24, 6, 3), (10, 0, 2)]).unwrap(),
            "Day 7, dawn: your lands sent 34 food, 6 arms and 5 men to 2 bands."
        );
        assert!(dawn_line(7, &[(1, 0, 0)]).unwrap().ends_with("to 1 band."));
    }

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

    /// UI-05 (ADR 0018): every food dealing's line about your warband,
    /// what it brought or why it brought nothing, joins the feed.
    #[test]
    fn food_dealing_lines_join_the_feed() {
        for shown in [
            "Northwell had no food to spare for Your warband (it holds 0.4 days of food and keeps 0.5)",
            "Your warband's baggage is full (4.0 of 4 days): nothing gathered at Ashford",
            "Your warband took food from Ashford (+1.0 days)",
            "Your warband found nothing to plunder at Ashford (its storehouse is empty)",
            "Ashford shared food with Your warband (+2.1 days)",
            "Your warband has no goods to trade for food at Ashford",
            "Ashford supplied Your warband (+1.5 days)",
            "Your warband bought food from Ashford (+0.7 days)",
        ] {
            assert!(log_line_shown(shown, ME), "{shown}");
        }
        assert!(!log_line_shown(
            "Ashford shared food with Rival (+2.1 days)",
            ME
        ));
    }

    fn terms(awe: f32, bond: f32) -> LoyaltyTerms {
        let mut t = LoyaltyTerms::default();
        t.set(LoyaltyTerm::Awe, awe);
        t.set(LoyaltyTerm::Bond, bond);
        t
    }

    /// GP-32: awe or the bond is named when it alone keeps a town above
    /// the revolt line, and not otherwise.
    #[test]
    fn awe_or_bond_is_named_when_it_holds_a_town() {
        let line = 0.35;
        // 0.40 with +0.10 awe: without it 0.30, under the line.
        assert_eq!(
            holding_term(0.40, &terms(0.10, 0.0), line),
            Some((LoyaltyTerm::Awe, 0.10))
        );
        assert_eq!(
            holding_term(0.40, &terms(0.0, 0.08), line),
            Some((LoyaltyTerm::Bond, 0.08))
        );
        // Comfortably above: nothing to say.
        assert_eq!(holding_term(0.70, &terms(0.10, 0.0), line), None);
        // Already under the line: it is not holding.
        assert_eq!(holding_term(0.30, &terms(0.10, 0.0), line), None);
        // No term, a negative term (a broken bond): nothing.
        assert_eq!(holding_term(0.40, &terms(0.0, 0.0), line), None);
        assert_eq!(holding_term(0.40, &terms(0.0, -0.10), line), None);
        // Both hold it: the larger is named.
        assert_eq!(
            holding_term(0.40, &terms(0.06, 0.09), line).map(|t| t.0),
            Some(LoyaltyTerm::Bond)
        );
    }

    #[test]
    fn the_holding_line_names_the_term() {
        assert_eq!(
            holds_line("Gorsefield", LoyaltyTerm::Awe, 0.1, 0),
            "Gorsefield stays loyal: awe of your army, +0.10"
        );
        assert_eq!(
            holds_line("Woldbrook", LoyaltyTerm::Bond, 0.08, 3),
            "Woldbrook stays loyal: the bond you kept, +0.08"
        );
        assert_eq!(
            holds_line("Woldbrook", LoyaltyTerm::Bond, 0.05, 0),
            "Woldbrook stays loyal: the bond you swore, +0.05"
        );
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
