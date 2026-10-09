# M4 Plan: Rise

Status: **built 2026-10-09**, awaiting the playtest (`docs/M4_PLAYTEST.md`). Decisions as built: ADRs 0013-0017; step notes: `docs/m4/`.
Design: `docs/PROGRESSION_DESIGN.md` (approved 2026-10-07 with its defaults). Spec: `docs/SEED.md` § "M4: Rise".

M4 delivers **one complete, playable hour**: a run from a wandering band of ten to taking the rival warlord's seat, on a fresh seed each time. The earlier "Conquest and faith" plan was put on hold by the M3 playtest. Its conquest, loyalty, save, chronicle, traits and pacing sections are kept here; faith moves to M4b.

## Requirements covered

| ID | Requirement | Where |
| --- | --- | --- |
| GP-21 | A run: about an hour, small island, fresh seed; starts with a landless band; ends in a win, a loss or abandonment | `sim_core::run`, worldgen `run` preset, `game_ui` new-game and end screens |
| GP-27 | Start draws: starting boon (pick one of three), band make-up, run modifiers | `sim_military::progression` |
| GP-28 | The rival warlord's seat grows on a clock and raids from mid-run; taking it wins | `sim_military::rival` (scripted; rules-based in M5) |
| GP-22 | Goodwill per settlement and faction; village requests from real state; invitations | `sim_agents::standing` |
| GP-23 | Monuments: placed by worldgen, guarded, small capped bonuses | `sim_world` (placement), `sim_military::monuments` |
| GP-24 | Dominion boons at 25% and 50% of a region's population | `sim_military::progression` |
| GP-25 | Captain slots from population held | `sim_military::orders` (command validation) |
| GP-05, GP-06 | Conquest transfers a settlement; loyalty drifts with treatment and food | `sim_military` (conquest), `sim_agents::loyalty` |
| GP-12 | Save and resume | `sim_core::save` + every sim crate |
| GP-16 | Pacing director: raiders, requests and rival bands spaced by tension; auto-pause; fast-forward to the next event | `sim_military::director` |
| SIM-24 | Food security drives loyalty | `sim_agents::loyalty` |
| SIM-34, SIM-35, SIM-36 | Traits; champion ranks, trait picks and succession; veterans' ranks | `sim_military::progression` |
| SIM-38 | Wild foraging feeds a landless band | `sim_military::upkeep` |
| UI-02, UI-04, UI-05, UI-09 | Settlement owner and loyalty; overlays; event feed; chronicle and run summary | `game_ui`, `game_render` |

**Not in M4:** faith, piety and heroes (SIM-50..52) and equipment tiers (SIM-37, GP-08) move to M4b. Town growth, seasons and trade stay in M4b. The rules-based rival, knowledge and sieges are M5. Meta-progression and origins are M6.

## Acceptance criteria and how each is tested

| Criterion | Test |
| --- | --- |
| A run starts from a seed: the band, starting boons, modifiers and nearby monuments come from it; the same seed gives the same start, and different seeds visibly differ | **Test `run_start`:** generate runs for 20 seeds. Each has 4 regions, all settlements populated, a reachable rival seat, 2–3 monuments within a day's march of the start, and three distinct boon offers. Two runs of one seed hash identically. Across the 20 seeds, start terrain, band make-up and modifiers each take at least three different values. |
| A landless band survives on foraging and requests, and starves if it does neither | **Scenario `wanderers`:** a band of 10 with 2 days of food that forages and answers a "bring food / drive off raiders" request is fed and growing at day 5. The same band ordered to stand still on rock starts deserting by `first_desertion_within_days`. |
| Monuments can be taken, held and lost; bonuses stack to their caps | **Scenario `monuments`:** the band takes an unguarded monument by holding it, and a guarded one by beating its guard. `FactionModifiers` shows exactly the summed bonus, capped at the type's cap. An enemy holding it longer takes it back, and the bonus goes. |
| Conquest or invitation transfers a settlement; loyalty responds to treatment and food security; a starved conquest revolts | **Scenario `conquest`** (from the earlier plan): attack, break the militia, hold the centre; the owner changes. It then branches from one save: a passive garrison with a fed village (loyalty rises); an aggressive garrison that plunders (loyalty falls); a village left without food (revolts). **Scenario `invitation`:** a village whose goodwill passes the threshold offers lordship; accepting it transfers the village with loyalty seeded from goodwill. |
| Holding 25% and 50% of a region offers one of three boons; captain slots follow population held | **Test `dominion`:** transfer settlements by command until a region crosses 25%. Exactly one offer of three appears; picking one applies it; crossing 25% again after losing land gives no second offer. Captain slots equal `floor(held / per_captain)`, capped at 8. Appointing beyond the slots is refused. |
| The champion ranks up and picks traits; soldiers rank up; succession works once | **Test `champion`:** deeds earn rank and a seeded offer of three traits; the pick applies. Soldiers who survive battles rank up with the configured stat gains. Killing the champion with a captain alive makes that captain champion (once); killing the next champion ends the run in a loss. |
| The rival grows on its clock and raids from mid-run; taking its seat wins | **Scenario `rival`:** with no player action, the rival's garrison grows on schedule and its first raid leaves within the configured window. Taking the seat ends the run in a win. **Bot test `bot_run`:** a scripted player (take monuments, take weak settlements, recruit, then march) wins at least 3 of 5 fixed seeds within 40 game days, headless. This guards the run's balance as the data changes. |
| Save and load restore an identical state hash | **Test `save_load`** (from the earlier plan): run with armies, a run in progress and pending offers; save; load into a fresh app; the hashes match, and they still match after M more ticks with the same commands. A coverage test fails if any hashed type isn't saved. |
| The chronicle tells the run, with causes; the pacing director avoids dead stretches | **Test `chronicle`:** a bot run's chronicle contains a conquest, a monument taken, a battle report and the run's end, each naming its cause. **Director test:** in bot runs, no stretch longer than the configured quiet limit passes without a notable event, and fast-forward stops at the next one. |
| **Playtest:** a run takes about an hour, and two seeds feel different in their first ten minutes | You play two or three runs (checklist `docs/M4_PLAYTEST.md`, written with the build). |

## Design

### The run (GP-21, GP-27)
- **Run config.** A run is its `seed` plus a `RunConfig`: difficulty (the director's personality), and in M6 the origin and unlocks. The config goes into the replay header next to the seed, and replays of runs reproduce exactly.
- **Island.** A `run` preset in `worldgen.ron`: a 1024-tile island, 4 regions and about 25 settlements. Every settlement is populated, with a seeded size: mostly hamlets (20–60 people) and villages (60–200), a few towns (200–400), and the rival's seat as the largest. That's about 4,000–5,000 people. Today's 2048-tile, one-region setup stays as the `developed` preset for stress tests, benchmarks and the M2/M3 tests.
  - Population today fills one region; it's extended to fill every settlement at its drawn size.
- **Start draws** (from `SimSeed::stream`):
  - The start spot is a passable tile at the edge of a region away from the rival, where terrain varies by seed.
  - The band comes from templates in `progression.ron` (hunters, farmhands, deserters with spears...), each giving a different mix of stats, gear and food.
  - The champion starts with one trait.
  - Run modifiers: one or two from a deck in `progression.ron`, using only systems that exist in M4. Examples: lean year (−15% crop yield), bumper harvest (+15%), rats in the stores (−10% stores), restless roads (raiders come sooner), sellswords (a band for hire near the start), and a stronger or weaker rival.
- **Starting boon.** Three offers drawn before the first tick; the pick is a `SimCommand` (`ChooseOffer`), so it replays.
- **End of a run.**
  - **Win:** the rival's seat changes owner to you.
  - **Loss:** the champion dies with no successor.
  - **Abandon:** from the menu.

  The sim records a `RunOutcome` and stops advancing. The UI shows the run summary.

### The wandering band (GP-22, SIM-38)
- **Foraging.** A new `Forage { radius }` order: the band stops and gathers food from wild tiles (grass and forest; rates in `progression.ron`). Each 16-tile patch keeps a wild-food level that foraging depletes and that regrows daily (hashed). A band can't sit on one spot forever, so it has to keep moving.
- **Goodwill.** Each settlement keeps goodwill toward each faction, in −1..1 (hashed; a small vector, sorted by faction). It changes when you:
  - complete requests (+)
  - trade or buy fairly (+)
  - seize food, plunder or kill villagers (−)

  A fraction of each change spreads to settlements within a radius, so word travels. When a settlement is taken, villagers' loyalty starts from its goodwill toward the new owner.
- **Requests.** Once per game hour, a serial step in SpawnId order checks each settlement's state and may post a request. Three kinds in M4:
  - **Drive off** raiders or beasts threatening it (threat already exists in M3)
  - **Bring food** when it has less than `request_food_days` of food
  - **Clear the camp** at a nearby guarded monument

  A request has a deadline and a reward (food, goods, volunteers, goodwill) drawn from the settlement's real stocks. It shows on the map and in the feed. Completion is detected by the sim, not claimed: the raiders gone, the food delivered by the `Trade` or a new `Give { settlement }` order, the camp cleared.
- **Invitation.** Goodwill above `invite_goodwill`, the band within the radius, and the settlement unowned or its owner weak: the settlement offers lordship (an offer, like boons). Accepting transfers it without a fight.
- **Drifters.** A daily seeded draw adds a few volunteers to a band near settlements with goodwill. Survivors of a settlement taken by raiders may also join. They are real villagers, enlisted through M3's recruit path.

### Monuments (GP-23)
- **Placement.** Worldgen places about 6 per region (`worldgen.ron`) on passable land away from settlement centres. Types are drawn by site: a holy well near water, a watchtower on hills. Monuments are sim entities (`Monument { kind, region }`, `Holder(FactionId)`), spawned with `spawn_sim`.
- **Types in M4.** Every bonus uses a system that already exists:

  | Type | Bonus per monument |
  | --- | --- |
  | Standing stones | +1% morale |
  | Ruined fort | +1% defence |
  | Holy well | +1% crop and fish yield in owned settlements |
  | Road marker | +1% march speed |
  | Ancient forge | +1% smithing speed |
  | Market cross | +1% trade value |
  | Wayshrine | +1% loyalty drift (piety in M4b) |
  | Watchtower | +1 tile threat-warning radius for owned settlements (sight in M5) |

- **Guards.** A guarded monument spawns a camp:
  - raider camps: M3 raiders that stay put
  - beasts: a raider variant with no food needs and no plunder

  Strength scales with distance from the start spot (`progression.ron`).
- **Taking and holding.** A monument changes holder when a faction's fighters alone stand within its radius for `hold_hours`. Unguarded monuments far (`drift_distance`) from any of your settlements drift back to neutral after `drift_days` without your men nearby. Both run in a serial step, in SpawnId order.
- **Bonuses.** `FactionModifiers` (hashed, one per faction) is rebuilt once per game hour from held monuments, set bonuses, boons, the champion's traits and run modifiers. Each type is capped (`progression.ron`, default +10%). The systems that use a stat read their faction's multiplier:
  - morale and defence: in combat and upkeep
  - yields: in recipe output
  - march speed: in the goal-field walk
  - trade value: in `run_orders`
  - loyalty: in its drift

### Conquest and loyalty (GP-05, GP-06, SIM-24)
Carried over from the earlier M4 plan, which already had your decisions:
- **Owners.** `Owner(FactionId)` per settlement (0 = independent). Villagers' allegiance follows it.
- **Conquest.** `Attack` succeeds when the militia is broken and the army holds the centre for `conquest_hours`.
- **Garrison.** A `Garrison { settlement }` order. Owned villages feed their owner's armies up to their surplus.
- **Loyalty, per villager (decision 2026-10-06).** It drifts daily toward a target set by food security, treatment, time under the owner, and the Wayshrine modifier.
- **Revolt.** Below a threshold for a day, with the garrison weaker than the would-be militia, the village rises and becomes independent.

### Dominion and captain slots (GP-24, GP-25)
- **Dominion.** Each region's population held is tallied hourly. Crossing 25% or 50% for the first time posts an offer of three boons. The offer is drawn from the boon deck, weighted by what you hold: farms, a smithy, the region's largest settlement. Boons are kept for the run (default). Each is a modifier, a cap raise or a capability, such as +1 captain slot or veterans ranking up faster.
- **Captain slots.** `floor(population held / per_captain)` (default 300), plus boons, capped at 8. `AppointCaptain` is refused beyond the slots. Your first village gives your first lieutenant.

### Champion, traits and veterans (SIM-34, SIM-35, SIM-36)
- **Traits** (`progression.ron`): each has a trigger (for captains and soldiers), small effects on stats or AI weights, and a chronicle phrase. Examples: Brave, Beloved, Feared, Forager, Quartermaster, Duelist, Shaken.
- **Champion ranks.** Deeds earn rank points: battles survived, monuments taken, settlements taken or invited into, requests completed. Each rank posts an offer of three traits, and you pick one (`ChooseOffer`).
- **Succession.** When the champion dies, the captain with the highest rank, ties broken by SpawnId, becomes champion and keeps half the traits (rounded down, in pick order). This happens once per run. The next champion's death ends the run.
- **Veterans.** A battle is a fighting episode of the army that ends with no foe engaged for an hour. Each survivor who struck or was struck gets +1 battle. Ranks Recruit, Soldier, Veteran and Elite come at 1, 3 and 6 battles (`army.ron`), each adding attack, defence and morale and lowering desertion. Elites get names in the chronicle.

### The rival (GP-28)
- **Seat and lands.** At world creation the rival owns the largest town in the region farthest from your start, plus that region's other settlements. It is a faction with an `Army`, scripted like M3's raiders: its commands carry `CommandOrigin::Ai` and aren't recorded.
- **Clock** (`progression.ron`, by difficulty):
  - The seat's garrison grows by recruiting from its lands every few days.
  - Its defence bonus rises in steps; it stands in for walls until M5's sieges.
  - From about day 10 (around minute 20 at the expected speeds), the director sends rival bands to raid your lands and contest your monuments.
- **Win.** Taking the rival's seat by conquest wins the run. The rest of its lands become independent.

### Pacing director (GP-16)
From the earlier plan, now also the run's conductor:
- It watches tension: threats near you, food, recent fights, and time since the last notable event.
- It spaces raider bands, requests, rival raids and quiet stretches, and makes sure the first few minutes bring a request or a raider band.
- Personalities (steady, chaotic, harsh) are set in `director.ron` and chosen at the start of a run. Its choices are seeded and replay exactly.
- In the UI: auto-pause on chosen event kinds, and a "fast-forward until something happens" button.

### Save and resume (GP-12)
From the earlier plan, with your approval of `postcard` and `zstd`:
- A versioned header carries the magic bytes, format version, build, seed and `RunConfig`.
- Every hashed type registers save and load functions, and a coverage test checks it.
- Entity references are saved as SpawnIds. Caches rebuild on the first tick.
- One save slot per run (default): it saves on exit and resumes from the menu. F5 and F9 quicksave and quickload stay for development.

### Events, chronicle and run summary (UI-05, UI-09)
- **Event feed.** Typed `SimEvent`s go to the feed, rate-limited and grouped; clicking one jumps the camera there. M3's military log folds into it. New events include:
  - monuments taken
  - requests posted and completed
  - offers waiting
  - conquest and revolt
  - rank-ups
  - rival raids
  - the run's end
- **Chronicle.** An append-only, saved history with causes ("Oakworth invited Brand's band in, after they drove off the raiders on day 4"). After each battle, a report gives the sides, losses, the turning point and the traits earned.
- **Run summary.** How the run ended, its timeline from the chronicle, and its numbers: days, settlements held, monuments, battles, veterans. M6 will add the legends book of past runs.

### UI and rendering
- **New-game screen:** seed (random or typed), difficulty, and the starting boon (three cards). The modifiers drawn are shown.
- **Run bar** (beside the time bar):
  - the day
  - your share of each region
  - the rival's strength and next step on its clock
  - waiting offers (boons, traits, invitations), which open as cards
- **Monuments:** landmark meshes per type, generated in `game_render::art` like the buildings. Hovering shows the type, bonus and holder, and there's an overlay.
- **Overlays:** ownership, goodwill or loyalty, food security, and monuments.
- **Panels:**
  - The settlement panel gains owner, goodwill, loyalty and requests.
  - The captain panel gains the champion's rank and traits, and captain slots.
  - The inspector shows a soldier's rank and traits.
- **End screen:** the run summary, with "New run" and "Same seed again".

### Data
- **New `assets/data/progression.ron`:** band templates, boon and modifier decks, monument types, bonuses, caps, guards and hold times, dominion thresholds, captain slots per population, champion ranks and traits, foraging rates, goodwill and request tuning, and the rival's clock by difficulty.
- **New `assets/data/director.ron`:** the director's personalities.
- **`worldgen.ron`:** the `run` and `developed` presets, monument placement, and settlement size draws.
- **`army.ron`:** veteran ranks, conquest and garrison tuning.

Every field is documented in `docs/dev/DATA.md` (the docs test enforces it), and the handbook, glossary and architecture schedule are updated with each system.

### Performance
- A run's island is cheaper than today's 20,000-agent stress world: about 5,000 people and a few hundred soldiers.
- New settlement-level work (requests, goodwill, dominion, monuments, modifiers) runs hourly or daily in serial steps, not per tick.
- The M3 budgets still hold for the `developed` stress world: ≤ 8 ms per tick (NFR-03), 60 fps (RND-04).
- New benchmark: `tick/run_island` (a bot run's mean and worst tick).

## Build order

1. **Run foundations.** `run` preset, populating every settlement by drawn size, `RunConfig` in replays, start draws, `ChooseOffer` and the offer queue, outcome and stop. Test `run_start`.
2. **Ownership, conquest, garrison, loyalty, revolt.** Scenarios `conquest` and `invitation` (invitation after step 3).
3. **Band life:** foraging, goodwill, requests, drifters. Scenario `wanderers`.
4. **Monuments and `FactionModifiers`.** Scenario `monuments`.
5. **Dominion boons and captain slots.** Test `dominion`.
6. **Champion, traits, veterans, succession.** Test `champion`.
7. **The rival and the director.** Scenario `rival`, director test.
8. **Save and resume.** Test `save_load`.
9. **Events, chronicle, run summary.** Test `chronicle`.
10. **UI and rendering:** new-game and end screens, run bar, cards, monuments, overlays, panels.
11. **Bot run and tuning.** `bot_run` test, the benchmark, then a playtest checklist and your playtest.
12. **Docs:** ADRs (runs and run config; monuments and modifiers; save format follows ADR 0001), handbook, SEED.md criteria.

Steps 1–9 are sim work, testable headless. Step 10 needs the game, and step 11 needs both.

## Defaults chosen (change any)

1. **Island:** 1024 tiles, 4 regions, about 25 settlements, about 5,000 people. The 2048-tile world stays as the `developed` preset for stress tests.
2. **Monument bonuses in M4** use only existing systems (table above). The Wayshrine affects loyalty until faith arrives in M4b, and the Watchtower affects threat warning until sight arrives in M5.
3. **Requests:** three kinds in M4 (drive off, bring food, clear the camp). Escorting and guarding the harvest need M4b's carters and seasons.
4. **No physical walls yet:** the rival's seat gets a stepped defence bonus. Walls and sieges stay in M5.
5. **Balance test:** the scripted bot must win 3 of 5 fixed seeds within 40 days. If tuning makes this flaky, we loosen the bar rather than fudge the data.
6. **The M2 and M3 tests keep the `developed` preset** and today's tuning, so nothing already guarded changes.
