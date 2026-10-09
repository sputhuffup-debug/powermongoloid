# Warlord Sim (working title)

An indirect-control medieval conquest simulation in Rust + Bevy. You command captains, not soldiers, on a living island where every villager is simulated: a name, a home, a job, needs and reasons. Armies march as crowds, live off the land and scare or provoke the villages they pass.

**Status:** M4 "Rise" (roguelite runs: a landless band against a rival warlord's seat, on a fresh island each time) is built and awaiting its playtest ([checklist](docs/M4_PLAYTEST.md)). Next comes M4b (living towns).

## Playing

```sh
cargo run -p game_app                       # a run (M4): new-game screen, pick a start boon, take the island
cargo run -p game_app -- --seed 7 --difficulty harsh   # a given run island and difficulty (steady, chaotic, harsh)
cargo run -p game_app -- --skirmish         # the M3 skirmish: your captain and 50 men, raiders on the far side
cargo run -p game_app -- --villages         # villages only
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
| Village orders | Right-click a village: gather food, recruit 10, trade, equip, give food (what the baggage holds beyond a day's need), attack, garrison (a village you hold), move here |
| Forage | F or the panel's *Forage* button: the army stands and gathers wild food around it until the baggage is full or the land is bare |
| Posture | Panel buttons, or P to cycle passive / neutral / aggressive |
| Return to camp / cancel orders | R / Backspace |
| Appoint a captain | Select one of your followers, then *Appoint captain* in the panel (in a run it greys out when your captain slots are full) |
| New run (M4) | On the new-game screen: type a seed (digits, Backspace) or *Random*, pick a difficulty, *Build it* if either changed (the island is built in a few seconds, in place), then click one of the three start-boon cards to begin |
| Save and resume | A run is saved when you close the game (one slot per run, in `saves/` next to the game binary, or `--save-dir <dir>`); the new-game screen lists saved runs under *Resume*. F5 quicksaves and F9 quickloads this run's slot (the game pauses after a load). A finished run's slot is deleted |
| Offers | Waiting offers (dominion boons, traits, invitations) are buttons in the run bar under the clock: click one to see its cards, click a card to choose, *Later* to close |
| Fast-forward | *>> Next event* in the run bar: 8x until the next notable event, then pause. The game also pauses on the events the difficulty's director lists (raiders, rival raids, a fallen champion) |
| Event feed | Bottom left: the chronicle's sentences (why things happened, battle reports) and your warband's log (dimmer); click a line to move the camera there |
| Overlays | O cycles ownership, goodwill / loyalty, food security, monuments, off |
| Monuments | Hover one for its kind, bonus and holder |
| Abandon a run | *Abandon run* in the run bar, twice. The end screen shows the run summary, the timeline (mouse wheel scrolls) and your battle reports, and offers *New run* and *Same seed again* |

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
