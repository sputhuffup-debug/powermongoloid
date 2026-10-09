//! M4 run screens (GP-21, GP-27, UI-09): the new-game screen (seed,
//! difficulty, the run modifiers drawn, the band, and the three start-boon
//! cards) and the end screen (outcome, cause, day and the run's numbers,
//! with "New run" and "Same seed again").
//!
//! Both are full-screen nodes above everything else; while one is up the
//! game is not [`crate::playing`], so hotkeys, world clicks and camera
//! input are ignored ([`game_render::InputGate`]).
//!
//! A different seed or difficulty needs a different world: the screen
//! sends [`NewRunRequest`] and `game_app` rebuilds the app.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use game_render::InputGate;
use sim_agents::Owner;
use sim_core::{
    Calendar, Chronicle, Difficulty, FactionId, NameTable, OfferKind, Offers, RunState,
    SimEventKind, SimSeed, SimTick,
};
use sim_military::{Army, ArmyStats, Holder, Progression, RequestBoard, RunProgress, Veteran};
use sim_world::Settlement;

use crate::style;
use crate::{NewRunRequest, UiScreen, UiSet};

pub struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Draft>()
            .add_systems(
                Update,
                (choose_screen, gate_input).chain().in_set(UiSet::Screens),
            )
            .add_systems(OnEnter(UiScreen::NewGame), spawn_new_game)
            .add_systems(OnExit(UiScreen::NewGame), despawn::<NewGameRoot>)
            .add_systems(OnEnter(UiScreen::Ended), spawn_end)
            .add_systems(OnExit(UiScreen::Ended), despawn::<EndRoot>)
            .add_systems(
                Update,
                (seed_typing, new_game_buttons, refresh_new_game)
                    .chain()
                    .run_if(in_state(UiScreen::NewGame))
                    .in_set(UiSet::Hud),
            )
            .add_systems(
                Update,
                end_buttons
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
/// waiting at tick 0, the clock paused) and the end screen once the run is
/// over.
fn choose_screen(
    run: Option<Res<RunState>>,
    tick: Option<Res<SimTick>>,
    offers: Option<Res<Offers>>,
    screen: Res<State<UiScreen>>,
    mut next: ResMut<NextState<UiScreen>>,
    mut time: ResMut<Time<Virtual>>,
    mut opened: Local<bool>,
) {
    let _span = info_span!("game_ui::screens::choose_screen").entered();
    let (Some(run), Some(tick), Some(offers)) = (run, tick, offers) else {
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
        if tick.0 == 0 && boon_waiting {
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
                    "This seed or difficulty makes a different island:",
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

/// The numbers on the end screen.
#[derive(Debug, Default, PartialEq)]
pub struct RunNumbers {
    pub day: u64,
    pub settlements: u32,
    pub monuments: u32,
    pub conquests: u32,
    pub battles: u32,
    pub kills: u32,
    pub lost: u32,
    pub veterans: u32,
    pub requests: u32,
    pub drifters: u32,
}

#[allow(clippy::too_many_arguments)] // Bevy system parameters.
fn spawn_end(
    mut commands: Commands,
    run: Res<RunState>,
    calendar: Res<Calendar>,
    tick: Res<SimTick>,
    army: Res<Army>,
    stats: Res<ArmyStats>,
    chronicle: Res<Chronicle>,
    board: Res<RequestBoard>,
    progress: Res<RunProgress>,
    seed: Res<SimSeed>,
    owners: Query<&Owner>,
    holders: Query<&Holder>,
    veterans: Query<(&Veteran, &sim_core::Allegiance)>,
) {
    let _span = info_span!("game_ui::screens::spawn_end").entered();
    let player = FactionId::PLAYER;
    let mine = stats.get(player);
    let n = RunNumbers {
        day: calendar.at(run.ended_at.unwrap_or(tick.0)).day + 1,
        settlements: owners.iter().filter(|o| o.faction == player).count() as u32,
        monuments: holders.iter().filter(|h| h.faction == player).count() as u32,
        conquests: mine.conquests,
        battles: chronicle
            .entries()
            .iter()
            .filter(|e| e.event.kind == SimEventKind::Battle && e.event.faction == player)
            .count() as u32,
        kills: mine.kills,
        lost: mine.lost,
        veterans: veterans
            .iter()
            .filter(|(v, a)| v.rank > 0 && a.0 == player)
            .count() as u32,
        requests: board.done,
        drifters: progress.drifters,
    };
    let outcome = run.outcome.map_or("over", |o| o.name());
    let headline = match outcome {
        "won" => "Victory: the island is yours".to_string(),
        "lost" => "Defeat".to_string(),
        "abandoned" => "Run abandoned".to_string(),
        o => format!("The run is {o}"),
    };
    let difficulty = run.config.unwrap_or_default().difficulty.name();
    commands.spawn((EndRoot, backdrop())).with_children(|p| {
        p.spawn(line(headline, 32.0, style::GOLD));
        p.spawn(line(
            format!(
                "Day {} | {} | seed {} | {difficulty}",
                n.day,
                crate::feed::cause_text(run.cause, &army),
                seed.0
            ),
            16.0,
            style::TEXT,
        ));
        p.spawn(line(
            format!(
                "Settlements held: {}   Monuments held: {}   Conquests: {}\n\
                 Battles: {}   Foes slain: {}   Men lost: {}   Veterans alive: {}\n\
                 Requests answered: {}   Drifters joined: {}",
                n.settlements,
                n.monuments,
                n.conquests,
                n.battles,
                n.kills,
                n.lost,
                n.veterans,
                n.requests,
                n.drifters
            ),
            15.0,
            style::TEXT,
        ));
        // TODO(M4 step 9): the chronicle timeline (`sim_military::chronicle::
        // describe`) and `RunSummary` go here, under the numbers.
        p.spawn(line(
            "(The chronicle of this run arrives with M4 step 9.)",
            12.0,
            style::DIM,
        ));
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

    #[test]
    fn random_seeds_are_short_and_vary() {
        let a = random_seed();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = random_seed();
        assert!(a < 1_000_000_000 && b < 1_000_000_000);
        assert_ne!(a, b);
    }
}
