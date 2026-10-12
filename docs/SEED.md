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
| GP-08 | Settlements with the right workers and materials can produce new equipment (bows, catapults, boats) when a captain commissions it | Should | M4b (tiers 1–3), M5 (siege) |
| GP-09 | Rival warlords (AI) pursue the same goals using the same rules and order system as the player | Must | M5 |
| GP-10 | The world is divided into regions; the campaign is won by controlling a set fraction of a region's population | Later | Grand campaign (runs are won by GP-28) |
| GP-11 | Time controls: pause, 1x, 2x, 4x, 8x; orders can be issued while paused | Must | M2 |
| GP-12 | Save and load the full world state at any time | Must | M4 |
| GP-13 | Multiplayer, with diplomacy (truces, tribute, alliances) between players and AI warlords | Later | — |
| GP-14 | Dilemma events arise from real sim state (a starving town, captured deserters) and offer a few choices with lasting effects on loyalty, morale and memory | Should | M5 |
| GP-15 | Walled settlements can be besieged: blockaded until their food runs out, or assaulted with ladders and siege equipment (GP-08) | Must | M5 |
| GP-16 | A pacing director spaces threats, calm and crises, with selectable personalities; auto-pause on chosen events and fast-forward to the next event | Must | M4 |
| GP-17 | Captains take standing orders (defend a region, patrol roads, forage within a radius, retreat rules) and owned towns take policies (rationing, conscription, levies in kind) | Must | M4b |
| GP-18 | Origins: the campaign can start as different kinds of warlord (exiled lord, bandit chief, prophet, merchant) with different men, goods and standing | Should | M6 |
| GP-19 | The player designs their banner and colours and can name captains and settlements they found or take | Should | M6 |
| GP-20 | Seeds, battles and whole campaigns can be shared as replay files and replayed exactly | Should | M6 |
| GP-21 | A run lasts about an hour on a small island and a fresh seed. It starts with the champion and a small band with no land, and ends in a win, the champion's death with no successor, or abandonment | Must | M4 |
| GP-22 | Villages hold goodwill toward each faction and post requests arising from their state; goodwill can make a village invite a lord | Must | M4 |
| GP-23 | Monuments: worldgen places typed sites, some guarded; holding one grants its holder a small bonus, capped per type | Must | M4 |
| GP-24 | Dominion boons: at 25% and 50% of a region's population held, the player picks one of three boons, kept for the run | Must | M4 |
| GP-25 | Captain slots come from population held | Must | M4 |
| GP-26 | Meta-progression between runs unlocks origins, trait choices and cards, never raw power | Should | M6 |
| GP-27 | Start draws: a starting boon picked from three, the band's make-up, and one or two run modifiers, all from the seed | Must | M4 |
| GP-28 | A rival warlord's seat grows on a clock and raids from mid-run; taking it wins the run | Must | M4 (scripted), M5 (rules-based) |
| GP-29 | A finished run leaves a record of its legends (names, epithets, deeds, fates), named weapons and the villages they saved, for the legends book of past runs; nothing in it changes a later run | Should | M4c (record), M6 (book) |
| GP-30 | In a run, the pacing director schedules small mishaps from real sim state (bad mutton, a dog at the rations, a wedding on the road, a slip in the shield wall); each is low-stakes and rate-limited, never touches the champion, a captain or a legend, and never ends a run | Must | M4c |

## Simulation requirements

The simulation must run 250,000 people across a full world, with up to 20,000 at full per-agent detail at once. Everything else runs in aggregate and can be promoted to full detail on demand without visible discontinuity.

### World and time

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-01 | World is a procedurally generated heightmap island, 2048 x 2048 tiles at launch, with terrain types: water, beach, grass, farmland, forest, hills, mountain, road | Must | M1 |
| SIM-02 | World generation is fully determined by a u64 seed | Must | M1 |
| SIM-03 | Simulation runs on a fixed tick (10 ticks per game-second at 1x); rendering is decoupled and interpolates | Must | M0 |
| SIM-04 | A game day lasts 4 real minutes at 1x; days have morning, work, evening and night phases | Must | M2 |
| SIM-05 | Seasons (30 days each) change crop growth, food decay, movement speed and cold exposure | Should | M4b |
| SIM-06 | Weather (clear, rain, storm, snow) varies by region and affects movement and farming | Should | M4b |
| SIM-07 | Fire: plundered or attacked buildings can burn and spread to neighbours; burned buildings become ruins until rebuilt | Should | M4b |

### Villagers

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-10 | Each villager has a generated name, age, home, workplace, occupation, allegiance, inventory and needs (hunger, rest, safety) | Must | M2 |
| SIM-11 | Occupations at launch: farmer, fisher, shepherd, woodcutter, miner, smith, baker, carter, idle | Must | M2 |
| SIM-12 | Villagers choose actions with utility AI scoring needs, job and danger; no scripted schedules | Must | M2 |
| SIM-13 | Villagers flee, hide or fight based on captain posture, local threat and loyalty | Must | M3 |
| SIM-14 | Villagers remember notable events (home burned, family killed, fed by a captain) as decaying memories that affect loyalty | Should | M5 |
| SIM-15 | Population changes through births, deaths (age, starvation, combat) and migration between settlements | Should | M4b |
| SIM-16 | Every settlement starts as a hamlet and grows through tiers (hamlet, village, town, city) by building on a master plan generated from its town style; it shrinks and leaves ruins when it loses people | Must | M4b |
| SIM-17 | Builders construct planned buildings from wood, plus stone from quarries for town and city buildings; construction stalls without materials or workers | Must | M4b |
| SIM-50 | Each settlement follows a faith (data-defined) and holds a piety level that rises with worship, offerings and feasts and falls with hunger, desecration and neglect | Must | M4b |
| SIM-51 | Sustained high piety brings divine favour: one of the settlement's militia becomes a hero (at most one living hero per settlement; cooldown after a hero dies) | Must | M4b |
| SIM-52 | Heroes are never controllable by the player or any AI warlord; a hero autonomously leads its settlement's militia within a radius (morale and combat bonus, rallying, engage or fall back) | Must | M4b |
| SIM-55 | In a run, every villager and soldier has one or two quirks (coward, glutton, drunkard, pious, fears geese, ...) dealt from a data deck by their own seeded stream; each nudges only its holder's behaviour, slightly, and outside a run nobody has one | Must | M4c |
| SIM-58 | A settlement saved from raiders by a named hero raises a memorial to them in its town style once it can build one; it gives small local effects (goodwill and loyalty there, recruits a little more willing, the hero's army steadier near it) and becomes a grave the town tends when the hero dies | Should | M4b |

### Economy

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-20 | Goods are physical items held in inventories or stockpiles: grain, fish, mutton, bread, wood, ore, tools, weapons | Must | M2 |
| SIM-21 | Production chains are data-driven recipes (inputs, outputs, workplace, worker, duration) loaded from RON files | Must | M2 |
| SIM-22 | Food decays over time; decay rate depends on type and season | Should | M4b |
| SIM-23 | Carters move surplus goods between nearby settlements along roads | Should | M4b |
| SIM-24 | Settlements track food security (days of food at current consumption), which drives loyalty and migration | Must | M4 |

### Armies and combat

| ID | Requirement | Priority | Milestone |
| --- | --- | --- | --- |
| SIM-30 | Armies move as crowds using flow fields toward a captain's goal; followers keep formation loosely | Must | M3 |
| SIM-31 | Each follower consumes food daily; unfed followers lose morale, then desert, then starve | Must | M3 |
| SIM-32 | Combat resolves per individual with simple stats (health, attack, defense, equipment) and morale-driven routing | Must | M3 |
| SIM-33 | Battles in aggregate regions resolve with a statistical model that produces the same expected outcome as per-agent combat | Must | M5 |
| SIM-34 | Captains, heroes and long-serving soldiers earn traits from what they live through (brave, cruel, beloved, shaken); traits change their AI and morale, and veterans gain ranks | Must | M4 (champion traits), M4c (earned traits) |
| SIM-35 | The champion ranks up from deeds and picks one of three traits per rank; once per run, a captain succeeds a fallen champion | Must | M4 |
| SIM-36 | Soldiers rank up by battles survived; ranks change stats and desertion | Must | M4 |
| SIM-37 | Equipment tiers are goods made by settlements of a sufficient tier; gear sets combat stats; bows are ranged | Must | M4b |
| SIM-38 | Wild foraging feeds a landless band at a small rate, from wild food that depletes and regrows | Must | M4 |
| SIM-39 | Every death in battle records who killed whom and how, including absurd but real ends; notable deaths are told one by one, and the rest are counted into the battle's report | Must | M4c |
| SIM-53 | Soldiers keep a service record (kills, battles, wounds, deeds) that outlives desertion and, on death, is kept for anyone told or named; they earn one epithet by rule, honourable or mocking, which says why | Must | M4c |
| SIM-54 | A few soldiers of great fame in each faction become legends: a legend steadies nearby men a little and their death shakes the army; legends gain no stats and are not heroes | Must | M4c |
| SIM-56 | A weapon that kills a notable foe (a captain, a champion, a legend, a monument's beast) gets a name and a history, and is passed on, looted, sold, taken home or lost as the sim moves it; named weapons gain no stats | Must | M4c |
| SIM-57 | Soldiers have kin (their household) and make friends and rivals by rule; kin grieve and avenge, rivals feud and make peace, and each soldier's mood is the bounded sum of its recorded causes, which the inspector explains | Must | M4c |

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
| RND-05 | Day and night lighting tied to sim time; seasonal palette shifts | Should | M4b |
| RND-06 | Render positions interpolate between the last two sim ticks so motion is smooth at any sim speed | Must | M2 |
| RND-07 | Each settlement has a town style drawn from a world building tradition (see docs/TOWNS_DESIGN.md), assigned per seed among styles that suit its site; a style sets layout and building looks, never building purpose | Must | M4b |
| RND-08 | Follow-cam on any person, time-lapse of a settlement's growth, and a photo mode | Should | M4b |
| RND-09 | Living legends are marked on the map; a cairn stands where a legend fell, and a memorial stone in their home village | Should | M4c |
| UI-01 | Captain panel: orders, posture slider, follower count, food carried, morale | Must | M3 |
| UI-02 | Settlement panel: population by occupation, stockpiles, food security in days, loyalty, owner | Must | M2 |
| UI-03 | Click any villager to see name, job, needs, current action and memories ("why is this person doing that") | Must | M2 |
| UI-04 | Map overlays: ownership, food security, loyalty, knowledge age, supply routes | Must | M4 |
| UI-05 | Event feed that surfaces notable simulation stories (famine in a village, desertion, revolt) with a click-to-jump | Must | M4 |
| UI-06 | Strategic map view at max zoom-out showing regions, settlements and known armies as icons | Should | M5 |
| UI-07 | Debug UI (egui): tick time per system set, entity counts by detail level, region LOD state, RNG seed | Must | M0 |
| UI-08 | Settlement panel shows faith and piety; heroes are visibly distinct and their inspector explains their decisions | Must | M4 |
| UI-09 | Chronicle: an automatic history of the island, short biographies of settlements and named characters, end-of-battle reports and an end-of-campaign timeline | Must | M4, M4c (biographies) |
| UI-10 | Kill lines, mishaps, mood notes, battle reports and biographies are told from phrase banks in data, in a deadpan dark voice, brutal by default and mild by choice; the choice is a viewer setting the game remembers, and it never changes the simulation | Must | M4c |

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
| NFR-09 | Saga bookkeeping (kill records, service records, legends, quirks, moods, bonds, named weapons) | At most +2% on the median (of three runs) mean tick of the `--battle 7500` clash, with and without the saga's run effects forced on (`--saga-bench`), and of the run preset (`--run --seed 7 --difficulty steady --ticks 24000`), over M4 baselines recorded in docs/m4c/; no allocation per blow |

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

Seven milestones, plus M3b "Developer handbook" and M4b "Living towns" (added 2026-10-06) and M4c "Saga" (added 2026-10-12), take the project from an empty repo to a playable campaign; M3 is the go/no-go point for the core fantasy. Each milestone ends only when every acceptance criterion passes and the build is green.

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

- [x] One region with 5,000 villagers runs a full day cycle: they go to work, produce goods, eat, return home and sleep
- [x] Clicking a villager shows name, job, needs and current action
- [x] Cutting off a village's farmers causes measurable hunger within 2 game-days

### M3: Armies (go/no-go)

Captains, followers, orders, posture, flow-field movement, combat, food upkeep.

- [x] The player orders a captain with 200 followers across the region; they move as a crowd around obstacles (`tests/army_march.rs`)
- [x] Posture visibly changes how followers treat villagers (`tests/posture.rs`; screenshots in the M3 report)
- [x] An army without food loses morale and starts deserting within the specified days (`tests/starving_army.rs`; deadline in `army.ron`)
- [x] 20,000 visible agents at 60 fps on the reference machine (run `cargo run --release -p game_app -- --stress 20000` there; the container renders in software). 2026-10-07, Ryzen 5 7600X (6 cores) and RTX 4070: 118–176 fps (frame 6–9 ms) with 20,000 agents. The first run froze for seconds when the armies clashed; after ADR 0012 the re-run's worst 5-second window during the clash was 81 fps (frame 12.5 ms, sim tick 7.2 ms average)
- [x] **Go/no-go**: playtest whether marching an army through living villages is interesting. If not, revise the design before M4 (checklist: `docs/M3_PLAYTEST.md`). 2026-10-07: everything runs well, but leading a big army should be the payoff, not the start; the design is being revised around progression (`docs/PROGRESSION_DESIGN.md`) before M4

### M3b: Developer handbook

Documentation so human developers can work alongside Claude Code. No gameplay. Plan: `docs/M3B_PLAN.md`.

- [x] Following only `docs/dev/GETTING_STARTED.md` in a fresh container builds and runs the game, headless and the tests (2026-10-06: a fresh clone built and ran headless and passed every test with its own `target/`; the game was built from the same clone but reused the shared Bevy build cache because the box had no disk for a second copy, then ran under Xvfb)
- [x] `cargo doc --workspace --no-deps` builds without warnings in CI, and every crate has an overview
- [x] Every data field is documented and every HOWTO recipe points at code that exists (both checked by tests: `tests/tests/docs.rs`)

### M4: Rise

One complete, playable hour: a roguelite run from a wandering band to the rival warlord's seat (design: `docs/PROGRESSION_DESIGN.md`, plan: `docs/M4_PLAN.md`). Conquest, loyalty, save and resume, event feed, chronicle, traits and the pacing director carry over from the earlier "Conquest and faith" plan; faith moves to M4b.

- [x] A run starts from a seed: band, starting boons, modifiers and nearby monuments come from it; the same seed gives the same start, and seeds visibly differ (`tests/run_start.rs`: twenty seeds, same-seed identity; `tests/worldgen.rs`)
- [x] A landless band survives on foraging and village requests, and starves if it does neither (`tests/wanderers.rs`)
- [x] Monuments can be taken, held and lost; their bonuses stack to the caps (`tests/monuments.rs`)
- [x] Conquest or invitation transfers a settlement; loyalty responds to treatment and food security; a starved conquest revolts (`tests/conquest.rs`, `tests/invitation.rs`)
- [x] Holding 25% and 50% of a region offers one of three boons; captain slots follow population held (`tests/dominion.rs`)
- [x] The champion ranks up and picks traits, soldiers rank up, and a captain succeeds a fallen champion once (`tests/champion.rs`)
- [x] The rival grows on its clock and raids from mid-run; taking its seat wins; a scripted bot wins most fixed seeds (`tests/rival.rs`; `tests/bot_run.rs`, ignored in the normal suite and run in release by CI: 5 of 5 seeds won on days 18-31, 2026-10-09; again 5 of 5, on days 15.5-26.3, after ADR 0018, 2026-10-12)
- [x] Save and load restore an identical state hash (`tests/save_load.rs`, which also checks NFR-07's 3 s; `every_hashed_type_is_saved`)
- [x] The chronicle tells the run with causes; the pacing director keeps runs free of long dead stretches (`tests/chronicle.rs`, `tests/director.rs`, and the quiet limit checked over every `bot_run` seed). UI-09's short biographies, and UI-04's knowledge-age and supply-route overlays, are not built in M4 (the biographies are planned for M4c)
- [ ] **Playtest:** a run takes about an hour, and two seeds feel different in their first ten minutes (playtest: `docs/M4_PLAYTEST.md`)

### M4c: Saga

Combat gets its stories, and every run its oddities, after the M4 playtest and before M4b: brutal kill lines (mild on request) where only notable deaths are told and the rest are summed up, battle reports with their moments, service records and epithets earned by deeds (honourable or mocking), absurd but real deaths, the traits soldiers earn from what they live through (SIM-34's triggers), and a few legends per faction who steady nearby men, shake their army when they fall and leave a cairn. Following the Dwarf Fortress decisions of 2026-10-12, villagers in a run have quirks, the pacing director schedules small mishaps, weapons that kill notable foes get names, and soldiers have kin, friends, rivals and moods; everything is told in one voice, deadpan dark, and rescues of villages are recorded for M4b's hero memorials. It also builds UI-09's biographies and the end screen's legends and oddities, and writes each run's legends down for M6's book, with no power carried between runs. The milestone may be split after step 8 of its plan: part 1 ("Deaths and oddities") and part 2 ("Legends and kin"). Design: `docs/SAGA_DESIGN.md`, plan: `docs/M4C_PLAN.md`.

- [ ] Every death by a blow names its killer and how it happened (armed or bare-handed, fleeing, last stand, one blow), and a rebel's kill is credited to the rebels (`tests/saga.rs`)
- [ ] Captains, the champion and legends are always told; any other death is told only if it clears its battle's rising bar, at most 10 a battle and 8 told deaths a tick; at 8x the feed shows only lines about your people or a legend (at most 8 a battle over every `bot_run` seed); a report's "more died" equals its losses minus its named dead (`tests/saga.rs`)
- [ ] Both sides' reports of a fight share its battle number and name; reports name first blood, who ran first, when the army broke, the told deaths and the best killer, and in a run an army killed to the last man still gets its report (`tests/saga.rs`)
- [ ] In a run, deeds earn one epithet by rule, and Brave, Shaken, Cruel and Beloved are earned from what a soldier lived through; each acts only through its holder (their own morale, aim and desertion, and a captain's own dealings), and a person without one, under a captain without one, behaves exactly as before (`tests/saga_traits.rs`)
- [ ] Legends rise by fame, at most three enlisted per faction; they steady men within 8 tiles, and their death shakes their army once and pauses the game for your own (`tests/legends.rs`; a `game_ui` test of the pause; `tests/bot_run.rs`: still 3 of 5 seeds won, and a player legend on at least 3 of them)
- [ ] Brutal and mild banks have the same keys; every line names its key's facts (killer, victim, place or count) and uses only its key's slots, though mild may leave out flavour such as the wound; no gendered words; the deadpan voice holds (no exclamation or question marks, no capitals, none of the winking or pitying words); feed lines fit one feed row; the tone never changes the state hash or a replay (`tests/saga_words.rs`)
- [ ] In a run, every villager and soldier has one or two quirks from the deck in `villagers.ron`, dealt from their own stream keyed by SpawnId: the same seed deals the same quirks in any order, beasts and the developed world, the skirmish and `--battle` have none, and `village_day` and `famine` pass unchanged; each quirk has its documented effect through its holder only, and a person without one behaves exactly as before (`tests/quirks.rs`)
- [ ] The pacing director schedules mishaps in a run, about one a game day on steady, each only when its condition holds in the sim and doing exactly what its line says; over every `bot_run` seed no mishap touches the champion, a captain or a legend, kills more than one ordinary soldier (and at most one a game day), takes a baggage below a day's food or delays a march with a foe near, none comes in a run's first 24 hours, and mishaps never reset the quiet clock or move the raider schedule (`tests/mishaps.rs`, `tests/bot_run.rs`)
- [ ] Mocking epithets (Twice-Fled, Turnip-Hand, the Unlucky, ...) are earned by rule and rank below every honourable one, so a deed replaces them and never the reverse; absurd deaths are told only for the ironies the sim produced, never for militia, rebel or villager victims; an answered drive-off request records its heroes, a legend first, and its line names them (`tests/saga.rs`)
- [ ] A weapon that kills a captain, a champion, a legend or a monument's beast is named (at most 2 a battle and 24 a run); it is looted, passed on, put in the baggage, sold, taken home or lost exactly as the sim moved it, and after every move it is in exactly one place (`tests/weapons.rs`)
- [ ] Kin (one household) grieve and avenge, and a soldier whose own army kills their kin takes it hard; friends and rivals form by rule and feuds end by rule; moods are bounded, expire and act only through their holder, a soldier with none behaves exactly as before, and the inspector explains each mood from its recorded causes (`tests/kin.rs`)
- [ ] The feed stays readable at 8x: over every `bot_run` seed, at most 8 deaths a battle and at most 2 mishap, weapon, kin and feud lines a game day pass the 8x filter, and no speed shows more than one mishap line every 20 real seconds (`tests/bot_run.rs`, a `game_ui` test)
- [ ] The inspector shows the biographies of soldiers and settlements (with quirks, weapons, kin and rescues), the end screen lists the run's legends, foes remembered, the fallen and the run's oddities, and cairns mark where legends fell (UI-09, RND-09)
- [ ] A finished run writes its legends record (facts and names, no sentences, with its named weapons and rescues), and no sim crate reads it (`tests/saga.rs`)
- [ ] NFR-09 holds against the M4 baselines in docs/m4c/, on `--battle 7500` with and without `--saga-bench` and on the run preset; developed, skirmish and `--battle` stdout are byte-identical to M4's; saves and determinism hold (`tests/save_load.rs`, `tests/determinism.rs`)
- [ ] **Playtest:** after a run you can name a legend and say how they fell, at least one death landed, a mishap or an odd death made you laugh or wince and none felt unfair, brutal reads grim without numbing, mild reads sober without being dry, and the deadpan reads dry, not silly (playtest: `docs/M4C_PLAYTEST.md`)

### M4b: Living towns

Settlements grow from hamlets to cities in their own town styles; population, construction, trade, seasons and food decay; equipment tiers; faith, piety and heroes (moved from M4); memorials to the heroes who saved villages, from M4c's rescue records (added 2026-10-12). Growth is tuned to a 30-day run. Design: `docs/TOWNS_DESIGN.md` (with "Hero memorials"), `docs/PROGRESSION_DESIGN.md`, `docs/SAGA_DESIGN.md` (section 14).

- [ ] Every settlement starts as a hamlet; under good conditions one grows to a village and then a town within the configured days, stalls in famine and shrinks after a sack
- [ ] Every town style produces a valid master plan on the sites it suits (no overlaps, every plot reachable), and a seed always yields the same styles and plans
- [ ] Carters move surplus between settlements; the supply overlay shows the flows
- [ ] Seasons change yields, decay and travel, and a year of play keeps villages alive without intervention
- [ ] Captains on standing orders defend, patrol and forage without new commands; plundered buildings burn and leave ruins; weather slows marches
- [ ] Follow-cam and a growth time-lapse work on any settlement
- [ ] Equipment tiers 1–3 are made by settlements of the right tier and change how soldiers fight; bows shoot
- [ ] Sustained piety raises a hero who leads the militia and obeys no one
- [ ] A village saved from raiders by a named hero (M4c's rescue record) raises a memorial to them in its town style once it can build one; while it stands it gives its small local effects (goodwill and loyalty there, recruits a little more willing, the hero's own army steadier near it), and when the hero dies it becomes a grave the town tends, with a chronicle line for each (`tests/memorials.rs`)

### M5: Scale and information

The rival plays by the player's rules, sieges and siege equipment, faction knowledge and messengers, dilemmas. Simulation LOD for a large "grand campaign" island is optional (decided 2026-10-07).

- [ ] (Optional, grand campaign) 250,000 total population across the island within NFR budgets
- [ ] (Optional, grand campaign) LOD consistency test passes; promotion causes no visible hitch below 30 fps
- [ ] The player's map shows only known, aged information; orders to lieutenants arrive by messenger
- [ ] The rival warlord expands and fights using the same command path as the player, replacing M4's script
- [ ] A walled city can be starved out by blockade or taken by assault
- [ ] Dilemma events arise from sim state, and their choices visibly change loyalty and morale

### M6: Campaign

The roguelite layer (meta-progression, origins, the legends book of past runs), onboarding, audio, options, heraldry, shareable replays, polish.

- [ ] Unlocks earned in runs carry over between runs and widen choices without adding raw power
- [ ] A new player can learn the basics through a short guided first region
- [ ] Each origin plays differently from the first day; banners and names are the player's own
- [ ] A shared replay file reproduces a campaign exactly on another machine with the same build

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
- [x] Should the player see and control a warlord avatar on the map, or only an abstract command view? (before M3) **Decided 2026-10-06: abstract command view. The warlord is the player, not a unit; the player starts with one captain (their champion) and may appoint up to 8 more (GP-01). From M5, orders travel by messenger from the player's seat (GP-07).**
- [x] Combat depth: individual duels with equipment, or simpler morale-dominant clashes? (during M3) **Decided 2026-10-06: duels with equipment, with morale-driven routing (SIM-32).**
- [x] Is the campaign one large island or a sequence of islands of increasing size? (before M6) **Decided 2026-10-07: neither; the game is roguelite runs of about an hour on small fresh islands (`docs/PROGRESSION_DESIGN.md`). A large island may return as an optional grand campaign.**
- [ ] Final title and setting details (before M6)
