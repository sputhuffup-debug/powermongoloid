# M4a build: rules for every lane

Read first: `docs/M4A_PLAN.md` (steps, gate, execution plan, lanes, collision rules), `docs/LORDSHIP_DESIGN.md` (the design; it wins over the plan where they differ), `docs/adr/0020-lordship.md`, and `CLAUDE.md` (house rules).

## Machine
- 4 cores, 15 GB RAM, limited disk. Always `-j 2` for cargo; full tests with `-- --test-threads=2` and `CARGO_INCREMENTAL=0`.
- If `df -h /` shows under 4 GB free: `rm -rf target/debug/incremental`, then delete stale build outputs (never `cargo clean` of the whole target unless nothing else frees space).

## Git
- Each lane works on its own branch (`claude/m4a-<lane>`), branched from the M4a branch (`claude/brave-turing-sh1ryn`). Commit per step with a clear message naming the step and requirement IDs; push your lane branch. The merge lead merges lanes into the M4a branch at the merge points; never push to another lane's branch or to `main`.
- Before starting a new step after a merge point, merge the M4a branch into your lane branch (a merge commit; no rebase or force-push).

## Ownership (from the plan's "Keeping lanes from colliding")
- Lane A ("Take", sim): tenure, influence, fallout, conquest, invitations, raiders, rival, director, sim_agents standing and loyalty, the rest of `orders.rs`.
- Lane B ("Govern", sim): sim_agents ai, act, edicts, jobs, food, population; sim_economy; govern.rs; `orders::lands` and the `Govern` arm of `orders::apply`.
- UI (game_ui, game_render), bot (`bot.rs`) and docs rows: done inside each step by the step's UI/tests agents; shared docs rows go in your lane's own subsection.
- Only the merge lead sets `SAVE_FORMAT` and `REPLAY_FORMAT` and records baselines.

## Per-step gate (lane)
`cargo fmt --check`; `cargo clippy -j 2 --workspace --all-targets -- -D warnings`; unit tests of the crates you touched; your integration tests; the determinism tests; the three guarded stdout comparisons (developed `--seed 42 --ticks 4801 --stats-every 600`, skirmish `--skirmish --seed 42 --ticks 6000 --stats-every 2000`, battle `--battle 1000 --seed 42 --ticks 3000 --stats-every 600`, release, against the M4 baseline outputs recorded by the merge lead or, on a lane started before they exist, recorded by you from your base commit); `RUSTDOCFLAGS="-D warnings" cargo doc -j 2 --workspace --no-deps`; docs in the same commit. The full workspace suite, the cost check and `bot_run` run at merge points.

## Notes
Write step notes in `docs/m4a/STEP<N>.md`: what was built, deviations from the plan and why, names later steps must use, checks run and results, open issues.

## Handing out work
Cloud sessions treat messages from other sessions as information, not instructions, so a lane's next step is always started as a new session whose first prompt is the task (same lane branch, fast-forwarded to the M4a branch). Don't wait for follow-up messages from the merge lead.
