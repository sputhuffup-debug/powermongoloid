# M1 Plan: World

Status: **approved 2026-10-06** ("complete up to milestone 1").
Spec: `docs/SEED.md` § "M1: World". Covers SIM-01, SIM-02, RND-01 and RND-02, plus settlement placement and road links.

## Acceptance criteria and how each is tested

| Criterion | Test |
| --- | --- |
| A seed produces the same 2048 x 2048 island every time, with all terrain types present | `tests/tests/worldgen.rs`: generate twice per seed and compare the map digest, settlements and roads; generate with 1 thread and with N threads and compare; check that all 8 terrain types appear for several seeds; check the border is all water. |
| Terrain renders in chunks; the camera pans, zooms and rotates smoothly | Unit tests for the camera rig math (pan relative to yaw, zoom clamps, 90-degree snapping, damping). The game is launched under Xvfb with scripted key and mouse input and screenshots are taken zoomed out, zoomed in and rotated. A sim test checks that changing one tile bumps only that chunk's revision, which is how the renderer knows which chunks to rebuild. |
| Settlements (40 to 80) are placed on suitable land with road links between neighbors | The same test file checks: settlement count is between 40 and 80; each center is flat Grass or Farmland on the main landmass; settlements are at least the minimum spacing apart; every settlement has at least one road; the road graph is connected; every road path is a contiguous run of Road tiles from center to center, never crossing water. |

The existing determinism suite and the hash coverage test must keep passing with the world in the hashed state.

## Design

**Data** (`assets/data/worldgen.ron`, embedded at compile time): map size, chunk size, noise octaves and frequencies (in fractions of the map, so shape doesn't depend on size), terrain proportions, settlement count and spacing, road costs, region count, and name syllables.

**`sim_world`**
- `noise`: in-house 2D gradient (Perlin) noise and fBm, with permutation tables shuffled by a `SimSeed` stream. See ADR 0005 for why the `noise` crate isn't used.
- `WorldMap` resource: flat row-major arrays for height, terrain, fertility and region, with 64 x 64 chunks.
  - The fields are private and the only way to change a tile is `set_terrain`. It bumps the chunk's revision (the renderer rebuilds only changed chunks, RND-01) and marks the chunk's cached hash digest dirty.
  - `StateHash` folds the per-chunk digests, so hashing the 4M-tile map is cheap.
- Generation pipeline, a pure function of `(seed, settings, threads)`:
  1. Height: domain-warped fBm plus ridged noise, times a noisy radial island falloff. Rows are computed in parallel with `std::thread::scope`; each tile is independent, so the result doesn't depend on thread count.
  2. Moisture: a second fBm.
  3. Terrain classes from quantiles of the height and moisture distributions. This makes the proportions stable and guarantees every terrain type appears.
     - Water: below sea level.
     - Beach: low land within 2 tiles of water.
     - Mountain and Hills: the top quantiles of land height.
     - Forest: wet lowland.
     - Grass: the rest.
  4. Fertility from moisture, altitude and slope.
  5. Landmasses by flood fill. Settlements go on the largest one only.
  6. Settlements: score candidate tiles on fertility, flatness and nearness to water; pick the best tile per grid cell, then place greedily with minimum spacing, relaxing the spacing until at least the minimum count fits.
  7. Roads: the minimum spanning tree over settlements plus links to each settlement's 2 nearest neighbors. Each link is routed with deterministic integer-cost A* (8-connected; terrain and slope costs; reusing existing road is cheap; water is impassable).
  8. Farmland: fertile land around each settlement, excluding roads.
  9. Regions: a breadth-first flood over land from region seeds, chosen by farthest-point sampling over settlements.
  10. Names: syllable combinations from the RON file, unique, drawn from a `SimSeed` stream.
- `WorldPlugin::finish` generates the world and spawns `Settlement` entities with `spawn_sim`. A Bookkeeping system refreshes dirty chunk digests.
- New hashed state: `WorldMap`, `RoadNetwork`, `WorldGenSettings`, `Settlement`, `NameTable` (in `sim_core`, shared with villagers in M2), and the placeholder `DemoAnchors`.
- The M0 placeholder wanderers now spawn around settlements and stay near them.

**`game_render`**
- Terrain: one mesh entity per 64 x 64 chunk, built from tile-corner heights with vertex colors by terrain type.
  - Building is spread over frames, nearest chunks first.
  - A chunk is rebuilt only when its revision changes.
  - Water is flattened to sea level, and an ocean plane extends past the map edge.
- Camera rig: orbit around a focus point on the terrain.
  - Pan with WASD or arrow keys, edge scrolling, or right/middle-mouse drag.
  - Zoom with the mouse wheel (exponential, clamped).
  - Q/E tapped rotates in 90-degree steps; Q/E held rotates freely.
  - N / Shift+N jump to the next / previous settlement (added during implementation).
  - Pitch follows zoom, and every motion is damped so it's smooth.
- Settlements: small house clusters placed from a hash of the settlement's ID, so their look is stable.

**`game_ui`**: settlement name labels projected from world positions, hidden when zoomed far out; the debug panel gains world stats (size, settlements, roads, generation time, chunks built) and camera state.

**`sim_headless`**: `--world-size` for faster experiments, recorded in replay files (replay format 2). Worldgen time is printed.

## Out of scope for M1 (noted for later)
- A per-tile faction `owner` array in `WorldMap`: added in M4 with conquest and overlays (UI-04).
- `SpatialGrid`: added in M2, when villagers need neighbor queries.
- Terrain mesh LOD: the full-detail map is about 8.4M triangles, fine on the reference GPU. Revisit if RND-04 (60 fps) needs the headroom.
- Rivers and erosion: a "should"-level polish item.
