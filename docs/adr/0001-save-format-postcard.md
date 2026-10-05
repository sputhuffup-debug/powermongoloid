# ADR 0001: Save format uses postcard, not bincode

Date: 2026-10-05 · Status: Accepted · Requirements: GP-12, NFR-07

## Context

SEED.md originally specified `serde` + `bincode` (zstd-compressed) for save files. The bincode project is dead. Its final crates.io release, 3.0.0, contains only `compile_error!` and a README saying development has ceased. The 2.x line still compiles but will never receive fixes.

## Decision

Save files are serialized with `serde` + `postcard` (1.1.x), compressed with `zstd`, and preceded by a versioned header (magic bytes, format version, game build, seed). `bincode` must not be added to the workspace, directly or as a deliberate choice of transitive dependency.

## Consequences

- postcard is maintained, serde-based and compact (varint encoding), so the data model needs no changes.
- postcard is not self-describing. Schema evolution relies on the header's format version plus explicit migration functions, which we would have needed with bincode anyway.
- Load/save throughput must be measured against NFR-07 (≤ 3 s for the full world) when M4 implements saving.
