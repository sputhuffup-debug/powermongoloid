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

## Step 10b: saves, resume and the chronicle in the game

Built on `3a8baa0` (steps 8-9 merged). Only `game_ui` and `game_app` changed; no sim crate, no data file, no accessor added.

### Saves (GP-12), `game_app/src/saves.rs`
- **One slot per run** in the save directory: `--save-dir <dir>`, else `saves/` next to the game binary (`target/debug/saves` under `cargo run`). A run's slot is `run-<seed>-<difficulty>.sav`; the developed starts quicksave to `world-<seed>.sav` but are never saved on exit or offered for resuming.
- **F5** saves this run's slot (refused once the run is over), **F9** loads it. **Closing the game** saves a run that has started (tick > 0) and isn't over, and deletes a finished run's slot; so does starting another run in place. Files are written through a `.sav.tmp` and renamed.
- **Resume**: the new-game screen lists up to three saved runs, newest first ("seed 42, steady, day 2", from `read_header` plus the calendar); a click loads one.
- **Loading** is `sim_core::load_world` into the running world, between ticks (`saves::handle_saves`, exclusive, in `AppSet::Saves` before `RenderSet::Epoch` and `UiSet::Screens`). A save of another format is refused before anything changes; otherwise the world is first saved to memory and restored if the load fails part-way. After a load: `WorldEpoch` += 1, the clock pauses, `OpeningShot` is set again (the champion, else the run's start, else your first captain), the resume list is re-read, and a notice says what happened.
- **New runs are in-process now.** A `NewRunRequest` (*Build it*, *New run*, *Same seed again*) leaves the current run, builds the new island on a worker thread with `sim_headless::headless_app(HeadlessConfig::run(..))` + `save_app` (exactly what a fresh process builds), then loads those bytes like any save. The window stays open; the new-game screen comes back with the new seed's cards. If the worker panics or the load fails, the game falls back to restarting itself with `--seed --difficulty --save-dir` (step 10's path).
- `game_ui::saves`: `SaveRequest {Save, Load, Resume(path)}` (F5/F9 in `quick_keys`, "Resume" buttons), `SavedRuns`/`SavedRun` (filled by game_app), and `Notice`, a one-line message under the run bar ("Saved day 1 21:34 (F9 loads it)", "Building the island for seed ...", "Loaded the saved run: day 2 23:48 (paused)").
- **After a new `WorldEpoch` the UI forgets the old world**: `SentOffers` (`cards::forget_sent`), the card popup (`run_bar::close_offer`) and the offer buttons, the feed (restarts from the loaded feed's last 8 lines without pausing on them; fast-forward stops), the new-game draft, and the screen (`choose_screen` closes any screen and decides again next frame: new game for a waiting start boon, end screen for a finished run, else the island). Terrain, models, labels and the selection already cleared on it.

### Chronicle in the UI (UI-05, UI-09)
- **Feed** (`feed.rs`): `read_feed` is exclusive now. New `EventFeed` entries are told by `sim_military::describe_entries` (one batch per frame, each entry described once and its line kept), merged by tick with `MilitaryLog::since` lines that name your warband (dimmer), minus those an event already tells (offers chosen, the run abandoned, the champion's rank or fall, requests answered, invitations, revolts, drifters, "took <place>"). Other factions' veteran ranks and answered offers get no line. Auto-pause and fast-forward work as before. The feed is 520 px wide (the sentences are longer). Its own `describe` and `who`/`whose` are gone; `cause_text` moved to `screens.rs`.
- **End screen**: `write_end_report` (exclusive, `OnEnter(Ended)`) fills `EndReport`, then `spawn_end` draws: the headline and "Day N | cause | seed | difficulty"; *Summary* (`run_summary(world).lines()` plus drifters); *Your battles (n)*: your last three `Battle` entries told with their reports; *Timeline (n)*: the chronicle's entries on your story (`on_timeline`: what you did and what was done to you, the run's start and end, anyone's conquests, rival raids; battles excluded), 12 lines at a time, the mouse wheel scrolls (it opens on the latest). `RunNumbers` is gone.

### Visual check (Xvfb, lavapipe, about 4 fps)
- New game, seed 42: no *Resume* row (empty save dir). Picked *Well fed*; at 8x the feed read "Day 1: Your warband chose the boon Well fed.", requests ("Ashmere asked your warband to bring food.") and "5 Raiders came down on the roads near Emberhaven, in a raid led by Briony Woolley." (it auto-paused on that).
- **F5** at day 1 21:34: notice "Saved day 1 21:34 (F9 loads it)", 350 KB written in 0.06 s. Played on to day 2 08:52, **F9**: back to day 1 21:34, paused, the feed showing the loaded feed's last lines, notice "Loaded the quicksave"; load took 0.71 s. Unpaused: the same raid ("led by Briony Woolley") came again at the same hour, and the game auto-paused on it again.
- Played to day 2 evening; closed the window (WM_DELETE_WINDOW): "saved ... run-42-steady.sav" on exit.
- Restarted with `--seed 7`: the new-game screen showed **"Resume: seed 42, steady, day 2"** above the seed row. Clicking it loaded seed 42 at day 2 23:48 in 0.79 s: terrain rebuilt (256 chunks), seed 7's labels and models gone, the camera on the champion, paused, the feed with the run's last lines.
- **Abandon** (twice): end screen "Run abandoned", "Day 2 | by your choice | seed 42 | steady", Summary ("Run abandoned (Player) after 1.8 days", settlements, monuments, battles, drifters), "Your battles (0): None fought.", "Timeline (9)": "Day 1: Fenna Jessop set out with a band of outlaws, far from the rival's seat at Dunmoor.", "Day 1: Your warband chose the boon Well fed.", six "asked in vain ..., for no help came in time.", "Day 2: The run was abandoned, by your warband's choice."
- **New run** from the end screen: run-42-steady.sav removed (run over), "Building the island for seed 503836236 (steady)...", built on the worker in 0.90 s, loaded in 0.69 s; the new-game screen came back with seed 503836236, band "Hunters and farmhands", modifiers and cards, in the same window. Picked *Known* and played: Yarrowley and the band drawn, the feed with the new run's lines. Closing saved run-503836236-steady.sav.
- Found in the check and fixed: the drifters' log line repeated the `DriftersJoined` sentence in the feed.
- Keyboard input under Xvfb still needs `xdotool windowfocus`; closing the window needs a WM_DELETE_WINDOW client message (no window manager), sent with a 20-line Xlib helper.

### Tests
- game_app: `a_save_loads_over_another_run` (a run's save loaded with `load_world` into an app already holding another run hashes the same and stays equal for 60 ticks: the in-game load path), `one_slot_per_run`, `the_resume_list_holds_run_saves_only`.
- game_ui: `log_lines_about_your_warband_join_the_feed_once`, `other_factions_offers_and_ranks_get_no_line`, `the_timeline_keeps_your_story`, `the_timeline_scrolls_in_a_window`.

### Checks run
- `cargo fmt --check`, `cargo clippy -j 2 --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`: clean.
- `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace --no-fail-fast -- --test-threads=2`: exit 0, 250 passed, 0 failed, 1 ignored.
- Release replay smoke (`--run --seed 42 --ticks 3000 --commands 50`, replayed multi- and single-threaded): "replay ok", `0x004905ef0f268f72`, unchanged (no sim crate changed). Developed and skirmish stats output byte-identical to step 5's.

### Open issues
- **No ironman**: F9 after a defeat reloads the last quicksave of that run until you leave it (its slot is deleted only when you leave a finished run).
- **Loads block the window** for 0.7-0.8 s (the map is regenerated on load); a new island also builds in the background for about a second.
- The summary's first line comes from `RunSummary::lines()` and reads "Run abandoned (Player) after 1.8 days" (the cause in `Debug` form); a worded cause belongs in sim_military.
- The timeline lists every request that lapsed; a long run will have many. It scrolls, but a filter (or grouping lapsed requests per day) may read better.
- The feed's duplicate filter for military log lines is a list of phrases in `feed.rs` (`log_line_shown`); a new log line that repeats an event needs adding there.
- Saves of the developed starts (`world-<seed>.sav`) are never cleaned up.
- The end screen's battle reports show only your last three; there is no per-battle drill-down.
