# Design: what makes this kind of game fun

Status: **decisions made 2026-10-06; requirements added to SEED.md**. It surveys what players love in this genre and recommends where each idea fits the roadmap. Per SEED.md's scope rule, nothing joins a milestone until it's approved, and the M3 playtest comes first.

Closest relatives: RimWorld, Dwarf Fortress, Crusader Kings, Mount & Blade, Battle Brothers, Manor Lords, Stronghold, Total War, Kenshi, Songs of Syx, Against the Storm.

## Our edge

Every villager is simulated: a name, a home, a job, needs and reasons. The games people talk about for years (Dwarf Fortress, RimWorld, Crusader Kings) are loved for **stories that happen by themselves**, and our simulation already produces them; today they're just invisible. The highest-value work is making those stories visible, personal and consequential, not adding systems.

## What players enjoy, and what it means here

### 1. Stories they retell (emergent narrative)
- **Chronicle and legends.** Dwarf Fortress has its legends mode; Crusader Kings has its chronicles. An automatic history of the island: who burned which village, the famine of year two, the hero who held the bridge. Each settlement and named character keeps a short biography, and the end of a campaign shows a timeline. *Cheap: it's built on the M4 event feed.*
- **Sim-grounded dilemmas.** Crusader Kings and RimWorld do these. A few decisions arise from real state, not random pop-ups. "Oakworth is starving: open your army's grain stores?" "Deserters caught: hang them or pardon them?" Each choice changes loyalty, morale and memories. *Moderate.*

### 2. Attachment to characters
- **Traits and veterans.** Battle Brothers and Mount & Blade. Captains, heroes and long-serving soldiers earn traits from what they lived through: Brave after a held line, Cruel after plunder, Beloved after feeding a famine town, Shaken after a rout. Traits nudge their AI and morale. Veterans get ranks and names you notice. *Moderate.*
- **Nemesis rivals.** From Shadow of Mordor, and Kenshi's bounty hunters. Enemy captains who survive remember you, return stronger and hold grudges; villages remember who burned them (SIM-14 memories). *Moderate; best with M5's rival warlords.*
- **Succession.** Crusader Kings. When your champion falls, an heir or second takes over, and the story continues instead of ending. *Cheap: M3 already passes command on.*

### 3. Spectacle and readable consequences
- **Sieges and walls.** Stronghold, Manor Lords and Total War; the most-requested medieval feature in every community. Cities get walls in M4b. Sieges add blockade (starve them out, which uses our real food simulation), assault, and siege equipment (GP-08). *Large; a natural M5 headline.*
- **Fire, weather, smoke and ruin.** Plundered villages burn and are rebuilt, rain and snow change marches (SIM-06 is currently "Later"), smoke rises from chimneys, and ruins tell what happened. *Moderate; mostly render-side.*
- **Follow-cam, time-lapse and photo mode.** Manor Lords, Cities: Skylines. Follow one villager through their day, watch a hamlet become a city in sped-up time, take screenshots. *Cheap; render-side.*
- **Audio early.** Village ambience, battle noise, and music that follows tension do more for "feel" than any single system. SEED.md puts audio in M6; a basic pass in M4b would make every playtest richer. *Moderate.*

### 4. Decisions under pressure
- **Scarcity and logistics.** Against the Storm, Battle Brothers. Food already drives everything, which is good. Making supply visible adds to it: baggage carts that follow armies, and trade caravans that can be raided (M4b carters). *Cheap, since the simulation exists.*
- **Delegation and standing orders.** Bannerlord's party AI, Total War's automation. With indirect control, the fun is in **orders and policies, not micromanagement**. Captains get standing orders (defend this region, patrol roads, forage within 20 tiles, retreat below 50% strength) and town policies (rationing, conscription quotas, taxes in kind). *Moderate; essential once there are 9 captains.*

### 5. Goals, pacing and mastery
- **Storyteller and pacing director.** RimWorld's storytellers. A director spaces threats, calm and crises so there's no dead time and no unfair pile-up, with selectable personalities: steady, chaotic, harsh. It drives the scripted raiders now and the rival warlords later. *Cheap to start.*
- **Renown, ambitions and victory paths.** Mount & Blade renown, Crusader Kings ambitions. Short-term ambitions ("hold three villages through winter"), renown that unlocks more captains and recruits, and several ways to win: conquest (GP-10), prosperity (the greatest city), faith (heroes and holy sites). *Moderate.*
- **Pacing controls.** Auto-pause on chosen events, notification filters, and "fast-forward until something happens" so long marches don't drag. *Cheap.*

### 6. Replayability and self-expression
- **Origins.** RimWorld scenarios, Kenshi starts. Different starts: exiled lord (few men, good gear), bandit chief (feared, aggressive), prophet (faith-led), merchant (rich, no army). *Moderate.*
- **Heraldry and naming.** Bannerlord's banner editor. Design your banner and colours; name your captains and the towns you found or take. *Cheap.*
- **Shareable seeds and replays.** Our determinism makes a replay a tiny file: share a battle or a whole campaign, and run weekly seed challenges. *Cheap: the harness exists.*
- **Modding.** RimWorld's lifeblood. Game data is already RON; add a mods folder that overrides or extends it, and a workshop later. *Moderate.*
- **Diplomacy.** Truces, tribute, marriages, alliances and betrayals with AI warlords. *Large; belongs with M5 rivals.*

## Traps this genre falls into

| Trap | Guard |
| --- | --- |
| Micromanagement fatigue at scale | Standing orders, town policies, notifications instead of babysitting |
| Dead time (long marches, waiting for growth) | Pacing director, fast-forward to the next event, early audio and spectacle |
| Unreadable outcomes ("why did my army rout?") | Inspector traces (we have them), chronicle entries that state causes, end-of-battle reports |
| Snowballing (late game becomes a formality) | Overextension: loyalty and revolts, rival coalitions against the leader, harder sieges, succession crises; from M4a, influence that falls off and is contested, bandits from violent takes and reprisals at the newest holding |
| Towns as flags (holding a place changes nothing you can see; the M4 playtest's verdict) | M4a "Lordship": ways to take a town that leave a lasting state, edicts that move villagers, a steward, building orders, influence that acts on neighbours, and a panel that says why (`docs/LORDSHIP_DESIGN.md`) |
| Slow start (every settlement starts as a hamlet) | A strong opening ambition, an early raider beat from the director, quick first growth |

## Adopted (2026-10-06)

| Feature | Requirement | Milestone |
| --- | --- | --- |
| Chronicle and legends, end-of-battle reports | UI-09 | M4 |
| Traits and veterans | SIM-34 | M4 |
| Pacing director, auto-pause, fast-forward to the next event | GP-16 | M4 |
| Standing orders and town policies | GP-17 | M4a (town edicts, moved up 2026-10-13 after the M4 playtest), M4b (standing orders; policies grow) |
| Fire and ruins; weather (SIM-06, moved up from "Later") | SIM-07, SIM-06 | M4b |
| Follow-cam, time-lapse, photo mode | RND-08 | M4b |
| Sieges and walls | GP-15 | M5 |
| Dilemma events | GP-14 | M5 (needs M5 memories for their lasting effects) |
| Origins | GP-18 | M6 |
| Heraldry and naming | GP-19 | M6 (chosen with the origin on the new-game screen) |
| Shareable seeds and replays | GP-20 | M6 |

**Not adopted for now:** nemesis rivals, audio before M6, renown and ambitions, modding. Victory paths stay as GP-10 in M6.

**Diplomacy** is deferred to multiplayer (GP-13, "Later"), so M5's rival warlords come without diplomacy.

## Original recommendation (for reference)

1. **Chronicle and legends, plus end-of-battle reports:** M4, with the event feed.
2. **Traits and veterans** for captains, heroes and soldiers: M4, with heroes.
3. **Pacing director and pacing controls:** M4.
4. **Standing orders and town policies:** M4b.
5. **Fire, ruins, follow-cam and time-lapse:** M4b, with growing towns.
6. **Basic audio:** M4b (moved earlier from M6).
7. **Sieges, nemesis rivals and diplomacy:** M5, with rival warlords and equipment.
8. **Renown, ambitions, victory paths, origins, heraldry, shareable replays and modding:** M6, the campaign milestone.
