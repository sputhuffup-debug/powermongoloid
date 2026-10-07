# ADR 0012: Melee crowding and the catch-up cap

Date: 2026-10-07 · Status: Accepted · Requirements: RND-04, NFR-03, NFR-05, SIM-30

## Context

The first reference-machine stress run (`game_app --stress 20000`, 2026-10-07) held 118–176 fps with 20,000 agents on screen. When the two 7,500-man armies met, though, frames took 0.5 to 14 seconds for about half a minute. Two problems compounded:

- **Crush.** In a fight every soldier walks at its nearest foe. Separation (ADR 0011) is too weak to resist a whole army pressing forward. Up to 7,400 fighters piled into one 8×8-tile cell, about 100 a tile. Every soldier's nearest-foe search then scanned that pile: about 115 million distance checks a tick. Headless on the 4-core box, `Decide` went from 1.3 ms to 65–80 ms a tick, with single ticks up to 260 ms.
- **Catch-up spiral.** At 8x the time bar let one frame hand the fixed loop up to 2 s of real time, which is 160 ticks. A slow tick made the frame slow, the next frame owed more ticks, and so on until frames took seconds.

## Decision

- **Tile capacity in melee.** A soldier or militiaman in a Fight action won't step into a tile that held `formation.max_per_tile` (3) or more fighters at the start of the tick; it slides along or waits, like at a wall. The counts come from the fighter grid, built serially in `Knowledge`, and are only looked up, so parallel moves stay deterministic. Marching soldiers and captains ignore the cap. A first version applied it to everyone, and a 7,500-man column jammed in narrow passes, fell out of baggage range and deserted.
- **Nearest-foe search.** `FighterGrid::nearest_hostile` visits cells nearest first and stops at cells farther than the best foe found, giving the same answer as a full search (unit test). Enemy and friend counts moved to `count_nearby`, which runs only when a soldier's decision trace refreshes, the only place they were used. With the old movement this produced identical state hashes.
- **Catch-up cap.** Each frame, the time bar caps how much real time the fixed loop may consume. The cap is as many ticks as fit in 25 ms at the measured tick cost (at least one), at the chosen speed. When the sim can't keep up, the game runs slower than the chosen speed instead of freezing. Ticks are never skipped, only delayed, so replays are unaffected.

## Consequences

- Headless, 5,000 villagers plus two armies of 7,500 meeting (4-core box): ticks during the clash average 10–15 ms (worst 46 ms), down from 72–85 ms (worst 260 ms). Before and after the clash nothing changes.
- Battles last longer and look like lines rather than a blob, since only the front ranks reach the enemy. army_march, posture, starving_army and orders still pass unchanged.
- At 8x during a big battle the game may run nearer 2–4x. The time bar still shows 8x.
- `sim_headless --timings --stats-every n` prints mean and worst tick and the costliest stages for each window of n ticks, to find spikes like this one.
