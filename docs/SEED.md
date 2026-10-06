# Warlord Sim: Bevy Seed Document

Oct 5, 2026 · @M

## Purpose and how to use this document

This is the seed spec for an indirect-control medieval conquest simulation in the spirit of early-1990s god games, rebuilt in Rust and Bevy to simulate hundreds of thousands of individual people. It is written to be handed to Claude Code as the source of truth for bootstrapping the repository.

How to use it:

1. Export this doc as Markdown and save it in a new repo as `docs/SEED.md`.
2. Copy the CLAUDE.md template from the bootstrap section into the repo root.
3. Start Claude Code in the repo and run the first prompt from the bootstrap section. Work milestone by milestone; do not ask Claude Code to build everything at once.

Requirement IDs (for example `SIM-03`) are stable so commits, issues and tests can reference them. "Must" means required for the milestone it belongs to; "should" means desirable; "later" means out of scope until noted.

The game is an original work. It borrows design ideas (indirect command, living villages, logistics, delayed information) but no names, art, maps, text or assets from any existing game.

## Vision and design pillars

The player is a warlord who conquers a large, living island by commanding captains, not soldiers. Every villager is simulated, food is the real currency, and orders take time to arrive. Scale serves the fantasy: a region should feel like a real place you are changing, not a board you are painting.

Working title: Warlord Sim (placeholder). Platform: Windows, macOS, Linux desktop. Single-player first.

| Pillar | What it means in play | What it rules out |
| --- | --- | --- |
| Indirect command | You give orders and a posture to captains; their followers act on their own | Selecting and micro-managing individual soldiers |
| A living world | Every villager has a name, job, home, needs and a daily routine | Abstract population counters near the player |
| Logistics is strategy | Armies eat; food, tools and weapons come from real production chains | Free unit upkeep, instant reinforcement |
| Imperfect information | Reports and orders travel physically and arrive late | Perfect global map knowledge |
| Readable at scale | The player can always see why something happened | Hidden simulation the player cannot interpret |

The core loop: scout a region, decide whether to trade, recruit or conquer, move captains, feed and equip armies from captured settlements, and manage the aftermath (unrest, famine, loyalty) as you expand.

## Functional gameplay requirements

The player interacts only through captains, settlements and the map. Requirements are grouped by area and tagged with the milestone (M0 to M6) that first needs them.

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| GP-01 | The player controls one warlord captain and may later recruit up to 8 lieutenant captains | Must | M3 |
| GP-02 | Captains accept orders: move to, attack settlement, recruit, gather food, trade, equip, scout, return | Must | M3 |
| GP-03 | Each captain has a posture setting (passive, neutral, aggressive) that changes how followers treat civilians and enemies | Must | M3 |
| GP-04 | Followers are individual entities that follow their captain and act autonomously within the posture | Must | M3 |
| GP-05 | Settlements can be visited to recruit, buy, seize food and goods, or conquer | Must | M4 |
| GP-06 | Conquest transfers a settlement's allegiance; loyalty then drifts based on treatment and food security | Must | M4 |
| GP-07 | Orders to lieutenants are carried by a messenger entity and take effect only on arrival | Must | M5 |
| GP-08 | Settlements with the right workers and materials can produce new equipment (bows, catapults, boats) when a captain commissions it | Should | M5 |
| GP-09 | Rival warlords (AI) pursue the same goals using the same rules and order system as the player | Must | M5 |
| GP-10 | The world is divided into regions; the campaign is won by controlling a set fraction of a region's population | Must | M6 |
| GP-11 | Time controls: pause, 1x, 2x, 4x, 8x; orders can be issued while paused | Must | M2 |
| GP-12 | Save and load the full world state at any time | Must | M4 |
| GP-13 | Multiplayer | Later | — |

## Simulation requirements

The simulation must run 250,000 people across a full world, with up to 20,000 at full per-agent detail at once. Everything else runs in aggregate and can be promoted to full detail on demand without visible discontinuity.

### World and time

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-01 | World is a procedurally generated heightmap island, 2048 x 2048 tiles at launch, with terrain types: water, beach, grass, farmland, forest, hills, mountain, road | Must | M1 |
| SIM-02 | World generation is fully determined by a u64 seed | Must | M1 |
| SIM-03 | Simulation runs on a fixed tick (10 ticks per game-second at 1x); rendering is decoupled and interpolates | Must | M0 |
| SIM-04 | A game day lasts 4 real minutes at 1x; days have morning, work, evening and night phases | Must | M2 |
| SIM-05 | Seasons (30 days each) change crop growth, food decay, movement speed and cold exposure | Should | M4 |
| SIM-06 | Weather (clear, rain, storm, snow) varies by region and affects movement and farming | Later | — |

### Villagers

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-10 | Each villager has a generated name, age, home, workplace, occupation, allegiance, inventory and needs (hunger, rest, safety) | Must | M2 |
| SIM-11 | Occupations at launch: farmer, fisher, shepherd, woodcutter, miner, smith, baker, carter, idle | Must | M2 |
| SIM-12 | Villagers choose actions with utility AI scoring needs, job and danger; no scripted schedules | Must | M2 |
| SIM-13 | Villagers flee, hide or fight based on captain posture, local threat and loyalty | Must | M3 |
| SIM-14 | Villagers remember notable events (home burned, family killed, fed by a captain) as decaying memories that affect loyalty | Should | M5 |
| SIM-15 | Population changes through births, deaths (age, starvation, combat) and migration between settlements | Should | M4 |

### Economy

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-20 | Goods are physical items held in inventories or stockpiles: grain, fish, mutton, bread, wood, ore, tools, weapons | Must | M2 |
| SIM-21 | Production chains are data-driven recipes (inputs, outputs, workplace, worker, duration) loaded from RON files | Must | M2 |
| SIM-22 | Food decays over time; decay rate depends on type and season | Should | M4 |
| SIM-23 | Carters move surplus goods between nearby settlements along roads | Should | M4 |
| SIM-24 | Settlements track food security (days of food at current consumption), which drives loyalty and migration | Must | M4 |

### Armies and combat

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-30 | Armies move as crowds using flow fields toward a captain's goal; followers keep formation loosely | Must | M3 |
| SIM-31 | Each follower consumes food daily; unfed followers lose morale, then desert, then starve | Must | M3 |
| SIM-32 | Combat resolves per individual with simple stats (health, attack, defense, equipment) and morale-driven routing | Must | M3 |
| SIM-33 | Battles in aggregate regions resolve with a statistical model that produces the same expected outcome as per-agent combat | Must | M5 |

### Information

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-40 | Each faction holds its own knowledge model: last-known state and timestamp per settlement and army | Must | M5 |
| SIM-41 | The map shows the player's knowledge, not ground truth; stale information is visually aged | Must | M5 |
| SIM-42 | Knowledge updates via line of sight of captains and followers, scouts and arriving messengers | Must | M5 |
| SIM-43 | Messengers are physical entities that can be delayed, intercepted or killed | Should | M5 |

## Technical stack and project structure

The project is a Rust Cargo workspace on Bevy, with the simulation in its own crate that never depends on rendering. That split is the single most important structural rule: it keeps the simulation fast, testable headless and portable to a dedicated server later.

| Concern | Choice | Notes |
| --- | --- | --- |
| Language | Rust, stable toolchain, edition 2024 | Pin with `rust-toolchain.toml` |
| Engine | Bevy, latest stable (0.19 as of June 2026) | Pin an exact version; Claude Code checks crates.io before starting. [source](https://gamedev.net/news/4045-bevy-019-released/) |
| Randomness | `rand` + `rand_chacha` | Every random draw comes from a seeded ChaCha RNG resource, never `thread_rng` |
| Noise | `noise` | Terrain heightmap and moisture |
| Data files | `serde` + `ron` | Recipes, occupations, unit stats, worldgen settings |
| Save files | `serde` + `postcard`, zstd-compressed | Versioned header. Never `bincode` (unmaintained; see docs/adr/0001) |
| Debug UI | `bevy_egui` | Inspector, sim stats, overlays; dev builds only |
| Profiling | `tracing` spans + Tracy (Bevy `trace_tracy` feature) | Every system has a span |
| Benchmarks | `criterion` | Headless sim ticks per second at fixed populations |

Dependency policy: keep third-party Bevy plugins to a minimum because they lag Bevy releases. Flow fields, utility AI, spatial hashing and the LOD system are written in-house.

### Workspace layout

```
warlord-sim/
  Cargo.toml              # workspace
  rust-toolchain.toml
  CLAUDE.md
  docs/SEED.md            # this document
  docs/adr/               # architecture decision records
  assets/
    data/                 # *.ron: recipes, occupations, units, worldgen
    models/ textures/ shaders/
  crates/
    sim_core/             # components, resources, tick schedule, RNG, time
    sim_world/            # terrain, worldgen, regions, spatial grid, flow fields
    sim_agents/           # villagers, needs, utility AI, occupations
    sim_economy/          # goods, inventories, recipes, stockpiles, trade
    sim_military/         # captains, followers, orders, combat, morale
    sim_knowledge/        # faction knowledge, messengers, line of sight
    sim_lod/              # promotion/demotion between aggregate and full detail
    sim_ai/               # rival warlord strategic AI
    game_render/          # terrain mesh, instanced agents, overlays, camera
    game_ui/              # HUD, panels, orders UI, event feed
    game_app/             # binary: wires plugins, settings, save/load
    sim_headless/         # binary: runs the sim with no window, for tests and benchmarks
  tests/                  # cross-crate integration and determinism tests
  benches/
```

Every `sim_*` crate depends on `bevy_ecs`, `bevy_app` and `bevy_math` only (or Bevy with default features off), never on rendering, windowing or asset-loading crates. Only `game_*` crates use the full engine.

## Architecture

Each crate exposes one Bevy plugin; `game_app` composes them, and `sim_headless` composes only the `sim_*` plugins. The simulation advances in Bevy's `FixedUpdate` schedule; rendering reads simulation state in `Update` and never writes to it.

### Plugin rules

- Each `sim_*` plugin registers its components, resources, events and systems in named `SystemSet`s, and nothing else.
- Cross-crate communication uses Bevy events (messages) or shared components defined in `sim_core`, never direct calls into another crate's systems.
- Player and AI input reach the simulation only as `Command` events (for example `IssueOrder { captain, order }`), so the player and rival warlords use the identical code path and replays are just a recorded event stream.

### Core data model

Components are small and plain. Starting set (names are a guide; Claude Code may refine them):

```rust
// sim_core
struct SimTick(u64);                    // resource: current tick
struct SimRng(ChaCha8Rng);              // resource: the only RNG
struct FactionId(u16);
struct Position(IVec2);                 // tile coords; sub-tile offset in Motion
struct Motion { offset: Vec2, velocity: Vec2 }
struct Allegiance(FactionId);
struct DetailLevel { full: bool }       // full per-agent sim vs dormant

// sim_agents
struct Villager { name: NameId, age_days: u16, home: Entity, settlement: Entity }
struct Occupation(OccupationId);
struct Needs { hunger: f32, rest: f32, safety: f32 }
struct CurrentAction { kind: ActionKind, target: Option<Entity>, until_tick: u64 }
struct Loyalty(f32);                    // toward current allegiance, 0..1

// sim_economy
struct Inventory { slots: SmallVec<[(GoodId, u16); 4]> }
struct Stockpile { goods: HashMap<GoodId, u32> }
struct Workplace { recipe: RecipeId, workers: SmallVec<[Entity; 8]> }

// sim_military
struct Captain { faction: FactionId, posture: Posture, orders: VecDeque<Order> }
struct Follower { captain: Entity }
struct CombatStats { health: f32, attack: f32, defense: f32, morale: f32 }

// sim_world
struct Settlement { name: NameId, region: RegionId, center: IVec2 }
struct RegionAggregate { /* see Simulation LOD */ }
```

Terrain is not stored as entities. It lives in a `WorldMap` resource of flat arrays (height, terrain type, fertility, owner), chunked in 64 x 64 tiles, alongside a `SpatialGrid` resource that buckets entities per chunk for neighbor queries.

### Tick schedule

Each fixed tick runs these system sets in order; sets 3 to 6 run their systems in parallel where data access allows.

1. **Input**: drain `Command` events into captain order queues.
2. **Knowledge**: update line of sight, deliver arrived messengers, age faction knowledge.
3. **Decide**: utility AI picks actions for full-detail villagers whose current action has ended (staggered so about 1/10 of agents decide per tick).
4. **Move**: flow-field steering, separation, and position integration.
5. **Act**: work, eat, trade, recruit, fight; produce and consume goods.
6. **Needs and morale**: update needs, morale, loyalty, desertion, death.
7. **Aggregate**: tick dormant regions on a slower cadence (once per game-second).
8. **LOD**: promote or demote regions based on interest.
9. **Bookkeeping**: despawn, update spatial grid, emit events for the UI feed.

### Simulation LOD

This system is what makes 250,000 people possible. Each region is either Active (every villager is an entity with full systems) or Dormant (villagers exist only as compact records inside a `RegionAggregate`).

- **Interest**: a region is Active if any captain, army or the camera focus is inside it or within one region of it. Hysteresis (stay Active for 2 game-minutes after interest leaves) prevents thrashing.
- **Dormant model**: per settlement, occupation counts, stockpile totals, food security, average loyalty and a list of compact villager records (name, age, occupation, home, loyalty). Production and consumption are computed as rates per game-second from the same recipe data as the full sim.
- **Promotion**: spawn entities from the compact records, placed at homes or workplaces appropriate to the time of day. Promotion of one region must finish within 2 frames; spread spawning across frames if needed.
- **Demotion**: fold entities back into compact records, preserving every persistent field.
- **Consistency rule**: a region simulated Active for N days and one simulated Dormant for N days, from the same start, must land within 5% on population, food and loyalty. A test enforces this (see Non-functional requirements).

## Rendering, camera and UI

The look is a clean, low-poly 3D isometric-style world that stays readable with thousands of figures on screen. Readability beats fidelity everywhere.

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| RND-01 | Terrain renders as chunked meshes (64 x 64 tiles) from the heightmap, with vertex colors by terrain type; chunks rebuild only when changed | Must | M1 |
| RND-02 | Camera: perspective with an orbit-style rig; pan (WASD, edge scroll, drag), zoom (wheel), rotate in 90-degree steps and freely (Q/E) | Must | M1 |
| RND-03 | Agents render as GPU-instanced low-poly figures, tinted by faction and occupation; distant agents switch to billboards, then to density dots | Must | M2 |
| RND-04 | 20,000 visible agents at 60 fps on the reference machine | Must | M3 |
| RND-05 | Day and night lighting tied to sim time; seasonal palette shifts | Should | M4 |
| RND-06 | Render positions interpolate between the last two sim ticks so motion is smooth at any sim speed | Must | M2 |
| UI-01 | Captain panel: orders, posture slider, follower count, food carried, morale | Must | M3 |
| UI-02 | Settlement panel: population by occupation, stockpiles, food security in days, loyalty, owner | Must | M2 |
| UI-03 | Click any villager to see name, job, needs, current action and memories ("why is this person doing that") | Must | M2 |
| UI-04 | Map overlays: ownership, food security, loyalty, knowledge age, supply routes | Must | M4 |
| UI-05 | Event feed that surfaces notable simulation stories (famine in a village, desertion, revolt) with a click-to-jump | Must | M4 |
| UI-06 | Strategic map view at max zoom-out showing regions, settlements and known armies as icons | Should | M5 |
| UI-07 | Debug UI (egui): tick time per system set, entity counts by detail level, region LOD state, RNG seed | Must | M0 |

## Non-functional requirements

Performance budgets and determinism are requirements from day one, enforced by benchmarks and tests in CI, because they are nearly impossible to retrofit.

Reference machine: 8-core desktop CPU from 2022 or later, 16 GB RAM, a mid-range discrete GPU (roughly RTX 3060 class).

| ID | Requirement | Target |
| --- | --- | --- |
| NFR-01 | Total population simulated (active + dormant) | 250,000 |
| NFR-02 | Full-detail agents at once | 20,000 |
| NFR-03 | Fixed tick cost at NFR-02 load, 1x speed | ≤ 8 ms on the reference machine |
| NFR-04 | Headless throughput, 20,000 active agents | ≥ 300 ticks per second (30x real time) |
| NFR-05 | Frame rate during normal play | 60 fps; never below 30 fps during region promotion |
| NFR-06 | Memory at full world | ≤ 4 GB |
| NFR-07 | Save or load full world | ≤ 3 seconds |
| NFR-08 | Determinism | Same seed + same command stream → bit-identical state hash after N ticks, on the same build and platform |

### Determinism rules

- All randomness comes from `SimRng` (or per-system RNGs derived from it by a fixed key), never from `thread_rng` or time.
- Systems that run in parallel must not depend on iteration order for outcomes. Where order matters, sort by a stable key (entity spawn ID, not `Entity` bits) before processing.
- No `HashMap` iteration in simulation logic unless sorted first; prefer `BTreeMap` or `IndexMap` with fixed hashing.
- Floating point is allowed; cross-platform bit-identical results are not required (later concern for multiplayer).

### Testing requirements

- **Unit tests** in every `sim_*` crate for needs, utility scoring, recipes, combat math and flow fields.
- **Determinism test**: run 10,000 ticks twice from the same seed and command log; compare a state hash. Runs in CI.
- **LOD consistency test**: one region Active vs Dormant for 10 game-days; population, food and loyalty within 5%.
- **Scenario tests** using `sim_headless`: small hand-built maps that assert outcomes (an unfed army deserts within N days; a conquered settlement with no food revolts).
- **Benchmarks** (criterion) for NFR-03 and NFR-04; CI flags regressions over 10%.
- **CI**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --workspace`, and benchmarks on a nightly job.

## Milestone roadmap

Seven milestones take the project from an empty repo to a playable campaign; M3 is the go/no-go point for the core fantasy. Each milestone ends only when every acceptance criterion passes and the build is green.

### M0: Skeleton

Workspace, CI, plugin structure, fixed-tick loop, seeded RNG, headless binary, debug UI shell.

- [x] `cargo run -p game_app` opens a window with an empty scene and the debug panel showing tick count and tick time
- [x] `cargo run -p sim_headless -- --ticks 1000 --seed 42` prints a state hash; running it twice prints the same hash
- [x] CI runs fmt, clippy and tests; all pass

### M1: World

Procedural island, terrain meshes, camera.

- [x] A seed produces the same 2048 x 2048 island every time, with all terrain types present
- [x] Terrain renders in chunks; the camera pans, zooms and rotates smoothly
- [x] Settlements (40 to 80) are placed on suitable land with road links between neighbors

### M2: A living village

Villagers, needs, occupations, utility AI, goods and recipes, day phases.

- [ ] One region with 5,000 villagers runs a full day cycle: they go to work, produce goods, eat, return home and sleep
- [ ] Clicking a villager shows name, job, needs and current action
- [ ] Cutting off a village's farmers causes measurable hunger within 2 game-days

### M3: Armies (go/no-go)

Captains, followers, orders, posture, flow-field movement, combat, food upkeep.

- [ ] The player orders a captain with 200 followers across the region; they move as a crowd around obstacles
- [ ] Posture visibly changes how followers treat villagers
- [ ] An army without food loses morale and starts deserting within the specified days
- [ ] 20,000 visible agents at 60 fps on the reference machine
- [ ] **Go/no-go**: playtest whether marching an army through living villages is interesting. If not, revise the design before M4

### M4: Conquest and economy

Settlement conquest, loyalty, trade, food decay, seasons, overlays, event feed, save/load.

- [ ] Conquering a settlement transfers it; loyalty responds to treatment and food security
- [ ] Carters move surplus between settlements; overlays show food security and supply
- [ ] Save and load restore an identical state hash

### M5: Scale and information

Simulation LOD across the full world, faction knowledge, messengers, rival AI warlords, equipment production.

- [ ] 250,000 total population across the island within NFR budgets
- [ ] LOD consistency test passes; promotion causes no visible hitch below 30 fps
- [ ] The player's map shows only known, aged information; orders to lieutenants arrive by messenger
- [ ] Two AI warlords expand, fight each other and the player using the same command path

### M6: Campaign

Win and lose conditions, multiple regions, onboarding, audio, options, polish.

- [ ] A complete campaign can be won or lost
- [ ] A new player can learn the basics through a short guided first region

## Claude Code bootstrap

Work one milestone at a time: plan, implement, verify against the acceptance criteria, commit. The CLAUDE.md below keeps every session anchored to this spec and its rules.

### CLAUDE.md template (save at repo root)

```markdown
# Warlord Sim

Indirect-control medieval conquest simulation in Rust + Bevy.
The full spec is docs/SEED.md. Read it before any non-trivial change.
Requirement IDs (GP-, SIM-, RND-, UI-, NFR-) are stable; reference them
in commits, tests and comments.

## Current milestone
M0 (update this line as milestones complete)

## Hard rules
- sim_* crates never depend on rendering, windowing, audio or asset crates.
- The simulation changes state only in FixedUpdate. Rendering and UI only read.
- Player and AI input reach the sim only as Command events.
- All randomness comes from SimRng or RNGs derived from it. Never thread_rng.
- No HashMap iteration in sim logic unless sorted. Sort parallel results
  by stable spawn ID where order affects outcomes.
- Game data (recipes, occupations, units, worldgen) lives in assets/data/*.ron,
  not in code.
- Every new system gets a tracing span and lives in a named SystemSet.
- Write the test first for sim logic when practical.
- Pin the Bevy version. Do not upgrade Bevy or add dependencies without asking.

## Commands
- Run game:       cargo run -p game_app
- Run headless:   cargo run --release -p sim_headless -- --seed 42 --ticks 10000
- Test:           cargo test --workspace
- Lint:           cargo clippy --workspace -- -D warnings && cargo fmt --check
- Bench:          cargo bench -p sim_headless

## Definition of done for any task
1. cargo fmt, clippy and tests pass.
2. The determinism test still passes.
3. Relevant acceptance criteria in docs/SEED.md are checked.
4. Significant design decisions get a short ADR in docs/adr/.

## Build settings
Use opt-level = 1 for dev and opt-level = 3 for dependencies in dev profile
so the game is playable in debug builds.
```

### First prompts, in order

1. **Orient**: "Read docs/SEED.md and CLAUDE.md. Check crates.io for the latest stable Bevy version and confirm the version you plan to pin. Then write a plan for milestone M0 only: files to create, crates, and how you will verify each acceptance criterion. Do not write code yet."
2. **Build M0**: "Implement the M0 plan. Create the workspace and all crates as empty plugins, the fixed-tick loop, SimRng, the headless binary with a state hash, the debug panel, and a GitHub Actions CI workflow. Verify every M0 acceptance criterion and report results."
3. **Determinism harness**: "Add the determinism integration test and a criterion benchmark skeleton in sim\_headless. Make the state hash cover every simulation component."
4. **Each later milestone**: "Plan milestone MN against docs/SEED.md, listing the requirement IDs it covers and how each acceptance criterion will be tested. Wait for my approval, then implement it in small commits."

### Working tips

- Use plan mode (or ask for a plan first) at the start of every milestone; review the plan before code is written.
- Ask Claude Code to run the headless sim and benchmarks itself and report numbers, rather than trusting that performance is fine.
- After each milestone, update the "Current milestone" line in CLAUDE.md and tick the acceptance boxes in SEED.md.
- When Bevy APIs fail to compile, ask Claude Code to check the migration guide for the pinned version rather than guessing from older examples.

## Open questions and risks

The biggest risk is design, not engineering: a huge simulation that the player cannot read or influence meaningfully. M3's go/no-go exists to catch that early.

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Scale is not fun on its own | High | Go/no-go playtest at M3; invest in readability (UI-03, UI-04, UI-05) before adding scale |
| Bevy breaking changes between releases | Medium | Pin the version; upgrade only between milestones with a dedicated migration task |
| LOD promotion causes visible pops or hitches | Medium | Spread spawning over frames; consistency test; place promoted agents by time-of-day routine |
| Determinism silently breaks | Medium | Determinism test in CI from M0; stable-key sorting rule |
| Aggregate and full sims diverge | Medium | Shared recipe data and LOD consistency test |
| Scope creep | High | Requirements tagged by milestone; anything new goes to "Later" until the current milestone ships |

Open questions to decide before or during the noted milestone:

- [x] Art direction: flat-shaded low-poly, or a stylized texture look? (before M2) **Decided 2026-10-06: both, by zoom level: flat-shaded low-poly beyond a detail distance, stylized textures closer in. See docs/M2_PLAN.md and ADR 0009.**
- [ ] Should the player see and control a warlord avatar on the map, or only an abstract command view? (before M3)
- [ ] Combat depth: individual duels with equipment, or simpler morale-dominant clashes? (during M3)
- [ ] Is the campaign one large island or a sequence of islands of increasing size? (before M6)
- [ ] Final title and setting details (before M6)
