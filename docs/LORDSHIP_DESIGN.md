# Design: Lordship, towns that answer back

Status: **draft 2026-10-13, revised the same day after a review against the code, for your approval.** It answers the M4 playtest's first verdict (`docs/M4_PLAYTEST.md`) the way `docs/PROGRESSION_DESIGN.md` answered M3's. Your binding decisions of 2026-10-13 are recorded in the Decisions section at the end, and every open question there has a recommended default. The milestone is **M4a "Lordship"**, planned in `docs/M4A_PLAN.md`, with its section ("M4a: Lordship") and new requirement rows in `docs/SEED.md`, and its decisions in [ADR 0020](adr/0020-lordship.md) (proposed). Grounded in the code at c20cc65; types marked **(new)** don't exist yet, and everything else is named as it stands.

The verdict, verbatim:

> "game is mildly interesting in concept; i need something more than moving an army around we need to be able to influence the cities or something, the game feels like we are running around taking points like king of the hill but my actions feel unweighted."

SEED.md named this risk on day one: "The biggest risk is design, not engineering: a huge simulation that the player cannot read or influence meaningfully."

Your answers (2026-10-13):
- **Milestone:** a new milestone, **M4a "Lordship"**, before M4c "Saga". The order is: M4 playtest, M4a Lordship, M4c Saga, M4b Living towns, M5, M6.
- **(a) Passive influence and strength,** in your words: "influence SPREADS from the towns you hold on its own: nearby free towns warm to you and may join without a fight; your holdings feed your strength (levies, tithes, renown-like power) without micromanagement; a visible influence map shows your sphere growing (and contesting the rival's)." The first draft dropped "renown-like power" in favour of PROGRESSION_DESIGN's earlier answer that renown and titles stay out, without asking. It is now an open question with a recommended reading (question 16, section 6): the power of a realm without a renown number or a title ladder.
- **(b) Conquest fallout, both ways.** Taking towns, especially by force, raises hostile bands: broken militia and dispossessed men who harass the rival's lands harder but may hit yours too, and rival reprisal raids against your newest holdings. Expansion has ripples and costs.
- **(c) Ways to take a town**, each leaving a different lasting state: conquer (fast, bloody, a resentful town), intimidate (a much stronger army demands submission: tribute, and it turns on you if you look weak), court (gifts, answered requests, protection, then an invitation: loyal, but it expects defence) and subvert (agents stirring the rival's unhappy towns: slow, cheap, a flip without a fight). Starving a town out stays in M5.
- **(d) Govern your towns.** Per-town edicts with visible trade-offs that change what villagers actually do (a food or weapons tithe, a recruit quota, rationing, a work focus on farms, forge or walls, a festival); building orders (a smithy that really supplies weapons, a granary, a palisade, a watchtower); a named steward whose traits matter; specialised towns worth targeting (mining, fishing, market...).
- **Not now:** faction-wide reputation and village memories (M5), and obligations and dilemmas (GP-14, M5). Lordship leaves hooks for them (section 14).
- **Overlap:** M4b already plans town policies (GP-17), construction, equipment tiers and carters. Lordship pulls forward only what it needs (edicts, a minimal building order, the smithy and weapons chain) in a form M4b grows from. M4c follows Lordship now, not the M4 playtest directly.

## 1. What is wrong, and what "weighted" means

**What the code does today** (read at c20cc65):
- **A town is a flag.** Taking one calls `sim_agents::set_owner` (`crates/sim_agents/src/standing.rs`). That flips `Owner` and reseeds every resident's `Loyalty`. Nothing a villager does or makes depends on who owns the town: not the work, not the stores, not the buildings, not the population.
- **Every town is taken the same way.** An `Attack`, then hold the centre for two hours (army.ron `conquest`). Invitations exist (`invitations.rs`), but they need a player captain within 40 tiles, so they feel like an Attack with a detour.
- **Once a town is yours there is nothing to do there.** `CommandKind` (`crates/sim_core/src/command.rs`) has no verb aimed at a town, and the settlement panel (`crates/game_ui/src/panels.rs`) is text with no buttons.
- **Causes are invisible.** Goodwill deeds are typed (`Deed`: trade, seize, plunder, provoke, killing, request...), but the ledger is cleared every tick, and nothing shows why a town feels as it does.
- **The economy can't arm you.** Measured on the M4 build: about 10 weapons on seed 1's whole island in 3.3 days, and none on seed 7. Only the second smithy of a town makes weapons, smithies stall full of one input, smiths with full hands loop, and finished weapons wait at the forge where Equip never looks (section 8.6). Holding a smithy town gives you nothing.
- **Nothing ripples.** The rival never answers your conquests, and in a run raiders prey on you alone (`raiders.rs`; BUILD_GUIDE D2).

**The goal: a town is a place you shape, not a flag you flip.** Lordship adds three loops beside the army loop:
1. **Take.** How you take a town leaves a lasting state with its own costs and duties.
2. **Govern.** Edicts, a steward and a building order change what real villagers do, and food, arms and men flow out of your towns to your armies.
3. **Radiate.** What you hold pushes outward as influence, contested by the rival's. Violence pushes outward too, as bandits and reprisals.

**The weight test.** An act feels weighted when it has all four of these. Every Lordship verb is designed against this table, and a verb that can't fill all four columns doesn't ship:
1. **Foresight:** you see its cost and likely result before you commit.
2. **A visible consequence:** something changes on the map, in people, within about a game hour (10 seconds at 1x), not only in a panel number. A slower verb still shows its first step inside the hour and a countdown for the rest.
3. **An echo:** the town or the world remembers it later (a dated line in the town's "why", a band of broken men, a vow of reprisal).
4. **An opportunity cost:** it closes or delays something else (men left behind, food spent, a dial locked for two days, one of two building slots).

| Verb | Before you commit | Within the hour, on the map | Later (the echo) | What it costs you |
| --- | --- | --- | --- | --- |
| **Attack** (conquer) | the militia and garrison; "about 11 will take to the woods" | the militia rises and breaks; the hold | a sword on the label; resentment fading over about ten days; broken men walk out; a vow if it lay in the rival's shadow | men, goodwill, a garrison for a week |
| **Attack at Aggressive** (sack) | adds the loot and "deeper resentment, more broken men" | the storehouse emptied into your baggage | resentment for weeks, a bigger band, neighbours hear of it | the town's love, and likely the town, unless held hard |
| **Demand submission** | "need 34 strength within 40 tiles, you have 37" | an hour's parley with your army at the gate, then a kneeling or a refusal | tribute on every pass; it tests you if you stop holding the land around it | ten days of holding the ground near it: an army, a garrisoned neighbour or your sphere |
| **Gift food** | "+0.10 goodwill here" | a feed line; the storehouse fills | progress toward an invitation | food from your baggage |
| **Send an agent** | "Hob goes; 10 food; about 6 days; suspicion now 0" | a man walks into the rival's town and stays | its loyalty slides on its panel; it opens its gates, or Hob hangs and the rival vows revenge | a man, goods and days |
| **Accept an invitation** | "Sworn: loyal, and they will expect you to defend them" | your colour goes up | a duty: a raid there left unanswered is a deed against you | an obligation |
| **Work: Forge** | "about −12% food; weapons from the third dawn" | within the hour the first smiths walk to the forge, more each dawn | arms in the tithe; food security, and so loyalty, sags | food, and the dial locked for two days |
| **Dues: Levy** | "3 men a day; loyalty −0.08" | the first men muster on the green within the hour | they join your next captain within reach | loyalty and workers |
| **Table: Feast** | "half a day's food; +0.08 goodwill here and word spreads" | the town gathers on the green for a day | goodwill here and nearby; a better loyalty target for days | food, then six days before the next |
| **Order a building** | wood and ore, hours, and which of two slots | a scaffold; carters haul wood to it | its effect (weapons, a food floor, stakes, sight) | the town's wood and ore, one of two slots for the run |
| **Appoint a steward** | "Wat Cooper, Veteran, a carter: Stern, Reeve; he will set Levy at dawn" | he walks home to the hall | his habits act on the dials you leave alone, sometimes against you | a veteran out of your army |
| **Override a steward** | "Wat will set Levy at dawn; you keep Light" | the pending habit is struck from the label | the dial is yours and locks like any other you set; what he would have done is in the log | his habit's gain, and the lock |

**Which verbs are deliberately army-at-a-point, and which are not.** The verdict was "running around taking points", so the mix is stated, and the playtest judges it:
- **Army at a point, by design:** Attack (conquer or sack), the Demand parley (one hour at the gate), and Gift food (a captain's `Give`). They are the opening moves and the way into the rival's heartland and its seat.
- **No army needed:** influence and its envoys, the Court path of answered requests and defended towns, an agent once placed (he works for days with no army near), every edict, the steward, a building order, the realm that thins the seat's defence (section 6.1), and the watch on a submitted town, which asks you to hold the land around it, not to park (section 5.2).
- **The target:** from the second town on, most of a run's days are spent on the second list. If the playtest still reads as running between points, the first list is too long and the answer is more of the second, not fewer verbs.

## 2. Name and place

**"M4a: Lordship".** The letters in M3b, M4b and M4c recorded the order milestones were added, not the order they are played, and M4c already comes before M4b. "M4a" is unused anywhere and gives the play order M4, M4a, M4c, M4b, M5, M6, which reads in order. "M4d" would follow the order-of-addition habit but would read as coming after M4c. "Lordship" already appears in the code for invitations ("offers lordship", `invitations.rs`, the GLOSSARY's Invitation), so the name fits what exists.

Files: this design, the plan `docs/M4A_PLAN.md`, [ADR 0020](adr/0020-lordship.md), the playtest checklist `docs/M4A_PLAYTEST.md` (written at the plan's last step) and step notes in `docs/m4a/`.

## 3. How a run plays with Lordship

A game day is 4 minutes at 1x (PROGRESSION_DESIGN section 1), so about 1 minute at 4x, and a run is about 30 days.

| Act | M4 today | With Lordship |
| --- | --- | --- |
| **1. Wanderers** (days 0–3) | Attack the nearest hamlet | Court a liked hamlet (a gift and an answered request bring an invitation) or demand a small one's submission with your whole band; or conquer it and watch its broken men take to the woods. Your first town comes with a steward of its own people, whose habits run it until you say otherwise. |
| **2. Holdfast** (days 3–10) | Attack town after town | Set Forge at the smithy town and weapons start to reach your men. Tithes and levies feed and fill the armies that pass through your lands. Your influence warms free neighbours, and one invites you with no army near. The rival vows revenge on your newest town in its shadow. |
| **3. Lord of the region** (days 10–20) | March toward the rival | The spheres meet on the influence map. An agent stirs a hungry rival town until it opens its gates. Bandits made of the men of towns you took plunder the rival's storehouses. You order a palisade at the frontier and hold what the rival tries to take back. |
| **4. The march** (days 18–30) | Conquer the seat | The seat can only be conquered, so this stays the climax. You come armed by your forges, fed by tithes and swollen by levies, against a garrison thinned by its own reprisals, behind a wall that sank as you pulled its towns away (section 6.1). |

**At four time scales:**
- **The minute** is unchanged: armies, posture, fights.
- **The hour: you reach a town and choose how.** Every town on your route asks a question: take it by force, make it kneel, win it over, or turn it from inside? The right-click menu and the panel show what each way would cost here, against this town's militia, mood and owner. Points become places, and each place asks something different.
- **The day: the realm answers.** An edict's first wave moves within the hour you set it; at dawn the later waves go, stewards act on their habits, the sphere's edge moves, bands rise and vows are made. The dawn news arrives as one grouped "Dawn" line in the feed, not a stream of alarms. You make one or two town decisions a day.
- **The run: your sphere against the rival's.** A field of your colour grows from your towns and pushes against the rival's around its seat. Acts 2 and 3 are about that frontier: make your towns strong and loyal so they pull their neighbours, rot the rival's towns with bandits and agents, and hold what the rival tries to take back.

**A short example** (seed 1, steady; the names are seed 1's, the numbers plausible, not measured):
- **Day 1.** You picked *Known*, so Nettlebrook (fishing, 187 people) already thinks well of you: its panel reads "+0.40, heard well of you (day 1)", and *Court: 0.40 of 0.50*. Raiders near Woldbrook post a drive-off request; your eleven men and the village's militia see them off (+0.20 there, +0.05 heard at Nettlebrook). At dusk you gift Nettlebrook a fifth of a day's food (+0.05). That was food you needed for the march.
- **Day 2.** At dawn Nettlebrook offers lordship: "Sworn: loyal, starts at 0.75; they will expect you to defend them." You accept. Tarnridge (140 people, defence 24) is next. Demanding its submission needs 36 strength and you have 21; ten recruits from your new town would make 31, still short, so you recruit them and attack. The militia breaks after a hard half hour; you hold the centre, and eleven of its men are dead. "Tarnridge is yours, by the sword." An hour later: "Eleven men of Tarnridge take to the woods." You watch them walk east, away from your towns, toward the rival's lands.
- **Day 3.** Tarnridge's panel: *loyalty 0.34, heading for 0.42: food +0.55, treatment +0.10, time 0, resentment −0.23 (fading)*. One hungry day would tip it, so your garrison stays. Its steward, a farmer of its own (Husband, Native), would set Fields at dawn and feast whenever it had two days of food, and the panel says so a dawn ahead; with the town this thin you want a firmer hand. You appoint Wat Cooper instead, a veteran carter of Nettlebrook (Stern, Reeve), and lose him from your band. The panel now reads "Wat will set Levy at dawn"; you override it to Tithe, the Reeve's other habit, since a levy is the last thing a sullen town needs. That evening: "Tarnridge's broken men plunder Sedgeholt (the rival's)." Sedgeholt's panel turns hungry and its loyalty starts to fall.
- **Day 10.** You hold six towns. Woldbrook invited you on day 8 with no army near: your two towns' influence had warmed it past the line. Gorsefield (mining, 160 people) knelt on day 9 to forty men, ten of them armed (it needed 39), and its forge now sends weapons with every tithe; it counts as watched because you keep a garrison in Woldbrook, 60 tiles off. Your agent Hob spent six days in Fenstead, a rival timber town Tarnridge's bandits had plundered twice; its loyalty slid from 0.80 to 0.48, and it opened its gates. Then the red line: "The rival vows to retake Fenstead (by tomorrow evening)." Your champion's forty men are two days away. Gorsefield stays watched by Woldbrook's garrison, so they are free to go, but Tarnridge still needs its own. You set Fenstead to Walls, order a palisade, and turn the champion around. The stakes are half up when ten of the rival's men arrive; the champion's band arrives with them. "Fenstead held." The rival's seat has ten fewer men.
- **Day 21.** Thirteen of 25 towns: four by the sword (one sacked, for its stores and weapons, before the march), two knelt, five sworn, two turned. Four captains and 190 men, 120 of them armed from Gorsefield's and Nettleley's forges, march on a seat thinned by two reprisals. Its last levy pool was harried by bandits for a week, and it holds only 45% of the people it started with, so its defence stands at +0.5 where its clock says +0.75.

## 4. Tenure: what a town remembers about how it was taken

This is the shared machinery of the four ways to take a town (section 5) and the town's "why" lines.

**`Tenure` (new; sim_military `tenure.rs`; on every populated settlement in a run).** Fields:
- `how: TakenBy` (new, append only): `Native` (worldgen's own), `Conquered`, `Sacked`, `Submitted`, `Invited`, `Subverted`, `Granted` (the rival's setup), `Freed` (gone independent by revolt or defiance);
- `by`, `since` (the owner and the tick);
- `scar` (resentment, 0..0.45, fading daily), `blood` (militia killed in the taking), `stir` (an agent's work, section 5.4);
- `kept`, `broken` (promises of defence kept and broken, for an invited town), `weak_days` and `watched_hours` (a submitted town's watch, section 5.2), `cool_until` (a refused demand's cooldown);
- `history`: the last four `(how, by, day)` takings, a ring kept for M5's village memories.

It is written in one place: `conquest::transfer_seeded` (`crates/sim_military/src/conquest.rs`), already the single chokepoint for every change of hands, takes the `TakenBy` from the `Cause` (section 11 lists the new causes). `rival::setup` writes `Granted`, and `rival::free_lands` writes `Freed`.

**`LoyaltyTerms` (new; sim_agents `loyalty.rs`; run only).** An array of itemised terms, `[f32; LoyaltyTerm::COUNT]`, with `LoyaltyTerm` (new, append only): `Resentment`, `Awe`, `Contest`, `Bond`, `Dues`, `Table`, `Steward`, `Stirred`. `tenure::step` (progress slot 14, hourly) rebuilds it from `Tenure`, `Edicts`, `Influence` and the steward. `loyalty_target` gains one parameter, `bias`, the sum of the terms, added before the clamp to 0..1. Outside a run the component is absent and the bias is `0.0`, so the developed world is an exact no-op.

Today's target under an owner is `0.55 × food + 0.3 × (goodwill + 1)/2 + 0.15 × time` (villagers.ron `loyalty`). The terms add to it:

| Term | From | Value |
| --- | --- | --- |
| Resentment | `Tenure.scar` | `−scar`; Conquered starts at 0.12 + 0.01 per militia killed (at most 0.3), Sacked at 0.25 + 0.01 per kill (at most 0.45), Subverted at 0.1; it fades 0.02 a day |
| Awe | a submitted town while it is watched (section 5.2) | +0.1 |
| Contest | the other side's influence there (section 6) | up to −0.25; your heartland +0.05 |
| Bond | an invited town (section 5.3) | +0.05, +0.02 per defence kept (at most +0.1); −0.1 once forsaken twice |
| Dues | the dues edict (section 8.1) | Tithe −0.05 (0 if submitted: it's the deal); Levy −0.08 (−0.12 if conquered or sacked; 0 if invited) |
| Table | the table edict | Ration −0.1; after a feast +0.08 for five days |
| Steward | the steward's traits (section 8.2) | Native +0.05 ("one of us"); nothing else: the other traits act through habits, not numbers |
| Stirred | an agent at work (section 5.4) | `−stir`, up to −0.4 |

**Terms are named when they decide.** The small terms (Awe, Bond, Steward) would be invisible as bare numbers, so the feed names them when they flip an outcome. A revolt's chronicle words cite the two largest negative terms ("Tarnridge rose against you: hunger and resentment"). And when a town's mean loyalty would be under `revolt_threshold` without the Awe or Bond term, and isn't with it, the Dawn line says so ("Gorsefield stays loyal: awe of your army, +0.10" or "Woldbrook stays loyal: the bond you kept, +0.08"). One line a town a day at most.

**`DeedLog` (new; sim_agents `standing.rs`; run only).** A ring of the last eight goodwill changes at the town, `{day, faction, deed, heard, delta}`, merging entries of the same day, faction, deed and `heard` (word of a deed done elsewhere, through the existing spread). `apply_goodwill` writes it where present. It is what makes a town's mood readable:

```
Loyalty 0.47, rising to 0.53:
  food +0.55 · treatment +0.13 · time +0.05 · resentment −0.14 (fading 0.02 a day) · rival's pull −0.06
Thinks of you +0.21:
  day 6 you plundered us −0.10 · day 7 a gift from you +0.09 · day 8 you drove off raiders +0.10 · daily your influence +0.03
```

That is the genre's cheapest weight-giver (Crusader Kings' opinion lines, Stronghold's popularity, Total War's public order), built almost entirely from state that already exists.

## 5. Four ways to take a town

Every way ends in `conquest::transfer_seeded(world, settlement, to, cause, seed)` with its own `Cause`, so ownership still changes in exactly one place, and every way leaves its own tenure.

### 5.1 Conquer, or sack (the existing `Attack`)

- **The verb is unchanged:** march, provoke, break the militia, hold the centre two hours (×1.5 with a palisade).
- **Blood is counted.** Every militia death already posts `Deed::Killing` to its home town (`combat.rs`). The same serial spot adds one to the town's `Tenure.blood` in a run.
- **Posture at the hold decides the scar.** Neutral or Passive: **Conquered**. Aggressive: **Sacked**, and the hold's completion runs a plunder dealing in the same orders step, inside the army's logistics step: the storehouse's food (keeping nothing, as aggressive gathering does today) and its weapons and tools go into the baggage, as far as it has room, with today's `Deed::Plunder`. The forecast warns before you commit.
- **The sack is the player's rule.** It applies to an Attack by a player captain only. Every rival band is created Aggressive (`rival::raid` splits its bands at `Posture::Aggressive`), so a rule keyed to posture would sack every town the rival retakes, rewrite the rival's books and make a reprisal mean something the AI never chose. The rival's takes stay `Cause::Conquest`: a retaken town is **Conquered** by the rival, with the ordinary scar and no plunder. The alternative, a rival that sacks its reprisal targets ("they will sack it" in the vow line), would raise the stakes and is listed in the decisions (19); `tests/rival.rs` and `tests/take_modes.rs` both pin the choice.
- **The loyalty seed** stays `(goodwill + 1)/2`, which the provoke and killing deeds have already pulled down.
- **How it feels, with numbers.** A fed conquered town with goodwill −0.2 toward you and eight militia killed aims at 0.55 + 0.12 + 0 − 0.2 = **0.47**, from a seed of 0.4: it holds if fed and garrisoned for the first day, and revolts if starved (0.275 + 0.12 − 0.2 = 0.195). A sacked town aims at about 0.34 even when fed, so it rises unless held by a garrison bigger than its militia: fast loot, a town held by force.
- **Fallout:** broken men from its own people (section 7.1), and a reprisal if it was the rival's or lay in its shadow (section 7.2).
- **Why choose it:** it's the only way with a small army against an unfriendly town, it's fast, and it's the only way into the rival's heartland and its seat.

### 5.2 Intimidate (`OrderSpec::Demand { settlement }`, new)

- **Where:** independent towns and rival towns other than the seat, not your own, not while a refusal's cooldown runs.
- **The parley.** The captain walks within `recruit.radius` (14 tiles) and parleys for an hour: the army is visibly at the gate, and the town's threat rises from its posture as now. A hostile army within `threat.radius` breaks the parley off. At the end of the hour, in the settlement block of `step_order`:
  - **your strength** = your unrouted soldiers within 40 tiles, 1 each, 1.5 if armed (the `player_soldiers_near` pattern of `invitations.rs`);
  - **its defence** = 0.25 × the town's able adults (`conquest::would_be_militia`, made `pub(crate)`) + 1 × its owner's garrison, ×1.5 with a palisade;
  - **strength ≥ 1.5 × defence:** it submits: `transfer_seeded(PLAYER, Cause::Submission { captain }, Some(0.45))`;
  - **otherwise** it refuses: `Deed::Threat` (−0.05), two days' cooldown, and a feed line with the numbers ("Tarnridge refuses: 96 would fight; you need 36 within 40 tiles and have 21").
- **Scale.** A hamlet of 50 (about 30 adults) needs 11 strength, so a starting band can make one kneel. A village of 200 (about 120 adults) needs 45, or 30 armed men. A town of 400 needs about 90: a demand is how a big army pays off without blood. (Strength counts unrouted soldiers at 1, or 1.5 if armed, so ten recruits from a neighbouring town are 10 and ten armed ones 15.)
- **Submitted: tribute, and a watch over the land.**
  - **Tribute.** Its Dues dial is held at Tithe, with no Dues loyalty term, until it settles.
  - **Awe.** +0.1 to its loyalty target while it is watched.
  - **The watch, counted by the hour, not by a daily snapshot.** Each hour `tenure::step` marks the town *watched* if any of these holds: (1) your strength within 80 tiles (unrouted soldiers, 1 each, 1.5 armed) is at least 0.75 × its defence, so an army passing through counts for the hours it is there; (2) a town of yours within 80 tiles has a garrison of at least 4 soldiers; (3) the influence of your other towns gives you a share of at least 0.5 at the town (its own power doesn't count). A day with fewer than 6 watched hours is a *weak day* (`weak_days` grows; any other day resets it). At two weak days in a row it **defies you**: `transfer(INDEPENDENT, Cause::Defiance)`, a `Defied` event, and, if any of your soldiers stand at the town, its militia rises against you for `garrison.revolt_hours`. The label and panel read "Watched today: yes, 14 of 24 hours · weak days 0 of 2".
  - **Settling.** After ten days without defying, the tribute lock, the watch and the awe end; `how` stays `Submitted` as a record.
- **Costs:** ten days of holding the land around it, which a garrisoned neighbour or a sphere that has grown over the area does as well as an army parked beside it: the duty is to own the ground, not to sit on it. No blood, no broken men, no scar.

### 5.3 Court (gifts, defence and requests, then an invitation)

The parts exist (requests, `Give`, invitations). Lordship makes the path deliberate, legible and rewarded:
- **Gifts.** In a run, `Give` at a town posts `Deed::Gift` (new) of `0.25 × food given ÷ the town's need for a day`, at most +0.12, doubled if the town is hungry (food security below `requests.food_days`), at most once a town a day. It still fills an open bring-food request. Gifts favour hamlets: a fifth of a day's food is little to a hamlet and a lot to a town of 300. `Give` needs a captain at the town (`orders.rs`), so a gift is an army-at-a-point act (section 1); the courtship that needs no one there is influence, answered requests and a defended border.
- **Defence.** `Deed::Defended` (new, +0.1, at most once a town a day): when a player army ends a battle against raiders or the rival with at least one kill, and its captain stands within `threat.radius` (45 tiles) of an independent or player town, that town saw you fight for it. Not when a drive-off request there was answered this hour (that already pays +0.2). **And never for a battle whose foes include bandits whose home is that town** (section 7.1): they are its own sons, and beating them in the woods must not read as "you fought for us". It reads the hour's `BattleReport`s (their `at` and `foes`), so it needs no new tracking. Your own towns gain too: goodwill toward their owner is half of the treatment term.
- **Influence** warms the town every day you hold land near it (section 6).
- **The invitation** is today's (goodwill ≥ 0.5, the owner weak), plus two run-only branches: **envoys**, with no captain near, when the town lies inside your sphere (section 6), and **turned gates**, for an agent's town (section 5.4). The card says what accepting means: "Nettlebrook offers lordship. Sworn: loyal (starts at 0.75); they will expect you to defend them."
- **Invited (sworn): a bond, and a duty.** Loyalty is seeded from goodwill as today. Bond +0.05, growing +0.02 with every raid you drive off there (an answered drive-off or a `Defended` deed; `Tenure.kept`), up to +0.1. **It expects defence:** a drive-off request there that lapses (`requests::fail`) posts `Deed::Forsaken` (new, −0.15, which spreads: "word travels: you abandon your own") and counts `broken`. At two, the bond turns to −0.1 for good, and the panel says "You forsook us (day 9, day 13)".
- **Costs:** food, time, and a duty that bandits and reprisals will test.

### 5.4 Subvert (`OrderSpec::Subvert { settlement }`, new; rival towns only)

- **Where:** a rival town other than the seat, with no agent of yours already in it.
- **The dealing.** The captain walks within `recruit.radius` and pays the bribe from the baggage (10 food value, or 2 weapons or tools). The bribe goes into purses, not the storehouse. One villager follower, the lowest-ranked, leaves the army and walks into the town to live there: discharged into it (a new `upkeep::discharge`, factored out of `apply_desertions`), with a home with room, as an idle resident carrying `Agent { town, by, since, suspicion }` (new; defined in sim_agents `standing.rs`, so the villager AI can read it). No villager in the band means no agent ("nobody to send"). At most three agents alive.
- **An agent is nobody's recruit and nobody's militia.** He would otherwise be an idle civilian, and `recruit_candidates` sorts idle first, so the rival's levy (every two days, round-robin over its towns), drifters, request volunteers and your own Recruit would take him within days, and he would count in `would_be_militia` and fight as militia if you attacked. So `recruit_candidates` and `would_be_militia` skip a marked villager (`Without<Agent>`), and the villager AI never picks Fight for him: he keeps his head down. `tests/take_modes.rs` pins both (`a_rival_levy_never_takes_an_agent`, `an_agent_never_takes_up_arms`).
- **Daily, for each living agent** (in `tenure::step`, SpawnId order):
  - `Deed::Whisper` (new, no spread): −0.06 toward the rival (×1.5 if the town is hungry or the rival levied there in the last day) and +0.05 toward you;
  - `Tenure.stir` += 0.1, at most 0.4 (the Stirred term);
  - **suspicion** += 0.06 + 0.08 × the town's loyalty + 0.1 per 10 rival soldiers within 30 tiles (about 0.12 a day in a fed town with no rival soldiers near). A player captain's `Give` at the town takes 0.3 off, once a day ("a second purse").
- **Caught** at suspicion 1.0 (about ten days unvisited, about five with a rival band of ten parked within 30 tiles): the agent is killed (`Cause::Caught`), the town posts `Deed::Plot` (new, −0.15, which spreads) toward you, and the rival vows a reprisal against your town nearest it (section 7.2). Deterministic: no roll anywhere.
- **Turned.** When the town's mean loyalty is below 0.5, its goodwill toward you is at least 0.25 and it has no rival garrison (only the seat has one), `invitations::step` posts the invitation with no captain near: "Fenstead will open its gates to you. Turned: fragile, and the rival will want it back." Accept, and `answer` hands it over with `Cause::Subversion` and a seed of 0.5. The agent stays, now your subject (and still nobody's recruit).
- **Subverted (fragile).** Resentment 0.1, fading; for six days it is the reprisal's favourite target, and reprisals against it come half again as strong.
- **Arithmetic (checked: the loop closes with no visit at all).** A fed rival town at loyalty 0.8 aims at about 0.44 after four days of an agent: stir −0.4, and goodwill toward the rival −0.24, which moves the target only −0.04 (treatment is 0.3 of it, halved). Its loyalty closes 40% of the gap a day (villagers.ron `drift_per_day`), so it crosses 0.5 on day 6. Goodwill toward you gains 0.05 a day and reaches the 0.25 line on day 5. So an unvisited agent turns a fed town on about day 6, with his suspicion at about 0.7 and four days of slack before it reaches 1.0; a hungry town (whispers ×1.5, loyalty already lower) turns on about day 4. A second purse is insurance, not part of the loop. Hunger from bandits, or your influence's contest term, brings it down faster. If loyalty falls below 0.35 for a day first, the town revolts and goes free (today's rule, with no garrison), and you court it instead. (The first draft had whispers at +0.03 and suspicion at 0.08 + 0.1 × loyalty, which let an unvisited agent be caught on day 7 and turn the town on day 9: the loop did not close. `tests/take_modes.rs` pins the corrected timing.)
- **Costs:** a man, goods, days and a risk with a stated price. No blood, no broken men.

### 5.5 Side by side

| | Conquer (sack) | Intimidate | Court | Subvert |
| --- | --- | --- | --- | --- |
| Verb | `Attack` (existing) | `Demand` (new order) | Gift food, defend, answer requests, then accept the invitation | `Subvert` (new order) |
| Works on | anything but your own | independent and rival towns, not the seat | independent towns; weak rival towns | rival towns, not the seat |
| Pays in | men and blood (and the town's love) | a big army, then ten days of holding the ground near it | food, time, then a duty | a man, goods, time and risk |
| Speed | hours | an hour, then ten days of holding the land | days | about a week |
| Lasting state | Conquered (Sacked): resentment that fades | Submitted: tribute; defies you if you look weak | Invited: a bond; expects defence | Subverted: fragile; the reprisal's favourite |
| Fallout | broken men; a reprisal in the rival's shadow | a defiance if neglected | Forsaken if a raid goes unanswered | a reprisal; a caught agent provokes one |
| Shown before you commit | militia, garrison, the broken men to expect | "need N within 40, have M" | goodwill of 0.5; influence of 0.5 | loyalty, suspicion, "rival soldiers near: 0", cost |

In one line each:
- **Conquer:** "Mine tonight, but I must sit on it and feed it, and those eleven men walking into the woods will be back."
- **Demand:** "Forty spears at the gate for an hour and they knelt. If I lose the land around them, they won't stay knelt."
- **Court:** "Three days of gifts and a raid driven off, and they asked me. Now I owe them."
- **Subvert:** "Hob has been in Fenstead five days. Tomorrow they open the gates, or the rival's soldiers find him first."

## 6. Influence: a sphere you can see grow

**`Influence` (new; sim_military `influence.rs`; on every populated settlement in a run): `{ power, player, rival }`.** `influence::step` (progress slot 13) recomputes it hourly; its effects act on the day tick.

1. **Power of a held town** (owner the player or the rival):
   `power = sqrt(people / 100) × (0.25 + 0.75 × mean loyalty) × (1 − scar)`, × 2.0 for the rival's seat. Influence is people and loyalty and nothing else: the first draft's bonuses for Market towns and watchtowers were bare multipliers and are cut.
   A loyal town of 300 (loyalty 0.7) has 1.34; a restless hamlet of 60 about 0.5; the rival's seat (450 people, loyalty 0.85) about 3.8.
2. **Pull at every settlement** s, per faction: `I_f(s) = Σ power(h) × max(0, 1 − d(h, s) / 200)` over that faction's towns, centre to centre. A town counts for itself at distance 0.
3. **Shares:** `player = I_P / (I_P + I_R + 0.5)` and `rival = I_R / (I_P + I_R + 0.5)`. The 0.5 is a town's own independence. **Net** = `player − rival`, in (−1, 1).

The function is pure (`influence::field_at`), so the renderer draws the same field the sim acts on.

**Scale on seed 1** (settlements about 120–150 tiles apart): one loyal town of 300 gives a neighbour at 145 tiles a net of about 0.43 ("warming"); two give about 0.6 (inside your sphere); the seat puts about −0.35 on a town halfway between it and one of yours, and its heartland can't be warmed. The cost is 625 distance terms an hour.

**What it does where it lands** (each through an existing seam: a ledger deed, an invitation, a loyalty term):

| Where | When | Effect |
| --- | --- | --- |
| A town you don't own, not the seat | net > 0.1 and goodwill toward you < 0.6 | daily `Deed::Influence` (new, no spread) of `0.12 × (net − 0.1)`: a free town at net 0.6 warms 0.06 a day, from 0 to the invitation's 0.5 in about eight days; one answered request halves that |
| An independent town | net ≥ 0.5 and goodwill ≥ 0.5 | **envoys:** it invites you with no captain within 40 tiles; at most one such invitation a day across the island |
| An owned town (either side), not the seat | the other side's share is higher | Contest term `−0.25 × (other − owner share)`; your heartland +0.05. Your border towns feel the rival; the rival's towns deep in your sphere grow restless, and with no garrison they revolt (today's rule) and become yours to court |
| The rival's seat | always | nothing: **the seat can only be conquered**, though its defence follows the share of its realm it still holds (section 6.1) |

**Why no spread.** Influence and Whisper deeds skip the 25%-within-150-tiles word of mouth (`spread_totals`), through a new `Deed::spreads()`, or the field would count twice.

**Strength from holdings, without micromanagement.** You asked for levies, tithes and "renown-like power". PROGRESSION_DESIGN's earlier answer (2026-10-07: "progression comes from land and people, equipment tiers, and the champion and veterans; renown and titles stay out") would drop the last, and the first draft did so without asking. This draft reads "renown-like power" as the power of a realm, with no renown number and no title ladder, and asks you to confirm (open question 16). What you hold feeds you through land and people:
- **Tithes and levies** (section 8.1): food, arms and men flow to the armies passing within 60 tiles of your towns, whenever a dial or a steward's habit has set them, without a click.
- **Arms:** forge towns are the island's weapons (section 8.6).
- **Captain slots** from people held, as today.
- **Towns that join:** influence brings envoys.
- **Rank and volunteers from a realm** (proposed, question 16): the champion gains rank credit for people held (`influence.rank_people`: `influence.rank_points` for every 500 people held, once a day, through the existing `champion::credit` with a new `Feat::Realm`), so a big realm ranks him without a battle; and drifters' goodwill line drops by `influence.drifter_ease` (0.1) at towns inside your sphere, so a sphere draws volunteers to your captains. Both are small, run-only, and hook the existing dominion and drifter steps.

**Guards against snowballing:**
- power grows with the square root of people, and the shares saturate;
- resentful and fresh towns project little (`1 − scar`, and loyalty);
- goodwill from influence alone stops at 0.6, and only one captain-free invitation comes a day;
- the seat is never swayed, the rival's towns contest your border, and its defence falls as you take its realm apart (section 6.1);
- violent takes raise bandits (more for rapid ones) and reprisals hit the newest holding;
- captain slots stay the army budget.

### 6.1 The seat follows its realm

The final point used to be disconnected from the sphere war: the seat's defence bonus (`rival::clock`: `defence_on(day) × strength`) rose on a clock alone, so turning, starving or taking the rival's towns did nothing to the last assault except thin its levy. In a run it now follows the rival's realm:

- **The realm.** `RivalState.realm` (new) records, at `rival::setup`, each rival settlement's starting people. The realm share is the starting people of the settlements the rival still owns, over its starting total (1.0 at the start, 0.0 with nothing left). Counting starting people, not today's residents, keeps its own levies from moving it.
- **The bonus.** `defence_bonus = defence_on(day) × strength × (realm_floor + (1 − realm_floor) × realm_share)`, with `realm_floor` 0.4 on steady and chaotic and 0.5 on harsh (a seat keeps its walls and a core garrison even when it holds nothing else). At share 1.0 the factor is exactly 1.0, so a run where the rival still holds everything, and every `tests/rival.rs` case, is unchanged.
- **What moves it.** Every rival town you take, turn by an agent, drive into revolt (bandits' plunder and hunger, the contest term) or win by an envoy's invitation takes its people out of the realm. A reprisal thins the garrison the usual way; this thins the wall.
- **Shown.** The seat's panel and the run bar read "The rival's realm: 4 of 9 towns, 1,150 people (45%) · seat defence +0.5 (its clock says +0.75)", so every pull on the rival's lands visibly weakens the final assault.
- **Cost.** One sum over the rival's settlements, hourly, inside `rival::clock`.

## 7. Conquest fallout, both ways

### 7.1 Broken men: bandits from the towns you take

- **Who.** A player conquest or sack (`Cause::Conquest` or `Cause::Sack`, never a submission, invitation or subversion) of a town of at least 60 people queues `PendingBand { settlement, men, due: now + 2h }` in `Fallout` (new resource). `men = 0.04 × people + 0.5 × blood`, ×1.5 for a sack, ×1.25 if your previous violent take was under two days ago, at most 16; under 4, none. Tarnridge (140 people, 11 killed) gives 11.
- **They are its own people.** `fallout::step` (progress slot 16, hourly) spawns a RAIDERS captain at the town's edge with `spawn_army` (`raider: true`, 2 days' food, unarmed), then enlists the first `men` of `recruit_candidates(town)` (idle first) with `enlist_into`: the town really loses them, and when they starve and desert, today's `apply_desertions` walks them home. "The broken men drift home in a week" is the natural end. It inserts `Bandits { home, raids }` (new) and writes `BanditsRose` with `Cause::Fallout { settlement }`: "Eleven men of Tarnridge take to the woods (Tarnridge was taken by the sword)." Spawning happens only in the driver (BUILD_GUIDE: `spawn_army`'s RNG is keyed by `NextSpawnId`), never inside the orders step that took the town.
- **The cap.** At most two bandit bands alive. A band that can't rise yet waits in the queue, and its town broods: its scar deepens by 0.05.
- **Where they go: where lords are weak.** In `raiders::think`, a run-only branch for `With<Bandits>` replaces the player-only filters. Each think, the target is the populated town, not the seat, not home and not the last target, with the best score:
  `weight(owner) / ((1 + d / 150) × (1 + garrison / 5) × (1 + your share there))`, with weights rival 2.0, player 1.0, independent 0.75.
  So they mostly harry the rival's lands and the no-man's-land between, but your newest, ungarrisoned frontier towns, outside your sphere, are exposed: **influence protects.** After two raids the rival weight drops to 1.0 and they take whatever is easiest ("may hit yours too").
- **Their own dead are remembered at home.** Killing an enlisted soldier posts no `Killing` deed today (`combat.rs` posts it only for a militiaman's death), so beating the bandits of a town you took would read as a favour: `Defended` goodwill and a "kept" step toward the bond, a loop of conquering a town to farm its sons. Two run-only rules close it. The `Defended` deed skips any battle whose foes include `Bandits` whose `home` is that town (section 5.3). And when a player's soldier kills a bandit, `combat::casualties` posts `Deed::Killing` toward the player at the bandit's home, the deed a dead militiaman posts: "you killed our sons in the woods." The echo cuts both ways: hunting your bandits down costs goodwill at the town they came from.
- **Ripples.** Plundering a rival storehouse posts the raiders' deed there as today; the town's food security falls, so its loyalty falls (food is 0.55 of the target), so it may revolt and go free, its levy pool thins, and an agent works faster there.
- **The director** counts bandits for tension when they come near you (they are at war with you, as today), but not toward its `max_bands` (`roaming` becomes `With<Raider>, Without<Bandits>`), so its paced bands keep their rhythm, and its own bands keep BUILD_GUIDE D2's player-only prey.
- **A fix it needs.** `combat::succeed` carries `RivalBand` to an heir but not `Raider`, so a raider band whose captain falls loses its AI. In a run, `succeed` carries `Raider` and `Bandits` as well.

### 7.2 Reprisals: the rival wants it back

- **The vow.** When you take a town (any way) that was the rival's, or where the rival's share is at least 0.3, or when one of your agents is caught, `fallout::step` sets `RivalState.reprisal = (town, due: now + 18h)` (new fields), unless one came in the last two days; the newest vow replaces an older pending one. A `ReprisalVowed` event says so, with its hour: "The rival vows to retake Fenstead (by tomorrow evening)." It pauses the game on steady.
- **The raid.** At slot 11, `rival::clock` launches it when due, from `reprisal_from_day` (day 6 on steady), if the garrison has at least 8: today's `raid()` splits `reprisal_size × strength` (at most half the garrison), ×1.5 against a fragile subverted town, with a new stream key for the lead (`sim_military::rival::reprisal`). `RivalBand.pinned` (new) holds the target: **your newest holding** when it launches (the latest `Owner.since`), which `rival::think` honours before its nearest-player scan. The band attacks, as rival bands do: your garrison and the town's militia fight it, and if it holds the centre the town is the rival's again, **Conquered** with the ordinary scar and no plunder (the sack is the player's rule, section 5.1). The next scheduled raid is pushed back half a period, so pressure rises by about a quarter rather than doubling.
- **The books.** The split counts in `RivalState.raids`, so `tests/rival.rs`'s garrison ledger still balances.
- **The choice it forces:** garrison the town, wall it (Walls and a palisade), or let it fall and take it back. **Every reprisal thins the seat**, so holding against one helps your march.

### 7.3 Rules this amends ([ADR 0020](adr/0020-lordship.md))

- **BUILD_GUIDE D2** ("keep raiders' prey on PLAYER, or the rival's growth curve becomes noisy") still holds for the director's bands. Bandits deliberately prefer the rival; `bot_run` and the rival's tests watch the noise.
- **[ADR 0018](adr/0018-village-food-in-a-run.md)** keeps plunder storehouse-only, and adds a granary's floor (section 8.3).
- **[ADR 0014](adr/0014-ownership-conquest-and-loyalty.md)** gains the new causes, `Tenure` and the loyalty terms.

## 8. Governing your towns

**Principles:**
- **Three dials, one building at a time and one steward per town. Nothing else.**
- **Sticky:** a dial runs until it is changed.
- **Prompt first, slow after:** a change starts within the game hour (10 seconds at 1x): the dial turns, the lock starts, and the first wave of moves, the first muster, the ration or the feast begin in the next hourly step (`govern::step` is already hourly). Later waves go at each dawn, up to a ten percent a day. The dial then stays locked for two days from the hour it started (one with a Reeve). No flip-flopping, so each choice carries weight; and the click that most directly answers "influence the cities" has an answer on the map at once. The label shows a countdown badge ("Forge: first wave now, about 3 dawns to finish").
- **Shown on people:** every option states its cost and changes what villagers do, where you can see it.
- **A town you never open still has someone running it, and he has habits:** a steward sets the dials his habits touch, shown a dawn ahead, sometimes against your interest, always overridable (section 8.2).

### 8.1 Edicts (`Edicts`, new; sim_agents `edicts.rs`; on every populated settlement in a run, acting only while you own it)

The player sets them with `CommandKind::Govern { settlement, act }` (new), validated in `orders::apply` like Garrison's owner check, consumed in `SimInputSet::Consume`, and recorded in replays. Every default is an exact no-op.

**Work** (Balanced by default):

| Option | What happens (first wave in the next game hour, then each dawn; up to 10% of workers a wave, at least 2, in SpawnId order) | You see | Cost |
| --- | --- | --- | --- |
| Balanced | workers drift back to the trade they had before an edict moved them (`Assignment.from`, new) | today's layout | — |
| Fields | idle working-age adults first, then woodcutters, miners and smiths beyond one each, then carters above 8% of workers, move to farms, fishing spots, pastures and the bakery with room, up to 85% of workers on food | people start walking out to new fields within the hour, over two or three days | no timber (so slow building) and no arms |
| Forge | every smithy's recipe turns to weaponsmithing in the first hour; idle adults, then farmers while the town has a day of food, move to smithies, the mine and the woodcutters' camp, up to 20% of workers there; leaving Forge restores the layout's alternation | smiths walk to the forge within the hour; weapons pile up in the storehouse | about 10–15% less food; no tools |
| Walls | idle adults, then farmers while the town has a day of food, become carters (up to 6 more); a site builds 1.5 times as fast; while it holds, the town's militia gets +0.25 defence within `garrison.radius` and its threat radius is ×1.25 (villagers see danger sooner) | haulers start streaming to the scaffold within the hour; the label shows a tower | food output |

Moves use one new function, `sim_agents::jobs::reassign` (pub): leave `Workplace.workers`, join the nearest workplace of the new kind with room (populate's own rule), set `Occupation` and `Employment`, and resize the carry for a carter. Only empty-handed villagers move. The inspector says why: "Smith, moved from farmer by the Forge edict at Northwell, day 6".

**Dues** (Light by default; men or goods, never both):

| Option | Effect | Loyalty term |
| --- | --- | --- |
| Light | today's behaviour exactly: owned towns feed an owner's army that gathers there, keeping a day | 0 |
| Tithe | `orders::lands` (new, a pass inside `run_orders`) hands the nearest owner captain within 60 tiles with room the food beyond half a day (pantry first, as `supply` reads it) and the storehouse's weapons and tools, then arms its followers (`arm_followers`): in the hour you set it, if a collector is within reach, and then once a day | −0.05 (0 if submitted) |
| Levy | in the hour you set it 2% of the people (at least 1), able adults with the idle first, are marked `Mustered` (new) and wait on the green instead of working, and another 2% at each dawn, up to 10% of the people; once a day `orders::lands` enlists them into the nearest owner captain within 60 tiles with `enlist_into`, and they walk to it along the rally field. A garrison captain standing at the town collects too, so **a garrisoned town grows its own garrison** | −0.08 (−0.12 conquered or sacked; 0 invited) |

No collector within reach means nothing moves that day, and nothing piles up out of sight. One `Tribute` line per receiving captain a day: "Your lands sent Edric's band 2.4 days of food, 6 arms and 3 men (Northwell, Fenstead)." Both stay inside the army's logistics step, so CLAUDE.md's "nothing else moves goods between two stockpiles" holds as written.

**Table** (Normal by default):

| Option | Effect | Cost |
| --- | --- | --- |
| Ration | from the next hourly step residents carry `Rationed` (new): hunger ×0.75 in `update_needs`, and `FoodSecurity`'s daily need ×0.75, so its days stay honest; work cycles ×1.1 longer; children born later get it at dawn | loyalty −0.1 |
| Normal | — | — |
| Feast | needs a day of food; in the next game hour half a day's need is eaten from the storehouse, and for 24 hours idle villagers gather on the green (the idle candidate picks a tile near the centre, and work scores ×0.3); at the end, if the town still has half a day of food, `Deed::Festival` (new, +0.08, spreads: word travels) and a +0.08 Table term for five days; then back to Normal, and six days before the next | half a day of food and a day's work |

### 8.2 Stewards: a person with habits

A first draft gave every town a steward who set sensible dials and a few quiet multipliers (food ×1.1, smithing ×1.15, build needs ×0.75). That risks the opposite of the playtest's complaint: governing becomes optional, and a player who ignores every panel gets nearly the same realm. So a steward is a personality, not an optimiser. Each trait is one visible habit, and some are against your interest in the wrong town. There are no hidden multipliers.

- **Every town you own has one.** At the first dawn you hold it (and whenever it has none), `govern::step` names the working-age resident of the town's own trade (a Smith or Miner for a mining town, a Fisher for a fishing town, a Woodcutter, a Carter for a market, a Farmer), lowest SpawnId first, else any working-age resident. Deterministic, no roll. `StewardNamed`: "Edric Ashby, a smith of Northwell, is its steward."
- **Traits come from their own life,** at most two, from different rows. A resident steward is always Native plus their trade's trait.

| Trait | From | His habit: the dial he sets at dawn when you haven't | When it hurts |
| --- | --- | --- | --- |
| Stern | a Veteran or Elite soldier | Dues: Levy in any town of 100 people or more. Table: Ration when the town has under two days of food; never a Feast | a levied, rationed town you wanted calm: resentment and hunger build, and at a sacked or fragile town it rises |
| Forge-wise | was a Smith or Miner | Work: Forge, always, even when the town is hungry | a hungry forge town sinks toward revolt for want of the farmers he took |
| Husband | was a Farmer, Fisher, Shepherd or Baker | Work: Fields when the town has under three days of food, and never Forge. Table: Feast whenever it has two days of food and none is due | a mining town with a Husband never arms you, and the feasts eat the tithe's food |
| Reeve | was a Carter | Dues: Tithe whenever the town has more than a day of food. His dials lock for one day, not two | a thin town is milked down to half a day, and your armies take the arms |
| Wright | was a Woodcutter | Work: Walls whenever a site is open or a vow names the town (haulers and a faster build), else Balanced | the farms lose hands to the haulers |
| Native | lives in or comes from this town | Dues: never sets Levy on his own. Steward term +0.05 ("one of us", the one number in the list, and it shows in the "why") | you never get a free garrison from him |

- **A dial no habit touches stays at its default,** an exact no-op. A steward with no habit for Work leaves Work Balanced. When two of his habits want the same dial, the trait named first in the table wins, except that Native's veto (never Levy on his own) beats any habit.
- **Shown a dawn ahead, and overridable.** The steward's choice is a deterministic function of the town's state, so the panel and the label can compute tomorrow's: "Your steward Wat Cooper (Stern, Reeve) will set Levy at dawn (a town of 140: his habit)", with the dial's own forecast and an **Override** button. Setting a dial yourself cancels his pending habit, and the dial then locks like any dial you set (two days, one with a Reeve). `Release` hands a dial back to his habit. When a habit does change a dial, the Dawn line says so ("by the steward's order: Wat set Levy at Tarnridge") and the inspector's trace for the villagers it moved says "by the Forge edict, set by the steward".
- **Men against towns.** `Govern { act: Steward { follower } }` appoints one of your army's villager followers whose captain stands within `recruit.radius` of the town: discharged into it (`upkeep::discharge`), he replaces the resident steward, who goes back to work. A veteran brings Stern; a native brings Native. Your army is a man short. The trade comes from `Veteran.trade` (new: the occupation before enlisting, captured in `enlist` in every preset, with no change of behaviour; M4c's service records reuse it).
- **Death and hand-over.** A steward who dies, enlists or leaves is replaced at the next dawn. When the town changes hands, the steward is deposed (`Cause::Deposed`, a chronicle line).
- **The point of all this.** A town you never open still runs, in a way you can read and sometimes regret. The decision to appoint, override or release is a real one: the Stern steward who is perfect at the frontier town is wrong at the sullen one, and you can see which a dawn before it matters. The playtest asks whether you overrode a steward and saw the difference, and a bot check does too (section 12).
- **Later:** M4c's quirks, kin and biographies reach stewards for free, since they are named villagers.

### 8.3 Building orders (one at a time; two per town per run)

1. **The order.** `Govern { act: Build(kind) }`, naming a **kind**, not a tile. Validated: you own the town, no site is open there, fewer than two built this run, at most one each of granary, palisade and watchtower, at most one extra smithy, and a site exists. Once ordered, it can't be cancelled: the slot is committed.
2. **The site.** `sim_economy::choose_site(world, settlement, kind)` (new, pub) wraps `pick_sites` with today's site rules and an `Occupied` rebuilt from every `Building` tile. M4b swaps its body for "the next open master-plan plot of this kind", and every caller stays the same. `sim_economy::start_building` (new, pub) spawns a real `Building` with `spawn_sim`, a `Stockpile`, a `Workplace` with no recipe and `Construction { needs, hours_left }` (new), and appends it to `SettlementEconomy.workplaces` (its SpawnId is the newest, so the order holds).
3. **Supply.** Carters' `haul_job` also offers "deliver input" jobs for a site's needs minus its stock, capped like the smithy fix (section 8.6). The site needs no workers.
4. **Build.** Hourly, from `govern::step`, once its stock covers its needs, `construction::advance` counts `hours_left` down (×1.5 faster under Walls). At zero it consumes the needs from its own stockpile, removes `Construction`, and writes `Built`.

| Kind | Needs (economy.ron `construction`) | Hours | Effect, each a no-op without the building |
| --- | --- | --- | --- |
| Smithy (existing kind) | 30 wood, 10 ore | 24 | six more smith places; weapons under Forge, else the layout's alternation, so a second smithy makes weapons |
| Granary (new kind) | 40 wood | 24 | plunder, seizure and the tithe leave 1.5 days of the town's need in its storehouse (`gather_food`, `spare_food`); the feed says so each time it withholds something ("Thornby's granary kept 40 food from the bandits"), and the Tithe forecast reads "granary keeps 1.5 days" |
| Palisade (new kind) | 80 wood | 36 | its militia get +0.5 defence within `garrison.radius` (a per-town lookup beside the rival seat's defence in `resolve_strikes`); an Attack's hold takes ×1.5; Demand's defence ×1.5 |
| Watchtower (new kind) | 30 wood | 18 | the town's threat radius ×1.5 (villagers hide and militia muster sooner, and drive-off requests post sooner, so you hear of a raid hours earlier and can answer it, or be forsaken). The map draws the ring on hover, and the feed says "Thornby's watchtower sights raiders" when the extra reach is what saw them. No influence bonus: the first draft's ×1.25 was a bare number |

A timber town builds with half the wood (a Wright steward's habit is Walls, not a discount). The palisade is one centre structure drawn as a ring of stakes, with no terrain change, so no `set_terrain` and no path-cache flush; walls as tiles are M4b's.

### 8.4 Specialised towns (`Specialty`, new; sim_agents `population.rs`; run only)

At populate, in a run only, each settlement counts the tiles within 40 of its centre and reads its road degree (`RoadNetwork`). The first matching rule in villagers.ron `specialties` wins, and its share multipliers scale the occupation shares before `apportion`, then renormalise, so the tag is real people. A specialty is worth targeting only where it changes what you can do, so each row has one thing the player can see, or none:

| Specialty | When | Shares | What owning it gives you (one visible flow, or none) |
| --- | --- | --- | --- |
| Mining | at least 150 Upland tiles (hills or mountain) within the mine radius (40) of its centre | Miner ×4, Smith ×2 | **Arms.** Ore that lasts and two smithies in a town of 300: its Forge dial turns out real weapons, its tithe carries them to your armies, and an ordered smithy needs its ore. The one place weapons come from in bulk |
| Market | road degree 3 or more | Carter ×1.3, Baker ×1.3 | **Men for food.** Every `market.hire_days` (5) a Market town you own posts a `Hire` offer (the sellswords offer that exists): a band of 3 to 6 camps at its edge, `people / 80` men, deterministic, priced in food a man (`hire.price_days`) and hired by your champion within `hire.radius`, as sellswords are today. The only place to buy men |
| Timber | at least 600 forest-edge tiles | Woodcutter ×3 | **Cheap works.** Buildings there need half the wood |
| Fishing | at least 400 coast tiles | Fisher ×2.5 | **A larder** (people, not a number): a food surplus, so a Tithe or a Feast there costs no hunger. No separate bonus |
| Farming | otherwise | Farmer ×1.1, Baker ×1.2 | **A larder,** as above |

The first draft's bare multipliers are cut: influence ×1.3 and trade ×1.2 at Market towns, "a tithe that costs no hunger" as a rule at Fishing towns, and "Fields moves twice as many" at Farming towns. A larder's value is the people it really has: more fishers and farmers, so a bigger food surplus for the forecasts to show, not an extra rule.

**Honest about what changes.** `apportion` and the shuffle draw the same number of times, so **the RNG draw counts are unchanged.** The layout is not: scaled counts change how many workplaces each town builds (`workers.div_ceil(slots)` in `build_settlement`), which shifts the shared `Occupied` set and the SpawnId of every later building and villager on the run island. BUILD_GUIDE G10 accepts exactly this in the run preset, and the developed world and the skirmish never take the branch. The step's check (`populate_draws_are_unchanged`) asserts draw counts, not SpawnIds; `bot_run` re-tunes the island. Mining is based on tiles that exist before anything is built (reachable Upland within the mine radius), not on "a mine sited", because mines are sited later by `build_settlement`.

Shown as an icon on the label and a line in the panel. Mining towns become the island's real ore and arms, and Market towns its mercenaries, so they are worth targeting.

### 8.5 How the dials meet the take modes

- A **sacked** town on a levy is the riskiest town you can own: −0.45 resentment, −0.12 dues.
- A **submitted** town is on tithe: its tribute feeds the armies that pass, and the garrison that watches the land around it.
- An **invited** town levies for free: they fight for their lord.
- A **subverted** town is fragile for six days: it wants a garrison, not a Stern steward, who would levy and ration it into revolt (override him).

### 8.6 The smithy and weapons chain (a prerequisite, measured broken)

Measured on the M4 build (seeds 1 and 7, ticks 1,200 to 8,000), five faults, all independent of the island's size:

| Fault | Fix (run only) |
| --- | --- |
| Smithies stall full of one input. `work_state` (`crates/sim_agents/src/ai.rs`) counts inputs as fullness, so a smithy holding 190 ore and no wood is "well stocked" and nobody works it | count only outputs as fullness; a workplace full of inputs is Ready |
| Carters bring full 50-unit carts of one input, several to the same job | deliver at most `haul.input_target` (10) cycles' worth minus what is there |
| Smiths with full hands loop (`Work`, `Done`) forever | a worker holding goods that aren't inputs drops them first |
| Weapons wait at the forge: carters collect only piles of 8, so Equip, the bot and the panel never see them | per-good minimums (`haul.min_load`: weapons 1, tools 2) |
| Only a town's second smithy makes weapons (the recipe alternates per building and never changes) | the Forge focus, and ordered smithies |

All five are gated by `in_run`, so the developed world, `village_day` and `famine` stay exactly as they are; M4b, which re-baselines growth anyway, removes the gate. **Targets** (`tests/smithy_chain.rs`): at least 40 weapons on seed 1's island in three days with no edicts (about 10 today), no smithy idle full of inputs for a day, and a town with six smiths on Forge putting at least six weapons a day in its storehouse. Weapons are +8 attack on 12 (army.ron), the biggest single strength lever in the game, which is why mining towns become targets and why `bot_run` watches the win days.

## 9. What you see

| Surface | What changes | Where |
| --- | --- | --- |
| **Settlement panel** | rebuilt: tenure and steward; the "why" (the loyalty target term by term, goodwill toward you and the rival, the dated deed log, revolt risk); for your towns, the three dials, Build and the steward; for others, the ways to take it; what its lands send | `crates/game_ui/src/panels.rs` |
| **Right-click menu** | Demand submission and Send an agent appended; Give food reads Gift food at towns not yours; each shows its forecast or why it's greyed | `crates/game_ui/src/army.rs` (`MenuAction`) |
| **Influence overlay** | appended to the O cycle: your colour to the rival's, a purple front where the shares meet, a dashed sphere edge at net 0.5, rings filling toward an invitation; a watchtower's threat ring on hover | `crates/game_render/src/overlay.rs` |
| **Labels** | a specialty icon; a tenure mark (sword, broken sword, bent knee, ribbon, eye); badges for Forge, Walls, Ration, Feast, Levy and a site, each with a countdown while its first wave runs; "watched today" on a submitted town; the steward's pending habit | `crates/game_ui/src/labels.rs` |
| **Run bar** | "Sphere 7, warming 3"; "Lands: +F food a day, M men, A arms"; "Broken men abroad: K"; a vow with its hour; "The rival's realm: 4 of 9 towns, 45% · seat defence +0.5 (clock +0.75)" | `crates/game_ui/src/run_bar.rs` |
| **Feed** | the new kinds (below), and one grouped **Dawn** line a day for edicts, musters, stewards and tribute | `crates/game_ui/src/feed.rs` |
| **Inspector** | a moved villager says why ("by the Forge edict, set by the steward"); a steward shows their traits and habits; an agent shows their suspicion (your own) | `crates/game_ui/src/panels.rs` |
| **Buildings** | a scaffold while `Construction` stands, three new models, crowds on the green at a feast or a muster (that one is the AI, not the renderer) | `crates/game_render/src/buildings.rs` |

**One of your towns:**
```
THORNBY · Mining town · 311 people · yours since day 6, by the sword (resentment fades in 5 days)
Steward: Wat Cooper (Stern, Reeve), appointed by you      Loyalty 0.48, rising to 0.56
  why: food +0.55 · treatment +0.10 · time +0.06 · resentment −0.10 · rival's pull −0.03
  remembers: day 6 you killed 9 of our men −0.18 · day 7 a gift from you +0.08
WORK   Balanced  Fields  [Forge]  Walls      yours since day 7 · Forge: 6 more smiths by day 9; about 12% less food
DUES   [Light]  Tithe  Levy                   at dawn Wat will set Levy (his habit: 100+ people): −0.08 loyalty, 3 men a day  [Override]
TABLE  Ration  [Normal]  Feast                (locked until day 9)
BUILD  1 of 2 used · Smithy: wood 30/30, ore 4/10
LANDS  yesterday: 14 weapons to Edric's band · 3 more at the forge
```

**A rival town:**
```
FENSTEAD · Timber town · 287 people · the rival's (granted at the start)
Loyalty to the rival 0.52, falling to 0.41
  why: food +0.55 · treatment +0.13 · time +0.15 · stirred by your agent −0.30 · your pull −0.02
Thinks of you +0.21 (your agent's whispers +0.05 a day · day 6 your broken men are not ours)
WAYS TO TAKE IT
  Conquer  militia about 80, no garrison. Fast, bloody: resentment, about 13 broken men, a vow.
  Demand   defence 40: you need 60 within 40 tiles and have 21.
  Court    a rival town invites only when unhappy (loyalty under 0.4), fond of you (0.5) and your captain is near.
  Subvert  your agent Hob · day 4 · suspicion 0.48 · a gift would take 0.3 off
```

**New feed and chronicle kinds** (`SimEventKind`, appended): Submitted, Defied, Subverted, BanditsRose, ReprisalVowed, RivalReprisal, AgentCaught, Forsaken, Built, StewardNamed, EdictSet, Tribute. The first seven are notable (they stop fast-forward); Forsaken, Built and StewardNamed are chronicled but not notable; EdictSet and Tribute are feed lines only. Auto-pause on steady and chaotic: ReprisalVowed, Defied and AgentCaught. Today `chronicled()` is a list of exclusions and `notable()` is `chronicled() || RequestPosted`, so a new kind would be chronicled and notable by default; section 11 says how both become explicit lists without changing a single existing kind.

## 10. Data (with the shipped numbers)

Every new block is `#[serde(default)]` with a `Default` (progression.ron is saved as RON text, as `GatherData` and `HireData` are), and every field goes in `docs/dev/DATA.md` (the docs test enforces it). Each block has a switch that makes it a no-op, so scenario tests that guard M4 behaviour can opt out.

**progression.ron**
```ron
influence: ( enabled: true, reach: 200.0, people_unit: 100.0, loyalty_floor: 0.25,
             seat_power: 2.0, independence: 0.5,
             warm_above: 0.1, goodwill_per_day: 0.12, goodwill_cap: 0.6,
             envoy_net: 0.5, max_envoys_per_day: 1, contest: 0.25, heartland: 0.05,
             warming: 0.2, sphere: 0.5, front: 0.1,
             rank_people: 500.0, rank_points: 1, drifter_ease: 0.1 ),   // the last three: open question 16
tenure: ( conquest_scar: 0.12, sack_scar: 0.25, scar_per_kill: 0.01, max_conquest_scar: 0.3,
          max_sack_scar: 0.45, subverted_scar: 0.1, scar_fade_per_day: 0.02,
          bond: 0.05, kept_bond: 0.02, max_bond: 0.1, forsaken_bond: -0.1, forsaken_after: 2,
          defended: 0.1 ),
demand: ( radius: 40.0, parley_hours: 1.0, ratio: 1.5, militia_weight: 0.25, garrison_weight: 1.0,
          armed_weight: 1.5, seed: 0.45, awe: 0.1, watch_radius: 80.0, hold_ratio: 0.75,
          watch_hours: 6, watch_garrison: 4, watch_share: 0.5, weak_days: 2,
          settle_days: 10.0, refuse_cooldown_days: 2.0 ),
court: ( gift_per_day_of_need: 0.25, max_gift: 0.12, hungry_mult: 2.0 ),
subvert: ( bribe_food: 10.0, bribe_goods: 2, max_agents: 3, whisper_rival: -0.06, whisper_you: 0.05,
           hungry_mult: 1.5, stir_per_day: 0.1, max_stir: 0.4, suspicion_base: 0.06,
           suspicion_per_10_guards: 0.1, suspicion_loyalty: 0.08, guard_radius: 30.0, purse_relief: 0.3,
           turn_loyalty: 0.5, turn_goodwill: 0.25, seed: 0.5, fragile_days: 6.0 ),
fallout: ( enabled: true, min_people: 60, per_people: 0.04, per_kill: 0.5, sack_mult: 1.5,
           haste_days: 2.0, haste_mult: 1.25, min_band: 4, max_men: 16, max_bands: 2,
           delay_hours: 2.0, food_days: 2.0, brood_scar: 0.05,
           weights: (rival: 2.0, player: 1.0, independent: 0.75), turn_after_raids: 2,
           distance_scale: 150.0, garrison_scale: 5.0, shadow: 0.3, vow_delay_hours: 18.0,
           vow_cooldown_days: 2.0 ),
edicts: ( lock_days: 2.0, reach: 60.0, moves_share: 0.1, min_moves: 2, carter_floor: 0.08,
          fields_cap: 0.85, forge_cap: 0.2, walls_carters: 6, walls_build: 1.5, walls_defence: 0.25,
          walls_threat: 1.25,
          tithe: (keep_days: 0.5, term: -0.05),
          levy: (per_day: 0.02, cap: 0.1, term: -0.08, harsh_term: -0.12),
          ration: (hunger: 0.75, work: 1.1, term: -0.1),
          feast: (min_food_days: 1.0, food_days: 0.5, gather_hours: 24.0, work: 0.3, deed: 0.08,
                  term: 0.08, term_days: 5.0, cooldown_days: 6.0) ),
stewards: ( habits: true,    // habits are data rows: (dial, setting, condition), applied in table order
            traits: [
              (name: "Stern", from: Veteran, habits: [(Dues, Levy, PeopleAtLeast(100)),
                      (Table, Ration, FoodDaysUnder(2.0))], never: [(Table, Feast)]),
              (name: "Forge-wise", from: Trades([Smith, Miner]), habits: [(Work, Forge, Always)]),
              (name: "Husband", from: Trades([Farmer, Fisher, Shepherd, Baker]),
                      habits: [(Work, Fields, FoodDaysUnder(3.0)), (Table, Feast, FoodDaysAtLeast(2.0))],
                      never: [(Work, Forge)]),
              (name: "Reeve", from: Trades([Carter]), habits: [(Dues, Tithe, FoodDaysAtLeast(1.0))],
                      lock_days: 1.0),
              (name: "Wright", from: Trades([Woodcutter]), habits: [(Work, Walls, SiteOpenOrVow)]),
              (name: "Native", from: Resident, never: [(Dues, Levy)], term: 0.05) ] ),
market: ( hire_days: 5.0, min_men: 3, max_men: 6, people_per_man: 80 ),
buildings: ( max_per_town: 2, granary_keep_days: 1.5, palisade_defence: 0.5, palisade_hold: 1.5,
             palisade_demand: 1.5, watchtower_threat: 1.5 ),
goodwill_deeds: ( /* existing rows */ defended: 0.1, festival: 0.08, forsaken: -0.15,
                  threat: -0.05, plot: -0.15 ),   // Gift, Influence and Whisper are computed
// each rival row gains: reprisal_from_day (steady 6, chaotic 5, harsh 5),
//   reprisal_size (8, 10, 12), reprisal_min_garrison: 8, turned_mult: 1.5,
//   realm_floor (0.4, 0.4, 0.5): the seat's defence at an empty realm, as a share of its clock
```

**economy.ron**
```ron
workplaces: [ /* existing */
  (kind: Granary,    slots: 0, radius: 12, spacing: 2, sites: [Centre]),
  (kind: Palisade,   slots: 0, radius: 8,  spacing: 2, sites: [Centre]),
  (kind: Watchtower, slots: 0, radius: 16, spacing: 2, sites: [Centre]) ],
construction: [ (kind: Smithy,     needs: [("wood", 30), ("ore", 10)], hours: 24.0),
                (kind: Granary,    needs: [("wood", 40)], hours: 24.0),
                (kind: Palisade,   needs: [("wood", 80)], hours: 36.0),
                (kind: Watchtower, needs: [("wood", 30)], hours: 18.0) ],
haul: ( run_fixes: true, input_target: 10, min_load: [("weapons", 1), ("tools", 2)] ),
```
Validation (`crates/sim_economy/src/data.rs`) gains: every construction kind has a workplaces row; needs name real goods; hours are positive.

**villagers.ron**
```ron
specialties: [   // min_tiles counts reachable tiles of the site within 40 of the centre, before anything is built
  (kind: Mining,  site: Some(Upland),     min_tiles: 150, road_degree: 0, shares: [(Miner, 4.0), (Smith, 2.0)]),
  (kind: Market,  site: None,             min_tiles: 0,   road_degree: 3, shares: [(Carter, 1.3), (Baker, 1.3)]),
  (kind: Fishing, site: Some(Coast),      min_tiles: 400, road_degree: 0, shares: [(Fisher, 2.5)]),
  (kind: Timber,  site: Some(ForestEdge), min_tiles: 600, road_degree: 0, shares: [(Woodcutter, 3.0)]),
  (kind: Farming, site: None,             min_tiles: 0,   road_degree: 0, shares: [(Farmer, 1.1), (Baker, 1.2)]) ],
```

**director.ron:** `auto_pause` gains ReprisalVowed, Defied and AgentCaught on steady and chaotic.

## 11. How it fits what's built (technical notes)

**New state** (every type hashed and saved in the same commit, with `register_hashed` and `register_saved`; entities save as SpawnId + 1):

| Type | Crate, file | On | Written by |
| --- | --- | --- | --- |
| `Tenure`, `TakenBy` | sim_military `tenure.rs` | run settlements | `transfer_seeded`, combat (blood), requests (kept, broken), `tenure::step` (incl. the watch's `watched_hours`) |
| `Agent` | sim_agents `standing.rs` (the villager AI and `recruit_candidates` read it) | an agent | the Subvert dealing, `tenure::step` |
| `Influence` | sim_military `influence.rs` | run settlements | `influence::step` only |
| `Fallout` (resource), `Bandits` | sim_military `fallout.rs`, `components.rs` | resource; bandit captains | `transfer_seeded` (queue only), `fallout::step`, `raiders::think` |
| `RivalState.reprisal`, `last_reprisal`, `realm`; `RivalBand.pinned` | sim_military `rival.rs` | existing | `fallout::step`, `rival::clock`; `realm` once, in `rival::setup` |
| `Steward`; `Veteran.trade` | sim_military | the steward; every soldier in every preset (`trade` is captured in `enlist` everywhere and read only for stewards, so behaviour is unchanged but state hashes change in all presets) | `govern::step`, the Steward act; `enlist` |
| `LoyaltyTerms`, `DeedLog` | sim_agents `loyalty.rs`, `standing.rs` | run settlements | `tenure::step`; `apply_goodwill` |
| `Edicts`, `Rationed`, `Mustered`, `Assignment` | sim_agents `edicts.rs` | run settlements; villagers | `orders::apply` (Govern), `govern::step` |
| `Specialty` | sim_agents `population.rs` | run settlements | `populate` (run only, once) |
| `Construction` | sim_economy `construction.rs` | a site | `start_building`, `construction::advance` |

**Append-only enums, with explicit tags:**
- `CommandKind` 7: `Govern { settlement, act: GovernAct }`. `GovernAct`, `WorkFocus`, `Dues`, `Table`, `BuildKind` and `Dial` are new sim_core payload enums (sim_economy maps `BuildKind` to `WorkplaceKind`). `GovernAct`: `Work`, `Dues`, `Table`, `Build`, `Steward { follower }`, `Release(Dial)`.
- `OrderSpec` 11 `Demand { settlement }`, 12 `Subvert { settlement }`.
- `Deed` 8 `Gift`, 9 `Defended`, 10 `Influence`, 11 `Whisper`, 12 `Festival`, 13 `Forsaken`, 14 `Threat`, 15 `Plot`; `Deed::spreads()` is false for Influence and Whisper.
- `Cause` 15 `Sack { captain }`, 16 `Submission { captain }`, 17 `Subversion`, 18 `Defiance`, 19 `Fallout { settlement }`, 20 `Reprisal { settlement }`, 21 `Edict`, 22 `Caught`, 23 `Deposed`.
- `SimEventKind` 22–33, in section 9's order: Submitted 22, Defied 23, Subverted 24, BanditsRose 25, ReprisalVowed 26, RivalReprisal 27, AgentCaught 28, Forsaken 29, Built 30, StewardNamed 31, EdictSet 32, Tribute 33.
- `WorkplaceKind` `Granary`, `Palisade`, `Watchtower`; `TakenBy`, `LoyaltyTerm` and `Specialty` new.
- No new `OfferKind`: envoys and turned gates reuse `Invitation`, and `answer` picks the cause from the town's state.
- M4c's plan names no tag numbers, so its kinds and causes take the next tags after these.

**Causes, event kinds and the counters they feed.** `transfer_seeded` today maps every cause except `Invitation` and `Revolt` to `Conquest`: that event's `ArmyStats.conquests` count, the champion's rank credit (every kind but `Revolt`) and the run summary's `settlements_taken` (counted from `Conquest` and `Invited` only, `chronicle.rs`). As first specified, a submission, a subversion and a sack would have been reported and counted as conquests, and a `Defiance` (to INDEPENDENT) would have read "Independents took X". Step 1 extends the match, and `tests/tenure.rs` pins the table (`every_cause_maps_to_its_kind_and_counters`):

| Cause | Event kind | `faction` / `other` | `ArmyStats.conquests` | Champion credit | Run summary |
| --- | --- | --- | --- | --- | --- |
| `Conquest`, `Sack` | `Conquest` | taker / previous owner | +1 | yes | taken / lost |
| `Invitation` | `Invited` | taker / previous owner | no | yes | taken / lost |
| `Submission` | `Submitted` (new) | taker / previous owner | no | yes | taken / lost |
| `Subversion` | `Subverted` (new) | taker / previous owner | no | yes | taken / lost |
| `Revolt` | `Revolt` | previous owner / new owner | no | no | lost |
| `Defiance` | `Defied` (new) | previous owner / INDEPENDENT, treated like a revolt | no | no | lost |

`Fallout`, `Reprisal`, `Edict`, `Caught` and `Deposed` ride on other events (`BanditsRose`, `RivalReprisal`, `EdictSet`, `AgentCaught`, the steward's chronicle line) and never transfer a town; a reprisal's successful retake is an ordinary `Cause::Conquest` by the rival. The summary's match in `chronicle.rs` gains `Submitted | Subverted` beside `Conquest | Invited`, and `Defied` beside `Revolt`; the words for each come from progression.ron `chronicle.causes`.

**Explicit event lists.** Today `chronicled()` is a list of exclusions and `notable()` is `chronicled() || RequestPosted`, so a new kind would be chronicled and notable by default, and "chronicled but not notable" and "feed only" cannot be said. Step 1 adds `EdictSet` and `Tribute` to `chronicled()`'s exclusions (feed only), adds `quiet()` listing the chronicled-but-not-notable kinds (`Forsaken`, `Built`, `StewardNamed`), and makes `notable()` `(chronicled() && !quiet()) || RequestPosted`. No existing kind changes; `every_event_kind_is_classified` pins all 34 in a table.

**Schedule:** four slots appended to `progression::progress` after the director (slot 12), never reordered, each in its own tracing span:

| Slot | Step | Cadence |
| --- | --- | --- |
| 13 | `influence::step`: the field; on the day tick, influence deeds and envoys flagged for slot 7 | hourly |
| 14 | `tenure::step`: `LoyaltyTerms` (summing the terms that `govern.rs` and `influence.rs` expose, so each lane owns its own terms); `Defended` deeds from the hour's battle reports; the watch's hours; on the day tick, scars fade, the submitted watch and defiance, agents | hourly |
| 15 | `govern::step`: hourly, pending edicts applied and the first wave of moves, musters, rations and feasts, then `construction::advance` and Market hire offers; at dawn, stewards and their habits, later waves | hourly |
| 16 | `fallout::step`: due bandit bands spawned; reprisal vows set, which slot 11 reads the next hour | hourly |

M4c's `saga::settle` takes slot 17. Inside existing systems: `orders::apply` (Govern), `run_orders` (Demand, Subvert, the sack, `orders::lands`, the granary floor), `conquest::transfer_seeded` (tenure, the fallout queue, the cause-to-kind map), `invitations::step` (envoys and turned gates), `rival::clock` and `think` (reprisals, the realm share), `raiders::think` (bandits), `director::step` (the count), `combat` (blood, `Killing` at a bandit's home, `succeed`'s carry, the palisade and Walls defence), `recruit_candidates` and `would_be_militia` (skip agents), `requests` (Forsaken, kept), `sellswords` and `drifters` (Market hire, the sphere's ease), `dominion` (rank credit), `apply_goodwill` (the log, `spreads`), `drift_loyalty` (the bias), `update_needs` and `FoodSecurity` (rationing), the AI (the green, the run-gated smithy fixes, haul jobs for sites), `threat` (watchtower, Walls), populate (specialties), and `setup_run` (inserting the new components).

**Writers stay single:** goodwill only through ledger deeds; ownership only through `transfer_seeded` (plus `rival::setup` and `free_lands`, as today); `FactionModifiers` untouched (per-town effects live in `Edicts`, `Tenure`, `Specialty`, the steward's habits and buildings); goods between stockpiles only in the army's logistics step (tithe, levy, sack and bribe are all there). Two serial writes to one town's own stores are new: a feast eats from its storehouse, and a finished site consumes its own stock, following `requests::pay`'s precedent, recorded in ADR 0020.

**Run only.** Every new component is inserted by `setup_run` (or populate's run branch), every consumer reads an `Option`, and every step checks `in_run`. Developed, `--skirmish` and `--battle` stdout stay byte-identical; state hashes may change where data text changes, as in every M4 step. New stream keys: `sim_military::rival::reprisal` only (the steward and his habits, the agent's discovery, the vow's hour, Market hire bands and the realm share are deterministic). Ties in "nearest" go by distance, then SpawnId. Two things reach every preset and are inert there: `Veteran.trade` is written in `enlist` everywhere (read only for stewards, so state hashes change in all presets while behaviour and stdout do not), and `recruit_candidates` and `would_be_militia` gain `Without<Agent>` filters that match nothing without agents.

**Formats.** `SAVE_FORMAT` 1 to 2 and `REPLAY_FORMAT` 5 to 6 (a new command and two orders), **each bumped exactly once, in step 1**: every saved layout lands in the step 1 skeleton, and a later change to one inside M4a amends the type in place (M4a has no saves in the wild to protect between its own steps; M4 saves are refused with the existing message). M4c then takes `SAVE_FORMAT` one above M4a's final value (3 as planned) and keeps `REPLAY_FORMAT` at 6.

**Cost.** Hourly per-settlement steps over 25 towns, 625 distance terms, at most 100 reassignments a day, two small bandit armies, one `Has<Rationed>` read per villager per tick on its own archetype, and at most one site per town in the carters' job loop. The overlay is computed render-side on a coarse grid, every two seconds while shown. NFR-11 is measured twice, because a botless run owns nothing and never sets a dial, so it exercises only the rival's influence and the smithy fixes: (1) the run island's live mean tick, at most +5% over the M4 baseline recorded in `docs/m4a/` (the always-on parts); and (2) in a bot run (`--run --bot --seed 42`, the windows before the win, never ticks after a run ended), Lordship's own stages (slots 13 to 16, `orders::lands`, `construction::advance`, `jobs::reassign`) together stay under 5% of the mean tick, which is where stewards, edicts, levies, tithes, bandits, reprisals and construction actually run.

## 12. The bot

`crates/sim_headless/src/bot.rs` plays through ordinary player commands, and learns each verb in the plan's step that adds it:
- **Its view** gains each town's tenure, influence net, edicts, specialty, demand defence, storehouse arms, and a reprisal's pinned town (the player sees it in the vow's feed line, so it isn't cheating).
- **Choosing how to take a town** (`take_mode`, from `main_task` and `conquest_target`), in order: Demand if its strength is at least 1.1 × the ratio × the defence; Subvert a rival town with loyalty under 0.6, no rival soldiers within 45 and a villager to send; Court an independent town with goodwill at least 0.35 and food to spare beyond three days (gift, then wait up to a day); otherwise Attack. It already accepts invitations, envoys and turned gates included.
- **Governing** (`govern`, daily). It reads each steward's pending habit (the forecast the panel shows) and overrides it where it plays better: Forge at its best smithy town, mining first, whatever the steward would set; Tithe at towns near its staging line; Levy at big loyal towns, and Light where a Stern steward would levy a sullen one; a smithy in a mining town without a weapons recipe; a palisade at the holding nearest the rival's seat. A `--no-overrides` flag leaves every habit alone, for the comparison below.
- **Holding:** garrison the newest holding while a vow names it; keep strength near a submitted town until it settles.
- **Bandits** aimed at a rival town and more than 60 tiles away are left out of its foes for a conquest: they are a chance, not a threat.

**What the bot tests prove.** A floor-only gate (win 3 of 5) cannot catch the likeliest failure: weapons (+8 attack), passive levies and tithes and envoy snowballs making runs trivially easy, or the new verbs being decoration. Envoys and turned gates cost the bot no skill, so counting them would let a bot that never chooses anything pass. So:
- **`bot_run`** (ignored, release, in CI) must still win 3 of 5 seeds on steady in 40 days, with the quiet limit, **and a win-day floor**: no seed is won before day 14 (M4's earliest was day 15.5), and the median win day stays within four days of the M4 baseline's median, recorded in `docs/m4a/baseline.md` at step 1. Too early means the weapons, tithes or envoys are too strong; the fix is data.
- **`bot_uses_lordship`** (new, ignored, release) counts what the bot chose, not what happened to it. Over the five seeds: at least 30% of its takes are not Attacks, counting only `Submitted`, `Subverted` and an `Invited` town that the bot had gifted in the three days before (envoys and turned gates by influence alone are listed but do not count); at least three successful Demands in all; and at least one agent-turned or gift-courted town.
- **`lordship_beats_the_attack_bot`** (new, ignored, release) proves the choices matter. An Attack-only bot (`--bot-style attack`: every town by Attack, no gifts, demands or agents, no dial ever set, so stewards' habits run) plays the same five seeds. The Lordship bot must win at least as many, and on the seeds both win, win earlier on median by at least a day.
- **`overrides_change_outcomes`** (new, ignored, release) proves stewards matter. The Lordship bot against a copy run with `--no-overrides` on the same seeds: the two differ in win day by a day or more, or in towns lost to revolt, on at least three seeds, and the overriding bot wins at least as many.

The step notes record per seed: the win day, takes by way, defiances, Forsaken deeds, weapons made and tithed, men levied, bandit bands and their raids on rival and player towns, reprisals and retakes, and revolts. Per HOWTO "Tune run balance": change data when the game is unbalanced and the bot when it plays badly, and loosen a threshold rather than fudge the data.

## 13. Tests

The plan lists each with its step. In brief, new files in `tests/tests/`: `tenure.rs` (terms, scars, the log, the seat, the cause-to-kind table, the explicit event lists), `influence.rs`, `take_modes.rs`, `fallout.rs`, `edicts.rs`, `stewards.rs`, `construction.rs`, `specialty.rs`, `smithy_chain.rs`; the ignored bot comparisons live in `bot_run.rs` beside `bot_run`; updated: `rival.rs` (reprisals in the books, the sack stays the player's, the seat follows its realm), `save_load.rs` (mid-construction, a pending vow, bandits abroad, an agent at work, edicts locked), `determinism.rs` (a bot run's replay with the new command and orders), `conquest.rs` and `invitation.rs` (re-run: the new terms move the drift they guard), `village_day` and `famine` (unchanged: the gate), and `bot_run` with `bot_uses_lordship`.

## 14. What later milestones inherit

**M4c "Saga"** (`docs/SAGA_DESIGN.md`, `docs/M4C_PLAN.md`) now follows M4a. It gains:
- **people with roles:** stewards and agents are named villagers, so quirks, kin, moods and biographies reach them for free; bandits are real villagers whose deaths can be told;
- **more rescues:** bandits and reprisals bring more real drive-off fights near villages, so more rescue records for M4b's memorials; M4a's `Defended` check (a player battle against raiders or the rival near a town) is the plain form of the rescue's "a fight near the village";
- **marks:** a sack is plunder by a captain's army (Cruel), and gifts are food given to a village (Beloved);
- **`Veteran.trade`,** which M4a adds for stewards, and M4c's service records reuse;
- **numbering:** M4c's ADR becomes 0021, its `SAVE_FORMAT` one above M4a's final value (M4a bumps exactly once, to 2, so 3), its `saga::settle` slot 17, and its baselines are taken on the M4a build.

**M4b "Living towns"** (`docs/TOWNS_DESIGN.md`) grows what M4a seeds:

| Area | M4a builds | M4b grows it into |
| --- | --- | --- |
| Town policies (GP-17) | `Edicts`: work focus, dues (tithe as a levy in kind, levy as a conscription quota), table; stewards' habits as data rows | standing orders for captains; policies set for a whole region; more edicts and habits |
| Specialised towns | a specialty from the site, scaling the occupation shares at populate in a run; Market towns post Hire offers | growth adds workplaces of the specialty's trades first; town styles can draw on it |
| Construction (SIM-17) | `Construction`, `choose_site`, `start_building`, carters supplying a site, a timer | `choose_site` takes the next open master-plan plot of the kind (plans reserve seeded plots per kind, as for memorials, so they stay a pure function of seed, site and style); builders' labour replaces the timer; stone and the quarry; growth opens sites through the same entry point |
| Equipment tiers (SIM-37) | a working chain and the Forge focus | `weapons` splits into tiers, and Forge makes the best tier the town can |
| Carters between towns (SIM-23) | none: armies within reach collect tithes and levies | carters may carry tithes to the seat or an army |
| Walls (GP-15) | the palisade, one centre structure | the Town tier's palisade is the same building kind; city walls as tiles; M5's sieges read them |
| The smithy fixes | gated to runs | M4b removes the gate when it re-baselines `village_day` and `famine` |

**M5 hooks** (nothing built now):
- **Reputation:** a faction-wide aggregate over the typed deeds (Plunder, Killing, Threat, Forsaken, Plot against Gift, Festival, Defended, Request) and over `Tenure.how` across towns ("how you rule"). Deeds already carry the faction and the kind.
- **Village memories (SIM-14):** `DeedLog` (dated deeds) and `Tenure.history` grow into memories without a schema change beyond longer rings.
- **Obligations and dilemmas (GP-14):** tenure terms are ready carriers: an invited town that expects defence while you are busy elsewhere, a submitted town's tribute in a famine, a forge town starving under its edict. The forecasts already make the choices legible.
- **The rival by your rules (GP-09):** `Govern`, `Demand` and `Subvert` are commands the rival can issue with `CommandOrigin::Ai`, and influence already computes its share. Rival towns posting drive-off requests (so you can court them) also waits for M5.
- **Knowledge (SIM-40):** a demand's strength and an agent's "rival soldiers near" read ground truth now; M5 swaps in faction knowledge.

## 15. Traps and guards

| Trap | Guard |
| --- | --- |
| **A menu game:** numbers change in panels, the map doesn't (the playtest complaint) | the weight test (section 1): every verb has a visible effect on people or the map within an hour and a feed line; edicts move villagers, not hidden multipliers; steward traits are habits, not yield bonuses; specialties are real shares with one visible flow or none; the inspector says why someone changed jobs |
| **King of the hill in new clothes:** the take verbs still start by marching to a point, and the climax is one point | section 1 states which verbs are army-at-a-point and which are not; the Demand watch is about holding land, not parking; influence, envoys, agents, edicts and stewards need no army; the seat's defence follows the share of its realm you leave it (section 6.1); the playtest judges the mix |
| **Governing is optional** (stewards do it all, so nothing you click matters) | a steward is a personality, not an optimiser: his habits are visible, sometimes against your interest, shown a dawn ahead and overridable; dials no habit touches stay at their defaults; `overrides_change_outcomes` and a playtest question ask whether overriding mattered |
| **Micromanagement** (15 towns × 3 dials) | stewards run the dials their habits touch, shown ahead; dials are sticky and lock for two days; one building at a time; one Dawn line a day; most towns are never opened |
| **Snowballing** (influence brings towns, which bring influence) | square-root power and saturating shares; resentful and fresh towns project little; goodwill from influence alone stops at 0.6; one envoy a day; the seat is immune; reprisals hit the newest holding; bandits grow with rapid violent takes; a submitted town ties up an army |
| **One way dominates** | each way pays in a different currency (blood; a standing army; food and a duty; a man, goods and time); `bot_uses_lordship` checks the mix; the playtest asks |
| **Flags again:** passive bonuses, no new verbs | holdings unlock verbs and flows: arms, tithes, levies, envoys, buildings, hired men; specialties change what a town can do, or are only people |
| **Alarm fatigue** | at most two bandit bands; one vow at a time, two days apart; only three kinds pause the game; Dawn lines group the day's small news; palisades, watchtowers and Walls let towns defend themselves; bandits aim mostly at the rival |
| **Opaque randomness** | every threshold is deterministic and shown; no new roll in any rule (the only new stream picks a reprisal's lead) |
| **Farming your own fallout:** killing a taken town's bandits pays goodwill | `Defended` skips battles against bandits from that town, and killing them posts `Killing` at their home (section 7.1) |
| **The rival's books drift:** a posture rule sacks every retake | the sack is the player's rule; the rival's retakes are Conquered (section 5.1) |
| **Weapons flood** (+8 attack) | the Forge cap, the input target and ore; measured at the smithy step before edicts exist; `bot_run`'s win days recorded, not only wins |
| **Breaking invariance or determinism** | run-only components and consumers; gated fixes; append-only enums; format bumps; the three stdout comparisons at every step |
| **Scope creep into M4b** | no master plans, builders, stone, tiers or carters between towns; `choose_site` and `Edicts` are the interfaces M4b grows |
| **The rival feels static** | reprisals answer your takes; its sphere contests yours; its towns can be turned; rival edicts wait for M5 |

## 16. Proposed roadmap change and new IDs

| Milestone | Was | Proposed |
| --- | --- | --- |
| **M4a "Lordship"** | — | New, after the M4 playtest and before M4c: four ways to take a town with lasting tenure, the "why" lines, influence spheres, bandits and reprisals, town edicts, stewards, building orders, specialised towns and a working smithy chain. Splittable after its step 6 (part 1 "Take and hold", about 59%; part 2 "Stewards and works", about 41%, which would follow M4c's part 1 if split off) |
| M4c "Saga" | after the M4 playtest | after M4a; ADR 0021, `SAVE_FORMAT` one above M4a's final value (3 as planned), slot 17, baselines on the M4a build |
| M4b "Living towns" | town policies, construction, tiers | grows M4a's edicts, building order and weapons chain (section 14) |
| GP-17 | M4b | M4a (town edicts), M4b (standing orders; policies grow) |

New rows, added to SEED.md on 2026-10-13 (SEED.md's wording is canonical): GP-31 influence, GP-32 four ways to take a town, GP-33 fallout, GP-34 governing; SIM-60 tenure and loyalty terms, SIM-61 the deed log, SIM-62 specialties, SIM-63 building orders, SIM-64 the smithy chain; UI-11 the settlement panel's "why", ways to take and governing; UI-12 the influence overlay; RND-12 sites, new buildings, crowds and town marks; NFR-11 Lordship's cost.

## Decisions (2026-10-13)

Yours, binding:
1. **A new milestone before M4c,** played in the order M4 playtest, M4a Lordship, M4c Saga, M4b Living towns, M5, M6. Named "M4a: Lordship" (section 2).
2. **Passive influence and strength,** verbatim: "influence SPREADS from the towns you hold on its own: nearby free towns warm to you and may join without a fight; your holdings feed your strength (levies, tithes, renown-like power) without micromanagement; a visible influence map shows your sphere growing (and contesting the rival's)." (section 6; "renown-like power" is open question 16)
3. **Conquest fallout, both ways:** violent takes raise bands of broken men who harass the rival's lands harder but may hit yours, and the rival sends reprisals against your newest holdings (section 7).
4. **Four ways to take a town, each with a lasting state:** conquer, intimidate, court and subvert; starving a town out stays in M5 (section 5).
5. **Govern your towns:** edicts with visible trade-offs that change what villagers do, building orders, a named steward whose traits matter, and specialised towns worth targeting (section 8).
6. **Not now, hooks only:** faction-wide reputation and village memories (M5), obligations and dilemmas (GP-14, M5) (section 14).
7. **M4b overlap:** Lordship pulls forward only edicts, a minimal building order and the smithy chain, in a form M4b grows from; M4c follows Lordship (section 14).

Defaults I chose to make the design concrete (change any):
8. **Influence is land and people:** people and loyalty, with no multipliers for specialties or buildings. Whether your holdings also feed "renown-like power" is open question 16, with a recommended reading that adds no renown number and no title ladder.
9. **The seat can only be conquered:** never swayed, demanded, courted or subverted.
10. **Sacking is the posture at the hold,** not a new verb, with a forecast.
11. **Bandits are the town's own villagers,** not fresh soldiers, and they never take towns, only plunder.
12. **Bandits don't count toward the director's `max_bands`,** but do count for its tension.
13. **Everything is run-only,** including the smithy fixes (M4b removes that gate).
14. **No new rolls:** stewards, discovery and vows are deterministic; only a reprisal's lead uses a new stream.
15. **Every owned town gets a resident steward automatically;** appointing a follower is optional and costs a man.
16. **Edicts start within the game hour and lock for two days from then;** later waves go at dawn. The steward is a personality: his habits set only the dials they touch, shown a dawn ahead and overridable.
17. **Two building orders per town per run, one at a time, never cancelled.**
18. **ADR 0020 is Lordship's;** M4c's planned ADR becomes 0021.
19. **The sack is the player's rule;** the rival's retakes are Conquered, because its bands are always Aggressive (section 5.1). The alternative is question 17.
20. **Demand's watch is counted by the hour and satisfied by land:** an army, a garrisoned neighbour within 80 tiles, or your sphere (section 5.2).
21. **The seat's defence follows the share of the rival's realm it still holds,** floor 0.4 (section 6.1).
22. **Subversion closes unvisited:** whispers +0.05 and suspicion 0.06 + 0.08 × loyalty, so an unvisited agent turns a fed town on about day 6 and is caught on about day 10 (section 5.4). Agents are never recruited and never take up arms.
23. **Bandits' deaths are remembered at home:** `Killing` at their home, and no `Defended` for fighting them (section 7.1).
24. **M4a bumps each format exactly once, in step 1;** M4c takes `SAVE_FORMAT` one above M4a's final value.

## Open questions (with recommended defaults)

1. **Can influence alone bring a town in?** *Default: only an independent town inside your sphere (net 0.5) that already likes you (goodwill 0.5), as an invitation card you may decline, at most one a day.*
2. **Demand on rival towns?** *Default: yes, except the seat, weighing the garrison fully (the rival's non-seat towns have none, so their defence is their militia).*
3. **Subvert independent towns too?** *Default: no; courting covers them, and agents stay the rival's problem.*
4. **Agents as real residents, or an abstract record?** *Default: a real villager follower who goes to live there; a band with no villager can't send one.*
5. **Bandits from fresh soldiers instead of villagers?** *Default: villagers: the town's loss and the walk home are the point.*
6. **Reprisals: extra, or replacing scheduled raids?** *Default: extra, at most one every two days, each pushing the next scheduled raid back by half a period.*
7. **What triggers a reprisal?** *Default: taking a rival town or a town where the rival's share is at least 0.3, and a caught agent; it always strikes your newest holding when it launches.*
8. **Tithe delivery.** *Default: collected by owner armies within 60 tiles, as a dealing in `run_orders`. The alternative, provisioning anywhere in your sphere, teleports goods against the sim's grain; M4b's carters do it properly.*
9. **Does the steward set dials on his own?** *Default: yes, by habit only: a dial no habit touches stays at its no-op default, the choice is shown a dawn ahead, and he never touches a dial you set; Release hands one back.*
10. **Edict timing.** *Default: the first wave in the next game hour, later waves at dawn, locked two days from the hour it started (one with a Reeve).*
11. **Ordered buildings.** *Default: two per town per run, one at a time, at most one each of granary, palisade and watchtower, at most one extra smithy, never cancelled.*
12. **Specialties reshape the run island's populate.** *Default: accept, run only: the island's layout and SpawnIds change (BUILD_GUIDE G10 allows it in the run preset) while RNG draw counts stay the same; re-tuned with `bot_run` at their step.*
13. **The edict factor in the `DecisionTrace`.** *Default: don't widen `Factors` (M4c widens it to five for quirks); the inspector derives the edict reason from the town's state.*
14. **Does the rival govern its towns?** *Default: no; its levy clock already acts as a levy, and rival edicts come with GP-09 in M5.*
15. **Split point.** *Default: build all eleven steps. A 15-minute smoke playtest at merge point 2 (Demand, the sack, edicts and the overlay) is your go or hold for part 2 before building orders start; part 1 (steps 1–6) is handed over for a fuller playtest at merge point 3. If you split part 2 off, it follows M4c's part 1 ("Deaths and oddities") and keeps the slots and formats step 1 reserved (see the plan).*
16. **"Renown-like power": which reading?** You asked for holdings to feed your strength with "levies, tithes, renown-like power". PROGRESSION_DESIGN (2026-10-07) says renown and titles stay out. *Default (built unless you overrule): the power of a realm without a renown number or a title ladder: captain slots from people held (exists), a small champion rank credit per 500 people held a day, and a lower goodwill line for drifters inside your sphere. Overrule to "no extras" (drop both small hooks) or to "a renown score that unlocks things" (a bigger design, not built now).*
17. **Should the rival sack the town it retakes?** *Default: no; its retakes are Conquered with no plunder, so the books stay as they are. The alternative, a reprisal that sacks ("they will sack it" in the vow line), raises the stakes and changes the rival's logistics.*

## Later (not in M4a)

- Starving a town out, sieges and walls as tiles (M5, GP-15).
- Master-plan plots, builders, stone, carters between towns, equipment tiers and growth (M4b).
- Reputation scales, village memories beyond the deed log, dilemmas and obligations as choices (M5, with the hooks in section 14).
- The rival issuing edicts, demands and agents (M5, GP-09).
- Bandit-held towns, and new factions for them (bandits are RAIDERS).
- A renown score or a title ladder (question 16 builds only the small land-and-people reading).
