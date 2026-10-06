# Getting started

How to build, run and test Warlord Sim from a fresh checkout. Next: [ARCHITECTURE.md](ARCHITECTURE.md) for how the code fits together, and [HOWTO.md](HOWTO.md) for common changes.

## 1. Prerequisites

### Rust
Use [rustup](https://rustup.rs). The toolchain is pinned in `rust-toolchain.toml` (Rust 1.97.0, with rustfmt and clippy); rustup installs it the first time you run `cargo` in the repo. Don't use a distro-packaged Rust.

### System packages (Linux)
Bevy needs these headers to build:

```sh
sudo apt-get install -y --no-install-recommends \
  pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
```

To **run** the game you also need a GPU driver with Vulkan (or software Vulkan: `mesa-vulkan-drivers`) and the X11 runtime libraries `libxkbcommon-x11-0 libxcursor1 libxi6 libxrandr2`. Headless runs and tests need none of these.

- **macOS:** nothing extra is needed.
- **Windows:** the MSVC build tools are needed.

The cloud sessions Claude Code runs in get all of this from `.claude/hooks/session-start.sh`.

### Hardware
Building the workspace needs about **16 GB of RAM** and **25 GB of disk** for `target/` (Bevy is large). On smaller machines, see [Low memory and disk](#low-memory-and-disk).

## 2. Build and run

| What | Command |
| --- | --- |
| The game (opens on the M3 skirmish start) | `cargo run -p game_app` |
| The game, villages only (M2 start) | `cargo run -p game_app -- --villages` |
| A different world | `cargo run -p game_app -- --seed 7` |
| The stress view (RND-04) | `cargo run --release -p game_app -- --stress 20000` |
| Headless, print the state hash | `cargo run --release -p sim_headless -- --seed 42 --ticks 10000` |
| Headless options | `cargo run --release -p sim_headless -- --help` |

The first build takes 10–20 minutes, mostly compiling Bevy; later builds take seconds.

**Debug builds are playable.** The dev profile builds our crates at `opt-level = 1` and dependencies at `opt-level = 3`. The hot sim crates (`sim_world`, `sim_agents`, `sim_economy`, `sim_military`) are also built at `opt-level = 3` in dev. See `Cargo.toml`.

**Controls** are listed in the [README](../../README.md#playing).

### Cargo aliases
Defined in `.cargo/config.toml`:

| Alias | Runs |
| --- | --- |
| `cargo game [args]` | the game, dev build (e.g. `cargo game --seed 7`) |
| `cargo headless [args]` | `sim_headless`, release build (e.g. `cargo headless --ticks 1000`) |
| `cargo lint` | clippy on everything with warnings as errors (run `cargo fmt --check` too) |
| `cargo test-fast` | unit tests only (the sim crates and UI helpers; seconds) |
| `cargo test-all` | every test, including the scenario and determinism suites (minutes) |

## 3. Test

```sh
cargo fmt --check                                     # formatting
cargo clippy --workspace --all-targets -- -D warnings # lints, warnings fail
cargo test --workspace                                # everything (~4 min on 4 cores)
```

- The integration tests in `tests/tests/` each build a 2048-tile world with thousands of villagers, so they take 10–100 s each. They run at full speed because the sim crates are optimised even in dev builds.
- Run one test with `cargo test -p integration_tests --test posture`.
- Add `-- --nocapture` to see what a test prints.
- [TESTING.md](TESTING.md) says what each test guards.

Before you push, run the same steps as CI: fmt, clippy, test, and `cargo doc --workspace --no-deps`.

## 4. Low memory and disk

- **Out of memory.** If the build is killed (`signal: 9`, exit status 137), build with fewer jobs: `cargo build -j 3` (or `-j 2`). This applies to `test` and `clippy` too.
- **Disk full.** `target/debug/incremental` grows by gigabytes over time and is safe to delete: `rm -rf target/debug/incremental`. `cargo clean` frees everything at the cost of a full rebuild. Dependency debug info is already off in `Cargo.toml`.
- **Release builds of the game** compile Bevy a second time (another ~5 GB). Use debug builds for day-to-day play, and release builds only for measuring performance.

## 5. Common problems

| Symptom | Fix |
| --- | --- |
| `Unable to find libxkbcommon-x11` or a blank window on Linux | Install `libxkbcommon-x11-0 libxcursor1 libxi6 libxrandr2` |
| `failed to find a suitable GPU adapter` | Install Vulkan drivers (`mesa-vulkan-drivers` for software rendering) |
| The game runs at 1–5 fps in a VM or container | Software rendering. Expected; measure performance on a real GPU |
| `ALSA lib ... No such file or directory` at startup | No sound card (common in containers and VMs). Harmless; the game has no audio yet |
| `Error adding plugin ... already added` | Two plugins add the same Bevy plugin; guard with `app.is_plugin_added::<T>()` (see `game_app/src/main.rs`) |
| A determinism test fails | Follow the checklist in [DETERMINISM.md](DETERMINISM.md#when-a-determinism-test-fails) |
| `state_hash_covers_every_sim_component` fails | You added a component or resource without registering it; see [HOWTO.md](HOWTO.md#add-a-component-or-resource) |
| Rustfmt reformatted code and a scripted edit no longer matches | Re-read the file before editing it again |

## 6. Screenshots without a display

To check the game where there is no screen (CI, cloud sessions):

```sh
Xvfb :99 -screen 0 1600x900x24 &
DISPLAY=:99 cargo run -p game_app &
sleep 50                                   # world generation and first frames
DISPLAY=:99 xdotool mousemove 650 340 click 1   # e.g. select a soldier
DISPLAY=:99 import -window root shot.png
```

`import` comes from ImageMagick. The game window is 1280x720 at the top left of the virtual screen.
