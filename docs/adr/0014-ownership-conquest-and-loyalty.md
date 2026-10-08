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
  3. Hold. The break calls off the attacker's provocation, so the militia stands down; otherwise its half-day timer would keep raising militia through the hold. For the same reason the attacker doesn't forage at the village it holds (an aggressive army short of food would plunder and provoke it again). The captain must then hold the centre with no militia or enemy soldiers near for `hold_hours`.

  A hold not won within `timeout_hours` fails. Attacking your own settlement is refused.
- **Owned settlements serve their owner** (`garrison` in army.ron):
  - The owner's armies never frighten it (unless it has risen against them), never pay, plunder or provoke it, and recruit there with a loyalty bonus.
  - They draw the storehouse's food beyond `keep_days` at most once a day. A draw that finds nothing spare doesn't use up the day. This is counted as `supplied`.
  - Another warlord's village sends no recruits.
  - A new `Garrison { settlement }` order holds a settlement the issuer owns until the order is replaced or the settlement is lost.
  - Deserters go home sworn to their settlement's owner, with its residents' current mean loyalty (the owner may have changed while they were away).
- **Revolts are a daily serial step** (`conquest::revolts`, third in the hourly `progression::progress` driver, `MilitarySet::Progress`). After a day of unrest:
  - **No garrison:** the settlement goes independent at once. Starving villagers can't fight, so a starved, unguarded conquest falls without a blow.
  - **A garrison smaller than the residents able to fight:** the residents rise. The settlement is provoked against its owner for `revolt_hours`, so its militia fights the garrison.
  - **A day later:** the rising wins if no garrison is left, or if the rebels still standing outnumber it. Otherwise it is put down, and every resident loses `crushed_loyalty`.

  A rebel's death shakes no soldier's morale.
- **No randomness** is drawn by any of it, and every multi-entity step runs in SpawnId order.

## Consequences

- **The developed world is unchanged.** It starts owning nothing, and until an `Attack` completes, revolts, supply, owner-aware threat and loyalty-gated recruiting never act there. Attack orders do create owned land in the developed and skirmish worlds (from tests, `--commands`/`sample_orders` and replays), as conquest is always on. Default runs issue no attacks: `sim_headless --seed 42` prints byte-identical output, and `--skirmish` prints the same numbers, except that each faction stats line now ends with the new `conquests` and `supplied` fields (`, conquests: 0, supplied: 0.0`).
- **New hashed state:** the four standing components, `Order.held_since` in `Captain`, and `FactionStats.{conquests, supplied}`. Hash values changed; nothing was golden.
- **An `Attack` now takes the village** instead of ending when the militia breaks. In the skirmish the defenders broke at tick 354, and the village was taken two game hours later. `orders`' `attacking_a_village_raises_its_militia` asserts the conquest.
- **A conquest moves a landless faction's seat**, and so where `Return` leads: in a run, from the start spot to the player's first village.
- **Risings rarely beat a sizeable garrison.** Only a handful of residents take up arms against soldiers standing in their village (1-6 of about 250 against 50 in the skirmish probe). A weak garrison loses. A strong one is beaten only by hunger or by leaving.
- **`Threat.provoked_by` is a single slot.** A raid during a rising overwrites it (last writer wins), and the rising is then put down at the next check.
- **Loyalty is tuned on the day tick's food security.** `FoodSecurity.days` counts the storehouse and homes but not farm stock, so at the day tick, when the drift runs, a fed village holds only about 0.45-1 day. With `loyalty.food_days` at 1.0, a fed conquest scored about 0.5 on food and drifted from its 0.5 seed down to about 0.42. At **0.5** a fed village scores (nearly) full food, so a fed conquest grows loyal, while a starved one still aims at about 0.16, below `revolt_threshold`. `garrison.keep_days` (1.0) stays at least `food_days`, so supply never costs a village its food term; sim_military's `shipped_data_loads` checks it.
- **The conquest scenario** (`tests/tests/conquest.rs`, run island of seed 42) plays the M4_PLAN criterion. The champion's band of ten takes the nearest hamlet (56 people) at tick 1603. From that shared prefix:
  - **Passive garrison, fed:** mean loyalty 0.50, 0.58, 0.64, 0.68 at the next three day ticks.
  - **Plundered:** the band leaves and a raiding band plunders the hamlet and moves on. The raid costs the hamlet its reserve and half a day's work, and it can't feed itself back: food falls to 0.05 days, and loyalty to 0.56, 0.47, 0.38.
  - **Starved, no garrison:** the hamlet revolts at tick 7200, 2.33 days after the conquest.
- **Harsh treatment by the owner reaches loyalty only through goodwill.** An owner's army never plunders its own village, so the M4_PLAN's "aggressive garrison that plunders" is played by a raiding band, whose harm reaches loyalty through food. Food outweighs treatment (`w_food` 0.55, `w_goodwill` 0.3): a fed village under an owner it hates still aims at about 0.55. Since M4 step 3 deeds move goodwill (see below); the weights are unchanged, and the playtest may revisit them if harsh owners must lose fed villages.
- **A small village rarely has food to spare:** a hamlet's storehouse holds well under a day, so its garrison lives on its own baggage (the scenario picks the start boon with food).
- **Tests:**
  - sim_agents unit tests for drift, aggregation and `set_owner`;
  - sim_military unit tests for `transfer` (events, seat, win, refusals), `revolts` (freed, risen, left alone, put down, and a rising that beats a weak garrison), the Garrison order, owner-aware threat, supply, recruiting, equipping, deserters, and an aggressive attacker leaving the village it holds alone;
  - the strengthened `orders` attack test;
  - the `conquest` scenario above, whose starved branch is also compared single- against multi-threaded, and `run_state_hash_covers_every_sim_component`, whose run now includes a conquest and a garrison.

## Addendum: goodwill deeds and their spread (M4 step 3, GP-22)

- **One write path.** Goodwill changes only through `sim_agents::GoodwillLedger`, a hashed queue of `GoodwillChange {settlement (SpawnId), faction, delta, cause: Deed}`. sim_military posts deeds from its serial steps (`orders::goodwill_deed`, deltas from progression.ron `goodwill_deeds`): fair trade and paid-for arms +, a hungry neutral army's seizure -, plunder -, a fresh provocation -, each militiaman killed - (toward the faction he fought), a request answered + (step 3b). The "Known" start boon posts its own amount as `Deed::Boon`; `RunProgress.pending_goodwill` is gone.
- **Applied once a tick, order-free.** `apply_goodwill` runs first in `AgentSet::Standing` (only when the ledger has entries). It sorts the ledger by (settlement, faction, deed, delta bits), then, visiting settlements in SpawnId order, sums each change in full at its own settlement and `goodwill.spread_fraction` of it at every other populated settlement whose centre is within `goodwill.spread_radius` (villagers.ron), adds each sum once (clamped to -1..1), and clears the ledger. Push order, spawn order and the executor never change the result (`spread_is_order_independent`). Deeds posted after Standing (the hourly progress driver) apply the next tick.
- **The developed world behaves as before.** Raiders' plunder and killings now move goodwill toward the raiders in the developed and skirmish worlds, so their hashes change, but loyalty reads goodwill only under an owner, and nothing there is owned unless an `Attack` completes. A conquest's loyalty seed, `(goodwill + 1) / 2`, now reflects the attack itself: provoking the village and killing its militia lower goodwill toward the attacker before it takes the village.
- **Measured (conquest scenario, seed 42).** The attack provokes the hamlet once and kills four of its militia, so goodwill toward the player is -0.18 (`provoke` -0.1, `killing` -0.02 each) and loyalty is seeded at 0.41 instead of 0.5. The fed garrison still grows loyal (0.52, 0.59, 0.64); the plundered hamlet falls to 0.34; the starved one sinks below the threshold at the first day tick (0.30) and revolts at tick 4800, 1.33 days after the conquest instead of 2.33, so the scenario now asserts one to three days.

## Addendum: requests, invitations and drifters (M4 step 3, GP-22)

- **Run-only, hourly, serial.** Three sub-steps of the progress driver, in this order after `wild::regrow`: `drifters::draw` (daily), `requests::step`, `invitations::step`. Each returns at once outside a run, visits settlements and captains in SpawnId order, and draws only from new streams (`sim_military::requests ^ settlement` at the posting tick, `sim_military::drifters ^ captain` at the day tick), so the developed and skirmish worlds are untouched.
- **Addressed to the player.** Requests, invitations and drifters concern only `FactionId::PLAYER`: only the player answers offers and requests in M4. Settlements owned by another warlord (and the rival's seat) never ask or invite.
- **Requests are detected, not claimed.** `RequestBoard` (hashed, always present) holds at most one open request per settlement and per-settlement bookkeeping. BringFood is posted on **`FoodSecurity.days`** (storehouse and homes, the measure loyalty reads), not `spare_food` (storehouse only, which dips to nothing as villagers fetch food home), after `requests.low_samples` low hourly samples in a row, so the work day's dip doesn't flap requests. Food delivered by the player's `Give` (the value `move_food` actually moved) is credited to the open request; the request is done at the next hourly sample with the need met. DriveOff is done once no army at war with the player has been within the threat radius for `requests.clear_hours` **and a player captain stands near**; if none does, the danger passed without the player and the request fails. Rewards are drawn when posting from the settlement's real stocks and paid, as far as stocks and baggage allow, to the nearest player captain within `requests.reward_radius`; completion posts `Deed::Request`. A request that ends either way starts a cooldown.
- **Invitations are offers.** A liked settlement with a player captain near and a weak owner (independent, or low loyalty and a garrison smaller than the player's soldiers near) posts an `Invitation` offer with an expiry. Accepting goes through `conquest::transfer` with `Cause::Invitation`, so the residents' loyalty is seeded from their goodwill and an `Invited` event is chronicled. Accepting, declining or lapsing sets a per-settlement cooldown in `RunProgress.invite_cooldowns`. The rival's seat never invites: it is won by conquest.
- **One way to join an army.** `orders::enlist_into(world, villager, captain)` holds the bookkeeping recruiting had inline (enlist, baggage capacity, the settlement's population, `FactionStats.recruited`). Recruit, drifters and request volunteers all use it, with recruiting's candidate list (working-age civilians, idle first, by SpawnId). Recruiting's results are unchanged.
