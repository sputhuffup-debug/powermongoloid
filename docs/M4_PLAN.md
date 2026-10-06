# M4 Plan: Conquest and faith

Status: **decisions made 2026-10-06; building starts after the M3 playtest says go**. No code written yet.
Spec: `docs/SEED.md` § "M4: Conquest and faith". On 2026-10-06 the economy half (town growth and styles, population, trade, seasons, decay) moved to **M4b "Living towns"**, designed in `docs/TOWNS_DESIGN.md`. Those sections remain below, marked, as input for the M4b plan.

**Depends on the M3 go/no-go playtest** (`docs/M3_PLAYTEST.md`). If the playtest changes the core loop, this plan changes with it. Nothing below is built until you've played M3 and said go.

## Requirements covered

| ID | Requirement | Where |
| --- | --- | --- |
| GP-05 | Settlements can be visited to recruit, buy, seize food and goods, or conquer | `sim_military` (conquest; the rest exists since M3) |
| GP-06 | Conquest transfers a settlement's allegiance; loyalty then drifts with treatment and food security | `sim_agents::loyalty` |
| GP-12 | Save and load the full world state at any time | `sim_core::save` + every sim crate |
| SIM-24 | Food security drives loyalty and migration (tracked since M2) | `sim_agents` |
| SIM-50 | Each settlement has a faith and a piety level | new `sim_faith` module |
| SIM-51 | Sustained high piety makes a militia member a hero (one per settlement, cooldown after death) | `sim_faith` |
| SIM-52 | Heroes are never controllable; they autonomously lead their settlement's militia | `sim_faith` + `sim_military` |
| UI-02 | Settlement panel gains owner and loyalty (placeholders since M2) | `game_ui` |
| UI-04 | Map overlays: ownership, food security, loyalty (supply routes in M4b, knowledge age in M5) | `game_render` + `game_ui` |
| UI-05 | Event feed of notable stories with click-to-jump | `sim_core::events` + `game_ui` |
| UI-08 | Faith and piety in the settlement panel; heroes distinct and explained in the inspector | `game_render` + `game_ui` |

## Acceptance criteria and how each is tested

| Criterion | Test |
| --- | --- |
| Conquering a settlement transfers it; loyalty responds to treatment and food security | **Scenario `conquest`:** an army attacks a village, breaks its militia and holds the centre, and the village changes owner. The run then branches three ways from the same save: (a) a passive garrison and a fed village, where loyalty rises; (b) an aggressive garrison that plunders, where loyalty falls; (c) a conquered village left with no food, which revolts (the SEED.md example). Each branch asserts the direction of loyalty and the revolt. |
| Save and load restore an identical state hash | **Test `save_load`:** run N ticks with armies, orders and a hero, then save and load into a fresh app; the hashes must match. Run both apps M more ticks with the same commands; the hashes must still match, which shows that caches (flow fields, paths, grids) rebuild identically. A coverage test fails if any hashed type isn't saved, mirroring `state_hash_covers_every_sim_component`. Save and load times are measured against NFR-07 (≤ 3 s; the full island arrives with LOD in M5). |
| Faith, piety and heroes (SIM-50..52) | **Scenario `divine_favour`:** a well-fed village with a shrine raises piety and gets a hero within the configured days; a hungry one doesn't. When raiders provoke it, the militia led by the hero has higher morale and kill rate than the same militia without one. Every command that names a hero is refused, and recruiting skips heroes. A hero's death starts the cooldown. |

## Design

### Ownership, conquest and loyalty (GP-05, GP-06, SIM-24)
- **Owners.** Each settlement gets an `Owner(FactionId)`; 0 means independent. Villagers' `Allegiance` follows their settlement's owner.
- **Conquest.** An `Attack` order succeeds when the defenders are broken and the army holds the village centre for a set time (`army.ron`). The village then changes owner, and the event feed reports it.
- **Garrison.** A new order, `Garrison { settlement }`: a captain and its men stay in the village, eat from its stores under their posture rules, and keep order. Owned villages feed their owner's armies without provocation, up to their surplus.
- **Loyalty.** Loyalty toward the current owner (0..1) drifts daily toward a target set by:
  - food security
  - treatment: plunder, killings and garrison posture
  - piety, which falls with desecration
  - time under the owner

  The settlement's mean loyalty is shown in the panel and drives events.
- **Revolt.** Loyalty below a threshold for a day, with the garrison weaker than the would-be militia, makes the village revolt. Its militia rises against the owner (heroes lead it), and with no garrison the village becomes independent again.
- **Granularity** is decision 1 below.

### Trade and carters (SIM-23) (moved to M4b)
- **Surplus and need.** Every so often each village works out what it can spare (food beyond `keep_days`, goods beyond its workshops' inputs) and what it lacks.
- **Carts.** A serial, SpawnId-ordered step matches surpluses to needs among neighbours on the road network and gives carters a `Deliver` job. A carter loads at its storehouse, travels the road path (cached A*), unloads through the transfer resolver, and returns with goods of equal value (decision 2).
- **Supply routes.** Each road segment keeps a decaying record of flows; it feeds the supply overlay and, later, M5's knowledge.
- **Prices.** M3's `trade_value` table moves to `economy.ron` as each good's value, so armies and carters trade on the same scale.

### Seasons, decay and cold (SIM-05, SIM-22) (moved to M4b)
- **Seasons.** The calendar already has four 30-day seasons. `economy.ron` gains per-season multipliers for crop yield (none in winter), decay (highest in summer) and walking speed (slowest in winter).
- **Decay.** Food in stockpiles and inventories decays daily by a per-good rate (bread and fish fast, grain slow). It is computed in whole units with a deterministic carry, so the state hash stays exact.
- **Cold.** In winter, households burn firewood from home stock, which woodcutters supply. Unheated villagers recover rest more slowly and lose a little health, so cold kills mostly the old and the hungry. This adds a reason to haul wood; it can be dropped (decision 4).

### Population (SIM-15) (moved to M4b)
- **Ageing and births.** Villagers age daily. Each household with an adult couple, food security above a threshold and room in the house has a seeded daily chance of a birth.
- **Deaths.** Old age, rising after 60; starvation, after days at maximum hunger (a slow health loss that already exists for soldiers); and combat, as in M3. The dead leave their house and job, as fallen militia do now.
- **Migration.** A household in a village with less than a day of food for several days moves to the most food-secure reachable neighbour with houses free. It walks there as a family, and the move shows in the event feed. No new houses are built (construction is out of scope).

### Faith, piety and heroes (SIM-50..52)
- **Faiths.** `faiths.ron` defines faiths (decision 3). Each settlement is given one at world generation, by a seeded draw per region so neighbouring villages tend to share. Each populated village gets a **shrine** building.
- **Piety (0..1, hashed, per settlement).** It rises with prayer: a new `Pray` action at the shrine, scored by the utility AI on devotion and time of day. It also rises with offerings (surplus food left at the shrine) and with feasts. A feast is held on a holy day when food security is high: it eats food and lifts piety and loyalty. Piety falls with hunger, with desecration (an aggressive army seizing, or fighting, within the shrine's radius), and slowly when neglected.
- **Divine favour.** Piety above the favour threshold for N days, with no living hero and the cooldown over, raises a hero from the village's eligible adults. The draw comes from a seeded stream, so it replays exactly. At most one living hero per settlement, and a cooldown follows a hero's death.
- **Heroes.** A `Hero { settlement }` villager with boosted stats and M3's leadership aura (morale and attack bonus, rallying routed militia).
  - **In peace,** the hero lives as a villager.
  - **When the village is provoked,** the hero musters the militia and decides with its own utility AI whether to engage, hold the village or fall back; every choice is shown in the inspector.
  - **Never controllable:** heroes can't be recruited, appointed or ordered. Command validation refuses anything that names a hero, and a test checks every command kind.
  - Whom a hero fights is decision 5.

### Save and load (GP-12)
- **Format.** ADR 0001: serde + postcard, zstd-compressed, behind a versioned header (magic bytes, format version, game build, seed). This adds the `postcard` and `zstd` crates, and I need your approval (decision 6).
- **Save registry.** Every hashed component and resource also registers save and load functions. A coverage test makes sure nothing hashed is missing from saves. Entity references are written as SpawnIds and remapped on load. Caches, indexes and grids aren't saved; they rebuild on the first tick.
- **Map.** The map's arrays are saved compressed rather than regenerated from the seed, so saves survive changes to world generation.
- **When.** Saves and loads happen only between ticks. In game: F5 quicksave, F9 quickload, and a short list of save slots in the time bar. Headless: `--save` and `--load`, so tests and replays can start from a save.

### UI and rendering (UI-02, UI-04, UI-05, UI-08)
- **Settlement panel:** owner, loyalty bar and trend, faith, piety bar and trend, the hero (if any), the garrison, and active trade.
- **Overlays** (toggled with O, or buttons):
  - ownership (faction colours)
  - food security (red to green)
  - loyalty
  - supply routes come with M4b's carters

  Territory is the area around each village; it is tinted on the terrain by a shader-free vertex-colour pass on the chunk meshes.
- **Event feed:**
  - The sim emits typed `SimEvent` messages in Bookkeeping (famine, plunder, conquest, revolt, desertion, migration, birth wave, hero risen or fallen, feast). They aren't hashed; their effects are.
  - The feed lists the latest, grouped and rate-limited, and clicking an entry moves the camera there.
  - M3's military log folds into it.
- **Heroes** get a halo, a distinct figure tint and their own banner.
- **Day and night:** the sun's angle and colour follow the clock, with a moonlit night. The seasonal palette (RND-05) comes in M4b.

### Performance
- New per-villager work, prayer and loyalty drift, runs inside the existing decision and needs systems.
- Settlement-level work (loyalty targets, trade matching, piety, events) runs once per game hour, not per tick.
- The 20,000-agent budgets from M3 (≤ 8 ms per tick, NFR-03) must hold, and the benchmarks gain `tick/year` (seasons, trade and births running).

## Decisions (answered 2026-10-06)
1. **Loyalty: per villager.** This follows SEED.md's data model; the settlement panel shows the average.
2. **No currency in M4.** Goods trade at the values in `economy.ron`, and carters bring back goods of equal value. Coin can come with M5's rival warlords.
3. **Faiths: small data-defined modifiers per faith.** This was the recommended default, and it is easy to reduce to names only.
4. **Cold exposure: a firewood need in winter.** Recommended default; it can be dropped without touching the rest.
5. **Heroes fight anyone who provokes their village, its owner included,** and lead revolts. The hero answers only to its village.
6. **`postcard` and `zstd` approved** for save files (ADR 0001).

## Out of scope for M4
- **Simulation LOD and the full 250,000-person island** (M5). Saves cover the populated region and armies now, and NFR-07 at full scale is measured in M5.
- **Faction knowledge, messengers, rival AI warlords** (M5). The scripted raiders stay.
- **Villager memories** (SIM-14, M5). Treatment works through settlement-level events for now.
- **Equipment commissions** (GP-08, M5).
- **Town growth, construction, town styles, population, trade, seasons and decay:** M4b (`docs/TOWNS_DESIGN.md`).
- **Win and lose conditions** (M6).
