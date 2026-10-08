# Game data reference

Everything tunable lives in `assets/data/*.ron`. Each file is embedded at compile time (`include_str!`), loaded into a resource at startup and validated (a bad file fails a `shipped_data_*` unit test). It is also hashed, so a tuning change changes the state hash.

**Rates "per day"** are converted to per-tick values using `calendar.ron` (2,400 ticks a day).

**After changing a value,** re-run the tests that guard it (listed per file; see [TESTING.md](TESTING.md)).

**Field paths** are written `section.field`. `[]` marks a list of entries.

`tests/tests/docs.rs` checks that every field in every data file appears on this page. Add a row when you add a field.

## calendar.ron (`sim_core::Calendar`)
SIM-04 and SIM-05. Guarded by `sim_core` calendar tests and `village_day`.

| Field | Meaning |
| --- | --- |
| `ticks_per_day` | Ticks in one game day. 2,400 at 10 Hz is 4 real minutes at 1x |
| `phases[].phase`, `phases[].fraction` | The day's phases in order (`Morning`, `Work`, `Evening`, `Night`) and each one's share of the day; the shares must sum to 1 |
| `days_per_season` | Days in a season (30) |
| `seasons_per_year` | Seasons in a year (4), so a year is 120 days |

## worldgen.ron (`sim_world::WorldGenSettings`)
SIM-01 and SIM-02 ([ADR 0005](../adr/0005-world-generation-and-terrain.md)); presets, sizes and monuments are M4 (GP-21, GP-23). Guarded by `tests/tests/worldgen.rs`, and for the `run` preset (monuments, sizes, settlements, regions) also by `run_start` ([tests/tests/run_start.rs](../../tests/tests/run_start.rs)). "Fraction" means a fraction of the map size; `*_tiles_at_2048` values scale with the map size.

**Presets.** The file holds two complete settings blocks, parsed into `sim_world::WorldGenPresets` and picked with `WorldGenSettings::preset(WorldPreset::..)`:

| Field | Meaning |
| --- | --- |
| `developed` | The 2048-tile world of M1-M3 with one populated region: stress tests, benchmarks and every M2/M3 test. `WorldGenSettings::default_data()` is always this preset. It places no monuments and draws no sizes, so it generates exactly what it did before M4 |
| `run` | The 1024-tile run island (GP-21): 4 regions, about 25 settlements that are all populated at a drawn size (about 4,000-5,000 people), and 6 monuments per region |

Every field below appears in both presets.

| Field | Meaning |
| --- | --- |
| `size` | Map edge in tiles (2048 developed, 1024 run); must be a multiple of `chunk_size` |
| `chunk_size` | Chunk edge in tiles (64), used for rendering, hashing and LOD |
| `height.octaves`, `height.frequency`, `height.lacunarity`, `height.gain` | Base fractal noise for the continent: layers, features per map width, frequency step and amplitude step per layer |
| `height.warp_frequency`, `height.warp_strength` | Domain warp for irregular coastlines (strength is a fraction) |
| `height.ridge_octaves`, `height.ridge_frequency`, `height.ridge_weight` | Ridged noise for mountain chains, and how much it adds |
| `height.island_inner`, `height.island_outer` | Radial falloff: land is full height inside `island_inner` and fades out by `island_outer` (fractions from the centre) |
| `height.coast_wobble`, `height.coast_frequency` | How much and how often the falloff radius wobbles around the island |
| `moisture.octaves`, `moisture.frequency`, `moisture.lacunarity`, `moisture.gain` | Noise for moisture, which decides forest and grass |
| `terrain.water_fraction` | Share of all tiles that are water |
| `terrain.beach_width` | Beach tiles from the water's edge |
| `terrain.beach_max_land_quantile` | Only land in this lowest height quantile can be beach |
| `terrain.mountain_land_fraction`, `terrain.hills_land_fraction` | Shares of land that are mountain and hills, by height |
| `terrain.forest_lowland_fraction` | Share of the remaining lowland that is forest, by moisture |
| `settlements.target`, `settlements.min`, `settlements.max` | Settlements to place, and the accepted range |
| `settlements.min_spacing_tiles_at_2048` | Minimum distance between settlements |
| `settlements.core_radius` | Radius of the cleared village green |
| `settlements.farmland_radius`, `settlements.farmland_min_fertility`, `settlements.farmland_falloff` | Farmland around each settlement: radius, fertility needed at the centre (0..255), and extra fertility needed at the edge |
| `settlements.spread_weight` | Placement bonus for being far from other settlements (higher spreads villages inland) |
| `roads.nearest_neighbours` | Each settlement links to this many nearest neighbours |
| `roads.max_link_tiles_at_2048` | Longest road link considered |
| `roads.cost_grass`, `roads.cost_farmland`, `roads.cost_beach`, `roads.cost_forest`, `roads.cost_hills`, `roads.cost_mountain`, `roads.cost_road` | Integer cost per tile for the road builder's A* (orthogonal step; diagonal is x1.41); an existing road is cheap, so roads merge |
| `roads.slope_cost` | Extra cost per unit of height difference |
| `roads.search_margin_tiles` | A* search box margin around each link's endpoints |
| `regions` | Regions the island is divided into (used by population and runs now, LOD in M5). A request: the map has at most one region per settlement, so code reads `WorldMap::region_count()` |
| `names.prefixes`, `names.suffixes` | Syllables for settlement names (prefix + suffix). Monument names use them too, never repeating a settlement's name, so there must be enough combinations for `settlements.max` plus `monuments.per_region` per region |
| `monuments.per_region` | Monuments placed in each region (GP-23): 6 in run, 0 in developed (none). Each slot draws a kind by weight, then takes the suitable tile nearest a seeded anchor in the region. If the kind fits nowhere, the kinds after it in the list are tried in turn, then the spacing is relaxed to a half and a quarter |
| `monuments.min_settlement_distance` | Minimum tiles from every settlement centre (52 in run). Keep it above army.ron `threat.radius` (45), or a guard camp keeps a village frightened; `run_preset_meets_its_criteria` reads both files and checks this |
| `monuments.min_spacing` | Minimum tiles between two monuments (40 in run), where possible: a slot that fits nowhere relaxes it to a half, then a quarter (see `monuments.per_region`) |
| `monuments.kinds[].kind`, `monuments.kinds[].weight`, `monuments.kinds[].site` | The kinds that may be drawn (`StandingStones`, `RuinedFort`, `HolyWell`, `RoadMarker`, `AncientForge`, `MarketCross`, `Wayshrine`, `Watchtower`; each at most once), their relative chance per slot (> 0), and where they may stand. No site is ever Water, Road, Mountain or Farmland |
| `site: NearWater(min, max)` | Open ground (grass, forest, beach, hills) `min..=max` steps from water (holy well: 2-6) |
| `site: Hills`, `site: HillsNearMountain(within)` | On hills; or on hills within `within` steps of a mountain (ancient forge: 6) |
| `site: NearRoad(min, max)` | Open ground `min..=max` steps from a road (road marker, market cross: 1-3) |
| `site: ForestEdgeOrRoad`, `site: GrassOrHills` | Forest next to open ground, or open ground beside a road (wayshrine); grass or hills (standing stones) |
| `sizes.tiers[].tier`, `sizes.tiers[].weight`, `sizes.tiers[].min`, `sizes.tiers[].max` | Settlement size tiers (GP-21): `Hamlet` 20-60 people, `Village` 60-200, `Town` 200-400 in run. Weights are shares of the settlements: tier counts are apportioned by weight (largest remainder), then shuffled over the settlements, and each settlement's people are uniform in its tier's `min..=max`. With 25 settlements the run weights 0.20 / 0.45 / 0.35 give 5 hamlets, 11 villages and 9 towns, about 4,400 people. Empty in developed: no sizes are drawn and population apportions one region as in M2/M3 |
| `sizes.seat_min` | The largest settlement becomes the rival's seat: raised to at least this many people (450 in run) and one more than any other, in the top tier. 0 when there are no tiers |

## economy.ron (`sim_economy::Economy`)
SIM-20 and SIM-21 ([ADR 0007](../adr/0007-economy-and-transfer-resolver.md)). Guarded by `village_day` and `famine`.

| Field | Meaning |
| --- | --- |
| `goods[].name`, `goods[].food` | Goods in id order. `food` is the hunger one unit removes (0 means not food). Recipes, army data and code refer to goods by name |
| `recipes[].name`, `recipes[].workplace`, `recipes[].worker` | A production step, where it happens and who does it (the occupation must work at that workplace) |
| `recipes[].inputs`, `recipes[].outputs` | `(good, quantity)` lists taken from and put into the workplace's stock |
| `recipes[].duration` | Ticks per work cycle |
| `workplaces[].kind` | Workplace kind (`Farm`, `FishingSpot`, `Pasture`, `WoodcutterCamp`, `Mine`, `Smithy`, `Bakery`, `Storehouse`) |
| `workplaces[].slots` | Workers per workplace |
| `workplaces[].radius`, `workplaces[].spacing` | Placement: tiles from the settlement centre, and tiles between workplaces of the same kind |
| `workplaces[].sites` | Site rules tried in order: `Farmland`, `Grass`, `Clearing` (forest cleared as a last resort), `Coast`, `ForestEdge`, `Upland`, `Centre` |
| `workplace_capacity` | Units a workplace holds before production stops |
| `storehouse_capacity` | Units a storehouse holds |
| `home_capacity` | Units a house holds |
| `carry` | Units a villager carries |
| `cart_carry` | Units a carter carries |
| `initial_stock_per_villager` | Starting storehouse stock, per villager (food is about one day, the M2 famine criterion) |
| `initial_home_food_per_resident` | Starting food in each house, per resident |

## villagers.ron (`sim_agents::Villagers`)
SIM-10..13 ([ADR 0006](../adr/0006-utility-ai-and-decision-traces.md)), and loyalty (GP-06, SIM-24). Guarded by `village_day`, `famine` and `posture`; `loyalty` by the sim_agents unit tests in `crates/sim_agents/src/loyalty.rs` and the `conquest` scenario (`tests/tests/conquest.rs`: a fed garrisoned conquest grows loyal, a plundered one loses faith, a starved one revolts within 1-3 days). Every utility score is `weight x` the product of its considerations; a `Logistic(mid, steepness)` curve maps an input in 0..1 to an S-curve centred on `mid`.

| Field | Meaning |
| --- | --- |
| `population.total` | Villagers to populate in the developed world (5,000; tests and the CLI override it). Ignored in the `run` preset, where every settlement is populated at its drawn size (worldgen.ron `sizes`) |
| `population.region` | `Auto` (the region with the most settlements) or `Index(n)` (developed world only) |
| `population.min_per_settlement` | Smallest populated settlement (developed world only) |
| `households.size_weights` | Relative weights of household sizes 1..7 |
| `households.adult_age`, `households.child_age`, `households.elder_age` | Age ranges in years (`min`, `max`) |
| `households.elder_chance` | Chance a household has an elder |
| `households.working_age` | Ages that work (and can be recruited or fight) |
| `occupations[].occupation`, `occupations[].share` | Share of working-age villagers in each occupation; shares sum to 1 |
| `needs.hunger_per_day` | Hunger gained per day (0..1 scale); soldiers use it too |
| `needs.rest_lost_per_day_awake`, `needs.rest_gained_per_day_asleep` | Rest rates |
| `needs.hungry`, `needs.starving` | Hunger thresholds: looks for food, and too hungry to work |
| `needs.sated` | Eating stops at this hunger |
| `needs.meal_ticks` | How long a meal takes |
| `ai.decision_interval` | Ticks between decisions (agents are staggered) |
| `ai.inertia` | Score bonus for the current action, so near-ties don't flip |
| `ai.eat.weight`, `ai.eat.hunger` | Eat: weight and hunger curve (`mid`, `steepness`) |
| `ai.fetch.weight`, `ai.fetch.home_stock_low` | Fetch food home: weight, and the household stock (in days) counted as low |
| `ai.sleep.weight`, `ai.sleep.tiredness`, `ai.sleep.night_drive`, `ai.sleep.night`, `ai.sleep.evening`, `ai.sleep.day` | Sleep: weight, tiredness curve, the minimum urge at night, and the time-of-day factors |
| `ai.work.weight`, `ai.work.morning`, `ai.work.work`, `ai.work.evening`, `ai.work.night` | Work: weight and time-of-day factors per phase |
| `ai.go_home.weight`, `ai.go_home.evening`, `ai.go_home.night`, `ai.go_home.day` | Go home: weight and time-of-day factors |
| `ai.idle.weight` | Idle baseline score |
| `ai.flee.weight`, `ai.flee.fear`, `ai.flee.radius`, `ai.flee.distance` | Flee (SIM-13): weight, fear curve (fear is 1 minus safety), flee only from armies within `radius` tiles, and run `distance` tiles |
| `ai.hide.weight`, `ai.hide.fear` | Hide at home: weight and fear curve |
| `ai.fight.weight`, `ai.fight.courage`, `ai.fight.spread`, `ai.fight.break_morale` | Fight as militia: weight, courage curve (morale plus a stable per-person `spread`), and the morale below which a defender gives up |
| `ai.distance_falloff` | Score multiplier lost per tile of distance |
| `movement.speed.water`, `.beach`, `.grass`, `.farmland`, `.forest`, `.hills`, `.mountain`, `.road` | Walking speed per terrain in tiles per tick; 0 is impassable. These also set path costs and soldiers' speed |
| `movement.slope_cost` | Path cost per unit of height difference |
| `idle.min_ticks`, `idle.max_ticks`, `idle.wander_radius` | How long an idle stroll lasts and how far it goes |
| `combat.health`, `combat.attack`, `combat.defense`, `combat.morale` | A civilian's fighting stats (for militia) |
| `loyalty` | Loyalty to a settlement's owner (GP-06, SIM-24), 0..1 per villager. Once a day each villager who isn't enlisted closes `drift_per_day` of the gap to a target. Under an owner the target is the weighted mean `(w_food x food + w_goodwill x treatment + w_time x time) / (w_food + w_goodwill + w_time)`; an independent settlement's target is `independent_target`. The sim_agents unit test `the_target_weighs_food_goodwill_and_time` checks that a starved conquest aims below `revolt_threshold` and a fed one above its neutral seed of 0.5 |
| `loyalty.food_days` | Days of food (storehouse and homes, `FoodSecurity.days`) that count as fully secure: food = days / `food_days`, capped at 1. Farm stock isn't counted, so at the day tick, when the drift runs, a fed village holds only about 0.45-2 days (the conquest scenario's garrisoned hamlet 0.7-1). At 0.5 a fed village scores (nearly) full food and its conquest grows loyal from the neutral seed; at 1.0 the same village scored about 0.5 and drifted down (M4 step 2 tuning). A starved village scores 0 |
| `loyalty.w_food`, `loyalty.w_goodwill`, `loyalty.w_time` | Relative weights (>= 0, not all 0) of food security, treatment (goodwill toward the owner as `(g + 1) / 2`, so 0.5 at neutral goodwill) and time under the owner in the target |
| `loyalty.settle_days` | Days under one owner until the time term is full: time = days owned / `settle_days`, capped at 1 |
| `loyalty.drift_per_day` | Fraction (0..1) of the gap to the target closed each day, times the owner's `LoyaltyDrift` modifier (`FactionModifiers`) |
| `loyalty.independent_target` | Target loyalty in independent settlements, every villager's starting loyalty, and the loyalty a settlement handed back to independence starts from |
| `loyalty.revolt_threshold` | An owned settlement whose mean loyalty is below this is in unrest (`SettlementLoyalty.below_since`); a day of it risks revolt (sim_military) |
| `goodwill` | How word of a deed travels (GP-22). Deeds (progression.ron `goodwill_deeds`, the "Known" boon) post goodwill changes to `GoodwillLedger`; once a tick (`apply_goodwill`, first in `AgentSet::Standing`) each change counts in full at its settlement and `spread_fraction` of it at every other populated settlement within `spread_radius`, the sums are added (clamped to -1..1) and the ledger is cleared. Checked by `spread_is_order_independent` (standing.rs) |
| `goodwill.spread_radius` | Tiles, centre to centre, within which word of a deed spreads (a day's march is about 312) |
| `goodwill.spread_fraction` | Fraction (0..1) of a change that reaches each settlement in range |
| `names.given`, `names.family` | Original names for villagers |

## army.ron (`sim_military::Army`)
GP-01..04 and SIM-30..32 ([ADR 0010](../adr/0010-armies-orders-and-combat.md)), and conquest, garrisons and revolts (GP-05, GP-06; [ADR 0014](../adr/0014-ownership-conquest-and-loyalty.md)). Guarded by `army_march`, `posture`, `starving_army` and `orders`; `conquest`, `garrison` and `recruit.loyalty_bonus` by `rejects_bad_conquest_and_garrison_data` (in `crates/sim_military/src/data.rs`), the sim_military conquest unit tests, `orders`' `attacking_a_village_raises_its_militia` and the `conquest` scenario (`tests/tests/conquest.rs`). `Army::from_ron` refuses negative or non-finite values. An hour is a game hour, a 24th of a day (100 ticks).

| Field | Meaning |
| --- | --- |
| `factions[].name`, `factions[].color` | Factions by id: 0 independent, 1 the player, 2 the raiders, 3 the rival warlord (M4 runs), 4 monument beasts (M4 runs); these match `FactionId::{INDEPENDENT, PLAYER, RAIDERS, RIVAL, BEASTS}`. Append only, never reorder. RGB colour 0..1. Warlord factions (1 and up) are at war with each other |
| `soldier`, `captain` (`health`, `attack`, `defense`, `morale`) | Base stats. `morale` is also the level that morale recovers to |
| `equipment.weapon_attack`, `equipment.tools_defense` | Bonus from a weapon or tools |
| `combat.engage_radius` | Enemies this close (tiles) are engaged |
| `combat.attack_range`, `combat.attack_cooldown` | Reach in tiles; ticks between blows |
| `combat.roll_min`, `combat.roll_max` | Range of the damage roll multiplier |
| `combat.rout_morale`, `combat.rally_morale` | Rout below; rally above, once no enemy is near |
| `combat.casualty_morale`, `combat.casualty_morale_cap`, `combat.casualty_radius` | Morale lost per allied death within the radius, capped per tick |
| `combat.kill_morale` | Morale gained per kill |
| `leadership.radius`, `leadership.morale_per_day`, `leadership.attack_bonus` | The captain's aura (heroes reuse it in M4): range, extra morale recovery, attack multiplier |
| `leadership.captain_loss_morale` | Morale every follower loses when the captain falls |
| `upkeep.hungry`, `upkeep.sated` | Hunger at which a follower counts as unfed; soldiers stop eating at `sated` |
| `upkeep.unfed_morale_per_day`, `upkeep.fed_morale_per_day` | Morale lost while unfed; recovered while fed |
| `upkeep.desert_below`, `upkeep.desert_chance_per_day` | Below this morale, an unfed or routed follower may desert, with this daily chance |
| `upkeep.starving`, `upkeep.starve_damage_per_day` | Hunger at which starvation damage starts, and how much |
| `upkeep.forage_radius` | Followers eat from the baggage within this, or within the formation if larger |
| `upkeep.first_desertion_within_days` | Acceptance limit checked by `starving_army` (an unfed army starts deserting by then) |
| `formation.spacing`, `formation.separation` | Distance between formation slots; personal space for separation |
| `formation.arrive_radius` | A move order is done within this distance |
| `formation.catch_up` | Followers further than this from their slot descend the rally field |
| `formation.max_per_tile` | A soldier or militiaman closing on a foe won't step into a tile already holding this many fighters, so ranks queue behind a melee instead of piling into it. Marching and captains ignore it |
| `baggage_per_head` | Baggage capacity per person |
| `carry` | Units a soldier carries |
| `max_captains` | Captains per faction (GP-01: 1 + 8) |
| `recruit.radius` | The army must be this close to the village centre to deal with it |
| `recruit.willing` (`passive`, `neutral`, `aggressive`) | Chance per adult asked to join, by posture |
| `recruit.hungry_village_bonus`, `recruit.fed_army_bonus` | Extra chance when the village is short of food, or the army well fed |
| `recruit.loyalty_bonus` | In a village the recruiter's faction owns, extra chance of this times the village's mean loyalty (`SettlementLoyalty.mean`, GP-06). A village owned by another warlord (not independent, not the recruiter) sends nobody, and the refusal is logged |
| `threat.radius`, `threat.passive`, `threat.neutral`, `threat.aggressive` | How frightening an army is to a settlement within `radius`, by posture. Keep `radius` below worldgen.ron run `monuments.min_settlement_distance` (52), or a monument's guard camp keeps a village frightened; `run_preset_meets_its_criteria` in tests/tests/worldgen.rs checks this |
| `gather_keep_days` (`passive`, `neutral`, `aggressive`) | Days of food a village keeps when an army gathers food |
| `gather_target_days` | Gathering fills the baggage up to this many days of food |
| `trade_keep_days` | Villages sell food beyond this many days |
| `trade_value` | `(good, food value)` per unit when an army barters |
| `give_keep_days` | A `Give` order (GP-22) hands the settlement's storehouse the food the baggage holds beyond this many days of the army's need (most nourishing first, as far as the storehouse has room). What actually arrives is counted in `FactionStats.donated`. Checked by sim_military's `giving_hands_over_food_beyond_the_keep` |
| `conquest` | How an `Attack` takes a settlement (GP-05). The army marches until within half `threat.radius` of the centre, then provokes the village and fights its militia. Once the defenders break it calls off the provocation and holds the centre; holding it uncontested hands the settlement over (`conquest::transfer`). Attacking a settlement you own is refused |
| `conquest.break_ticks` | Ticks of fighting before the defenders can count as broken (no militia standing within `militia_radius`) |
| `conquest.give_up_days` | Days of fighting after which an assault whose militia still stands is given up |
| `conquest.militia_radius` | Tiles from the centre: militia defending against the attacker (not routed) this close keep the defenders unbroken, and they or soldiers at war with the attacker contest a hold. Keep it at least `hold_radius` |
| `conquest.hold_radius` | The captain must be this close to the centre (tiles) to hold it |
| `conquest.hold_hours` | Game hours of uncontested holding that take the settlement; any contest restarts the count |
| `conquest.timeout_hours` | Game hours after the defenders broke when a hold not yet won fails and the order ends |
| `garrison` | Holding settlements (GP-05, GP-06). A `Garrison` order (only on a settlement the issuer owns) keeps the army at the centre until it is given other orders or the settlement is lost. A settlement's own owner's armies never frighten it (unless it has risen against them), never pay, plunder or provoke it, and draw its spare food (`FactionStats.supplied`) |
| `garrison.radius` | Tiles from the centre: the owner's soldiers (not routed) this close are the settlement's garrison, and militia risen against the owner this close are its rebels |
| `garrison.keep_days` | An owned settlement feeds its owner's armies (at most once a day each, up to `gather_target_days` of baggage) from the food its storehouse holds beyond this many days for its people. Keep it at least villagers.ron `loyalty.food_days` (sim_military's `shipped_data_loads` checks it), so supply never costs the village its food security, and so its loyalty. A small village rarely has that much in its storehouse: a garrison there lives on its own baggage |
| `garrison.revolt_hours` | Game hours a rising's provocation lasts; at least one day (shorter is refused), so the rising lasts until the next daily check, which settles it |
| `garrison.crushed_loyalty` | Loyalty (0..1) every resident loses when a rising is put down |

Revolts (GP-06) run once a day, settlements in SpawnId order. An owned settlement whose mean loyalty has been below villagers.ron `loyalty.revolt_threshold` for a day goes independent at once if it has no garrison. If the garrison is smaller than the residents able to fight (adult, alive, not starving, not already fighting, with morale at least villagers.ron `ai.fight.break_morale`), they rise: the settlement is provoked against its owner for `revolt_hours`. A day later the rising wins (independence) if no garrison is left or the rebels still standing outnumber it; otherwise it is put down.

## progression.ron (`sim_military::Progression`)
Runs: GP-21, GP-27 and SIM-34 (M4, see [M4_PLAN.md](../M4_PLAN.md)). The start, bands, traits, boons and modifiers are read only in a run (`RunState.config` is `Some`); the developed world and the M2/M3 tests never draw from them. `goodwill_deeds` and `forage` apply wherever the deeds and the `Forage` order happen, in every preset (goodwill feeds loyalty only in owned settlements, so the developed world behaves as before). Guarded by the `sim_military` progression unit tests (`shipped_data_is_valid`, `a_run_start_is_seeded_and_fits`, `a_run_is_set_up_its_boon_picked_once_and_abandoned`), by `run_start` ([tests/tests/run_start.rs](../../tests/tests/run_start.rs); `twenty_seeds_draw_valid_varied_starts` checks 2-3 nearby monuments and start variety over 20 seeds, so re-run it after changing `start.*`, `bands`, `modifiers` or their weights), and by the run tests in `determinism` (`run_state_hash_covers_every_sim_component`, `a_run_replays`). Cards are numbered by their position: each `id` must equal its index, and lists are append only, because offers, `RunProgress` and saves store the index.

**The start spot** (`sim_military::draw_start`). The rival's seat is the most populous settlement. The other regions are tried from the farthest (by settlement centroid) to the nearest; the band starts in the first one with a tile that has `min_monuments..=max_monuments` monuments nearby (else in the farthest). Tiles are ranked by class (monument count fits, then on the far side of the region from the seat, then of the drawn kind, then near the region's edge); in the best class the tile farthest from the seat plus a seeded jitter wins.

| Field | Meaning |
| --- | --- |
| `start.monument_radius` | Monuments within this many tiles (straight line) of the start count as nearby. A day's march is about 312 tiles |
| `start.min_monuments`, `start.max_monuments` | The start should have this many monuments nearby (2-3) |
| `start.min_settlement_distance` | A start tile is at least this many tiles from every settlement centre |
| `start.edge_distance` | Tiles within this distance of the start region's edge (coast or border) are preferred |
| `start.jitter` | Seeded spread (tiles, 0..jitter) added to a tile's distance from the seat when picking the start |
| `start.terrain_weights` | `(kind, weight)`: the kind of start drawn, `Coast` (beach or within 2 tiles of water), `ForestEdge` (forest beside open ground) or `Hills`. If no good tile has the drawn kind, the spot keeps its own kind (`Open` if none) |
| `bands[].name` | A band template (GP-27); one is drawn per run |
| `bands[].groups[].count` | Followers in this group (the champion not counted; about ten in all) |
| `bands[].groups[].unit` (`health`, `attack`, `defense`, `morale`) | The group's fighting stats, as army.ron's `soldier`. The champion has army.ron's `captain` stats. Morale still recovers towards army.ron's `soldier.morale` |
| `bands[].groups[].armed`, `bands[].groups[].tools` | The group starts with weapons, tools |
| `bands[].food_days` | Days of food in the baggage for the whole band |
| `champion_traits` | Trait ids the champion's starting trait is drawn from (SIM-34) |
| `traits[].id`, `traits[].name`, `traits[].phrase` | A trait: its index, display name and the chronicle's phrase ("Brand, who never gave ground") |
| `traits[].effects` | `(stat, value)` pairs: `sim_core::Stat` changes while the holder lives, as fractions (0.05 is +5%). Folded into `FactionModifiers` from M4 step 4 |
| `boons[].id`, `boons[].name`, `boons[].text` | A boon card: its index, display name and card text |
| `boons[].when` | `Start` (the starting boon, GP-27) or `Dominion` (holding 25% or 50% of a region, GP-24, M4 step 5) |
| `boons[].weight` | Relative chance to be offered. Three distinct cards are drawn by weight, without repeats |
| `boons[].effects` | What the boon does: `FoodDays(d)` adds d days of food for the whole army to the champion's baggage; `ArmBand` gives the champion and every follower a weapon; `Goodwill(g)` changes the goodwill of the populated settlement nearest the champion by g (through the goodwill ledger, so it also spreads; villagers.ron `goodwill`); `Stat(stat, v)` is a modifier for the picker's faction (from M4 step 4). Effects apply when the card is picked |
| `modifiers_per_run` | `(min, max)` run modifiers drawn per run (GP-27) |
| `modifiers[].id`, `modifiers[].name`, `modifiers[].text` | A run modifier card: its index, display name and text |
| `modifiers[].group` | Cards sharing a group (e.g. "harvest") never come in the same run; `None` for no group |
| `modifiers[].weight` | Relative chance to be drawn (without repeats) |
| `modifiers[].effects` | `Stores(f)` removes fraction f of the food in every storehouse at the start; `Stat(stat, v)` applies to everyone (the world row of `FactionModifiers`, from M4 step 4); `RaiderPace(x)` (raiders come x times as often) and `RivalStrength(x)` (the rival's levies and defence scale by x) take effect in M4 step 7; `Sellswords(n)` (a band for hire near the start) in a later step |
| `goodwill_deeds` | Goodwill change (-1..1) at a settlement, toward the doer's faction, for each deed done there (GP-22). Deeds post to `sim_agents::GoodwillLedger`; villagers.ron `goodwill` spreads a fraction to nearby settlements. Checked by sim_military's `deeds_move_goodwill_and_word_spreads` |
| `goodwill_deeds.trade` | Food bought with goods: a `Trade` order or a passive army's gathering |
| `goodwill_deeds.equip` | Arms bought with food (a passive `Equip`) |
| `goodwill_deeds.seize` | A hungry neutral army takes food from the reserve |
| `goodwill_deeds.plunder` | An aggressive army plunders food (it also provokes) |
| `goodwill_deeds.provoke` | The settlement is provoked into arms by the faction (once per provocation, not per tick) |
| `goodwill_deeds.killing` | Each of the settlement's militia killed, toward the faction they fought |
| `goodwill_deeds.request` | A request of the settlement answered (M4 step 3b) |
| `forage` | Wild food (SIM-38, `sim_military::wild`). The map is cut into 16-tile patches (`WildFood`); a `Forage { radius }` order makes the band stand where it is and gather from the patches with any tile within the radius, in patch order, until its baggage holds army.ron `gather_target_days` of food or no wild food is left there. The food is the least nourishing food good (grain). Checked by `patches_deplete_and_regrow` (wild.rs) and `a_foraging_band_lives_off_the_land` (orders.rs) |
| `forage.per_tile` | Food value a full patch holds per grass or forest tile (a 16 x 16 patch of grass: 2.56) |
| `forage.regrow_per_day` | Fraction (0..1) of a patch's capacity that grows back each day (the progress driver, once the patches exist: built at run setup, elsewhere by the first `Forage` order) |
| `forage.every_ticks` | Ticks between a foraging band's harvests (100 = a game hour) |
| `forage.per_member` | Food value each member (the captain too) gathers per harvest, times the faction's `ForageYield` modifier; units are rounded stochastically. 0.05 per hour is about 1.3 days of food per head per day of foraging |
| `forage.max_radius` | The largest `Forage` radius; a radius of 0 or above this is refused ("bad forage radius") |
| `forage.default_radius` | The radius the game's Forage button and F key use |

## render.ron (`game_render::RenderSettings`)
RND-03 and RND-06 ([ADR 0009](../adr/0009-art-direction-and-detail-bands.md)). Guarded by `game_render` band tests.

| Field | Meaning |
| --- | --- |
| `detail_distance` | Within this camera distance, textured figures and buildings |
| `billboard_distance` | Beyond this, camera-facing silhouettes (between the two: flat low-poly) |
| `dot_distance` | Beyond this, dots |
| `hysteresis` | Switch out at (1 + h) x a threshold and back in at (1 - h) x, so nothing flickers |
| `terrain_texture_tiles` | Tiles per repeat of the terrain detail texture |
