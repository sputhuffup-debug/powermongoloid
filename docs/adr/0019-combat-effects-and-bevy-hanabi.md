# ADR 0019: Combat effects and bevy_hanabi

Date: 2026-10-12 · Status: Proposed. You approved the dependency on 2026-10-12; the design awaits your approval with [SAGA_DESIGN.md](../SAGA_DESIGN.md), and this record moves to Accepted then, gaining its "as built" notes in M4c steps 3b, 4, 4b, 8b and 10b · Requirements: RND-10, RND-11, NFR-10 (also SIM-59, RND-03, RND-04, NFR-05, UI-10). Follows [ADR 0008](0008-agent-rendering-and-interpolation.md) and [ADR 0009](0009-art-direction-and-detail-bands.md); keeps [ADR 0002](0002-workspace-and-sim-render-split.md)'s split.

## Context

M4c makes combat brutal in words ([SAGA_DESIGN.md](../SAGA_DESIGN.md)). You asked for the fights to look it too (2026-10-12):
- blood on hits and bursts on kills;
- pools that fade in about a game hour;
- severed heads, arms and legs on told kills, matching the kill line;
- bodies that linger;
- over the top in the brutal tone, and dust and falls only in the mild;
- a kill cam;
- bevy_hanabi as the particle engine, with automatic fallbacks, and severed parts, pools and bodies drawn identically on every backend, so only the spray's density degrades.

What the renderer has today:
- **One render entity per person.** Bevy batches the instances automatically: one mesh per detail band and one material per (band, look).
- **The figure has no limbs:** a capsule and a head sphere.
- **A sim entity that despawns pops its visual the same frame.** The renderer can't tell a death from a desertion: in the 7,500-a-side battle, 19 desert for each one who dies.
- **Health is no proof of a blow.** A recruit who deserts takes civilian stats again, and their health drops by 40 at full health (100 to 60); starvation, a mishap's slip and a camp brawl lose health without a blow.
- **`Casualties` exists for one tick only** and is exempt from the hash. At 8x several ticks run in a frame, so reading it in `Update` loses deaths. After a run ends, every `SimSet` is gated off, so nothing clears it again: it keeps the ending tick's lists for good.
- **Lighting:** shadows are off, there's no depth prepass, and MSAA is the default. Bevy 0.19's forward decals need a depth prepass, and clustered decals need compute shaders.
- **Time at speed.** game_ui's catch-up cap clamps `Time<Virtual>`'s raw delta before the speed multiplies it, so in a big clash at 8x virtual time ÷ speed runs slower than real time.

The measurements (seed 42, headless):
- **Deaths.** About 300 die in either the 1,000- or the 7,500-a-side battle, at most 5 in a tick, and 1.2 to 1.4 a tick at the busiest. The front line limits the rate (ADR 0012's `max_per_tile`), not the army size.
- **Blows.** They're uncounted. Kills take 5 to 11 blows, so the peak is estimated at 25 to 50 a tick.
- **Frame time.** The reference machine (Ryzen 5 7600X, RTX 4070) ran the big clash at 118 to 176 fps, and 81 fps in its worst window. That leaves about 4 ms of a 60-fps frame.

bevy_hanabi 0.19.0 (read from its source):
- **Compatibility.** It requires bevy "0.19" (compatible with our `=0.19.1`) and Rust 1.95 (we use 1.97). Its licence is MIT OR Apache-2.0. It brings three new crates (itself, anyhow, rand_pcg) and changes no feature of a crate we already build.
- **GPU only:** compute shaders, no CPU simulation; on the web, WebGPU only.
- **Its `finish()`** eagerly builds compute pipelines and checks only the device's `max_bind_groups`. On a device without compute shaders, wgpu raises a validation error, and Bevy's default error handler quits the game. Its `finish()` also panics without a render sub-app.
- **Capacity.** An `EffectAsset`'s `capacity` is how many particles each of its instances can keep alive at once. All instances of one asset share a GPU buffer (a slab) of at least 65,536 particles (`ParticleSlab::MIN_CAPACITY`), each with a slice of `capacity`. An instance fired again while its last burst lives drops whatever doesn't fit.
- **Behaviour:**
  - it has no collision and no decals;
  - it simulates in global space by default, so a moved instance leaves its live particles where they are;
  - its clock, `Time<EffectSimulation>`, advances by `Time<Virtual>`'s delta times its own relative speed;
  - it seeds its CPU-side RNG from `rand::rng()`;
  - instances aren't batched, so tens of instances are fine and hundreds aren't.
- **Its own CI** runs on lavapipe, a software Vulkan driver.

## Decision

- **One effects layer, render-side only** (`game_render::effects`). The game turns what the sim recorded into effect requests: hit, kill, sever, maim, pool, body, dust. One backend draws the spray:
  - **GPU** draws it with bevy_hanabi: four assets (hit spray, kill burst, stump fountain, dust), each with its own capacity, and about 40 pre-spawned instances, fired round-robin.
  - **CPU** draws it with a recycled pool of our own instanced droplets, a few dozen per burst.
  - **Off** draws no spray.

  Bodies, pools and severed parts are ordinary meshes, drawn identically on every backend, Off included, so only the spray's density differs. The mild tone, not the backend, is the switch for gore.
- **Inputs, read only.**
  - **After every tick that ran,** a `FixedLast` system copies `Casualties.struck` (this tick's landed blows), `kills`, `fallen` and `maims` into a render-side buffer. It copies only when `SimTick` has advanced since its last copy, so a finished run never sends its last deaths again. `fallen` gains the victim's `Entity` and SpawnId (exempt scratch).
  - **Hits** are the sim's landed blows, never a visual's health falling between frames.
  - **Told deaths** are read from `Saga` with a render-side cursor. A `Fall` gains its position and the victim's role.
  - **A death becomes a body** in `sync_visuals` only if the buffer lists it. Everyone else vanishes as before.
- **One function names the part.** `Teller::told_wound(fall, tone)` is the wound of a told death in brutal (none in mild), and the feed and the renderer both call it.
  - Its part comes from `saga::wound`, which works from (victim SpawnId, tick) by a stable hash over saga.ron's `wounds` table. It's flavour, like a weapon word, so it's unhashed, and the sim never reads it.
  - It reads only the record, never which form a line took. Every brutal telling of a severed death names the part: the variants, a `terse` twin with `{wound_terse}`, long forms, report lines and fate clauses. Names in a told line are those at the death, so a later epithet never changes its fit.
- **The dependency.** `bevy_hanabi = { version = "=0.19.0", default-features = false, features = ["3d"] }`:
  - in `[workspace.dependencies]`, used by game_render only, in one module (`effects/gpu.rs`), added in M4c step 4b after the CPU backend and the first playable slice;
  - no cargo feature: the module is the seam if a Bevy upgrade ever comes before a matching bevy_hanabi;
  - sim crates never see it;
  - Hanabi's `gpu_tests` feature stays off;
  - imports are explicit (its prelude clashes with Bevy's `AlphaMode` and `Gradient`).
- **Selection.** `EffectsPlugin` is added by game_app only, after `DefaultPlugins`. In its own `finish()`, which runs after Bevy's `RenderPlugin::finish`, it reads `RenderAdapter`, `RenderAdapterInfo` and `RenderDevice`.
  - **Compute** means all of these:
    - the adapter's downlevel flags include `COMPUTE_SHADERS`, `INDIRECT_EXECUTION` and `VERTEX_STORAGE`;
    - the **device's** limits (`RenderDevice::limits()`): at least 8 storage buffers per stage, `max_compute_workgroup_storage_size` not 0, and at least 4 bind groups. Under `WGPU_SETTINGS_PRIO=webgl2` the adapter still reports compute while the device has WebGL2's limits, which is why Bevy's own GPU preprocessing reads the device;
    - the backend isn't GL: `Backends::from(info.backend) != Backends::GL`.
  - **A software adapter** is a `DeviceType::Cpu` adapter, matched by its debug name: Bevy doesn't re-export the type, and a direct wgpu dependency isn't worth one comparison.
  - **With compute, and unless forced to `cpu` or `off`,** the plugin calls `app.add_plugins(HanabiPlugin)`, which runs its `build` and registers it (so `is_plugin_added` works and a second add panics), then `HanabiPlugin.finish(app)` by hand. That works because bevy_app 0.19.1 fixes the range of plugins it finishes when the loop starts, and refuses new plugins only after it has finished; none of Hanabi's sub-plugins has a `finish` or `cleanup` of its own. Nothing else may add `HanabiPlugin`.
  - **Without a `RenderApp`** it chooses Off and adds nothing.
  - **The setting.** The machine's choice is Full (GPU) with compute on a hardware adapter, and Reduced (CPU) otherwise, including software adapters. A software adapter with compute still loads Hanabi, so Full stays selectable, since Hanabi can't be added after startup. The GPU instances exist only while the GPU backend draws. The Effects setting (Full, Reduced, Off) and `--effects gpu|cpu|off` override the choice.
  - `select_backend` is a pure function with a unit test for every row.
- **Time.** Effects run on an effect clock: game speed (`Time<Virtual>`) up to 1x, real time (`Time<Real>`) above it, stopped when paused. On the GPU, a system in `Last` sets `Time<EffectSimulation>`'s relative speed to 1 at 1x and below, and above 1x to the frame's real delta ÷ virtual delta (clamped), so both backends agree even under the catch-up cap. Lifetimes are the later of game hours and a real-time floor.
  - The kill cam is a `game_ui` policy that sets `Time<Virtual>` to 0.25 for a real second and restores it only if nobody changed it. It fires for a captain, a legend or the champion killed by a blow, found by the victim's role, never at 8x or in fast-forward. It never touches sim state.
- **Bounds.**
  - Effects only inside the detail band (bodies also in the low-poly band).
  - Per-frame burst caps, ranked told kill > kill > maim > hit, nearest first. On the GPU, 8 hits and 2 kills a frame, sized so each instance holds its bursts: (bursts a second ÷ instances) × droplets × lifetime stays within its asset's capacity (hit 1,024, kill 4,096, fountain 512, dust 1,024 to start), checked by a test over render.ron.
  - Capped droplets, bodies (300), pools (200) and parts (40), in render.ron `effects`.
  - Everything clears on a new world (`RenderSet::Epoch`).
- **Randomness.** Hanabi's internal `rand::rng()` stays inside the crate and never reaches the sim. CLAUDE.md's "never thread_rng" covers our code, which still never calls it (clippy's `disallowed-methods`). Our droplets and part arcs use stable hashes of record ids.

## Consequences

- **The sim, saves and replays are untouched.** No sim crate depends on game_render. The only sim-side changes are exempt scratch (`Casualties.struck`, `fallen`'s victim, `maim_checks`, `maims`) and a `Fall`'s position, role and the killer's epithet as it stood, which the saga needs anyway. Nothing of the layer is saved or recorded, and after a load the field is clean.
- **Headless builds, tests and CI need no GPU.** `sim_headless` and `integration_tests` don't depend on game_render. game_app's tests build apps without render plugins. `cargo test --workspace` compiles Hanabi into game_render and never runs it. CI's workflow doesn't change.
- **Every machine draws something sensible:**
  - a GPU with compute gets the full spectacle;
  - WebGL2, GL, old drivers and software renderers get the CPU spray at a few dozen droplets a burst;
  - Off draws no spray; bodies, pools and parts stay, and the mild tone has no blood or parts.
- **NFR-10 is the budget:**
  - on the reference machine, the stress battle's worst clash window keeps 60 fps and stays within 1.5 ms of the same window measured before the effects layer, and the effects systems (ours and Hanabi's main-world systems) take 0.5 ms of CPU or less;
  - the CPU fallback takes 1 ms or less at its caps;
  - on lavapipe, Reduced adds at most 10% over Off.

  Real clashes take minutes to reach, and about 15 minutes at a software renderer's 3 to 4 fps, so `--effects-bench` drives the layer at its caps with no battle. Absolute fps is judged on your RTX 4070, as a check handed over to you that doesn't block a step; the dev box (lavapipe under Xvfb) checks the smoke runs, the screenshots and the relative cost.
- **A Bevy upgrade now also waits for bevy_hanabi,** or puts `effects/gpu.rs` behind a feature until it catches up, and the CPU fallback carries on.
- **The finish-time gate depends on bevy_app's finish loop** (0.19.1). Bevy is pinned exactly, and the smoke runs exercise it on every backend: lavapipe forced to each, `WGPU_SETTINGS_PRIO=webgl2` and `WGPU_BACKEND=gl` on the dev box, and automatic on your RTX 4070.
- **Memory.** Each Hanabi asset's slab holds the larger of 65,536 particles and the sum of its instances' capacities, at about 48 bytes a particle: about 3 MB an asset, 12 MB for the four.
- **Pools are flat meshes, not decals:** they lie on the terrain's normal and fade in quantized material steps to keep batching.
- **The figure has no limbs.** A severed arm or leg leaves the body unchanged, a severed head swaps in a headless figure mesh, and maims show as an eye patch, a bound stump and a limp.
- **A dead captain's banner falls** instead of vanishing.

## Rejected

- **Our own GPU particles** (a compute shader of ours). Weeks of WGSL and pipeline work to maintain against every Bevy release, for what Hanabi already does, tested on lavapipe in its own CI. Its pooled-burst pattern (`EffectSpawner::reset()` on pre-spawned instances) fits our requests directly.
- **CPU particles only.** They work everywhere, and they're kept as the fallback. But over-the-top density in a 7,500-a-side battle would spend CPU frame time the sim needs: its share is already capped at 25 ms a frame (ADR 0012). The GPU does the dense part where it can.
- **No effects** (words only). You asked for the spectacle; the words alone were the first plan.
- **Off removing bodies, pools and parts too.** It would make Off exactly today's picture, but your decision draws them identically on every backend, and the mild tone already takes the gore away.
- **Hits from health drops seen by the renderer.** Cheap, but a deserting recruit's stats change drops their health by 40 with no blow, and starvation, slips and brawls lose health too; the sim's own list of landed blows is exact and survives 8x.
- **Adding `HanabiPlugin` unconditionally,** or inside `game_render::RenderPlugin`. Without compute shaders the game quits on a validation error; without a renderer Hanabi's `finish()` panics, which would hit any future test that adds game_render's plugin.
- **A cargo feature for the GPU backend now.** Nothing would build its off path (CI's clippy and `cargo lint` build default features), so it would rot before the Bevy upgrade it exists for. The single module is enough of a seam until then.
- **Probing the adapter before `App::new`** with a direct `wgpu = "=29.0.4"`. It's cleaner than a finish-time gate and lets Bevy be pinned to the probed adapter. But it's a new direct dependency (your OK needed) and costs tens to hundreds of milliseconds at startup. Kept as an option.
- **Creating the renderer by hand** (`RenderCreation::Manual` with `initialize_renderer`). No new dependency, but there's no window yet, so adapter choice can't ask whether it can present, which matters on machines with two GPUs. Kept as the fallback if the finish-time gate ever breaks.
- **Decals for pools.** Forward decals need a depth prepass over every opaque mesh, including the instanced crowds. Clustered decals need compute shaders, so they wouldn't draw the same on every backend.
- **One Hanabi instance per hit.** A new instance compiles and allocates before it ticks, and instances don't batch. Hundreds would cost frames and arrive late.
- **Storing the wound part in the sim.** It's flavour derived from the record, like a weapon word. Storing it would hash and save words. Maims are the exception, because they change the sim (SIM-59).
