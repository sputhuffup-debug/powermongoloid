# Design: Saga, kill lines and legends

Status: **draft 2026-10-12, for your approval** (revised the same day after review, again with your follow-up decisions: hero memorials, and Dwarf-Fortress-style oddities in a deadpan dark voice, once more after a second review of those additions against the code, then with your combat-spectacle decisions: gore effects drawn by bevy_hanabi or a CPU fallback, maimed survivors and a kill cam, sections 20 and 21, and last after a review of those against the code and Hanabi's source). All your decisions are recorded in the Decisions section at the end, and the open questions there each have a recommended default. M4c is planned in `docs/M4C_PLAN.md`, and SEED.md has its section ("M4c: Saga") and new requirement rows. This doc answers your ask, "for combat we need some flavor in the kill messages to make it more brutal and we can make specific soldiers have legendary histories", and your follow-up: every run should be strange in its own way, told with a straight face.

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

Your combat-spectacle decisions (2026-10-12, sections 20 and 21, decisions 42–47; my defaults for them are 48–66):
- **Gore, on the render side only.** Blood spatters on hits, bigger bursts come on kills, and pools fade in about a game hour. On told kills, severed heads, arms and legs arc, bounce and settle. Bodies lie where they fell, then fade. **The part that flies is the one the kill line names:** both come from the kill record.
- **Over the top** (Battle Brothers, Dwarf Fortress) in the brutal tone. The mild tone shows no blood and no parts: dust and falls only.
- **Maimed survivors are a sim fact.** A survivor of a bad wound can lose a hand, an eye or a leg. They get a small lasting penalty, a service-record line and an epithet (One-Hand, ...), and lines and visuals agree. It's deterministic, hashed and saved, and its effects are run-only, like the rest of M4c (section 21).
- **Kill cam.** About a second of slow motion and a camera nudge when a legend, the champion or a captain dies near the camera. It has an options toggle and never plays at 8x or in fast-forward. Slow motion is a `Time<Virtual>` effect, never sim state.
- **bevy_hanabi is approved** as a new dependency: `=0.19.0`, default features off plus `3d`, in game_render only.
- **One effects layer, three backends.** The game turns sim records into render-side effect requests, and one backend draws them:
  - **GPU** particles (Hanabi) where the adapter has compute shaders;
  - **CPU**, our own droplets, where it hasn't, or on a software adapter;
  - **Off** for headless builds, tests, CI, or by your choice.

  Hanabi does only the droplet spray. Severed parts, pools and lingering bodies are a handful of ordinary meshes per battle, drawn identically on every backend, so only the spray's density degrades (Off draws no spray at all). The backend is chosen from the adapter, and an Effects setting and a command-line flag override it. The sim, saves and replays are untouched, and tests and CI never need a GPU ([ADR 0019](adr/0019-combat-effects-and-bevy-hanabi.md)).

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
- **Blood and bodies.** Up close, a fight looks like one: blood on every blow, a burst on every kill, the dead lying where they fell, and, on a told kill, the part its line names flying off. In the mild tone it's dust and falls (section 20).
- **The kill cam.** When a captain, the champion or a legend is cut down near your camera, the game slows for a second and the camera leans in (section 20).
- **Maimed survivors.** A soldier who lives through a bad wound may come out of it One-Hand, One-Eye or One-Leg, a little weaker for good. It happens at most once in a life (section 21).

**Goals:**
- You remember people: a few legends, many odd names, one strange death a run.
- Runs differ from their first hour: quirks are dealt per person, and mishaps come from each run's own state.
- Brutal where it's fighting and deadpan everywhere, never cruel toward villagers.
- Everything told is something the sim did, and everything shown agrees with what is told: the part that flies is the one the line names.
- Battles worth watching up close: over the top in the brutal tone, plain in the mild one.
- Readable at 8x, and cheap in a 7,500-a-side battle and on a run island of 4,000 to 5,000 villagers. The effects stay inside the frame budget (NFR-10) on every backend.
- Small effects through the people who carry them, so runs stay winnable (`bot_run` wins 3 of 5).

**Non-goals:**
- Hero units, big stats, or power carried between runs.
- Flavour with nothing in the sim behind it.
- A blow-by-blow log, nemesis AI, prisoners or executions.
- Animals with names (M4b), tall tales about you (M5), weather moods (M4b).
- Gore in the simulation. Effects never touch the sim, saves or replays. No decals, ragdolls or per-limb damage model.

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
| **A, always** | The victim is a captain, the champion or a legend; or a legend makes a milestone kill (their first and fifth of a fight, and every tenth of their life); or the death is a mishap's (at most one a game day, section 11); or the victim is the first of their army to starve and a Glutton (irony 6, section 7) | Always, with one line per death, by precedence champion > legend > captain. The champion's death is told by its `ChampionFell` event, now naming the killer, and a legend's by `LegendFell` (section 9); their `Fall` is kept as told, so the feed shows the event's line and not a second one. A lethal mishap is the other way round: it pushes no `Mishap` event, and its `Fall` carries its one line (section 11). Everyone else's is a told death in `Saga` |
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

**Hunger is not a blow.** A starved captain, champion or legend gets a line, and so does the first of an army to starve when they're a Glutton (irony 6). Other starvation deaths are summed once an hour per army, leaving out anyone told by name: "Four of Garrick Hale's band starved near Ashby."

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
| `{killer}`, `{victim}` | The name. With an epithet, the given name and the epithet, as they stood at the death: a `Fall` keeps the killer's epithet then, so a later one never rewrites an old line | "Wat Ashby", "Tobin Banner-Taker", "Kael Nine-Kills" |
| `{killer_given}`, `{victim_given}` | The given name, so lines never need a pronoun | "Tobin" |
| `{weapon}` | A named weapon's name, when the killer carries one (section 12). Otherwise, armed: a weapon word fixed for life per person, so Wat Ashby always swings the same axe. Unarmed: their old trade's tool, or a word from the unarmed bank | "Vane's Bane", "an axe", "a felling axe", "a flail", "a fence post", "bare hands" |
| `{wound}` | Where the blow went: a body part worked out from the death's record by `saga::wound`, the same part the renderer shows (section 20). A severing part says so. Brutal only: mild lines never use it | "through Ulric Vane's jaw", "took Rook Vane's head off at the neck" |
| `{wound_terse}` | A severing wound in a few words, for the `terse` forms, report lines and fate clauses that must still name it (section 20). Brutal only | "head off", "arm off" |
| `{maim}` | The part a maimed survivor lost, from their record (section 21) | "a hand", "an eye", "a leg" |
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
- **Fit the feed.** A feed row is 520 px at 12 pt, about 80 characters, and the line follows a "Day N: " prefix, so a feed line gets **72 characters**. The data's longest fills (an 18-character name, a 23-character name with an epithet, 17-character weapon names, 12-character places) leave a two-name line about 14 characters of its own, so a fixed limit on every variant would leave only bare facts. The rule is settled now (not left to step 4):
  - **Feed form and long form.** A key that can reach the feed has its feed variants, and, where a key wants two sentences or more, a long form for reports, the inspector, biographies and the end screen (up to 110 characters filled). Report-only keys ("more died", deserters, moments) have only the long form.
  - **Every feed variant** fits 72 characters filled with the data's median names, epithets, weapon names and places, so most lines show as written. **Each key also has a `terse` form** per tone, facts only ("{killer} killed {victim} at {place}."), that fits 72 filled with the longest.
  - **Fit at read time.** When the chosen variant, filled with the real names, runs over 72, the Teller takes the next variant in the stepped order (see "Variety without randomness") that fits, and the `terse` form if none does. The fit depends only on records and on names as they stood at the death, so it's deterministic, the same line shows in the game, in headless and after a load, and a told line never changes after it's told.
  - **A severed wound is in every telling.** When a death's wound severs (section 20), every brutal form that tells it names it: the Teller picks only among variants carrying `{wound}`, the `terse` form has a severing twin with `{wound_terse}` that also fits 72 with the longest names, and long forms, report lines and the fate clauses of biographies and epitaphs carry `{wound}` or `{wound_terse}` too.
  - The sample lines in this doc are feed forms unless marked long; their names are short examples, and the lint checks the real fits.
- **Name first** in every line that can show at 8x (the `champion`, `legend`, `captain` and `by_legend` keys), so a glance gets who.
- **Toward militia, rebels and villagers:** grave, never gleeful and never funny, even in brutal. They die "in the lane, fork in hand". (Civilians can't be struck at all; only militia and rebels die by blows.)
- **Deadpan** (section 4): no exclamation marks, no question marks, no winking words; the lint has the list.
- **Never:** sexual violence, torture as spectacle, mutilating the dead for laughs, slurs, or children.
- **No gender.** There is no gender in the sim, so lines use names, given names, roles or "they". The lint denies the pronouns (he, she, his, her, him, hers) and gendered nouns (man, men, woman, women, son, daughter, wife, husband, brother, sister, mother, father, widow, lad, lass, boy, girl). Design prose may still say "men" for soldiers; lines may not. Family ties are "kin" or "of one household".
- **Mild is sober, not cute.** "Killed", "fell", "was cut down": a clerk's battle report, not a children's book.
- **Body parts only through `{wound}`, `{wound_terse}` and `{maim}`.** A line's own text names no body part of the victim. The lint takes the nouns from saga.ron's `wounds` table, so a line never names a part the renderer doesn't show, or the reverse (section 20). Some sample lines below name a part in their text ("jaw", "skull", "collarbone to belly", "neck", "face", "ribs", "throat", "hip to hip"). Read those words as the line's `{wound}` fill; in saga.ron they are the slot.
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
| vengeance | Tobin Kettle saw Hild fall, found Osric Pike, and settled it. | Tobin Kettle killed Osric Pike, who had killed Tobin's kin. |
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
| The mutton was bad. Four of the band spent the day in the ditch. | Food poisoning hits the band hard, and things get messy. | Concrete and physical, no innuendo, and never a sentence that reads as a death when nobody died |
| Nine battles, and Wat Nine-Kills died to a baker's peel at Loweby. | In an ironic twist of fate, Wat was defeated by a mere baker. | Never name the irony; let the facts do it |
| Cedric Tanner, a baker of Loweby, died in the lane, fork in hand. | Cedric Tanner died waving a fork, like a fool. | Villagers defending their homes are told grave |
| Vane's Bane was traded at Loweby for grain. It hangs in the storehouse. | The legendary Vane's Bane, sadly, ended up as small change. | Facts, then stop |
| (mild) Odo Venn fell in the press at Loweby and was crushed. | (mild) Odo Venn had a little accident. | Mild is sober, not coy |

**The lint** (`tests/tests/saga_words.rs`, section 3's lint plus these): no `!` and no `?` in any line; no word in all capitals; no ellipsis outside the feed's fold line; and a deny-list of winking and pitying words in both tones ("lol", "haha", "oops", "whoops", "funny", "hilarious", "ironic", "irony", "tragic", "tragically", "sadly", "unfortunately", "poor", "epic", "honk", "splat"). The lint strips `{slot}` tokens before these checks, so a line's `{irony}` slot isn't the banned word. The playtest asks whether the odd lines read funny-grim or merely silly.

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
| `joined`, `rejoined` | The tick they first enlisted (copied from `Veteran.joined` when the record starts), and how many times they came back after deserting | — |
| `kills`, `unarmed`, `fleeing` | Kills, and how many were bare-handed or of fleeing foes | — |
| `streak` | Kills in the current battle, with its number | — |
| `battles`, `held`, `fled`, `wounds` | Battles survived; battles held while their army broke or lost a quarter of its fighters; battles in which they ran; battles survived after losing at least a quarter of their health to blows | — |
| `mishaps` | How many mishaps of each kind they were a drawn **subject** of (section 11): the drunk, the sick, the slip's victim, the dice loser, the sleepwalker, the routed goose-fearer, the brawlers. An army-wide mood (a wedding, a feast, a dog at the rations) counts for nobody | One byte per kind |
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
| Avenged kin | Slew the slayer of their kin in the same battle (section 13); the deed keeps the kin's `NameId`, so a biography can say whom | 3 |
| Rescuer | One of a rescue's heroes: fought in a battle against raiders or the rival near a village whose drive-off request was then answered (section 14) | 3 |

**Epithets: earned by rule, one per person, explained.**
- **Rules** are rows in `progression.ron` (`saga.epithets`). Each row has a stable `id`, which saves and run records store, and a separate `rank`. A higher rank outranks a lower one, so ranks can be re-tuned and new epithets slotted in anywhere without relabelling old saves or records. Ids are append-only.
- **Honourable, scarred and mocking.** Each row is `honour`, `scar` or `mock`. Mocking epithets (section 7) take the lowest ranks, so a deed can wash one off ("Twice-Fled became Banner-Taker at Kemp's Ford") and none ever replaces an honourable one. The scar epithets of maimed survivors (section 21) rank just above the mocking ones and below every honourable one.
- **One per person.** A person carries one epithet, replaced only by a higher-ranked one ("Nine-Kills became Banner-Taker at Kemp's Ford").
- **Words.** Each epithet's words, in both tones, are in saga.ron, keyed by id. Where there are several variants, one is chosen per person for life.
- **Every epithet says why** in a tooltip: "Nine-Kills: nine kills, at Loweby and Crane's Ford".
- **Who.** Epithets are run-only and come to every faction except beasts.

The honourable epithets, at ranks 11 to 29. The mocking ones (ids 19 to 26, ranks 0 to 7) are in section 7, and the scar epithets of maimed survivors (ids 27 to 29, ranks 8 to 10) in section 21. The ranks were 8 to 26 before the scar epithets were slotted in; ids don't change.

| Id | Rank | Rule | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 0 | 11 | 8 battles | Old Iron | Old Iron |
| 1 | 12 | 4 kills bare-handed | Bare-Hands / Knuckles | the Wrestler |
| 2 | 13 | 2 wounds | Half-Face / the Stitched / Old Scars | the Scarred |
| 3 | 14 | Doorwarden | Doorwarden | Doorwarden |
| 4 | 15 | Ran first, and 5 kills | the Hare | the Runner |
| 5 | 16 | 9 kills | Nine-Kills | Nine-Kills |
| 6 | 17 | Beast-slayer | Wolfsbane / Boar-Sticker / Beast-Bane | Beast-Bane |
| 7 | 18 | Shield | Shieldfast / the Shield | the Shield |
| 8 | 19 | Held 3 times (`held` ≥ 3) | the Unbroken / the Wall | the Unbroken |
| 9 | 20 | Last stand | Last-Standing / the Stubborn | the Steadfast |
| 10 | 21 | 20 kills | the Butcher / Red-Hands / the Reaper | the Grim |
| 11 | 22 | Cruel, and 6 kills of fleeing foes | the Hound | the Hunter |
| 12 | 23 | Beloved, and Breadgiver | Breadgiver | Breadgiver |
| 13 | 24 | Redeemed | the Redeemed | the Redeemed |
| 14 | 25 | Banner-taker | Banner-Taker / Captainsbane | Banner-Taker |
| 15 | 26 | Avenger | Blood-Debt / the Avenger | the Avenger |
| 16 | 27 | Kinslayer | Kinslayer | Kinslayer |
| 17 | 28 | Legend-breaker | Legend-Breaker / Giantkiller | Legend-Breaker |
| 18 | 29 | Champion-slayer | the Headsman / Champion's Bane | Champion's Bane |

Every epithet is gender-neutral. "Spearwife" from the first pitch is dropped (open question 3).

## 7. Unflattering epithets and absurd deaths (SIM-53, SIM-39)

Dwarf Fortress's best names are the ones nobody wanted. A few epithets are mocking, and some deaths are absurd. Both come only from records: a mocking name needs the deeds (or the lack of them) to earn it, and an absurd death is a real death whose facts happen to be ridiculous.

**What you'll read:**
- "They call Odo Rook Twice-Fled now. Not to Odo's face." *(mild: "Odo Rook is called Twice-Fled now.")*
- "Hollis Fenwick is Turnip-Hand now: four battles, still no kills." *(mild: "Hollis Fenwick is called Turnip-Hand: four battles, no kills.")*
- "Odo Twice-Fled is Banner-Taker now. Nobody uses the old name." *(mild: "Odo Rook, once Twice-Fled, is called Banner-Taker now.")*
- "Nine battles, and Wat Nine-Kills died to a baker's peel at Loweby."
- "Aric Brook joined the band at dawn and was dead by noon."
- "Hild Crane had never killed. Then Hild killed Kael Nine-Kills."
- "Garrick Hale's band feasted after Loweby. Bram Tull choked."
- "Hild Crane, a glutton, was the first of the band to starve."

**Mocking epithets.** Appended to `saga.epithets` as ids 19 to 26, with `kind: mock`, at ranks 0 to 7, below every honourable epithet (whose ranks move up by 8, and by 3 more for the scar epithets of section 21; ids don't change). They stick until an honourable epithet replaces them. A Vain soldier who earns one takes it badly (a `mocked` mood, section 13).

| Id | Rank | Rule (from the record) | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 19 | 0 | Ran in two battles (`fled` ≥ 2) | Twice-Fled | Twice-Fled |
| 20 | 1 | A farmer once, 4 battles and no kills | Turnip-Hand / Plough-Arm | the Farmer |
| 21 | 2 | Deserted and came back twice (`rejoined` ≥ 2) | Come-Back / the Homesick | the Returner |
| 22 | 3 | Lost at dice three times (the loser is the dice mishap's subject; the winner isn't counted, section 11) | Empty-Purse / Dice-Fodder | Empty-Purse |
| 23 | 4 | A glutton, made sick by bad mutton twice (its subject both times) | Bread-Belly | the Hungry |
| 24 | 5 | Routed by geese twice (section 11) | Goose-Shy | Goose-Shy |
| 25 | 6 | The subject of three mishaps of any kind (army-wide moods don't count) | the Unlucky | the Unlucky |
| 26 | 7 | The only one of their army to survive a battle that killed at least five of it | Last-Out / the Leftover | the Survivor |

**Absurd deaths.** When a death is classified (in `casualties`' second pass, section 17), the first **irony** in the tables below that fits is stored on its `Fall`, scores +50 (section 2), picks its `absurd_*` key (a mishap's death keeps its `mishap_*` key), and reaches tier-A lines through `{irony}`. Ironies never apply to militia, rebel or villager victims: their deaths stay grave. Ids are append-only.

*Real today: the sim already produces these, and M4c only notices them.*

| Id | Irony | Rule | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 0 | Felled by a tool | The killer is militia, unarmed (their old trade's tool); the victim had 4+ battles or an epithet | Nine battles, and Wat Nine-Kills died to a baker's peel at Loweby. | Wat Nine-Kills, after nine battles, was killed by a baker of Loweby. |
| 1 | First day | The victim enlisted or was spawned (`Veteran.joined`, set in `enlist` and at spawn) less than a game day before, and has no battle in their record | Aric Brook joined the band at dawn and was dead by noon. | Aric Brook was killed on the day of joining. |
| 2 | A first kill | The killer had never killed; the victim is a legend or has an epithet | Hild Crane had never killed. Then Hild killed Kael Nine-Kills. | Kael Nine-Kills was Hild Crane's first kill. |
| 3 | Home lane | The victim enlisted from this settlement; its militia killed them | Wat Ashby came home with the warband. Ashby's militia saw to it. | Wat Ashby was killed by the militia of Ashby, Wat's own village. |
| 4 | Survived the wars | A beast killed someone of 5+ battles | Six battles, and Merrick Ellis died to the boar of the Barrow Stones. | Merrick Ellis survived six battles and was killed by a boar. |
| 5 | Named that day | Died within a game day of earning their epithet | They called Odo Rook the Wall at noon. The wall came down after dark. | Odo Rook was called the Wall for less than a day. |
| 6 | Glutton, starved first | The army's first death by hunger (`Captain::starved`, the army's run-long count, was 0) was a Glutton (section 10). The one hunger death that is classified, and told (tier A) | Hild Crane, a glutton, was the first of the band to starve. | Hild Crane, a glutton, starved first. |
| 7 | Own weapon | Killed with a named weapon they once carried (section 12) | Wat Ashby met Vane's Bane again, from the wrong end. | Wat Ashby was killed with Vane's Bane, Wat's old axe. |

*New in M4c, on purpose: the two mishaps that can kill (section 11).*

| Id | Irony | Rule | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 8 | Trodden by friends | A slip in the shield wall finished them | Odo Venn slipped in the shield wall. The wall walked on. (Long form, for the report: "Odo Venn slipped in the shield wall at Loweby and was trodden flat by friends.") | Odo Venn fell in the press at Loweby and was crushed. |
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
**Performance:** the irony checks run per classified death, only for deaths with a killer or a mishap, and for an army's first death by hunger (irony 6 only), reading the records the classification already holds.
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
- **The kill cam.** If the legend is cut down near your camera (not if they starve), the game slows for a second and the camera leans in, as for a captain or the champion (section 20).

**Grudges, a light nemesis (open question 1):**
- **Named.** The enemy who killed a legend is named in later lines and on the end screen: "Osric Pike, who slew Brenna the Stubborn".
- **Avenged.** If one of the legend's faction later kills that enemy, the death is always told and the killer earns the Avenger deed.
- **No more than that.** No AI hunts anyone, and nobody is promoted for it. FUN_DESIGN's "nemesis rivals" otherwise stay not adopted. (Feuds between two soldiers of one army are a different thing: section 13.)

## 10. Quirks (SIM-55)

In a run, every villager and soldier has one or two **quirks**: who they are, before anything happens to them. Each has a tiny, real effect on what its holder does, and the director's mishaps look for them (section 11). Quirks make two villages of the same size behave a little differently, and give every recruit something to be remembered by.

**What you'll read:**
- Inspector: "Hild Crane · a farmer of Ashby · Glutton, Fears geese"; its tooltip: "Glutton: gets hungry 15% faster".
- "Perrin Dyer fled a goose at Ashby. The goose held the field." *(mild: "Perrin Dyer ran from a goose at Ashby.")*
- "Jory Lowe, a sleepwalker, woke up in the barley, alone." *(mild: "Jory Lowe sleepwalked out of camp.")*
- "The band plundered Ashby. Tobin Kettle, pious, took it badly." *(mild: "Tobin Kettle, pious, was troubled by the plunder of Ashby.")*
- "Linnet Ware left the band near Ashby. Someone in Ashby was waiting." *(mild: "Linnet Ware left the band to stay in Ashby.")*
- Biography: "A coward, by all accounts. Ran first at Loweby and at Kemp's Ford, and outlived both."
- "Hild Crane, a glutton, was the first of the band to starve." (an absurd death, section 7)

**The deck** (ids append-only). The villager columns live in `villagers.ron` `quirks.deck`; the soldier columns and the mishap invitations live in `progression.ron` `saga.quirks`, keyed by quirk id (see Data below). The effects act only through the holder, run-only, and are exact no-ops for anyone without the quirk.

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
| 9 | Sweetheart | 6 | Joins an army ×0.8 as willingly | A follower out of battle and within 12 tiles of home: 5% a day to walk home (a desertion; captains and the champion never walk home) | — |
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
- **How the walk home happens.** `saga::settle` (progress slot 13) runs after this tick's desertions are done: `apply_desertions` is the last step of `MilitarySet::Upkeep`, and `tally` clears the `Deserters` list at the start of the next tick's, so a push there would be lost (and the list is exempt from the hash and reset on load). So slot 13 draws the walk home and sets the follower's hashed, saved `Follower.deserting` flag, as `soldier_upkeep` does for an ordinary desertion; the next tick's `tally` lists them, and `apply_desertions` sends them home like any deserter. The cause (homesick) is recorded in their `Service` at slot 13, for the words and `bot_run`'s desertions by cause.
- **Through the holder, and small.** No quirk touches another person or the faction, or goes into `FactionModifiers`. Soldier effects fold into `Veteran`'s cached nudges (section 17), clamped with traits and moods.

**Data,** split by crate, so sim_agents never holds a sim_military id and a soldier-side tweak never touches villagers.ron:
- `villagers.ron` gains `quirks: (two_chance, deck: [(id, key, weight, excludes, effects)])` (`#[serde(default)]`; an empty deck deals nothing) with the villager effects, an append-only enum: `Hunger(mult)`, `RestGain(mult)`, `Work(phase, mult)`, `FarmWork(mult)`, `Idle(phase, mult)`, `Courage(add)`, `FleeRadius(mult)`, `LoyaltyDrift(mult)`, `Willing(mult)`. `Hunger` applies to both lives: `update_needs` for villagers, and the cached nudges for soldiers (`soldier_upkeep` feeds soldiers' hunger, and `update_needs` skips the enlisted).
- `progression.ron` `saga.quirks: [(quirk, soldier: [...], invites: [(mishap, weight)])]` holds the soldier effects, keyed by the deck's quirk ids and checked against the deck when the data loads (an unknown id fails): `BreakPoint(add)`, `MoraleTarget(add)`, `FedMorale(days, add)`, `Desertion(mult)`, `PlunderMood(add, hours)`, `NearFarms(tiles, add)`, `HomePull(tiles, per_day)`, `Vanity(epithet, envy, mocked)`; the invitations name mishap kinds (section 11).
- Their names, adjectives and tooltips, in both tones, are words in saga.ron. Editing villagers.ron re-runs `village_day` and `famine` (CLAUDE.md); with no quirks outside a run, they must not move. Editing the soldier rows doesn't touch villagers.ron.

**Determinism:** the deal stream above; every effect is a rule. Nothing else draws, except the sweetheart's hourly walk home (`sim_military::saga::homesick ^ SpawnId` at the tick).
**Hashing and saving:** `Quirks` (sim_agents, up to two ids per person, two bytes) is hashed and saved, and `Villagers` and `Progression` hash their data text as before.
**Performance** (4,000 to 5,000 villagers): two bytes a person. `ai::decide`, `update_needs` and the daily `drift_loyalty` read one more optional component and skip all quirk work when it's absent; a recruit's willingness is read once per candidate in a recruit dealing, on the same draw as today. Soldiers' quirk effects are in `Veteran`'s cached nudges: `soldier_upkeep` already reads `Veteran`, and `soldiers::decide` gains one read of it, for the break-point offset (section 17). Fears geese and Sweetheart are checked hourly in `saga::settle` for their holders only (about one in eight people), against the settlement list.
**UI:** the inspector's quirk line with tooltips; the villager `DecisionTrace` shows a quirk as its own factor ("drunkard (morning) 0.9"), so `Factors` grows from four slots to five; biographies name quirks; the end screen's oddities count them.
**Tests:** `quirks_are_dealt_per_person` (the same seed deals the same quirks; dealing in any order or at spawn gives the same result; one or two each; exclusions hold; the developed world and the skirmish have none); `quirks_act_through_their_holder` (each row's effect, measured on a hand-built world, and a person without quirks bit-identical to today); `sweetheart_walks_home` (near home and out of battle at the documented rate; never far from home or in battle; a walk home drawn at slot 13 survives a save and load taken before the next tick, and the loaded game matches the original).

## 11. Mishaps (GP-30)

The pacing director (GP-16) already brings raiders when it's too quiet. In M4c it also schedules **mishaps**: small things going wrong, chosen from what is true right now. A mishap is a low-tension beat: it gives a calm stretch a story without making it dangerous. Mishaps are low stakes and rate-limited, and they never end a run.

**What you'll read** (brutal; mild in italics):
- "Six of Garrick Hale's band were drunk at noon. Raiders were near." *(Six of Garrick Hale's band were drunk before the fight.)*
- "The mutton was bad. Four of the band spent the day in the ditch." *(Bad mutton made four of Garrick Hale's band sick.)*
- "A Loweby dog took the mutton. The band gave chase. The dog won." *(A dog stole some of the band's food at Loweby.)*
- "Ashby held a wedding. Garrick Hale's band stayed for the bread." *(Garrick Hale's band stopped two hours for a wedding at Ashby.)*
- "Odo Venn slipped in the shield wall and got up with fewer teeth." *(Odo Venn fell in the press at Loweby and was hurt.)*
- "Odo Venn slipped in the shield wall. The wall walked on." *(Odo Venn fell in the press at Loweby and was crushed.)* A lethal mishap: this is its `Fall`'s line, and there is no second one.
- "Garrick Hale's band feasted after Loweby. Bram Tull choked." *(Bram Tull choked at the feast after Loweby.)*
- "Jory Lowe, a sleepwalker, woke up in the barley, alone." *(Jory Lowe sleepwalked out of camp.)*
- "Wat Ashby won Odo Rook's supper at dice." *(the same in mild.)* When the loser takes a grudge (from part 2, section 13), the variant says so: "Wat Ashby won Odo Rook's supper at dice. Odo has not forgotten."
- "Perrin Dyer fled a goose at Ashby. The goose held the field." *(Perrin Dyer ran from a goose at Ashby.)*
- "Odo Rook and Perrin Dyer fought behind the baggage. Both lost." *(Odo Rook and Perrin Dyer came to blows in camp.)*

**The kinds.** Each needs real state to happen and has a real effect; the words only decorate it. Kind ids are append-only.

| Kind | When it can happen | What it does | Can it kill |
| --- | --- | --- | --- |
| Drunk before battle | An army within `threat.radius` of a hostile army, not yet in battle, within 6 tiles of a settlement | 1 to 6 members (drunkards first): morale +0.10 now, then a `hungover` mood, −0.03 for 12 hours | No |
| Bad mutton | The baggage holds mutton | A quarter of the mutton spoils; 1 to 4 members (gluttons ×3) get a `sick` mood, −0.05 for 12 hours | No |
| A dog at the rations | Within 8 tiles of a settlement, with food in the baggage | Food worth a quarter of a day's need is lost (the words name the good) | No |
| A wedding on the road | A march (`MoveTo` or `Return`) within 6 tiles of a settlement whose goodwill toward the army's faction is 0 or more, with no hostile army within `threat.radius`, and whose goal isn't a settlement with an open request | The march halts up to 2 hours (`Captain::halt_until`); a `wedding` mood, +0.03 for 12 hours, for every member. Any new command for that captain, or a hostile army coming within `threat.radius`, ends the halt at once | No |
| A slip in the shield wall | An army in battle (a blow this hour) with 10 or more members fighting | One fighting member (cowards ×2) loses 20% of their health; if that ends it, they die | Yes |
| The victory feast | A battle the army didn't lose ended this hour, and the baggage holds 3 days or more | Half a day's food is eaten; a `feasted` mood, +0.04 for 12 hours, for every member; one chance in 20 that one member (gluttons ×4) chokes | Yes |
| Wandered off | Night, an army holding (no goal), with a sleepwalker in it | **New behaviour, a direct move:** the sleepwalker's `Position` is set to a passable tile 4 to 8 tiles away with no hostile army within `threat.radius` of it (no such tile, no mishap), their `Motion` is reset, and the rally field brings them back | No |
| Dice | An army holding, with a gambler and another member | Even odds: the loser's hunger +0.15 and the winner's −0.15 (a supper changes hands), with a `dice` mood ±0.02 for 12 hours; from part 2, one chance in 3 that the loser takes a grudge (section 13) | No |
| Goose rout | A member who fears geese within 3 tiles of a farm or pasture, marching, with no foe within `engage_radius` | **New behaviour, a timed flee:** a `geese` mood entry, −0.05 for 6 hours, whose first ticks (enough to cover 4 tiles) carry a flee point, the farm. `soldiers::decide` honours it while no foe is near, moving them away from the farm with the rout's movement; then they follow the rally field back. It isn't a rout: it marks no `routed`, counts toward no army breaking, and opens no desertion roll | No |
| A camp brawl (with section 13) | Two rivals in one army, holding | Both lose 5% of their health (never below 1); one chance in 3 (a roll from the mishap stream) that the feud ends | No |

**Subjects.** Each kind names its drawn subjects: the drunk, the sick, the slip's victim, the choker, the dice loser (never the winner), the sleepwalker, the goose-fearer and both brawlers. Only they count the mishap in `Service.mishaps` (section 6). A dog at the rations has no subject, and a wedding's or a feast's mood for every member makes nobody a subject.

**When.** In `director::step` (slot 12 of the progress driver, hourly, run only), after the raider check, the **mishap clock**:
1. Nothing in the run's first 24 hours (`free_hours`), so a start stays as it was drawn; nothing while a run end is pending.
2. Once `spacing_hours` have passed since the last mishap and today's count is under `max_per_day`, each hour has a `chance_per_hour` chance of one.
3. Most kinds need tension below `tension_below` (a calm hour). The two battle kinds (drunk before battle and a slip in the shield wall) ignore tension instead, and are capped at one per battle.
4. The candidates are every (kind, army) whose condition holds now. **The side is drawn first:** the player's side with `player_share` (2 in 3), else the side at war with the player, so a third of mishaps fall on raiders and the rival however many armies each side has; if only one side has a candidate, it's that side. Then one candidate within the side is drawn, weighted by the kind's weight and by the quirk holders in that army. Beasts, monument guards and bands for hire are never subjects.
5. Its subjects and rolls are drawn; it applies; the clock resets.

| Personality | `spacing_hours` | `chance_per_hour` | `max_per_day` | `tension_below` | About |
| --- | --- | --- | --- | --- | --- |
| Steady | 16 | 0.3 | 2 | 0.5 | About one a game day (15 to 30 in a bot run, which lasts 15 to 27 days) |
| Chaotic | 10 | 0.3 | 3 | 1.0 | One and a half a day, bunched |
| Harsh | 20 | 0.3 | 1 | 0.25 | One a day at most |

**Never run-ending.** These are rules in code, each with a test:
- **Never the important people.** The champion, any captain and any legend are never a mishap's subject. Their bands may be.
- **Deaths.** Only the slip and the feast can kill: one ordinary soldier at most, only in an army of 10 or more (`min_army`), and at most one mishap death a game day across the island (`max_deaths_per_day`).
- **Food.** A food mishap never takes the baggage below one day of the army's need (`food_floor_days`), and never touches a settlement's stores.
- **Delays.** Only marches, only with no hostile army near and no open request at the march's goal, at most 2 hours, and at most once a day per army. Any new command for that captain (`apply_commands`), or a hostile army coming within `threat.radius` (`run_orders`), clears `halt_until` at once: a wedding never overrides your order and never holds an army while a fight comes to it.
- **No strategic change.** A mishap never changes an owner, goodwill, loyalty, an order, a posture or an allegiance; never starts a battle; never spawns anything.
- **One a day per army.** An army that had a mishap today is not a candidate again until tomorrow.

**Pacing.** A mishap is the new `SimEventKind::Mishap` (`subject` the army's captain, `other` the person or 0, `at` the tile, `amount` the kind). It's chronicled, for biographies and the end screen, and **never counts as a notable event**: it doesn't reset the quiet clock, count as the run's first event, stop fast-forward or pause the game. Mishaps fill the quiet; they're never the reason a raid is delayed. They do change the state the director reads (a halted march moves where the champion is, a sick band fights a little worse), so a run with mishaps unfolds differently from one without, as any change to a run does; the guarantee is about notability, not about an identical schedule.

**A mishap's death is told once.** A lethal slip or choke sets health to 0 and notes its manner in `Saga`; `find_fallen` finds the dead on the next tick, like a soldier who starved (G4), and `casualties` tells a tier-A `Fall` on the kind's `mishap_*` key (irony 8 or 9). That `Fall` is its only line, in the feed and everywhere else: a lethal mishap pushes no `Mishap` event, so it shows once and counts once against the feed's limits. The end screen counts mishaps by kind from both (events and mishap `Fall`s), and `Director.mishaps` counts every one.

**Data:** `director.ron` gains `mishaps: (spacing_hours, chance_per_hour, max_per_day, tension_below)` in each personality, and top-level `mishap_rules: (free_hours, player_share, min_army, max_deaths_per_day, food_floor_days)` and `mishap_kinds: [...]` with each kind's weight and numbers (the table above). All are hashed, as the director's RON text. The lines are words in saga.ron, a lethal and a non-lethal bank where a kind can kill.
**Determinism:** `sim_military::director::mishap` at the hour's tick draws whether a mishap comes, its side, its kind and its army; `sim_military::mishap ^ n` (n = the run's mishap count) at the same tick draws its subjects, counts and rolls (the choke, the dice, the dice grudge, the brawl's peace, the wandering tile). Both are new keys; no existing stream moves.
**Hashing and saving:** `Director` gains `next_mishap`, the day's count and the day, `mishaps` (the run's count) and `deaths_day`; `Captain` gains `halt_until`; moods are `Mood` entries (section 13). All hashed and saved.
**Performance:** hourly, a pass over the armies (tens) and one army's members; per tick, only `run_orders` checking `halt_until` per captain, and the cached nudges.
**UI:** feed lines, dimmer than battle lines. At 1x–4x, all of yours and those of armies near yours; at 8x and fast-forward, only a mishap that befell your champion's army (one that killed one of yours is a tier-A `Fall`, shown by the death filter); at every speed, at most one mishap line every 20 real seconds, and at 8x the hard cap of section 15. The inspector shows the moods a mishap left ("sick: bad mutton, 9 hours"); the end screen's oddities count them.
**Tests:**
- `each_mishap_does_what_it_says`: each kind on a hand-built world, with its condition false and then true. Also: a lethal slip leaves one `Fall` and no `Mishap` event; a new command, or a hostile army arriving, ends a wedding's halt; a march to a settlement with an open request gets no wedding; the goose rout's flee ends when its ticks run out or a foe comes near, and marks no `routed`; a sleepwalker never lands within `threat.radius` of a hostile army; only the drawn subjects count the mishap.
- `mishaps_never_wipe_a_run`: on all three personalities (steady, chaotic and harsh), with compressed data overrides (`free_hours` 0, `spacing_hours` 1, `chance_per_hour` 1.0) so it fits the 60 s test budget (G34), and again over every `bot_run` seed (steady): no mishap touched a champion, captain or legend, at most one death per mishap and one a day, no baggage under the floor, no delay with a foe near, at most `max_per_day`, none in the first `free_hours`.
- `mishaps_are_not_notable`: a unit test that a chronicled `Mishap` leaves `Chronicle::last_notable`, `Director.first_event` and `eventful()` unchanged; and a scenario that runs the mishap clock with every kind's effect stubbed out (rolled and recorded, never applied), whose quiet clock and raider spawns match the same run with no mishap kinds.

## 12. Named weapons (SIM-56)

A weapon that kills someone who matters gets a name, and from then on a history: whose hands it was in, what it killed, and where it went when they died.

**What you'll read:**
- "Jory Lowe's axe killed captain Rook Vane. It is Vane's Bane now." *(mild: "Jory Lowe's axe is called Vane's Bane since Kemp's Ford.")* Long form, for the report and the inspector: "Jory Lowe's axe went through captain Rook Vane at Kemp's Ford. The band calls it Vane's Bane now."
- "Vane's Bane passed to Hild Lowe, of Jory's household." *(mild: same)*
- "Vane's Bane went into the baggage. Nobody wanted it yet."
- "A raider took Vane's Bane off Jory Lowe's body." *(mild: "A raider took Vane's Bane from the field.")*
- "Vane's Bane was traded at Loweby for grain, and hangs there now." Long form: "Vane's Bane was traded at Loweby for grain. It hangs in the storehouse."
- "Vane's Bane lies in the furrows near Loweby. Nobody went back." *(mild: "Vane's Bane was lost near Loweby.")*
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
**Hashing and saving:** `NamedWeapon` is hashed and saved (its index is stable, since `Saga.weapons` only grows). A weapon's record changes in place (its place, holders and kills), so `Saga` hashes the weapons in full, not through its append-only digest (section 17); there are at most 24.
**Performance:** one check on a tier-A kill by an armed killer; one lookup when a holder dies; nothing per blow beyond the existing kill push.
**UI:** the inspector's weapon line ("Carries Vane's Bane, an axe; named at Kemp's Ford; seven kills"), clicking it shows the history; the settlement panel lists relics in its storehouse; kill lines name it in `{weapon}`; the end screen names the run's most storied weapon and its fate. The feed tells namings of your weapons, and losses and sales of them, at 1x–4x; at 8x, only a legend's weapon.
**Tests:** `a_weapon_is_named_by_a_notable_kill` (and not by an ordinary one, nor past the caps); `named_weapons_change_hands` (looted by a soldier and by militia, passed to kin, into the baggage and out first, sold last, taken home and back, lost when the army is wiped, and always in exactly one place).

## 13. Kin, friends, rivals and moods (SIM-57)

Soldiers aren't strangers to each other. Some came from the same household; some held a line side by side; some can't stand each other. These bonds, and what happens to them, give soldiers moods: small, bounded nudges with causes the inspector can name.

**What you'll read:**
- "Tobin Kettle saw Hild fall, found Osric Pike, and settled it." *(mild: "Tobin Kettle killed Osric Pike, who had killed Tobin's kin.")*
- "Jory Lowe and Bram Tull held at Kemp's Ford. Shield-friends since." *(mild: "Jory Lowe and Bram Tull are friends since Kemp's Ford.")*
- "Odo Rook has not forgiven Perrin Dyer for running at Loweby." *(mild: "Odo Rook blames Perrin Dyer for running at Loweby.")*
- "Odo Rook and Perrin Dyer both held at Crane's Ford. The feud is off." *(mild: "Odo Rook and Perrin Dyer made peace after Crane's Ford.")*
- "Perrin Dyer died at Ashby. Odo Rook did not attend." *(mild: "Perrin Dyer died at Ashby; Odo Rook's feud is over.")*
- "Hollis Crane, Hild Crane's kin, died at Ashby by the warband's hand." *(mild: "Hollis Crane, kin of Hild Crane, was killed at Ashby by the warband.")* Grave in both tones: the dead is a villager. Long form: "The warband killed Hollis Crane in Ashby's lane. Hollis was Hild Crane's kin. Hild is still marching."
- Inspector mood note: "Wat is upset: Hild Ashby, kin, died at Loweby, and the mutton was bad."

**Bonds.**
- **Kin** are one household: people with the same `Villager.home`. Population builds households, and `Home.residents` lists each one, enlisted members included (they keep their home until they die). Kin are never stored; they're looked up through `Home.residents` (a handful of people). Spawned soldiers (`Soldier`) have no household and no kin. Kin on opposite sides are the existing `blood` key.
- **Friends** (shield-friends) form at `end_battles`: in an army that broke or lost a quarter of its fighters, two survivors who both held and ended the battle within 2 tiles of each other. In SpawnId order, each such survivor pairs with the nearest one not yet their friend: at most one new friend a battle, and two in all.
- **Rivals** form from:
  - **dice:** the loser, one chance in 3 (section 11; the dice mishap arrives in part 1 without it, and the grudge joins it here);
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

**Moods.** A sparse `Mood` on enlisted people, introduced with mishaps (section 11) and filled out here: at most four timed entries (cause, until, morale-target change, desertion multiplier, about whom and where). A fifth replaces the one ending soonest. **About whom is plain data,** never an `Entity`: the other's SpawnId and `NameId`, the place and the tick. Grief is about someone dead, and the dead are despawned in `casualties`, so an `Entity` would name nobody (and would save as 0 and load as a placeholder); a `NameId` still names them. The sum is clamped: morale target ±0.08, desertion ×0.5 to ×2. Whenever an entry is added or expires (hourly, in `saga::settle`), the sums are written into `Veteran`'s cached nudges, so the parallel loops read nothing new from `Mood` but two entries: vengeance (its aim, resolving the killer's SpawnId through the fighter grid) and the goose rout's flee (section 11), which `soldiers::decide` reads directly, for their few holders.

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
**Determinism:** rules over records, in serial steps (casualties, `end_battles`, `settle`), in SpawnId order. The only draws are the dice grudge and the brawl's peace (both from the mishap's stream, `sim_military::mishap ^ n`) and the sweetheart (section 10).
**Hashing and saving:** `Bonds`, `Mood` and `Veteran`'s cached nudges are hashed and saved; an entry's `about` is plain data (SpawnId, `NameId`, place, tick), saved as written.
**Performance:** kin lookups happen only when a villager-recruit dies (spawned soldiers have no household, so `--battle` pays nothing); friends form hourly, at `end_battles`, with the fighter grid; moods expire hourly over the sparse set; the hot loops read only the cached nudges and, for the few with one, the vengeance entry.
**UI:** the inspector's "Kin in the band: Hild Ashby", "Shield-friend: Bram Tull", "Feud: Perrin Dyer (since Loweby)", and the mood note; the captain panel's "Mourning: 5 · Feuds: 2". In the feed: vengeance kills are told deaths (section 2); `kin_killed` lines for your soldiers at 1x–4x; feud and friendship lines at 1x–2x only; none of them at 8x.
**Tests:** `kin_grieve_and_avenge` (grief, the vengeance aim, the deed, and `kin_killed` when your own army kills a recruit's household); `friends_and_rivals_form_by_rule` (each source, the caps, and each way a feud ends); `moods_are_bounded_and_expire` (the clamp, replacement, expiry, and the cached nudges matching the entries); `moods_are_exact_no_ops` (no bonds and no moods: bit-identical morale and desertion).

## 14. Rescues and hero memorials

**Rescues (M4c).** A `DriveOff` request completes when the hostiles have been gone `clear_hours` and any player captain stands within `reward_radius` (`requests.rs`), and a director's band withdraws on its own after `band_linger_hours`. So an answered request doesn't mean anyone fought, and a rescue must be earned in a fight. When a village's `DriveOff` request is answered (`requests::complete`), M4c records a rescue **only if a real fight drove the raiders off:**
- **The battle.** At least one battle since the request was posted in which the answering captain's army fought an army at war with the player (raiders or the rival), with its first blow within army.ron's `threat.radius` (45 tiles) of the settlement's centre. The first blow's position is kept on the battle (section 2), so a fight whose nearest settlement is a neighbour still counts if it's within that radius.
- **The rescuers** are the members of the answering captain's army who fought in such a battle, read from `Service.fights`.
- **The heroes** are up to three of them: legends first, then those with an epithet by fame, then that battle's best killer. If none of them is named, the answering captain is the hero of record (their army fought).
- **No fight, no rescue.** Without such a battle, nothing is recorded: the `RequestDone` line keeps its built words, nobody earns a deed, and M4b raises no memorial. Words never claim a rescue that didn't happen.
- **The deed.** Each hero earns the Rescuer deed (section 6).
- **The record.** `Saga.rescues` keeps the settlement, the tick, the battle, the faction, the heroes (`NameId`s and SpawnIds) and the request id.
- **The words.** The `RequestDone` line names the hero: "Wat Nine-Kills saved Ashford from the raiders." The settlement's history keeps it: "Saved from the raiders on day 6 by Wat Nine-Kills. Ashford remembers."
- **No new effect in M4c.** The request's reward and goodwill are as built.
- **Test:** `rescues_are_recorded` (a legend named first, then the named by fame, then the best killer; the captain when nobody who fought is named; a battle just inside `threat.radius` but nearest a neighbour counts; and a request answered after the band withdrew without a fight records nothing).

**Memorials in M4c are render-only:**
- a **cairn** where a legend fell, and a **memorial stone** at their home village's centre, if they had one;
- hovering either shows the epitaph;
- both are drawn by the renderer from the sim's records; there's no sim entity;
- from M4b, when a legend's home village has also raised that legend a hero memorial, the memorial replaces the render-only stone there, so one centre never shows two markers for one person.

A rescue leaves no mark on the map in M4c.

**Hero memorials (M4b).** From M4b, a village that remembers a rescue raises a **memorial** to its hero once it can build one: a statue, cairn or shrine in its town style, named for the hero. It's a real building with small, local effects: goodwill and loyalty there, recruits a little more willing, and the hero's own army steadier near it. If the hero dies, the memorial becomes a **grave the town tends**, with a chronicle line ("Ashford tends Wat Nine-Kills's grave now"); the grave keeps the goodwill, loyalty and recruit effects, and only the army effect ends. Its plot is reserved in every master plan from the seed, and opens only for a settlement with a rescue record, so plans stay a pure function of the seed, site and style. Across runs, the hero and the memorial appear only in M6's legends book, as history. The design is in `docs/TOWNS_DESIGN.md` ("Hero memorials"); M4c's part is the rescue record it reads.

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
| **Feed** | Told deaths (read from `Saga`), `Starved`, `Named`, `TraitEarned`, `LegendRose`, `LegendFell` and `Mishap` lines, and saga notes (weapons, kin, feuds, maims), coloured by kind (your dead, your kills, legends in gold, mishaps dimmer); the speed filter and fold (section 2) and the tighter limits for the rest (below); clicking a line selects the person if alive, or else moves the camera there; a Brutal/Mild switch |
| **Inspector** | The epithet after the name; quirks; "Kills 14 · Battles 6 · Wounds 2"; what they lost ("Lost a hand at Loweby, day 4", section 21); deeds in words; earned traits with what earned them; the weapon carried and its history; kin in the band, friends and a feud; the mood note; a legend's "Steadies those nearby". The biography replaces "Memories: none yet" for anyone with a record (other villagers keep it until M5's memories, and show their quirks) |
| **Captain panel** | "Notables": the army's legends and up to three other named men; clicking one selects them. "Mourning: N · Feuds: N" |
| **Settlement panel** | The settlement's short history, its rescues and the relics in its storehouse |
| **Battle reports** | The feed's battle line opens the whole report with its moments; the end screen uses the same words |
| **End screen** | "Legends of the run": up to six, yours first, each with their numbers and fate in one line and the epitaph on hover. "Foes remembered": enemy legends, and whoever killed one of your legends or your champion. "The Fallen": how many of your people died, with the named ones listed. "Oddities of the run": the strangest death (the highest-scoring irony), the most storied weapon and its fate, the run's mishaps by kind, and its longest feud. The bloodiest battle, by name. The timeline gains legends risen and fallen |
| **Map** | A small pennant over each living legend inside the detail band; cairns and memorial stones; blood, bodies and severed parts, and the kill cam (section 20); maimed figures: an eye patch, a bound stump, a limp (section 21) |
| **New-game screen** | "Words: Brutal / Mild" beside the difficulty buttons, with "Effects: Full / Reduced / Off" and "Kill cam: On / Off" (section 20). The same three switches sit in a small view menu by the time bar during play. M6's options screen takes them over later |

**Readable at 8x, for the new lines too.** Mishap, weapon, kin and feud lines are lower priority than told deaths and events, and the throttle drops them first. At 8x and fast-forward they pass only as sections 11 to 13 say (a mishap that hit your champion's army; a legend's weapon), and then through a **hard cap in the feed: at most 2 such lines a game day** (about 30 real seconds at 8x), whatever the difficulty or the run. The rest fold into the day's "…and 3 more" line. It's a throttle, not a statistic, so chaotic's three mishaps a day can't break it; `bot_run` still counts what the filter passes, as a check. At every speed, at most one mishap line every 20 real seconds.

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
Input      orders::apply_commands   + any new command for a captain clears its Captain::halt_until
           orders::run_orders       plunder and food given leave cruelty and kindness marks on the captain;
                                    a halted march waits out Captain::halt_until, cleared when a hostile army
                                    comes within threat.radius; named weapons go out first in arming and last
                                    in a barter
Knowledge  index::build_index       + who ran first and when each army broke (broke_share);
                                    LegendIndex (each faction's enlisted legends; run only)
Decide     sim_agents::ai::decide   quirk factors (run only; a fifth Factors slot shows them)
           soldiers::decide         + reads Veteran: personal break point (cached nudges: quirks, traits, moods,
                                    a legend nearby); reads Mood for its few holders: the vengeance aim and the
                                    goose rout's timed flee; Cruel aim (run only)
Act        combat::resolve_strikes  clears the tick's kills, blows and maim checks first; a landed blow pushes (victim,
                                    striker) onto Casualties.struck; a kill pushes (victim, killer, credited side, one
                                    blow); a blow adds to the victim's hurt this battle, and one that takes a living
                                    person past half their health this battle notes a maim check; the battle
                                    bookkeeping joins or allocates battle numbers
           combat::casualties       (only when someone fell) pass 1, killers in strike order: snapshot each killer, start
                                    or update their record, streaks, best killer, weapon naming
                                    pass 2, the dead in SpawnId order: classify (manner, irony); tell tier A, then tier B
                                    against the bar and the caps (Fall, with its position, the victim's role and maim,
                                    and the killer's epithet as it stood / LegendFell / ChampionFell); keep named deaths;
                                    a named weapon's fate; kin and friend moods, vengeance; a legend's shock;
                                    then today's work (morale entry, goodwill, succession, despawn)
           combat::maims (new)      (only when a maim check was noted) the checks of the living in SpawnId order: roll,
                                    record, note (section 21); runs on ticks with no death too
Needs      sim_agents::update_needs quirk hunger and rest rates (run only)
           upkeep::soldier_upkeep   marks "ran this battle"; legend aura; cached nudges to break point, morale target,
                                    desertion and hunger
           progress slot 1          end_battles: wounds, held and broke marks, deeds, the report's moments, judging
                                    survivors; friends and rivals form; feuds end
           progress slot 6          requests::step: an answered DriveOff records its rescue
           progress slot 12         director::step: + the mishap clock and the mishaps (run only)
           progress slot 13 (new)   saga::settle: judge every record changed since the last hour; the hourly starvation
                                    lines; expire moods and rewrite cached nudges; Fears geese and Sweetheart checks
                                    (a walk home sets Follower.deserting, taken by the next tick's tally and
                                    apply_desertions); reconcile named weapons' places
Words      chronicle::Teller        kill lines, moments, mishaps, notes, biographies; the dead named from the saga's
                                    records; a tone parameter; told_wound, the part a told death's line names
Render     FixedLast effects::collect   (game_render, after every tick that advanced SimTick) copies this tick's
                                    blows, kills, falls and maims into a render-side buffer; Update turns them into
                                    effect requests (section 20)
```

**The sim stores no text** (ADR 0017):
- Records hold SpawnIds, `NameId`s, ids for epithets, deeds, traits, quirks, ironies, mishap kinds and mood causes, and counts.
- Epithet strings and weapon names never go into the `NameTable`, which is hashed and saved.
- The dead are named from their kept `Fall`s, copied before they are despawned. Anyone ever named (section 2) has their death kept, so a named person is never told as "a soldier".

**Told deaths and notes are not events.** They live in `Saga`, append-only, and the feed reads them with its own cursor. So a big battle can't push `OfferPosted` or `LegendFell` out of the 256-entry `EventFeed` ring during a frame hitch, and the auto-pause is never lost. `Saga` keeps every told death of the run, so the feed never misses one either. The same holds for **saga notes** (`Saga.notes`: weapon namings and fates, friendships, feuds begun and ended, kin killed), which are personal history, not events. Mishaps are events (`SimEventKind::Mishap`) because they're the director's beats, and they're not notable.

**New state:**

| Type | What | Hash | Save |
| --- | --- | --- | --- |
| `Casualties.kills`, `struck`, `maim_checks`, `maims` | This tick's kills (replaces `slain_by`); its landed blows as (victim, striker), a reused list whose length is the blow count; the maim checks the strike loop noted; and the maims recorded this tick, with where. All are cleared at the top of `resolve_strikes`. With `fallen` (which gains the victim's `Entity` and SpawnId), they're what the renderer's collector reads (section 20) | Exempt scratch, as today | No |
| `Veteran` gains `joined`, `routed`, `hurt`, `trade`, and the holder's cached nudges | The tick they enlisted or were spawned (set in `enlist` and at spawn; reset on re-enlisting like the rest); ran this battle; share of health lost to blows this battle; trade before enlisting; break-point offset, morale-target offset, desertion multiplier and hunger multiplier, and the maim's attack, guard and pace multipliers (section 21; at most one of them below 1.0), written only by serial steps (the quirk deal, `judge`, mood changes, rebuilt from `Service`, `Quirks` and `Mood` on enlisting) so the parallel loops never read the sparse components. The nudges are **fixed-point** (`i8` hundredths for the offsets, `i16` thousandths for the multipliers), so `Veteran` keeps `Eq` and stays small | Yes | Yes |
| `Service` (new, sparse) | The service record (section 6), with the maim, if any: the part, the tick and the battle (section 21) | Yes | Yes |
| `Legend` (new) | `{ faction, rose }` | Yes | Yes |
| `Quirks` (new, sim_agents) | One or two quirk ids per person, in a run | Yes | Yes |
| `Mood` (new, sparse) | Up to four timed entries (section 13) | Yes | Yes |
| `Bonds` (new, sparse) | Up to three friend or rival entries | Yes | Yes |
| `NamedWeapon` (new, sparse) | Index into `Saga.weapons` | Yes | Yes |
| `Saga` (new resource) | Told and kept deaths (`Fall`, append-only, with manner, irony, the position where they fell, the victim's role (captain, champion, legend), the part the victim had lost, if any, and the killer's epithet as it stood), battles (number, first blow and its position, place, ordinal, terrain, lines told, first blood), per-settlement battle counts, legends risen, grudges, weapons, rescues, notes, pending mishap deaths, untold count, `next_battle` | **Two parts.** The append-only parts (`Fall`s, notes, rescues, closed battles) as lengths and a running digest, like the `Chronicle`: each is folded in when pushed, and a battle when it closes. The parts changed in place (open battles with their lines told and first blood, named weapons with their place, holders and kills, per-settlement battle counts, grudges, pending mishap deaths, the untold count, `next_battle`) are hashed in full, which stays small: the battles open now and at most 24 weapons. A divergence in either part reaches the checkpoint hash | Yes |
| `Captain` gains `battle`, `battle_broke`, `battle_best`, `battle_ran_first`, `battle_deserted`, `starved`, `starved_told`, `halt_until` | The battle number, this army's moments, its run-long count of hunger deaths and how many of them the hourly summary has told (irony 6 reads the first), and a wedding's halt | Yes | Yes |
| `Director` gains `next_mishap`, `mishap_day`, `mishaps_today`, `mishaps`, `deaths_day` | The mishap clock | Yes | Yes |
| `BattleReport` gains moments | Section 5 | In the chronicle's digest | Yes |
| `LegendIndex` | Rebuilt in Knowledge before any reader | Exempt | No |
| `SagaWords` | saga.ron | Exempt ("words only; the sim never reads it") | No |
| Tone, feed cursors and throttle state | game_ui | Outside the sim | Tone in the preferences file |
| Effect requests, bodies, pools, parts, droplets, the effect clock, the kill cam | game_render and game_ui (section 20) | Outside the sim | No; the Effects and Kill cam settings in the preferences file |

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
- **Draws (amended).** The first draft said every rule is a threshold and no new RNG stream is needed. That still holds for kills, records, deeds, epithets, ironies, traits, legends, weapons and moods, and for every word. Bond formation is rule-based too, except for two mishap rolls: the dice grudge and the brawl's peace. Your follow-up adds draws where chance is the point, each from its own new key, so no existing stream moves:

| Stream key | Seeded at | Draws |
| --- | --- | --- |
| `sim_agents::quirks ^ SpawnId` | tick 0 | A person's quirks: one or two, which, by weight |
| `sim_military::director::mishap` | The hour's tick | Whether a mishap comes, its side, its kind and its army |
| `sim_military::mishap ^ n` (n = the run's mishap count) | The hour's tick | Its subjects, counts and rolls: the choke, the dice, the dice grudge, the brawl's peace, the wandering tile |
| `sim_military::saga::homesick ^ SpawnId` | The hour's tick | A sweetheart's walk home (it sets `Follower.deserting`) |
| `sim_military::saga::maim ^ SpawnId` | The tick of the blow | Whether a survivor past half their health is maimed, and which part (section 21) |

The renderer's own variety (droplet spread, a part's arc) never draws from these: it comes from stable hashes of record ids, like `game_render::art` (section 20).

**The run gate.** Records and lines exist in every preset. Behaviour changes only in a run:

| | Every preset | Run only (`in_run`) |
| --- | --- | --- |
| Killers, battle numbers, records, told deaths, `Starved` lines, battle moments | Yes: hashes change, stdout doesn't | |
| A rebel's kill credited to the rebels | Yes, wherever rebels fight; the guarded developed, skirmish and battle commands have none | |
| Epithets (honourable and mocking), ironies, deeds that need a run (Breadgiver, Plunderer, Rescuer), earned traits and their effects, legends, aura, shock, the report of an army wiped out | | Yes |
| Quirks, mishaps, named weapons, bonds and moods, rescue records | | Yes |
| Maims recorded, with their notes and visuals (section 21) | Yes: hashes change, stdout doesn't | |
| A maim's attack, guard and pace penalties, and the scar epithets | | Yes |
| Combat effects and the kill cam (section 20): render and UI only | Wherever the game draws a battle | |

Developed, skirmish and `--battle` stdout stay byte-identical. Kill lines appear in the skirmish too, which helps test the words. `villagers.ron` and `director.ron` gain sections, so the developed world's state hash changes (`Villagers` and `DirectorData` hash their text) while its behaviour doesn't: `village_day` and `famine` are re-run to show it.

**Performance (NFR-09).**
- **Measure live ticks only.** A run that has ended freezes: its ticks run only Input, Needs and Bookkeeping, at about 0.03 ms. `--run --seed 7 --difficulty steady --ticks 24000` has no bot, so its run ends early (lost at tick 8,969 on day 3.7, on the M4 build), and about 63% of its ticks are frozen: its whole-run mean (0.54 ms) mostly measures when the run ended, which M4c moves by design. So the run measurements use only ticks while the run is going.
- **Baselines.** Recorded three times at step 1, with their spread, in `docs/m4c/`:
  - M4's numbers for the 7,500-a-side battle (seed 42: deaths a tick, the clash window's mean tick, and the Act, Decide and Needs stage times);
  - **the run island's live window:** `--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400`, the first three game days, which end before the run does (no `run ended` line). Its mean tick and Decide and Needs stage times, where 4,000 to 5,000 villagers live;
  - **a bot run's live windows,** the M4 convention (`docs/m4/PROGRESS_BOT.md`): `--run --bot --seed 42 --ticks 96000 --timings --stats-every 250`, averaging only the windows before the win, with the stage times.
- **Per blow:** one byte added to the victim's battle hurt; a push of two entities onto `Casualties.struck`; and a push onto a vector on a kill, or when a survivor's hurt first passes half their health in a battle (a maim check, section 21). No query, and no allocation once warm: the scratch lists are cleared, never dropped, so they keep their capacity.
- **Per death:** a few microseconds of reads and one record; a weapon lookup; a household lookup for villager-recruits only.
- **Per soldier, in a run:** at most three distance checks for the aura, and none when the faction has no legend. Traits, quirks and moods are read from `Veteran`'s cached nudges: `soldier_upkeep` already reads `Veteran`, and `soldiers::decide` gains one read of it (the break-point offset). The developed stress world has none of them and pays nothing beyond that read.
- **Per villager, in a run:** one optional two-byte component read in `decide` and `update_needs`.
- **Hourly:** mishaps (tens of armies), mood expiry and quirk place checks (sparse sets), weapon reconciliation (tens of weapons).
- **Measurements.**
  - `--battle 7500 --timings` covers every preset's bookkeeping.
  - `--battle 7500 --saga-bench --timings` turns on the saga's run-only effects (not the rest of the run rules) and gives each army three legends, every soldier a quirk, every tenth soldier an earned trait and a friend, every fifth a mood, and three named weapons, so the per-soldier costs are paid.
  - `--run --seed 7 --difficulty steady --ticks 7200 --timings --stats-every 2400` covers quirks on the villagers, and the hourly steps from the second day (mishaps start after `free_hours`). The step notes confirm no `run ended` line before tick 7,200; if a step's run ends sooner, the window stops at the last full window before the end, and the notes say so.
  - `--run --bot --seed 42 --ticks 96000 --timings --stats-every 250`, the windows before the win, covers armies, battles, mishaps and the saga's records in a played run.
- **Target.**
  - For the two battles and the run island's live window, the median of three runs of the mean tick (the clash window's, for the battles) rises at most 2%. If three runs of one build spread more than 2%, the gate compares the Act, Decide and Needs stage times instead.
  - Quirks and mishaps change a played run's path by design (other fights, other armies), so a bot run's mean tick compares two different games. Its gate is **the saga's own spans** (the quirk deal, `saga::settle`, the mishap clock, and the saga parts of `casualties` and `end_battles`, each in its own tracing span) at most 2% of the live mean tick; the whole-tick mean and stage times are recorded beside it, not gated.

**Data:**
- `progression.ron` gains a `saga` section (`#[serde(default)]`, like `gather` and `hire`), holding:
  - the battle terms (`broke_share`);
  - the notability weights, bar and caps;
  - deed fame;
  - epithet rows (id, kind, rank, rule) and the irony rules' numbers (`ironies`);
  - earned-trait triggers and effects;
  - the legend rule, aura and shock;
  - `quirks`: the soldier effects and mishap invitations, keyed by the deck's quirk ids (section 10);
  - `weapons`, `bonds` and `moods` (sections 12 and 13).
- `progression.ron`'s `traits` gains Cruel (id 7), and `chronicle.causes` gains the phrase for `Cause::Earned`.
- `villagers.ron` gains `quirks`: the deck and the villager effects (section 10).
- `director.ron` gains `LegendFell` in `auto_pause`, `mishaps` in each personality, and `mishap_rules` and `mishap_kinds` (section 11).
- The new `saga.ron` holds the words, two tones of the same shape, so a key missing from one tone fails to parse. It also holds each key's fact and flavour slots, its feed variants, `terse` form and long form (section 3), quirk and mishap words, weapon-name patterns, mood words, and the feed throttle (with the 8x cap on saga lines), which is viewer data like `render.ron`.
- Every field is documented in `docs/dev/DATA.md` (the docs test enforces it).

## 18. Traps and guards

| Trap | Guard |
| --- | --- |
| Log spam in big battles | Tier A narrowed to captains, the champion and legends (and a legend's milestone kills, and the rare mishap death); the rising bar; at most 10 tier-B lines a battle and 8 told deaths a tick; summaries; the feed's speed filter, rate limit and fold; `bot_run` checks at most 8 lines a battle pass the 8x filter |
| Unreadable at 8x | Only tier A about you or a legend at 8x and fast-forward, rate-limited; mishap, weapon, kin and feud lines are dropped first, with a hard cap of 2 a game day at 8x in the feed (on every difficulty; `bot_run` also counts them), and one mishap line every 20 real seconds at any speed; feed lines fit 72 characters (variants fit with median names, a `terse` form with the longest, chosen at read time), the name first |
| One death told twice | One line per death: `ChampionFell` and `LegendFell` carry theirs and their `Fall` isn't shown; a lethal mishap pushes no `Mishap` event, and its `Fall` carries the line; the hourly hunger sum leaves out anyone told by name |
| A rescue nobody fought for | A rescue is recorded only when the answering army fought raiders or the rival within `threat.radius` of the village; heroes only from those who fought; otherwise nothing, and no memorial in M4b |
| Repetition kills brutality | Composition, weapons fixed per person, the prime-step variant choice, a lint test of bank sizes, and a brutal flourish rationed to the top moments of a report |
| Brutality turns tasteless, especially toward villagers | Militia and rebel victims always take a grave key and never an irony; written content rules; a deny-list test; a playtest question |
| Deadpan tips into silly | The voice rules (section 4), with do and don't examples in DATA.md; the lint bans `!`, `?`, capitals and winking words; a playtest question |
| Mild reads as comic | A "clerk's report" register; a playtest question |
| Words claim things that didn't happen | "Invent the manner, never the facts": every fact comes from a record, times only from `{daypart}` and `{then}`, trades only from the sim's occupations; flavour only on a real effect; ironies only from section 7's table, and the "not in M4c" ends have no key |
| Quirks are noise | Each has a real effect, listed with its numbers and tested; the geese are the one flavoured trigger, on a real farm; Hates rain waits for weather |
| Quirks shift balance or a village's food | Tiny effects, through the holder, run-only; `bot_run` after the quirk step; `village_day` and `famine` re-run after the villagers.ron change |
| Mishaps feel like punishment | Low-tension beats; a third fall on your enemies; good ones too (a wedding, a feast); a playtest question |
| Mishaps wipe a run or a band | No champion, captain or legend as subject; deaths from two kinds only, one a day, in armies of 10 or more; food floors; no delay with a foe near; each guarded by a test over every `bot_run` seed |
| Mishaps change pacing | Never notable: the quiet clock, the first event and fast-forward don't see them (a unit test, and a run with stubbed effects whose quiet clock and raids match a run without mishaps). Their effects can change a run's path, like any change to a run |
| Mishaps take control away | A wedding's halt ends at any new command for that captain or when a hostile army comes near, and never holds a march to a village asking for help; the goose rout and the sleepwalker are short, and never land anyone near a foe |
| Mishaps tested only on steady | `mishaps_never_wipe_a_run` runs all three personalities with compressed data; the 8x cap is a throttle, not a statistic |
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
| Cost | A push on a kill is the only new work per blow; nudges cached on `Veteran`, fixed-point; hourly work for the sparse parts; NFR-09 is measured on `--battle 7500`, with and without `--saga-bench`, on the run island's live ticks, and as the saga's own spans in a bot run's live windows (never on frozen ticks after a run ended) |
| Saga state diverges unseen | Append-only parts by length and digest; the parts changed in place (open battles, weapons, counts, grudges, pending deaths) hashed in full |
| Save between ticks loses a walk home | The Sweetheart's walk home sets the hashed, saved `Follower.deserting`, never the exempt `Deserters` scratch |
| The gore contradicts the words | One function, `Teller::told_wound`, gives the feed its `{wound}` and the renderer its part, from the record alone, whatever form a line takes; every brutal telling of a severed death names the part (feed variants, the `terse` twin, long forms, report lines, fate clauses; lint); names in a told line are frozen at the death, so a later epithet never changes its fit; fixed text names no body part (lint); a maimed victim's lost part is never wounded again; a test checks every form of every told death of a seeded battle (section 20) |
| A deserter leaves a body, or bleeds | Bodies come only from `Casualties.fallen` and sprays only from `Casualties.struck`, read after every tick in `FixedLast`, never from "a visual whose person vanished" or "a visual whose health fell": in the 7,500-a-side battle, 19 desert for every one who dies, and a deserting recruit's health drops by 40 when they take civilian stats again |
| A finished run replays its last deaths | After a run ends, `Casualties` keeps the ending tick's lists, since the combat steps no longer run. The collector copies only when `SimTick` has advanced since its last copy, so a frozen tick yields nothing |
| A load shows the old world's dead | Bodies, pools, parts, droplets and the collector's buffer clear in `RenderSet::Epoch`, and its last tick resets; the saga cursor starts at the end of the loaded `Saga` |
| Effects eat the frame in a big battle | Effects only in the detail band (bodies to the low-poly band); per-frame burst caps ranked told kill > kill > maim > hit, nearer the camera first, sized to each Hanabi asset's capacity; capped droplets, bodies, pools and parts; pooled GPU instances, never one per hit; NFR-10 gates on the stress battle, at 1,000 a side and with `--effects-bench` |
| A GPU-only dependency breaks CI, tests or a weak machine | Only game_app adds `EffectsPlugin`, which adds `HanabiPlugin` only when the render device has compute shaders; headless builds don't compile game_render; the CPU fallback and Off; smoke runs with each backend forced and with WebGL2 limits ([ADR 0019](adr/0019-combat-effects-and-bevy-hanabi.md)) |
| Sprays vanish at 8x, or freeze oddly | The effect clock runs at game speed up to 1x and on real time above it (`Time<Real>`, not `Time<Virtual>`, whose catch-up cap would slow it), and stops when paused; bodies and pools last the later of their game hours and a floor of real seconds |
| The kill cam fights the player | Rare (captains, the champion and legends, on screen), never at 8x or in fast-forward, once per 10 real seconds; any speed key, pause or camera input ends it; it restores the speed only if nobody changed it; a pause the death brings waits and is never lost; a toggle |
| Maims snowball into a band of cripples | At most one maim in a life, about a quarter of those past half their health; small penalties through the holder only, run-only; a lost leg never slows a captain's march (their pace is the army's); `bot_run` counts them |
| A maim is missed on a quiet tick | The roll is its own serial step after `casualties`, run whenever a maim check was noted, because `casualties` runs only on ticks when someone fell, and most battle ticks have no death |
| Gore toward villagers | Militia, rebel and villager victims take grave keys, which never use `{wound}`, so no part leaves them; they bleed like anyone struck, in the brutal tone only (open question 19) |
| Name clashes | Goodwill's `Deed` and the champion's `Feat` already exist: the new type is `SagaDeed`, the cause `Cause::Earned`, and the glossary says "Saga deed". "Fame" avoids FUN_DESIGN's "renown"; "Mood" and "Bonds" are new; a quirk is never called a trait |

## 19. Proposed roadmap change

| Milestone | Was | Proposed |
| --- | --- | --- |
| **M4c "Saga"** | — | New, after the M4 playtest and before M4b: kill lines and battle moments, service records, deeds and epithets (honourable and mocking), absurd deaths, SIM-34's earned traits, legends and render-only memorials, quirks, mishaps, named weapons, kin, friends, rivals and moods, rescue records, UI-09's biographies, end-screen legends and oddities, the tone setting and the deadpan voice, and the run record for M6; then the combat spectacle (render-only gore drawn by bevy_hanabi or a CPU fallback, bodies that linger, severed parts that match the kill lines, the kill cam) and maimed survivors. About twice the first draft's size (1.25 times the plan before the combat spectacle); the plan suggests a split after its step 8b (part 1 about 64%) |
| M4b | Living towns | Also: hero memorials raised by saved villages, which become tended graves (section 14); animals with names; weather quirks |
| SIM-34 | M4 | M4 (champion traits), M4c (earned traits) |
| UI-09 | M4 | M4, M4c (biographies) |
| M5 | Scale and information | Also: tall tales and rumours about you, through faction knowledge |
| M6 | The legends book of past runs | Unchanged; it reads M4c's run records (and M4b's memorials) |

Requirement IDs added to SEED.md on 2026-10-12 (proposed, with M4c and M4b). SEED.md's wording is canonical; this table copies it:

| ID | Requirement |
| --- | --- |
| GP-29 | A finished run leaves a record of its legends (names, epithets, deeds, fates), named weapons and the villages they saved, for the legends book of past runs; nothing in it changes a later run |
| GP-30 | In a run, the pacing director schedules small mishaps from real sim state (bad mutton, a dog at the rations, a wedding on the road, a slip in the shield wall); each is low-stakes and rate-limited, never touches the champion, a captain or a legend, and never ends a run |
| SIM-39 | Every death in battle records who killed whom and how, including absurd but real ends; notable deaths are told one by one, and the rest are counted into the battle's report |
| SIM-53 | Soldiers keep a service record (kills, battles, wounds, deeds) that outlives desertion and, on death, is kept for anyone told or named; they earn one epithet by rule, honourable or mocking, which says why |
| SIM-54 | A few soldiers of great fame in each faction become legends: a legend steadies nearby men a little and their death shakes the army; legends gain no stats and are not heroes |
| SIM-55 | In a run, every villager and soldier has one or two quirks (coward, glutton, drunkard, pious, fears geese, ...) dealt from a data deck by their own seeded stream; each nudges only its holder's behaviour, slightly, and outside a run nobody has one |
| SIM-56 | A weapon that kills a notable foe (a captain, a champion, a legend, a monument's beast) gets a name and a history, and is passed on, looted, sold, taken home or lost as the sim moves it; named weapons gain no stats |
| SIM-57 | Soldiers have kin (their household) and make friends and rivals by rule; kin grieve and avenge, rivals feud and make peace, and each soldier's mood is the bounded sum of its recorded causes, which the inspector explains |
| SIM-59 | A survivor of a bad wound may be maimed once in their life (a hand, an eye or a leg) by rule, from their own seeded stream: in a run, a small lasting penalty to their own attack, guard or pace and an epithet (One-Hand, One-Eye, One-Leg); in every preset, a line in their service record; kill lines and visuals never contradict it |
| SIM-58 (M4b) | A settlement saved from raiders by a named hero who fought them there (M4c's rescue record) raises a memorial to them in its town style once it can build one; it gives small local effects (goodwill and loyalty there, recruits a little more willing, the hero's army steadier near it) and becomes a grave the town tends when the hero dies |
| RND-09 | Living legends are marked on the map; a cairn stands where a legend fell, and a memorial stone in their home village |
| RND-10 | Battles show their blows inside the detail band with render-only effects: blood on hits, bursts on kills, pools that fade, severed heads, arms and legs on told kills (the part the kill line names), and bodies that lie where they fell and fade; over the top in the brutal tone, dust and falls only in the mild; drawn by GPU particles where the adapter has compute shaders, a CPU fallback where it hasn't or on a software adapter, or off by choice; the simulation, saves and replays never see them, and headless builds, tests and CI need no GPU |
| RND-11 | Kill cam: when a captain, the champion or a legend is killed by a blow on screen near the camera, the game slows to a quarter speed for about a second and the camera leans in; never at 8x or in fast-forward, at most once in 10 seconds, switchable off; a viewing effect, never sim state |
| UI-10 | Kill lines, mishaps, mood notes, battle reports and biographies are told from phrase banks in data, in a deadpan dark voice, brutal by default and mild by choice; the choice is a viewer setting the game remembers, and it never changes the simulation |
| NFR-09 | Saga bookkeeping (kill records, service records, legends, quirks, moods, bonds, named weapons, mishaps): at most +2% on the median (of three runs) mean tick of the `--battle 7500` clash, with and without the saga's run effects forced on (`--saga-bench`), and of a run's live ticks (`--run --seed 7 --difficulty steady --ticks 7200`, a window that ends before the run does), over M4 baselines recorded in docs/m4c/; in a bot run (`--run --bot --seed 42`, the windows before the win), the saga's own spans stay under 2% of the mean tick; never measured over ticks after a run ended; no allocation per blow |
| NFR-10 | Combat effects cost: on the reference machine with Full effects, the effects systems (ours and Hanabi's main-world systems) take at most 0.5 ms of CPU a frame (median over a clash), `--stress 20000`'s worst clash window stays at 60 fps or more and within 1.5 ms of the same window measured before the effects layer, and a 1,000-a-side clash framed in the detail band holds 60 fps; the CPU fallback at its caps takes at most 1 ms a frame, and on a software adapter the Reduced setting adds at most 10% to the median frame time over Off |

## 20. Combat spectacle: blood, bodies and the kill cam (RND-10, RND-11, NFR-10)

You asked for fights that look as brutal as their lines read: blood on hits, bursts on kills, pools, severed parts that match the line, bodies that stay where they fell, and a kill cam. All of it is drawn by the renderer from records the sim already keeps. The sim, saves and replays never see it. The decision and its alternatives are in [ADR 0019](adr/0019-combat-effects-and-bevy-hanabi.md).

**What you see** (brutal tone, Full effects, 1x, the camera in the detail band over the front at the Battle of Loweby Fields):
1. The lines meet. Every blow that lands throws a short red spray off the one struck, away from whoever struck them.
2. A raider folds and drops where they stood, a gush going out of them. A second later the furrow under them darkens into a pool.
3. Jory Lowe's axe finds captain Rook Vane. The feed reads "Jory Lowe took captain Rook Vane's head off at the neck in the barley." The game drops to a quarter speed and the camera leans in. The head goes up, turns over, hits the furrow twice and rolls to a stop a few paces from the body, which is still pumping. A second later the speed is back where you had it.
4. Odo Rook takes a blow past half their health and lives. A hand goes into the barley ("Odo Rook lost a hand at Loweby, and kept swinging."). From then on Odo's figure carries a bound stump.
5. The raiders break. The pools fade within the next game hour. The dead lie where they fell for three game hours (at least half a minute of real time), then sink into the ground, oldest first.

In the mild tone the same fight is dust kicked up and people falling. Nobody bleeds and no part leaves anyone, though the dead still lie where they fell. With Effects off there are no droplets at all, not even dust, and everything else stays: the dead fall and lie, and in the brutal tone pools spread and parts fly. Off turns off the spray; the mild tone is the switch for gore.

**What each setting shows:**

| | Brutal | Mild | Effects Off (no spray, either tone) |
| --- | --- | --- | --- |
| A blow that lands | A blood spray | A small dust flick (GPU only) | Nothing |
| A death | A burst, the body falls and lies, a pool spreads | A dust puff, the body falls and lies | The body falls and lies; in brutal a pool spreads |
| A told death with a severing wound | The part its line names flies, arcs, bounces and settles; the stump pumps; a larger pool | As any death: no part, and the line names none | In brutal, the part flies and the larger pool spreads, with no fountain; in mild, as any death |
| A maim (section 21) | The hand or lower leg flies, or a burst at the head for an eye; a small pool | A dust flick | In brutal, the hand or leg flies and a small pool spreads; in mild, nothing |
| Maimed figures | An eye patch, a bound stump, a limp | The same (they aren't gore) | The same |
| Kill cam | On by default (its own toggle) | The same | The same |

**The effects layer.** One layer in game_render turns what the sim recorded into **effect requests**, and one **backend** draws them. Requests are render-side values; nothing of them is ever written into the sim.

| Request | Made from | Drawn as |
| --- | --- | --- |
| `Hit { at, dir }` | This tick's landed blows (`Casualties.struck`: the victim and the striker), copied after every tick. A blow on someone killed in the same tick is left to the kill. Several blows on one person in a frame merge into one | A spray (droplets) |
| `Kill { at, dir, told }` | This tick's kills and falls (`Casualties.kills`, `fallen`), copied after every tick | A burst (droplets), then a `Pool` and a `Corpse` |
| `Sever { at, dir, part }` | A told death whose wound severs (`Teller::told_wound`, below), read from `Saga` with a render-side cursor | A part (a mesh) and, on the GPU, a stump fountain |
| `Maim { at, dir, part }` | This tick's maims (`Casualties.maims`, section 21) | A part for a hand or a leg, a burst at the head for an eye, and a small pool |
| `Pool { at, size }` | A kill or a maim, after a short delay | A flat splat mesh on the ground |
| `Corpse { visual, at, facing, headless }` | A death, matched to the dead person's visual | The visual itself, tipped over, lingering, then sinking |
| `Dust { at }` | Hits and kills in the mild tone | A puff (droplets) |

**Where requests come from** (reading only):
- **After every tick.** A game_render system in `FixedLast`, as `snapshot_positions` runs in `FixedFirst`, copies this tick's `Casualties.struck`, `kills`, `fallen` and `maims` into a render-side buffer, with the positions of the people they name. Several ticks can run in one frame at 8x, so reading `Casualties` in `Update` would lose deaths. `Casualties.fallen` gains the victim's `Entity` and SpawnId (exempt scratch, reset on load: no hash, save or replay change), so starvation deaths get a body too.
- **Only a tick that ran.** The collector remembers the last `SimTick` it copied (a `Local`, reset on a new world or a load) and copies only when the tick has advanced. Once a run has ended, every `SimSet` is gated off, so nothing clears `Casualties` again and the ending tick's lists stay as they were. Without the check, the collector would send the run's last deaths again ten times a second under the end screen.
- **A blow, not a health drop.** Hits come from the sim's own list of landed blows, never from a visual's health falling between frames. A recruit who deserts takes civilian stats again, so their health drops by 40 at full health (100 to 60) with no blow; a starving soldier, a slip in the shield wall and a camp brawl lose health without one too.
- **A death, not a desertion.** Bodies come only from that buffer. `sync_visuals` checks it before despawning a visual whose person is gone: a dead person's visual becomes a corpse (its `AgentVisual` removed, its pose tipped over), and everyone else's vanishes as today. In the 7,500-a-side battle, 19 people desert for every one who dies, so "a visual whose person vanished" would be wrong 19 times in 20.
- **Told deaths.** A render-side cursor (a `Local`, like the feed's, reset on a new world) reads new `Fall`s from `Saga`. A `Fall` holds the victim's SpawnId and, new in M4c, **the position where they fell** (`at`) and **their role** (captain, champion or legend), which the kill cam, the feed's "move the camera there" and the cairn also need. The renderer keeps each visual's SpawnId, so a `Fall` finds its corpse.
- **The killer's direction.** From `Casualties.kills` and `struck` the collector reads the striker's position (usually still alive) for the spray's direction. Without one (hunger, a mishap), the spray goes up and out on a hashed bearing.

**The body part: one function for words and visuals.** The part that flies is the part every line names, because both come from one function of the record:
- **`saga::wound(fall, words) -> Option<Wound>`** is a pure function in `sim_military::saga`'s words. `Wound` is a part (head, neck, face, throat, chest, belly, back, arm, hand, leg or eye; ids append-only) and whether it's severed.
- **None** when the death has no killer (hunger, a mishap), or when the victim is militia, a rebel or a villager: their grave keys never name a wound.
- **The part** is a weighted pick from saga.ron's `wounds` table, among the rows for the killer's kind (armed, unarmed or beast) and leaving out the part the victim had lost, if any (`Fall.maim`, section 21). The pick uses a stable integer hash of (victim SpawnId, death tick), never `SimRng`.
- **Severed** when the row can sever (head and neck give the head; arm; hand; leg), the killer is armed or a beast, and the blow was a single one, or the victim was tier A, or the killer is a legend, or a second hash falls under the row's `sever_percent` (about half for limbs, a third for the head). Over the top, as you asked.
- **`Teller::told_wound(fall, tone)`** is that wound for a told death in the brutal tone, and none in mild. It reads only the record and the `wounds` table, never which form a line took, so a line that falls back to a shorter form, or a killer who earns an epithet later, can't change it. The renderer calls this same function.
- **Every telling names a severed wound** (section 3). The Teller picks only among a key's variants that carry `{wound}`; the key's `terse` form has a severing twin with `{wound_terse}` ("head off"), which fits 72 characters with the longest names; and long forms, report lines and the fate clauses of biographies and epitaphs carry `{wound}` or `{wound_terse}` too. The lint checks that every kill key has them, at least four brutal variants with `{wound}`, and a terse phrase for every severable part. A wound that doesn't sever is flavour: a variant may name it or not, since nothing flies. Mild never uses `{wound}`, so mild shows no part.
- **Names are frozen at the death.** A `Fall` keeps the killer's epithet as it stood, so a told line fits the same way forever and reads the same in the feed, on the end screen and after a load.
- **Worked out, never stored.** The part is flavour worked out from the record, like a weapon word: the sim never reads it, and the `wounds` table lives in saga.ron (words, exempt, unsaved). Editing it changes what is told and shown, both together, and nothing in the sim.

So the sample line "Rowan Kemp hacked through captain Garrick Hale's neck" is, in saga.ron, `{killer} hacked {wound}` with the neck cut filled in, and if that wound severs, Garrick Hale's head is what you see go, and the report reads "Slain: captain Garrick Hale, head off, by Rowan Kemp."

**Three backends.** Hanabi does only the droplet spray, and only the spray differs between them: the GPU draws it densely, the CPU sparsely, and Off not at all. Severed parts, pools and lingering bodies are a handful of ordinary meshes per battle, drawn identically on every backend, Off included (your decision 47).

| Backend | Draws the spray with | Droplets: hit / kill / told kill | Live droplets | When |
| --- | --- | --- | --- | --- |
| **GPU** | bevy_hanabi: one `EffectAsset` per kind (hit spray, kill burst, stump fountain, dust), each with its own `capacity` from render.ron, and a pool of about 40 pre-spawned instances (12 hit, 12 kill, 4 fountain, 12 dust) in global simulation space, so moving an instance to its next burst leaves the last burst's droplets where they are. Fired round-robin with `EffectSpawner::reset()`. Droplets die by age and stop on a per-burst ground height carried in a particle attribute; Hanabi has no collision | 40 / 250 / 600, plus 300 over 1.5 s from the stump on a severing | Each kind's instances × its capacity (below) | The default where the device has compute shaders and the adapter isn't a software one |
| **CPU** | Our own droplets: a pool of small instanced meshes (one mesh, a few materials, batched by Bevy), moved on the CPU, oldest recycled | Full: 8 / 32 / 64. Reduced: 4 / 16 / 32 | Full 1,500; Reduced 600 | No compute shaders (WebGL2, GL, old drivers), a software adapter (lavapipe, llvmpipe, WARP), or `--effects cpu` |
| **Off** | No droplets. Bodies, pools and parts as on the other backends | — | — | Effects Off; `--effects off`. Headless builds, tests and CI have no renderer, so `EffectsPlugin` is never added there and nothing is drawn |

**Hanabi's capacities.** An `EffectAsset`'s `capacity` is how many particles each of its instances can keep alive at once. All instances of one asset share a GPU buffer (a slab) of at least 65,536 particles, rounded up (`ParticleSlab::MIN_CAPACITY`), and each gets a slice of its asset's capacity. So:
- **The live cap** of a kind is its instances × its capacity. An instance fired again while its last burst lives drops whatever doesn't fit.
- **The per-frame burst caps must fit:** each instance fires (bursts a second ÷ instances) times a second and must hold that × droplets × lifetime. A test checks this over render.ron.
- **Slab memory** per asset is the larger of 65,536 and the sum of its instances' capacities, times the particle's size (about 48 bytes: position, velocity, age, lifetime, colour and the ground height). That's about 3 MB an asset and 12 MB for the four.

| Asset | Droplets a burst | Lifetime | Instances | Capacity each | Bursts a second at the cap | Live per instance at the cap |
| --- | --- | --- | --- | --- | --- | --- |
| Hit spray | 40 | 0.5 s | 12 | 1,024 | 480 (8 a frame) | 40 × 0.5 × 480 ÷ 12 = 800 |
| Kill burst | 250, or 600 told | 1.2 s | 12 | 4,096 | 120 (2 a frame), plus told kills | 250 × 1.2 × 120 ÷ 12 = 3,000, with room for a told burst |
| Stump fountain | 300 over 1.5 s | 1 s | 4 | 512 | Told severings only, a handful a battle | About 300 |
| Dust | 24 | 0.8 s | 12 | 1,024 | 480 (8 a frame) | 24 × 0.8 × 480 ÷ 12 = 768 |

**Automatic selection, and overrides:**
- **Where.** `EffectsPlugin` lives in game_render, but only game_app adds it, after `DefaultPlugins` and game_render's own plugin. Its `finish()` runs after Bevy's `RenderPlugin::finish`, which puts `RenderDevice`, `RenderAdapter` and `RenderAdapterInfo` into the main world. Without a `RenderApp`, it chooses Off.
- **Compute** means all of these, read from what Bevy will actually use:
  - the adapter's downlevel flags (`RenderAdapter`) include `COMPUTE_SHADERS`, `INDIRECT_EXECUTION` and `VERTEX_STORAGE`;
  - the **device's** limits (`RenderDevice::limits()`, not the adapter's): `max_storage_buffers_per_shader_stage` at least 8, `max_compute_workgroup_storage_size` not 0 (Bevy's own canary for WebGL2 limits), and `max_bind_groups` at least 4. With `WGPU_SETTINGS_PRIO=webgl2` on a Vulkan adapter, the adapter still reports compute while the device is held to WebGL2's limits, which is why Bevy's GPU preprocessing reads the device too. Reading the adapter's limits would load Hanabi there and quit the game;
  - the backend isn't GL: `Backends::from(info.backend) != Backends::GL` (Bevy re-exports `Backends`). Bevy turns its GPU preprocessing off on GL, and Hanabi's CI runs Vulkan only.
- **A software adapter** is a `DeviceType::Cpu` adapter (`RenderAdapterInfo`'s `device_type`), matched by its debug name: Bevy doesn't re-export the type, and a direct wgpu dependency isn't worth it for one comparison. This is the only check that matches a name.
- **Hanabi is added only with compute,** by `EffectsPlugin::finish`:
  - it calls `app.add_plugins(HanabiPlugin)`, which runs Hanabi's `build` and registers it, so `is_plugin_added` works and a second add panics at once;
  - then it calls `HanabiPlugin.finish(app)` by hand, because the finish loop never reaches a plugin added during it.

  That works because bevy_app 0.19.1 fixes the range of plugins it finishes when the loop starts, and refuses new plugins only after it has finished. None of Hanabi's sub-plugins has a `finish` or `cleanup` of its own. Added without compute, Hanabi's eager compute pipelines raise a validation error, which quits the game, and without a renderer its `finish` panics. With compute, it's added even on a software adapter, so Full stays possible there, unless the flag forces `cpu` or `off`. A Hanabi plugin can't be added after startup, so this keeps the setting switchable at runtime.
- **The GPU instances exist only while the GPU backend draws.** Switching to Reduced or Off despawns them, and switching back to Full respawns them; their shaders are compiled by then.
- **The Effects setting** (Full, Reduced, Off) starts at the machine's choice: Full where there's compute on a hardware adapter, Reduced otherwise. The preferences file stores only a choice you made.
  - Full draws GPU where Hanabi is loaded, and CPU at its Full caps where it isn't.
  - Reduced draws CPU at the Reduced caps.
  - Off draws no droplets.
  - Switching takes effect at once: Off despawns every droplet, and bodies, pools and parts stay.
- **The flag** `--effects gpu|cpu|off` forces a backend for the session and isn't stored. `gpu` without compute logs a warning and draws CPU.
- **No compile-time switch.** The GPU backend is one module (`effects/gpu.rs`), the only code that names Hanabi. If a Bevy upgrade ever comes before a matching bevy_hanabi, that module can be put behind a feature then, or dropped, and the CPU fallback carries on. A feature now would be a code path that nothing builds.

**The effect clock.** Effects run on their own clock:
- **At 1x or slower, game speed** (`Time<Virtual>`). Pausing freezes droplets in mid-air, and the kill cam's slow motion slows them.
- **Faster than 1x, real time** (`Time<Real>`), stopped while `Time<Virtual>` is paused. Not `Time<Virtual>` divided by the speed: in a big clash the catch-up cap (game_ui's `cap_catch_up`) clamps virtual time's delta before the speed multiplies it, so virtual time ÷ speed runs slower than real time, and sprays would play in partial slow motion exactly when 8x is busiest.

On the GPU, Hanabi's `Time<EffectSimulation>` advances by `Time<Virtual>`'s delta times its relative speed. A system in `Last` sets that speed: 1 at 1x and below, and above 1x this frame's real delta ÷ virtual delta (clamped to 0.05–4), which Hanabi applies to the next frame. A frame's lag after a speed change doesn't show. The CPU backend reads the same effect clock, so both agree. Lifetimes are "the later of" a span in game hours (sim ticks) and a floor in effect seconds. At 8x a game hour is 1.25 real seconds, and a pool that faded in that time would only flash.

**Bodies, pools and parts:**
- **Bodies.** The visual of the dead tips over in 0.35 effect seconds, away from the killer, and lies flat. A severed head swaps it to a headless figure mesh (the figure has no limbs, so an arm or a leg leaves the body as it was). It lingers 3 game hours, at least 30 effect seconds, then sinks into the ground over 4 seconds and is despawned. Sinking, not alpha, keeps it in the same batch as the living. At most 300 bodies, the oldest sinking first when more fall. A dead captain's banner falls with them.
- **Pools.** Brutal only. A pool starts 0.4 seconds after the burst and spreads over 1.5 seconds: 0.5 to 0.8 tiles across for a kill, 1.0 to 1.4 for a told kill, 0.4 for a maim. It's a flat quad laid along the terrain's normal just above the ground, with one of four splat textures from `game_render::art`. It lasts 1 game hour, at least 12 effect seconds, and fades over its last third in four steps, one material each, so pools still batch. At most 200; the oldest fades first.
- **Parts.** Brutal only. A head, an arm, a hand or a lower leg: low-poly meshes coloured like the figure, with a cut face. A part leaves from the height the wound names, on a ballistic arc out from the killer with a spin, under gravity on the effect clock. It bounces at most three times on the terrain's height (`surface_height_at`) and settles. Its path comes from a stable hash of the `Fall`'s index, so a screenshot is reproducible. It stays as long as its body, and sinks with it. At most 40.
- **A switch to mild** despawns every pool and part, gives headless bodies their heads back and keeps the bodies.
- **A new world or a load** (`WorldEpoch`, cleared in `RenderSet::Epoch`) despawns every body, pool, part and CPU droplet, and, while the GPU draws, respawns its instances from their assets (no old droplet survives). It also empties the collector's buffer, resets its last tick, and starts the saga cursor at the end of the loaded `Saga`, so no death from before the load is shown. After a load the field is clean.

**Detail bands** (render.ron):
- **Sprays, bursts, parts, pools and maim effects** are requested only for people inside the detail band (150 world units, with its hysteresis), as you asked. A burst beyond it is never requested.
- **Bodies** are made inside the low-poly band too (600), since a figure that vanishes there would pop. Beyond it, the dead vanish as today.
- **A body or pool already lying** stays when the camera pulls back, until its time is up. It's drawn with the band's mesh like any figure, or skipped beyond the low-poly band.

**Budgets and caps.** The measured numbers (seed 42, headless) set the scale:
- **Deaths.** About 300 die in either the 1,000- or the 7,500-a-side battle. The front line, not the army size, limits the rate: at most 5 in a tick, and 1.2 to 1.4 a tick at the busiest.
- **Blows.** There's no blow count yet (step 1 adds `Casualties.struck`, whose length is the count). Kills take 5 to 11 blows, so the peak is estimated at 25 to 50 blows a tick.
- **The reference machine** (Ryzen 5 7600X, RTX 4070) ran the 7,500-a-side clash at 118 to 176 fps, and 81 fps in its worst window (ADR 0012). That leaves about 4 ms of a 60-fps frame.

| Cap (render.ron `effects`) | GPU | CPU, Full | CPU, Reduced |
| --- | --- | --- | --- |
| Bursts started a frame: hits / kills | 8 / 2 | 6 / 2 | 3 / 1 |
| So, bursts a real second at 60 fps, at most: hits / kills | 480 / 120 | 360 / 120 | 180 / 60 |
| Told kills a frame (up to 8 a tick) | Never dropped | Never dropped | Never dropped |
| Live droplets | Instances × capacity: hit 12,288, kill 49,152, fountain 2,048, dust 12,288 | 1,500 | 600 |
| Bodies / pools / parts | 300 / 200 / 40 | 300 / 200 / 40 | 200 / 120 / 24 |
| CPU time of the effects systems a frame (median, at the caps) | 0.5 ms, ours and Hanabi's main-world systems | 1.0 ms | 0.6 ms |

**The demand these caps meet.**
- **The whole battle** makes, at its busiest at 1x, about 14 deaths and 250 to 500 blows a real second (10 ticks a second). Only the slice of the front inside the detail band asks for bursts, so the GPU caps take nearly every one of them and the CPU caps most.
- **At 8x** the 25 ms catch-up cap keeps a big clash near one tick a frame, so the demand per frame barely grows.
- **Parts** come only from told deaths: at most 10 tier-B deaths a battle, plus its tier A, and 8 a tick.
- **Bodies** are a few hundred a battle at most.

When more is asked than a frame allows, requests are ranked: told kill > kill > maim > hit, nearer the camera first. Hits closer together than half a tile merge, and the rest are dropped. This is render-only, so nothing is lost but a droplet.

**The kill cam** (game_ui decides; game_render nudges the camera):
- **Who.** A captain, a legend or the champion of any faction, **killed by a blow**. A captain or a legend is found by the victim's role in its `Fall`, with its `at`, never by the `Fall`'s tier: tier A also covers a legend's milestone kill of an ordinary soldier, a mishap's death and a Glutton's starvation. The champion is found by `ChampionFell`, which already carries `at` and its cause. A death with no killer (hunger, a mishap) never starts one. The death must be on screen (`Camera::world_to_viewport`) and within render.ron's `detail_distance` of the camera.
- **Never** at 8x, in fast-forward (`FastForward`), while paused, behind a dialog (`InputGate.blocked`), during the opening shot, or with the kill cam switched off.
- **What.** At 1x to 4x, `Time<Virtual>`'s relative speed goes to 0.25 for 1 real second (eased in over 0.1 s and out over 0.2 s), then back to the speed it replaced. The camera rig's targets move 35% of the way toward the death and zoom in 15%, no closer than the rig's minimum, on `Time<Real>`, and the camera stays there: no snap back. There's no lead-in: deaths are found after the tick, so the kill cam frames the aftermath (the burst, the fall, a part in flight), which the effect clock slows too.
- **Once in 10 real seconds.** With several in one frame, the champion comes before a legend before a captain, nearest first. A chain of falling heirs shows only the first.
- **You win.** A speed key, a pause or any camera input ends it at once and keeps what you chose. It restores the speed only if `Time<Virtual>` is still at its 0.25 and unpaused. While it runs, the time bar lights the speed it will return to.
- **The pause waits for it.** `ChampionFell`, and your own `LegendFell`, auto-pause (director.ron `auto_pause`). When a kill cam starts for that death, the feed holds the pause until it ends, about 1.3 real seconds, in which the sim moves about 3 ticks. So the slow motion lands and the pause follows on the aftermath. Without a kill cam the pause is immediate, as today. A held pause is never lost: it's applied when the kill cam ends, for any reason.
- **Data and switches.** Its numbers are in render.ron `kill_cam` (slow, real seconds, eases, cooldown, nudge, zoom). Kill cam On/Off is in the preferences file, and `--no-kill-cam` turns it off for the session.
- **Never sim state.** `Time<Virtual>` only paces `FixedUpdate`; the ticks and their order don't change, so a state hash at a given tick never depends on it.

**Performance gates (NFR-10)**, in the effects steps' notes and again at the end:
- **`--stress 20000`** (the 7,500-a-side seed-42 battle), on the reference machine, with Full effects:
  - the worst clash window keeps 60 fps or more (RND-04), and its mean frame time rises at most 1.5 ms over the same window measured before the effects layer (on the build before step 3b, recorded in `docs/m4c/`), so bodies, pools and parts count as well as the spray;
  - measured once at the opening shot and once zoomed into the detail band over the front;
  - `stress_log` gains the live droplets, bodies, pools, parts, bursts a second and the effects systems' spans.
- **1,000 a side,** the camera in the detail band over the front in the busiest window (ticks 3,400 to 3,500): 60 fps with Full effects, and never below 30 (NFR-05).
- **What the CPU budget covers.** Our effects systems, and Hanabi's main-world systems (its clock in `First`, and `EffectSystems` in `PreUpdate` and `PostUpdate`), each in a span. Hanabi's render-world systems are logged beside them, not gated: they run in the render world, pipelined with the next frame, and show in the frame time.
- **`game_app --effects-bench <bursts a second>`,** a synthetic load with no battle: it requests bursts, bodies, pools and parts at the caps around the camera's focus for 60 seconds. The effects systems' median span must stay within the table's CPU time on each backend. Reaching a real clash takes minutes even at 8x, and about 15 minutes at a software renderer's 3 to 4 fps; the bench reaches the caps at once.
- **On a software adapter** (lavapipe under Xvfb, as on the dev box): Reduced under `--effects-bench` adds at most 10% to the median frame time over Off, as the median of three 60-second runs. The four cores are shared by the sim and lavapipe's rasteriser, so the gate compares medians and the effects systems' own spans, not single frames.
- **Your machine's checks don't block a step.** The reference-machine numbers need your RTX 4070. Each effects step's notes give the commands, the numbers join the notes when you run them, and NFR-10 is ticked only once they're in (step 15).
- **The sim's tick cost is unchanged.** The collector runs after the tick and copies a few dozen entries at most, and NFR-09's gates don't move.

**What is never touched:**
- **The sim.** No sim crate depends on game_render or bevy_hanabi. The layer's systems only read sim components and resources. The collector runs in `FixedLast` and writes render-side resources only. The `Casualties` additions are exempt scratch, reset on load.
- **Saves.** Nothing of the layer is saved; after a load the field is clean.
- **Replays.** There's no new command. The Effects setting, the tone's gore and the kill cam are viewer settings, not inputs.
- **Randomness.** Hanabi seeds its own render RNG inside the crate. That never reaches the sim, and the "never `thread_rng`" rule is about our code, which still never calls `rand::rng()` (clippy). Our CPU droplets and part arcs use stable hashes of record ids.
- **Headless, tests and CI.** `sim_headless` doesn't depend on game_render. No test adds the effects plugin, and Hanabi's `gpu_tests` feature stays off, so nothing needs a GPU. game_render's tests are pure functions.

**Data:**
- `render.ron` gains `effects` (the caps above per backend, droplet counts, each GPU asset's instances and `capacity`, lifetimes and floors, band choices, pool sizes, delays and fade steps) and `kill_cam` (the numbers above). It's viewer data, like the rest of render.ron, documented in `docs/dev/DATA.md`.
- `saga.ron` gains `wounds` (per part: its nouns for the lint, the killer kinds it fits, its weight, whether it severs and `sever_percent`, its phrases in both tones, and a severing part's terse phrase; mild's are never used in a line).
- The preferences file gains `effects` and `kill_cam` beside the tone.

**Tests** (no GPU anywhere):
- `select_backend` (game_render unit): every row of the selection, as a pure function over (adapter flags, device limits, backend, device type, setting, flag), including a Vulkan adapter whose device has WebGL2's limits.
- `effect_requests_follow_the_records` (game_render unit, over hand-built buffers):
  - a death makes a burst, a body and a pool, and a desertion makes nothing;
  - a landed blow makes a hit; a deserter's or a recruit's change of stats makes none, and neither do starvation, a slip or a brawl;
  - a frozen tick (no `SimTick` advance since the last copy) yields no request;
  - the bands decide what is requested;
  - the caps and the ranking hold, and each GPU burst cap fits its asset's capacity (a check over render.ron);
  - mild gives dust and no pools or parts;
  - Off gives no droplets and the same bodies, pools and parts as the other backends.
- `bodies_and_pools_fade_and_clear` (game_render unit): lifetimes on the effect clock with their floors, the caps (oldest first), and a new epoch clearing everything, resetting the collector's last tick and moving the saga cursor to the end.
- `wound_and_line_agree` (`tests/saga_words.rs`): for every told death of a seeded 1,000-a-side battle, every brutal form that tells it (the fitted feed variant, the `terse` form, the long form, its report line and a fate clause) names a severing wound exactly when `told_wound` severs, and the part is the one the renderer's request builder sends. The same holds after the killer earns a new epithet. Mild lines never contain one; militia and rebel deaths never get one.
- Kill-cam unit tests in game_ui (`kill_cam_*`), as a pure policy over (deaths this frame, speed, fast-forward, pause, input, cooldown):
  - who triggers, and that a legend's milestone kill of an ordinary soldier and a starving captain don't;
  - never at 8x, in fast-forward, while paused or behind a dialog;
  - the cooldown and the chain of heirs;
  - the speed restored only when untouched;
  - a held pause applied when the kill cam ends, for any reason.
- **Smoke runs** (step notes, not CI): the game starts and draws a skirmish clash with each backend forced, with automatic selection, with `WGPU_SETTINGS_PRIO=webgl2` (no compute: CPU, and no validation exit) and with `WGPU_BACKEND=gl`. Screenshots of each.

**Built:** the layer with its CPU and Off backends, bodies and pools in plan step 3b; severed parts, the gore following the tone and the Effects setting in step 4; the GPU backend (bevy_hanabi) in step 4b; the maim visuals in step 8b; the kill cam in step 10b; the final frame checks in step 15.

## 21. Maimed survivors (SIM-59)

A soldier who survives a bad wound may come out of it without a hand, an eye or a leg. It's a sim fact: recorded, hashed and saved. In a run it costs its holder a little for good and earns them an epithet. Battle Brothers' lasting injuries, kept small.

**What you'll read** (brutal; mild in italics):
- "Odo Rook lost a hand at Loweby, and kept swinging." *(Odo Rook lost a hand at Loweby.)*
- "Hild Crane left an eye in the mud at Kemp's Ford." *(Hild Crane lost an eye at Kemp's Ford.)*
- "Bram Tull lost a leg at Loweby, and lived." *(Bram Tull lost a leg at Loweby.)*
- "They call Odo Rook One-Hand now. Odo swings with the other." *(Odo Rook is called One-Hand now.)*
- Biography: "Lost a hand at Loweby on day 4; fights on with the other."
- Inspector: "Lost a hand (Loweby, day 4) · Attack −15%".

In saga.ron the part is always the `{maim}` slot, never fixed text (section 3's rule).

**The rule:**
- **When.** A blow takes a person with a `Veteran` (a soldier, a captain or the champion; never a beast, never militia) past half their maximum health lost to blows in this battle (`Veteran.hurt` crossing `maim.hurt`, 0.5), and they live through that blow. The strike loop notes the crossing in `Casualties.maim_checks`: a compare per blow, and a push only when it crosses. Hurt resets each battle, so a person is checked at most once a battle.
- **The roll is its own step.** A new serial system, `combat::maims`, runs after `casualties` in `MilitarySet::Combat` whenever a check was noted (its `run_if`). It can't be a pass of `casualties`, which runs only on ticks when someone fell, and most battle ticks have no death. For each check, in SpawnId order, for those still alive after the tick's blows: `SimSeed::stream(rng_key("sim_military::saga::maim") ^ SpawnId, tick)` draws whether they're maimed (`maim.chance`, 0.25) and which part, by weight: hand 4, eye 3, leg 3.
- **At most once in a life.** Wounds never heal. After a maim, less than half their health is left, so losing another half would kill them first, and a killing blow never maims. The code enforces it as well: a person with a maim is never rolled again. If M4b brings healing, a second maim is a decision for then.
- **Who never.** Beasts (no `Veteran`), militia and villagers (their wounds stay untold); the guard armies of monuments and bands for hire are ordinary soldiers and can be maimed.

**What a maim does** (run only, through the holder, read from `Veteran`'s cached nudges, fixed-point, exact no-ops at 1.0):

| Part | Effect | Read where |
| --- | --- | --- |
| Hand | Their blows land at ×0.85 attack | `resolve_strikes`, on the striker's `Veteran` (already borrowed for `rank_gain`) |
| Eye | Their guard at ×0.9 defence | `resolve_strikes`, on the victim's `Veteran` (already borrowed) |
| Leg | Their pace at ×0.85 in a chase, in a rout and when catching up with their place in the band; a captain's (the champion's too) only in a rout | `move_soldiers` (one new optional read of `Veteran`'s cached pace) and `move_captains` (in a rout) |

- **A lost leg never slows a march.** A captain's march pace is the whole army's: followers move at their captain's speed (GP-23), and the captain waits for stragglers. A one-legged captain marching at ×0.85 would slow every follower and every order for the rest of the run. So a captain limps only when routing, and a one-legged follower keeps up on the march (followers can move faster than their captain to catch up) and falls behind only in a chase or a rout.
- Nothing else changes: no morale, desertion or ranks. A maimed captain still leads at the army's pace, and a maimed champion still picks traits.
- Outside a run, maims are recorded (and shown) but change nothing, so developed, skirmish and `--battle` stdout stay byte-identical.

**Records:**
- `Service` gains `maim`: the part, the tick and the battle number, if there is one.
- The maim is a saga note (`Saga.notes`, kind `Maimed`), which the feed tells for your people at 1x–4x and never at 8x.
- `Casualties.maims` (exempt scratch) carries it to the renderer for the moment it happens.
- When a maimed person dies, their kept `Fall` copies the part, so the words and the renderer never wound a part that's gone.

**Epithets** (appended to `saga.epithets` as ids 27 to 29, a new kind `scar`, at ranks 8 to 10: above every mocking epithet and below every honourable one, whose ranks moved up to 11 to 29; ids don't change):

| Id | Rank | Rule | Brutal | Mild |
| --- | --- | --- | --- | --- |
| 27 | 8 | Lost a hand | One-Hand / Stump | One-Hand |
| 28 | 9 | Lost an eye | One-Eye / Squint | One-Eye |
| 29 | 10 | Lost a leg | One-Leg / Hop-Along | One-Leg |

- A scar epithet washes off a mocking one ("Twice-Fled is One-Hand now. Nobody uses the old name.").
- Any honourable deed replaces it: a maimed legend keeps their honour, and "One-Hand" appears in their biography instead.
- A Vain soldier takes a scar epithet as mocked (the `mocked` mood, section 10).

**Visuals** (section 20), which agree with the record by construction:
- **The moment.** In brutal, a lost hand or lower leg flies from the right height with a spray and a small pool, and a lost eye is a burst at the head. In mild it's a dust flick. With Effects off, there's no spray, and in brutal the part and the pool still come.
- **After.** In the detail band, in both tones and on every backend, the figure carries it: an eye patch (a figure mesh variant), a bound stump at the side for a lost hand (another variant), and a limp, a rocking tilt while moving, for a lost leg. These are a few extra meshes, still batched; the limbless figure can't show more.
- **Later.** A later kill line and its severed part never take the part already lost (`saga::wound` leaves it out).

**Wounds never heal.** Health never comes back in the sim (open question 10), and a maim is for life by the same rule. The same rule makes a maim once in a life (above). If M4b brings healing, it heals health, never a lost hand, and decides then whether a second maim can come.

**Data:** `progression.ron` `saga.maims: (hurt: 0.5, chance: 0.25, parts: [(part: Hand, weight: 4, attack: 0.85), (part: Eye, weight: 3, defence: 0.9), (part: Leg, weight: 3, pace: 0.85)])`, hashed like the rest of `saga`. The epithet rows join `saga.epithets`, and the words (notes, epithets, the `{maim}` slot) go in saga.ron.
**Determinism:** the new stream key `sim_military::saga::maim ^ SpawnId` at the tick of the blow; no existing stream moves. The roll is the serial `combat::maims` step, in SpawnId order.
**Hashing and saving:** `Service.maim`, `Fall`'s maim part, and `Veteran`'s three cached multipliers are hashed and saved. `Casualties.maim_checks` and `maims` are exempt scratch. `SAVE_FORMAT` goes up with the step.
**Performance:** per blow, one compare; per crossing (a few a tick at the busiest), one stream and a record, in a step that runs only when there's a check; the hot loops read two more fixed-point fields of `Veteran`, which they already hold, and the movement systems one optional read.
**Tests** (`tests/maims.rs`):
- `maims_follow_the_rule`:
  - a scripted blow past half health rolls, and the same seed gives the same maims;
  - a crossing on a tick when nobody dies is rolled on that tick;
  - no maim on a killing blow, below the threshold, or for someone already maimed (a second crossing in a later battle, set up by hand);
  - never a beast or militia.
- `maims_act_through_their_holder`: one-handed attack ×0.85, one-eyed defence ×0.9, and one-legged pace ×0.85 in a chase and a rout, exactly; a one-legged captain marches the army at full pace and routs at ×0.85.
- `maims_are_exact_no_ops`: outside a run, and for anyone without a maim, bit-identical blows and movement.
- `maim_words_and_visuals_agree`:
  - the note names the part rolled;
  - the epithet follows rank;
  - a maimed victim's later wound never takes the lost part;
  - the renderer's request carries the same part.
- `save_load` round-trips the records; `bot_run` prints maims per seed.
**Built:** plan step 8b, at the end of part 1, after the epithets it extends.

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

Defaults from the second review, of the follow-up against the code (change any):

33. **A rescue needs a fight:** the answering army fought raiders or the rival within `threat.radius` of the village, and its heroes come from those who fought; otherwise nothing is recorded (section 14).
34. **One line per death:** a lethal mishap is told by its `Fall` alone; the hourly hunger sum leaves out anyone told by name (sections 2 and 11).
35. **A mishap counts only for its drawn subjects,** so army-wide moods never earn anyone the Unlucky or Bread-Belly, and the dice loser is the dice's subject (sections 6, 7 and 11).
36. **Mishaps leave you in control:** a wedding's halt ends at a new order or an approaching foe and never delays a march to a village asking for help; the goose rout is a short timed flee, and the sleepwalker never wakes near a foe (section 11).
37. **The side is drawn first** (yours 2 in 3), and the 8x cap on saga lines is a hard throttle in the feed, on every difficulty (sections 11 and 15).
38. **Feed lines are fitted at read time:** each variant fits 72 characters with median names, each key has a `terse` form that fits with the longest, and long forms serve reports, the inspector and biographies (section 3).
39. **Quirk data is split by crate:** the deck and villager effects in `villagers.ron`, the soldier effects and mishap invitations in `progression.ron` `saga.quirks` (section 10).
40. **State that keeps its word:** the Sweetheart's walk home goes through the hashed `Follower.deserting`; moods name people by SpawnId and `NameId`; `Saga` hashes its in-place parts in full; the cached nudges are fixed-point; `Veteran` gains `joined` (sections 10, 13 and 17).
41. **NFR-09 measures live ticks only,** and in a played run gates the saga's own spans (section 17).

Yours, binding (combat spectacle, 2026-10-12):

42. **Gore particles, render-side only:**
    - blood spatter on hits and bigger bursts on kills;
    - blood pools that fade in about a game hour;
    - severed head, arm and leg chunks that arc, bounce and settle on told kills;
    - bodies that linger where they fell and fade, after the sim despawns the dead.

    The body part shown is the one the kill line names; both are worked out from the kill record (section 20).
43. **Over the top in the brutal tone** (Battle Brothers, Dwarf Fortress). The mild tone shows no blood and no parts: dust and falls only.
44. **Maimed survivors are a sim fact.** A survivor of a bad wound can lose a hand, an eye or a leg. They get a small stat penalty, a service-record line and an epithet (One-Hand, ...), and kill lines and visuals match. It's deterministic, hashed and saved, with run-only effects like the rest of M4c (section 21).
45. **Kill cam:** about a second of slow motion and a camera nudge when a legend, the champion or a captain dies near the camera. It has an options toggle and never plays during 8x fast-forward. Slow motion is a UI time-scale effect (`Time<Virtual>`), never sim state.
46. **bevy_hanabi is approved** as a new dependency: pinned `=0.19.0`, `default-features = false`, `features = ["3d"]`, in game_render only ([ADR 0019](adr/0019-combat-effects-and-bevy-hanabi.md)).
47. **One effects layer with three swappable backends,** in your words: the game turns sim events into render-side effect requests ("blood burst here", "sever arm", "pool", "corpse"), and a backend draws them:
    - **GPU** (Hanabi) when the adapter supports compute shaders: the full spectacle;
    - **CPU fallback** (our own small instanced-mesh droplets, a few dozen per burst) when compute shaders are missing (WebGL2, old drivers) or a software adapter (wgpu `DeviceType::Cpu`: lavapipe, llvmpipe, WARP) is detected;
    - **Off** for headless builds, tests, CI, or the player's choice.

    "Hanabi does only the droplet spray; severed parts, pools and lingering bodies are a handful of ordinary meshes per battle drawn identically on every backend, so only spray density degrades." Selection is automatic from the adapter's capabilities and type, overridable by an Effects setting (Full, Reduced, Off) and a command-line flag that forces each backend. The sim, saves and replays are untouched. Tests and CI never need a GPU (headless builds don't add render plugins, and Hanabi's `gpu_tests` feature is off). The CPU fallback gets a frame-time check on the software renderer.

Defaults for the combat spectacle (change any):

48. **One function names the part** (`Teller::told_wound`, built on `saga::wound`), for the feed and the renderer alike, from the record alone. The `wounds` table is words (saga.ron), not sim data. Fixed text names no body part, and mild never uses `{wound}` (section 20).
49. **A `Fall` gains its position (`at`), the victim's role and maim, and the killer's epithet as it stood;** `Casualties` gains `struck` (this tick's landed blows) and the victim in `fallen`. These are the only sim-side additions for the effects, and the `Casualties` ones are exempt scratch.
50. **Hanabi is added in `EffectsPlugin::finish`,** only with compute shaders: `app.add_plugins(HanabiPlugin)`, then `HanabiPlugin.finish(app)` by hand. This needs no direct wgpu dependency, and Bevy still picks the adapter it can present with. The alternatives are kept as fallbacks: creating the renderer by hand (`RenderCreation::Manual`), or probing the adapter before the app with a direct wgpu dependency, which would need your OK (open question 20).
51. **The machine's choice:** Full on a hardware adapter with compute shaders, Reduced otherwise. Software adapters with compute still load Hanabi, so Full stays possible there, and the setting switches at runtime.
52. **Effects only inside the detail band;** bodies also in the low-poly band. Per-frame burst caps are ranked told kill > kill > maim > hit, nearest first, with the caps and budgets of section 20 in render.ron.
53. **Lifetimes are the later of game hours and a real-time floor:** pools 1 game hour (at least 12 s), bodies 3 game hours (at least 30 s). The effect clock follows game speed up to 1x and real time (`Time<Real>`) above it.
54. **Effects Off turns off the spray only,** as your decision 47 says: bodies, pools and parts are drawn on every backend, Off included, and the mild tone is the switch for gore. (The first version of this default had Off remove bodies too, which contradicted decision 47.)
55. **The kill cam slows to 0.25x for 1 real second,** nudges the camera 35% toward the death and zooms in 15%, fires at most once per 10 real seconds, at 1x–4x only. A pause the death brings waits for it (open question 21).
56. **Maims:** a blow past half a person's health this battle, one chance in four, hand, eye or leg (4 : 3 : 3), at most once in a life. The penalties are attack ×0.85, defence ×0.9 and pace ×0.85 (a captain's only in a rout). They're recorded in every preset and act only in a run. Scar epithets (ids 27–29) rank between the mocking and the honourable ones (section 21).
57. **Effects, kill-cam and tone switches are pulled forward from M6's options screen** (the new-game screen and a small view menu), and stored in the same preferences file.
58. **`--effects-bench`** measures the effects at their caps without waiting for a clash, which matters on a software renderer.

Defaults from the review of the combat spectacle against the code (change any):

59. **Hits come from the sim's list of landed blows** (`Casualties.struck`, step 1's blow count), never from health drops seen by the renderer: a deserting recruit's health drops by 40 when they take civilian stats again (section 20).
60. **The collector copies only when `SimTick` has advanced,** so a finished run, whose combat steps no longer clear `Casualties`, never replays its last deaths (section 20).
61. **The maim roll is its own step, `combat::maims`, after `casualties`,** because `casualties` runs only on ticks when someone fell. A maim comes at most once in a life: wounds never heal, so a second crossing of half health would kill first (section 21).
62. **A lost leg slows a captain (and the champion) only in a rout,** because a captain's march pace is the army's (section 21).
63. **Every brutal telling of a severed death names the part** (feed variants, a `terse` twin with `{wound_terse}`, long forms, report lines, fate clauses), and names in a told line are frozen at the death (sections 3 and 20).
64. **Hanabi's capacities are per asset, in render.ron,** and the GPU burst caps (8 hits and 2 kills a frame) are sized to fit them: about 12 MB of particle buffers in all (section 20).
65. **No compile-time feature for the GPU backend.** It's one module, the only code that names Hanabi; a feature waits until a Bevy upgrade outpaces bevy_hanabi, so no untested code path rots in the meantime.
66. **The CPU and Off backends come first (plan step 3b), and Hanabi after the kill lines (step 4b),** so the first playable slice doesn't wait on the new dependency. Your RTX 4070's frame checks are handed over and don't block a step; NFR-10 is ticked only once they're in.

## Open questions (with recommended defaults)

1. **A light nemesis.** Should the enemy who killed a legend be named in later lines and on the end screen, with an Avenger deed for whoever kills them? This reopens FUN_DESIGN's "nemesis rivals: not adopted" in the smallest way. *Default: yes, words and the deed only, with no AI change.*
2. **Memorials with effects.** *Resolved 2026-10-12 by your decision 14:* M4c keeps render-only cairns and stones and records rescues; M4b raises sim memorials with small effects for rescues, which become tended graves (section 14, `docs/TOWNS_DESIGN.md`). Villagers mourning near a legend's cairn would still fit M5's memories (SIM-14).
3. **Gendered epithets ("Spearwife").** *Default: no. Everything stays neutral, and a test bans pronouns and gendered nouns. Tagging given names in `villagers.ron` is possible later; it's hashed data, so the village_day and famine tests would have to be re-run.*
4. **How graphic.** *Resolved 2026-10-12 by your decision 43:* over the top in the brutal tone, in words and visuals, with a grave register for militia and rebels. Wound detail now goes through `{wound}`, the same part the renderer shows (section 20).
5. **Kill lines outside runs** (the skirmish and `--battle`). *Default: yes for records and lines, no for effects.*
6. **A feed-volume option** (all, mine, legends) besides the tone. *Default: not in M4c; the speed filter covers it, and volume joins M6's options screen. The tone, Effects and Kill cam switches are the exceptions pulled forward from M6 (decision 57).*
7. **Switching tone mid-run.** *Default: yes, from the switch on the feed; the whole history re-renders.*
8. **A blow-by-blow battle log** (Dwarf Fortress's combat reports). *Default: not in M4c. If you want it later, it's a saved UI ring like the event feed, for your own armies only.*
9. **Kinslayer and blood killings in revolts.** They're dark, rare and memorable. *Default: yes.*
10. **Healing.** Wounds never heal, because the sim has no healing; a twice-wounded legend fights on with half their health or less. A maim (section 21) is for life by the same rule, and comes at most once in a life: after one, less than half their health is left, so losing another half kills them first. Measuring the threshold against health at the start of each battle would allow a second maim, but then the already wounded would be maimed more easily. *Default: leave it, and say so in the inspector ("fights wounded"); one maim a life; healing is a question for M4b's towns, and if it comes it heals health, never a lost hand, eye or leg.*
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
13. **Mishaps on your enemies.** Should raiders and the rival have mishaps too? *Default: yes, a third of them (the side is drawn first, so it stays a third however many armies each side has), so the odd luck isn't only yours; you see theirs when they're near your armies.*
14. **Deadly mishaps.** Should a mishap ever kill? *Default: yes, the slip and the feast only, one ordinary soldier, at most one a game day. They're the absurd deaths people retell. Say no and both become wounds.*
15. **Quirks before recruiting.** Should you see a villager's quirks before they join? *Default: yes, in the inspector; recruiting still takes whoever is willing, so it informs and doesn't choose.*
16. **Mocking epithets on legends.** "Odo Twice-Fled, legend of the warband." *Default: allowed. Any honourable epithet the legend earns replaces it, so it's rare.*
17. **Rivals across the lines.** Should a recruit whose household member serves the rival get a grudge? *Default: no new bond; the `blood` key already tells it when they meet.*
18. **Starting numbers for the new parts:**
    - quirks: one, or two with 0.4; the deck's weights and effects as in section 10;
    - mishaps: steady 16 hours apart, 0.3 an hour after that, 2 a day (about one a game day, 15 to 30 in a bot run); one death a game day; a food floor of one day;
    - weapons: 24 a run, 2 a battle;
    - bonds: two friends and one rival each; grief −0.04 for 2 days, `kin_killed` −0.08 and ×2 desertion for 3 days; moods clamped to ±0.08 and ×0.5 to ×2;
    - at most 2 mishap, weapon, kin and feud lines a game day passing the 8x filter.

    *Default: tune with the bot's counts (quirks, mishaps by kind, weapons, feuds, desertions by cause) and the playtest.*
19. **Gore on militia and rebels.** Their deaths take grave keys, which never use `{wound}`, so no part ever leaves them, and their lines stay grave. Should they bleed, in the brutal tone, like anyone struck? *Default: yes, sprays, bursts and pools (blood is physical, and a defended village shouldn't look bloodless), but never a severed part, so "over the top" stops at the soldiers, raiders and beasts.*
20. **How Hanabi is gated.** Decision 50 adds it in our plugin's `finish`, which relies on Bevy fixing the finish loop's range when it starts and accepting new plugins until it has finished (both checked in bevy_app 0.19.1's source). Bevy is pinned exactly, and step 4b's smoke runs guard it. The cleaner alternative probes the adapter before the app starts, with a direct `wgpu = "=29.0.4"` dependency (already in the lockfile, a new direct dependency nonetheless), and costs tens to hundreds of milliseconds at startup. *Default: decision 50, no new dependency; switch to `RenderCreation::Manual` (no dependency) if the smoke test ever fails, and ask before adding wgpu.*
21. **Slow motion before an auto-pause.** The champion's death, and your legend's, pause the game in the same frame today. *Default: the pause waits about 1.3 real seconds for the kill cam, during which the sim moves about 3 ticks. The alternative skips the slow motion for deaths that pause and keeps only the camera nudge, which runs on real time and plays over a frozen spray.*
22. **Severing rates.** About half of limb wounds and a third of head wounds on told deaths sever, and always on a single blow, a tier-A victim or a legend's kill. *Default: as written; tune on screenshots and the playtest ("grim, not numbing" applies to pictures too).*
23. **Kill cam for the enemy's captains.** A raider captain dying near your camera is a moment too, but captains fall often in a big run. *Default: yes, any faction's captains, champion and legends, with the 10-second cooldown; if it fires too often in the playtest, limit it to your own people and enemy legends.*

## Later (not in M4c)

- **Animals with names (M4b):** livestock and pets as agents. They make Fears geese literal, "trampled by sheep" possible, and give the dog at the rations a name.
- **Tall tales and rumours about you (M5):** villages repeating, and distorting, what your armies did, through faction knowledge (SIM-40).
- **Weather quirks (M4b):** Hates rain, and moods from cold and storms (SIM-05, SIM-06).
- **Stale food (M4b):** food decay (SIM-22) makes "the bread was stale" a real mood cause.
- **Drowning at a ford:** needs rivers or fords, which the island doesn't have.
- **Healing and festering wounds (M4b).**
- **Grief at home (M5):** villagers remembering their dead (SIM-14).
- **Hero memorials (M4b):** decision 14 and `docs/TOWNS_DESIGN.md`.
- **Decals and blood on things:** Bevy 0.19's forward decals need a depth prepass on the camera, and clustered decals need compute shaders, so pools are flat meshes for now; blood on figures, weapons and buildings waits for an art pass.
- **Figures with limbs:** a figure that can show a missing arm or leg, and ragdoll falls, need new figure art (M6's polish).
- **A web build:** Hanabi needs WebGPU; on WebGL2 the CPU fallback draws the spray, which section 20 already covers.
