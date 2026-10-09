//! M4 run screens (GP-21, GP-27, GP-12, UI-09): the new-game screen (seed,
//! difficulty, the run modifiers drawn, the band, the three start-boon
//! cards, and "Resume" for saved runs) and the end screen (outcome, cause
//! and day; the run summary, its timeline from the chronicle and your
//! battle reports; "New run" and "Same seed again").
//!
//! Both are full-screen nodes above everything else; while one is up the
//! game is not [`crate::playing`], so hotkeys, world clicks and camera
//! input are ignored ([`game_render::InputGate`]).
//!
//! A different seed or difficulty needs a different world: the screen
//! sends [`NewRunRequest`] and `game_app` builds it and loads it in place.
//! After any load (`WorldEpoch`) the screens are decided afresh: the
//! new-game screen for a run whose start boon still waits, the end screen
//! for a finished run, the island otherwise.

use std::path::PathBuf;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use game_render::{InputGate, WorldEpoch};
use sim_core::{
    Calendar, Cause, Chronicle, ChronicleEntry, Difficulty, FactionId, NameTable, OfferKind,
    Offers, RunState, SimEvent, SimEventKind, SimSeed, SimTick,
};
use sim_military::{Army, Progression, RunProgress};
use sim_world::Settlement;

use crate::style;
use crate::{NewRunRequest, SaveRequest, SavedRuns, UiScreen, UiSet};

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Draft>()
            .init_resource::<EndReport>()
            .add_systems(
                Update,
                (choose_screen, gate_input).chain().in_set(UiSet::Screens),
            )
            .add_systems(OnEnter(UiScreen::NewGame), spawn_new_game)
            .add_systems(OnExit(UiScreen::NewGame), despawn::<NewGameRoot>)
            .add_systems(
                OnEnter(UiScreen::Ended),
                (write_end_report, spawn_end).chain(),
            )
            .add_systems(OnExit(UiScreen::Ended), despawn::<EndRoot>)
            .add_systems(
                Update,
                (
                    seed_typing,
                    new_game_buttons,
                    resume_buttons,
                    refresh_new_game,
                )
                    .chain()
                    .run_if(in_state(UiScreen::NewGame))
                    .in_set(UiSet::Hud),
            )
            .add_systems(
                Update,
                (end_buttons, scroll_timeline)
                    .run_if(in_state(UiScreen::Ended))
                    .in_set(UiSet::Hud),
            );
    }
}

/// A seed from the clock (the new-game screen's "Random", and new runs).
/// Not simulation randomness: it only picks which world to build.
pub fn random_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    // SplitMix64, so consecutive clicks give unrelated seeds.
    let mut z = nanos.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    // Short enough to read out and type.
    (z ^ (z >> 31)) % 1_000_000_000
}

/// The seed and difficulty typed on the new-game screen.
#[derive(Resource, Default)]
struct Draft {
    seed: String,
    difficulty: Difficulty,
    /// Filled from the world when the screen opens.
    ready: bool,
}

#[derive(Component)]
struct NewGameRoot;

#[derive(Component)]
struct EndRoot;

#[derive(Component)]
struct SeedText;

/// Shown while the typed seed or difficulty differ from the world's.
#[derive(Component)]
struct RebuildRow;

/// Hidden while [`RebuildRow`] shows: the cards belong to the world shown.
#[derive(Component)]
struct CardsRow;

#[derive(Component, Clone, Copy, PartialEq)]
enum NewGameButton {
    Difficulty(Difficulty),
    Random,
    Build,
}

/// Resumes the saved run in this file (GP-12).
#[derive(Component, Clone, PartialEq)]
struct ResumeButton(PathBuf);

/// The end screen's timeline text (scrolled with the wheel).
#[derive(Component)]
struct TimelineText;

#[derive(Component, Clone, Copy, PartialEq)]
enum EndButton {
    NewRun,
    SameSeed,
}

fn despawn<T: Component>(mut commands: Commands, roots: Query<Entity, With<T>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Opens the new-game screen on a run's first frame (the start boon still
/// waiting, normally at tick 0; the clock paused) and the end screen once
/// the run is over.
#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn choose_screen(
    run: Option<Res<RunState>>,
    offers: Option<Res<Offers>>,
    screen: Res<State<UiScreen>>,
    epoch: Res<WorldEpoch>,
    mut next: ResMut<NextState<UiScreen>>,
    mut time: ResMut<Time<Virtual>>,
    mut draft: ResMut<Draft>,
    mut opened: Local<bool>,
) {
    let _span = info_span!("game_ui::screens::choose_screen").entered();
    if epoch.is_changed() && !epoch.is_added() {
        // A new world (a load or a new run): decide afresh. A screen that
        // is up closes first, so the one chosen next frame is rebuilt.
        *opened = false;
        draft.ready = false;
        if *screen.get() != UiScreen::Playing {
            next.set(UiScreen::Playing);
            return;
        }
    }
    let (Some(run), Some(offers)) = (run, offers) else {
        return;
    };
    if !run.in_run() {
        return;
    }
    if run.outcome.is_some() {
        if *screen.get() != UiScreen::Ended {
            next.set(UiScreen::Ended);
        }
        return;
    }
    if !*opened {
        *opened = true;
        let boon_waiting = offers
            .for_faction(FactionId::PLAYER)
            .any(|o| o.kind == OfferKind::StartBoon);
        if boon_waiting {
            time.pause();
            next.set(UiScreen::NewGame);
        }
    }
}

fn gate_input(screen: Res<State<UiScreen>>, mut gate: ResMut<InputGate>) {
    let blocked = *screen.get() != UiScreen::Playing;
    if gate.blocked != blocked {
        gate.blocked = blocked;
    }
}

/// A full-screen backdrop that swallows clicks.
fn backdrop() -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(12.0),
            padding: UiRect::all(Val::Px(24.0)),
            ..default()
        },
        BackgroundColor(style::SCREEN),
        Interaction::None,
        GlobalZIndex(20),
    )
}

fn line(text: impl Into<String>, px: f32, color: Color) -> impl Bundle {
    (Text::new(text), style::font(px), TextColor(color))
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn spawn_new_game(
    mut commands: Commands,
    seed: Res<SimSeed>,
    run: Res<RunState>,
    offers: Res<Offers>,
    progression: Res<Progression>,
    progress: Res<RunProgress>,
    names: Res<NameTable>,
    (saved, calendar): (Res<SavedRuns>, Res<Calendar>),
    settlements: Query<(&sim_core::SpawnId, &Settlement)>,
    mut draft: ResMut<Draft>,
) {
    let _span = info_span!("game_ui::screens::spawn_new_game").entered();
    if !draft.ready {
        draft.seed = seed.0.to_string();
        draft.difficulty = run.config.unwrap_or_default().difficulty;
        draft.ready = true;
    }
    let place = |id: u64| {
        settlements
            .iter()
            .find(|(s, _)| s.0 == id)
            .map_or_else(|| "?".into(), |(_, s)| names.get(s.name).to_string())
    };
    let band = progression
        .data
        .bands
        .get(progress.band as usize)
        .map_or("?", |b| b.name.as_str());
    let modifiers: Vec<String> = progress
        .modifiers
        .iter()
        .filter_map(|&m| progression.modifier(m))
        .map(|m| format!("  {}: {}", m.name, m.text))
        .collect();
    let start = offers
        .for_faction(FactionId::PLAYER)
        .find(|o| o.kind == OfferKind::StartBoon)
        .cloned();
    commands
        .spawn((NewGameRoot, backdrop()))
        .with_children(|p| {
            p.spawn(line("A new run", 30.0, style::GOLD));
            p.spawn(line(
                "Rise from a wandering band to lord of the island. Beat the rival warlord, or die trying.",
                14.0,
                style::DIM,
            ));
            // GP-12: saved runs, newest first.
            if !saved.0.is_empty() {
                p.spawn(Node {
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn(line("Resume:", 16.0, style::GOLD));
                    for run in saved.0.iter().take(3) {
                        let h = &run.header;
                        let at = calendar.at(h.tick);
                        let label = format!(
                            "seed {}, {}, day {}",
                            h.seed,
                            h.run.unwrap_or_default().difficulty.name(),
                            at.day + 1
                        );
                        style::button(r, ResumeButton(run.path.clone()), label, 14.0);
                    }
                });
            }
            // Seed and difficulty.
            p.spawn(Node {
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                r.spawn((SeedText, line("", 16.0, style::TEXT)));
                style::button(r, NewGameButton::Random, "Random", 14.0);
                r.spawn(line("   Difficulty:", 16.0, style::TEXT));
                for d in Difficulty::ALL {
                    style::button(r, NewGameButton::Difficulty(d), capitalise(d.name()), 14.0);
                }
            });
            p.spawn(line("Type digits for a seed; Backspace deletes.", 12.0, style::DIM));
            p.spawn((
                RebuildRow,
                Node {
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                Visibility::Hidden,
            ))
            .with_children(|r| {
                r.spawn(line(
                    "A new seed or difficulty needs a fresh start:",
                    14.0,
                    style::GOLD,
                ));
                style::button(r, NewGameButton::Build, "Build it", 15.0);
            });
            p.spawn((
                CardsRow,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(line(format!("Your band: {band}"), 15.0, style::TEXT));
                c.spawn(line("This island's run modifiers:", 15.0, style::TEXT));
                let text = if modifiers.is_empty() {
                    "  none".to_string()
                } else {
                    modifiers.join("\n")
                };
                c.spawn(line(text, 14.0, style::DIM));
                c.spawn(line("Choose a boon to begin:", 18.0, style::GOLD));
                match &start {
                    Some(o) => crate::cards::spawn_cards(c, o, &progression, &place),
                    None => {
                        c.spawn(line("(no start boon waiting)", 14.0, style::DIM));
                    }
                }
            });
        });
}

/// Digits typed anywhere on the new-game screen edit the seed.
fn seed_typing(mut keys: MessageReader<KeyboardInput>, mut draft: ResMut<Draft>) {
    let _span = info_span!("game_ui::screens::seed_typing").entered();
    for k in keys.read() {
        if k.state != ButtonState::Pressed {
            continue;
        }
        match &k.logical_key {
            Key::Character(c) => {
                for ch in c.chars().filter(char::is_ascii_digit) {
                    if draft.seed.len() < 19 {
                        draft.seed.push(ch);
                    }
                }
            }
            Key::Backspace => {
                draft.seed.pop();
            }
            _ => {}
        }
    }
}

fn new_game_buttons(
    buttons: Query<(&Interaction, &NewGameButton), Changed<Interaction>>,
    mut draft: ResMut<Draft>,
    mut out: MessageWriter<NewRunRequest>,
) {
    let _span = info_span!("game_ui::screens::new_game_buttons").entered();
    for (interaction, b) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *b {
            NewGameButton::Difficulty(d) => draft.difficulty = d,
            NewGameButton::Random => draft.seed = random_seed().to_string(),
            NewGameButton::Build => {
                let seed = draft.seed.parse().unwrap_or(0);
                info!("new run: seed {seed}, {}", draft.difficulty.name());
                out.write(NewRunRequest {
                    seed,
                    difficulty: draft.difficulty,
                });
            }
        }
    }
}

fn resume_buttons(
    buttons: Query<(&Interaction, &ResumeButton), Changed<Interaction>>,
    mut out: MessageWriter<SaveRequest>,
) {
    let _span = info_span!("game_ui::screens::resume_buttons").entered();
    for (interaction, b) in &buttons {
        if *interaction == Interaction::Pressed {
            info!("resume: {}", b.0.display());
            out.write(SaveRequest::Resume(b.0.clone()));
        }
    }
}

#[allow(clippy::type_complexity)] // Queries.
fn refresh_new_game(
    draft: Res<Draft>,
    seed: Res<SimSeed>,
    run: Res<RunState>,
    mut text: Query<&mut Text, With<SeedText>>,
    mut buttons: Query<(&NewGameButton, &mut BackgroundColor)>,
    mut rows: ParamSet<(
        Query<&mut Visibility, With<RebuildRow>>,
        Query<&mut Visibility, With<CardsRow>>,
    )>,
) {
    let _span = info_span!("game_ui::screens::refresh_new_game").entered();
    if let Ok(mut t) = text.single_mut() {
        t.0 = format!("Seed: {}_", draft.seed);
    }
    for (b, mut bg) in &mut buttons {
        let active = matches!(b, NewGameButton::Difficulty(d) if *d == draft.difficulty);
        bg.0 = if active {
            style::BUTTON_ACTIVE
        } else {
            style::BUTTON
        };
    }
    let same = draft.seed.parse::<u64>().ok() == Some(seed.0)
        && Some(draft.difficulty) == run.config.map(|c| c.difficulty);
    let (rebuild, cards) = if same {
        (Visibility::Hidden, Visibility::Inherited)
    } else {
        (Visibility::Inherited, Visibility::Hidden)
    };
    for mut v in &mut rows.p0() {
        v.set_if_neq(rebuild);
    }
    for mut v in &mut rows.p1() {
        v.set_if_neq(cards);
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Plain words for why a run ended.
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
        Cause::Lapsed => "left unanswered".into(),
        Cause::Neglect => "neglect".into(),
        Cause::Fight { faction } => {
            format!("fighting {}", army.faction_name(faction).to_lowercase())
        }
    }
}

/// Whether a chronicle entry is on the end screen's timeline (UI-09): what
/// you did and what was done to you, the run's start and end, and anyone's
/// conquests and the rival's raids. Battles have their own list.
pub fn on_timeline(e: &SimEvent) -> bool {
    let player = FactionId::PLAYER;
    let from_me = matches!(e.kind, SimEventKind::MonumentTaken | SimEventKind::Conquest)
        && e.other == player.0 as u64;
    match e.kind {
        SimEventKind::Battle => false,
        SimEventKind::RunStarted
        | SimEventKind::RunEnded
        | SimEventKind::Conquest
        | SimEventKind::RivalRaid => true,
        _ => e.faction == player || from_me,
    }
}

/// Timeline lines shown at once; the wheel scrolls.
const TIMELINE_LINES: usize = 12;
/// Your battles reported at most (the latest).
const BATTLES_SHOWN: usize = 3;

/// The end screen's text, written when it opens (UI-09).
#[derive(Resource, Default)]
struct EndReport {
    headline: String,
    subline: String,
    /// `RunSummary::lines()` and a few more numbers.
    summary: Vec<String>,
    /// The timeline's sentences, oldest first.
    timeline: Vec<String>,
    /// The first timeline line shown.
    scroll: usize,
    /// Your latest battles, told with their reports.
    battles: Vec<String>,
    /// How many battles you fought.
    battle_count: usize,
}

impl EndReport {
    fn timeline_text(&self) -> String {
        if self.timeline.is_empty() {
            return "Nothing of note happened.".into();
        }
        let end = (self.scroll + TIMELINE_LINES).min(self.timeline.len());
        let mut out = Vec::new();
        if self.scroll > 0 {
            out.push(format!("({} earlier: scroll up)", self.scroll));
        }
        out.extend(self.timeline[self.scroll..end].iter().cloned());
        if end < self.timeline.len() {
            out.push(format!(
                "({} later: scroll down)",
                self.timeline.len() - end
            ));
        }
        out.join("\n")
    }
}

/// Fills [`EndReport`] from the world: the outcome, `run_summary`, and the
/// chronicle's entries told by `describe_entries` (exclusive: the sentences
/// read names from the world).
fn write_end_report(world: &mut World) {
    let _span = info_span!("game_ui::screens::write_end_report").entered();
    let run = *world.resource::<RunState>();
    let tick = world.resource::<SimTick>().0;
    let day = world
        .resource::<Calendar>()
        .at(run.ended_at.unwrap_or(tick))
        .day
        + 1;
    let headline = match run.outcome.map_or("over", |o| o.name()) {
        "won" => "Victory: the island is yours".to_string(),
        "lost" => "Defeat".to_string(),
        "abandoned" => "Run abandoned".to_string(),
        o => format!("The run is {o}"),
    };
    let subline = format!(
        "Day {day} | {} | seed {} | {}",
        cause_text(run.cause, world.resource::<Army>()),
        world.resource::<SimSeed>().0,
        run.config.unwrap_or_default().difficulty.name()
    );
    let mut summary: Vec<String> = sim_military::run_summary(world)
        .lines()
        .iter()
        .map(|l| capitalise(l))
        .collect();
    summary.push(format!(
        "Drifters joined: {}",
        world.resource::<RunProgress>().drifters
    ));
    let entries: Vec<ChronicleEntry> = world.resource::<Chronicle>().entries().to_vec();
    let story: Vec<ChronicleEntry> = entries
        .iter()
        .filter(|e| on_timeline(&e.event))
        .copied()
        .collect();
    let mine: Vec<ChronicleEntry> = entries
        .iter()
        .filter(|e| e.event.kind == SimEventKind::Battle && e.event.faction == FactionId::PLAYER)
        .copied()
        .collect();
    let latest = &mine[mine.len().saturating_sub(BATTLES_SHOWN)..];
    let timeline = sim_military::describe_entries(world, &story);
    let battles = sim_military::describe_entries(world, latest);
    let scroll = timeline.len().saturating_sub(TIMELINE_LINES);
    world.insert_resource(EndReport {
        headline,
        subline,
        summary,
        timeline,
        scroll,
        battles,
        battle_count: mine.len(),
    });
}

fn heading(text: impl Into<String>) -> impl Bundle {
    line(text, 18.0, style::GOLD)
}

fn spawn_end(mut commands: Commands, report: Res<EndReport>) {
    let _span = info_span!("game_ui::screens::spawn_end").entered();
    let column = |width: f32| Node {
        width: Val::Percent(width),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        ..default()
    };
    commands.spawn((EndRoot, backdrop())).with_children(|p| {
        p.spawn(line(report.headline.clone(), 32.0, style::GOLD));
        p.spawn(line(report.subline.clone(), 16.0, style::TEXT));
        p.spawn(Node {
            width: Val::Percent(94.0),
            column_gap: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(column(40.0)).with_children(|c| {
                c.spawn(heading("Summary"));
                c.spawn(line(report.summary.join("\n"), 14.0, style::TEXT));
                c.spawn(heading(format!("Your battles ({})", report.battle_count)));
                let battles = if report.battles.is_empty() {
                    "None fought.".to_string()
                } else {
                    let mut b = report.battles.clone();
                    if report.battle_count > b.len() {
                        b.insert(0, format!("(the last {} of them)", b.len()));
                    }
                    b.join("\n")
                };
                c.spawn(line(battles, 13.0, style::DIM));
            });
            row.spawn(column(54.0)).with_children(|c| {
                c.spawn(heading(format!("Timeline ({})", report.timeline.len())));
                c.spawn((
                    TimelineText,
                    line(report.timeline_text(), 13.0, style::TEXT),
                ));
            });
        });
        p.spawn(Node {
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|r| {
            style::button(r, EndButton::NewRun, "New run", 18.0);
            style::button(r, EndButton::SameSeed, "Same seed again", 18.0);
        });
    });
}

/// The mouse wheel scrolls the end screen's timeline.
fn scroll_timeline(
    mut wheel: MessageReader<MouseWheel>,
    mut report: ResMut<EndReport>,
    mut text: Query<&mut Text, With<TimelineText>>,
) {
    let _span = info_span!("game_ui::screens::scroll_timeline").entered();
    let mut lines = 0.0;
    for w in wheel.read() {
        lines += match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y / 20.0,
        };
    }
    let step = lines.round() as i64;
    if step == 0 {
        return;
    }
    let max = report.timeline.len().saturating_sub(TIMELINE_LINES) as i64;
    let scroll = (report.scroll as i64 - step).clamp(0, max) as usize;
    if scroll == report.scroll {
        return;
    }
    report.scroll = scroll;
    if let Ok(mut t) = text.single_mut() {
        t.0 = report.timeline_text();
    }
}

fn end_buttons(
    buttons: Query<(&Interaction, &EndButton), Changed<Interaction>>,
    seed: Res<SimSeed>,
    run: Res<RunState>,
    mut out: MessageWriter<NewRunRequest>,
) {
    let _span = info_span!("game_ui::screens::end_buttons").entered();
    let difficulty = run.config.unwrap_or_default().difficulty;
    for (interaction, b) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let seed = match b {
            EndButton::NewRun => random_seed(),
            EndButton::SameSeed => seed.0,
        };
        out.write(NewRunRequest { seed, difficulty });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UI-09: the timeline tells your story and the island's big turns;
    /// battles have their own list.
    #[test]
    fn the_timeline_keeps_your_story() {
        let ev = |kind, f: FactionId| SimEvent::new(kind, f);
        assert!(on_timeline(&ev(
            SimEventKind::RunStarted,
            FactionId::PLAYER
        )));
        assert!(on_timeline(&ev(
            SimEventKind::MonumentTaken,
            FactionId::PLAYER
        )));
        assert!(on_timeline(&ev(SimEventKind::Conquest, FactionId::RIVAL)));
        assert!(on_timeline(&ev(SimEventKind::RivalRaid, FactionId::RIVAL)));
        // Something taken from you.
        assert!(on_timeline(
            &ev(SimEventKind::MonumentTaken, FactionId::BEASTS).other(FactionId::PLAYER.0 as u64)
        ));
        assert!(!on_timeline(&ev(
            SimEventKind::MonumentTaken,
            FactionId::BEASTS
        )));
        assert!(!on_timeline(&ev(
            SimEventKind::RaidersArrived,
            FactionId::RAIDERS
        )));
        assert!(!on_timeline(&ev(SimEventKind::Battle, FactionId::PLAYER)));
    }

    #[test]
    fn the_timeline_scrolls_in_a_window() {
        let mut r = EndReport {
            timeline: (0..20).map(|i| format!("line {i}")).collect(),
            ..default()
        };
        let text = r.timeline_text();
        assert!(text.starts_with("line 0\n") && text.ends_with("(8 later: scroll down)"));
        r.scroll = 8;
        let text = r.timeline_text();
        assert!(text.starts_with("(8 earlier: scroll up)\nline 8") && text.ends_with("line 19"));
        assert_eq!(
            EndReport::default().timeline_text(),
            "Nothing of note happened."
        );
    }

    #[test]
    fn random_seeds_are_short_and_vary() {
        let a = random_seed();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = random_seed();
        assert!(a < 1_000_000_000 && b < 1_000_000_000);
        assert_ne!(a, b);
    }
}
