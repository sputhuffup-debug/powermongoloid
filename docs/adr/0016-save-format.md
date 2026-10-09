# ADR 0016: Save format and resume

Date: 2026-10-09 · Status: Accepted · Requirements: GP-12, NFR-07, NFR-08 (follows [ADR 0001](0001-save-format-postcard.md))

## Context

M4 saves and resumes a run ([M4_PLAN.md](../M4_PLAN.md), "Save and resume"). ADR 0001 chose serde + postcard + zstd behind a versioned header, never bincode. This decision covers how the world is written and read back. Constraints:

- **A resumed game must be the same game.** After a load, the state hash must equal the saved one, and both must stay equal tick for tick under the same commands (NFR-08), with either executor.
- **Entities change identity.** `Entity` bits are not stable across apps; `SpawnId`s are.
- **The map is big.** The run island has a million tiles of four arrays (about 7 MB), the developed world four times that, and world generation is deterministic.
- **Not everything the sim reads is hashed.** The army index and fighter grid (exempt: rebuilt each tick) are read by the next tick's input step before they are rebuilt; the decision trace is exempt but every person carries one.
- **NFR-07:** saving and loading the full world in at most 3 s.

## Decision

- **Layout.** `sim_core::save_world(&mut World) -> Vec<u8>` writes a `SaveHeader` with postcard (`magic: b"WLSV"`, `format` = `SAVE_FORMAT` = 1, `build` = the crate version, `seed`, `preset` name, `run: Option<RunConfig>`, `tick`), then the zstd payload (level 3, with a checksum):
  1. the SpawnIds of every simulation entity, ascending;
  2. every registered resource, in registration order: its type name, a presence byte and a length-prefixed blob;
  3. every registered component type, in registration order: its type name, a count, and a length-prefixed blob of `(entity index, value)` in SpawnId order.
  `read_header` reads the header alone (a resume menu). `load_world(&mut World, &[u8])` checks the magic, the format and every type name against the registry **before** changing anything, then despawns any simulation entities, clears pending commands and events, spawns one entity per SpawnId, loads resources and components, and runs the load hooks. Each blob must be read to its last byte, which catches a `load` that doesn't mirror its `save`.
- **`SaveState` mirrors `StateHash`.** Each saved type writes its fields to a `SaveWriter` and reads them from a `SaveReader` in the same order. Integers are little-endian and floats keep their bits; `Entity` fields are written as `SpawnId + 1` (0 for none, e.g. a despawned target) and read back as the entity now carrying that SpawnId (`Entity::PLACEHOLDER` for none, which like the despawned entity refers to nothing). Plain structs use `sim_core::save_fields!`, newtypes `save_newtype!`, markers `save_unit!`, and serde enums without entities `save_serde!` (postcard stores the variant index, so saved enums stay append only, as `CommandKind` and `OrderSpec` already are).
- **Registration.** `AppSaveExt::register_saved` / `register_saved_resource` next to every `register_hashed`; `register_saved_resource_with` for resources that load from others; `on_load` for hooks. `HashRegistry` now keeps the hashed TypeIds apart from the exempt ones, and the test `every_hashed_type_is_saved` fails if a hashed type isn't saved. Exempt types may be saved too.
- **Data resources are saved as their RON text** (Calendar, Economy, Villagers, Army, WorldGenSettings, Progression, DirectorData) and rebuilt with their `from_ron` (Villagers against the loaded calendar, Army against the calendar and economy), so a save keeps its tuning and the hash, which reads the same text, matches.
- **The map is regenerated** from the loaded seed and worldgen settings when `WorldMap` loads; the save holds its digest and the terrain and revision of every chunk changed since generation, laid over the regenerated map, and a different digest refuses the save ("does not match this build"). The run island takes about 0.4 s to regenerate, the developed world about 1.4 s.
- **What is rebuilt, and what is saved although exempt.** Load hooks start `PathCache`, `FlowCache` and the inbox, casualties and deserters empty, and rebuild `SpatialGrid` from positions (as Bookkeeping left it). The `ArmyIndex` (armies and slot signatures, without flow fields) and the `FighterGrid` are saved, because the next tick's raiders, rival and attack hold read them before `build_index` runs. `DecisionTrace` is saved as presence only. The `EventFeed` and `MilitaryLog` are saved so the UI's cursors carry on. Pending commands are not saved: save between ticks, and after a load schedule the rest of a session's commands with `sim_headless::resume_commands`.
- **sim_headless.** `SimPlugins.generate` (default true) and `WorldPlugin.generate`: `false` builds an app without a world for a save to load into. `load_app(bytes, threading) -> Result<App, SaveError>` builds it from the header and loads; `save_app`, `continue_run` and `resume_commands` run the rest of a session. CLI: `--save <file>` (at the end, or `--save-at <tick>`), `--load <file>` (`--ticks` more, or with `--replay`, the rest of that replay with its hashes checked).

## Consequences

- Measured on seed 42 (test profile): a run saved after a day is 354 KB, saved in 17 ms and loaded in about 0.45 s; a developed skirmish save is 468 KB and loads in 1.4 s (release). Both are well inside NFR-07.
- `save_load_restores_an_identical_hash` plays a run with the bot for a day (a warband takes a hamlet, the rival's lands and garrison, raiders, guard camps, the start offer left pending), saves, loads with both executors, compares the hash and every entity's components, re-saves to identical bytes, and plays 1,000 more ticks of the same commands with equal hashes every 50 ticks. The CLI checks the same against a replay: `--replay run.ron --save s.sav --save-at 1500`, then `--load s.sav --replay run.ron`.
- The conquest scenario's branches now load one saved prefix instead of replaying it.
- A save only loads into the build that wrote it (same registered types and format, and the same worldgen code, or the digest check refuses it). Migration between formats is future work, keyed on `SAVE_FORMAT`.
- Archetype order differs after a load (entities arrive in SpawnId order), so any system whose result depends on query order would diverge; the save/load test is the guard for that rule (DETERMINISM.md).
- Tests: `save_load` (2), `sim_core::save` unit tests (round trip with entity references, refusals before any change, reader errors), the conquest branches.
