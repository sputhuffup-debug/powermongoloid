# M4 build notes (working files)

Working notes for building M4 "Rise" (docs/M4_PLAN.md). Not part of the developer handbook; they can be removed once M4 ships.

- BUILD_GUIDE.md: the code map and binding decisions (D1-D13) the M4 steps were built against, with gotchas. Line numbers drift; search by name.
- PROGRESS.md: what each step actually built, its deviations from the guide (these override the guide), names later steps use, and open issues. Steps 1-9, plus the step 12 closing entry at the end.
- PROGRESS_UI.md: step 10 (game screens) and step 10b (saves, resume and the chronicle in the game).
- PROGRESS_BOT.md: step 11 (the winning bot, balance run, benchmark, playtest checklist).

M4 was built on 2026-10-07 to 2026-10-09 and awaits the human playtest in docs/M4_PLAYTEST.md. The decisions are recorded in ADRs 0013-0017 (each ends with an "As built" section written at step 12), the acceptance criteria are ticked in docs/SEED.md, and the agent rules for runs, standing, modifiers and saves are in CLAUDE.md.
