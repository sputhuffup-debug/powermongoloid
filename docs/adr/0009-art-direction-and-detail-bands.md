# ADR 0009: Art direction: stylized textures near, low-poly far

Date: 2026-10-06 · Status: Accepted · Requirements: RND-01, RND-03; SEED.md open question "Art direction"

## Context

SEED.md left art direction open until M2: flat-shaded low-poly, or a stylized texture look. The answer (2026-10-06) was both, by zoom level.

## Decision

**One detail distance** (`render.ron`, initially 150 world units) separates the styles for everything at once, with hysteresis:
- **Terrain:** chunks inside the distance swap to a material whose stylized detail texture (UV-mapped by global tile coordinates, so seamless across chunks) multiplies the existing vertex colours. Outside, a flat material whose base colour matches the texture's average brightness, so switching changes texture, not tone. It is a material swap, never a mesh rebuild.
- **Buildings:** each part (plaster, roof, wood, stone) swaps between a textured and a flat material.
- **Villagers:** see ADR 0008; the detail band uses a cloth-textured figure.

**Textures are generated procedurally at startup** in `game_render::art` from a fixed integer hash: painterly terrain detail, plaster with timber, planks, shingles, stone, cloth, and a figure silhouette. They are tileable, deterministic, need no asset files, and borrow nothing from other games. Materials hold image handles only, so hand-painted textures can replace any of them later without code changes.

## Consequences

- The close-up look is placeholder-quality but has the right structure: real art drops into the same slots.
- The SEED.md open question is closed. The art pass (RND-05 lighting, seasonal palettes) builds on these two bands.
