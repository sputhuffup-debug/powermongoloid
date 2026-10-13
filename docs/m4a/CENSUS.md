# M4a: the smithy and edict balance census (lane B, 2026-10-13)

Lane B's work in the gap before the smoke-playtest verdict (MP2, [M4A_PLAN.md](../M4A_PLAN.md) "Dependencies and merge points"). It measures what a forge town makes with and without Forge, what Forge and Fields cost in food, what a Tithe hands an army within reach, and how big a Levy is, on the run islands of seeds 1 and 7. It was measured twice: on the MP2 layout (specialties off) and with step 9's specialties ([STEP9a.md](STEP9a.md)).

## How it was measured

`tests/tests/census.rs` (ignored; it measures and asserts nothing):

```sh
cargo test --release -j 2 -p integration_tests --test census -- --ignored --nocapture
CENSUS_SPECIALTIES=0 cargo test --release -j 2 -p integration_tests --test census -- --ignored --nocapture   # the MP2 layout
```

Every free town but the rival's is handed to the player at tick 0 as an invitation (no fallout, no scar), with its people at loyalty 0.9 and the director silenced. Each variant then sets its dials at tick 0, which the first hourly step applies at tick 100, and runs six days and an hour:

| Variant | Dials |
| --- | --- |
| owned | defaults (the "without Forge" line) |
| forge | Work at Forge everywhere |
| fields | Work at Fields everywhere |
| tithe | Forge everywhere, and Tithe at the four best forge towns, each with a player captain of 10 unarmed men parked 20 tiles off |
| levy | Levy at the four biggest towns, each with a captain parked at it |

"Weapons" means a town's weapons in its storehouse, workplaces and homes and in its residents' hands. Nobody takes them in these variants except the tithe, so the change is what the town made. Food is `FoodSecurity` (storehouse and homes).

## Weapons per forge town

Without Forge, only a town's second smithy makes weapons, because the layout alternates recipes. "Specialties" means step 9 as shipped, with Fishing at 160 coast tiles and Timber at 200 forest-edge tiles (STEP9a deviation 1).

| | seed 1, no specialties | seed 1, specialties | seed 7, no specialties | seed 7, specialties |
| --- | --- | --- | --- | --- |
| Island, six days, owned (no Forge) | 212 (three two-smithy towns) | 82 (one) | 0 | 86 (Mining #24) |
| Island, six days, Forge | 1,716 | 1,543 | 1,492 | 1,514 |
| A one-smithy town on Forge, while its ore lasts | 12–25 a day | 12–25 a day | 12–24 a day | 12–24 a day |
| A two-smithy town on Forge | 24–39 a day | #17: 33–43 a day | — | Mining #24: 30–39 a day |

**Ore decides how long it lasts.** A town starts with one ore per villager (economy.ron `initial_stock_per_villager`), and a weapon takes two ore and a wood. A town with no mine burns through its stock on Forge in 2 to 5 days (a big one in about 7) and then makes nothing. Under specialties, the towns that keep going are the ones whose mine keeps pace:

| Town (specialties) | People | Smithies, mines | Weapons a day on Forge, days 1–6 | Ore at each dawn |
| --- | --- | --- | --- | --- |
| seed 1 #20 Mining | 163 | 1, 1 | 13 19 18 20 20 20 | 248 222 214 220 190 144 |
| seed 1 #22 Mining | 152 | 1, 1 | 14 22 18 12 15 16 | 265 243 250 226 234 216 |
| seed 1 #18 Mining | 68 | 1, 1 | 10 9 11 8 8 11 | 98 84 58 42 42 22 |
| seed 1 #19 Mining | 43 | 1, 1 | 2 3 5 2 4 2 | 78 90 86 84 80 90 |
| seed 7 #24 Mining | 310 | 2, 2 | 31 33 30 35 39 35 | 533 488 467 448 441 418 |
| seed 7 #14 Mining | 161 | 1, 1 | 13 13 12 18 15 17 | 264 248 231 208 200 177 |
| seed 1 #5 Farming (for contrast) | 156 | 1, 0 | 19 18 16 16 6 3 | 232 156 54 6 0 0 |

Specialties change which towns sustain it. Without them, seed 1's #20 and #22 drew their ore down (10 and 9 forge trades). With them they have 18 and 16 forge trades (miners x4, smiths x2), and their ore mostly holds. Seed 7's #24 gains a second smithy and mine and makes weapons even without Forge (86 in six days, none before). `tests/tests/specialty.rs` prints the trades: miners 3 to 8 and smiths 3 to 6 at seed 1's #20, and miners 4 to 15 and smiths 6 to 10 at seed 7's #24.

**Forge moves few people at a Mining town:** its smithy, mine and camp are already full ("forge trades owned -> forge" 18 -> 18, 16 -> 16, 31 -> 31). Its Forge works by flipping the recipe, not by moving hands. More hands need more places: step 8's ordered smithy (step 4's deviation 1 still holds).

## The food cost of Forge and Fields

Island food value (storehouses and homes) after six days:

| | owned | Forge | Fields |
| --- | --- | --- | --- |
| seed 1 (specialties) | 9,345 | 8,904 (−5%) | 9,717 (+4%) |
| seed 7 (specialties) | 8,027 | 7,488 (−7%) | 8,894 (+11%) |
| seed 1 (no specialties) | 9,547 | 8,833 (−7%) | 10,084 (+6%) |
| seed 7 (no specialties) | 8,443 | 7,945 (−6%) | 9,138 (+8%) |

Under Forge, every Mining town ends six days with 3 to 7 days of food, its hunger unchanged (0.05–0.09), and no one starving. The design priced Forge at "about 10–15% less food"; it costs 5–7%, because Forge moves farmers only while a town has a day of food, and only into places that are free. The towns that end low on Forge were already low: seed 1's #24 (Market, 111 people) has 1.2 days owned and 0.9 on Forge, and #15 (Market, 168) drops from 4.7 to 2.6 days. Fields adds 4–11% in six days, through more hands at food workplaces that have room.

**Seed 7's #22 starves in every variant.** It is a 280-person Farming town that runs out of food by day 6 whatever its dials (hunger 0.5 to 0.8, with or without specialties). Nearly all the island's starving people (199 owned, 107 on Forge, 105 on Fields in the final run; which variant is worse varies from run to run) are its people. This is the site's problem, not an edict's: it is too big for its land. It is noted for step 10 and M4b.

## Tithes to an army within reach

A captain of 10 parked 20 tiles off, on Forge and Tithe. Each cell is a day's tribute line, food value then arms (specialties):

| Town | The hour it is set | Days 1–6 |
| --- | --- | --- |
| seed 1 #17 (two smithies) | 29 food | 4/16, 6/16, 6/19, 4/14, 4/22, 6/12 |
| seed 1 #23 Market (with a mine) | 29 food | 6/6, 6/14, 9/6, 8/13, 6/7, 7/13 |
| seed 7 #24 Mining | 29 food | 6/11, 6/19, 9/19, 7/18, 7/16, 6/16 |
| seed 7 #6 Market | 29 food | 5/6, 7/9, 9/14, 6/9, 7/11, 6/9 |

A Forge-and-Tithe town hands a passing army **6 to 22 arms a day**, every day, and keeps its baggage full of food. The food is limited by the baggage's room (army.ron `gather_target_days`): a band of 10 eats about 9 food a day, so after the first fill it takes only what it ate. A ten-man band is fully armed in the first day. (In the run without specialties, seed 7's #24 sent no food at all. The ordinary supply of an owned town most likely kept its collector's baggage full; this was not traced further.)

## Levy sizes

A captain parked at each town collects every muster within the hour, so nothing waits on the green at dawn:

| Town | People | Men in six days (seven lines) | People after |
| --- | --- | --- | --- |
| seed 1 #17 | 358 | 47 | 311 |
| seed 1 #2 | 352 | 46 | 306 |
| seed 7 #4 | 310 | 41 | 269 |
| seed 7 #24 Mining | 310 | 41 | 269 |
| seed 7 #22 (starving) | 280 | 26 (five lines) | 279 |

That is **2% of the people a dawn**, about 7 men a day from a town of 350, as designed (`levy.per_day` 0.02). The town shrinks by 13% in six days, and the levied men are unarmed recruits. A levy town paired with a Forge-and-Tithe town nearby raises and arms a band of about 40 in six days.

## Verdict and data proposals

**A Forge town really arms you without starving itself.** A Mining town on Forge makes 8 to 39 weapons a day on its own ore and keeps going; a farming town makes 12 to 25 a day until its starting ore is gone; both stay fed. Forge's food cost is 5–7%, well within the design's 10–15%. No edict or economy data change is clearly needed for that goal, so **none is applied** (the specialty thresholds were changed for a different reason: STEP9a deviation 1). Proposals for step 10 (balance, against `bot_run`'s win-day floor):

1. **Weapons may now be too plentiful.** Forge gives 12–25 a day per smithy, above step 10's target of "8–12 a day for a mining town". Weapons are +8 attack (army.ron), the largest strength lever. If the bot's win days fall under the floor once it forges and tithes, slow weaponsmithing **in a run only**: a run-only cycle multiplier on Forge, or fewer starting ore per villager in the run preset. economy.ron's shared `weaponsmithing.duration` and `initial_stock_per_villager` would move the developed world's stdout.
2. **Forge at a full Mining town moves nobody.** Its places are full. If step 10 wants Forge to visibly move people there, raise `edicts.forge_cap` only together with more places (step 8's ordered smithy, six places). Raising it alone changes nothing.
3. **Seed 7's #22** (280 people, starving whatever its dials) wants a worldgen or size look in M4b. Edicts don't cause it.
4. **Tithe food is bounded by baggage room.** That is right: an army collects what it can carry. The design's "2.4 days of food" in one line happens only for an empty, large baggage.
