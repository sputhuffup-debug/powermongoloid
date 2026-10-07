# Glossary

| Term | Meaning |
| --- | --- |
| **Activity** | A person's current action (`ActionKind`), its target, its stage (travel, perform, deliver, done) and timers. The SEED.md data model calls it `CurrentAction` |
| **Allegiance** | The faction a person belongs to. Villagers are 0 (independent); soldiers have their warlord's faction |
| **Army index** (`ArmyIndex`) | A read-only summary of every army, rebuilt each tick in Knowledge: position, flow fields, head count, food. Lets soldiers read their army in parallel |
| **Band** (detail band) | How a person or building is drawn at a camera distance: textured detail, flat low-poly, billboard or dot (`render.ron`) |
| **Band template** | Who follows the champion at the start of a run: groups of followers with their stats, weapons and tools, plus days of food (`bands` in `progression.ron`). One is drawn per run (GP-27) |
| **Baggage** | An army's food and goods: a `Stockpile` on the captain's entity |
| **Boon** | A card picked from an offer of three: the starting boon before the first tick (GP-27), dominion boons later (GP-24). Kept for the run in `RunProgress.boons` |
| **Captain** | A commander with an order queue and a posture. The player commands captains, not soldiers |
| **Champion** (`Champion`) | The player's own figure in a run: the captain of the starting band, with `Traits`. A marker on the person, never in `Captain`, so it stays with the person. Its death ends the run unless a successor is left (SIM-35, M4 step 6) |
| **Chronicle** (`Chronicle`) | The run's append-only history of notable events with their causes (UI-09). Hashed and saved; tests assert on it, never on messages |
| **Command** (`SimCommand`) | The only way input reaches the sim: player or AI, logged for replays if the player issued it |
| **DecisionTrace** | The scored options behind a person's last decision, shown in the inspector. Not hashed |
| **Enlisted** | Marks a soldier: villager AI, movement and needs skip it, and `sim_military` drives it |
| **Event feed** (`EventFeed`) | The last 256 events, chronicled or not, numbered by `seq` for the UI's feed (UI-05). Not hashed |
| **Exclusive system** | A system taking `&mut World`. Used for rare serial steps (commands, orders, casualties, desertion) |
| **Faction** (`FactionId`) | `FactionId::INDEPENDENT` (0), `PLAYER` (1), `RAIDERS` (2, the scripted raiders), `RIVAL` (3, the rival warlord of a run) and `BEASTS` (4, monument guards); names and colours in `army.ron` |
| **Fighter grid** (`FighterGrid`) | Everyone who can fight, bucketed in 8-tile cells with a bitmask of sides per cell, rebuilt each tick |
| **Flow field** | A Dijkstra cost field toward a goal over a window of the map; following it downhill walks the cheap way around obstacles. Captains walk the **goal field** |
| **Follower** | A soldier attached to a captain, with a formation slot |
| **Food security** | Days of food a settlement holds at its population's eating rate |
| **Game hour** | A 24th of a day: 100 ticks (`Calendar::ticks_per_hour`). Data fields ending in `_hours` use it; the UI's clock face does not |
| **Logistics step** | `orders::run_orders`: the serial step where captains deal with settlements (recruit, food, arms, trade) |
| **Militia** | Villagers whose AI chose `Fight` to defend a provoked settlement (stance `Militia { against }`) |
| **Monument** (`Monument`) | A landmark the run preset's worldgen places in every region (GP-23): standing stones, a ruined fort, a holy well, a road marker, an ancient forge, a market cross, a wayshrine or a watchtower, on a site that suits its kind and away from settlements. A sim entity with a `SpawnId`; holding one gives a small, capped bonus (M4 step 4) |
| **Morale** | 0..1. Low morale routs, and unfed or routed men may desert |
| **Occupation** | A villager's trade (farmer, fisher, ...), tied to a workplace kind |
| **Offer** (`Offers`) | A choice waiting for the player: the starting boon, a dominion boon, a trait, an invitation. Answered with a recorded `ChooseOffer` command, so picks replay (GP-27) |
| **Posture** | Passive, neutral or aggressive. How an army treats villages: buy, ask, or take and frighten |
| **Preset** (`WorldPreset`) | One of the two complete settings blocks in `worldgen.ron`: `developed` (the 2048-tile M1-M3 world; `WorldGenSettings::default_data()`) or `run` (the 1024-tile run island) |
| **Rally field** | A flow field toward the captain, over the army's extent. Followers far from their place walk it |
| **Region** (`RegionId`) | A part of the island: settlements are grouped around farthest-apart seeds and every land tile takes the region of the nearest settlement over land. Region 0 is always the central one; count them with `WorldMap::region_count()`. A region edge borders water or another region |
| **Replay** | A seed, scenario and the player's commands, plus expected hashes (RON). Re-running it must reproduce every hash |
| **Rival** (rival warlord) | The run's opposing warlord (`FactionId::RIVAL`). Its **seat** is drawn at the start of a run, and taking it wins the run. From M4 step 7 it holds its region's settlements, grows on a clock and raids from mid-run (GP-28), scripted: its commands are `SimCommand::ai` and aren't recorded |
| **Rout** | A soldier whose morale broke, fleeing until it rallies |
| **Run modifier** | One or two cards drawn at the start of a run that change its rules for everyone: a lean year, rats in the stores, a stronger rival... (`modifiers` in `progression.ron`, GP-27) |
| **RunProgress** | What a run has drawn and picked so far: start spot, rival's seat, band, run modifiers, boons. Hashed; all defaults outside a run |
| **Run** | One game from a landless band to a win, a loss or abandonment, on a fresh seed (GP-21). `RunState` holds its config and outcome; when it ends, the clock stops |
| **Run gate** | Every `SimSet` runs only while `run_in_progress` holds. When a run ends (`RunState::finish`, made final by `settle_outcome`), that tick completes and every later tick does nothing: `SimTick` and the state hash freeze, and commands are no longer applied. Outside a run the gate is always open |
| **RunConfig** | What sets up a run besides the seed: its `Difficulty`. Stored in replay headers |
| **Scenario** | What is set up before the first tick: `Villages`, `Skirmish` (the playtest start), `Battle(n)` or `Run(config)` (a run on the run island). `sim_headless::set_up_scenario` sets each one up, for headless runs and the game alike |
| **Seat** (rival's seat) | `RunProgress.rival_seat`: the run island's largest settlement, always in the top size tier and strictly larger than any other; the rival warlord holds it, and taking it wins the run (GP-28) |
| **Seed** (`SimSeed`) | The u64 that fixes the world and every random stream |
| **SimEvent** | A typed message reporting something notable (a conquest, an offer, the run's end) with a `Cause`. Recorded each tick, in sorted order, into the event feed and the chronicle |
| **SimSet** | One of the nine ordered phases of a tick: Input, Knowledge, Decide, Move, Act, Needs, Aggregate, Lod, Bookkeeping |
| **Size tier** (`SizeTier`, `SettlementSize`) | Hamlet, village or town: a settlement's drawn size in the run preset, with the number of people it is populated with. Developed settlements have none |
| **Slot** | A follower's place in the loose sunflower formation around its captain |
| **SpawnId** | A stable id given to every sim entity in spawn order and never reused. Everything order-dependent sorts by it |
| **Spatial grid** (`SpatialGrid`) | Moving entities bucketed in 8-tile cells, for neighbour queries and picking |
| **Stat** / **FactionModifiers** | A stat a bonus can change (morale, food yield, march speed...) and the per-faction sums of those bonuses. Read at the point of use; exactly 1.0 with none |
| **Start spot** | Where a run's band starts: a tile in a region far from the rival's seat, away from settlements, with two or three monuments nearby, of a seeded kind (coast, forest edge, hills). Drawn by `sim_military::draw_start` |
| **State hash** | An FNV hash of every registered component and resource. Equal hashes mean equal state |
| **Stream** (RNG stream) | `SimSeed::stream(key, tick)`: an independent random generator for a system, agent and tick |
| **Strike** | A person's next blow: target, swing and cooldown. Resolved serially in SpawnId order |
| **Threat** | How frightening nearby armies are to a settlement (0..1), and which faction provoked it. Drives villagers' safety need |
| **Tick** | One fixed simulation step; 10 per game-second at 1x, 2,400 per day |
| **Trait** (`Traits`) | A small, named quality of a person (Brave, Forager...) with stat effects; the champion starts with one (SIM-34) |
| **Transfers** / **transfer resolver** | Agents post goods orders on themselves; `resolve_transfers` applies them in SpawnId order. The only way agents touch a building's stockpile |
| **Utility AI** | Each option is scored as weight x considerations, and the best wins (with inertia for the current action) |
