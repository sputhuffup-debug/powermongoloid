# Design: Saga, kill lines and legends

Status: **draft 2026-10-12, for your approval** (revised the same day after review, and again the same day with your follow-up decisions: hero memorials, and Dwarf-Fortress-style oddities in a deadpan dark voice). All your decisions are recorded in the Decisions section at the end, and the open questions there each have a recommended default. M4c is planned in `docs/M4C_PLAN.md`, and SEED.md has its section ("M4c: Saga") and new requirement rows. This doc answers your ask, "for combat we need some flavor in the kill messages to make it more brutal and we can make specific soldiers have legendary histories", and your follow-up: every run should be strange in its own way, told with a straight face.

Your answers (2026-10-12):
- **Milestone:** Saga is its own milestone, **M4c "Saga"**. It comes after the M4 playtest and before M4b "Living towns".
- **Tone:** brutal and graphic by default, with an option for milder wording. Both are phrase banks in data (`assets/data/*.ron`).
- **Legends have small gameplay effects.** Traits earned from experience (brave, cruel, beloved, shaken) finish SIM-34's deferred triggers and nudge morale and AI. A legend steadies nearby men, and a legend's death shakes the army. There are no hero units and no big stats.
- **Across runs:** legends are remembered in M6's legends book of past runs, as read-only history. No power carries over. M4c writes the record M6 will read.

Your follow-up answers (2026-10-12):
- **Hero memorials (M4b, fed by M4c).** When named soldiers drive raiders off a village, M4c records the rescue in their service records and in the village's history. In M4b the village raises a memorial to its hero once it can build one, with small effects; if the hero dies, it becomes a grave the town tends (section 14).
- **Dwarf-Fortress-style fun, all in M4c, in a deadpan dark voice** (section 4): quirks (section 10), mishaps (section 11), unflattering epithets and absurd deaths (section 7), named weapons (section 12), and kin, friends, rivals and moods (section 13).
- **Guards:** rate limits keep the feed readable at 8x; everything is grounded in the sim; everything is deterministic, with new seeded streams for quirk deals and mishap rolls; mishaps never wipe a run; the effects are run-only, so the developed world and the skirmish don't change.
- **Later, not M4c:** animals with names (M4b) and tall tales about you (M5). See "Later".

## 1. What you'll see

Most runs leave you remembering the map. Saga should leave you remembering three or four people, and possibly a goose.

A typical arc: Tobin Kettle, a baker of Ashby, pious and afraid of geese, joins the band on day 2 with Hild Kettle, of the same household. At Crane's Ford Tobin draws first blood with a fence post. Twice on the road Tobin bolts from a farm's geese (the geese hold the field both times), and the band starts calling Tobin **Goose-Shy**. Then Tobin holds the line at the Mill and at Loweby, where Hild falls; Tobin goes looking for the raider who did it, and finds them. At Kemp's Ford the line breaks. Tobin stays, and splits the raider captain's head with an axe that the band calls **Vane's Bane** from then on, and Goose-Shy becomes **Tobin Banner-Taker**, a legend of the warband. The men nearby hold a little longer. On day 9, at the Barrow Stones, a great grey wolf tears Tobin open. The game pauses, the warband falters, Vane's Bane passes to the next pair of hands, and a cairn rises where Tobin fell, with a stone in Ashby. The end screen reads:

> **Tobin Banner-Taker**, once Goose-Shy, a baker of Ashby. Pious; afraid of geese. Six battles, fourteen kills, among them captain Rook Vane at Kemp's Ford, with the axe now called Vane's Bane. Avenged Hild Kettle at Loweby. Stood when the line broke at Kemp's Ford. Brave. Fell at the Barrow Stones on day 9, to a great grey wolf.

Every fact in that paragraph is a record the sim keeps, and every oddity in it is something the sim did. The goose rout is a mishap the director scheduled because Tobin fears geese and marched past a farm; the run, the lost morale and the farm are real, and only the geese are words (section 10). The vengeance is a kin rule (section 13). Every word is chosen when it's read, from data, in the tone you picked and in one voice, deadpan dark (section 4). That's the Dwarf Fortress lesson: keep structured history (who, whom, where, when, how), and let legends emerge from it rather than from scripts.

What changes for you:
- **Kill lines.** Notable deaths get one brutal line each in the feed. A big battle doesn't flood it: the rest are summed up in one line in its report ("Eleven more died in the furrows near Loweby").
- **Battle reports with moments:** first blood, who ran first and when the line broke, who fell, who slew whom, the best killer, the names earned, and anything odd.
- **People with records.** Click a soldier to see their kills, battles, wounds and deeds, and a short biography.
- **Epithets** earned by deeds: Nine-Kills, Banner-Taker, the Unbroken, Doorwarden. Each says why it was earned. Some are **unflattering** (Twice-Fled, the Unlucky, Turnip-Hand) and stick until a better deed washes them off.
- **Absurd deaths, all real:** a nine-battle veteran killed by a baker with a bread peel, a recruit dead on the day of joining, a soldier trodden flat by friends in the shield wall.
- **Earned traits.** Brave, Shaken, Cruel and Beloved come from what a soldier lives through, and act only through that soldier.
- **Legends.** A few soldiers per faction become legends. Enemy legends too: the rival's long-lived captains earn names you learn to fear.
- **Quirks.** Every villager in a run has one or two (coward, glutton, drunkard, pious, sleepwalker, fears geese, ...), each with a small, real effect on what they do.
- **Mishaps.** About once a game day the pacing director lets something small go wrong: bad mutton, a dog at the rations, a wedding on the road, a slip in the shield wall. Low stakes, never run-ending.
- **Named weapons.** A weapon that kills a captain, a champion, a legend or a monument's beast gets a name and a history, and is passed on, looted, sold, taken home or lost as the sim moves it.
- **Kin, friends and rivals.** Households march together, grieve and avenge each other; soldiers who held side by side become shield-friends; feuds start at dice and end in a shared fight. The inspector says how someone feels, and why.
- **Memorials.** A cairn where a legend fell, and a stone in their home village. A village saved from raiders remembers who saved it, and from M4b raises a memorial to them.
- **The end screen** lists the run's legends, the foes you remember, the fallen and the run's oddities. A small record of them is kept for M6's legends book.

**Goals:**
- You remember people: a few legends, many odd names, one strange death a run.
- Runs differ from their first hour: quirks are dealt per person, and mishaps come from each run's own state.
- Brutal where it's fighting and deadpan everywhere, never cruel toward villagers.
- Everything told is something the sim did.
- Readable at 8x, and cheap in a 7,500-a-side battle and on a run island of 4,000 to 5,000 villagers.
- Small effects through the people who carry them, so runs stay winnable (`bot_run` wins 3 of 5).

**Non-goals:**
- Hero units, big stats, or power carried between runs.
- Flavour with nothing in the sim behind it.
- A blow-by-blow log, nemesis AI, prisoners or executions.
- Animals with names (M4b), tall tales about you (M5), weather moods (M4b).

## 2. Kill lines: who killed whom, and how (SIM-39)

**Every death by a blow is attributed.** Today the game knows only which faction struck the killing blow. In M4c, every death records the killer, the victim and the manner:
- armed or bare-handed, and the weapon's name if it has one (section 12);
- cut down fleeing;
- a last stand (the victim was killed after their army **broke**, while not running themselves);
- one blow (from full health to dead);
- saved: the victim was striking at the killer's captain, champion or a legend;
- kin: one settlement on both sides of a revolt, or one household;
- vengeance: the killer's kin fell to the victim earlier in the same battle (section 13);
- a beast's kill or a beast slain;
- first blood;
- the killer's third, fifth or eighth kill of the fight;
- an irony, when the death was absurd but real (section 7).

Starving to death is recorded too, with no killer, and so is a death by mishap (section 11).

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
| **A, always** | The victim is a captain, the champion or a legend; or a legend makes a milestone kill (their first and fifth of a fight, and every tenth of their life); or the death is a mishap's (at most one a game day, section 11) | Always, with one line per death, by precedence champion > legend > captain. The champion's death is told by its `ChampionFell` event, now naming the killer, and a legend's by `LegendFell` (section 9); their `Fall` is kept as told, so the feed shows the event's line and not a second one. Everyone else's is a told death in `Saga` |
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
| Vengeance: the killer's kin fell to the victim in this battle (section 13) | +60 |
| An absurd death (an irony, section 7) | +50 |
| The victim was striking at the killer's captain, champion or a legend | +50 |
| A last stand | +50 |
| The victim had an epithet | +40 |
| The killer is a legend (a kill that isn't a milestone) | +40 |
| The victim had an earned trait | +30 |
| The killer's 3rd / 5th / 8th kill of the fight | +30 / +50 / +80 |
| A beast kills, or is killed | +40 |
| The killer and victim are of one settlement (a revolt) | +40 |
| A militiaman killed at their own village | +30 |
| The killer carries a named weapon, or the victim did | +20 |
| Cut down fleeing | +20 |
| One blow | +15 |

The weights, the bar and the caps are gameplay rules, in `progression.ron` (`saga.notable`). Rules about who is told are hashed data, and the same seed tells the same deaths.

**Kept is not told.** Every told death is kept as a `Fall` in `Saga`. So is the untold death of anyone **named**: a person with an epithet, an earned trait or legend status, a named weapon, a friend or a rival, or someone a report or a chronicled line names (a told death's killer, a best killer, whoever ran first). An untold `Fall` gets no line, but biographies, reports and the end screen can name the dead. Everyone else's death is counted. That bounds what's kept to the told deaths plus the few named people.

**Hunger is not a blow.** A starved captain, champion or legend gets a line. Other starvation deaths are summed once an hour per army: "Four of Garrick Hale's band starved near Ashby."

**Reading at 4x and 8x.** People read about four words a second. At 8x a three-minute clash lasts about 22 real seconds: room for five to seven short lines. The feed therefore filters told deaths by speed. This is display only: the sim, the reports, the end screen and headless output keep every told death.

| Speed | Told deaths shown in the feed |
| --- | --- |
| 1x–2x | Any where your people killed or died, plus every tier-A death of anyone |
| 4x | Tier A involving your faction or a legend, plus first blood in your battles, and absurd deaths of your people |
| 8x and fast-forward | Tier A involving your faction or a legend |

At every speed the feed shows at most one line every 1.5 real seconds per place, and folds the overflow into "…and 3 more fell at Loweby". The speed filter is a pure function in `sim_military::saga` (`feed_filter`), so `bot_run` can check the volume: over every seed, the deaths the 8x filter passes stay at **8 or fewer a battle**. The other saga lines (mishaps, weapons, kin and feuds) have their own, tighter limits (section 15).

## 3. Words: brutal by default, mild on request (UI-10)

**Where the words live.** A new `assets/data/saga.ron` holds words only, in two tones. The sim never reads it:
- It's not hashed and not saved, so editing a line changes no hash, replay or save, and an old save reads in the current build's words.
- Rules (what's notable, deeds, epithets, traits, legends, quirk effects, mishaps, bonds and moods) stay in hashed data (`progression.ron`, `villagers.ron`, `director.ron`). Ids join the files.

**Tone is your setting, not the world's.** Brutal or Mild is passed to the words layer when a line is read. It's never a command, a run setting, part of a save or part of a replay.
- Switching it rewrites the whole history at once: the feed, the chronicle, reports and biographies.
- The game remembers your choice in a small preferences file next to its saves. You set it on the new-game screen, or with a switch on the feed.
- Headless has `--mild`.
- Both tones speak in the same voice, deadpan dark (section 4). Mild drops the gore, not the straight face.

**How a line is built.**
- **Key.** A death picks the most specific key that fits, in this order: `champion`, `legend`, `captain`, `rebel`, `militia_victim`, `by_legend`, `blood`, `vengeance`, `absurd_*` (one key per irony, section 7), `mishap_*` (section 11), `first_blood`, `saved`, `last_stand`, `kin`, `streak`, `militia_killer`, `beast_kills`, `beast_slain`, `fleeing`, `one_blow`, `raider`, `armed` / `unarmed`.
  - A militiaman or rebel victim always takes its grave key (`militia_victim` or `rebel`), so a villager's death is never told with a soldier's brutal line, and never as a joke.
  - First blood and a streak still reach those lines through the `{moment}` slot, and an irony reaches the tier-A keys through the `{irony}` slot.
- **Variant.** Each key has at least six brutal and four mild variants.
- **Slots.** The variant's slots are filled from part banks:

| Slot | Fills with | Example |
| --- | --- | --- |
| `{killer}`, `{victim}` | The name. With an epithet, the given name and the epithet | "Wat Ashby", "Tobin Banner-Taker", "Kael Nine-Kills" |
| `{killer_given}`, `{victim_given}` | The given name, so lines never need a pronoun | "Tobin" |
| `{weapon}` | A named weapon's name, when the killer carries one (section 12). Otherwise, armed: a weapon word fixed for life per person, so Wat Ashby always swings the same axe. Unarmed: their old trade's tool, or a word from the unarmed bank | "Vane's Bane", "an axe", "a felling axe", "a flail", "a fence post", "bare hands" |
| `{wound}` | Where the blow went (flavour; brutal lines use it, mild lines usually don't) | "through Ulric Vane's jaw" |
| `{ground}` | The terrain where it happened | farmland "in the furrows", forest "among the roots", beach "on the shingle", road "on the road", a settlement "in the lane" |
| `{place}` | The nearest settlement | "near Loweby" |
| `{home}`, `{trade}` | The victim's village and old trade (one of the sim's occupations) | "of Ashby", "a baker" |
| `{quirk}` | A quirk's adjective, where a key is about it (section 10) | "a glutton", "a sleepwalker" |
| `{kin}`, `{kin_given}` | The household member a `vengeance` or kin line is about (section 13) | "Tobin Kettle", "Tobin" |
| `{beast}` | A beast noun, fixed per beast. Beasts carry human names today; the words never use them | "a great grey wolf", "the boar of the Barrow Stones" |
| `{foes}`, `{army}` | Faction nouns from saga.ron (this fixes "against Rival"), and an army by its captain | "the raiders", "the rival's band"; "Garrick Hale's band" |
| `{count}` | A number in words up to twenty | "Eleven" |
| `{daypart}` | The time of day, from the tick | "at dawn", "at noon", "after dark" |
| `{then}` | The time of day of the earlier fact a key names (joining, being named), from its record's tick | "at dawn" |
| `{moment}` | A short first-blood or streak phrase for lines whose key doesn't say it, in the key's register | "First blood at Loweby:" |
| `{irony}` | A short clause for a tier-A line whose death was absurd (section 7) | "after nine battles," |

**What each key must say.** Each key lists its **fact slots** (for example `{killer}` and `{victim}`; `{count}` and `{place}` for a summary) and its **flavour slots** (`{weapon}`, `{wound}`, `{ground}`, `{daypart}`, `{moment}`, `{irony}`). Every line, in both tones, names all of its key's fact slots and uses no slot outside the key's list. Mild lines may leave the flavour out.

**Variety without randomness.** Words never draw from `SimRng`:
- **Kill lines.** A told death picks its variant by its index in `Saga`'s told deaths: variant `(index × 37 + salt) mod bank length`, where every bank is shorter than 37. Two told deaths closer together in the record than a bank's length never share a variant of that bank.
- **Other parts** (mishap lines, weapon names, notes) use a stable hash of ids and the tick.
- **The result:** the same event reads the same in the feed, on the end screen, in headless and after a load.
- **Breadth from composition.** About 6 lines × 7 weapons × 8 wounds × 2–3 grounds gives roughly 700 to 1,000 armed variants per tone.
- **A brutal flourish is rationed.** In a battle report only the top one or two moments get one; the rest use terse report lines. Brutality hits harder when it's rarer.

**Content rules** (written into `docs/dev/DATA.md` with the banks, and checked by a lint test where a test can check them):
- **Toward soldiers, raiders and beasts:** graphic. Concrete verbs, one wound, the ground, about 14 words at most.
- **Fit the feed.** A feed row is 520 px at 12 pt, about 80 characters, and the line follows a "Day N: " prefix. So every line that can reach the feed fits **72 characters** when filled with the longest names, epithets, weapon names and places in the data. Report-only lines ("more died", deserters, moments) may run to 110.
- **Name first** in every line that can show at 8x (the `champion`, `legend`, `captain` and `by_legend` keys), so a glance gets who.
- **Toward militia, rebels and villagers:** grave, never gleeful and never funny, even in brutal. They die "in the lane, fork in hand". (Civilians can't be struck at all; only militia and rebels die by blows.)
- **Deadpan** (section 4): no exclamation marks, no question marks, no winking words; the lint has the list.
- **Never:** sexual violence, torture as spectacle, mutilating the dead for laughs, slurs, or children.
- **No gender.** There is no gender in the sim, so lines use names, given names, roles or "they". The lint denies the pronouns (he, she, his, her, him, hers) and gendered nouns (man, men, woman, women, son, daughter, wife, husband, brother, sister, mother, father, widow, lad, lass, boy, girl). Design prose may still say "men" for soldiers; lines may not. Family ties are "kin" or "of one household".
- **Mild is sober, not cute.** "Killed", "fell", "was cut down": a clerk's battle report, not a children's book.
- **Words may invent the manner, never the facts.** The wound, the weapon word, the ground, a mishap's flavour (the geese, the wedding, the dog) are flavour. Who, whom, where, when, first, last, how much and the outcome always come from records. Times come only from the `{daypart}` and `{then}` slots, never from a fixed phrase. Flavour may only decorate an effect the sim really had (section 11).

**Sample lines** (names are examples; trades are the sim's occupations):

| Key | Brutal (default) | Mild |
| --- | --- | --- |
| armed | Edric Moss put an axe through Ulric Vane's jaw in the barley. | Edric Moss killed Ulric Vane near Loweby. |
| armed | Wat Ashby opened Hild Crane from collarbone to belly in the mud. | Wat Ashby cut down Hild Crane. |
| armed, named weapon | Jory Lowe put Vane's Bane through a raider's ribs at Loweby. | Jory Lowe killed a raider with Vane's Bane. |
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
| legend, with `{irony}` | Kael Nine-Kills, after nine battles, fell to a baker's peel. | Kael Nine-Kills, after nine battles, was killed by a baker. |
| saved | Jory Lowe split Odo Venn's head a breath from Garrick Hale's back. | Jory Lowe killed Odo Venn, who was attacking Garrick Hale. |
| vengeance | Hild Kettle saw Tobin fall, found Osric Pike, and settled it. | Hild Kettle killed Osric Pike, who had killed Hild's kin. |
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
| named, mocking | They call Odo Rook Twice-Fled now. Not to Odo's face. | Odo Rook is called Twice-Fled now. |
| trait_earned | Perrin Dyer, Shaken at Loweby, stood at Ashby. Brave now. | Perrin Dyer held at Ashby and is Brave now. |
| legend_rose | Tobin Banner-Taker, fourteen dead behind, is a legend now. | Tobin Banner-Taker has become a legend of the warband. |

The absurd deaths, mishaps, weapons and kin have their own sample lines in sections 7 and 10 to 13. Hunger is told plainly in both tones: brutality is for blows. There are no execution lines, because there are no prisoners. A routed soldier struck down is a fleeing kill.

## 4. The voice: deadpan dark (UI-10)

Every line in the game, kill line or mishap, brutal or mild, speaks in one voice. **Deadpan dark** means:
- **The world is grim, and the narrator has stopped being surprised by it.** Battles, hunger, bad mutton and geese are all reported in the same flat register.
- **The fact, then the consequence, then stop.** Short declarative sentences. The second sentence often undercuts the first: "The band gave chase. The dog won."
- **The joke is in the facts, never in the narrator.** Absurd outcomes are funny because they are told as plainly as a casualty count. If a line needs an exclamation mark, an adjective like "hilarious" or a wink to be funny, it isn't deadpan, and it's cut.
- **Understatement over emphasis.** "Not to Odo's face" carries more than "the whole warband mocked Odo mercilessly".
- **The brutal and the absurd share a register.** "Wat Ashby put an axe through Ulric Vane's jaw." and "Perrin Dyer fled a goose." are said the same way, by the same clerk.
- **Gravity is kept where it's owed.** Villagers, militia and rebels are told grave, never as a joke (section 3). Absurdity is for soldiers, captains, raiders, beasts, mishaps, weapons and the world. The narrator never mocks a death in a defended village.
- **Mild keeps the deadpan and drops the gore:** a clerk who has seen everything, writes it down, and goes back to the ledger.

| Do | Don't | Why |
| --- | --- | --- |
| Perrin Dyer fled a goose at Ashby. The goose held the field. | Perrin Dyer was chased off by a terrifying goose! Honk! | No exclamation marks, no sound effects, no adjective doing the joke |
| Garrick Hale's band feasted after Loweby. Bram Tull choked. | Poor Bram Tull, tragically, choked at the victory feast. | No pity and no "tragically": state it |
| They call Odo Rook Twice-Fled now. Not to Odo's face. | Odo Rook, the biggest coward in the warband, LOL. | Understatement; the narrator never mocks |
| The mutton was bad. Four of the band went into the ditch and stayed there. | Food poisoning hits the band hard, and things get messy. | Concrete and physical, no innuendo |
| Nine battles, and Wat Nine-Kills died to a baker's peel at Loweby. | In an ironic twist of fate, Wat was defeated by a mere baker. | Never name the irony; let the facts do it |
| Cedric Tanner, a baker of Loweby, died in the lane, fork in hand. | Cedric Tanner died waving a fork, like a fool. | Villagers defending their homes are told grave |
| Vane's Bane was traded at Loweby for grain. It hangs in the storehouse. | The legendary Vane's Bane, sadly, ended up as small change. | Facts, then stop |
| (mild) Odo Venn fell in the press at Loweby and was crushed. | (mild) Odo Venn had a little accident. | Mild is sober, not coy |

**The lint** (`tests/tests/saga_words.rs`, section 3's lint plus these): no `!` and no `?` in any line; no word in all capitals; no ellipsis outside the feed's fold line; and a deny-list of winking and pitying words in both tones ("lol", "haha", "oops", "whoops", "funny", "hilarious", "ironic", "irony", "tragic", "tragically", "sadly", "unfortunately", "poor", "epic", "honk", "splat"). The playtest asks whether the odd lines read funny-grim or merely silly.

## 5. Battle reports with moments

Each army's battle report (`BattleReport`, UI-09) gains its moments:
- **The battle.** Its number and name, so both sides' reports of one fight are recognisably one fight.
- **First blood:** the battle's first told death, and whether this army drew it. It's stored once on the battle, so both reports agree.
- **Who ran first, and when the army broke.** The report already has the turning point, the tick its first man ran; now it names who ran, and adds the tick its army broke (section 2), if it did.
- **The told deaths** this army suffered or dealt, in order (read from `Saga` by battle number).
- **The best killer:** most kills in the fight, ties to the lower SpawnId. The honour stands even if the best killer died later in the battle.
- **The wounded, and the deserters** during the battle. In big battles desertion dwarfs death.
- **What its people earned:** veteran ranks (as today), epithets, earned traits and legends risen.
- **The odd:** the battle's absurd deaths, a mishap during it (a slip in the shield wall), weapons named in it, and kin avenged (read from `Saga` by battle number).
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
> Fell: Brenna Hale, on a raider's spear. Slain: raider captain Oswin Thorne, by Jory Lowe, whose axe is called Thorne's Bane now.
> Best killer: Kael Dyer, four. Eleven more of yours died in the furrows, and nineteen of theirs. Six threw down their spears and ran.
> Odd: Odo Venn slipped in the shield wall and was trodden flat by friends.
> Earned: Jory Lowe is called Banner-Taker; Odo Rook is Brave; three rose to Veteran.

**The same report (mild):** "The Battle of Loweby Fields, day 4: your warband against the raiders. First blood: Wendel Orme killed Aric Brook. Perrin Rook fled first; the raiders broke at noon. Fell: Brenna Hale. Slain: captain Oswin Thorne, by Jory Lowe, whose axe is called Thorne's Bane now. Best: Kael Dyer, four kills. Eleven more of yours died, and nineteen raiders. Six deserted. Odo Venn fell in the press and was crushed. Jory Lowe is called Banner-Taker; Odo Rook is Brave; three became Veterans."

## 6. Service records, deeds and epithets (SIM-53)

**A service record** (`Service`) starts the first time a person has something to record: a kill, a wound, a deed, a mishap or a mark toward a trait. Beasts never get one. It holds:

| Field | What | Bound |
| --- | --- | --- |
| `trade`, `home` | Their old trade (captured when they enlist, before enlisting makes them idle) and home settlement, if any | — |
| `joined`, `rejoined` | The tick they first enlisted, and how many times they came back after deserting | — |
| `kills`, `unarmed`, `fleeing` | Kills, and how many were bare-handed or of fleeing foes | — |
| `streak` | Kills in the current battle, with its number | — |
| `battles`, `held`, `fled`, `wounds` | Battles survived; battles held while their army broke or lost a quarter of its fighters; battles in which they ran; battles survived after losing at least a quarter of their health to blows | — |
| `mishaps` | How many mishaps of each kind befell them (section 11) | One byte per kind |
| `fights` | The latest battles they fought, as (battle number, kills) | 16 |
| `deeds` | Each deed once, as (deed id, tick, battle number or nearest settlement, victim `NameId` if any) | One per kind of deed |
| `marks` | Four counters toward the earned traits (section 8) | — |
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
| Avenged kin | Slew the slayer of their kin in the same battle (section 13) | 3 |
| Rescuer | One of a rescue's heroes: fought in the battle that drove raiders off a village whose drive-off request was then answered (section 14) | 3 |

**Epithets: earned by rule, one per person, explained.**
- **Rules** are rows in `progression.ron` (`saga.epithets`). Each row has a stable `id`, which saves and run records store, and a separate `rank`. A higher rank outranks a lower one, so ranks can be re-tuned and new epithets slotted in anywhere without relabelling old saves or records. Ids are append-only.
- **Honourable and mocking.** Each row is `honour` or `mock`. Mocking epithets (section 7) take the lowest ranks, so a deed can wash one off ("Twice-Fled became Banner-Taker at Kemp's Ford") and none ever replaces an honourable one.
- **One per person.** A person carries one epithet, replaced only by a higher-ranked one ("Nine-Kills became Banner-Taker at Kemp's Ford").
- **Words.** Each epithet's words, in both tones, are in saga.ron, keyed by id. Where there are several variants, one is chosen per person for life.
- **Every epithet says why** in a tooltip: "Nine-Kills: nine kills, at Loweby and Crane's Ford".
- **Who.** Epithets are run-only and come to every faction except beasts.

The honourable epithets (the mocking ones, ids 19 to 26 at ranks 0 to 7, are in section 7):

| Id | Rank | Rule | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 0 | 8 | 8 battles | Old Iron | Old Iron |
| 1 | 9 | 4 kills bare-handed | Bare-Hands / Knuckles | the Wrestler |
| 2 | 10 | 2 wounds | Half-Face / the Stitched / Old Scars | the Scarred |
| 3 | 11 | Doorwarden | Doorwarden | Doorwarden |
| 4 | 12 | Ran first, and 5 kills | the Hare | the Runner |
| 5 | 13 | 9 kills | Nine-Kills | Nine-Kills |
| 6 | 14 | Beast-slayer | Wolfsbane / Boar-Sticker / Beast-Bane | Beast-Bane |
| 7 | 15 | Shield | Shieldfast / the Shield | the Shield |
| 8 | 16 | Held 3 times (`held` ≥ 3) | the Unbroken / the Wall | the Unbroken |
| 9 | 17 | Last stand | Last-Standing / the Stubborn | the Steadfast |
| 10 | 18 | 20 kills | the Butcher / Red-Hands / the Reaper | the Grim |
| 11 | 19 | Cruel, and 6 kills of fleeing foes | the Hound | the Hunter |
| 12 | 20 | Beloved, and Breadgiver | Breadgiver | Breadgiver |
| 13 | 21 | Redeemed | the Redeemed | the Redeemed |
| 14 | 22 | Banner-taker | Banner-Taker / Captainsbane | Banner-Taker |
| 15 | 23 | Avenger | Blood-Debt / the Avenger | the Avenger |
| 16 | 24 | Kinslayer | Kinslayer | Kinslayer |
| 17 | 25 | Legend-breaker | Legend-Breaker / Giantkiller | Legend-Breaker |
| 18 | 26 | Champion-slayer | the Headsman / Champion's Bane | Champion's Bane |

Every epithet is gender-neutral. "Spearwife" from the first pitch is dropped (open question 3).

## 7. Unflattering epithets and absurd deaths (SIM-53, SIM-39)

Dwarf Fortress's best names are the ones nobody wanted. A few epithets are mocking, and some deaths are absurd. Both come only from records: a mocking name needs the deeds (or the lack of them) to earn it, and an absurd death is a real death whose facts happen to be ridiculous.

**What you'll read:**
- "They call Odo Rook Twice-Fled now. Not to Odo's face." *(mild: "Odo Rook is called Twice-Fled now.")*
- "Hollis Fenwick is Turnip-Hand now: four battles, still no kills." *(mild: "Hollis Fenwick is called Turnip-Hand: four battles, no kills.")*
- "Twice-Fled became Banner-Taker at Kemp's Ford. Nobody uses the old name." *(mild: "Odo Rook, once Twice-Fled, is called Banner-Taker now.")*
- "Nine battles, and Wat Nine-Kills died to a baker's peel at Loweby."
- "Aric Brook joined the band at dawn and was dead by noon."
- "Hild Crane had never killed anything. Then Hild killed Kael Nine-Kills."
- "Garrick Hale's band feasted after Loweby. Bram Tull choked."
- "Hild Crane, a glutton, was the first of the band to starve."

**Mocking epithets.** Appended to `saga.epithets` as ids 19 to 26, with `kind: mock`, at ranks 0 to 7, below every honourable epithet (whose ranks move up by 8; ids don't change). They stick until an honourable epithet replaces them. A Vain soldier who earns one takes it badly (a `mocked` mood, section 13).

| Id | Rank | Rule (from the record) | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 19 | 0 | Ran in two battles (`fled` ≥ 2) | Twice-Fled | Twice-Fled |
| 20 | 1 | A farmer once, 4 battles and no kills | Turnip-Hand / Plough-Arm | the Farmer |
| 21 | 2 | Deserted and came back twice (`rejoined` ≥ 2) | Come-Back / the Homesick | the Returner |
| 22 | 3 | Lost at dice three times (section 11) | Empty-Purse / Dice-Fodder | Empty-Purse |
| 23 | 4 | A glutton, and two food mishaps (bad mutton, a feast) | Bread-Belly | the Hungry |
| 24 | 5 | Fled from geese twice (section 11) | Goose-Shy | Goose-Shy |
| 25 | 6 | Three mishaps of any kind | the Unlucky | the Unlucky |
| 26 | 7 | The only one of their army to survive a battle that killed at least five of it | Last-Out / the Leftover | the Survivor |

**Absurd deaths.** When a death is classified (in `casualties`' second pass, section 17), the first **irony** in the tables below that fits is stored on its `Fall`, scores +50 (section 2), picks its `absurd_*` key (a mishap's death keeps its `mishap_*` key), and reaches tier-A lines through `{irony}`. Ironies never apply to militia, rebel or villager victims: their deaths stay grave. Ids are append-only.

*Real today: the sim already produces these, and M4c only notices them.*

| Id | Irony | Rule | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 0 | Felled by a tool | The killer is militia, unarmed (their old trade's tool); the victim had 4+ battles or an epithet | Nine battles, and Wat Nine-Kills died to a baker's peel at Loweby. | Wat Nine-Kills, after nine battles, was killed by a baker of Loweby. |
| 1 | First day | The victim joined less than a game day before and had fought no battle | Aric Brook joined the band at dawn and was dead by noon. | Aric Brook was killed on the day of joining. |
| 2 | A first kill | The killer had never killed; the victim is a legend or has an epithet | Hild Crane had never killed anything. Then Hild killed Kael Nine-Kills. | Kael Nine-Kills was Hild Crane's first kill. |
| 3 | Home lane | The victim enlisted from this settlement; its militia killed them | Wat Ashby came home to Ashby with the warband. Ashby's militia saw to it. | Wat Ashby was killed by the militia of Ashby, Wat's own village. |
| 4 | Survived the wars | A beast killed someone of 5+ battles | Six battles, and then the boar of the Barrow Stones. | Merrick Ellis survived six battles and was killed by a boar. |
| 5 | Named that day | Died within a game day of earning their epithet | They called Odo Rook the Wall at noon. The wall came down after dark. | Odo Rook was called the Wall for less than a day. |
| 6 | Glutton, starved first | The first of their army to starve (`Captain::starved` was 0), and a Glutton (section 10) | Hild Crane, a glutton, was the first of the band to starve. | Hild Crane, a glutton, starved first. |
| 7 | Own weapon | Killed with a named weapon they once carried (section 12) | Wat Ashby met Vane's Bane again, from the wrong end. | Wat Ashby was killed with Vane's Bane, Wat's old axe. |

*New in M4c, on purpose: the two mishaps that can kill (section 11).*

| Id | Irony | Rule | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 8 | Trodden by friends | A slip in the shield wall finished them | Odo Venn slipped in the shield wall at Loweby and was trodden flat by friends. | Odo Venn fell in the press at Loweby and was crushed. |
| 9 | Choked at the feast | The victory feast's choke | Garrick Hale's band feasted after Loweby. Bram Tull choked. | Bram Tull choked at the feast after Loweby. |

*Not in M4c: the sim can't produce these, so no line may claim them.*

| End | Why not | When |
| --- | --- | --- |
| Drowned fording | The island has no rivers or fords, and water is impassable to every walker | A terrain change: rivers in worldgen, or fords with M4b's seasons and weather |
| Trampled by sheep | There are no animals; a pasture is a workplace | M4b's animals |
| Struck by lightning, frozen | No weather or cold | M4b (SIM-05, SIM-06) |
| Fell from the walls | No walls or sieges | M4b's walls, M5's sieges (GP-15) |
| A festering wound | Wounds never heal or fester | M4b's healing |

**Data:** the epithet rows (`kind`, rank, rule) and the irony rules' numbers (battles, days) are in `progression.ron` `saga.epithets` and `saga.ironies`; the words in saga.ron.
**Determinism:** rules over records, no RNG.
**Hashing and saving:** the irony id is a field of `Fall` (in `Saga`'s digest); `fled`, `rejoined` and `mishaps` are fields of `Service`.
**Performance:** the irony checks run per classified death, only for deaths with a killer or a mishap, reading the records the classification already holds.
**UI:** the epithet tooltip says why, as for any epithet ("Turnip-Hand: a farmer once; four battles, no kills"); absurd deaths are shown at 4x for your people (section 2) and listed on the end screen's "Oddities of the run".
**Tests:** `mocking_epithets_follow_rank` (Twice-Fled is replaced by Banner-Taker and never the reverse; each mocking rule from a scripted record); `absurd_deaths_are_real` (each irony from a scripted state, none for a militia victim, and the lint finds no `absurd_*` key whose irony the tables lack).
**Built:** ironies 0 to 6 and 8 to 9 with the epithets (plan step 8); irony 7 with named weapons (step 11).

## 8. Earned traits (SIM-34)

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
- **Quirks are not traits.** Quirks are dealt (who someone is, section 10); traits are earned (what they became). Both act through the holder, and both feed the same cached nudges on `Veteran` (section 17), whose sums are clamped (break point ±0.06, morale target ±0.10, desertion ×0.5 to ×2).

**Picked and earned traits are separate.**
- The champion's picked traits stay in `Traits` and act faction-wide through `FactionModifiers`, as built. Earned traits live only in `Service` and act on the holder.
- **Offers** (`draw_traits`) ignore earned traits.
- **Succession** moves half the picked traits, as today, and never moves earned ones: they belong to the person.
- **One trait both ways.** An earned trait the champion already picked isn't awarded again, because the faction-wide effect already covers the champion. An earned trait opposite a picked one is allowed: the picked one still acts on the faction, and the earned one on the champion alone. The inspector shows both: "Brave (chosen) · Shaken (earned at Crane's Ford)".
- **A new event kind.** Earning a trait is the new `TraitEarned`, not today's `TraitGained`. `TraitGained` is chronicled and notable: it resets the director's quiet clock and stops fast-forward, which a rare pick at a rank-up should do and dozens of earned traits per run should not. A separate kind keeps `notable()` a plain rule over kinds.

## 9. Legends (SIM-54)

**Who becomes a legend:**
- **Threshold.** Fame 25, at least three battles survived, and an epithet (a mocking one counts: "Odo Twice-Fled, legend of the warband" is a real possibility, if rare).
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
- **Memorials.** A cairn where the legend fell, and a stone in their home village: render-only in M4c (section 14).

**Grudges, a light nemesis (open question 1):**
- **Named.** The enemy who killed a legend is named in later lines and on the end screen: "Osric Pike, who slew Brenna the Stubborn".
- **Avenged.** If one of the legend's faction later kills that enemy, the death is always told and the killer earns the Avenger deed.
- **No more than that.** No AI hunts anyone, and nobody is promoted for it. FUN_DESIGN's "nemesis rivals" otherwise stay not adopted. (Feuds between two soldiers of one army are a different thing: section 13.)

## 10. Quirks (SIM-55)

In a run, every villager and soldier has one or two **quirks**: who they are, before anything happens to them. Each has a tiny, real effect on what its holder does, and the director's mishaps look for them (section 11). Quirks make two villages of the same size behave a little differently, and give every recruit something to be remembered by.

**What you'll read:**
- Inspector: "Hild Crane · a farmer of Ashby · Glutton, Fears geese"; its tooltip: "Glutton: gets hungry 15% faster".
- "Perrin Dyer, who fears geese, fled one at Ashby. The goose held the field." *(mild: "Perrin Dyer ran from a goose at Ashby.")*
- "Jory Lowe, a sleepwalker, woke up in the barley, alone." *(mild: "Jory Lowe sleepwalked out of camp.")*
- "The band plundered Ashby. Tobin Kettle, pious, took it badly." *(mild: "Tobin Kettle, pious, was troubled by the plunder of Ashby.")*
- "Linnet Ware left the band near Ashby. Someone in Ashby was waiting." *(mild: "Linnet Ware left the band to stay in Ashby.")*
- Biography: "A coward, by all accounts. Ran first at Loweby and at Kemp's Ford, and outlived both."
- "Hild Crane, a glutton, was the first of the band to starve." (an absurd death, section 7)

**The deck** (`villagers.ron` `quirks.deck`; ids append-only). The effects act only through the holder, run-only, and are exact no-ops for anyone without the quirk.

| Id | Quirk | Weight | As a villager | As a soldier | Mishaps it invites (section 11) |
| --- | --- | --- | --- | --- | --- |
| 0 | Coward | 8 | Militia courage −0.15; flees from 1.25× as far | Breaks at +0.04 (0.29 instead of 0.25) | A slip in the shield wall (×2) |
| 1 | Stout | 8 | Militia courage +0.10 | Breaks at −0.02 | — |
| 2 | Glutton | 10 | Gets hungry 1.15× as fast | Gets hungry 1.15× as fast; morale target +0.02 while the baggage holds 3 days or more | Bad mutton (×3), the feast's choke (×4) |
| 3 | Drunkard | 10 | Work score ×0.9 in the morning | Morale target +0.02 | Drunk before battle (drunkards first) |
| 4 | Pious | 8 | Loyalty drifts ×0.8 as fast (slow to change sides; faith itself comes with M4b, SIM-50) | Desertion ×0.85; a `pious` mood, −0.03 for 12 hours, when their army plunders | — |
| 5 | Vain | 6 | — | Morale target +0.03 while holding an honourable epithet or a named weapon; an `envy` mood, −0.02 for a day, when a comrade in their army earns an epithet; a `mocked` mood, −0.04 for two days, on earning a mocking one | — |
| 6 | Sleepwalker | 6 | Rest gained asleep ×0.85 | — | Wandered off |
| 7 | Gambler | 8 | Idle score ×1.1 in the evening | — | Dice |
| 8 | Fears geese | 4 | Work score ×0.9 at farms and pastures | A `geese` mood, −0.03, for each hour they end within 3 tiles of a farm or pasture | Goose rout |
| 9 | Sweetheart | 6 | Joins an army ×0.8 as willingly | Out of battle and within 12 tiles of home: 5% a day to walk home (a desertion) | — |
| 10 | Early riser | 8 | Work score ×1.1 in the morning | — | — |

- **Deal.** One quirk, or two with `two_chance` (0.4), drawn by weight without replacement, honouring each row's `excludes` (Coward and Stout exclude each other; a pious drunkard is allowed). Weights sum to 82.
- **Who and when.** In a run only:
  - every person alive at the end of `setup_run`, after every other setup spawn (the band, guards, the rival, the sellswords), gets their deal;
  - everyone `spawn_army` spawns later in the run (raiders, rival levies, drifters' bands) gets theirs at spawn;
  - beasts never get quirks.
  
  The developed world, the skirmish and `--battle` deal none.
- **Their own stream.** Each person's deal comes from `SimSeed::stream(rng_key("sim_agents::quirks") ^ SpawnId, 0)`. So a deal depends only on the seed, the deck and the person's SpawnId: never on when or in what order people are dealt, and never on `populate`'s draws, which stay untouched (`docs/m4/BUILD_GUIDE.md` G10). Adding people never shifts anyone else's quirks.
- **The geese are words.** Fears geese reads a real place: a farm or pasture workplace within 3 tiles. The geese themselves aren't simulated (animals come in M4b). The rout, the lost morale and the farm are real. Hates rain is left out until there is rain (M4b, SIM-06).
- **Sweetheart is the love in "desertion for love".** The sim has no courtship. A sweetheart's pull is a real desertion near their real home settlement; "someone in Ashby was waiting" is the words. Spawned soldiers have no home, so the quirk does nothing for them, and the deck skips it when dealing to them.
- **Through the holder, and small.** No quirk touches another person or the faction, or goes into `FactionModifiers`. Soldier effects fold into `Veteran`'s cached nudges (section 17), clamped with traits and moods.

**Data:** `villagers.ron` gains `quirks: (two_chance, deck: [(id, key, weight, excludes, effects)])` (`#[serde(default)]`; an empty deck deals nothing). The effects are an append-only enum: `Hunger(mult)`, `RestGain(mult)`, `Work(phase, mult)`, `FarmWork(mult)`, `Idle(phase, mult)`, `Courage(add)`, `FleeRadius(mult)`, `LoyaltyDrift(mult)`, `Willing(mult)`, `BreakPoint(add)`, `MoraleTarget(add)`, `FedMorale(days, add)`, `Desertion(mult)`, `PlunderMood(add, hours)`, `NearFarms(tiles, add)`, `HomePull(tiles, per_day)`, `Vanity(epithet, envy, mocked)`, `Invites(mishap, weight)`. Their names, adjectives and tooltips, in both tones, are words in saga.ron. Editing villagers.ron re-runs `village_day` and `famine` (CLAUDE.md); with no quirks outside a run, they must not move.
**Determinism:** the deal stream above; every effect is a rule. Nothing else draws, except the sweetheart's hourly walk home (`sim_military::saga::homesick ^ SpawnId` at the tick).
**Hashing and saving:** `Quirks` (sim_agents, up to two ids per person, two bytes) is hashed and saved, and `Villagers` hashes its data text as before.
**Performance** (4,000 to 5,000 villagers): two bytes a person. `ai::decide`, `update_needs` and the daily `drift_loyalty` read one more optional component and skip all quirk work when it's absent; a recruit's willingness is read once per candidate in a recruit dealing, on the same draw as today. Soldiers read nothing new in the hot loops: their quirk effects are in the cached nudges. Fears geese and Sweetheart are checked hourly in `saga::settle` for their holders only (about one in eight people), against the settlement list.
**UI:** the inspector's quirk line with tooltips; the villager `DecisionTrace` shows a quirk as its own factor ("drunkard (morning) 0.9"), so `Factors` grows from four slots to five; biographies name quirks; the end screen's oddities count them.
**Tests:** `quirks_are_dealt_per_person` (the same seed deals the same quirks; dealing in any order or at spawn gives the same result; one or two each; exclusions hold; the developed world and the skirmish have none); `quirks_act_through_their_holder` (each row's effect, measured on a hand-built world, and a person without quirks bit-identical to today); `sweetheart_walks_home` (near home and out of battle at the documented rate; never far from home or in battle).

## 11. Mishaps (GP-30)

The pacing director (GP-16) already brings raiders when it's too quiet. In M4c it also schedules **mishaps**: small things going wrong, chosen from what is true right now. A mishap is a low-tension beat: it gives a calm stretch a story without making it dangerous. Mishaps are low stakes and rate-limited, and they never end a run.

**What you'll read** (brutal; mild in italics):
- "Six of Garrick Hale's band were drunk at noon. Raiders were near." *(Six of Garrick Hale's band were drunk before the fight.)*
- "The mutton was bad. Four of the band went into the ditch and stayed there." *(Bad mutton made four of Garrick Hale's band sick.)*
- "A dog from Loweby took a side of mutton. The band gave chase. The dog won." *(A dog stole some of the band's food at Loweby.)*
- "Ashby held a wedding. Garrick Hale's band stayed for the bread. Two hours lost." *(Garrick Hale's band stopped two hours for a wedding at Ashby.)*
- "Odo Venn slipped in the shield wall and got up with fewer teeth." *(Odo Venn fell in the press at Loweby and was hurt.)*
- "Odo Venn slipped in the shield wall at Loweby and was trodden flat by friends." *(Odo Venn fell in the press at Loweby and was crushed.)*
- "Garrick Hale's band feasted after Loweby. Bram Tull choked." *(Bram Tull choked at the feast after Loweby.)*
- "Jory Lowe, a sleepwalker, woke up in the barley, alone." *(Jory Lowe sleepwalked out of camp.)*
- "Wat Ashby won Odo Rook's supper at dice. Odo has not forgotten." *(Wat Ashby won Odo Rook's supper at dice.)*
- "Perrin Dyer fled a goose at Ashby. The goose held the field." *(Perrin Dyer ran from a goose at Ashby.)*
- "Odo Rook and Perrin Dyer fought behind the baggage. Both lost." *(Odo Rook and Perrin Dyer came to blows in camp.)*

**The kinds.** Each needs real state to happen and has a real effect; the words only decorate it. Kind ids are append-only.

| Kind | When it can happen | What it does | Can it kill |
| --- | --- | --- | --- |
| Drunk before battle | An army within `threat.radius` of a hostile army, not yet in battle, within 6 tiles of a settlement | 1 to 6 members (drunkards first): morale +0.10 now, then a `hungover` mood, −0.03 for 12 hours | No |
| Bad mutton | The baggage holds mutton | A quarter of the mutton spoils; 1 to 4 members (gluttons ×3) get a `sick` mood, −0.05 for 12 hours | No |
| A dog at the rations | Within 8 tiles of a settlement, with food in the baggage | Food worth a quarter of a day's need is lost (the words name the good) | No |
| A wedding on the road | A march (`MoveTo` or `Return`) within 6 tiles of a settlement whose goodwill toward the army's faction is 0 or more, with no hostile army within `threat.radius` | The march halts 2 hours (`Captain::halt_until`); a `wedding` mood, +0.03 for 12 hours, for every member | No |
| A slip in the shield wall | An army in battle (a blow this hour) with 10 or more members fighting | One fighting member (cowards ×2) loses 20% of their health; if that ends it, they die | Yes |
| The victory feast | A battle the army didn't lose ended this hour, and the baggage holds 3 days or more | Half a day's food is eaten; a `feasted` mood, +0.04 for 12 hours, for every member; one chance in 20 that one member (gluttons ×4) chokes | Yes |
| Wandered off | Night, an army holding (no goal), with a sleepwalker in it | The sleepwalker wakes 4 to 8 tiles away on passable ground and rejoins by the rally field | No |
| Dice | An army holding, with a gambler and another member | Even odds: the loser's hunger +0.15 and the winner's −0.15 (a supper changes hands), with a `dice` mood ±0.02 for 12 hours; one chance in 3 that the loser takes a grudge (section 13) | No |
| Goose rout | A member who fears geese within 3 tiles of a farm or pasture, marching, with no foe within `engage_radius` | They run 4 tiles away from the farm and take a `geese` mood, −0.05 for 6 hours | No |
| A camp brawl (with section 13) | Two rivals in one army, holding | Both lose 5% of their health (never below 1); one chance in 3 that the feud ends | No |

**When.** In `director::step` (slot 12 of the progress driver, hourly, run only), after the raider check, the **mishap clock**:
1. Nothing in the run's first 24 hours (`free_hours`), so a start stays as it was drawn; nothing while a run end is pending.
2. Once `spacing_hours` have passed since the last mishap and today's count is under `max_per_day`, each hour has a `chance_per_hour` chance of one.
3. Most kinds need tension below `tension_below` (a calm hour). The two battle kinds (drunk before battle and a slip in the shield wall) ignore tension instead, and are capped at one per battle.
4. The candidates are every (kind, army) whose condition holds now, weighted by the kind's weight and by the quirk holders in that army. The player's armies weigh 2 and armies at war with the player weigh 1, so about a third of mishaps fall on raiders and the rival. Beasts, monument guards and bands for hire are never subjects.
5. One candidate is drawn; its subjects and rolls are drawn; it applies; the clock resets.

| Personality | `spacing_hours` | `chance_per_hour` | `max_per_day` | `tension_below` | About |
| --- | --- | --- | --- | --- | --- |
| Steady | 16 | 0.3 | 2 | 0.5 | One a game day, 25 to 35 a run |
| Chaotic | 10 | 0.3 | 3 | 1.0 | One and a half a day, bunched |
| Harsh | 20 | 0.3 | 1 | 0.25 | One a day at most |

**Never run-ending.** These are rules in code, each with a test:
- **Never the important people.** The champion, any captain and any legend are never a mishap's subject. Their bands may be.
- **Deaths.** Only the slip and the feast can kill: one ordinary soldier at most, only in an army of 10 or more (`min_army`), and at most one mishap death a game day across the island (`max_deaths_per_day`).
- **Food.** A food mishap never takes the baggage below one day of the army's need (`food_floor_days`), and never touches a settlement's stores.
- **Delays.** Only marches, only with no hostile army near, at most 2 hours, and at most once a day per army: a wedding never makes you late for a fight.
- **No strategic change.** A mishap never changes an owner, goodwill, loyalty, an order, a posture or an allegiance; never starts a battle; never spawns anything.
- **One a day per army.** An army that had a mishap today is not a candidate again until tomorrow.

**Pacing.** A mishap is the new `SimEventKind::Mishap` (`subject` the army's captain, `other` the person or 0, `at` the tile, `amount` the kind). It's chronicled, for biographies and the end screen, and **not notable**: it doesn't reset the quiet clock, count as the run's first event, stop fast-forward or pause the game. So raids keep coming when they would have, and mishaps only fill the quiet. A mishap's death is a told `Fall` (tier A, irony 8 or 9), found by `find_fallen` on the next tick like a soldier who starved (G4).

**Data:** `director.ron` gains `mishaps: (spacing_hours, chance_per_hour, max_per_day, tension_below)` in each personality, and top-level `mishap_rules: (free_hours, enemy_weight, min_army, max_deaths_per_day, food_floor_days)` and `mishap_kinds: [...]` with each kind's weight and numbers (the table above). All are hashed, as the director's RON text. The lines are words in saga.ron, a lethal and a non-lethal bank where a kind can kill.
**Determinism:** `sim_military::director::mishap` at the hour's tick draws whether a mishap comes, its kind and its army; `sim_military::mishap ^ n` (n = the run's mishap count) at the same tick draws its subjects, counts and rolls (the choke, the dice, the grudge, the wandering tile). Both are new keys; no existing stream moves.
**Hashing and saving:** `Director` gains `next_mishap`, the day's count and the day, `mishaps` (the run's count) and `deaths_day`; `Captain` gains `halt_until`; moods are `Mood` entries (section 13). All hashed and saved.
**Performance:** hourly, a pass over the armies (tens) and one army's members; per tick, only `run_orders` checking `halt_until` per captain, and the cached nudges.
**UI:** feed lines, dimmer than battle lines. At 1x–4x, all of yours and those of armies near yours; at 8x and fast-forward, only a mishap that killed one of yours or befell your champion's army; at every speed, at most one mishap line every 20 real seconds. The inspector shows the moods a mishap left ("sick: bad mutton, 9 hours"); the end screen's oddities count them.
**Tests:** `each_mishap_does_what_it_says` (each kind on a hand-built world, with its condition false and then true); `mishaps_never_wipe_a_run` (over every `bot_run` seed: no mishap touched a champion, captain or legend, at most one death per mishap and one a day, no baggage under the floor, no delay with a foe near, at most `max_per_day`, none in the first 24 hours); `mishaps_are_not_notable` (the quiet clock, the first event and the raider schedule are unchanged by them).

## 12. Named weapons (SIM-56)

A weapon that kills someone who matters gets a name, and from then on a history: whose hands it was in, what it killed, and where it went when they died.

**What you'll read:**
- "Jory Lowe's axe went through captain Rook Vane at Kemp's Ford. The band calls it Vane's Bane now." *(mild: "Jory Lowe's axe is called Vane's Bane since Kemp's Ford.")*
- "Vane's Bane passed to Hild Lowe, of Jory's household." *(mild: same)*
- "Vane's Bane went into the baggage. Nobody wanted it yet."
- "A raider took Vane's Bane off Jory Lowe's body." *(mild: "A raider took Vane's Bane from the field.")*
- "Vane's Bane was traded at Loweby for grain. It hangs in the storehouse."
- "Vane's Bane lies in the furrows near Loweby. Nobody went back for it." *(mild: "Vane's Bane was lost near Loweby.")*
- "Jory Lowe deserted, and took Vane's Bane home to Ashby."
- History, in the inspector: "Vane's Bane, an axe. Named at Kemp's Ford for captain Rook Vane. Seven kills in three hands. Traded at Loweby on day 19 for grain."

**Identity, kept small.** `Equipment.weapon` stays a bool, and an ordinary weapon stays anonymous and vanishes with its holder, as today. A named weapon is:
- a record in `Saga.weapons`: the weapon word of its first namer, the naming (tick, battle, victim `NameId`), its holders (up to 8, each with how they got it), its kills while named, its notable kills (up to 8), and its **place**: held by a person, in a captain's baggage, in a settlement's storehouse, taken home by a deserter, or lost at a tile;
- a sparse `NamedWeapon(index)` on whoever holds it, which always has `Equipment.weapon` true.

A named weapon has **no stats**: it hits like any weapon (`army.ron` `weapon_attack`). Its only effects are where it goes: a looter is armed by it, and a baggage or storehouse holds one more `weapons` unit.

**Naming.** In `casualties`' first pass, when an armed killer kills a captain, the champion, a legend or a monument's beast, and the killer's weapon has no name yet, it's named. At most 2 a battle and 24 a run (`saga.weapons`). The name is words, from patterns in saga.ron ("{victim_family}'s Bane", "the {place} Cleaver", "Old {adjective}"), chosen by a stable hash of the weapon's index, so it's the same in both tones and every time.

**Where it goes**, decided in `casualties`' second pass when its holder dies (victim SpawnId order), the first that applies:
1. **Looted.** The killer is of another faction, alive and unarmed: a soldier takes it in hand (`Equipment.weapon` becomes true); a militiaman's village takes it, as one `weapons` unit in its storehouse ("it hangs in Loweby's storehouse"). Beasts never loot.
2. **Passed on.** Else, to an unarmed, living member of the holder's household in the same army; else to the nearest unarmed, living member of that army within 3 tiles (ties to the lower SpawnId).
3. **Into the baggage.** Else, if the army still has a captain (or an heir), as one `weapons` unit in the baggage.
4. **Lost.** Else it lies where its holder fell, for good in M4c.

And while it's not in a hand:
- **A deserter takes it home.** On re-enlisting, they bring it back as their weapon.
- **Arming.** `arm_followers` hands named weapons out first, oldest name first, to the first unarmed follower in SpawnId order, and an `Equip` dealing at a storehouse holding one hands it out first.
- **Barter sells it last.** A barter spends named weapons only after every anonymous one. If one goes, it's **sold**, and its place becomes that settlement's storehouse.
- **Reconciled hourly.** `saga::settle` checks that every place holds at least as many `weapons` units as the named weapons placed there. If some other path took them, the newest named one there is marked lost, so a name never sits on a unit that's gone.

**Data:** `progression.ron` `saga.weapons: (max_per_run: 24, max_per_battle: 2, pass_radius: 3.0)`; names and fates are words in saga.ron.
**Determinism:** rules only, in the serial casualty passes and dealings; no RNG.
**Hashing and saving:** `NamedWeapon` is hashed and saved (its index is stable, since `Saga.weapons` is append-only); the records are in `Saga`'s digest.
**Performance:** one check on a tier-A kill by an armed killer; one lookup when a holder dies; nothing per blow beyond the existing kill push.
**UI:** the inspector's weapon line ("Carries Vane's Bane, an axe; named at Kemp's Ford; seven kills"), clicking it shows the history; the settlement panel lists relics in its storehouse; kill lines name it in `{weapon}`; the end screen names the run's most storied weapon and its fate. The feed tells namings of your weapons, and losses and sales of them, at 1x–4x; at 8x, only a legend's weapon.
**Tests:** `a_weapon_is_named_by_a_notable_kill` (and not by an ordinary one, nor past the caps); `named_weapons_change_hands` (looted by a soldier and by militia, passed to kin, into the baggage and out first, sold last, taken home and back, lost when the army is wiped, and always in exactly one place).

## 13. Kin, friends, rivals and moods (SIM-57)

Soldiers aren't strangers to each other. Some came from the same household; some held a line side by side; some can't stand each other. These bonds, and what happens to them, give soldiers moods: small, bounded nudges with causes the inspector can name.

**What you'll read:**
- "Hild Kettle saw Tobin fall, found Osric Pike, and settled it." *(mild: "Hild Kettle killed Osric Pike, who had killed Hild's kin.")*
- "Jory Lowe and Bram Tull held together at Kemp's Ford. Shield-friends since." *(mild: "Jory Lowe and Bram Tull are friends since Kemp's Ford.")*
- "Odo Rook has not forgiven Perrin Dyer for running at Loweby." *(mild: "Odo Rook blames Perrin Dyer for running at Loweby.")*
- "Odo Rook and Perrin Dyer held together at Crane's Ford. The feud is off." *(mild: "Odo Rook and Perrin Dyer made peace after Crane's Ford.")*
- "Perrin Dyer died at Ashby. Odo Rook did not attend." *(mild: "Perrin Dyer died at Ashby; Odo Rook's feud is over.")*
- "The warband killed Hollis Crane in Ashby's lane. Hollis was Hild Crane's kin. Hild is still marching." *(mild: "The warband killed Hollis Crane at Ashby, kin of Hild Crane, who serves in it.")*
- Inspector mood note: "Wat is upset: Hild Ashby, kin, died at Loweby, and the mutton was bad."

**Bonds.**
- **Kin** are one household: people with the same `Villager.home`. Population builds households, and `Home.residents` lists each one, enlisted members included (they keep their home until they die). Kin are never stored; they're looked up through `Home.residents` (a handful of people). Spawned soldiers (`Soldier`) have no household and no kin. Kin on opposite sides are the existing `blood` key.
- **Friends** (shield-friends) form at `end_battles`: in an army that broke or lost a quarter of its fighters, two survivors who both held and ended the battle within 2 tiles of each other. In SpawnId order, each such survivor pairs with the nearest one not yet their friend: at most one new friend a battle, and two in all.
- **Rivals** form from:
  - **dice:** the loser, one chance in 3 (section 11);
  - **running first:** the held best killer of an army that broke, against whoever ran first;
  - **a weapon:** a named weapon passed outside its dead holder's household while one of that household served in the army, who takes the grudge against the new holder.
  
  At most one rival each.
- **Storage:** a sparse `Bonds` on enlisted people in a run, at most three entries (the other's SpawnId, friend or rival, since when).

**What bonds do** (through the holder, run-only):

| Bond | When | Effect |
| --- | --- | --- |
| Kin in the same army | Killed by a foe | A `grief` mood, −0.04 for 2 days. Within 12 tiles at the time, **vengeance** until the battle ends: morale +0.05 once, and aim at the killer whenever the killer is within 3 tiles of the nearest foe. Killing the killer is the Avenged kin deed and a `vengeance` line |
| Kin at home | Killed by a blow of the soldier's own faction (their army attacked the village) | A `kin_killed` mood, −0.08 for 3 days, with desertion ×2 for those days |
| Kin | The soldier killed their own kin (the `blood` key) | The same `kin_killed` mood, and the Kinslayer deed as today |
| Friend | Dies | A `grief` mood, −0.04 for 2 days |
| Friend | Serving in the same army | Desertion ×0.9 |
| Rival | Serving in the same army | Morale target −0.02, for both |
| Rival | Both hold in one battle | The feud ends |
| Rival | Dies, or a brawl's one-in-three | The feud ends |

**Moods.** A sparse `Mood` on enlisted people, introduced with mishaps (section 11) and filled out here: at most four timed entries (cause, until, morale-target change, desertion multiplier, about whom and where). A fifth replaces the one ending soonest. The sum is clamped: morale target ±0.08, desertion ×0.5 to ×2. Whenever an entry is added or expires (hourly, in `saga::settle`), the sums are written into `Veteran`'s cached nudges, so the parallel loops read nothing new. Vengeance is the one entry `soldiers::decide` reads directly, for its aim, and only its holders have it.

| Cause | Morale target | Desertion | Lasts |
| --- | --- | --- | --- |
| `grief` (kin or friend fell) | −0.04 | — | 2 days |
| `kin_killed` (by your own side, or by your own hand) | −0.08 | ×2 | 3 days |
| `vengeance` | +0.05 morale, once; the aim | — | Until the battle ends |
| `sick` (bad mutton) | −0.05 | — | 12 hours |
| `hungover` | −0.03 | — | 12 hours |
| `wedding` | +0.03 | — | 12 hours |
| `feasted` | +0.04 | — | 12 hours |
| `dice` | ±0.02 | — | 12 hours |
| `geese` (near a farm, or a goose rout) | −0.03 / −0.05 | — | The hour / 6 hours |
| `pious` (their army plundered) | −0.03 | — | 12 hours |
| `envy` / `mocked` (Vain) | −0.02 / −0.04 | — | 1 day / 2 days |

Rivals and friends act through the bonds themselves, and a sweetheart through the quirk; they aren't entries.

**Mood notes.** The inspector composes "{given} is {state}: {cause}, and {cause}." from the two strongest entries, the bonds and live needs (hungry; an empty baggage). The state comes from the summed morale-target change: ≤ −0.05 "upset", below 0 "uneasy", 0 nothing (no note), above 0 "content", ≥ +0.05 "in high spirits". Every cause names a record: "Hild Ashby, kin, died at Loweby", "the mutton was bad", "Perrin Dyer is in the same band". "The bread was stale" waits for M4b's food decay (SIM-22): food doesn't go stale in M4c.

**Data:** `progression.ron` `saga.bonds: (friend_radius: 2.0, max_friends: 2, max_rivals: 1, vengeance_radius: 12.0, vengeance_aim: 3.0, friend_desertion: 0.9, rival_morale: -0.02)` and `saga.moods` (one row per cause: morale, desertion, hours, and the clamps); words (bond lines, mood states, cause clauses) in saga.ron.
**Determinism:** rules over records, in serial steps (casualties, `end_battles`, `settle`), in SpawnId order. The only draws are the dice grudge (the mishap's stream) and the sweetheart (section 10).
**Hashing and saving:** `Bonds`, `Mood` and `Veteran`'s cached nudges are hashed and saved; an entry's `about` is an `Entity`, saved as SpawnId + 1 like any other.
**Performance:** kin lookups happen only when a villager-recruit dies (spawned soldiers have no household, so `--battle` pays nothing); friends form hourly, at `end_battles`, with the fighter grid; moods expire hourly over the sparse set; the hot loops read only the cached nudges and, for the few with one, the vengeance entry.
**UI:** the inspector's "Kin in the band: Hild Ashby", "Shield-friend: Bram Tull", "Feud: Perrin Dyer (since Loweby)", and the mood note; the captain panel's "Mourning: 5 · Feuds: 2". In the feed: vengeance kills are told deaths (section 2); `kin_killed` lines for your soldiers at 1x–4x; feud and friendship lines at 1x–2x only; none of them at 8x.
**Tests:** `kin_grieve_and_avenge` (grief, the vengeance aim, the deed, and `kin_killed` when your own army kills a recruit's household); `friends_and_rivals_form_by_rule` (each source, the caps, and each way a feud ends); `moods_are_bounded_and_expire` (the clamp, replacement, expiry, and the cached nudges matching the entries); `moods_are_exact_no_ops` (no bonds and no moods: bit-identical morale and desertion).

## 14. Rescues and hero memorials

**Rescues (M4c).** When a village's `DriveOff` request is answered (`requests::complete`), M4c records who saved it:
- **The rescuers** are the members of the answering captain's army who fought a battle at that settlement (its nearest settlement, section 2) since the request was posted, read from `Service.fights`.
- **The heroes** are up to three of them: legends first, then those with an epithet by fame, then that battle's best killer. If none of the rescuers is named, the captain is the hero of record.
- **The deed.** Each hero earns the Rescuer deed (section 6).
- **The record.** `Saga.rescues` keeps the settlement, the tick, the battle, the faction, the heroes (`NameId`s and SpawnIds) and the request id.
- **The words.** The `RequestDone` line names the hero when there is one: "Wat Nine-Kills saved Ashford from the raiders." The settlement's history keeps it: "Saved from the raiders on day 6 by Wat Nine-Kills. Ashford remembers."
- **No new effect in M4c.** The request's reward and goodwill are as built.

**Memorials in M4c are render-only:**
- a **cairn** where a legend fell, and a **memorial stone** at their home village's centre, if they had one;
- hovering either shows the epitaph;
- both are drawn by the renderer from the sim's records; there's no sim entity.

A rescue leaves no mark on the map in M4c.

**Hero memorials (M4b).** From M4b, a village that remembers a rescue raises a **memorial** to its hero once it can build one: a statue, cairn or shrine in its town style, named for the hero. It's a real building with small, local effects: goodwill and loyalty there, recruits a little more willing, and the hero's own army steadier near it. If the hero dies, the memorial becomes a **grave the town tends**, with a chronicle line ("Ashford tends Wat Nine-Kills's grave now"). Across runs, the hero and the memorial appear only in M6's legends book, as history. The design is in `docs/TOWNS_DESIGN.md` ("Hero memorials"); M4c's part is the rescue record it reads.

## 15. Biographies, and where it all shows (UI-09, RND-09)

**Biographies** are two to four sentences, put together from the record when read:
- *"Tobin Kettle, called Banner-Taker, once Goose-Shy. A baker of Ashby, pious, afraid of geese; joined the warband on day 2."*
- *"Six battles, fourteen kills, among them captain Rook Vane at Kemp's Ford, with Vane's Bane; wounded twice."*
- *"Held the line three times; stood when it broke at Kemp's Ford; avenged Hild Kettle at Loweby. Brave."*
- *"Fell at the Barrow Stones on day 9, to a great grey wolf."* or *"Lives, scarred, with Garrick Hale's band; still feuding with Perrin Dyer."*

Spawned soldiers have no home: "a sword of the starting band", "a raider out of the north". An ordinary soldier gets one line: "3 battles, 2 kills, wounded once. A glutton."

**Settlement histories** are the other half of UI-09's biographies: *"Ashby sent fourteen to the warband; six lie dead. Tobin Banner-Taker came from here. Conquered on day 3; rose on day 6, when Linnet Ware was killed in the square. Saved from the raiders on day 11 by Wat Nine-Kills. Vane's Bane hangs in its storehouse."*

| Where | What you get |
| --- | --- |
| **Feed** | Told deaths (read from `Saga`), `Starved`, `Named`, `TraitEarned`, `LegendRose`, `LegendFell` and `Mishap` lines, and saga notes (weapons, kin, feuds), coloured by kind (your dead, your kills, legends in gold, mishaps dimmer); the speed filter and fold (section 2) and the tighter limits for the rest (below); clicking a line selects the person if alive, or else moves the camera there; a Brutal/Mild switch |
| **Inspector** | The epithet after the name; quirks; "Kills 14 · Battles 6 · Wounds 2"; deeds in words; earned traits with what earned them; the weapon carried and its history; kin in the band, friends and a feud; the mood note; a legend's "Steadies those nearby". The biography replaces "Memories: none yet" for anyone with a record (other villagers keep it until M5's memories, and show their quirks) |
| **Captain panel** | "Notables": the army's legends and up to three other named men; clicking one selects them. "Mourning: N · Feuds: N" |
| **Settlement panel** | The settlement's short history, its rescues and the relics in its storehouse |
| **Battle reports** | The feed's battle line opens the whole report with its moments; the end screen uses the same words |
| **End screen** | "Legends of the run": up to six, yours first, each with their numbers and fate in one line and the epitaph on hover. "Foes remembered": enemy legends, and whoever killed one of your legends or your champion. "The Fallen": how many of your people died, with the named ones listed. "Oddities of the run": the strangest death (the highest-scoring irony), the most storied weapon and its fate, the run's mishaps by kind, and its longest feud. The bloodiest battle, by name. The timeline gains legends risen and fallen |
| **Map** | A small pennant over each living legend inside the detail band; cairns and memorial stones |
| **New-game screen** | "Words: Brutal / Mild" beside the difficulty buttons |

**Readable at 8x, for the new lines too.** Mishap, weapon, kin and feud lines are lower priority than told deaths and events, and the throttle drops them first. At 8x and fast-forward they pass only as sections 11 to 13 say (a mishap that killed one of yours or hit your champion's army; a legend's weapon), and over every `bot_run` seed **at most 2 such lines a game day** (about 30 real seconds at 8x) pass the 8x filter. At every speed, at most one mishap line every 20 real seconds.

Two military log lines that print a SpawnId where a name belongs ("the captain fell; 4127 takes command", and the champion's succession line) are fixed to print the name.

## 16. What M6's legends book will read (GP-29)

When a run ends, the game writes a small **run record** next to its saves (in `legends/` under the save directory), before the finished run's save slot is removed. The record:
- **Holds facts and proper nouns, never sentences.** Names and places are written as plain strings, because a run's name table dies with it. Epithets, deeds, traits, quirks, ironies and mishap kinds are ids, so M6's book can tell them in its reader's tone with its build's words.
- **Is read-only history.** No code reads it in M4c, and nothing in it changes a later run (your decision 4).
- **Is built the same way in headless.** A pure function builds it from the finished world, as the run summary is built, so `sim_headless --legends-out <path>` writes the same record.
- **Is one file per ending:** `legends/<seed>-<difficulty>-<ended tick>.ron`. A run finished again after a quickload, at the same tick, overwrites it, and a different ending writes another file. Both are true histories of play, and M6 decides how to show them.
- **Covers abandoned runs too,** with outcome `Abandoned`, so the book can choose to list or hide them.

```ron
(
    record: 1, seed: 7, difficulty: Harsh, outcome: Won, cause: Conquest, days: 31,
    champion: (name: "Brand Ashby", epithet: Some(14), rank: 3, traits: [0, 5], earned: [], quirks: [3]),
    legends: [
        (name: "Wat Ashby", home: Some("Ashford"), faction: 1, trade: Some(Baker), epithet: Some(5),
         quirks: [2, 8], kills: 14, unarmed: 4, battles: 9, wounds: 2,
         deeds: [(FirstBlood, day: 3, place: "Crane's Ford", victim: Some("Aric Brook")), (LastStand, day: 11, place: "Kemp's Ford", victim: None)],
         earned: [Brave], weapon: Some("Vane's Bane"),
         rose_day: 12, fate: Fell(day: 27, place: "Loweby", killer: Some("Alda Venn"), killer_epithet: Some(14), irony: None)),
    ],
    foes: [ /* enemy legends, and whoever killed one of your legends or your champion, same shape */ ],
    fallen: (yours: 23, named: ["Brenna Hale", "Garrick Hale"]),
    battles: [ /* the three bloodiest, by number and name facts, with losses and kills */ ],
    weapons: [ (name: "Vane's Bane", word: "axe", named_day: 11, named_for: "Rook Vane", kills: 7,
                holders: ["Jory Lowe", "Hild Lowe", "Wat Ashby"], fate: Sold(day: 19, place: "Loweby")) ],
    rescues: [ (place: "Ashford", day: 6, heroes: ["Wat Ashby"]) ],
    oddities: (mishaps: [(BadMutton, 3), (Wedding, 2)], strangest: Some((name: "Bram Tull", irony: 9, day: 14))),
)
```

The weapon's `name` is written as words in the run's tone-neutral form (weapon names are the same in both tones), since the pattern that made it may change. M4b adds `memorials` (section 14).

## 17. How it fits what's built (technical notes)

**The pipeline in one picture.** The schedule gains one step, **slot 13 of the hourly progress driver, after `director::step`**. The driver has 12 slots today: `dominion::refresh_slots` shares slot 10, the rival clock is 11 and the director is 12. Everything else lives inside existing systems.

```
Setup      progression::setup_run   + quirks dealt to every person, after every other setup spawn (run only)
           spawn::spawn_army        + quirks dealt to the people it spawns in a run (never beasts)
Input      orders::run_orders       plunder and food given leave cruelty and kindness marks on the captain;
                                    a halted march waits out Captain::halt_until; named weapons go out first
                                    in arming and last in a barter
Knowledge  index::build_index       + who ran first and when each army broke (broke_share);
                                    LegendIndex (each faction's enlisted legends; run only)
Decide     sim_agents::ai::decide   quirk factors (run only; a fifth Factors slot shows them)
           soldiers::decide         personal break point (cached nudges: quirks, traits, moods, a legend nearby);
                                    Cruel aim and vengeance aim (run only)
Act        combat::resolve_strikes  a kill pushes (victim, killer, credited side, one blow); a blow adds to the victim's
                                    hurt this battle; the battle bookkeeping joins or allocates battle numbers
           combat::casualties       pass 1, killers in strike order: snapshot each killer, start or update their record,
                                    streaks, best killer, weapon naming
                                    pass 2, the dead in SpawnId order: classify (manner, irony); tell tier A, then tier B
                                    against the bar and the caps (Fall / LegendFell / ChampionFell); keep named deaths;
                                    a named weapon's fate; kin and friend moods, vengeance; a legend's shock;
                                    then today's work (morale entry, goodwill, succession, despawn)
Needs      sim_agents::update_needs quirk hunger and rest rates (run only)
           upkeep::soldier_upkeep   marks "ran this battle"; legend aura; cached nudges to break point, morale target,
                                    desertion and hunger
           progress slot 1          end_battles: wounds, held and broke marks, deeds, the report's moments, judging
                                    survivors; friends and rivals form; feuds end
           progress slot 6          requests::step: an answered DriveOff records its rescue
           progress slot 12         director::step: + the mishap clock and the mishaps (run only)
           progress slot 13 (new)   saga::settle: judge every record changed since the last hour; the hourly starvation
                                    lines; expire moods and rewrite cached nudges; Fears geese and Sweetheart checks;
                                    reconcile named weapons' places
Words      chronicle::Teller        kill lines, moments, mishaps, notes, biographies; the dead named from the saga's
                                    records; a tone parameter
```

**The sim stores no text** (ADR 0017):
- Records hold SpawnIds, `NameId`s, ids for epithets, deeds, traits, quirks, ironies, mishap kinds and mood causes, and counts.
- Epithet strings and weapon names never go into the `NameTable`, which is hashed and saved.
- The dead are named from their kept `Fall`s, copied before they are despawned. Anyone ever named (section 2) has their death kept, so a named person is never told as "a soldier".

**Told deaths and notes are not events.** They live in `Saga`, append-only, and the feed reads them with its own cursor. So a big battle can't push `OfferPosted` or `LegendFell` out of the 256-entry `EventFeed` ring during a frame hitch, and the auto-pause is never lost. `Saga` keeps every told death of the run, so the feed never misses one either. The same holds for **saga notes** (`Saga.notes`: weapon namings and fates, friendships, feuds begun and ended, kin killed), which are personal history, not events. Mishaps are events (`SimEventKind::Mishap`) because they're the director's beats, and they're not notable.

**New state:**

| Type | What | Hash | Save |
| --- | --- | --- | --- |
| `Casualties.kills` | This tick's kills (replaces `slain_by`) | Exempt scratch, as today | No |
| `Veteran` gains `routed`, `hurt`, `trade`, and the holder's cached nudges | Ran this battle; share of health lost to blows this battle; trade before enlisting; break-point offset, morale-target offset, desertion multiplier and hunger multiplier, written only by serial steps (the quirk deal, `judge`, mood changes, rebuilt from `Service`, `Quirks` and `Mood` on enlisting) so the parallel loops never read the sparse components | Yes | Yes |
| `Service` (new, sparse) | The service record (section 6) | Yes | Yes |
| `Legend` (new) | `{ faction, rose }` | Yes | Yes |
| `Quirks` (new, sim_agents) | One or two quirk ids per person, in a run | Yes | Yes |
| `Mood` (new, sparse) | Up to four timed entries (section 13) | Yes | Yes |
| `Bonds` (new, sparse) | Up to three friend or rival entries | Yes | Yes |
| `NamedWeapon` (new, sparse) | Index into `Saga.weapons` | Yes | Yes |
| `Saga` (new resource) | Told and kept deaths (`Fall`, append-only, with manner and irony), battles (number, first blow, place, ordinal, terrain, lines told, first blood), per-settlement battle counts, legends risen, grudges, weapons, rescues, notes, pending mishap deaths, untold count, `next_battle` | Lengths and a running digest, like the `Chronicle` (O(1)) | Yes |
| `Captain` gains `battle`, `battle_broke`, `battle_best`, `battle_ran_first`, `battle_deserted`, `starved`, `halt_until` | The battle number, this army's moments, and a wedding's halt | Yes | Yes |
| `Director` gains `next_mishap`, `mishap_day`, `mishaps_today`, `mishaps`, `deaths_day` | The mishap clock | Yes | Yes |
| `BattleReport` gains moments | Section 5 | In the chronicle's digest | Yes |
| `LegendIndex` | Rebuilt in Knowledge before any reader | Exempt | No |
| `SagaWords` | saga.ron | Exempt ("words only; the sim never reads it") | No |
| Tone, feed cursors and throttle state | game_ui | Outside the sim | Tone in the preferences file |

- **Append-only enums.**
  - `SimEventKind` gains `Starved`, `Named`, `TraitEarned`, `LegendRose`, `LegendFell` and `Mishap`.
  - `Cause` gains `Earned { deed }`.
  - Deed ids, epithet ids, trait id 7, quirk ids, quirk effects, mishap kinds, irony ids, mood causes, bond kinds, note kinds, weapon places and the death-manner bits are append-only as well. Epithet ranks are data and may change.
- **Saves.** `SAVE_FORMAT` goes to 2, and up again with each later step that changes a saved layout. M4 playtest saves will be refused; M4c follows the playtest, so that's expected.
- **Replays.** The replay format doesn't change: no new commands, and the tone isn't in the header. Like every M4 step, the checkpoint hashes change, so replays recorded before M4c must be re-recorded.

**Pacing:**
- Told deaths and saga notes are not events; `Starved` is feed-only, not chronicled.
- `Named`, `TraitEarned`, `LegendRose` and `Mishap` are chronicled for biographies and the end screen, but are **not notable**. They don't reset the pacing director's quiet clock, count as the run's first event, or stop fast-forward. The end-screen timeline hides `Named`, `TraitEarned` and `Mishap`.
- `LegendFell` is chronicled and notable. It's rare and happens in battles, which are notable already.

**Determinism:**
- Kills are recorded in the serial strike loop, in striker SpawnId order, and battle numbers are joined in the same order.
- Deaths are classified in the serial casualty loop, in victim SpawnId order (tier A told first, then tier B), so the indices of told deaths are deterministic. Weapon fates, kin moods and vengeance are decided there too.
- Judging, bond formation, mood expiry and weapon reconciliation run in SpawnId order in serial steps.
- The only new parallel writes are "ran this battle", each soldier's own flag. The aura, traits, quirks and moods change only the morale and hunger each soldier already writes, through cached nudges.
- **Draws (amended).** The first draft said every rule is a threshold and no new RNG stream is needed. That still holds for kills, records, deeds, epithets, ironies, traits, legends, weapons, bonds and moods, and for every word. Your follow-up adds draws where chance is the point, each from its own new key, so no existing stream moves:

| Stream key | Seeded at | Draws |
| --- | --- | --- |
| `sim_agents::quirks ^ SpawnId` | tick 0 | A person's quirks: one or two, which, by weight |
| `sim_military::director::mishap` | The hour's tick | Whether a mishap comes, its kind and its army |
| `sim_military::mishap ^ n` (n = the run's mishap count) | The hour's tick | Its subjects, counts and rolls: the choke, the dice, the grudge, the wandering tile |
| `sim_military::saga::homesick ^ SpawnId` | The hour's tick | A sweetheart's walk home |

**The run gate.** Records and lines exist in every preset. Behaviour changes only in a run:

| | Every preset | Run only (`in_run`) |
| --- | --- | --- |
| Killers, battle numbers, records, told deaths, `Starved` lines, battle moments | Yes: hashes change, stdout doesn't | |
| A rebel's kill credited to the rebels | Yes, wherever rebels fight; the guarded developed, skirmish and battle commands have none | |
| Epithets (honourable and mocking), ironies, deeds that need a run (Breadgiver, Plunderer, Rescuer), earned traits and their effects, legends, aura, shock, the report of an army wiped out | | Yes |
| Quirks, mishaps, named weapons, bonds and moods, rescue records | | Yes |

Developed, skirmish and `--battle` stdout stay byte-identical. Kill lines appear in the skirmish too, which helps test the words. `villagers.ron` and `director.ron` gain sections, so the developed world's state hash changes (`Villagers` and `DirectorData` hash their text) while its behaviour doesn't: `village_day` and `famine` are re-run to show it.

**Performance (NFR-09).**
- **Baselines.** Recorded three times at step 1, with their spread, in `docs/m4c/`: M4's numbers for the 7,500-a-side battle (seed 42: deaths a tick, the clash window's mean tick, and the Act, Decide and Needs stage times), and the run preset's mean tick and Decide and Needs stage times (`--run --seed 7 --difficulty steady --ticks 24000 --timings`), where 4,000 to 5,000 villagers live.
- **Per blow:** one byte added to the victim's battle hurt, and a push onto a vector, on a kill only. No query and no allocation.
- **Per death:** a few microseconds of reads and one record; a weapon lookup; a household lookup for villager-recruits only.
- **Per soldier, in a run:** at most three distance checks for the aura, and none when the faction has no legend. Traits, quirks and moods are read from `Veteran`'s cached nudges, which the loops already hold. The developed stress world has none of them and pays nothing.
- **Per villager, in a run:** one optional two-byte component read in `decide` and `update_needs`.
- **Hourly:** mishaps (tens of armies), mood expiry and quirk place checks (sparse sets), weapon reconciliation (tens of weapons).
- **Measurements.**
  - `--battle 7500 --timings` covers every preset's bookkeeping.
  - `--battle 7500 --saga-bench --timings` turns on the saga's run-only effects (not the rest of the run rules) and gives each army three legends, every soldier a quirk, every tenth soldier an earned trait and a friend, every fifth a mood, and three named weapons, so the per-soldier costs are paid.
  - `--run --seed 7 --difficulty steady --ticks 24000 --timings` covers quirks on the villagers and the hourly steps.
- **Target.** For each, the median of three runs of the mean tick (the clash window's, for the battles) rises at most 2%. If three runs of one build spread more than 2%, the gate compares the Act, Decide and Needs stage times instead.

**Data:**
- `progression.ron` gains a `saga` section (`#[serde(default)]`, like `gather` and `hire`), holding:
  - the battle terms (`broke_share`);
  - the notability weights, bar and caps;
  - deed fame;
  - epithet rows (id, kind, rank, rule) and the irony rules' numbers (`ironies`);
  - earned-trait triggers and effects;
  - the legend rule, aura and shock;
  - `weapons`, `bonds` and `moods` (sections 12 and 13).
- `progression.ron`'s `traits` gains Cruel (id 7), and `chronicle.causes` gains the phrase for `Cause::Earned`.
- `villagers.ron` gains `quirks` (section 10).
- `director.ron` gains `LegendFell` in `auto_pause`, `mishaps` in each personality, and `mishap_rules` and `mishap_kinds` (section 11).
- The new `saga.ron` holds the words, two tones of the same shape, so a key missing from one tone fails to parse. It also holds each key's fact and flavour slots, quirk and mishap words, weapon-name patterns, mood words, and the feed throttle, which is viewer data like `render.ron`.
- Every field is documented in `docs/dev/DATA.md` (the docs test enforces it).

## 18. Traps and guards

| Trap | Guard |
| --- | --- |
| Log spam in big battles | Tier A narrowed to captains, the champion and legends (and a legend's milestone kills, and the rare mishap death); the rising bar; at most 10 tier-B lines a battle and 8 told deaths a tick; summaries; the feed's speed filter, rate limit and fold; `bot_run` checks at most 8 lines a battle pass the 8x filter |
| Unreadable at 8x | Only tier A about you or a legend at 8x and fast-forward, rate-limited; mishap, weapon, kin and feud lines are dropped first, at most 2 a game day pass the 8x filter (`bot_run` checks), and one mishap line every 20 real seconds at any speed; feed lines fit 72 characters, the name first |
| Repetition kills brutality | Composition, weapons fixed per person, the prime-step variant choice, a lint test of bank sizes, and a brutal flourish rationed to the top moments of a report |
| Brutality turns tasteless, especially toward villagers | Militia and rebel victims always take a grave key and never an irony; written content rules; a deny-list test; a playtest question |
| Deadpan tips into silly | The voice rules (section 4), with do and don't examples in DATA.md; the lint bans `!`, `?`, capitals and winking words; a playtest question |
| Mild reads as comic | A "clerk's report" register; a playtest question |
| Words claim things that didn't happen | "Invent the manner, never the facts": every fact comes from a record, times only from `{daypart}` and `{then}`, trades only from the sim's occupations; flavour only on a real effect; ironies only from section 7's table, and the "not in M4c" ends have no key |
| Quirks are noise | Each has a real effect, listed with its numbers and tested; the geese are the one flavoured trigger, on a real farm; Hates rain waits for weather |
| Quirks shift balance or a village's food | Tiny effects, through the holder, run-only; `bot_run` after the quirk step; `village_day` and `famine` re-run after the villagers.ron change |
| Mishaps feel like punishment | Low-tension beats; a third fall on your enemies; good ones too (a wedding, a feast); a playtest question |
| Mishaps wipe a run or a band | No champion, captain or legend as subject; deaths from two kinds only, one a day, in armies of 10 or more; food floors; no delay with a foe near; each guarded by a test over every `bot_run` seed |
| Mishaps change pacing | Not notable: the quiet clock, the first event and the raider schedule don't see them (tested) |
| Moods snowball into desertion waves | Four entries at most, clamped sums, desertion ×2 at most and only from the rare `kin_killed`; tested |
| Named weapons duplicate or vanish | One place per weapon; named out first in arming, last in a barter; hourly reconciliation; tested in every way it moves |
| Kin and feuds cost too much | Kin are never stored (`Home.residents`); bonds and moods are sparse and hourly; spawned soldiers have no household, so big battles pay nothing |
| Legends turn into heroes | Three enlisted per faction, an aura a third of a captain's with no attack bonus, a 0.10 shock, traits through their holder only; tests pin the sizes |
| Legends snowball (they kill more, rise more) | Fame counts each deed once; the cap; the aura touches morale only |
| Legends too rare to notice | Tuned against `bot_run` (at least one on three of five seeds); epithets, far more common, carry the mid-game |
| Earned traits everywhere | Marks count only from eligibility, one of each kind per battle; target rates checked against `bot_run` |
| Balance drift | Every effect is run-only and exactly nothing without a trait, quirk, mood, bond or legend; `bot_run` is re-run after each step that may change a run |
| Lost names of the dead | Kept `Fall`s hold `NameId`s; anyone ever named has their death kept |
| Beasts read as people | `{beast}` nouns; beasts never fill a person's slot, never get a record or quirks, and never loot |
| Cost | A push on a kill is the only new work per blow; nudges cached on `Veteran`; hourly work for the sparse parts; NFR-09 is measured on `--battle 7500`, with and without `--saga-bench`, and on the run preset |
| Name clashes | Goodwill's `Deed` and the champion's `Feat` already exist: the new type is `SagaDeed`, the cause `Cause::Earned`, and the glossary says "Saga deed". "Fame" avoids FUN_DESIGN's "renown"; "Mood" and "Bonds" are new; a quirk is never called a trait |

## 19. Proposed roadmap change

| Milestone | Was | Proposed |
| --- | --- | --- |
| **M4c "Saga"** | — | New, after the M4 playtest and before M4b: kill lines and battle moments, service records, deeds and epithets (honourable and mocking), absurd deaths, SIM-34's earned traits, legends and render-only memorials, quirks, mishaps, named weapons, kin, friends, rivals and moods, rescue records, UI-09's biographies, end-screen legends and oddities, the tone setting and the deadpan voice, and the run record for M6. About one and a half times the first draft's size; the plan suggests a split after its step 8 |
| M4b | Living towns | Also: hero memorials raised by saved villages, which become tended graves (section 14); animals with names; weather quirks |
| SIM-34 | M4 | M4 (champion traits), M4c (earned traits) |
| UI-09 | M4 | M4, M4c (biographies) |
| M5 | Scale and information | Also: tall tales and rumours about you, through faction knowledge |
| M6 | The legends book of past runs | Unchanged; it reads M4c's run records (and M4b's memorials) |

Requirement IDs added to SEED.md on 2026-10-12 (proposed, with M4c and M4b):

| ID | Requirement |
| --- | --- |
| GP-29 | A finished run leaves a record of its legends (names, epithets, deeds, fates), named weapons and the villages they saved, for the legends book of past runs; nothing in it changes a later run |
| GP-30 | In a run, the pacing director schedules small mishaps from real sim state (bad mutton, a dog at the rations, a wedding on the road, a slip in the shield wall); each is low-stakes and rate-limited, and none ends a run |
| SIM-39 | Every death in battle records who killed whom and how, including absurd but real ends; notable deaths are told one by one, and the rest are counted into the battle's report |
| SIM-53 | Soldiers keep a service record (kills, battles, wounds, deeds) that outlives desertion and, on death, is kept for anyone told or named; they earn one epithet by rule, honourable or mocking, which says why |
| SIM-54 | A few soldiers of great fame in each faction become legends: a legend steadies nearby men a little and their death shakes the army; legends gain no stats and are not heroes |
| SIM-55 | In a run, every villager and soldier has one or two quirks dealt from a data deck by their own seeded stream; each nudges only its holder's behaviour, slightly |
| SIM-56 | A weapon that kills a notable foe gets a name and a history, and is passed on, looted, sold, taken home or lost as the sim moves it; named weapons gain no stats |
| SIM-57 | Soldiers have kin (their household) and make friends and rivals by rule; kin grieve and avenge, rivals feud and make peace, and each soldier's mood is the bounded sum of its recorded causes, which the inspector explains |
| SIM-58 (M4b) | A settlement saved from raiders by a named hero raises a memorial to them in its town style once it can build one; it gives small local effects and becomes a tended grave when the hero dies |
| RND-09 | Living legends are marked on the map; a cairn stands where a legend fell, and a memorial stone in their home village |
| UI-10 | Kill lines, mishaps, mood notes, battle reports and biographies are told from phrase banks in data, in a deadpan dark voice, brutal by default and mild by choice; the choice is a viewer setting the game remembers, and it never changes the simulation |
| NFR-09 | Saga bookkeeping adds at most 2% to the median mean tick of the `--battle 7500` clash, with and without the saga's run effects forced on, and of the run preset, with no allocation per blow |

## Decisions (2026-10-12)

Yours, binding:
1. **M4c "Saga" is its own milestone,** after the M4 playtest and before M4b "Living towns".
2. **Brutal and graphic by default, with an option for milder wording;** both are phrase banks in `assets/data/*.ron`.
3. **Legends have small gameplay effects:** earned traits (Brave, Cruel, Beloved, Shaken) nudge morale and AI; a legend steadies nearby men; a legend's death shakes the army. No hero units, no big stats.
4. **Across runs, legends are history only.** M6's legends book reads them; no power carries over. M4c writes the record.

Defaults I chose to make the design concrete (change any):

5. **Words in a new saga.ron, rules in hashed data.** The words are exempt from the hash and not saved; the rules are hashed.
6. **The tone is a viewer setting,** kept in a preferences file next to the saves: never a command, run setting, save or replay field. Headless takes `--mild`.
7. **Records and kill lines in every preset; effects only in runs.** Developed, skirmish and `--battle` stdout stay byte-identical.
8. **Earned traits act through their holder.** They never reach `FactionModifiers` or another person's morale, and only captains, the champion, legends and three-battle soldiers earn them.
9. **Three enlisted legends per faction at most,** for any faction but beasts.
10. **Memorials are render-only in M4c:** a cairn where a legend fell, and a stone in their home village. (Your decision 14 adds M4b's memorials with effects for rescues.)
11. **The run record holds facts and proper nouns,** not sentences.
12. **Battles get a number,** so both sides' reports of one fight share its name, first blood and rising bar.
13. **Told deaths live in `Saga`, not in the event feed ring.**

Yours, binding (follow-up, 2026-10-12):

14. **Hero memorials (M4b, fed by M4c).** M4c records rescue deeds: which named soldiers, a legend first, drove raiders off a village, in their service records and the village's history ("Wat Nine-Kills saved Ashford from the raiders"). In M4b a saved village raises a memorial (a statue, cairn or shrine in its town style) named for the hero once it can build one, with small effects: goodwill and loyalty there, recruits a little more willing, and the hero's own army steadier near it. If the hero dies, it becomes a grave the town tends, with a chronicle line. Across runs, only in M6's legends book. M4c keeps its memorials render-only. This resolves open question 2.
15. **Dwarf-Fortress-style fun is adopted into M4c, in a deadpan dark voice:** a grim world, absurd outcomes reported matter-of-factly, funny because played straight, consistent with the brutal kill lines (section 4).
16. **Quirks:** one or two per villager from a data deck, with tiny real effects on behaviour (section 10).
17. **Mishaps:** small sim-grounded incidents scheduled by the pacing director, low stakes, never run-ending, rate-limited (section 11).
18. **Unflattering epithets and absurd-but-real deaths,** told in the deadpan voice (section 7).
19. **Named weapons:** weapons that kill notable foes get names and histories, and are passed on, lost, looted or sold (section 12).
20. **Kin, friends and rivals in the ranks:** vengeance, desertion for love, camp feuds, and mood notes in the inspector (section 13).
21. **Guards:** rate limits keep the feed readable at 8x; everything is sim-grounded, never pure noise; everything is deterministic, with new `SimSeed` stream keys for quirk deals and mishap rolls (section 17 lists them; this amends the first draft's "no new RNG"); mishaps never wipe a run; the effects are run-only, so the developed world and the skirmish don't change.
22. **Not adopted for M4c:** animals with names (M4b) and tall tales or rumours about you (M5's knowledge). See "Later".

Defaults for the follow-up (change any):

23. **Eleven quirks** (section 10) in `villagers.ron`, dealt only in runs. The geese of Fears geese are words on a real farm or pasture; Hates rain waits for M4b's weather.
24. **Mishaps are chronicled, never notable:** they don't reset the quiet clock, stop fast-forward or pause. About one a game day on steady; a third fall on armies at war with you.
25. **Only two mishaps can kill** (a slip in the shield wall, the feast's choke): one ordinary soldier at most, one mishap death a game day, in armies of 10 or more.
26. **Mocking epithets rank below every honourable one,** so a deed can wash one off and nothing honourable is ever replaced by one.
27. **Absurd deaths only from causes the sim produces** (section 7's table). Drowning, sheep, weather and walls wait for the milestones that add them.
28. **A named weapon is a sparse `NamedWeapon` beside the `Equipment.weapon` bool,** with no stats; ordinary weapons stay anonymous.
29. **Kin are one household (`Villager.home`) and never stored;** friends and rivals are a sparse `Bonds`, at most three; moods are a sparse `Mood`, at most four entries, clamped and cached on `Veteran`.
30. **Moods are for the ranks only.** Villagers' memories and grief at home stay M5's (SIM-14).
31. **Love is the Sweetheart quirk:** a desertion near home, never a bond between two simulated people (the sim has no courtship).
32. **Rescues are recorded for the player's answered drive-off requests,** the only rescues the sim knows; they add no effect in M4c.

## Open questions (with recommended defaults)

1. **A light nemesis.** Should the enemy who killed a legend be named in later lines and on the end screen, with an Avenger deed for whoever kills them? This reopens FUN_DESIGN's "nemesis rivals: not adopted" in the smallest way. *Default: yes, words and the deed only, with no AI change.*
2. **Memorials with effects.** *Resolved 2026-10-12 by your decision 14:* M4c keeps render-only cairns and stones and records rescues; M4b raises sim memorials with small effects for rescues, which become tended graves (section 14, `docs/TOWNS_DESIGN.md`). Villagers mourning near a legend's cairn would still fit M5's memories (SIM-14).
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
13. **Mishaps on your enemies.** Should raiders and the rival have mishaps too? *Default: yes, a third of them, so the odd luck isn't only yours; you see theirs when they're near your armies.*
14. **Deadly mishaps.** Should a mishap ever kill? *Default: yes, the slip and the feast only, one ordinary soldier, at most one a game day. They're the absurd deaths people retell. Say no and both become wounds.*
15. **Quirks before recruiting.** Should you see a villager's quirks before they join? *Default: yes, in the inspector; recruiting still takes whoever is willing, so it informs and doesn't choose.*
16. **Mocking epithets on legends.** "Odo Twice-Fled, legend of the warband." *Default: allowed. Any honourable epithet the legend earns replaces it, so it's rare.*
17. **Rivals across the lines.** Should a recruit whose household member serves the rival get a grudge? *Default: no new bond; the `blood` key already tells it when they meet.*
18. **Starting numbers for the new parts:**
    - quirks: one, or two with 0.4; the deck's weights and effects as in section 10;
    - mishaps: steady 16 hours apart, 0.3 an hour after that, 2 a day; one death a game day; a food floor of one day;
    - weapons: 24 a run, 2 a battle;
    - bonds: two friends and one rival each; grief −0.04 for 2 days, `kin_killed` −0.08 and ×2 desertion for 3 days; moods clamped to ±0.08 and ×0.5 to ×2;
    - at most 2 mishap, weapon, kin and feud lines a game day passing the 8x filter.

    *Default: tune with the bot's counts (quirks, mishaps by kind, weapons, feuds, desertions by cause) and the playtest.*

## Later (not in M4c)

- **Animals with names (M4b):** livestock and pets as agents. They make Fears geese literal, "trampled by sheep" possible, and give the dog at the rations a name.
- **Tall tales and rumours about you (M5):** villages repeating, and distorting, what your armies did, through faction knowledge (SIM-40).
- **Weather quirks (M4b):** Hates rain, and moods from cold and storms (SIM-05, SIM-06).
- **Stale food (M4b):** food decay (SIM-22) makes "the bread was stale" a real mood cause.
- **Drowning at a ford:** needs rivers or fords, which the island doesn't have.
- **Healing and festering wounds (M4b).**
- **Grief at home (M5):** villagers remembering their dead (SIM-14).
- **Hero memorials (M4b):** decision 14 and `docs/TOWNS_DESIGN.md`.
