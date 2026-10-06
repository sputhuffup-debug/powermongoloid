# ADR 0008: Agent rendering bands and interpolation

Date: 2026-10-06 · Status: Accepted · Requirements: RND-03, RND-06, NFR-05

## Context

RND-03 asks for GPU-instanced low-poly figures, tinted by faction and occupation, that switch to billboards and then density dots with distance. RND-06 asks for smooth motion at any sim speed. Rendering must never write to sim entities (ADR 0005).

## Decision

**One render entity per villager.** It is spawned and despawned by `game_render::agents` and mapped from the sim entity in a render-side `AgentVisuals` resource.

**Bands.** Each render entity is in one of four bands chosen by camera distance, from `assets/data/render.ron`:

| Band | Distance | Mesh | Material |
| --- | --- | --- | --- |
| Detail | under 150 | Figure (capsule + head) | Stylized cloth texture tinted by occupation |
| LowPoly | 150–600 | Same figure | Flat colour |
| Billboard | 600–1,400 | Camera-facing quad | Unlit silhouette |
| Dot | beyond 1,400 | Disc scaled with distance | Unlit, constant on-screen size |

Switching uses hysteresis (out at 1.1x a threshold, back in at 0.9x).

**Instancing.** There is one mesh per band and one material per (band, occupation), so Bevy batches each group into instanced draws. Faction tint joins occupation tint when factions exist (M3/M4).

**Interpolation.** A render system in `FixedFirst` snapshots each villager's position before every tick. `Update` blends from the snapshot to the current position by `Time<Fixed>::overstep_fraction`. At 8x speed, with several ticks per frame, this still blends across the latest tick.

**Picking.** Picking uses the sim's `SpatialGrid`, after a ray march from the cursor to the terrain surface. Selection is UI state.

## Consequences

- The per-frame cost is one transform update per villager. That is fine at 5,000; RND-04 (20,000 at 60 fps) is measured in M3 on real hardware.
- The container used for development renders in software (1–10 fps), so frame-rate claims wait for the reference machine.
