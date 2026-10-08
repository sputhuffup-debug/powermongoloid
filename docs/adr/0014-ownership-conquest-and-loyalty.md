# ADR 0014: Ownership, conquest and loyalty

Date: 2026-10-08 · Status: Accepted · Requirements: GP-05, GP-06, GP-22, SIM-24

## Context

M4 lets warlords own settlements ([M4_PLAN.md](../M4_PLAN.md), "Conquest and loyalty"). An `Attack` that wins hands a settlement over. Owned villages feed and garrison their owner's armies. Loyalty responds to food and treatment, and a starved or mistreated conquest revolts. Several constraints shape the design:

- **Crate order.** sim_agents (villagers) can't see sim_military (armies). Villagers' allegiance and loyalty must therefore live in sim_agents, while conquest and garrisons live in sim_military.
- **Determinism.** Ownership changes touch hundreds of villagers at once and must not depend on spawn or executor order.
- **The developed world must not change** ([ADR 0013](0013-runs-presets-and-run-config.md)). Conquest and revolts are general rules, not run-only ones, and they run in every preset. The M2/M3 tests and outputs must stay as they are.
- **The coverage test only sees types that exist** (guide G5). State that appears only after a conquest would escape it.

## Decision

- **Standing lives in sim_agents** (`standing.rs`, `loyalty.rs`), always present:
  - `Owner {faction, since}`, `Goodwill` (a sorted vector, -1..1 per faction) and `SettlementLoyalty` on every populated settlement;
  - `Loyalty` (0..1) on every villager, in every preset.

  All four are hashed. Independent settlements have owner 0.
- **One transfer path.** `sim_agents::set_owner` is the only writer of `Owner`.
  - It hands the settlement's residents who aren't enlisted to the new owner, in SpawnId order: `Allegiance` and a seeded `Loyalty`.
  - The seed is `(goodwill + 1) / 2` toward the new owner, or `independent_target` for independence.
  - It calls off the settlement's provocation and restarts its unrest clock.

  sim_military wraps it in `conquest::transfer(world, s, to, cause)`. The wrapper:
  - reports `Conquest`, `Invited` or `Revolt`;
  - counts conquests;
  - seats a faction that held no land at its first settlement;
  - ends the run in a win when the player takes the rival's seat (GP-28).
- **Loyalty drifts daily toward a target** (`AgentSet::Standing`, after `AgentSet::Needs`).
  - Under an owner, the target is the weighted mean of food security, treatment and time under the owner. Treatment is goodwill toward the owner, so seizing, plundering, killing, feeding and requests all reach loyalty through goodwill (step 3).
  - Independent settlements aim at a fixed `independent_target`.
  - Each day closes `drift_per_day` of the gap, scaled by the owner's `LoyaltyDrift` modifier.
  - A serial pass then writes each settlement's mean and minimum. It sets `below_since` while an owned settlement's mean is under `revolt_threshold`.
  - `MilitarySet::Upkeep` runs after Standing, because desertions change who is enlisted.
- **Conquest is an `Attack` in three phases** (`conquest` in army.ron):
  1. March.
  2. Provoke and fight until the militia breaks.
  3. Hold. The break calls off the attacker's provocation, so the militia stands down; otherwise its half-day timer would keep raising militia through the hold. The captain must then hold the centre with no militia or enemy soldiers near for `hold_hours`.

  A hold not won within `timeout_hours` fails. Attacking your own settlement is refused.
- **Owned settlements serve their owner** (`garrison` in army.ron):
  - The owner's armies never frighten it (unless it has risen against them), never pay, plunder or provoke it, and recruit there with a loyalty bonus.
  - They draw the storehouse's food beyond `keep_days` at most once a day. A draw that finds nothing spare doesn't use up the day. This is counted as `supplied`.
  - Another warlord's village sends no recruits.
  - A new `Garrison { settlement }` order holds a settlement the issuer owns until the order is replaced or the settlement is lost.
  - Deserters go home sworn to their settlement's owner.
- **Revolts are a daily serial step** (`conquest::revolts`, third in the hourly `progression::progress` driver, `MilitarySet::Progress`). After a day of unrest:
  - **No garrison:** the settlement goes independent at once. Starving villagers can't fight, so a starved, unguarded conquest falls without a blow.
  - **A garrison smaller than the residents able to fight:** the residents rise. The settlement is provoked against its owner for `revolt_hours`, so its militia fights the garrison.
  - **A day later:** the rising wins if no garrison is left, or if the rebels still standing outnumber it. Otherwise it is put down, and every resident loses `crushed_loyalty`.

  A rebel's death shakes no soldier's morale.
- **No randomness** is drawn by any of it, and every multi-entity step runs in SpawnId order.

## Consequences

- **The developed world is unchanged.** It owns nothing, so revolts, supply, owner-aware threat and recruiting never act there. `sim_headless --seed 42` and `--skirmish` print the same statistics as before.
- **New hashed state:** the four standing components, `Order.held_since` in `Captain`, and `FactionStats.{conquests, supplied}`. Hash values changed; nothing was golden.
- **An `Attack` now takes the village** instead of ending when the militia breaks. In the skirmish the defenders broke at tick 354, and the village was taken two game hours later. `orders`' `attacking_a_village_raises_its_militia` asserts the conquest.
- **A conquest moves a landless faction's seat**, and so where `Return` leads: in a run, from the start spot to the player's first village.
- **Risings rarely beat a sizeable garrison.** Only a handful of residents take up arms against soldiers standing in their village (1-6 of about 250 against 50 in the skirmish probe). A weak garrison loses. A strong one is beaten only by hunger or by leaving.
- **`Threat.provoked_by` is a single slot.** A raid during a rising overwrites it (last writer wins), and the rising is then put down at the next check.
- **Loyalty tuning is still open.** At the day tick, a fed skirmish village holds about 0.45 days of food, so a fed conquest drifts from its 0.5 seed down to about 0.42 rather than up. The conquest scenario is expected to retune `loyalty.food_days` or the weights.
- **Tests:**
  - sim_agents unit tests for drift, aggregation and `set_owner`;
  - sim_military unit tests for `transfer` (events, seat, win, refusals) and `revolts` (freed, risen, left alone, put down);
  - the strengthened `orders` attack test.
