# ADR 0006: Utility AI and decision traces

Date: 2026-10-06 · Status: Accepted · Requirements: SIM-10, SIM-11, SIM-12, UI-03

## Context

SIM-12 requires villagers to choose actions with utility AI scoring needs, job and danger, with no scripted schedules. UI-03 requires the player to see "why is this person doing that". Behaviour also has to be deterministic, parallel, and cheap for 5,000 villagers now and 20,000 agents in M3.

## Decision

**Actions.** Eat, Fetch (restock the household from the storehouse), Sleep, Work, Haul, GoHome and Idle. Each action runs as a small state machine (`Activity`: Travel, Perform, Deliver, Done) in `sim_agents::act`. Flee, Hide and Fight arrive with threats in M3; the `safety` need already exists for them.

**Scoring.** Each candidate scores `weight x product of considerations`. Considerations are response curves (logistic or linear) and factors from `villagers.ron`: hunger, tiredness, phase of day, whether food or work is available, and travel distance. Rules learned while tuning, all in data:
- Hunger damps work (`fit = 1 - 0.6 x appetite`), so villagers break for meals instead of working until they starve.
- At night the urge to sleep has a floor (`night_drive`), so rested villagers stay asleep rather than wandering.
- A worker whose workplace is well stocked hauls its output to the storehouse, and one whose workshop lacks inputs fetches them. Carters alone couldn't keep up with farms.
- The current action gets an `inertia` bonus, so villagers don't flip between near-equal options.

**Ties.** Ties keep the earlier option in a fixed action order. With `f32` scores that are pure functions of the agent's own state and read-only world state, the choice is deterministic in the parallel `decide` system.

**Staggering.** An agent decides when its action finishes, or every `decision_interval` ticks (10, offset by `SpawnId`), so about 1/10 of agents decide per tick, as SEED.md specifies.

**Decision traces.** Each decision stores its top three options, with scores and named factors, in a `DecisionTrace` component, which the inspector shows (UI-03). The sim never reads it and it can be recomputed, so it is exempt from the state hash with a reason.

**Randomness.** The few random choices (idle spots, idle durations) use `SimSeed::stream(key ^ spawn_id, tick)`, so they don't depend on scheduling.

## Consequences

- Behaviour is tuned in `villagers.ron`; the M2 acceptance tests (`village_day`, `famine`) guard the tuning.
- Villagers stuck because they can't do something, such as eating while their hands are full, show up in traces. This found two bugs during M2 (full inventories blocking meals; work beating meals).
- Decide costs about 0.5 ms per tick at 5,000 villagers in release builds; the whole tick costs about 1.1 ms.
- `sim_agents` and `sim_economy` build at `opt-level = 3` in the dev profile (like `sim_world`), so the 5,000-villager tests run in seconds.
