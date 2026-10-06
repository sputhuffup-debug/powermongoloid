# ADR 0005: World generation, the tile map and terrain rendering

Date: 2026-10-06 · Status: Accepted · Requirements: SIM-01, SIM-02, RND-01, RND-02

## Context

M1 needs a 2048 x 2048 procedural island that is fully determined by a u64 seed, with all eight terrain types, 40 to 80 settlements linked by roads, and chunked terrain meshes that rebuild only when changed. SEED.md lists the `noise` crate for heightmaps.

## Decisions

**In-house noise instead of the `noise` crate.** `noise` 0.9 (last released March 2024) seeds its permutation tables with its own `rand_xorshift` RNG from a 32-bit seed. That breaks the hard rule that all randomness comes from `SimSeed`, and it would add a second copy of `rand` (0.8) to the sim crates. `sim_world::noise` is about 100 lines of improved Perlin noise with fBm and ridged variants, and its permutation tables are shuffled by `SimSeed::stream`. No new dependency was added.

**Generation is a pure function of `(seed, settings)`.** `generate_world(seed, settings, threads)` computes the noise fields in parallel with `std::thread::scope`. Each tile depends only on its coordinates, so the thread count changes speed but never the result, and a test checks 1 thread against 4. Everything after the noise is serial. A* uses integer costs with ties broken on tile index; sorts use total orders with index tie-breaks. Generation runs once, in `WorldPlugin::finish`, before the first tick, in both the game and headless runs.

**Proportions come from quantiles, not fixed thresholds.** Sea level and the mountain, hills, beach and forest cut-offs are quantiles of the height and moisture distributions, taken from a 65,536-bin histogram (deterministic, no float sort). Every seed gets the configured mix, about 58% water with 6% mountains and 14% hills of the land, so every terrain type is always present.

**Settlements use best-candidate sampling.** Candidate tiles are flat Grass or Forest on the largest landmass, at least 3 tiles from water. The best candidate per grid cell is kept, then sites are picked one at a time to maximise score plus a bonus for distance from sites already chosen. Pure score-greedy placement packed villages along the fertile coast and left the interior empty. Spacing starts from the island's land area per site and relaxes until at least `min` settlements fit.

**Roads are a spanning tree plus nearest neighbours.** Prim's minimum spanning tree guarantees the network is connected. Links to each settlement's 2 nearest neighbours add redundancy. Links are routed shortest first, so later roads reuse earlier ones (road tiles are cheap to enter).

**`WorldMap` controls all changes to tiles.** Its arrays are private; `set_terrain` is the only way to change a tile. It bumps the chunk's revision, which the renderer compares to decide what to rebuild (RND-01), and marks the chunk's hash digest dirty. `StateHash` folds 1,024 cached chunk digests instead of hashing 4M tiles at every checkpoint. Dirty chunks are digested fresh during hashing, so the hash is never stale even before the Bookkeeping refresh runs.

**The world data is embedded at compile time.** `assets/data/worldgen.ron` is included with `include_str!`, so every build generates the same world regardless of working directory. Replay files record the map size (format 2); the other settings come from the build, and the hashed `WorldGenSettings` resource makes any mismatch show up at the first checkpoint.

**Terrain meshes.** There is one mesh per 64 x 64 chunk, with a vertex at every tile corner. Corner height and colour are the average of the up-to-four tiles that share the corner, and normals come from central differences over the whole map, so there are no seams at chunk borders. Builds are capped at 48 chunks per frame, nearest the camera first. Water is flattened to sea level and darkened with depth, and an ocean plane extends past the map edge. The full map is about 8.4M triangles; level of detail is deferred until RND-04 measurements say it's needed.

**Rendering never writes to sim entities.** The render and UI plugins track which settlements already have houses and labels in system-local sets, not marker components, so the "rendering only reads" rule holds and the hash coverage check stays clean.

## Consequences

- Generating the 2048 map takes about 1 s in release on a 4-core machine (noise about 0.45 s, classification about 0.2 s, roads about 0.3 s). `sim_world` builds at `opt-level = 3` in the dev profile, an exception to the opt-level 1 default, so tests and debug runs aren't slowed by tens of seconds.
- Every determinism test now generates a full world. The suite takes about 12 s.
- Changing `worldgen.ron` changes every world and every state hash; old replays then fail at their first checkpoint, as intended.
- Rivers, erosion, and a per-tile faction `owner` array are deferred; see `docs/M1_PLAN.md`.
