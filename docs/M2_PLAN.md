# M2 Plan: A living village

Status: **proposed, awaiting approval**. No code written yet.
Spec: `docs/SEED.md` § "M2: A living village".

## Requirements covered

| ID | Requirement | Where |
| --- | --- | --- |
| GP-11 | Time controls: pause, 1x, 2x, 4x, 8x; orders can be issued while paused | `game_ui` (scales Bevy's virtual time) |
| SIM-04 | A day lasts 4 real minutes at 1x (2,400 ticks), split into morning, work, evening and night | `sim_core::calendar` |
| SIM-10 | Villagers have a name, age, home, workplace, occupation, allegiance, inventory and needs (hunger, rest, safety) | `sim_agents` |
| SIM-11 | Occupations: farmer, fisher, shepherd, woodcutter, miner, smith, baker, carter, idle | `sim_agents` + `occupations.ron` |
| SIM-12 | Utility AI scoring needs, job and danger; no scripted schedules | `sim_agents::ai` |
| SIM-20 | Physical goods: grain, fish, mutton, bread, wood, ore, tools, weapons | `sim_economy` |
| SIM-21 | Data-driven recipes from RON | `sim_economy` + `recipes.ron` |
| RND-03 | Instanced low-poly figures tinted by occupation (and faction), switching to billboards and then density dots with distance | `game_render::agents` |
| RND-06 | Render positions interpolate between the last two ticks | `game_render::agents` |
| UI-02 | Settlement panel: population by occupation, stockpiles, food security, loyalty, owner | `game_ui` |
| UI-03 | Click a villager to see name, job, needs, current action and "why" | `game_ui` |

These M1 deferrals also land here: the `SpatialGrid`, and removing the M0 placeholder wanderers.

## Acceptance criteria and how each is tested

| Criterion | Test |
| --- | --- |
| One region with 5,000 villagers runs a full day cycle: they go to work, produce goods, eat, return home and sleep | **Headless scenario** `tests/tests/village_day.rs`: populate one region with 5,000 villagers and run 1 game day (2,400 ticks). It samples the population at each phase and asserts: ≥ 85% of workers are at or travelling to work during the work phase; ≥ 90% of villagers are at home asleep at night; every occupation with workers produced its goods; every villager ate at least once; nobody's hunger is above the "starving" threshold. The same run checks the state hash is reproducible. |
| Clicking a villager shows name, job, needs and current action | **Unit tests** cover picking (cursor ray to terrain to nearest villager) and the inspector text built from the components. **In the real game**, under Xvfb, `xdotool` clicks a villager and the screenshot must show the panel, as in M1. |
| Cutting off a village's farmers causes measurable hunger within 2 game days | **Scenario** `tests/tests/famine.rs`: two runs from the same seed. In one, a chosen village's farmers are set to idle at tick 0. After 2 game days, that village's mean hunger must exceed the control run's by a set margin and cross the "hungry" threshold, and its food security must fall below 1 day. Other villages must stay within the control's range, so the test proves the effect is local. |

The determinism suite, the hash coverage check and parallel-vs-serial agreement must keep passing with 5,000 villagers in the hashed state. A new benchmark, `tick/villagers/5000`, records the tick cost.

## Design

### Time (SIM-04, GP-11)
- `GameClock` is derived from `SimTick`, never stored separately: day number, tick of day, and phase. It comes from `assets/data/calendar.ron`, which sets the day length (2,400 ticks) and the phase boundaries (for example: morning 0.20, work 0.55, evening 0.15, night 0.10 of the day).
- Speed control is presentation, not simulation: the UI sets Bevy's `Time<Virtual>` relative speed (0, 1, 2, 4 or 8x). It never touches sim state, so replays are unaffected.
- Commands can still be issued while paused: they queue as `SimCommand` messages and apply on the next tick. At 8x the sim runs 80 ticks/s. The fixed-step catch-up limit will be raised so high speeds don't drop ticks; a test checks that 8x advances 8 times as many ticks as 1x.

### Data files (hard rule: game data in RON)
- `goods.ron`: each good's food value (or none), decay (unused until M4, SIM-22) and carry size.
- `recipes.ron`: workplace kind, worker occupation, inputs, outputs, and duration in ticks.
  - farm (farmer): grain
  - fishing spot (fisher): fish
  - pasture (shepherd): mutton
  - woodcutter's camp (woodcutter): wood
  - mine (miner): ore
  - smithy (smith): ore + wood → tools, or ore + wood → weapons
  - bakery (baker): grain → bread
- `occupations.ron`: each occupation's workplace kind, the share of the working population, and the terrain its workplace needs.
- `villagers.ron`:
  - population per settlement and age distribution
  - name syllables for first and family names
  - needs rates: hunger per tick, and rest lost awake and recovered asleep
  - utility weights and response curves
- All are embedded with `include_str!` like `worldgen.ron`, validated at load, and hashed as resources.

### Settlement layout (`sim_economy::layout`, deterministic, at world creation)
- **Buildings:** each populated settlement gets Building entities.
  - Houses (capacity 4–6) on free land around the green.
  - A storehouse (the settlement stockpile) at the centre.
  - A bakery and a smithy near the centre.
  - Farms on farmland tiles, fishing spots on beach and coast, pastures on grass, woodcutter camps at forest edges, mines on hills or mountains.
- **Count:** the number of each workplace follows the occupation mix and what terrain exists nearby. A village with no coast gets no fishers, and those workers are redistributed.
- **Rendering:** houses are drawn from these real entities, replacing M1's hash-placed houses.

### Villagers (`sim_agents`, SIM-10/11)
- Components follow SEED.md's core data model:
  - `Villager { name, age_days, home, settlement }`
  - `Occupation`, `Workplace` (link), `Needs { hunger, rest, safety }`
  - `CurrentAction { kind, target, until_tick }`
  - `Inventory`: a fixed `[(GoodId, u16); 4]` array, so no `smallvec` dependency
  - `Allegiance`: neutral faction in M2, conquest comes in M4
  - `DetailLevel { full: true }`
- **Households:** villagers share a home and a family name. Children (under 16 years, with a 120-day year: four 30-day seasons per SIM-05) don't work.
- **Population:** only **one region** is populated at full detail in M2: 5,000 villagers across its settlements, scaled by settlement score. Other regions stay empty until the LOD system arrives (M5). This is configurable in `villagers.ron`.

### Utility AI (SIM-12)
- **Actions:** Eat, Sleep, Work, Haul (carters: move goods from workplace output to the storehouse), GoHome, Idle (loiter near home or the green).
  - Flee, Hide and Fight arrive with threats in M3 (SIM-13). The `safety` need and its "danger" consideration exist now, so they plug in without restructuring.
- **Scoring:** each action is a product of considerations, each a response curve from data, times a weight.
  - Inputs include hunger, rest, phase of day, whether food or work is available, and travel distance.
  - The highest score wins; ties break on a fixed action order.
- **Staggering:** an agent re-decides when its action ends, or every 10th tick by `SpawnId % 10`. That's about 1/10 of agents per tick, as SEED.md's Decide set requires.
- **"Why" trace:** each decision keeps a `DecisionTrace` with the top 3 actions, their scores and their strongest considerations. The inspector shows it, which answers UI-03's "why is this person doing that". It's derived, recomputable data that the sim never reads, so it's exempted from the hash with a reason.
- **Memories:** these are SIM-14 (M5). The inspector shows an empty memories section until then.

### Movement and space
- **Paths:** M2 trips are local (home, work, storehouse, bakery). Paths are A* on tiles, reusing M1's integer router generalised into `sim_world::path`. They're cached per (from, to) building pair in a `PathCache` (a `BTreeMap`, filled deterministically in a serial system). Crowd flow fields are M3 (SIM-30).
- **Speed:** depends on terrain; roads are fastest.
- **Spatial grid:** `SpatialGrid` buckets entities per 64×64 chunk. It's rebuilt in Bookkeeping in `SpawnId` order and used for picking now and for threat queries in M3.

### Economy (SIM-20/21)
- **Stockpiles:** `Stockpile { goods: BTreeMap<GoodId, u32> }` on storehouses and workplaces. It's a `BTreeMap`, not the spec's `HashMap`, per the determinism rule.
- **Production:** a worker at their workplace advances the recipe; on completion, outputs go to the workplace's output buffer. Carters haul buffers to the storehouse. Smiths and bakers fetch inputs from the storehouse.
- **Contention:** many agents draw from one storehouse, so updates use a request pattern.
  - Agents emit take or deposit requests in parallel.
  - A single serial system sorts them by `(storehouse, SpawnId)` and applies them, so the outcome never depends on scheduling.
  - The same pattern will serve trade and looting later.
- **Eating:** a villager eats from their household's carried food or from the storehouse, preferring bread, then fish or mutton, then grain.
- **Food security:** each settlement's `FoodSecurity` is days of food at current consumption (needed for UI-02; it also feeds loyalty in M4, SIM-24). Starting stocks are about 1.5 days of food, so that cutting off farmers shows up within 2 days.
- **Starvation:** in M2, hunger saturates at "starving" without killing anyone. Deaths and migration are SIM-15 (M4).

### Rendering (RND-03, RND-06)
- **Render entities:** one per villager, mapped from sim entities in a render-side resource. Rendering never writes to sim entities (ADR 0005).
- **Three distance bands:**
  - **Near (under about 150 units):** a low-poly figure (body + head).
  - **Mid (up to about 600 units):** a camera-facing billboard quad.
  - **Far:** a per-chunk density-dot mesh, refreshed a few times a second.
- **Instancing:** one shared mesh plus one material per (occupation, faction) pair, so Bevy batches each group into instanced draws.
- **Interpolation:** a render system in `FixedFirst` snapshots each villager's position before the tick runs. `Update` interpolates between the snapshot and the current position using `Time<Fixed>::overstep_fraction`.

### UI (UI-02, UI-03, GP-11)
- **Toolkit:** player-facing panels use `bevy_ui`; egui stays the dev-only debug panel, as the spec says.
- **Time bar:** day, phase and clock, plus pause / 1x / 2x / 4x / 8x buttons. Keys: Space to pause, 1–5 to set speed.
- **Settlement panel** (click a settlement label or its storehouse): population by occupation, stockpile contents, food security in days, and loyalty and owner. Loyalty and owner show neutral placeholders until M4.
- **Villager inspector** (click a villager, picked with a terrain ray and the nearest agent in the `SpatialGrid`):
  - name, age, occupation, home, workplace, need bars
  - current action and its target
  - the decision trace
  - memories (empty until M5)

### Determinism
- **Hashing:** every new component and resource is registered with the state hash; the coverage test enforces it.
- **Parallel work:** parallel systems only do per-agent work that can't affect other agents, such as needs decay, scoring and movement integration. Anything touching shared state goes through sorted requests.
- **Replay format 3:** drops `demo_agents` and records the population settings instead, since the placeholder wanderers and `DemoAnchors` are removed.

## Commit plan (small steps)
1. **Clock:** calendar, `GameClock`, time controls, and the virtual-time catch-up fix.
2. **Data:** RON files, loaders and validation.
3. **Layout:** settlement buildings, real house rendering, and removal of the wanderers.
4. **Population:** villagers, households, names and ages, for one region.
5. **Needs and AI:** needs, utility AI and decision traces.
6. **Movement:** paths, the path cache, movement and the `SpatialGrid`.
7. **Economy:** production, stockpiles, the request resolver and food security.
8. **Rendering:** agent rendering with distance bands and interpolation.
9. **UI:** the time bar, settlement panel, villager inspector and picking.
10. **Verification:** the scenario tests (day cycle, famine), the 5k benchmark, and Xvfb screenshots.
11. **ADRs:**
    - 0006: utility AI and decision traces
    - 0007: economy and the request resolver
    - 0008: agent rendering bands and interpolation

## Performance targets for M2
- 5,000 villagers: ≤ 2 ms per tick in release on the 4-core dev box. That leaves headroom for NFR-03 (≤ 8 ms at 20,000) in M3.
- Building the population and layout when the world is created: ≤ 1 s.

## Decisions needed before starting
1. **Art direction** (SEED.md open question, due before M2). Recommendation: **flat-shaded low-poly with vertex colours**, matching M1's terrain. It's cheapest to keep readable with thousands of figures.
2. **Population scope.** Recommendation: **one region at full detail, other regions empty until M5.** The alternative is a thin population everywhere, which costs performance budget now and would be thrown away when LOD lands.
3. **Player UI toolkit.** Recommendation: **`bevy_ui` for player panels, egui for debug only**, as the spec says. egui everywhere would be faster to build but isn't shippable.

## Out of scope (later milestones)
- Fleeing, hiding and fighting (SIM-13, M3)
- Memories (SIM-14, M5)
- Births, deaths and migration (SIM-15, M4)
- Food decay and seasons (SIM-22, SIM-05, M4)
- Carters between settlements (SIM-23, M4)
- Loyalty dynamics (M4)
- Inter-region simulation (M5)
