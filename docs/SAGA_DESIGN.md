# Design: Saga, kill lines and legends

Status: **draft 2026-10-12, for your approval** (revised the same day after review). Your four decisions are recorded in the Decisions section at the end, and the open questions there each have a recommended default. M4c is planned in `docs/M4C_PLAN.md`, and SEED.md has its section ("M4c: Saga") and new requirement rows. This doc answers your ask: "for combat we need some flavor in the kill messages to make it more brutal and we can make specific soldiers have legendary histories".

Your answers (2026-10-12):
- **Milestone:** Saga is its own milestone, **M4c "Saga"**. It comes after the M4 playtest and before M4b "Living towns".
- **Tone:** brutal and graphic by default, with an option for milder wording. Both are phrase banks in data (`assets/data/*.ron`).
- **Legends have small gameplay effects.** Traits earned from experience (brave, cruel, beloved, shaken) finish SIM-34's deferred triggers and nudge morale and AI. A legend steadies nearby men, and a legend's death shakes the army. There are no hero units and no big stats.
- **Across runs:** legends are remembered in M6's legends book of past runs, as read-only history. No power carries over. M4c writes the record M6 will read.

## 1. What you'll see

Most runs leave you remembering the map. Saga should leave you remembering three or four people.

A typical arc: Tobin Kettle, a baker of Ashby, joins the band on day 2. At Crane's Ford Tobin draws first blood with a fence post. Tobin holds the line there, at the Mill and at Loweby, and the warband starts calling Tobin **the Unbroken**. At Kemp's Ford the line breaks; Tobin stays, fights, and kills the raider captain. Now it's **Tobin Banner-Taker**, a legend of the warband, and the men nearby hold a little longer. On day 9, at the Barrow Stones, a great grey wolf tears Tobin open. The game pauses, the warband falters, and a cairn rises where Tobin fell, with a stone in Ashby. The end screen reads:

> **Tobin Banner-Taker**, a baker of Ashby. Six battles, fourteen kills, among them captain Rook Vane at Kemp's Ford. Stood when the line broke at Kemp's Ford. Brave. Fell at the Barrow Stones on day 9, to a great grey wolf.

Every fact in that paragraph is a record the sim keeps (section 5 lists the fields). Every word is chosen when it's read, from data, in the tone you picked. That's the Dwarf Fortress lesson: keep structured history (who, whom, where, when, how), and let legends emerge from it rather than from scripts.

What changes for you:
- **Kill lines.** Notable deaths get one brutal line each in the feed. A big battle doesn't flood it: the rest are summed up in one line in its report ("Eleven more died in the furrows near Loweby").
- **Battle reports with moments:** first blood, who ran first and when the line broke, who fell, who slew whom, the best killer, and the names earned.
- **People with records.** Click a soldier to see their kills, battles, wounds and deeds, and a short biography.
- **Epithets** earned by deeds: Nine-Kills, Banner-Taker, the Unbroken, Doorwarden. Each says why it was earned.
- **Earned traits.** Brave, Shaken, Cruel and Beloved come from what a soldier lives through, and act only through that soldier.
- **Legends.** A few soldiers per faction become legends. Enemy legends too: the rival's long-lived captains earn names you learn to fear.
- **Memorials.** A cairn where a legend fell, and a stone in their home village.
- **The end screen** lists the run's legends, the foes you remember and the fallen. A small record of them is kept for M6's legends book.

## 2. Kill lines: who killed whom, and how (SIM-39)

**Every death by a blow is attributed.** Today the game knows only which faction struck the killing blow. In M4c, every death records the killer, the victim and the manner:
- armed or bare-handed;
- cut down fleeing;
- a last stand (the victim was killed after their army **broke**, while not running themselves);
- one blow (from full health to dead);
- saved: the victim was striking at the killer's captain, champion or a legend;
- kin: one settlement on both sides of a revolt, or one household;
- a beast's kill or a beast slain;
- first blood;
- the killer's third, fifth or eighth kill of the fight.

Starving to death is recorded too, with no killer.

**Three battle terms,** defined in data (`progression.ron` `saga`) and in the glossary, because deeds, traits and words depend on them:
- **Broke:** an army breaks at the first tick when at least `broke_share` (0.3) of its living members are routing. `build_index` sets the new `Captain::battle_broke` beside `battle_rout` (the tick the first man ran, which stays the report's turning point).
- **Lost:** an army lost its battle if it broke, or if its battle ended with more losses than kills.
- **Held:** a person held in a battle if they fought in it and never routed in it (`Veteran.routed` stayed false).

**Battles get a number.** Today a battle is one army's episode, so two armies' reports of one fight can't be matched. In M4c:
- **Joining.** When a blow lands between two armies, an army already in a battle keeps its number. One that isn't takes the other army's number, or a new one from `Saga.next_battle` if neither has one. This happens in the strike loop's battle bookkeeping, in striker SpawnId order. Two armies already in different battles keep their own numbers, so a third army joining a fight joins the number of whoever it struck first.
- **Militia** have no army: a militia blow joins the battle of the army it fights.
- **What a battle keeps.** `Saga` keeps each battle's facts: the tick and position of its first blow, the settlement nearest it, its ordinal at that settlement, the terrain there, its lines told and its first blood. The number is stored on `Captain` with `battle_since` and passes to an heir.
- **Whose battle a death is.** A death counts in its victim's army's battle, or in the striker's for militia and rebel victims. A death with no army on either side is told only if it's tier A.

**Only notable deaths are told.** A battle of 1,000 a side kills about 300 people, and the feed shows a handful of lines. Deaths fall into three tiers:

| Tier | Who | Told |
| --- | --- | --- |
| **A, always** | The victim is a captain, the champion or a legend; or a legend makes a milestone kill (their first and fifth of a fight, and every tenth of their life) | Always, with one line per death, by precedence champion > legend > captain. The champion's death is told by its `ChampionFell` event, now naming the killer, and a legend's by `LegendFell` (section 7); their `Fall` is kept as told, so the feed shows the event's line and not a second one. Everyone else's is a told death in `Saga` |
| **B, if remarkable enough** | Everyone else, scored by what made the death remarkable (table below) | If the score reaches the battle's **rising bar**, at most 10 a battle |
| **C, counted** | The rest | In the battle report's "N more died" |

The rising bar is RimWorld's tale interest, made deterministic. A tier-B death is told if its score reaches **40 + 15 × (lines already told in its battle)**. So the first notable death needs 40, the fourth needs 85 and the seventh needs 130. Early moments get told, and later ones must be remarkable.

**Caps, for every tier.**
- **Per tick:** at most 8 told deaths a tick across all tiers. Tier A takes its slots first and is never dropped. Tier B gets what remains, in victim SpawnId order.
- **Per battle:** tier B is capped at 10 told deaths a battle (`max_per_battle`).
- **The bar:** every told line, tier A or B, raises its battle's bar.

So a big battle tells at most 10 tier-B lines plus its captains, champion and legends.

| What made it remarkable | Score |
| --- | --- |
| The victim: a soldier or raider / a militiaman | 10 / 15 |
| First blood of the battle | +60 |
| The killer and victim are one household (a revolt) | +80 |
| The victim was striking at the killer's captain, champion or a legend | +50 |
| A last stand | +50 |
| The victim had an epithet | +40 |
| The killer is a legend (a kill that isn't a milestone) | +40 |
| The victim had an earned trait | +30 |
| The killer's 3rd / 5th / 8th kill of the fight | +30 / +50 / +80 |
| A beast kills, or is killed | +40 |
| The killer and victim are of one settlement (a revolt) | +40 |
| A militiaman killed at their own village | +30 |
| Cut down fleeing | +20 |
| One blow | +15 |

The weights, the bar and the caps are gameplay rules, in `progression.ron` (`saga.notable`). Rules about who is told are hashed data, and the same seed tells the same deaths.

**Kept is not told.** Every told death is kept as a `Fall` in `Saga`. So is the untold death of anyone **named**: a person with an epithet, an earned trait or legend status, or someone a report or a chronicled line names (a told death's killer, a best killer, whoever ran first). An untold `Fall` gets no line, but biographies, reports and the end screen can name the dead. Everyone else's death is counted. That bounds what's kept to the told deaths plus the few named people.

**Hunger is not a blow.** A starved captain, champion or legend gets a line. Other starvation deaths are summed once an hour per army: "Four of Garrick Hale's band starved near Ashby."

**Reading at 4x and 8x.** People read about four words a second. At 8x a three-minute clash lasts about 22 real seconds: room for five to seven short lines. The feed therefore filters told deaths by speed. This is display only: the sim, the reports, the end screen and headless output keep every told death.

| Speed | Told deaths shown in the feed |
| --- | --- |
| 1x–2x | Any where your people killed or died, plus every tier-A death of anyone |
| 4x | Tier A involving your faction or a legend, plus first blood in your battles |
| 8x and fast-forward | Tier A involving your faction or a legend |

At every speed the feed shows at most one line every 1.5 real seconds per place, and folds the overflow into "…and 3 more fell at Loweby". The speed filter is a pure function in `sim_military::saga` (`feed_filter`), so `bot_run` can check the volume: over every seed, the deaths the 8x filter passes stay at **8 or fewer a battle**.

## 3. Words: brutal by default, mild on request (UI-10)

**Where the words live.** A new `assets/data/saga.ron` holds words only, in two tones. The sim never reads it:
- It's not hashed and not saved, so editing a line changes no hash, replay or save, and an old save reads in the current build's words.
- Rules (what's notable, deeds, epithets, traits, legends) stay in hashed `progression.ron`. Ids join the two files.

**Tone is your setting, not the world's.** Brutal or Mild is passed to the words layer when a line is read. It's never a command, a run setting, part of a save or part of a replay.
- Switching it rewrites the whole history at once: the feed, the chronicle, reports and biographies.
- The game remembers your choice in a small preferences file next to its saves. You set it on the new-game screen, or with a switch on the feed.
- Headless has `--mild`.

**How a line is built.**
- **Key.** A death picks the most specific key that fits, in this order: `champion`, `legend`, `captain`, `rebel`, `militia_victim`, `by_legend`, `blood`, `first_blood`, `saved`, `last_stand`, `kin`, `streak`, `militia_killer`, `beast_kills`, `beast_slain`, `fleeing`, `one_blow`, `raider`, `armed` / `unarmed`.
  - A militiaman or rebel victim always takes its grave key (`militia_victim` or `rebel`), so a villager's death is never told with a soldier's brutal line.
  - First blood and a streak still reach those lines through the `{moment}` slot.
- **Variant.** Each key has at least six brutal and four mild variants.
- **Slots.** The variant's slots are filled from part banks:

| Slot | Fills with | Example |
| --- | --- | --- |
| `{killer}`, `{victim}` | The name. With an epithet, the given name and the epithet | "Wat Ashby", "Tobin Banner-Taker", "Kael Nine-Kills" |
| `{killer_given}`, `{victim_given}` | The given name, so lines never need a pronoun | "Tobin" |
| `{weapon}` | Armed: a weapon word fixed for life per person, so Wat Ashby always swings the same axe. Unarmed: their old trade's tool, or a word from the unarmed bank | "an axe", "a felling axe", "a flail", "a fence post", "bare hands" |
| `{wound}` | Where the blow went (flavour; brutal lines use it, mild lines usually don't) | "through Ulric Vane's jaw" |
| `{ground}` | The terrain where it happened | farmland "in the furrows", forest "among the roots", beach "on the shingle", road "on the road", a settlement "in the lane" |
| `{place}` | The nearest settlement | "near Loweby" |
| `{home}`, `{trade}` | The victim's village and old trade (one of the sim's occupations) | "of Ashby", "a baker" |
| `{beast}` | A beast noun, fixed per beast. Beasts carry human names today; the words never use them | "a great grey wolf", "the boar of the Barrow Stones" |
| `{foes}`, `{army}` | Faction nouns from saga.ron (this fixes "against Rival"), and an army by its captain | "the raiders", "the rival's band"; "Garrick Hale's band" |
| `{count}` | A number in words up to twenty | "Eleven" |
| `{daypart}` | The time of day, from the tick | "at dawn", "at noon", "after dark" |
| `{moment}` | A short first-blood or streak phrase for lines whose key doesn't say it, in the key's register | "First blood at Loweby:" |

**What each key must say.** Each key lists its **fact slots** (for example `{killer}` and `{victim}`; `{count}` and `{place}` for a summary) and its **flavour slots** (`{weapon}`, `{wound}`, `{ground}`, `{daypart}`, `{moment}`). Every line, in both tones, names all of its key's fact slots and uses no slot outside the key's list. Mild lines may leave the flavour out.

**Variety without randomness.** Words never draw from `SimRng`:
- **Kill lines.** A told death picks its variant by its index in `Saga`'s told deaths: variant `(index × 37 + salt) mod bank length`, where every bank is shorter than 37. Two told deaths closer together in the record than a bank's length never share a variant of that bank.
- **Other parts** use a stable hash of ids and the tick.
- **The result:** the same event reads the same in the feed, on the end screen, in headless and after a load.
- **Breadth from composition.** About 6 lines × 7 weapons × 8 wounds × 2–3 grounds gives roughly 700 to 1,000 armed variants per tone.
- **A brutal flourish is rationed.** In a battle report only the top one or two moments get one; the rest use terse report lines. Brutality hits harder when it's rarer.

**Content rules** (written into `docs/dev/DATA.md` with the banks, and checked by a lint test where a test can check them):
- **Toward soldiers, raiders and beasts:** graphic. Concrete verbs, one wound, the ground, about 14 words at most.
- **Fit the feed.** A feed row is 520 px at 12 pt, about 80 characters, and the line follows a "Day N: " prefix. So every line that can reach the feed fits **72 characters** when filled with the longest names, epithets and places in the data. Report-only lines ("more died", deserters, moments) may run to 110.
- **Name first** in every line that can show at 8x (the `champion`, `legend`, `captain` and `by_legend` keys), so a glance gets who.
- **Toward militia, rebels and villagers:** grave, never gleeful, even in brutal. They die "in the lane, fork in hand". (Civilians can't be struck at all; only militia and rebels die by blows.)
- **Never:** sexual violence, torture as spectacle, mutilating the dead for laughs, slurs, or children.
- **No gender.** There is no gender in the sim, so lines use names, given names, roles or "they". The lint denies the pronouns (he, she, his, her, him, hers) and gendered nouns (man, men, woman, women, son, daughter, wife, husband, brother, sister, mother, father, widow, lad, lass, boy, girl). Design prose may still say "men" for soldiers; lines may not.
- **Mild is sober, not cute.** "Killed", "fell", "was cut down": a clerk's battle report, not a children's book.
- **Words may invent the manner, never the facts.** The wound, the weapon word and the ground are flavour. Who, whom, where, when, first, last and the outcome always come from records. Times come only from the `{daypart}` slot, never from a fixed phrase.

**Sample lines** (names are examples; trades are the sim's occupations):

| Key | Brutal (default) | Mild |
| --- | --- | --- |
| armed | Edric Moss put an axe through Ulric Vane's jaw in the barley. | Edric Moss killed Ulric Vane near Loweby. |
| armed | Wat Ashby opened Hild Crane from collarbone to belly in the mud. | Wat Ashby cut down Hild Crane. |
| unarmed, old trade | Hild Crane, a farmer once, flailed Bram Tull's skull to pulp. | Hild Crane, a farmer once, killed Bram Tull with a flail. |
| unarmed, bare hands | Sten Cole held a raider under the ditch till the kicking stopped. | Sten Cole drowned a raider in the ditch, bare-handed. |
| one_blow | One blow from Sten Cole, and Ulric Vane's skull came apart. | Sten Cole killed Ulric Vane with a single blow. |
| fleeing | Bram Tull ran. Piers Rook ran faster, and hacked till Bram stopped. | Bram Tull fled and was cut down by Piers Rook. |
| last_stand | The line broke around Brenna Hale, who stood and was cut apart. | Brenna Hale stood after the line broke, and fell. |
| first_blood | First blood: Wendel Orme's cleaver, through Aric Brook's face. | First blood: Wendel Orme killed Aric Brook. |
| streak | Three dead at Kael Dyer's feet, and Kael still swinging. | Kael Dyer has killed three in this fight. |
| by_legend | Kael Nine-Kills made it ten, splitting a raider from hip to hip. | Kael Nine-Kills took a tenth life. |
| captain | Rowan Kemp hacked through captain Garrick Hale's neck. | Rowan Kemp killed captain Garrick Hale. |
| champion | Isolde Marsh, the champion, died on Odo Venn's axe. | Isolde Marsh, the champion, was killed by Odo Venn. |
| legend | Ulric the Wall fell to Osric Pike's spear at Ashby Ford. | Ulric the Wall was killed by Osric Pike at Ashby Ford. |
| saved | Jory Lowe split Odo Venn's head a breath from Garrick Hale's back. | Jory Lowe killed Odo Venn, who was attacking Garrick Hale. |
| avenged (open question 1) | Freya Barrow gutted Osric Pike, and Brenna the Stubborn is avenged. | Freya Barrow killed Osric Pike, avenging Brenna the Stubborn. |
| militia_victim (grave) | Cedric Tanner, a baker of Loweby, died in the lane, fork in hand. | Cedric Tanner, a baker of Loweby, died defending the village. |
| militia_victim with `{moment}` | First blood at Loweby: Cedric Tanner, a baker, died in the lane. | First blood at Loweby: Cedric Tanner, a baker, was killed. |
| militia_killer | Odo Rook of Loweby buried a hay fork in a raider's chest. | Odo Rook of Loweby killed a raider. |
| rebel (grave) | Linnet Ware rose against the warband and was hacked down in Ashby. | Linnet Ware rose against the warband and was killed in Ashby. |
| blood | Jory Lowe killed Hollis Lowe: one hearth, two sides, one grave. | Jory Lowe killed Hollis Lowe, of the same household. |
| beast_kills | A great grey wolf tore out Merrick Ellis's throat. | A grey wolf killed Merrick Ellis at the Barrow Stones. |
| beast_slain | Hollis Fenwick hacked at the boar long after it stopped moving. | Hollis Fenwick killed the boar of the Barrow Stones. |
| raider | Raiders left Una Greaves gutted in the mud at Kemp's Ford. | Raiders killed Una Greaves at Kemp's Ford. |
| starved (sober in both) | Four of Garrick Hale's band starved to death near Ashby. | Four of Garrick Hale's band starved near Ashby. |
| more_died (report) | Eleven more died in the furrows near Loweby, and the crows came down. | Eleven more died near Loweby. |
| deserted (report) | Six more threw down their spears and ran for home. | Six deserted during the fight. |
| named | They call Kael Dyer Nine-Kills now, and they mean it. | Kael Dyer is called Nine-Kills now. |
| trait_earned | Perrin Dyer, Shaken at Loweby, stood at Ashby. Brave now. | Perrin Dyer held at Ashby and is Brave now. |
| legend_rose | Tobin Banner-Taker, fourteen dead behind, is a legend now. | Tobin Banner-Taker has become a legend of the warband. |

Hunger is told plainly in both tones: brutality is for blows. There are no execution lines, because there are no prisoners. A routed soldier struck down is a fleeing kill.

## 4. Battle reports with moments

Each army's battle report (`BattleReport`, UI-09) gains its moments:
- **The battle.** Its number and name, so both sides' reports of one fight are recognisably one fight.
- **First blood:** the battle's first told death, and whether this army drew it. It's stored once on the battle, so both reports agree.
- **Who ran first, and when the army broke.** The report already has the turning point, the tick its first man ran; now it names who ran, and adds the tick its army broke (section 2), if it did.
- **The told deaths** this army suffered or dealt, in order (read from `Saga` by battle number).
- **The best killer:** most kills in the fight, ties to the lower SpawnId. The honour stands even if the best killer died later in the battle.
- **The wounded, and the deserters** during the battle. In big battles desertion dwarfs death.
- **What its people earned:** veteran ranks (as today), epithets, earned traits and legends risen.
- **Died to the last.** In a run, an army killed to the last man still gets its report. Today it gets none (ADR 0017).

"Eleven more died" is worked out when the report is read: its losses minus its named dead.

**Battle names** are words, but the facts behind them are fixed when the battle starts. The first blow fixes:
- the settlement nearest it;
- its ordinal at that settlement, counted in `Saga` at that moment;
- the terrain there.

The words make "the Battle of Loweby Fields" (the settlement plus a ground noun) and later "the Second Battle of Loweby Fields". Both sides' reports and every later line name the fight the same way.

**A report as told (brutal):**
> **The Battle of Loweby Fields**, day 4. Your warband against the raiders.
> First blood: Wendel Orme's cleaver, through Aric Brook's face.
> Perrin Rook ran first; the raiders broke at noon.
> Fell: Brenna Hale, on a raider's spear. Slain: raider captain Oswin Thorne, by Jory Lowe.
> Best killer: Kael Dyer, four. Eleven more of yours died in the furrows, and nineteen of theirs. Six threw down their spears and ran.
> Earned: Jory Lowe is called Banner-Taker; Odo Rook is Brave; three rose to Veteran.

**The same report (mild):** "The Battle of Loweby Fields, day 4: your warband against the raiders. First blood: Wendel Orme killed Aric Brook. Perrin Rook fled first; the raiders broke at noon. Fell: Brenna Hale. Slain: captain Oswin Thorne, by Jory Lowe. Best: Kael Dyer, four kills. Eleven more of yours died, and nineteen raiders. Six deserted. Jory Lowe is called Banner-Taker; Odo Rook is Brave; three became Veterans."

## 5. Service records, deeds and epithets (SIM-53)

**A service record** (`Service`) starts the first time a person has something to record: a kill, a wound, a deed or a mark toward a trait. Beasts never get one. It holds:

| Field | What | Bound |
| --- | --- | --- |
| `trade`, `home` | Their old trade (captured when they enlist, before enlisting makes them idle) and home settlement, if any | — |
| `joined` | The tick they first enlisted | — |
| `kills`, `unarmed`, `fleeing` | Kills, and how many were bare-handed or of fleeing foes | — |
| `streak` | Kills in the current battle, with its number | — |
| `battles`, `held`, `wounds` | Battles survived; battles held while their army broke or lost a quarter of its fighters; battles survived after losing at least a quarter of their health to blows | — |
| `fights` | The latest battles they fought, as (battle number, kills) | 16 |
| `deeds` | Each deed once, as (deed id, tick, battle number or nearest settlement, victim `NameId` if any) | One per kind of deed |
| `marks` | Four counters toward the earned traits (section 6) | — |
| `traits` | The latest earned-trait changes, as (trait id, tick, battle number) | 4 |
| `epithet` | The epithet id, when it was earned and the word variant | 1 |

So the tooltip's "nine kills, at Loweby and Crane's Ford" reads `fights`. The epitaph's "captain Rook Vane at Kemp's Ford" reads the Banner-taker deed. "Joined the warband on day 2" reads `joined`.

It **outlives desertion and re-enlistment**: today's `Veteran` is reset on re-enlistment and removed on desertion, and `Service` is neither. When the person dies, their record is copied into their kept `Fall` if their death is kept (section 2: told, or named). Otherwise it's counted and dropped.

Health never comes back in the sim today. A soldier with two wounds fights on with half their health or less. Their blows land as hard as ever, since damage doesn't depend on health, but they die sooner. The record says "the Scarred", and the body agrees.

**Deeds** (`SagaDeed`) are each recorded once per person. Fame is worked out from the record when needed, never stored: **fame = kills + battles + the weights of their deeds.** Fame is a per-person score with no unlocks. It isn't FUN_DESIGN's faction renown, which stays not adopted.

| Deed | Earned when | Fame |
| --- | --- | --- |
| First blood | Drew the first blood of a battle | 2 |
| Banner-taker | Slew a captain | 6 |
| Champion-slayer | Slew a champion | 10 |
| Legend-breaker | Slew a legend | 10 |
| Beast-slayer | Slew a monument's beast | 4 |
| Last stand | Made a kill after their army broke, and survived the battle | 5 |
| Shield | Slew a foe who was striking at their captain, champion or a legend | 4 |
| Five in a fight | Five kills in one fight | 4 |
| Survivor | Lost half their health in one battle, and lived | 2 |
| Held the line | Held in a battle in which their army broke or lost a quarter of its fighters | 1 |
| Doorwarden | As militia, killed a soldier or raider at their own village | 3 |
| Kinslayer | Killed one of their own settlement (a revolt) | 2 |
| Ran first | The first of their army to run (shame) | 0 |
| Redeemed | Became Brave after being Shaken | 3 |
| Breadgiver | A captain whose army fed a hungry village | 2 |
| Plunderer | A captain whose army plundered | 0 |
| Avenger | Slew the slayer of one of their faction's legends (open question 1) | 6 |

**Epithets: earned by rule, one per person, explained.**
- **Rules** are rows in `progression.ron` (`saga.epithets`). Each row has a stable `id`, which saves and run records store, and a separate `rank`. A higher rank outranks a lower one, so ranks can be re-tuned and new epithets slotted in anywhere without relabelling old saves or records. Ids are append-only.
- **One per person.** A person carries one epithet, replaced only by a higher-ranked one ("Nine-Kills became Banner-Taker at Kemp's Ford").
- **Words.** Each epithet's words, in both tones, are in saga.ron, keyed by id. Where there are several variants, one is chosen per person for life.
- **Every epithet says why** in a tooltip: "Nine-Kills: nine kills, at Loweby and Crane's Ford".
- **Who.** Epithets are run-only and come to every faction except beasts.

| Id | Rank | Rule | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 0 | 0 | 8 battles | Old Iron | Old Iron |
| 1 | 1 | 4 kills bare-handed | Bare-Hands / Knuckles | the Wrestler |
| 2 | 2 | 2 wounds | Half-Face / the Stitched / Old Scars | the Scarred |
| 3 | 3 | Doorwarden | Doorwarden | Doorwarden |
| 4 | 4 | Ran first, and 5 kills | the Hare | the Runner |
| 5 | 5 | 9 kills | Nine-Kills | Nine-Kills |
| 6 | 6 | Beast-slayer | Wolfsbane / Boar-Sticker / Beast-Bane | Beast-Bane |
| 7 | 7 | Shield | Shieldfast / the Shield | the Shield |
| 8 | 8 | Held 3 times (`held` ≥ 3) | the Unbroken / the Wall | the Unbroken |
| 9 | 9 | Last stand | Last-Standing / the Stubborn | the Steadfast |
| 10 | 10 | 20 kills | the Butcher / Red-Hands / the Reaper | the Grim |
| 11 | 11 | Cruel, and 6 kills of fleeing foes | the Hound | the Hunter |
| 12 | 12 | Beloved, and Breadgiver | Breadgiver | Breadgiver |
| 13 | 13 | Redeemed | the Redeemed | the Redeemed |
| 14 | 14 | Banner-taker | Banner-Taker / Captainsbane | Banner-Taker |
| 15 | 15 | Avenger | Blood-Debt / the Avenger | the Avenger |
| 16 | 16 | Kinslayer | Kinslayer | Kinslayer |
| 17 | 17 | Legend-breaker | Legend-Breaker / Giantkiller | Legend-Breaker |
| 18 | 18 | Champion-slayer | the Headsman / Champion's Bane | Champion's Bane |

Every epithet is gender-neutral. "Spearwife" from the first pitch is dropped (open question 3).

## 6. Earned traits (SIM-34)

SIM-34's triggers were deferred in M4: only the champion gains traits, by picking them at a rank-up. In M4c, **captains, the champion, legends and soldiers who have survived three battles** earn traits from what they live through. That matches SIM-34's "captains, heroes and long-serving soldiers" and keeps the numbers small. Heroes come in M4b and can reuse this.

**Earned traits act only through their holder:**
- on the holder's own morale, break point, aim and desertion;
- for a captain, also on the captain's own dealings (recruiting, plunder).

They never touch another person's morale, and never go into the faction-wide `FactionModifiers`, so one brave soldier never makes a whole faction braver. A person with no earned trait, under a captain with none, behaves exactly as before.

| Trait | Earned when (marks in the service record) | Effect, through the holder | Opposite |
| --- | --- | --- | --- |
| **Brave** | 2 *held* marks: a battle in which they held while their army broke or lost a quarter of its fighters | Breaks at morale 0.20 instead of 0.25; morale target +0.03; desertion ×0.75 | Replaces Shaken (and earns the Redeemed deed) |
| **Shaken** | 2 *broke* marks: a battle in which they ran and their army lost, or they ran first, or they lost half their health | Breaks at 0.30; morale target −0.03; desertion ×1.25 | Replaces Brave |
| **Cruel** (new) | 3 *cruelty* marks: a battle in which they killed a fleeing foe or one of their own settlement; for a captain, also each plunder by their army | +0.02 morale per kill (double the usual); aims at a fleeing foe within 3 tiles of the nearest foe; a Cruel captain's plunder costs ×1.5 goodwill | Replaces Beloved |
| **Beloved** | 2 *kindness* marks: for a captain, each time their army gives food to a village short of food or completes a bring-food request; for anyone, a battle in which they held at their own village | Desertion ×0.75; a Beloved captain's recruit chance ×1.1 in their own dealings | Replaces Cruel |

- **Marks count only from eligibility,** and a battle gives at most one mark of each kind. Marks earned before a person is eligible are not banked, so nobody turns Cruel the moment they pass their third battle.
- **Opposites replace each other.** This gives arcs ("Shaken at Crane's Ford, Brave again at Loweby") and keeps the effects bounded: at most two earned traits per person, and at most one earned per battle.
- **Target rates,** checked by `bot_run`'s counts: at the end of a won run, at most a quarter of eligible veterans hold an earned trait, and Cruel is no more common than Brave.
- **Reuse.** Brave, Beloved and Shaken reuse their names and phrases from `progression.ron`. Cruel is appended as trait id 7 and kept out of the champion's pool.
- **Data.** The numbers above are in `progression.ron` (`saga.earned`).
- **Run-only, and an exact no-op without a trait** (`× 1.0`, `+ 0.0`), so the developed world, the skirmish and the M2/M3 tests don't move.

**Picked and earned traits are separate.**
- The champion's picked traits stay in `Traits` and act faction-wide through `FactionModifiers`, as built. Earned traits live only in `Service` and act on the holder.
- **Offers** (`draw_traits`) ignore earned traits.
- **Succession** moves half the picked traits, as today, and never moves earned ones: they belong to the person.
- **One trait both ways.** An earned trait the champion already picked isn't awarded again, because the faction-wide effect already covers the champion. An earned trait opposite a picked one is allowed: the picked one still acts on the faction, and the earned one on the champion alone. The inspector shows both: "Brave (chosen) · Shaken (earned at Crane's Ford)".
- **A new event kind.** Earning a trait is the new `TraitEarned`, not today's `TraitGained`. `TraitGained` is chronicled and notable: it resets the director's quiet clock and stops fast-forward, which a rare pick at a rank-up should do and dozens of earned traits per run should not. A separate kind keeps `notable()` a plain rule over kinds.

## 7. Legends (SIM-54)

**Who becomes a legend:**
- **Threshold.** Fame 25, at least three battles survived, and an epithet.
- **When.** Checked when a battle ends and once an hour.
- **The marker.** `Legend { faction, rose }`: the faction they rose in and the tick.
- **Cap.** At most **three enlisted legends per faction**, counted by current allegiance (the same set `LegendIndex` holds). When the cap is full, nobody new rises until one falls or leaves.
- **For life.** A legend is never demoted. A legend who deserts keeps the title in words ("Tobin Banner-Taker, gone home to Ashby") but frees the slot. If they re-enlist, they may take a faction over three for a while, and nobody new rises there until the count drops below three. Scarcity keeps it special.
- **Any faction except beasts.** The rival's long-lived bands make **enemy legends** ("The rival's band speaks of Alda the Butcher"), which makes a Legend-breaker possible. Raiders rarely live long enough.
- **Numbers, to be tuned against the bot:** one to three player legends in a won run, and at least one on three of the five `bot_run` seeds. Epithets are more common, three to ten a run, and carry the mid-game.

**A legend steadies nearby men.** This is a small aura, about a third of a captain's:

| | Radius | Effect | Attack |
| --- | --- | --- | --- |
| A captain (today, `army.ron` `leadership`) | 14 tiles | Morale target +0.1, recovery 0.6 a day | +15% |
| A legend (new) | 8 tiles | Allies break at 0.03 lower morale; morale target +0.03 | None |

Auras don't stack, a legend doesn't steady themself, and only an enlisted legend counts. A legend who deserts goes home and steadies no one.

**A legend's death is a moment:**
- **One event per death,** by precedence champion > legend > captain. A champion who is a legend gets `ChampionFell` (which already pauses), with the legend's record. A captain who is a legend gets `LegendFell`.
- **Shock.** Their army, and their faction's fighters within 12 tiles, lose 0.10 morale once. A captain who is also a legend costs the larger of the captain's 0.15 and 0.10, not the sum. Foes within 12 tiles gain 0.03 once.
- **The event.** `LegendFell` is chronicled and notable. Fast-forward stops on it.
- **Pause.** The game auto-pauses for your own legends only. It's added to every difficulty's `auto_pause` in `director.ron`, and the feed checks the faction.
- **The record.** The death is kept with the legend's full record. The killer's name is kept too, even if the killer dies the same moment.
- **Memorials.** A **cairn** stands where the legend fell, and a **memorial stone** at their home village's centre, if they had one. Hovering either shows the epitaph. Both are drawn by the renderer from the sim's records; there's no sim entity.

**Grudges, a light nemesis (open question 1):**
- **Named.** The enemy who killed a legend is named in later lines and on the end screen: "Osric Pike, who slew Brenna the Stubborn".
- **Avenged.** If one of the legend's faction later kills that enemy, the death is always told and the killer earns the Avenger deed.
- **No more than that.** No AI hunts anyone, and nobody is promoted for it. FUN_DESIGN's "nemesis rivals" otherwise stay not adopted.

## 8. Biographies, and where it all shows (UI-09, RND-09)

**Biographies** are two to four sentences, put together from the record when read:
- *"Tobin Kettle, called Banner-Taker. A baker of Ashby; joined the warband on day 2."*
- *"Six battles, fourteen kills, among them captain Rook Vane at Kemp's Ford; wounded twice."*
- *"Held the line three times; stood when it broke at Kemp's Ford. Brave."*
- *"Fell at the Barrow Stones on day 9, to a great grey wolf."* or *"Lives, scarred, with Garrick Hale's band."*

Spawned soldiers have no home: "a sword of the starting band", "a raider out of the north". An ordinary soldier gets one line: "3 battles, 2 kills, wounded once."

**Settlement histories** are the other half of UI-09's biographies: *"Ashby sent fourteen to the warband; six lie dead. Tobin Banner-Taker came from here. Conquered on day 3; rose on day 6, when Linnet Ware was killed in the square."*

| Where | What you get |
| --- | --- |
| **Feed** | Told deaths (read from `Saga`), `Starved`, `Named`, `TraitEarned`, `LegendRose` and `LegendFell` lines, coloured by kind (your dead, your kills, legends in gold); the speed filter and fold (section 2); clicking a line selects the killer if alive, or else moves the camera there; a Brutal/Mild switch |
| **Inspector** | The epithet after the name; "Kills 14 · Battles 6 · Wounds 2"; deeds in words; earned traits with what earned them; a legend's "Steadies men nearby". The biography replaces "Memories: none yet" for anyone with a record (other villagers keep it until M5's memories) |
| **Captain panel** | "Notables": the army's legends and up to three other named men; clicking one selects them |
| **Settlement panel** | The settlement's short history |
| **Battle reports** | The feed's battle line opens the whole report with its moments; the end screen uses the same words |
| **End screen** | "Legends of the run": up to six, yours first, each with their numbers and fate in one line and the epitaph on hover. "Foes remembered": enemy legends, and whoever killed one of your legends or your champion. "The Fallen": how many of your people died, with the named ones listed. The bloodiest battle, by name. The timeline gains legends risen and fallen |
| **Map** | A small pennant over each living legend inside the detail band; cairns and memorial stones |
| **New-game screen** | "Words: Brutal / Mild" beside the difficulty buttons |

Two military log lines that print a SpawnId where a name belongs ("the captain fell; 4127 takes command", and the champion's succession line) are fixed to print the name.

## 9. What M6's legends book will read (GP-29)

When a run ends, the game writes a small **run record** next to its saves (in `legends/` under the save directory), before the finished run's save slot is removed. The record:
- **Holds facts and proper nouns, never sentences.** Names and places are written as plain strings, because a run's name table dies with it. Epithets, deeds and traits are ids, so M6's book can tell them in its reader's tone with its build's words.
- **Is read-only history.** No code reads it in M4c, and nothing in it changes a later run (your decision 4).
- **Is built the same way in headless.** A pure function builds it from the finished world, as the run summary is built, so `sim_headless --legends-out <path>` writes the same record.
- **Is one file per ending:** `legends/<seed>-<difficulty>-<ended tick>.ron`. A run finished again after a quickload, at the same tick, overwrites it, and a different ending writes another file. Both are true histories of play, and M6 decides how to show them.
- **Covers abandoned runs too,** with outcome `Abandoned`, so the book can choose to list or hide them.

```ron
(
    record: 1, seed: 7, difficulty: Harsh, outcome: Won, cause: Conquest, days: 31,
    champion: (name: "Brand Ashby", epithet: Some(14), rank: 3, traits: [0, 5], earned: []),
    legends: [
        (name: "Wat Ashby", home: Some("Ashford"), faction: 1, trade: Some(Baker), epithet: Some(5),
         kills: 14, unarmed: 4, battles: 9, wounds: 2,
         deeds: [(FirstBlood, day: 3, place: "Crane's Ford", victim: Some("Aric Brook")), (LastStand, day: 11, place: "Kemp's Ford", victim: None)],
         earned: [Brave],
         rose_day: 12, fate: Fell(day: 27, place: "Loweby", killer: Some("Alda Venn"), killer_epithet: Some(14))),
    ],
    foes: [ /* enemy legends, and whoever killed one of your legends or your champion, same shape */ ],
    fallen: (yours: 23, named: ["Brenna Hale", "Garrick Hale"]),
    battles: [ /* the three bloodiest, by number and name facts, with losses and kills */ ],
)
```

## 10. How it fits what's built (technical notes)

**The pipeline in one picture.** The schedule gains one step, **slot 13 of the hourly progress driver, after `director::step`**. The driver has 12 slots today: `dominion::refresh_slots` shares slot 10, the rival clock is 11 and the director is 12. Everything else lives inside existing systems.

```
Input      orders::run_orders       plunder and food given leave cruelty and kindness marks on the captain
Knowledge  index::build_index       + who ran first and when each army broke (broke_share);
                                    LegendIndex (each faction's enlisted legends; run only)
Decide     soldiers::decide         personal break point (Brave, Shaken, a legend nearby); Cruel aim (run only)
Act        combat::resolve_strikes  a kill pushes (victim, killer, credited side, one blow); a blow adds to the victim's
                                    hurt this battle; the battle bookkeeping joins or allocates battle numbers
           combat::casualties       pass 1, killers in strike order: snapshot each killer, start or update their record,
                                    streaks, best killer
                                    pass 2, the dead in SpawnId order: classify; tell tier A, then tier B against the bar
                                    and the caps (Fall / LegendFell / ChampionFell); keep named deaths; a legend's shock;
                                    then today's work (morale entry, goodwill, succession, despawn)
Needs      upkeep::soldier_upkeep   marks "ran this battle"; legend aura; trait nudges to morale target and desertion
           progress slot 1          end_battles: wounds, held and broke marks, deeds, the report's moments, judging survivors
           progress slot 13 (new)   saga::settle: judge every record changed since the last hour; the hourly starvation lines
Words      chronicle::Teller        kill lines, moments, biographies; the dead named from the saga's records; a tone parameter
```

**The sim stores no text** (ADR 0017):
- Records hold SpawnIds, `NameId`s, ids for epithets, deeds and traits, and counts.
- Epithet strings never go into the `NameTable`, which is hashed and saved.
- The dead are named from their kept `Fall`s, copied before they are despawned. Anyone ever named (section 2) has their death kept, so a named person is never told as "a soldier".

**Told deaths are not events.** They live in `Saga`, append-only, and the feed reads them with its own cursor. So a big battle can't push `OfferPosted` or `LegendFell` out of the 256-entry `EventFeed` ring during a frame hitch, and the auto-pause is never lost. `Saga` keeps every told death of the run, so the feed never misses one either.

**New state:**

| Type | What | Hash | Save |
| --- | --- | --- | --- |
| `Casualties.kills` | This tick's kills (replaces `slain_by`) | Exempt scratch, as today | No |
| `Veteran` gains `routed`, `hurt`, `trade`, and the holder's cached nudges | Ran this battle; share of health lost to blows this battle; trade before enlisting; break-point offset, morale-target offset and desertion multiplier, written only by `judge` (and rebuilt from `Service` on enlisting) so the parallel loops never read the sparse `Service` | Yes | Yes |
| `Service` (new, sparse) | The service record (section 5) | Yes | Yes |
| `Legend` (new) | `{ faction, rose }` | Yes | Yes |
| `Saga` (new resource) | Told and kept deaths (`Fall`, append-only), battles (number, first blow, place, ordinal, terrain, lines told, first blood), per-settlement battle counts, legends risen, grudges, untold count, `next_battle` | Lengths and a running digest, like the `Chronicle` (O(1)) | Yes |
| `Captain` gains `battle`, `battle_broke`, `battle_best`, `battle_ran_first`, `battle_deserted`, `starved` | The battle number and this army's moments | Yes | Yes |
| `BattleReport` gains moments | Section 4 | In the chronicle's digest | Yes |
| `LegendIndex` | Rebuilt in Knowledge before any reader | Exempt | No |
| `SagaWords` | saga.ron | Exempt ("words only; the sim never reads it") | No |
| Tone, feed cursor and throttle state | game_ui | Outside the sim | Tone in the preferences file |

- **Append-only enums.**
  - `SimEventKind` gains `Starved`, `Named`, `TraitEarned`, `LegendRose` and `LegendFell`.
  - `Cause` gains `Earned { deed }`.
  - Deed ids, epithet ids, trait id 7 and the death-manner bits are append-only as well. Epithet ranks are data and may change.
- **Saves.** `SAVE_FORMAT` goes to 2. M4 playtest saves will be refused; M4c follows the playtest, so that's expected.
- **Replays.** The replay format doesn't change: no new commands, and the tone isn't in the header. Like every M4 step, the checkpoint hashes change, so replays recorded before M4c must be re-recorded.

**Pacing:**
- Told deaths are not events; `Starved` is feed-only, not chronicled.
- `Named`, `TraitEarned` and `LegendRose` are chronicled for biographies, but are **not notable**. They don't reset the pacing director's quiet clock or stop fast-forward. The end-screen timeline hides `Named` and `TraitEarned`.
- `LegendFell` is chronicled and notable. It's rare and happens in battles, which are notable already.

**Determinism:**
- Kills are recorded in the serial strike loop, in striker SpawnId order, and battle numbers are joined in the same order.
- Deaths are classified in the serial casualty loop, in victim SpawnId order (tier A told first, then tier B), so the indices of told deaths are deterministic.
- Judging runs in SpawnId order.
- The only new parallel write is "ran this battle", each soldier's own flag. The aura and trait nudges change only the morale each soldier already writes.
- Every rule is a threshold, so no new RNG stream is needed. The words use stable arithmetic, never `SimRng`.

**The run gate.** Records and lines exist in every preset. Behaviour changes only in a run:

| | Every preset | Run only (`in_run`) |
| --- | --- | --- |
| Killers, battle numbers, records, told deaths, `Starved` lines, battle moments | Yes: hashes change, stdout doesn't | |
| A rebel's kill credited to the rebels | Yes, wherever rebels fight; the guarded developed, skirmish and battle commands have none | |
| Epithets, deeds that need a run (Breadgiver, Plunderer), earned traits and their effects, legends, aura, shock, the report of an army wiped out | | Yes |

Developed, skirmish and `--battle` stdout stay byte-identical. Kill lines appear in the skirmish too, which helps test the words.

**Performance (NFR-09).**
- **Baseline.** M4's numbers for the 7,500-a-side battle (seed 42: deaths a tick, the clash window's mean tick, and the Act, Decide and Needs stage times) are measured three times at step 1 and recorded, with their spread, in `docs/m4c/`. Nothing in the repo records them yet.
- **Per blow:** one byte added to the victim's battle hurt, and a push onto a vector, on a kill only. No query and no allocation.
- **Per death:** a few microseconds of reads and one record.
- **Per soldier, in a run:** at most three distance checks for the aura, and none when the faction has no legend. Trait nudges are read from `Veteran`, which the loops already hold. The developed stress world has no legends and pays nothing.
- **Two measurements.** `--battle 7500 --timings` covers every preset's bookkeeping. `--battle 7500 --saga-bench --timings` turns on the saga's run-only effects (not the rest of the run rules) and gives each army three legends and every tenth soldier an earned trait, so the per-soldier costs are paid.
- **Target.** For each, the median of three runs of the clash window's mean tick rises at most 2%. If three runs of one build spread more than 2%, the gate compares the Act, Decide and Needs stage times instead.

**Data:**
- `progression.ron` gains a `saga` section (`#[serde(default)]`, like `gather` and `hire`), holding:
  - the battle terms (`broke_share`);
  - the notability weights, bar and caps;
  - deed fame;
  - epithet rows (id, rank, rule);
  - earned-trait triggers and effects;
  - the legend rule, aura and shock.
- `progression.ron`'s `traits` gains Cruel (id 7), and `chronicle.causes` gains the phrase for `Cause::Earned`.
- `director.ron` gains `LegendFell` in `auto_pause`.
- The new `saga.ron` holds the words, two tones of the same shape, so a key missing from one tone fails to parse. It also holds each key's fact and flavour slots, and the feed throttle, which is viewer data like `render.ron`.
- Every field is documented in `docs/dev/DATA.md` (the docs test enforces it).

## 11. Traps and guards

| Trap | Guard |
| --- | --- |
| Log spam in big battles | Tier A narrowed to captains, the champion and legends (and a legend's milestone kills); the rising bar; at most 10 tier-B lines a battle and 8 told deaths a tick; summaries; the feed's speed filter, rate limit and fold; `bot_run` checks at most 8 lines a battle pass the 8x filter |
| Unreadable at 8x | Only tier A about you or a legend at 8x and fast-forward, rate-limited; feed lines fit 72 characters, the name first |
| Repetition kills brutality | Composition, weapons fixed per person, the prime-step variant choice, a lint test of bank sizes, and a brutal flourish rationed to the top moments of a report |
| Brutality turns tasteless, especially toward villagers | Militia and rebel victims always take a grave key; written content rules; a deny-list test; a playtest question |
| Mild reads as comic | A "clerk's report" register; a playtest question |
| Words claim things that didn't happen | "Invent the manner, never the facts": every fact comes from a record, times only from `{daypart}`, trades only from the sim's occupations |
| Wording edits churn hashes and saves | saga.ron is words only, exempt and unsaved; only rules are hashed |
| Kill lines change pacing or stop fast-forward | Told deaths are not events; `Named`, `TraitEarned` and `LegendRose` are not notable; only the rare `LegendFell` is |
| Kill lines push other events out of the feed ring | Told deaths are read from `Saga` with the feed's own cursor, never through `EventFeed` |
| Legends turn into heroes | Three enlisted per faction, an aura a third of a captain's with no attack bonus, a 0.10 shock, traits through their holder only; tests pin the sizes |
| Legends snowball (they kill more, rise more) | Fame counts each deed once; the cap; the aura touches morale only |
| Legends too rare to notice | Tuned against `bot_run` (at least one on three of five seeds); epithets, far more common, carry the mid-game |
| Earned traits everywhere | Marks count only from eligibility, one of each kind per battle; target rates checked against `bot_run` |
| Balance drift | Every effect is run-only and exactly nothing without a trait or legend; `bot_run` is re-run after each step that may change a run |
| Lost names of the dead | Kept `Fall`s hold `NameId`s; anyone ever named has their death kept |
| Beasts read as people | `{beast}` nouns; beasts never fill a person's slot and never get a record |
| Cost | A push on a kill is the only new work per blow; nudges cached on `Veteran`; NFR-09 is measured with `--timings`, with and without `--saga-bench` |
| Name clashes | Goodwill's `Deed` and the champion's `Feat` already exist: the new type is `SagaDeed`, the cause `Cause::Earned`, and the glossary says "Saga deed". "Fame" avoids FUN_DESIGN's "renown" |

## 12. Proposed roadmap change

| Milestone | Was | Proposed |
| --- | --- | --- |
| **M4c "Saga"** | — | New, after the M4 playtest and before M4b: kill lines and battle moments, service records, deeds and epithets, SIM-34's earned traits, legends and memorials, UI-09's biographies, end-screen legends, the tone setting, and the run record for M6 |
| SIM-34 | M4 | M4 (champion traits), M4c (earned traits) |
| UI-09 | M4 | M4, M4c (biographies) |
| M6 | The legends book of past runs | Unchanged; it reads M4c's run records |

New requirement IDs, added to SEED.md on 2026-10-12 (proposed, with M4c):

| ID | Requirement |
| --- | --- |
| GP-29 | A finished run leaves a record of its legends (names, epithets, deeds, fates) for the legends book of past runs; nothing in it changes a later run |
| SIM-39 | Every death in battle records who killed whom and how; notable deaths are told one by one, and the rest are counted into the battle's report |
| SIM-53 | Soldiers keep a service record (kills, battles, wounds, deeds) that outlives desertion and, on death, is kept for anyone told or named; they earn one epithet by rule, which says why |
| SIM-54 | A few soldiers of great fame in each faction become legends: a legend steadies nearby men a little and their death shakes the army; legends gain no stats and are not heroes |
| RND-09 | Living legends are marked on the map; a cairn stands where a legend fell, and a memorial stone in their home village |
| UI-10 | Kill lines, battle reports and biographies are told from phrase banks in data, brutal by default and mild by choice; the choice is a viewer setting the game remembers, and it never changes the simulation |
| NFR-09 | Saga bookkeeping adds at most 2% to the median mean tick of the `--battle 7500` clash, with and without the saga's run effects forced on, with no allocation per blow |

## Decisions (2026-10-12)

Yours, binding:
1. **M4c "Saga" is its own milestone,** after the M4 playtest and before M4b "Living towns".
2. **Brutal and graphic by default, with an option for milder wording;** both are phrase banks in `assets/data/*.ron`.
3. **Legends have small gameplay effects:** earned traits (Brave, Cruel, Beloved, Shaken) nudge morale and AI; a legend steadies nearby men; a legend's death shakes the army. No hero units, no big stats.
4. **Across runs, legends are history only.** M6's legends book reads them; no power carries over. M4c writes the record.

Defaults I chose to make the design concrete (change any):
5. **Words in a new saga.ron, rules in progression.ron.** The words are exempt from the hash and not saved; the rules are hashed.
6. **The tone is a viewer setting,** kept in a preferences file next to the saves: never a command, run setting, save or replay field. Headless takes `--mild`.
7. **Records and kill lines in every preset; effects only in runs.** Developed, skirmish and `--battle` stdout stay byte-identical.
8. **Earned traits act through their holder.** They never reach `FactionModifiers` or another person's morale, and only captains, the champion, legends and three-battle soldiers earn them.
9. **Three enlisted legends per faction at most,** for any faction but beasts.
10. **Memorials are render-only:** a cairn where a legend fell, and a stone in their home village.
11. **The run record holds facts and proper nouns,** not sentences.
12. **Battles get a number,** so both sides' reports of one fight share its name, first blood and rising bar.
13. **Told deaths live in `Saga`, not in the event feed ring.**

## Open questions (with recommended defaults)

1. **A light nemesis.** Should the enemy who killed a legend be named in later lines and on the end screen, with an Avenger deed for whoever kills them? This reopens FUN_DESIGN's "nemesis rivals: not adopted" in the smallest way. *Default: yes, words and the deed only, with no AI change.*
2. **Memorials with effects.** Should a cairn or stone be a sim entity that does something, such as villagers mourning or morale near it? *Default: no in M4c; render-only. Effects fit M5's memories (SIM-14), which can read the same records.*
3. **Gendered epithets ("Spearwife").** *Default: no. Everything stays neutral, and a test bans pronouns and gendered nouns. Tagging given names in `villagers.ron` is possible later; it's hashed data, so the village_day and famine tests would have to be re-run.*
4. **How graphic.** Invented wound detail ("through the eye") as flavour, with a grave register for militia and rebels. *Default: yes, as written in section 3.*
5. **Kill lines outside runs** (the skirmish and `--battle`). *Default: yes for records and lines, no for effects.*
6. **A feed-volume option** (all, mine, legends) besides the tone. *Default: not in M4c; the speed filter covers it, and volume joins M6's options screen. Pulling the tone setting forward from M6 is the one exception.*
7. **Switching tone mid-run.** *Default: yes, from the switch on the feed; the whole history re-renders.*
8. **A blow-by-blow battle log** (Dwarf Fortress's combat reports). *Default: not in M4c. If you want it later, it's a saved UI ring like the event feed, for your own armies only.*
9. **Kinslayer and blood killings in revolts.** They're dark, rare and memorable. *Default: yes.*
10. **Healing.** Wounds never heal, because the sim has no healing; a twice-wounded legend fights on with half their health or less. *Default: leave it, and say so in the inspector ("fights wounded"); healing is a question for M4b's towns.*
11. **Moving the chronicle's cause phrases into saga.ron,** so they get tones too. *Default: not in M4c (it would change every run hash for words); revisit with M6's options.*
12. **Starting numbers:**
    - the bar 40 + 15 per told line, at most 10 tier-B lines a battle and 8 told deaths a tick;
    - an army breaks at 30% of its living members routing;
    - fame 25 and 3 battles for a legend, and 3 enlisted per faction;
    - an aura of 8 tiles (−0.03 break point, +0.03 morale target);
    - a death shock of 0.10, and 0.03 to foes;
    - traits ±0.05 on the break point, ±0.03 on the morale target, desertion ×0.75 or ×1.25;
    - at most 8 lines a battle passing the 8x filter, and at most a quarter of eligible veterans with an earned trait.

    *Default: tune with the bot's saga counts toward one to three player legends and three to ten epithets per won run.*
