# ADR 0010: Armies, orders, combat and morale

Date: 2026-10-06 · Status: Accepted · Requirements: GP-01..04, SIM-13, SIM-31, SIM-32, UI-01

## Context

M3 adds armies to a village simulation that must stay deterministic and fast. The player commands captains from above; there is no warlord unit (decided 2026-10-06). Followers act on their own within a posture. Villagers react to armies, and a settlement's militia will be led by heroes in M4 (SIM-50..52).

## Decision

**Data.** `assets/data/army.ron` holds factions, unit stats, equipment bonuses, combat and morale numbers, upkeep, formation, recruiting, threat and trade values. Rates are per day and turned into per-tick values with the calendar. It is hashed as a resource.

**People.** A soldier is a person entity with `CombatStats`, `Strike`, `Needs`, `Inventory`, `Activity` and `DecisionTrace`, the same components villagers have. A recruit stays the same entity: it keeps its `Villager` (name and home), gains `Enlisted`, `Follower` and `Equipment`, and leaves its job. Villager AI, movement and needs skip `Enlisted`. A deserter who was a villager goes home and is a villager again; other soldiers leave the simulation. Starting soldiers carry a `Soldier` name instead of `Villager`.

**Captains.** A `Captain { faction, posture, orders, goal, ... }` sits on a person entity, whose `Stockpile` is the army's baggage. Its capacity is set per head. When a captain falls, its first follower (by SpawnId) takes over the orders and baggage, and every follower loses morale. The `Factions` resource records each faction's seat, which orders will be sent from in M5.

**Commands.** Orders arrive only as `SimCommand`s (`IssueOrder`, `SetPosture`, `CancelOrders`, `AppointCaptain`), which name captains, followers and settlements by `SpawnId`. Each command is checked against the issuer: someone else's captain, an unknown settlement or a tile off the map is refused, counted and logged. Commands now carry an origin, and only player commands are recorded; the scripted raiders, standing in for rival warlords until M5, emit AI commands again when a replay runs. The rule that heroes are never controllable (SIM-52) will be enforced in the same validation step: a hero is never anyone's captain or follower.

**Orders and logistics.** Each captain works on its first order, serially and in SpawnId order. Orders that deal with a settlement (recruit, gather food, equip, trade, attack) run when the army arrives. They move goods directly between the storehouse and the baggage, which makes this the army's serial logistics step, the counterpart of the transfer resolver in ADR 0007. Posture decides how food is obtained:
- **Passive:** buys with the baggage's goods.
- **Neutral:** asks for a share, and takes a day's food when hungry.
- **Aggressive:** takes up to four days' food, and provokes the settlement.

Armies also forage this way from villages they pass.

**Combat.** The knowledge step builds a `FighterGrid` of everyone who can fight: soldiers, and militia (villagers whose AI chose `Fight`).
- Each cell keeps a bitmask of the sides in it, so a soldier skips cells holding no possible foe.
- Soldiers pick a foe and decide to follow, fight or rout in parallel, each writing only its own components.
- Blows are resolved serially in SpawnId order: damage is `(attack + weapon) x leadership x roll - (defense + tools)`, with the roll drawn from a stream seeded by the attacker and the tick.
- The fallen are removed in SpawnId order.

**Morale and upkeep.** Unfed followers lose morale. Below a threshold, an unfed or routed follower may desert, a chance rolled each tick from a seeded stream. Starvation damage begins only at saturated hunger, which comes after desertion has begun; `starving_army` checks this order. Allied deaths nearby cost morale, capped per tick so large battles don't collapse at once.

**Leadership aura.** A follower within its captain's leadership radius hits harder and recovers morale faster. M4 heroes will use the same aura.

**Villagers (SIM-13).** Each settlement's `Threat` comes from the posture and distance of nearby armies, and it is full while the settlement is provoked. It drives the `safety` need. Villagers score `Hide`, `Flee` and `Fight` with the same utility AI as everything else (ADR 0006). Fighting needs an adult with enough courage (morale plus a stable per-person spread) and a provoked settlement.

## Consequences

- Posture changes outcomes in measurable ways. `posture` asserts that seizures, harm and villager reactions order passive < neutral < aggressive.
- Military systems stay off the hot path when there are no armies. Exclusive steps run only when commands, the fallen or deserters exist, and per-soldier systems query `Enlisted`, never all villagers.
- With few factions, a soldier's hostility is a 64-bit mask. More than 32 factions would need a wider mask.
- Combat numbers are a first pass. A 1,000-against-1,000 battle produces a decisive rout within a game day; tuning continues in the playtest.
