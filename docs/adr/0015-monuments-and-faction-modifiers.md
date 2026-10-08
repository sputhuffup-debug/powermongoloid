# ADR 0015: Monuments and faction modifiers

Date: 2026-10-08 · Status: Accepted · Requirements: GP-23 (also GP-22, GP-24, GP-27, SIM-34)

## Context

M4 gives a run's island monuments that bands take and hold for small, capped bonuses ([M4_PLAN.md](../M4_PLAN.md), "Monuments"). Worldgen already places them in the run preset ([ADR 0013](0013-runs-presets-and-run-config.md)); this decision covers who holds them, who guards them, and how their bonuses, together with boons, traits and run modifiers, reach the systems that use each stat. Constraints:

- **Crate order.** Villagers' yields, smithing and loyalty are computed in sim_agents, which can't see sim_military. Whatever they read must live in sim_core or sim_agents.
- **The developed world must not change** (M4_PLAN default 6). The modifiers are read in hot, everyday code (combat, upkeep, movement, work), which also runs in the developed world and the M2/M3 tests.
- **Data hashes cover only the data text** (guide G9). Folding a bonus into `Army` or `Economy` at runtime would change behaviour without changing the hash.
- **Determinism.** Holding is decided from crowds of soldiers; guard camps are spawned from the seed.

## Decision

- **One resource, rebuilt hourly.** `sim_core::FactionModifiers` holds per-faction sums of `Stat` bonuses plus a world row for everyone, and a breakdown of every contribution for the UI. It is hashed and always present. Only `sim_military::rebuild_modifiers` writes it, from scratch, once a game hour as the last state step of the progress driver (tick 0 counts, so the start boon picked on the first tick applies at once). It sums, in a fixed order:
  1. held monuments: per faction (never faction 0) and kind, `min(count x bonus, cap)`, then a **set bonus** beyond the cap for each region where the faction holds every monument of the kind (at least `set_min` of them, so a region's lone monument is no set);
  2. the player's boons;
  3. every champion's traits, for the champion's faction;
  4. run modifiers' `Stat` effects, into the world row.
- **Consumers read at the point of use**, as `mult(faction, stat)` (1 + value) or `value` for flat stats, and never write it:
  - `Morale`: the soldiers' morale target in `soldier_upkeep`;
  - `Attack` and `Defence`: `resolve_strikes` multiplies the striker's damage and divides the damage the victim takes;
  - `MarchSpeed`: `move_captains` **and** `move_soldiers` (followers take their captain's faction), so a band doesn't string out;
  - `TradeValue`: one helper, `trade_price`, prices both what a barter offers and what it pays;
  - `ThreatRadius` (flat tiles): `update_threats` widens each settlement's radius by its owner's value;
  - `FoodYield` and `SmithSpeed`: `act::perform`, for economy.ron recipes tagged with an optional `modifier` (farming and fishing; toolmaking and weaponsmithing), read for the settlement's owner;
  - `LoyaltyDrift`: the daily drift (ADR 0014).
  Equip's passive price for arms keeps army.ron's `trade_value`: trade value is what your goods fetch, not a discount on what you buy.
- **Exact no-ops at 1.0.** Every consumer multiplies or divides by the multiplier (`x * 1.0`, `d / 1.0`) or adds a value of 0.0, which IEEE arithmetic leaves bit-identical. Yields round their fraction stochastically (stream `sim_agents::yield ^ villager` at the tick), drawing only when there is a fraction; smithing rounds `duration / mult`, exact for whole durations. A yield bonus beyond the recipe's units goes into the worker's hands over the carry limit (`add_over_capacity`), while the recipe's own units keep today's capped `add`, so even a worker carrying food produces exactly as before (guide G23). With nothing held and nothing picked, as in every developed and M2/M3 world, every multiplier is 1.0.
- **Holding** (`sim_military::monuments`). Every monument gets a hashed `Holder {faction, since, contender, last_presence}` at run setup. Hourly (slot 2 of the progress driver), monuments in SpawnId order: soldiers (not militia, not routed) within `hold_radius` are counted by faction from this tick's fighter grid. One faction alone there that isn't the holder becomes the contender; after `hold_hours` alone it takes the monument (`MonumentTaken`, `MonumentLost`, cause `Hold`). Any other faction there resets the contest. A holder whose soldiers haven't stood there for `drift_days`, and that owns no settlement within `drift_distance`, loses it to nobody.
- **Guards are ordinary armies.** At setup each monument, in SpawnId order, draws its guard (stream `sim_military::monuments::guards ^ monument`, tick 0): a raider camp (`FactionId::RAIDERS` without the `Raider` AI) or beasts (`FactionId::BEASTS`), sized `base + per_tile x distance from the start` (capped), so the camps near the start are weak. The guard faction holds the monument. Every member, captain and followers alike, carries `Guard { monument }` (so the camp survives its captain's death) and `NoUpkeep`: `soldier_upkeep` leaves their hunger alone, `eat` skips them, the logistics step never gathers for them, and raiders never hunt them. A guard captain has no orders, so the camp holds its tile and fights anything at war within `engage_radius`. A guarded monument changes hands only once its guards are gone from its radius or routed.
- **Clear-the-camp requests run on their own track.** A settlement may have a clear-the-camp request open beside a drive-off or bring-food one, with its own cooldown, so a camp nearby never stops a hungry village asking for food. Each camp with a captain that still holds its monument is named by at most one open request, from the nearest settlement within `requests.camp_radius` that may ask. It is done when the camp has no captain left, or has lost the monument (its men routed and someone held it for `hold_hours`; a guard captain has no orders, so a routed camp never walks back), and a player captain stands within `reward_radius` of the monument. Its reward is drawn from its own stream (`sim_military::requests::camp ^ settlement`), since a settlement may post it in the same hour as another request.
- **Threat radius and camps.** Monuments stand at least 52 tiles from every centre; the threat radius (45) plus the widest watchtower bonus (cap 2 and a set of 1 in each of the 4 regions) stays below that, so a guard camp never frightens a village (`shipped_monument_data_fits_the_island`).

## Consequences

- **The developed world is unchanged:** `sim_headless --seed 42 --ticks 4801 --stats-every 600` and `--skirmish --ticks 6000 --stats-every 2000` print byte-identical output before and after. Hash values changed (new hashed types and the economy.ron text); nothing is golden.
- **Run outcomes change:** guard camps (about half the monuments; 12 of 24 on seed 42) are new armies on the island; they fight bands that walk into them, and their RAIDERS camp seats the raiders' faction where it is spawned. Many settlements near camps post clear-the-camp requests at once (11 on seed 42); step 11 may narrow `camp_radius`.
- **The modifier set is small and closed:** consumers exist for the stats above; `CaptainSlots`, `VeteranRate`, `Desertion` and `RecruitChance` are summed but read only from steps 5 and 6, and `ForageYield` by wild foraging (step 3). A new stat needs a consumer that is an exact no-op at 1.0 (HOWTO "Add a modifier").
- **Set bonuses sit outside the cap,** per region, so the cap bounds only the per-monument sum. A faction can exceed a kind's cap only by completing regional sets.
- **Hand-set modifiers don't survive an hour** in a full app: tests set the data (boons, traits, holders) or run a consumer system once with a hand-built resource.
- **Tests:** `caps_and_set_bonus`, `shipped_monument_data_fits_the_island` and the monuments.rs unit tests (guards hold and never hunger; holding takes an unguarded monument and drift returns it; a cleared camp answers its request and frees the monument); `modifiers_scale_yields_and_cycles_and_are_no_ops_at_one` (sim_agents); `recipe_modifiers_are_food_yield_or_smith_speed` (sim_economy). The `monuments` scenario (`tests/tests/monuments.rs`: take an unguarded and a guarded monument, the capped sum with a set, an enemy taking it back, a camp's request done) and the run's hash coverage with a monument changing hands came in the second part of the step.
