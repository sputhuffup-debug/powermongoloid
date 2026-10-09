# M4 step 10: UI and rendering (branch `claude/m4-ui-screens`)

Built in parallel with steps 8 and 9 (branch `claude/brave-turing-sh1ryn`), from commit `a4abbd6`. No `sim_*` crate was changed.

## What was built

### game_app
- **A run is the default start.** `--seed <u64>` (a run's default comes from the clock; the developed starts keep 42), `--difficulty steady|chaotic|harsh`. `--skirmish`, `--villages` and `--stress n` keep the developed preset. `--run` is still accepted and means the default. Setup goes through `sim_headless::set_up_scenario`, as before.
- **New runs restart the process** (`new_run`, on a `game_ui::NewRunRequest`): the binary spawns itself with `--seed S --difficulty D` and exits. Why not in-process: winit can't create a second event loop in one process, so a fresh `App` is impossible, and resetting every sim resource in place (about 130 `init_resource`/`insert_resource` sites across the sim crates, more in step 8) is the fragile path the guide warned about. See "Hooks" for the in-process path once saves exist.
- `frame_start` now runs in `Update` whenever `OpeningShot` changes, not only in `PostStartup`.
- `AppSet::Game` holds `new_run`, `frame_start` and `stress_log`, each with a span.

### game_ui (bevy_ui; egui stays dev-only)
- **`UiScreen` state** (`Playing` default, `NewGame`, `Ended`), `UiSet {Screens, Hud}` (chained), the `playing` run condition, and `NewRunRequest`.
- **`screens.rs`**:
  - `choose_screen` opens `NewGame` on a run's first frame while the start boon waits (and pauses the clock), and `Ended` once `RunState.outcome` is set. `gate_input` sets `game_render::InputGate`.
  - The new-game screen: the seed (typed digits via `KeyboardInput`, Backspace; *Random* from `random_seed()`, which hashes `SystemTime`), difficulty buttons, the band drawn, the run modifiers drawn, and the three start-boon cards. When the typed seed or difficulty differ from the world's, the cards hide and *Build it* sends `NewRunRequest`.
  - The end screen: outcome, cause, day, seed, difficulty, and the numbers available now: settlements and monuments held, conquests, battles (chronicle `Battle` entries of the player), foes slain, men lost, living veterans, requests answered, drifters. *New run* (random seed, same difficulty) and *Same seed again*.
- **`cards.rs`**: offer cards for start and dominion boons, traits (name, phrase, effects) and invitations (accept or decline). A click sends `ChooseOffer` through `army::send` (now `pub(crate)`) once (`SentOffers`); on the new-game screen it also switches to `Playing` and starts the clock at 1x, so the pick applies at tick 0.
- **`run_bar.rs`** (under the time bar): day, your share of each region (`RunProgress.dominion`), the rival's men (its captains plus their followers), seat defence bonus and the time to its next levy and raid (`RivalState`), captain slots used/available, the overlay mode; waiting offers as buttons that open a centred card popup (*Later* closes it); *>> Next event* (8x until the next notable event, then pause); *Abandon run* (click twice within 6 s; sends `AbandonRun` and unpauses so the sim takes it).
- **`feed.rs`** (bottom left): reads `EventFeed` by `seq` cursor, a plain sentence per event (`describe`, `cause_text`), `VeteranRank` floods grouped into one line, answered offers skipped. Clicking a line moves the camera to the event. It pauses the game on the difficulty's `auto_pause` kinds (director.ron; `OfferPosted` only while the offer still waits for you) and ends fast-forward on a notable event.
- **`tooltip.rs`**: hovering within 9 tiles of a monument shows its name, kind, region, bonus per monument and cap (progression.ron `monuments.kinds`) and holder. The ground ray-cast runs only when the cursor or the camera moves; the box flips left near the right edge.
- **Panels**:
  - Settlement panel: owner, goodwill toward you (bar and value), loyalty mean and lowest with "unrest since day N" (`below_since`), and the open request with its time left and reward.
  - Captain panel: the champion's rank, points and next threshold, traits; captain slots used/available in a run; *Appoint captain* greys out when the slots are full (the sim still refuses an appointment). The recent-log day now comes from `Calendar` (was `tick / 2400`). Garrison and Give were already in the village menu, Forage in the panel and on F.
  - Inspector: a soldier's veteran rank and battles, and traits if any.
- **Gating**: `speed_keys`, `pick`, `right_click`, `hotkeys`, `bar_buttons`, `click_feed`, the tooltip and the dev debug panel run only while `playing`. Full-screen screens are `GlobalZIndex(20)` nodes with an `Interaction`.
- Labels and the selection clear on a `WorldEpoch` change (labels moved from a `Local` to a resource).

### game_render
- **`RenderSet {Epoch, Overlay, World}`** (chained), **`WorldEpoch(u32)`** and `epoch_changed`, **`InputGate`**.
- **`monuments.rs`**: a cuboid model per `MonumentKind` (standing stones, ruined fort, holy well, road marker, ancient forge, market cross, wayshrine, watchtower) sharing `Finish` with buildings.rs, plus a pole with a pennant in the holder's faction colour (army.ron), hidden while nobody holds it.
- **`overlay.rs`**: `Overlay`/`OverlayMode` (O cycles off, ownership, goodwill / loyalty, food security, monuments). Discs around settlements (radius by population) or monuments, colours quantised to 8 levels a channel; recomputed twice a second, and `generation` moves only when a disc changes. Monument mode also draws gizmo rings at `monuments.hold_radius` in the holder's colour.
- **terrain.rs**: each chunk remembers the overlay signature (the discs touching it) it was built with; it rebuilds when that changes (`build_chunk_mesh_tinted`). Chunks and the ocean clear on `WorldEpoch`.
- buildings.rs and monuments.rs drop their models on `WorldEpoch`.
- The camera (`rig_input`, `apply_rig`) runs on real time, so it moves while the game is paused (it froze before), and ignores input while the `InputGate` is closed.
- The rival and beasts already had colours in army.ron; `faction_color` reads them.

## Files touched
- `crates/game_app/src/main.rs`
- `crates/game_ui/src/{lib,army,cards,feed,labels,panels,run_bar,screens,selection,style,time_bar,tooltip,debug_panel}.rs` (cards, feed, run_bar, screens, tooltip are new)
- `crates/game_render/src/{lib,buildings,camera,monuments,overlay,terrain}.rs` (monuments, overlay are new)
- Docs: `README.md` (start commands, controls), `docs/dev/ARCHITECTURE.md` (crate rows, a new "The game's frame (Update)" table of sets and systems), `docs/dev/GLOSSARY.md` (Input gate, Overlay, Run bar, Screen, World epoch), `docs/dev/TESTING.md` (game_render and game_ui unit tests), this file.

## Sim accessors added
None. Everything needed was already public (`RunProgress`, `RivalState`, `Offers`, `Progression::{boon, trait_data, modifier}`, `MonumentData::bonus`, `Goodwill::get`, `RequestBoard.open`, `ArmyStats::get`, `Chronicle::entries`, `DirectorData::personality`).

## Hooks left for steps 8 and 9
- **Step 8 (save and resume)**, `game_app/src/main.rs` crate docs (TODO there): F5/F9 quicksave/quickload, save on exit and a resume menu are not built. For an in-process new run or load: build the run headless (or read the save), `load_world` it into the game's `World`, then bump `game_render::WorldEpoch`; terrain chunks, the ocean, building and monument models, labels and the selection already clear on it, and `frame_start` reruns when `OpeningShot` is set again. Also reset `game_ui`'s `SentOffers` then (it is keyed by offer id, which restarts per run). Until then `new_run` restarts the process. A resume entry would sit on the new-game screen (`screens::spawn_new_game`).
- **Step 9 (chronicle)**: `feed.rs` (module TODO) should use `sim_military::chronicle::describe` instead of its own `describe`, and merge `MilitaryLog` lines once they carry a `seq`. The end screen (`screens::spawn_end`, TODO) has a placeholder line where the chronicle timeline and `RunSummary` go; its numbers can then come from `RunSummary`.

## Visual check (Xvfb, `--seed 42`, lavapipe at 3-4 fps)
All seen working:
- New-game screen: seed 42, *Steady* active, band "Outlaws", modifiers "Weaker rival" and "Restless roads", cards Hardy / Well fed / Woodwise. Typing 7 gave seed 427 and showed the *Build it* row; *Harsh* highlighted.
- Choosing *Well fed*: the screen closed and the clock ran at 1x; the run bar showed day, shares R0-R3 0%, the rival's 20 men, levy in 1d 22h and raid in 9d 20h; the feed listed requests.
- Captain panel on the band: champion rank 0 (0 points, next at 3), trait Brave, captain slots 0/0 with *Appoint* greyed; the inspector showed "Rank: Recruit (0 battles)".
- Overlays: ownership (independent discs around each village) and monuments (discs and rings in holder colours).
- A monument up close: the holy well with a red raider pennant, its guard camp and ring. Its tooltip: "Pellcombe Well (holy well, region 3) +1% food yield each (up to +10% food yield) Held by: Raiders".
- Auto-pause: the game paused on "5 raiders appeared, bound for Emberhaven".
- Offer card popup from the run bar (tested with a local, uncommitted hack that skipped the new-game screen so the start boon waited in the bar); clicking a card sent `ChooseOffer` and closed it.
- Clicking a feed line moved the camera to Gorsecross.
- Abandon (twice) led to the end screen: "Run abandoned", "Day 1 | by your choice | seed 42 | steady", the numbers, *New run* and *Same seed again*. *Same seed again* restarted the game as `--seed 42 --difficulty steady` with the same cards.

Keyboard input under Xvfb needs `xdotool windowfocus` first (mouse clicks don't).

## Checks run
- `cargo fmt --check`, `cargo clippy -j 2 --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: clean.
- `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace -- --test-threads=2`: exit 0, 234 passed, 0 failed, 1 ignored (the integration suite included; no sim crate changed).

## Open issues and rough edges
- **New runs restart the process** (window closes and reopens, about as long as a cold start). In-process needs step 8 (see Hooks).
- **The start boon's `OfferPosted` is in the feed at tick 0.** It is skipped as a line and doesn't pause once answered, but a run started past the new-game screen (only possible with a hack) pauses at tick 1-2 on it, as designed for waiting offers.
- **`auto_pause` fires on events anywhere** (any raiders arriving, any rival raid), not only near the player.
- **Monument models have flat materials only** (no textured detail band swap like buildings) and no LOD; 24 per island, so cheap.
- **Overlay discs are a stand-in for territory**: a disc per settlement sized by population, not the settlement's real footprint; overlapping discs take the nearest centre.
- **Region shares are labelled R0..R3** (no region names exist).
- **The rival's "men"** counts its captains and followers, including raiding bands away from the seat.
- **The card popup and run bar** are fixed-width (920 px); on windows narrower than about 960 px they overflow. The time bar's text wraps at some times of day (pre-existing).
- **Captain slots in the panel** read `RunProgress.captain_slots` (hourly), so the grey-out can lag a boon pick by up to an hour; the sim checks the live value.
- **The end screen's numbers** are computed when it opens from live state (settlements and monuments held at the end, living veterans); step 9's `RunSummary` should replace them.
- **No test drives the screens headlessly** (they need a window); coverage is the unit tests listed in TESTING.md plus the Xvfb check above.
