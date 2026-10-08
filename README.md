# Warlord Sim (working title)

An indirect-control medieval conquest simulation in Rust + Bevy. You command captains, not soldiers, on a living island where every villager is simulated: a name, a home, a job, needs and reasons. Armies march as crowds, live off the land and scare or provoke the villages they pass.

**Status:** M3 (armies) is built and awaiting its go/no-go playtest ([checklist](docs/M3_PLAYTEST.md)). Next come M3b (developer handbook), M4 (conquest and faith) and M4b (living towns).

## Playing

```sh
cargo run -p game_app                  # opens on the skirmish: your captain and 50 men, raiders on the far side
cargo run -p game_app -- --villages    # villages only
cargo run -p game_app -- --seed 7      # another island
```

| Action | Control |
| --- | --- |
| Pan | WASD / arrows / screen edges (Shift for faster), or right or middle mouse drag |
| Zoom | Mouse wheel |
| Rotate | Q / E (90° steps; hold to rotate freely) |
| Jump between settlements | N / Shift+N |
| Pause, speed | Space; 1 / 2 / 3 / 4 for 1x / 2x / 4x / 8x (or the bar at the top) |
| Inspect | Click a villager or soldier: needs, what they're doing and why. Click a settlement or its name: population, food, stores, threat. Esc clears |
| Select an army | Click one of its soldiers or its captain. The captain panel opens (bottom centre) |
| Move | Right-click the ground (Shift queues) |
| Village orders | Right-click a village: gather food, recruit 10, trade, equip, attack, garrison (a village you hold), move here |
| Posture | Panel buttons, or P to cycle passive / neutral / aggressive |
| Return to camp / cancel orders | R / Backspace |
| Appoint a captain | Select one of your followers, then *Appoint captain* in the panel |

## Developing

| | |
| --- | --- |
| Start here | [docs/dev/GETTING_STARTED.md](docs/dev/GETTING_STARTED.md) |
| How the code fits together | [docs/dev/ARCHITECTURE.md](docs/dev/ARCHITECTURE.md) |
| Determinism rules | [docs/dev/DETERMINISM.md](docs/dev/DETERMINISM.md) |
| Recipes for common changes | [docs/dev/HOWTO.md](docs/dev/HOWTO.md) |
| Every data field | [docs/dev/DATA.md](docs/dev/DATA.md) |
| Every test | [docs/dev/TESTING.md](docs/dev/TESTING.md) |
| Terms | [docs/dev/GLOSSARY.md](docs/dev/GLOSSARY.md) |
| Workflow and review | [CONTRIBUTING.md](CONTRIBUTING.md) |
| API docs | `cargo doc --workspace --no-deps --open` |

**Design:**
- Spec: [docs/SEED.md](docs/SEED.md)
- Milestone plans: [M0](docs/M0_PLAN.md), [M1](docs/M1_PLAN.md), [M2](docs/M2_PLAN.md), [M3](docs/M3_PLAN.md), [M3b](docs/M3B_PLAN.md), [M4](docs/M4_PLAN.md)
- Design notes: [towns](docs/TOWNS_DESIGN.md), [fun](docs/FUN_DESIGN.md)
- Decisions: [docs/adr](docs/adr)
- Agent guide: [CLAUDE.md](CLAUDE.md)

**Headless:** `cargo run --release -p sim_headless -- --help`. Runs the sim with no window: state hashes, replays, `--skirmish`, `--battle N`, `--stats-every`, `--timings`.
