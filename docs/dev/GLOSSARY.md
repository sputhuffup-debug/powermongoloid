# Glossary

| Term | Meaning |
| --- | --- |
| **Activity** | A person's current action (`ActionKind`), its target, its stage (travel, perform, deliver, done) and timers. The SEED.md data model calls it `CurrentAction` |
| **Allegiance** | The faction a person belongs to. Villagers follow their settlement's `Owner` (0, independent, until it changes hands); soldiers have their warlord's faction |
| **Army index** (`ArmyIndex`) | A read-only summary of every army, rebuilt each tick in Knowledge: position, flow fields, head count, food. Lets soldiers read their army in parallel |
| **Band** (detail band) | How a person or building is drawn at a camera distance: textured detail, flat low-poly, billboard or dot (`render.ron`) |
| **Band template** | Who follows the champion at the start of a run: groups of followers with their stats, weapons and tools, plus days of food (`bands` in `progression.ron`). One is drawn per run (GP-27) |
| **Baggage** | An army's food and goods: a `Stockpile` on the captain's entity |
| **Boon** | A card picked from an offer of three: the starting boon before the first tick (GP-27), dominion boons later (GP-24). Kept for the run in `RunProgress.boons` |
| **Captain** | A commander with an order queue and a posture. The player commands captains, not soldiers |
| **Champion** (`Champion`) | The player's own figure in a run: the captain of the starting band, with `Traits`. A marker on the person, never in `Captain`, so it stays with the person. Its death ends the run unless a successor is left (SIM-35, M4 step 6) |
| **Chronicle** (`Chronicle`) | The run's append-only history of notable events with their causes (UI-09). Hashed and saved; tests assert on it, never on messages |
| **Command** (`SimCommand`) | The only way input reaches the sim: player or AI, logged for replays if the player issued it |
| **Conquest** | Taking a settlement by force (GP-05). An `Attack` provokes the village and fights its militia. Once the defenders break, the army holds the centre uncontested for `conquest.hold_hours`, and `conquest::transfer` hands the settlement and its residents over (`SimEventKind::Conquest`, `FactionStats.conquests`). In a run, taking the rival's seat wins |
| **Deed** (`Deed`) | What moved a settlement's goodwill (GP-22): trade, equip, seize, plunder, provoke, killing, request or boon. Deltas per deed are in progression.ron `goodwill_deeds` |
| **DecisionTrace** | The scored options behind a person's last decision, shown in the inspector. Not hashed |
| **Drifter** | A villager who leaves a settlement that likes the player to join a player band passing near (GP-22): once a day in a run, a seeded few per settlement within progression.ron `drifters.radius` with goodwill at least `drifters.goodwill`, enlisted with `enlist_into` like a recruit (`DriftersJoined`) |
| **Enlisted** | Marks a soldier: villager AI, movement and needs skip it, and `sim_military` drives it |
| **Event feed** (`EventFeed`) | The last 256 events, chronicled or not, numbered by `seq` for the UI's feed (UI-05). Not hashed |
| **Exclusive system** | A system taking `&mut World`. Used for rare serial steps (commands, orders, casualties, desertion) |
| **`enlist_into`** | The one way a villager joins an army (GP-02, GP-22): `enlist` plus the shared bookkeeping (baggage capacity, the home settlement's population, `FactionStats.recruited`). Used by Recruit, drifters and request volunteers |
| **Faction** (`FactionId`) | `FactionId::INDEPENDENT` (0), `PLAYER` (1), `RAIDERS` (2, the scripted raiders), `RIVAL` (3, the rival warlord of a run) and `BEASTS` (4, monument guards); names and colours in `army.ron` |
| **FactionModifiers** | The per-faction sums of stat bonuses, plus a world row for everyone (run modifiers), with a breakdown of where each came from (GP-23). Rebuilt from scratch once a game hour by `sim_military::rebuild_modifiers` from held monuments, the player's boons, champions' traits and run modifiers; hashed. Consumers read `mult(faction, stat)` (1 + value) or `value` where the stat is used and never write it; with no bonuses every multiplier is exactly 1.0, so the developed world is unchanged |
| **Fighter grid** (`FighterGrid`) | Everyone who can fight, bucketed in 8-tile cells with a bitmask of sides per cell, rebuilt each tick |
| **Flow field** | A Dijkstra cost field toward a goal over a window of the map; following it downhill walks the cheap way around obstacles. Captains walk the **goal field** |
| **Forage** (wild foraging) | A `Forage { radius }` order (SIM-38): the band stands and gathers wild food from the patches around it until its baggage is full or the land is bare. Not the automatic gathering from settlements an army passes (`orders::forage`, "foraging" in the M3 code) |
| **Follower** | A soldier attached to a captain, with a formation slot |
| **Food security** | Days of food a settlement holds at its population's eating rate |
| **Game hour** | A 24th of a day: 100 ticks (`Calendar::ticks_per_hour`). Data fields ending in `_hours` use it; the UI's clock face does not |
| **Garrison** | The owner's soldiers within `garrison.radius` of a settlement's centre. The `Garrison { settlement }` order keeps an army there; it is valid only on a settlement the issuer owns. A garrison deters revolts, and is fed by owned-village supply when the village has food to spare; a hamlet's garrison lives on its own baggage |
| **Guard** (guard camp, `Guard`) | An army spawned at a monument at run setup to guard it (GP-23): a raider camp (raiders without the raider AI) or beasts, larger the farther it is from the start. Every member carries `Guard { monument }` and `NoUpkeep`; the captain has no orders, so the camp holds its tile and fights whatever comes within `engage_radius`. Raiders never hunt it. Its faction holds the monument until the camp is beaten and someone else holds the ground. Villages nearby ask for it to be cleared (a clear-the-camp request) |
| **Give** | A `Give { settlement }` order (GP-22): the army hands the food its baggage holds beyond `give_keep_days` of its need to the settlement's storehouse; what arrives is `FactionStats.donated`, and the army doesn't help itself to villages' food for the next quarter day |
| **Goodwill** (`Goodwill`) | A settlement's standing with each faction, -1..1, sorted by faction (GP-22). Deeds move it through the goodwill ledger: fair trade and requests answered raise it; seizing, plundering, provoking and killing its militia lower it, and word spreads to nearby settlements. Goodwill toward the owner is the treatment term of loyalty, and seeds loyalty when the settlement changes hands |
| **Goodwill ledger** (`GoodwillLedger`) | This tick's goodwill changes (`GoodwillChange`: settlement, faction, delta, deed), posted by deeds from serial steps and applied with their spread, sorted, by `apply_goodwill` first in `AgentSet::Standing`. Hashed |
| **Holder** (`Holder`) | Who holds a monument (GP-23), on every monument in a run: the faction (0 for nobody), since when, a contender standing there alone, and the last hour the holder's soldiers stood there. One faction's soldiers alone within `monuments.hold_radius` for `monuments.hold_hours` take it; a holder long absent and far from its lands loses it to nobody (drift). Hashed |
| **Invitation** | A settlement offering the player lordship (GP-22): an `Invitation` offer (accept or decline) posted hourly in a run when its goodwill toward the player passes `invitations.goodwill`, a player captain is near and its owner is weak. Accepting transfers it without a fight (`Cause::Invitation`, `Invited`), with loyalty seeded from goodwill; answering or letting it lapse starts a cooldown |
| **Logistics step** | `orders::run_orders`: the serial step where captains deal with settlements (recruit, food, arms, trade) |
| **Loyalty** (`Loyalty`, `SettlementLoyalty`) | A villager's loyalty to its settlement's owner, 0..1 (GP-06, SIM-24). Once a day it drifts toward a target set by food security, goodwill toward the owner and time under the owner (`loyalty` in villagers.ron); independent settlements aim at a fixed level. `SettlementLoyalty` holds each settlement's mean and minimum, and `below_since`, when its mean fell below the revolt threshold (unrest) |
| **Militia** | Villagers whose AI chose `Fight` to defend a provoked settlement (stance `Militia { against }`) |
| **Monument** (`Monument`) | A landmark the run preset's worldgen places in every region (GP-23): standing stones, a ruined fort, a holy well, a road marker, an ancient forge, a market cross, a wayshrine or a watchtower, on a site that suits its kind and away from settlements. A sim entity with a `SpawnId` and a `Holder`; holding one gives its holder a small, capped bonus (progression.ron `monuments.kinds`) |
| **Morale** | 0..1. Low morale routs, and unfed or routed men may desert |
| **NoUpkeep** | A marker on soldiers who never get hungry and never eat: monument guards (GP-23). `soldier_upkeep` leaves their hunger alone and `eat` skips them |
| **Occupation** | A villager's trade (farmer, fisher, ...), tied to a workplace kind |
| **Offer** (`Offers`) | A choice waiting for the player: the starting boon, a dominion boon, a trait, an invitation. Answered with a recorded `ChooseOffer` command, so picks replay (GP-27) |
| **Owner** (`Owner`) | The faction holding a settlement and since when (GP-05): `FactionId::INDEPENDENT` until conquest or invitation. Changed only by `sim_agents::set_owner`, which also sets the residents' allegiance and loyalty and calls off their militia |
| **Posture** | Passive, neutral or aggressive. How an army treats villages: buy, ask, or take and frighten |
| **Preset** (`WorldPreset`) | One of the two complete settings blocks in `worldgen.ron`: `developed` (the 2048-tile M1-M3 world; `WorldGenSettings::default_data()`) or `run` (the 1024-tile run island) |
| **Progress driver** | `progression::progress`: the one exclusive, hourly system in `MilitarySet::Progress` that runs settlement- and run-level steps in a fixed order (guide D7). In step 2 it runs only `conquest::revolts` |
| **Rally field** | A flow field toward the captain, over the army's extent. Followers far from their place walk it |
| **Region** (`RegionId`) | A part of the island: settlements are grouped around farthest-apart seeds and every land tile takes the region of the nearest settlement over land. Region 0 is always the central one; count them with `WorldMap::region_count()`. A region edge borders water or another region |
| **Replay** | A seed, scenario and the player's commands, plus expected hashes (RON). Re-running it must reproduce every hash |
| **Request** (`RequestBoard`) | A settlement asking the player for help (GP-22): drive off a hostile army near it, or bring food when its food security stays low (clear a guarded camp comes with monuments). Posted hourly in a run, at most one per settlement, with a deadline and a reward from its real stocks; the sim detects completion (the food delivered by `Give`, the danger gone with a player captain there), pays the nearest player captain and raises goodwill. Hashed |
| **Revolt** (rising) | An owned settlement throwing off its owner (GP-06). After a day with mean loyalty below `loyalty.revolt_threshold` (unrest), it goes independent at once if it has no garrison. If the garrison is smaller than the residents able to fight, they rise: militia provoked against the owner. A day later the rising wins if it outnumbers what is left of the garrison, or is put down, costing every resident `garrison.crushed_loyalty` (`conquest::revolts`) |
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
| **Set bonus** | An extra bonus for holding every monument of a kind in one region (at least `monuments.set_min` of them): `monuments.kinds[].set_bonus` for each such region, on top of the kind's capped per-monument bonus (GP-23) |
| **SimEvent** | A typed message reporting something notable (a conquest, an offer, the run's end) with a `Cause`. Recorded each tick, in sorted order, into the event feed and the chronicle |
| **SimSet** | One of the nine ordered phases of a tick: Input, Knowledge, Decide, Move, Act, Needs, Aggregate, Lod, Bookkeeping |
| **Size tier** (`SizeTier`, `SettlementSize`) | Hamlet, village or town: a settlement's drawn size in the run preset, with the number of people it is populated with. Developed settlements have none |
| **Slot** | A follower's place in the loose sunflower formation around its captain |
| **SpawnId** | A stable id given to every sim entity in spawn order and never reused. Everything order-dependent sorts by it |
| **Spatial grid** (`SpatialGrid`) | Moving entities bucketed in 8-tile cells, for neighbour queries and picking |
| **Stat** (`sim_core::Stat`) | A number a bonus can change: morale, defence, attack, food yield, march speed, smithing speed, trade value, loyalty drift, threat radius (flat tiles) and, from later steps, captain slots, veteran rate, desertion, forage yield, recruit chance. Append only: data files and saves name it |
| **Start spot** | Where a run's band starts: a tile in a region far from the rival's seat, away from settlements, with two or three monuments nearby, of a seeded kind (coast, forest edge, hills). Drawn by `sim_military::draw_start` |
| **State hash** | An FNV hash of every registered component and resource. Equal hashes mean equal state |
| **Stream** (RNG stream) | `SimSeed::stream(key, tick)`: an independent random generator for a system, agent and tick |
| **Strike** | A person's next blow: target, swing and cooldown. Resolved serially in SpawnId order |
| **Supply** (owned-village supply) | An army drawing food from a settlement its faction owns: up to its baggage target, at most once a day, from the storehouse's food beyond `garrison.keep_days`, never paid for and never provoking (`FactionStats.supplied`). Replaces gathering by posture there, so an aggressive army never plunders its own village |
| **Threat** | How frightening nearby armies are to a settlement (0..1), and which faction provoked it. Drives villagers' safety need |
| **Tick** | One fixed simulation step; 10 per game-second at 1x, 2,400 per day |
| **Trait** (`Traits`) | A small, named quality of a person (Brave, Forager...) with stat effects; the champion starts with one (SIM-34) |
| **Transfers** / **transfer resolver** | Agents post goods orders on themselves; `resolve_transfers` applies them in SpawnId order. The only way agents touch a building's stockpile |
| **Utility AI** | Each option is scored as weight x considerations, and the best wins (with inertia for the current action) |
| **Wild food** (`WildFood`) | The map's 16-tile patches of food a foraging band gathers (SIM-38). A patch holds `forage.per_tile` per grass or forest tile, empties as it is foraged and grows back daily. Built at run setup, elsewhere by the first `Forage` order. Hashed |
