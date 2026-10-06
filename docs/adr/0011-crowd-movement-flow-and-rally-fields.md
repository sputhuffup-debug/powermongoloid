# ADR 0011: Crowd movement with flow fields and rally fields

Date: 2026-10-06 · Status: Accepted · Requirements: SIM-30, NFR-03, NFR-04

## Context

Armies of hundreds must move as loose crowds around lakes and mountains (SIM-30) without an A* search per soldier per order. Movement must be deterministic and parallel.

## Decision

**Flow fields.** `sim_world::flow` builds an integer Dijkstra field toward a goal tile over a window of the map. It uses the villagers' terrain costs and slope penalty, water is impassable, and it never cuts corners past impassable tiles. Fields are pure functions of the map, goal and window. `FlowCache` keeps up to 64, evicting the least recently used, and clears on map changes. It is exempt from the hash: the orders step asks for every field it needs every tick, so a missing field is rebuilt identically and the cache can never change outcomes.

**Two fields per army.**
- The **goal field** covers the army and its goal. The captain walks it at 85% of terrain speed, and slows when more than an eighth or a third of the army straggles.
- The **rally field** points toward the captain. It covers the army's extent plus a margin, and is rebuilt when the captain has moved 6 tiles, plus every 120 ticks to take in strays.

Followers far from their place descend the rally field, not the goal field. The first version used the goal field, and followers on a different route around a lake overtook the captain and never rejoined.

**Steering.** Each follower has a slot in a sunflower disc around the captain, which keeps the formation loose whichever way the army faces.
- **Slots:** renumbered in SpawnId order every 50 ticks, and only for armies whose membership changed, so a battle's losses don't reshuffle the ranks every tick. A slot on impassable ground is pulled in toward the captain until it lands on passable ground, so big armies don't pile up on a shoreline.
- **Close to its slot,** a follower heads straight for it; far away, it descends the rally field.
- **Separation:** it pushes away from neighbours found in the `SpatialGrid`, checking at most 48 entries in a fixed order, so dense crowds cost a bounded amount. The grid's cells shrank from 64 to 8 tiles, and its rebuild touches only occupied cells.
- **Blocked steps:** a step into impassable terrain slides along it. When every way is blocked, the follower re-centres on its own tile, from where the field's next tile is always reachable.

**Spawning.** Armies are only placed on tiles reachable from the captain. A spit of beach cut off by water once stranded 25 men for good.

**Villager paths.** Each villager plans its own path in parallel: a cache hit, a straight line for a clear hop of up to 8 tiles, or an A* search. New paths are added to the cache afterwards, in key order. Paths are pure, so the result doesn't depend on order.

## Consequences

- `army_march`: 200 followers march 220 tiles around a lake the test places across their straight line. All arrive, 95% stay within 1.5 times the loose radius, nobody stands on water, and multi- and single-threaded runs match.
- Tick cost on the 4-core development box:
  - 20,000 villagers: 4.4 ms, down from 6.6 ms (parallel path planning, inline decision factors).
  - 5,000 villagers plus two armies of 1,000 at war: about 3.6 ms.
  - 5,000 villagers plus two armies of 7,500 in battle: 8.1 ms, down from 19 ms after bounding separation, pulling slots off water and renumbering slots only on change.

  NFR-03 (8 ms) and NFR-04 (3.3 ms) are targets for the 8-core reference machine and must be confirmed there.
- Rally windows are capped at 160 tiles from the captain. A follower beyond that steers straight for the captain and can be stuck behind water. This hasn't been seen in the scenarios.
