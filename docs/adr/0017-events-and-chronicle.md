# ADR 0017: Events, the chronicle and the run summary

Date: 2026-10-09 · Status: Accepted · Requirements: UI-05, UI-09 (also GP-16, GP-21, SIM-36)

## Context

M4 tells the player what happened ([M4_PLAN.md](../M4_PLAN.md), "Events, chronicle and run summary"): an event feed, a chronicle with causes ("Oakworth invited Brand's band in, after they drove off the raiders on day 4"), a report after each battle, and a summary at the end of a run. The event plumbing landed in step 1 ([ADR 0013](0013-runs-presets-and-run-config.md) mentions it); this records it with the step 9 additions. Constraints:

- **The sim stores no text** (render and UI only read; names and phrasing change without changing outcomes).
- **Determinism.** Writers in one system set run in executor order; messages live two ticks and are not hashed.
- **A big battle promotes dozens of soldiers at once,** and the feed keeps 256 entries.
- **Names outlive people.** A fallen champion or a promoted soldier may be despawned before anyone reads about them.

## Decision

- **Events.** Any sim crate writes a `SimEvent` (kind, faction, subject, other, place, amount, `Cause`). One reader, `record_events` (Bookkeeping, `BookkeepingSet::Record`), sorts the tick's events by a total key, stamps the tick, appends every event to the `EventFeed` (exempt, a 256-entry ring numbered by `seq`; saved so UI cursors carry on) and chronicled kinds to the `Chronicle` (hashed as lengths plus a running digest, so hashing stays O(1); saved). `settle_outcome` records `RunEnded` itself. `SimEventKind` and `Cause` are append only.
- **Causes.** Every chronicled event names why it happened where there is a why. Step 9 appends `Cause::Lapsed` (a request nobody answered in time), `Cause::Neglect` (a monument that drifted back to nobody) and `Cause::Fight { faction }` (a battle, against its first foe), and sets causes on raids (`Raid { captain }`) and on the director's raiders. Only `RunStarted` and `RankUp` keep `Cause::None`.
- **Battle reports.** `veterans::end_battles` writes a `BattleReport` into `Chronicle::reports` with each army's `Battle` event (same tick, faction and captain; `Chronicle::report_for` finds it): the factions it fought (`Captain.battle_foes`, bits set by `resolve_strikes`), its dead and the foes it killed (`battle_losses`, `battle_kills`), the turning point (`battle_rout`: the first tick `build_index` sees one of its men running; `None` if the line held), and the ranks earned with each soldier's `NameId`. Reports are folded into the chronicle's digest. Traits earned from triggers come with SIM-34's triggers in a later milestone; for now "traits earned" are the veteran ranks.
- **Grouping.** `VeteranRank` is one event per army and rank reached (`other` = how many), not per soldier; the names are in the report. `ChampionFell` carries the champion's `NameId` in `amount`. `OfferChosen`'s `amount` is the offer kind's tag, so a reader knows which deck the card is from.
- **Words.** `sim_military::chronicle::describe(world, entry)` (and `describe_entries` for many) writes one sentence per entry: "Day N: " + what happened, with settlement, monument and people names from the `NameTable`, faction names from army.ron, and the cause worded by progression.ron `chronicle.causes` (one phrase per `Cause`, with `{faction}`, `{settlement}`, `{captain}`, `{boon}` and `{foe}` filled in, validated). A `Battle` sentence adds its report: who fought, who fell, the foes slain, where the line broke or that it held, and the Elites by name.
- **Summary.** `RunSummary::of(chronicle, stats, progress, run_state, ticks_per_day, now, top_rank)` is a pure function: outcome and cause, days played, settlements taken, lost and held (from `Conquest`, `Invited` and `Revolt` entries), people held (the last dominion tally), monuments taken and held, battles, kills and losses, veterans and Elites (distinct soldiers in the player's reports), requests answered, boons. `sim_military::run_summary(world)` reads it from a world.
- **The military log** gains a `seq` counter and `since(cursor)`, so the feed can merge its lines with the `EventFeed` by cursor.
- **sim_headless.** `stats::run_summary(app)` and `stats::chronicle(app)` give lines; `--chronicle` prints the chronicle in words and the summary at the end of a plain run, `--bot` or `--load`; the `--stats-every` loop prints the summary when a run ends.

## Consequences

- The UI reads `EventFeed` by `seq` and `MilitaryLog::since` for the feed, `Chronicle::entries`/`reports` plus `describe_entries` for the chronicle, and `run_summary` for the end screen. It never formats events itself.
- New `Captain` fields and causes change every scenario's state hash (nothing is golden); the developed and skirmish outputs are byte-identical.
- The chronicle grows by a report per battle (about 30 in ten days of a bot run); its hash stays O(1).
- Tests: `chronicle` (a three-day bot run: a battle with its report, a monument taken by holding it, a request lapsed or answered, the run's end, each with its cause; every entry described; the summary agrees), `champion` (the grouped `VeteranRank` and the report's promotions), and the sim_military unit tests `entries_read_as_sentences_with_their_causes`, `the_summary_counts_the_run` and `a_battle_ends_after_a_quiet_hour_and_ranks_its_fighters`.
