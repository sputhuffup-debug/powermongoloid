# Glossary

| Term | Meaning |
| --- | --- |
| **Activity** | A person's current action (`ActionKind`), its target, its stage (travel, perform, deliver, done) and timers. The SEED.md data model calls it `CurrentAction` |
| **Allegiance** | The faction a person belongs to. Villagers are 0 (independent); soldiers have their warlord's faction |
| **Army index** (`ArmyIndex`) | A read-only summary of every army, rebuilt each tick in Knowledge: position, flow fields, head count, food. Lets soldiers read their army in parallel |
| **Band** (detail band) | How a person or building is drawn at a camera distance: textured detail, flat low-poly, billboard or dot (`render.ron`) |
| **Baggage** | An army's food and goods: a `Stockpile` on the captain's entity |
| **Captain** | A commander with an order queue and a posture. The player commands captains, not soldiers |
| **Command** (`SimCommand`) | The only way input reaches the sim: player or AI, logged for replays if the player issued it |
| **DecisionTrace** | The scored options behind a person's last decision, shown in the inspector. Not hashed |
| **Enlisted** | Marks a soldier: villager AI, movement and needs skip it, and `sim_military` drives it |
| **Exclusive system** | A system taking `&mut World`. Used for rare serial steps (commands, orders, casualties, desertion) |
| **Faction** (`FactionId`) | 0 independent, 1 the player, 2 the scripted raiders (`army.ron`) |
| **Fighter grid** (`FighterGrid`) | Everyone who can fight, bucketed in 8-tile cells with a bitmask of sides per cell, rebuilt each tick |
| **Flow field** | A Dijkstra cost field toward a goal over a window of the map; following it downhill walks the cheap way around obstacles. Captains walk the **goal field** |
| **Follower** | A soldier attached to a captain, with a formation slot |
| **Food security** | Days of food a settlement holds at its population's eating rate |
| **Logistics step** | `orders::run_orders`: the serial step where captains deal with settlements (recruit, food, arms, trade) |
| **Militia** | Villagers whose AI chose `Fight` to defend a provoked settlement (stance `Militia { against }`) |
| **Morale** | 0..1. Low morale routs, and unfed or routed men may desert |
| **Occupation** | A villager's trade (farmer, fisher, ...), tied to a workplace kind |
| **Posture** | Passive, neutral or aggressive. How an army treats villages: buy, ask, or take and frighten |
| **Rally field** | A flow field toward the captain, over the army's extent. Followers far from their place walk it |
| **Replay** | A seed, scenario and the player's commands, plus expected hashes (RON). Re-running it must reproduce every hash |
| **Rout** | A soldier whose morale broke, fleeing until it rallies |
| **Scenario** | What is set up before the first tick: `Villages`, `Skirmish` (the playtest start) or `Battle(n)` |
| **Seed** (`SimSeed`) | The u64 that fixes the world and every random stream |
| **SimSet** | One of the nine ordered phases of a tick: Input, Knowledge, Decide, Move, Act, Needs, Aggregate, Lod, Bookkeeping |
| **Slot** | A follower's place in the loose sunflower formation around its captain |
| **SpawnId** | A stable id given to every sim entity in spawn order and never reused. Everything order-dependent sorts by it |
| **Spatial grid** (`SpatialGrid`) | Moving entities bucketed in 8-tile cells, for neighbour queries and picking |
| **State hash** | An FNV hash of every registered component and resource. Equal hashes mean equal state |
| **Stream** (RNG stream) | `SimSeed::stream(key, tick)`: an independent random generator for a system, agent and tick |
| **Strike** | A person's next blow: target, swing and cooldown. Resolved serially in SpawnId order |
| **Threat** | How frightening nearby armies are to a settlement (0..1), and which faction provoked it. Drives villagers' safety need |
| **Tick** | One fixed simulation step; 10 per game-second at 1x, 2,400 per day |
| **Transfers** / **transfer resolver** | Agents post goods orders on themselves; `resolve_transfers` applies them in SpawnId order. The only way agents touch a building's stockpile |
| **Utility AI** | Each option is scored as weight x considerations, and the best wins (with inertia for the current action) |
