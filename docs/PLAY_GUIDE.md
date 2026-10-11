# How to win your first run

This guide shows one plain, safe way to win a Warlord Sim run on **Steady**, the easiest difficulty. It follows the plan of the game's test bot, which won every Steady island it was tested on, between about day 14 and day 35 (days 18 to 31 on its five main test islands).

> Days here count from the start of the run. The clock starts on Day 1, so "after 10 days" is when the clock shows Day 11.

## 1. What you are doing

You are a **champion** with a small band. A rival warlord rules the island from its biggest town, the **seat**. You win when you take the seat. You lose if your champion dies and no other captain of yours can take over.

You never steer soldiers one by one. You give orders to **captains** (move, recruit, gather food, attack), and their men follow. At first your champion is your only captain. A run lasts some 20 to 30 game days: 20 to 30 minutes at 4x, or 40 minutes to an hour with pauses.

## 2. Starting a run

From the game's folder, run:

```
cargo run --release -p game_app -- --seed 1
```

Seed 1 is a gentle island: the biggest band, 3 days of food and a good harvest. Leave out `--seed 1` for a random island. Steady is the default.

The new-game screen shows the **Seed** and **Difficulty** (keep *Steady*; click *Build it* if you change either), **Your band**, **This island's run modifiers** (one or two twists, such as *Lean year*), and three boon cards under **Choose a boon to begin**.

Clicking a card starts the clock at 1x. Pause it at once: if the **II** button on the clock bar is already lit, the game has paused itself for a waiting offer (such as Sellswords), so leave it. Otherwise press Space, which switches between pause and play.

**Saves.** F5 saves, F9 loads, and closing the game saves too. Each seed and difficulty has one save. To continue, click *Resume* on the new-game screen **before** picking a boon: a boon starts the seed over, and that replaces the save. Always launch with `--release`; a debug build keeps its saves apart.

## 3. The screen in one minute

- **Clock (top centre).** Space pauses and plays; keys 1 to 4 set 1x, 2x, 4x and 8x. A game day lasts 4 minutes at 1x, 1 minute at 4x.
- **Run bar (under the clock).** Your share of each region's people (`R0 12%`), the rival's men, its seat's defence, its next levy and raid, and *Captains besides your champion: used/slots*. Then offer buttons, *>> Next event* (8x until something happens) and *Abandon run*.
- **Event feed (bottom left).** Only the 8 newest lines. Bright lines are news; click one to fly there. Dim lines are your band's log, including your orders and "order refused (...)" with the reason.
- **Captain panel (bottom centre),** when your army is selected: **Men**, **Food** (in days), **Orders**, and the posture buttons *Passive*, *Neutral*, *Aggressive* (the current one is lit).
- **Settlement panel (bottom right):** **Owner**, **Goodwill to you**, **Loyalty** (only if someone holds it), **Asks you:** (only during a request), **Population**, **Food security** and **Storehouse**.
- **Overlays.** O cycles ownership, goodwill / loyalty, food security, monuments, off. You are blue, the rival purple, free villages pale beige.
- **Sim debug** (top left) is a developer box. Ignore it, or collapse it with the arrow on its title.

The game pauses by itself when raiders arrive, the rival raids, your champion falls or an offer is waiting.

WASD pans, the wheel zooms, Q and E rotate. **Don't right-drag to pan while your army is selected:** right-clicks give orders.

**Clicking.** A left-click picks the nearest person first, so to open a village's panel, zoom in and click open ground near its centre. A right-click near a village opens its order menu instead of moving you (*Move here* walks there). Esc closes the menu, but also deselects your army.

## 4. Your first hour

### Pick a start boon

Hunger is the first danger, so pick by your band:

| Your band | Starts with | Pick |
| --- | --- | --- |
| Outlaws | 1 day of food, mostly unarmed | *Well fed* |
| Deserters with spears | 1.5 days of food, all armed | *Well fed* |
| Hunters and farmhands | 2 days of food, mostly armed | *Armed*, else *Known* |
| Kin and neighbours | 3 days of food, all unarmed | *Armed* |

If your pick isn't offered, go down this list: *Well fed* (three more days of food), *Armed* (spears for all; weapons are rare later), *Known* (the nearest village likes you), *Woodwise*, *Hardy*. Skip *Silver tongue*.

### Look around

1. Click your champion (captains are a lighter blue than soldiers) and read **Food**.
2. Read the feed before giving orders. "... far from the rival's seat at *Name*" names the town you must take. If that line isn't in the feed (after a load the feed shows only its latest lines, and saves of runs begun with *Build it*, *New run* or *Same seed again* before 11 October 2026 never had it), press O: the seat is the biggest town (about 450 people) in the purple region, and its panel says "Owner: Rival".
3. Zoom out or pan from your band to find the villages near you. (N and Shift+N fly through every settlement on the island in a fixed order, not nearest first.)

### Recruit, then feed the new mouths

At the nearest free village of a good size:

1. With your army selected, right-click the village and click **Recruit 10**. The feed says something like "7 of Ashford joined Your warband".
2. Hold **Shift**, right-click the same village and click **Gather food**. Shift queues an order instead of replacing the current one.

Recruits bring no food, so your days of food drop with every one. Your band starts **Neutral**; keep it there. P cycles the posture, so one stray P makes you *Aggressive*.

### Keep food above about 1.2 days

Check **Food** at every pause. Under about 1.2 days, get food first:

- **Gather food** at a village with food to spare. At Neutral it hands you half of what its storehouse holds beyond a day of the village's own need. So read the panel's **Storehouse**, not Food security: a day's food is roughly 3 units of grain, fish, mutton or bread per person. Gather where the storehouse holds clearly more.
- **Forage (F)** when no village is near. The band gathers wild food a bit faster than it eats. When the feed says "nothing left to forage here", walk about 50 tiles on and press F again. This feeds a small band; an army of 50 strips a spot in under a day.

**Under 1 day of food, your army helps itself.** *Gather food* then also takes whatever is missing for one day, even from the village's reserve. Without any order, it does the same at the nearest village within 25 tiles that you don't own. Each seizure costs -0.05 goodwill there and a little at neighbours within 150 tiles.

Starving men lose morale, desert, then lose health.

### The first raiders

The first event comes within about 4 game hours, often a raider band. On Steady, raiders are unarmed: 5 men at first, one more every two days, at most 16. They plunder a village near you, then hunt armies **no bigger than themselves**. Stay bigger and they leave you alone.

The plundered village asks you to *drive off raiders*. If you clearly outnumber them, go and fight, and stay within 40 tiles until no raiders have been near for an hour. You earn food, +0.2 goodwill and a champion point.

Villages also ask you to *bring food (x/y)*: answer with **Give food** only when you have well over a day of food, because it hands over everything beyond one day. *Clear the camp* names a guarded monument: skip it early. Ignoring a request costs nothing.

### If you drew Sellswords

If the run modifiers list *Sellswords*, 12 armed sellswords camp about 16 tiles from your start, under a pale beige banner. Clicking one shows "Sellswords for hire: see the run bar". Nobody fights them, and they eat nothing while they wait.

Your champion starts within reach (40 tiles). Click *Sellswords (12)* in the run bar, then *Hire*. It costs 11 food from your champion's baggage, and then the twelve eat with you, so your days of food fall by more than half. Hire if that leaves about half a day, then gather food at once. A refused hire gets a dim feed line saying why, and the button comes back once the clock runs. *Decline*, or a day without an answer, sends them away for good.

### Avoid fights you don't need

Hover over a monument before you go near it. Leave "Held by: Raiders" (an armed camp, up to 24 men) and "Held by: Beasts" (an unarmed lair, up to 18) alone early; camps farther from your start are bigger. Pennants show the holder: red Raiders, brown Beasts, lowered for nobody.

## 5. Growing (the first week or so)

**Recruit around.** Visit free villages, and later your own, with **Recruit 10** then **Gather food**. Rest each village half a day between visits. *Recruit 10* suits villages of about 80 people or more. The rival's villages send nobody.

**Take nearby monuments.** If one says "Held by: nobody", stand your band within 10 tiles of it (the monuments overlay rings that area), alone, for 2 game hours. Each gives a small bonus and 2 champion points. It stays yours while you own a village within 120 tiles or visit it every 3 days.

**Make friends.** Goodwill (-1 to +1) is how much a village likes you. An answered request gives +0.2. At 0.25 a village near your captain sends a few volunteers a day. At 0.5 a free village may invite you in while a captain of yours stands within 40 tiles: click *Invitation: Name*, then **Accept**, and it is yours without a fight. *Known* gives +0.4 to one village, so *Known* plus one answered request there is enough. Elsewhere it takes several requests, and attacks and seizures nearby work against you.

**Take your first village.** Grow to about 30 to 40 men first. Attack only with at least 1.5 days of food and at least 1.6 times the village's fighters plus 5. Fighters (militia) are townsfolk who take up arms, mostly 10 to 20% of the people; plan on 12%:

| Village people | Men you want |
| --- | --- |
| 50 | 20 (the bot's minimum) |
| 120 | about 28 |
| 200 | about 43 |
| 300 | about 63 |

Attack the **biggest** free village within reach that your count beats: more people means more captain slots and region boons. A small village is only the safe fallback. The bot took its first village on day 3.8 with 44 men (118 people, seed 1).

1. Right-click the village and click **Attack**. There is no "are you sure?".
2. The feed says "*Name* takes up arms against Your warband", then "Your warband broke the defenders of *Name*".
3. Your captain then **holds the centre**: within 12 tiles of it for 2 game hours, with no enemy fighters within 35 tiles. The Attack order does this; just don't order the army away.
4. "Day N: Your warband took *Name* (P people), taken by force under *Captain*." Your first village becomes your home: R (*Return*) leads there.

If the feed says "gave up the attack" (the defenders didn't break within a game day) or "failed to take" (12 hours of holding without success), pull back, grow and try again.

## 6. Holding what you take

**Appoint a second captain.** Owning a village gives you a captain slot. A second captain is your champion's **heir**: if the champion dies, it takes over, once per run. With no other captain, the champion's death ends the run. Do this once your army has about 24 men and a day of food, not during an attack:

1. Check the run bar: *Captains besides your champion: 0/1* means a slot is free. *Appoint captain* is greyed while the slots are full; right after a conquest it can stay grey for up to a game hour, so click it anyway.
2. Click one of your soldiers (not the champion), then **Appoint captain**. The new captain takes every second soldier and half the baggage.
3. Click the new captain, right-click your village and click **Garrison**.

**Keep your villages fed and guarded.** A village taken by force starts unhappy. Loyalty climbs while it has food and falls while it goes hungry. Under 35% for a full day, it goes free if none of your soldiers are within 30 tiles, and with too few there its people rise. Watch for "unrest since day N" in its panel.

**Eat from your own villages.** *Gather food* there takes what the storehouse holds beyond a day of the village's need, and never costs loyalty. Your villages also feed your armies by themselves, once a day, when you pass within 25 tiles.

**More captains.** You get a slot per 300 people you rule (at least one), plus boons, up to 8. Appoint later captains when your main army has 40 men or more. When a garrison is under about 16 men and its village has 60+ people, queue there: **Recruit 10**, Shift **Gather food**, Shift **Garrison**.

## 7. Getting stronger

- **Region boons** come at 25% and 50% of a region's people. Good picks: *Seat of power* (+1 captain slot), *Iron discipline*, *Granary of the region*.
- **Champion traits** come as the champion earns points (battles, monuments, villages, requests). Good picks: *Duelist*, *Brave*, *Beloved*.
- **Veterans** who live through 1, 3 and 6 battles hit harder and desert less. Keep them alive.
- **Weapons** are rare. If a storehouse lists weapons, **Equip** there. Don't wait for gear: numbers win.

## 8. Threats

- **Raider bands.** Usually no more than two at once, but a new one comes whenever 10 game hours pass with nothing notable. They reach 16 men by day 22 and go for your village nearest the champion. Keep your main army bigger.
- **The rival.** Every 2 days it drafts 6 villagers into the seat ("levy in"). From day 10, every 3 days, up to 10 men leave the seat ("raid in") to attack your nearest village or monument, or hunt your nearest captain. A raid that beats your garrison takes the village for the rival, so garrison the villages near its region. Meet raids with a bigger army; each raid leaves the seat weaker.
- **The seat's defence** grows: +25% after 5 days, +50% after 10, +75% after 20, +100% after 30. At +100% your blows near the seat do half their damage.
- **Rival modifiers.** *Stronger rival* makes the seat's garrison, levies and defence a quarter bigger; *Weaker rival*, a fifth smaller. The run bar's numbers already include this.

## 9. Taking the rival's seat

**Find it.** The island's biggest town (about 450 people), in the purple region, 1.5 to 3 days' march from your start.

**Count what you need.** The seat holds the rival's men plus about 54 townsfolk who take up arms (12% of 450). They are unarmed and weak, but the seat's defence protects them too:

> Men you need = (Rival men + 54) x (1 + seat defence)

"Rival: N men" counts raiders out too, so this errs on the safe side. Count soldiers across all your captains. The bot adds a margin early and drops it as the clock runs: before day 8 add half again; from day 8 to 16 add a quarter; from day 16 use the plain number; from day 24 it goes with three-quarters, because waiting only makes the seat stronger. Example: 30 rival men at +50% make (30 + 54) x 1.5 = 126 men.

**Gather everyone.** With the main army holding 1.5 days of food, pause, select each captain with 6 or more men in turn, and right-click a meeting point about 70 tiles from the seat, on your side. Wait until all are there, but no more than about 10 game hours.

**Strike together.** Pause again. Select each captain in turn, right-click the seat and click **Attack**. Break the defenders and hold the centre for 2 game hours, as with any village. Then you see **Victory: the island is yours**.

**If it goes wrong** ("gave up the attack", "failed to take", or about 40% of the men you needed left), press Backspace, march home with R, regrow and strike again before the defence climbs.

## 10. Common mistakes

- **No heir.** Some of the bot's losses came from a lone champion dying with no other captain.
- **Overstretching.** Take only the villages you can feed and guard.
- **Aggressive posture.** It plunders every village it passes whenever it has under 3 days of food, raising their militia and costing goodwill. *Passive* gets no food without trade goods. Stay *Neutral*.
- **Stuck orders.** If "Orders:" never changes, press Backspace and give a new order.
- **Offers.** *Later* only closes the cards. Invitations last 24 hours, sellswords one day.
- **Abandon run** asks "Really abandon?"; a second click within 6 seconds ends the run for good and deletes its save. The end screen then offers *New run* and *Same seed again*.

## 11. Cheat sheet

| To do this | Press or click |
| --- | --- |
| Pause / speed | Space (pause and play); 1, 2, 3, 4 for 1x, 2x, 4x, 8x |
| Skip to the next event | *>> Next event* in the run bar |
| Select your army | Left-click your champion or any of your blue soldiers |
| Move | Right-click the ground (hold Shift to queue) |
| Village orders | Right-click near a village: Gather food, Recruit 10, Trade, Equip, Give food, Attack, Garrison, Move here |
| Open a village's panel | Zoom in, left-click open ground near its centre (not a person) |
| Forage | F |
| Cancel all orders | Backspace |
| Return home | R |
| Posture | P cycles Passive / Neutral / Aggressive (or the panel buttons). Keep Neutral lit |
| Appoint a captain | Check *Captains besides your champion: x/y*, click a soldier (not the captain), then *Appoint captain* |
| Answer an offer | Click its button in the run bar, then a card (*Later* closes the cards) |
| Overlays | O |
| Fly through settlements | N / Shift+N (whole island, fixed order) |
| Pan, zoom, rotate | WASD or arrows (or middle-drag); mouse wheel; Q / E |
| Close the village menu (also deselects your army) | Esc |
| Quicksave / quickload | F5 / F9 |
| Continue a saved run | *Resume* on the new-game screen, before picking a boon |

**The plan, day by day (Steady):**

- **Start:** pick the boon for your band (food first). Pause, unless II is already lit. Read the feed for the seat's name. Click your champion and read Food.
- **First 2 days:** Recruit 10, then Shift Gather food, at the nearest big free village. Keep Food above 1.2 days: gather where the storehouse has food to spare, or forage (F) and move 50 tiles when the land is bare.
- **First 3 days:** drive off small raider bands near you, take unguarded monuments, and hire the sellswords if you drew them.
- **Days 3-8:** with 30-40 men, 1.5 days of food and 1.6 x the militia + 5, Attack the biggest free village within reach that you beat. Let the captain hold the centre for 2 hours.
- **Right after:** appoint a captain (your heir) and Garrison the village.
- **Days 4-15:** recruit, feed, take villages, accept invitations, pick boons and traits. Keep every village fed and guarded, and every monument visited or near a village of yours.
- **From day 10:** the rival raids every 3 days. Garrison the villages near it and meet the raids with bigger armies.
- **Days 16-24:** when your captains together reach (Rival men + 54) x (1 + seat defence), meet 70 tiles from the seat, then all Attack at once.
- **The end:** hold the seat's centre for 2 hours. Victory.
- **If the assault fails:** pull back, regrow and strike again before the defence climbs.
