# ADR 0007: Economy, physical goods and the transfer resolver

Date: 2026-10-06 · Status: Accepted · Requirements: SIM-20, SIM-21, SIM-24, UI-02

## Context

Goods must be physical (SIM-20): held in inventories or stockpiles and moved by people. Recipes are data (SIM-21). Many villagers use the same storehouse in the same tick, while villager systems run in parallel and must stay deterministic.

## Decision

**Data.** `assets/data/economy.ron` lists goods (with food values), recipes (workplace, worker, inputs, outputs, duration) and workplaces (worker slots, search radius, spacing, site rules). Names are resolved to ids at load, and the data is hashed as a resource.

**Storage.** `Stockpile` (a `BTreeMap<GoodId, u32>` with a capacity) sits on buildings; `Inventory` (four slots plus a unit capacity) sits on agents. SEED.md suggested `HashMap` and `SmallVec`. `BTreeMap` follows the determinism rule, and a fixed array avoids adding `smallvec`.

**Transfers.** Agents never touch a stockpile directly. They post a `Transfers` order on their own entity: `Take`, `TakeFood`, `TakeAny`, `TakeMeal` (ignores carrying capacity, for food eaten on the spot) or `PutAll`. In `EconomySet::Resolve`, after the parallel act system, `resolve_transfers` sorts all orders by `(store SpawnId, agent SpawnId)` and applies them one by one. Contention resolves the same way every run: earlier SpawnIds are served first. The same pattern will carry trade and looting.

**Production.** A worker at its workplace takes the recipe's inputs from the workplace stock, works `duration` ticks, and puts the outputs back. Carters, and workers whose workplace is well stocked, haul outputs to the storehouse. Workshops are supplied from the storehouse the same way.

**Layout.** Each populated settlement gets a storehouse, bakery, smithy, houses (every other tile, leaving lanes) and outlying workplaces. These are only placed on land reachable on foot from the centre, after M2 testing found houses cut off across water. Farms and pastures may use cleared forest as a last resort, so forest-bound villages can still feed themselves.

**Food security.** Each settlement's `FoodSecurity` holds the days of food in its storehouse and homes at its population's rate of eating. It drives the settlement panel now and loyalty and migration in M4 (SIM-24).

## Consequences

- Starting stock is about a day of food: enough to start, too little to hide a production collapse. The famine test relies on that.
- Hauling capacity, not production, was the bottleneck while tuning. Self-hauling by workers fixed it; future goods-heavy features should budget for transport.
- Food balance across days is tuned for M2 only. Seasons and decay (M4) will need re-tuning, with the `--stats-every` CLI output as the tool.
