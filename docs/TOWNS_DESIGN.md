# Design: growing towns and town styles

Status: **decisions made 2026-10-06; built in milestone M4b, after M4 and M4c**. Hero memorials (section 5) were added 2026-10-12. Requested: settlements start small and grow into towns and cities over time, and each settlement gets an urban design style drawn from the world's major building traditions, chosen per seed. Buildings keep their purposes, so only their placement and appearance vary.

## 1. Growth: hamlet to city

### Tiers
Tiers are defined in a new `towns.ron` file, with thresholds as data.

| Tier | Population | What it has |
| --- | --- | --- |
| Hamlet | up to ~40 | a few houses, fields or pasture or fishing, a shared store pile |
| Village | up to ~150 | storehouse, shrine, bakery, woodcutter, carters |
| Town | up to ~500 | more of every workplace, a smithy, a market square, a palisade |
| City | 500+ | stone walls with gates, a large temple, a second storehouse, dense housing |

Each tier adds more of the **same building kinds** and better-looking versions of them: thatch becomes tile, timber becomes stone. Walls, the market square and the large temple are the only additions. They are drawn and occupy tiles, but serve no new purpose in M4. Walls could give a defence bonus later.

### How a settlement grows
1. **More people.** Births and families moving in from hungry neighbours (SIM-15) fill the houses.
2. **Planning to build.** When the houses are about 90% full, food security is above a threshold and the storehouse holds enough wood, the settlement opens a **construction site** on the next plot of its master plan (below).
3. **Building.** Builders (idle adults, and carters when free) carry materials from the storehouse and work the site. When it's done, the building appears and the settlement's workers are reassigned to the new workplaces, using the occupation shares from `villagers.ron`.
4. **Promotion.** A settlement crossing a tier threshold is promoted, the event feed reports it ("Oakworth is now a town"), and higher-tier plots open up.
5. **Decline.** When a settlement loses people (famine, war, migration), empty houses decay into ruins that can be rebuilt later. A sacked city visibly shrinks.

### Pace
With a 120-day year (8 hours at 1x, 1 hour at 8x), the pace is set in data so a thriving village reaches town in roughly one to two game years and a city in a long campaign. Conquest and famine slow growth or reverse it, and good rule speeds it.

### The world at the start
Each settlement gets a starting tier by a seeded draw weighted by its site: fertility, coast, road junctions. Most start as hamlets or villages, a few as towns, and rarely one city per region (decision 2). The region's starting population is lower than M2's 5,000 and grows over the game, so the M2 and M3 tuning tests will be re-baselined.

## 2. The master plan (simulation side, deterministic)

When the world is generated, each settlement's **style** produces a **master plan**: an ordered list of plots, each with a tile footprint, a building kind and the tier at which it opens. Streets are part of the plan.
- **Growth takes plots in order,** so a town always grows outward in its style: rings for radial towns, along the street for linear villages, block by block for grids, compound by compound for clustered layouts.
- **Street tiles become road terrain** when the plot beside them is built, through `WorldMap::set_terrain`, so pathfinding and the renderer pick them up.
- **The plan is a pure function** of the seed, the site and the style, and is hashed.
- **Plots are claimed against neighbouring plans,** using the reachability mask from M2, so plans never overlap and never cross water.

The style's **layout parameters** live in `towns.ron`: street pattern, plot sizes and spacing, what sits at the centre, how much the town follows slopes, wall shape and orientation. Its **look** lives render-side in `render.ron`. So a style changes where buildings go and how they look, never what they do.

## 3. Town styles

Each style takes its layout and look from a real building tradition: inspired by it, low-poly and procedural, never a replica. Styles are data, so the list can be edited, extended or trimmed.

| Style | Layout | Houses | Centre and shrine | City walls | Suits |
| --- | --- | --- | --- | --- | --- |
| Northern European | organic lanes radiating from a market square | timber-frame, steep thatch, later tile | square with a stone chapel and steeple | curtain wall with round towers | grass, forest |
| Mediterranean | compact, terraced up slopes, stepped lanes | whitewashed walls, low terracotta roofs | piazza with a domed church | stone wall | hills, coast |
| Middle Eastern and North African | dense courtyard houses, winding lanes and dead ends, a covered market street | flat-roofed mudbrick or stone around courtyards | domed shrine with a slender tower | thick earthen ramparts with great gates | dry land, grass |
| Chinese | grid aligned north–south; walled family compounds around courtyards | grey brick, curved tiled gable roofs | temple compound on the central axis | square wall with gate towers | farmland, rivers |
| Japanese | grid streets with an organic temple quarter | timber, deep eaves, hipped roofs | shrine behind a gate | castle keep at city tier | coast, forest |
| Eastern Slavic | linear street village, houses gable-end to the road; towns grow a fortified core (kremlin) | log houses with carved trim and steep plank roofs | wooden church with onion domes | timber, later stone, kremlin walls | forest, rivers |
| Norse | scattered farmsteads becoming a harbour town | longhouses with turf roofs | great hall and stave church | earth bank and palisade | coast |
| West African Sahel | clustered family compounds with shared earthen walls | earthen rooms, flat roofs and conical thatch | earthen shrine with projecting beams | earthen town wall | dry land, grass |
| Southern African | homesteads in rings around a central enclosure; cities with dry-stone enclosures | round thatched huts | central enclosure; conical stone tower | curved dry-stone walls | grass, hills |
| South Asian | grid of courtyard houses around a stepped water tank | plastered walls, flat roofs, verandas | temple with a tall curved tower | fort walls with bastions | farmland, rivers |
| Southeast Asian | along rivers, shores and roads; houses on stilts | raised timber, steep thatch or saddle roofs | stupa or tiered temple | timber stockade | coast, wet lowland |
| Mesoamerican | grid around a great plaza | flat-roofed plastered houses | stepped platform shrine | earthworks | forest, hills |
| Andean | terraces on slopes; walled blocks of houses around shared yards | fitted stone, steep thatch | platform temple | fitted-stone walls | hills, mountain edges |
| Central Asian steppe | round-tent camps at hamlet tier, becoming mudbrick towns with a caravan inn | round tents, then domed brick | domed shrine | mudbrick wall | grass, dry land |

### What a style shapes
- **Layout:** street pattern, centre, walls, how the town meets slopes and water. Layout affects pathing and tests, so it is simulation state.
- **Look:**
  - mesh builders per (style, building kind, tier)
  - wall, roof and trim colours
  - the generated textures in `game_render::art` (plaster, brick, thatch, log, tile)
- **Shrine look:** the style's form. The settlement's faith (M4) stays separate, and its colours tint the shrine.
- **Names (optional, decision 4):** settlement and villager names drawn from per-style name lists in `towns.ron`, written as original names in the style's spirit, not real places.

### Choosing styles per seed
The request was random per settlement. Options are in decision 1. All options use a seeded draw, so a given seed always gives the same towns.

## 4. Effect on the rest of the game

- **Economy:** construction needs wood (and stone at town and city tier, decision 3), which gives woodcutters, carters and trade a purpose beyond food.
- **Conquest:** owning a growing town matters more than owning a hamlet. Sacking reverses growth, and walls appear at city tier, ready for sieges later.
- **Simulation LOD (M5):** dormant regions keep each settlement's tier and its place in the master plan, and grow by rate, so a region away from the camera still develops.
- **Performance:** a city of 1,000+ people is more agents in one spot. The 20,000-agent budget (NFR-03) is unchanged, but the island's population curve has to stay inside the M5 LOD budget.
- **Saves:** the plans aren't saved. They are regenerated from the seed and the site, and saves record only how far each plan has progressed and which plots are ruined.

## 5. Hero memorials (added 2026-10-12)

Decided with the saga design (`docs/SAGA_DESIGN.md`, section 14 and decision 14). M4c records who saved a village from raiders; M4b lets the village build them a memorial.

- **Who gets one.** A settlement with a rescue in M4c's records (an answered drive-off request, with its heroes, a legend first) whose hero is named: a legend or someone with an epithet. One memorial per settlement, to its greatest hero (a legend first, then by fame). A later rescue never replaces a standing memorial.
- **Building it.** From village tier, the master plan gains a memorial plot near the centre (on the style's axis or square). It opens like any other construction site (section 1, "How a settlement grows"), after the houses it needs, and takes wood, plus stone at town tier. A hamlet waits until it's a village.
- **Its look, by style** (render-side, in `render.ron`): a statue on a plinth (Northern European), a carved column (Mediterranean), an inscribed fountain (Middle Eastern and North African), a stele on a stone tortoise (Chinese), a stone lantern (Japanese), a roofed and carved wooden post (Eastern Slavic), a rune stone (Norse), an earthen pillar shrine (West African Sahel), a stone cairn (Southern African), a hero stone (South Asian), a small stupa (Southeast Asian), a stela (Mesoamerican), a carved standing stone (Andean), a stone figure (Central Asian steppe). Hovering it shows the hero, the rescue and the epitaph.
- **Small, local effects** (numbers in `towns.ron` `memorials`; exact no-ops where there's no memorial; nothing goes into `FactionModifiers`):
  - **goodwill:** a daily `Memorial` deed toward the hero's faction (+0.01, spread like other deeds);
  - **loyalty:** residents' loyalty target +0.03 while the hero's faction owns the settlement;
  - **recruits:** the hero's faction's recruit chance there ×1.1;
  - **the hero's army:** while the hero lives and serves, their own army's morale target +0.03 within 12 tiles of it.
- **A grave the town tends.** When the hero dies, the memorial becomes a grave: its style's grave marks appear (flowers, lamps, offerings), the chronicle says so ("Ashford tends Wat Nine-Kills's grave now"), and the army effect ends while the goodwill and loyalty effects stay. A sacked town's memorial can burn (SIM-07) and be rebuilt like any building.
- **Changing hands.** A conquered settlement keeps its memorial, and its effects still favour the hero's faction, so a memorial to an enemy hero is a small thorn in a conquest.
- **Sim state.** The memorial is a building entity (spawned with `spawn_sim`, hashed and saved) holding its hero's `NameId` and SpawnId, the rescue and its state: standing, grave or ruin. Its plot is part of the master plan, regenerated from the seed with the rest.
- **Across runs.** Only in M6's legends book, as history: the run record gains `memorials` in M4b. Nothing carries into a later run.
- **Tests.** `tests/memorials.rs`: a saved village raises its memorial once it can build one, each effect applies exactly, it becomes a grave when the hero dies, and a village without one is unchanged.

## Decisions (answered 2026-10-06)
1. **Styles are random per settlement, among styles that suit its site.** Stilts go by water, terraces on hills, courtyards in dry land, and the draw is seeded.
2. **Every settlement starts as a hamlet.** *Amended 2026-10-07 for roguelite runs (`docs/PROGRESSION_DESIGN.md`): a run's island starts with a seeded mix of hamlets, villages and a few towns, so there are people to recruit by the final act, and growth is tuned to a 30-day run.* Consequences of the original decision:
   - **Faster pacing.** A good hamlet should reach village size in about half a game year and town size within two (tuned in `towns.ron`).
   - **Small start, whole island.** The starting island holds roughly 60 settlements of 20–40 people, about 2,000 agents. That is cheap enough to simulate **the whole island at full detail at first**, so simulation LOD (M5) becomes necessary only as towns grow.
   - **A developed start for tests.** Tests, benchmarks and `--stress` keep a "developed" start option (a worldgen setting that grows every plan to a chosen tier at creation), so the M2 and M3 scenarios keep their large villages.
3. **Builders and materials:** wood, plus stone from a new quarry (a new good and workplace in `economy.ron`) for town and city buildings.
4. **Per-style names:** not answered, so the recommended default applies: settlement and villager names come from per-style lists of original names. Easy to drop.
5. **Scheduling:** a new milestone, **M4b "Living towns"**, after M4. Growth, town styles, population (SIM-15), trade (SIM-23), seasons and decay (SIM-05, SIM-22), and the seasonal palette (RND-05) move there.
6. **Hero memorials (2026-10-12):** a village saved from raiders by a named hero raises a memorial to them in its town style, with small local effects, and tends it as a grave when the hero dies (section 5, SIM-58). M4c, which now comes before M4b, records the rescues.
