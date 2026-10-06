# M3 Plan: Armies (go/no-go)

Status: **built 2026-10-06; awaiting the reference-machine stress run and the go/no-go playtest** (`docs/M3_PLAYTEST.md`).

## Changes from the plan while building
- **Followers use a rally field toward their captain, not the goal field.** With the goal field, followers on another route around a lake overtook the captain and never rejoined (ADR 0011).
- **Neutral, when hungry, takes one day's food even from a village's reserve.** Asking alone got nothing from villages with under a day spare.
- **Aggressive armies take at most four days of food** (`gather_target_days`). Uncapped, raiders emptied whole villages.
- **Captains hold their ground in battle.** Losing a captain costs 15% morale (was 30%), and morale lost to nearby deaths is capped per tick: large battles used to collapse in a few ticks.
- **Baggage capacity is per head.** A fixed 4,000 units couldn't feed a 1,000-man army.
- **The game opens on the skirmish start by default;** `--villages` gives the M2 start. `--stress N` stages a battle so that about N agents run.
- **The scripted raiders raid villages when short of food and otherwise hunt the nearest enemy army.**
Spec: `docs/SEED.md` § "M3: Armies". M3 is the go/no-go point: if marching an army through living villages isn't interesting, the design changes before M4.

## Requirements covered

| ID | Requirement | Where |
| --- | --- | --- |
| GP-01 | The player starts with one captain (their champion) and may appoint up to 8 more; abstract command view, so the warlord is the player, not a unit | `sim_military` |
| GP-02 | Orders: move to, attack settlement, recruit, gather food, trade, equip, scout, return | `sim_military::orders` |
| GP-03 | Posture: passive, neutral or aggressive, changing how followers treat civilians and enemies | `sim_military` + `sim_agents` |
| GP-04 | Followers are individual entities acting autonomously within the posture | `sim_military::followers` |
| SIM-13 | Villagers flee, hide or fight based on posture, local threat and loyalty | `sim_agents` (new actions) |
| SIM-30 | Armies move as crowds on flow fields; loose formation | `sim_world::flow` + `sim_military` |
| SIM-31 | Followers eat daily; unfed followers lose morale, then desert, then starve | `sim_military::upkeep` |
| SIM-32 | Per-individual combat (health, attack, defense, equipment) with morale-driven routing | `sim_military::combat` |
| RND-04 | 20,000 visible agents at 60 fps on the reference machine | `game_render` (measured on your hardware) |
| UI-01 | Captain panel: orders, posture slider, follower count, food carried, morale | `game_ui` |

## Acceptance criteria and how each is tested

| Criterion | Test |
| --- | --- |
| The player orders a captain with 200 followers across the region; they move as a crowd around obstacles | **Scenario `army_march`:** a captain with 200 followers is ordered across the populated region, with a lake or mountain in the straight-line path. Checks: at least 95% arrive within a time limit; nobody stands on water or mountains; the army's spread stays within a loose-formation radius throughout; the order arrives only as a `SimCommand`; the run replays to the same hash. |
| Posture visibly changes how followers treat villagers | **Scenario `posture`:** the same army passes the same village three times, once per posture. Passive: no food seized, no villager harmed, and villagers mostly carry on. Aggressive: food seized, villagers flee or fight, and the panel and event log show it. Neutral sits between the two. The test asserts the counts order passive < neutral < aggressive. Xvfb screenshots show villagers scattering from an aggressive army. |
| An army without food loses morale and starts deserting within the specified days | **Scenario `starving_army`:** an army with no food and no village to forage from. Checks: morale falls from day 1; the first desertion happens within the limit set in `army.ron` (proposed: by the end of day 2); starvation damage only after desertions have begun. A fed control army in the same run doesn't desert. |
| 20,000 visible agents at 60 fps on the reference machine | This can't be measured here, because the container renders in software. I'll add a `--stress 20000` mode to `game_app` that fills view with agents and logs frame times. **You run it on the reference machine** (8 cores, RTX 3060 class) and send me the numbers. Headless NFR-03 and NFR-04 at 20,000 agents are benchmarked here, and optimised until they pass on 8 cores. |
| **Go/no-go playtest** | A playable start: your first captain with about 50 followers at the edge of the populated region, 2–3 days of food, and a hostile captain's band nearby (driven by scripted commands, since real rival AI is M5). **You play it**, and I'll provide a short checklist of what to look for. |

## Design

### Factions and captains (GP-01, GP-02, GP-03)
- **`Factions` resource:** faction 0 is "independent" (settlements). Faction 1 is the player. In M3, faction 2 is a scripted hostile band for testing and the playtest.
- **`Captain`** follows SEED.md: `{ faction, posture, orders: VecDeque<Order> }`.
- **Abstract command view:** there is no warlord unit; you command your captains from above. You start with one captain, your champion, and the structure allows appointing up to 8 more (`AppointCaptain` is a command). Losing a captain hurts that army's morale; losing them all doesn't end the game in M3. Win and lose conditions are M6.
- **Your seat:** in M5, orders travel from your seat by messenger (GP-07). M3 records a seat (your starting camp) so that hook exists.
- **Orders arrive only as `SimCommand::IssueOrder { captain, order }`,** with the queue rules `Replace` and `Append`. This keeps the player and future AI warlords on one code path and keeps replays working.
  - `MoveTo(tile)`, `Return`.
  - `Recruit(settlement)`: villagers join as followers. Idle adults come first, then workers. A posture-dependent willingness check uses food and safety now; loyalty is added in M4.
  - `GatherFood(settlement)`: passive buys (barters army goods); neutral asks, then seizes if the army is hungry; aggressive seizes.
  - `Equip(settlement)`: takes weapons and tools from the storehouse, with the same posture rules.
  - `Attack(settlement)`: fights its defenders. Transferring the settlement to the attacker is conquest, which is M4 (GP-05/06).
  - `Trade(settlement)`: barters surplus goods for food at food-value rates. Currency and real trade are M4.
  - `Scout(tile)`: moves there and back. Its value arrives with faction knowledge in M5.

### Followers (GP-04, SIM-31)
- **Follower components:** `Follower { captain }`, `CombatStats { health, attack, defense, morale }`, and `Equipment` (weapons add attack, tools add a little defense). Followers carry food in their inventory, plus an army baggage share.
- **Recruits stay the same entity:** a recruited villager keeps its `Villager` identity (name and home), so its village notices the loss. It gains `Follower` and drops its job. Villager AI skips followers.
- **Follower utility AI:** the same scoring machinery as villagers (ADR 0006), with these actions:
  - FollowCaptain (loose formation slot)
  - Eat (from carried or army food)
  - Rest (when the army halts)
  - Forage or Seize (posture-gated)
  - Fight (engage a nearby enemy)
  - Rout (flee when morale breaks)
  - Desert (walk home when morale is very low)
- **Upkeep:** each follower needs food daily. While unfed, morale drops by a set amount per day. Below a desertion threshold, desertion becomes a daily chance, drawn from a seeded stream. When hunger saturates, health drops. All numbers live in `army.ron`.

### Crowd movement (SIM-30)
- **Flow fields:** `sim_world::flow` builds a Dijkstra integration field toward a captain's goal. It covers a window around the army and its goal (not the whole map), uses the same terrain costs as villager movement, and treats water as impassable. Fields are cached per (goal, window) and rebuilt when the map changes. Building one is serial and deterministic.
- **Steering:** each follower combines the field gradient, a formation slot behind the captain (loose, by index), and separation from neighbours found through `SpatialGrid`. The steering math is per-agent and parallel; neighbour reads are read-only within the tick.
- **Captains:** the captain walks the field at the army's slowest member's pace.

### Combat (SIM-32)
- **Engagement:** hostile units within a few tiles engage.
- **Damage:** each attacker picks its nearest enemy (ties by SpawnId) and rolls damage from a stream seeded by tick and attacker. Damage is `attack x (1 + weapon bonus) x roll − defense`.
- **Resolution:** damage requests are applied serially in SpawnId order, the same pattern as the M2 transfer resolver.
- **Morale:** it drops with nearby casualties, outnumbering, hunger and losing the captain, and rises with victories and the captain nearby. Below the rout threshold, a unit flees from enemies. Routed units regroup or desert.
- **Villagers (SIM-13):** they gain Flee, Hide (indoors at home) and Fight actions, scored from the `safety` need. Safety drops near hostile or aggressive armies, scaled by posture. Fight requires an adult with high morale; M4 adds a loyalty weight.
- **Combat depth (decided):** duels with equipment. Weapons and tools from the smithy change damage and defense, so the M2 economy matters in war.

### UI (UI-01) and rendering (RND-04)
- **Selecting and ordering:**
  - Click your captain or a follower to select the army.
  - Right-click ground for `MoveTo`; right-click a settlement for a context menu (attack, recruit, gather food, trade, equip).
  - Hotkeys for the common orders.
- **Captain panel:** order queue, posture slider (passive, neutral, aggressive), follower count, food carried (in days), morale bar, and equipment summary.
- **Visuals:** soldiers render through the M2 bands, tinted by faction. Captains get a banner. Combat gets simple swing and impact feedback. Routed units render desaturated.
- **Speed:** profile with Tracy (`--features tracy`), and reduce per-entity work in render sync (change detection; update far bands at a lower rate) for RND-04.

### Performance (NFR-03, NFR-04)
At 20,000 villagers, a tick costs 5.1 ms on this 4-core box. Targets on the 8-core reference machine are ≤ 8 ms per tick (NFR-03) and ≤ 3.3 ms headless (NFR-04, 300 ticks/s). Planned work:
- skip work for agents whose state can't change
- cut allocations in `decide`
- precompute per-settlement summaries (food, haul jobs) once per tick, so they aren't recomputed per villager

Benchmarks are added for armies too (`tick/army/2000`).

## New feature (requested 2026-10-06): faith, piety and heroes, built in M4

Scheduled for **M4**, next to loyalty, since both are settlement moods with the same inputs and both use the M4 event feed. SEED.md now lists:

| ID | Requirement |
| --- | --- |
| SIM-50 | Each settlement follows a faith (data-defined in `faiths.ron`) and holds a **piety** level (0..1). Piety rises with worship (a shrine building and villagers' prayer actions), offerings of surplus food, and feasts. It falls with hunger, desecration (an aggressive army seizing from or fighting near the shrine) and neglect. |
| SIM-51 | When a settlement's piety stays above a favour threshold for a set time, its gods favour it: one of the settlement's soldiers becomes a **hero**. At most one living hero per settlement, with a cooldown after a hero dies. |
| SIM-52 | A hero is **never controllable** by the player or any AI warlord: no command can target it. It autonomously leads the settlement's units within a radius. It raises their morale and combat strength, rallies routing units, and decides when to engage or fall back. |
| UI-08 | The settlement panel shows faith and piety. Heroes are visibly distinct, and their inspector explains their decisions like any villager's. |

How it fits the existing design:
- **The settlement's soldiers** are its militia: villagers who take up arms to defend it (the SIM-13 Fight action, organised). **M3 builds the militia**, so heroes have someone to lead in M4.
- **A hero leads only its own settlement's militia (decided).** Villagers you recruit from that settlement stay under your captain, so a hero never commands your units.
- **Hero behaviour** is the follower AI plus a leadership aura (morale and attack bonus, rally) and its own decisions about engaging and falling back. **M3 builds the leadership aura for captains**, so heroes reuse it in M4.
- **Determinism:** piety is hashed state, and a hero's emergence is drawn from a seeded stream, so it replays exactly.
- **Commands:** the "never controllable" rule is enforced where commands are validated. An order targeting a hero, or a hero's units, is rejected, and a test checks it.

## Decisions (answered 2026-10-06)
1. **Warlord:** abstract command view. You command captains from above; no warlord unit.
2. **Combat depth:** duels with equipment, with morale-driven routing.
3. **Faith and heroes:** built in M4, next to loyalty. M3 builds the militia and leadership aura they rely on.
4. **Who a hero leads:** only its own settlement's militia.

## Out of scope for M3
- Faith, piety and heroes (SIM-50 to 52, UI-08; M4). M3 builds the militia and leadership aura they need.
- Conquest and allegiance transfer (GP-05/06, M4)
- Real trade and currency (M4)
- Faction knowledge and messengers (M5)
- Rival AI warlords (GP-09, M5); M3 uses a scripted hostile band
- Statistical combat for dormant regions (SIM-33, M5)
- Equipment production commissions (GP-08, M5)
