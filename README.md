# Warlord Sim (working title)

An indirect-control medieval conquest simulation in Rust + Bevy. You command captains, not soldiers, on a living island of 250,000 simulated people.

- Spec: [docs/SEED.md](docs/SEED.md)
- Plans: [M0](docs/M0_PLAN.md), [M1](docs/M1_PLAN.md), [M2](docs/M2_PLAN.md); decisions in [docs/adr](docs/adr)
- Agent guide: [CLAUDE.md](CLAUDE.md)

## Running

```
cargo run -p game_app [-- --seed <u64>]
```

Camera: WASD / arrows or screen edges to pan (Shift for faster), right or middle mouse drag to pan, mouse wheel to zoom, Q / E to rotate 90° (hold to rotate freely), N / Shift+N to jump between settlements (populated ones first).

Time: Space pauses, 1 / 2 / 3 / 4 set 1x / 2x / 4x / 8x (or use the bar at the top).

Inspect: click a villager to see their needs, what they're doing and why; click a settlement (or its name) for its population, food and stores. Esc clears.

Headless: `cargo run --release -p sim_headless -- --help`.
