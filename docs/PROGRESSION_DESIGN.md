# Design: progression, from wandering band to warlord

Status: **draft 2026-10-07, for review**. Nothing here is built or added to SEED.md until you approve it. It answers the M3 go/no-go playtest: the armies run well, but leading a big army isn't fun as the starting point. It should be the payoff you work up to.

Your answers (2026-10-07):
- **Start:** a wandering band. Small settlements and **monuments** around the region are the early targets. Each monument gives a small bonus (about 1%), so no single one is overpowered, but by the time you hold 25% of a region the bonuses add up to a real boost.
- **Progression comes from:** land and people, equipment tiers, and the champion and veterans. Renown and titles stay out.
- **Structure:** roguelite runs.
- **Roadmap:** this design doc first, then decide where it lands.
- **Run length: about one hour**, because roguelite fun comes from the early game and its luck.

## 1. The shape of a run

A run lasts **about an hour**. It's short, the first minutes are dense with choices, and when it ends you start a new seed rather than sinking an evening into one island.

- **Time.** A game day is 4 minutes at 1x, so an hour at mostly 2x–4x is about **30 game days**.
- **A small island.** A run is played on a `run` preset in `worldgen.ron`: about 4 regions and 25 settlements, roughly a quarter of today's 2048-tile island. The big island stays for stress tests and benchmarks (open question 7).
- **An antagonist on a clock.** A **rival warlord** holds a seat on the far side of the island and grows stronger as days pass:
  - their bands raid from about minute 20
  - their seat gains walls and a bigger garrison

  You win by taking their seat. You lose if your champion dies with no successor. The clock stops you turtling, and it makes the march of a big army the climax of every run, which is the end goal you described. In M4 the rival is scripted, like M3's raiders; a rival that plays by your rules comes in M5.

| Act | Minutes | You have | You do | It ends when |
| --- | --- | --- | --- | --- |
| **1. Wanderers** | 0–10 | The champion and ~10 followers, a few days' food | Forage, answer village requests, take the nearest **monuments**, pick up drifters | You hold your first settlement: taken by force, or invited in |
| **2. Holdfast** | 10–25 | A seat, 20–60 men, first veterans | Arm the men from the seat's smithy, take nearby settlements and monuments | You hold 25% of your home region: the first **dominion boon** |
| **3. Lord of the region** | 25–40 | Several villages, 1–3 captains | Keep villages loyal and fed, beat the rival's raiders, push into a second region | You're strong enough to march, or the rival's clock forces your hand |
| **4. The march** | 40–60 | An army of hundreds | March on the rival's seat and take it: the army game M3 already built | Win or lose |

Acts 1–3 are made mostly of systems we already have (villagers, food, raiders, recruiting, combat) used at a smaller scale.

**The world at the start.** A run needs hundreds of people to recruit by act 4, so the island can't start as all hamlets of 20–40 people. I recommend a seeded mix: mostly hamlets and villages, a few towns, and the rival's seat as the largest town. That reverses the 2026-10-06 decision that every settlement starts as a hamlet, for runs only (open question 6). Within a run, settlements still grow, shrink and burn.

## 2. The first ten minutes (early-game luck)

Variety lives at the start. Every draw comes from `SimSeed` streams, so the same seed always gives the same start, and a seed worth sharing is a seed worth replaying (GP-20).
- **Where you wake up, and with whom.** The start spot can be a coast, a forest edge or the hills. The band's make-up varies: six hunters and four farmhands, or eight deserters with spears. The champion starts with one trait.
- **Starting boon.** Before the first tick, pick one of three, for example:
  - Well fed: +3 days of food
  - Armed: spears for everyone
  - Known: goodwill in the nearest village
- **The neighbourhood.** Two or three monuments lie within the first day's march, each with a different type and guard, so the opening route is your first real decision.
- **Run modifiers.** One or two per run, drawn from a deck and shown at the start, for example:
  - a hard winter
  - rats in the granaries
  - a bumper harvest
  - mercenaries for hire
  - a hero already risen
- **Early pressure.** The pacing director (GP-16) makes sure a village request or a raider band turns up in the first few minutes.

**Rhythm.** Aim for a meaningful pick about every 10 minutes:
1. the starting boon
2. the champion's ranks
3. the dominion boons
4. whatever village requests and monuments the map throws at you

## 3. The wandering band (act 1)

- **No land, real hunger.** The band carries food in its baggage and forages from wild land (forest and grass yield a little; `progression.ron`). It buys from villages with goods, or seizes food and makes enemies. Food is the clock of act 1, which uses the M3 upkeep, morale and desertion systems unchanged.
- **Standing with villages.** Each settlement keeps a goodwill score toward each faction. This is M4's loyalty, extended to settlements you don't own. Helping raises it, plundering lowers it, and word spreads to neighbours.
- **Village requests.** A village under strain posts a request that comes from its real state, never at random:
  - raiders nearby: drive them off
  - harvest season: guard the fields
  - a carter heading out: escort it
  - hungry: bring food

  Rewards are food, goods (tools, weapons), recruits who volunteer, and goodwill. A village whose goodwill passes a threshold may invite you in as its lord, so you take it without a fight.
- **Drifters.** Deserters, the survivors of raided hamlets, and young people from crowded villages join a band they've heard well of. Growth comes from people, not points.

## 4. Monuments

Monuments are fixed sites placed by worldgen: about 6 per region, so about 24 on a run's island (`worldgen.ron`). Each has a type, a guard, and a bonus for whoever holds it.

| Type (working names) | Bonus while held (each) |
| --- | --- |
| Standing stones | +1% morale |
| Old watchtower | +1 tile of sight and scouting (M5 knowledge) |
| Wayshrine | +1% piety in owned settlements (M4 faith) |
| Spring or holy well | +1% food yield in owned settlements |
| Ruined fort | +1% defence for your men |
| Ancient forge | +1% equipment production speed |
| Old road marker | +1% march speed |
| Market cross | +1% trade value |

- **Taking one.** Some are unguarded and you hold the site for a few hours. Others have a guard to beat: a raider camp, a hermit's militia, or a beast's lair (a raider variant with no food needs). Guards get stronger the farther the monument is from your start, so a band of 10 can take the near ones on day one.
- **Holding one.** It stays yours until an enemy army holds it longer than your garrison or neighbours do. Unguarded monuments far from your land drift back to neutral over time (open question 3), so the map doesn't stay painted forever.
- **Stacking.** Bonuses of the same type add up, with a cap per type in `progression.ron` (default +10%). Holding every monument of a type in one region adds a small **set bonus** (+2%). It adds variety without a big number.
- **Visible.** Monuments are landmarks on the map, show their bonus and holder on hover, and appear in an overlay and the chronicle ("Brand's band took the Grey Stones from the raiders of Oakworth").

With about 6 monuments per region, a quarter of a region holds one or two of them: about +1–2% here and there. That's a nudge, not a boost, which is why the next section exists.

## 5. Dominion boons (the 25% moment)

When you hold **25%** of any region's population, and again at **50%**, you choose **one of three boons** drawn from a deck (`progression.ron`). That makes three to five boons in a typical run. This is the roguelite choice moment. The boons are bigger than monuments (+5% to +10%, or a new capability), and the three on offer reflect what you hold:
- Hold many farms: "Granary of the region: +10% food yield in owned settlements."
- Hold a forge: "Smiths' guild: tier 2 equipment one settlement tier earlier."
- Have fought often: "Hardened: veterans rank up 25% faster."
- Hold a shrine: "Pilgrims: piety +10%; heroes rise sooner."
- Hold the region's capital: "Seat of power: +1 captain slot."

Boons are kept for the run, even if the region is later lost (open question 2). Monuments plus boons give you the "good boost" you described at 25%: steady small bonuses on the way, and a felt jump at each threshold.

## 6. Land and people: what you can field

There are no points. What you can field is the land you hold:
- **Men.** Followers eat. An army bigger than its food dies or deserts (M3). The real limit is the food your settlements can spare.
- **Captains.** One captain slot per N people in settlements you hold (`progression.ron`, default 300), up to the 8 of GP-01. Your first lieutenant comes with your first village, not with a level-up.
- **Recruits.** Recruiting asks real villagers (M3). Loyal, fed villages give more and better recruits. Starved or plundered villages give few, and those few desert.
- **Garrisons and loyalty.** M4's conquest and loyalty rules decide whether what you hold stays held. Overextension is the brake: a large realm of hungry villages revolts.

## 7. Equipment tiers

Equipment is goods made in settlements you hold. A higher tier needs a bigger settlement (M4b tiers) and the right workers and materials. The `weapons` good splits into one good per tier (`economy.ron`).

| Tier | Gear | Made in | Needs |
| --- | --- | --- | --- |
| 0 | Tools and clubs | Anywhere | What villagers carry |
| 1 | Spears and shields | Hamlet with a smithy | Smith, wood, ore |
| 2 | Bows | Village | Bowyer (new occupation), wood |
| 3 | Mail and helms | Town | Smith, ore, leather (tannery) |
| 4 | Siege engines | City | Carpenters, wood, rope (GP-08, GP-15) |

Each tier sets attack, defence and range in `army.ron`. A soldier's gear is visible on the figure and in the inspector. The M3 `Equip` order hands out the best gear in the baggage. Bows add ranged attacks, the first new combat mechanic.

## 8. Champion and veterans

- **The champion** is you: a named figure who fights, leads (the M3 leadership bonus) and can die. They rank up from what they live through:
  - battles survived
  - monuments taken
  - villages that invited them in

  At each rank you pick **one of three traits**, extending SIM-34: Brave, Beloved, Feared, Forager, Quartermaster, Duelist and so on. Each nudges stats or AI by a few percent.
- **Succession.** If the champion falls and a captain lives, the most senior captain becomes the champion with half their traits, once per run. With no captain, the run ends.
- **Veterans.** Every soldier ranks from Recruit to Soldier, Veteran and Elite by battles survived (`army.ron`). Each rank adds attack, defence and morale, and lowers desertion. Elites are named in the chronicle, and losing them hurts.

## 9. The roguelite layer

- **Each run is a fresh seed.** The island, regions, monuments, rival, starting boons and modifiers differ. The seed is shown and shareable (GP-20).
- **Quick to restart.** When a run ends, you get a summary: how it ended, the chronicle's highlights, what you unlocked. A new seed is a click away, and the small island generates in seconds.
- **Kept between runs (meta-progression):** unlocks, not raw power, so a run is still won by play.
  - **Origins** (GP-18): bandit chief, exiled lord, prophet and merchant each start the band differently. The first is free; the others unlock by deeds, for example "hold 50% of a region" unlocks the exiled lord.
  - **Starting trait choices** for the champion, unlocked by traits earned in past runs.
  - **New monument and boon cards** added to the deck as you see them.
  - **A legends book** of past runs: the chronicle of each run, its champion, how it ended.
- **Difficulty** comes from the pacing director's personalities (GP-16): steady, chaotic, harsh.
- **Saving.** One save slot per run, to stop and resume (GP-12). Optionally ironman: no reloading (open question 4).

## 10. How it fits what's built (technical notes)

- **Determinism.** A run is defined by its seed plus its run config: origin, unlocked cards, difficulty. The config goes in the replay header, so replays and shared seeds reproduce exactly. Meta-progression lives in a profile file outside the sim. It is read only to build the run config and never touches the state hash.
- **Choices are commands.** Picking a boon or a trait is a `SimCommand` (new kinds), recorded in replays like any order. Each card offer is drawn from `SimSeed::stream`.
- **New data.** `assets/data/progression.ron` holds the monument types and bonuses, caps, boon deck, dominion thresholds, captain slots per population, ranks and traits. Monument placement goes in `worldgen.ron`, and tiered goods in `economy.ron`. Every field gets documented in docs/dev/DATA.md, which the docs test enforces.
- **Bonuses** are one `FactionModifiers` resource per faction: hashed, rebuilt each day from held monuments, boons and traits, and read by the systems they affect (food yield, morale, march speed...). One place to inspect "why is my army faster?".
- **Performance.** A run's island is smaller than today's, and act 4's battles are the loads already measured (RND-04, NFR-03 pass). M5's 250,000-person island isn't needed for runs (open question 7).

## 11. Proposed roadmap change

Nothing moves until you approve. My recommendation:

| Milestone | Was | Proposed |
| --- | --- | --- |
| **M4** | Conquest and faith | **Rise**: one complete, playable hour. It includes:<ul><li>the `run` island preset, the start draws (starting boon, modifiers) and the wandering band</li><li>village standing and requests, monuments, conquest, ownership and loyalty (from M4)</li><li>dominion boons, captain slots from land, champion ranks, veterans and traits (SIM-34)</li><li>the scripted rival seat and clock; a run's start, win, lose and restart</li><li>save and resume (GP-12), the event feed and chronicle (UI-05, UI-09), the pacing director (GP-16)</li></ul> |
| **M4b** | Living towns | Living towns, with growth tuned to a 30-day run, plus **equipment tiers 1–3** (GP-08 moves up from M5) and faith, piety and heroes (moved from M4) |
| **M5** | Scale and information | **The rival plays by your rules** (GP-09 replaces M4's script), sieges and tier 4 (GP-15), knowledge and messengers, dilemmas. The 250,000-person LOD work becomes optional (open question 7) |
| **M6** | Campaign | **Roguelite layer**: origins, unlocks, the legends book, run summary, heraldry, shared seeds, onboarding, audio |

The M4 acceptance test would be a playtest of whole runs: "a run from 10 men to the rival's seat takes about an hour, and two seeds play noticeably differently in their first ten minutes", plus scenario tests for each system and a headless bot run that checks a run can be won.

Proposed new requirement IDs, added to SEED.md on approval:

| ID | Requirement |
| --- | --- |
| GP-21 | A run lasts about an hour on a small island and a fresh seed. It starts with the champion and a small band with no land, and ends in a win (the rival's seat taken), the champion's death with no successor, or abandonment |
| GP-27 | Start draws: a starting boon picked from three, the band's make-up, and one or two run modifiers, all from the seed |
| GP-28 | A rival warlord's seat grows on a clock and raids from mid-run; taking it wins the run (scripted in M4, rules-based in M5) |
| GP-22 | Villages hold goodwill toward each faction and post requests arising from their state; goodwill can make a village invite a lord |
| GP-23 | Monuments: worldgen places typed sites with guards; holding one grants a small capped bonus to its holder |
| GP-24 | Dominion boons: at 25% and 50% of a region's population held, the player picks one of three boons |
| GP-25 | Captain slots come from population held |
| GP-26 | Meta-progression between runs unlocks origins, trait choices and cards, never raw power |
| SIM-35 | Champion ranks with trait picks, and succession to a captain once per run |
| SIM-36 | Soldiers rank up by battles survived; ranks change stats and desertion |
| SIM-37 | Equipment tiers as goods made by settlement tier; gear sets combat stats; bows are ranged |
| SIM-38 | Wild foraging feeds a landless band at a small rate |

## Open questions

Answered: **run length is about one hour** (2026-10-07). Unanswered questions keep the default shown.

2. **Boons when land is lost.** Are boons kept for the run (default, since loss is already a punishment), or lost when you drop below the threshold?
3. **Monuments drifting back.** Should far-off monuments you don't garrison drift back to neutral (default)? In an hour-long run this may be unnecessary, so the alternative is that they stay yours until taken.
4. **Saving.** A resumable save per run (default), or strict ironman with no reloading as an option?
5. **Faith and heroes.** Move them to M4b (default), or keep them in M4?
6. **The world at the start.** For runs, a seeded mix of hamlets, villages and a few towns (default, so there are people to recruit by act 4), or all hamlets as decided on 2026-10-06, with growth sped up to fit 30 days?
7. **The big island.** With hour-long runs on small islands, should M5's 250,000-person island and its LOD work become a later "grand campaign" mode (default), or be dropped from the roadmap?
