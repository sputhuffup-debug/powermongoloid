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
| `recipes[].modifier` | Optional (default `None`): the faction modifier that applies to the recipe where villagers work it (GP-23, [ADR 0015](../adr/0015-monuments-and-faction-modifiers.md)), read for the settlement's owner (`sim_core::FactionModifiers`). `Some(FoodYield)` (farming, fishing) scales the outputs by the multiplier, rounding the fraction up with that chance (stream `sim_agents::yield ^ villager` at the tick, drawn only when there is a fraction); a bonus unit goes into the worker's hands even past `carry`. `Some(SmithSpeed)` (toolmaking, weaponsmithing) divides `duration` by the multiplier (rounded, at least 1 tick). Any other stat is refused. Without modifiers (the developed world) both are exact no-ops |
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
GP-01..04 and SIM-30..32 ([ADR 0010](../adr/0010-armies-orders-and-combat.md)), conquest, garrisons and revolts (GP-05, GP-06; [ADR 0014](../adr/0014-ownership-conquest-and-loyalty.md)), and veteran ranks (SIM-36). Guarded by `army_march`, `posture`, `starving_army` and `orders`; `conquest`, `garrison` and `recruit.loyalty_bonus` by `rejects_bad_conquest_and_garrison_data` (in `crates/sim_military/src/data.rs`), the sim_military conquest unit tests, `orders`' `attacking_a_village_raises_its_militia` and the `conquest` scenario (`tests/tests/conquest.rs`). `Army::from_ron` refuses negative or non-finite values. An hour is a game hour, a 24th of a day (100 ticks).

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
| `give_keep_days` | A `Give` order (GP-22) hands the settlement's storehouse the food the baggage holds beyond this many days of the army's need (most nourishing first, as far as the storehouse has room). What actually arrives is counted in `FactionStats.donated`. A gift pauses the army's automatic gathering from villages for a quarter day: at 1 day (a neutral army's trigger) it would otherwise seize the gift straight back. Checked by sim_military's `giving_hands_over_food_beyond_the_keep` |
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
| `veterans` | Veteran ranks (SIM-36, M4). A battle is a fighting episode of one army: it starts with the first blow a member strikes or takes and ends at the first hourly step at least `quiet_hours` after the last one (`veterans::end_battles`), reporting a `Battle` event (the army's faction and captain, how many fought, its losses). Every living member who struck or was struck counts one more battle. In a run only, survivors rank up; outside one, ranks stay 0, so the M3 numbers never move. Guarded by `champion` ([tests/tests/champion.rs](../../tests/tests/champion.rs)), the veterans.rs unit tests and `rejects_bad_conquest_and_garrison_data` |
| `veterans.quiet_hours` | Game hours without a blow that end an army's battle (more than 0) |
| `veterans.thresholds` | Battles for rank 1, 2, ... (strictly ascending from 1; shipped 1, 3, 6). A faction's `VeteranRate` modifier (the "Hardened" boon) counts battles that much faster: the rank is the thresholds that `battles x (1 + VeteranRate)` reaches, and a rank is never lost |
| `veterans.names` | Rank names from rank 0 (Recruit, Soldier, Veteran, Elite): one more than `thresholds` |
| `veterans.gains` (`attack`, `defense`, `morale`, `desertion`) | What each rank from 1 up gives in all (not on top of the rank below), one per threshold: `attack` and `defense` points added where blows land (`resolve_strikes`), `morale` added to the morale the soldier recovers to, and `desertion` the share (0..1) taken off its chance to desert (`soldier_upkeep`). Rank 0 adds nothing |

Revolts (GP-06) run once a day, settlements in SpawnId order. An owned settlement whose mean loyalty has been below villagers.ron `loyalty.revolt_threshold` for a day goes independent at once if it has no garrison. If the garrison is smaller than the residents able to fight (adult, alive, not starving, not already fighting, with morale at least villagers.ron `ai.fight.break_morale`), they rise: the settlement is provoked against its owner for `revolt_hours`. A day later the rising wins (independence) if no garrison is left or the rebels still standing outnumber it; otherwise it is put down.

## progression.ron (`sim_military::Progression`)
Runs: GP-21, GP-27 and SIM-34 (M4, see [M4_PLAN.md](../M4_PLAN.md)). The start, bands, traits, boons and modifiers are read only in a run (`RunState.config` is `Some`); the developed world and the M2/M3 tests never draw from them. `goodwill_deeds` and `forage` apply wherever the deeds and the `Forage` order happen, in every preset; `requests`, `invitations` and `drifters` are run-only (the progress driver checks `RunState`), and so is `monuments` (monuments exist only on the run island; its bonuses reach the developed world never, since every multiplier is then exactly 1.0) (goodwill feeds loyalty only in owned settlements, so the developed world behaves as before). Guarded by the `sim_military` progression unit tests (`shipped_data_is_valid`, `a_run_start_is_seeded_and_fits`, `a_run_is_set_up_its_boon_picked_once_and_abandoned`), by `run_start` ([tests/tests/run_start.rs](../../tests/tests/run_start.rs); `twenty_seeds_draw_valid_varied_starts` checks 2-3 nearby monuments and start variety over 20 seeds, so re-run it after changing `start.*`, `bands`, `modifiers` or their weights), by the run tests in `determinism` (`run_state_hash_covers_every_sim_component`, `a_run_replays`), and `goodwill_deeds`, `forage`, `requests`, `invitations` and `drifters` by the band-life scenarios `wanderers` ([tests/tests/wanderers.rs](../../tests/tests/wanderers.rs): foraging and an answered request keep a band of ten fed and growing to day 5) and `invitation` ([tests/tests/invitation.rs](../../tests/tests/invitation.rs)) and the sim_military unit tests in `requests.rs`, `invitations.rs`, `drifters.rs` and `wild.rs`; `monuments` by `caps_and_set_bonus`, `shipped_monument_data_fits_the_island` and the monuments.rs unit tests; `dominion` (run-only too) and the Dominion boons' `favour` by `dominion` ([tests/tests/dominion.rs](../../tests/tests/dominion.rs)) and the dominion.rs unit tests; `champion`, `champion_traits` and `traits` by `champion` ([tests/tests/champion.rs](../../tests/tests/champion.rs)) and the champion.rs unit tests; `rival` (run-only) by `rival` ([tests/tests/rival.rs](../../tests/tests/rival.rs)) and the rival.rs unit tests; `hire` by `sellswords` ([tests/tests/sellswords.rs](../../tests/tests/sellswords.rs)) and the sellswords.rs unit tests. Cards are numbered by their position: each `id` must equal its index, and lists are append only, because offers, `RunProgress` and saves store the index.

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
| `champion_traits` | The champion's trait pool (SIM-34, SIM-35): trait ids its starting trait is drawn from, and from which each rank offers three distinct traits (ones it doesn't hold first, topped up with held ones; a held trait picked again stacks). At least three distinct ids |
| `traits[].id`, `traits[].name`, `traits[].phrase` | A trait: its index, display name and the chronicle's phrase ("Brand, who never gave ground") |
| `traits[].effects` | `(stat, value)` pairs: `sim_core::Stat` changes while the holder lives, as fractions (0.05 is +5%). A champion's traits are folded into its faction's `FactionModifiers` at the hourly rebuild (`rebuild_modifiers`), so a picked trait counts from the next game hour. Use only stats something reads: `Morale`, `Attack`, `Defence`, `MarchSpeed`, `ForageYield`, `LoyaltyDrift`, `TradeValue`, `FoodYield`, `SmithSpeed`, `ThreatRadius`, `CaptainSlots`, `VeteranRate` (army.ron `veterans.thresholds`), `Desertion` (scales soldiers' chance to desert) and `RecruitChance` (scales the chance each villager asked joins a `Recruit`) |
| `boons[].id`, `boons[].name`, `boons[].text` | A boon card: its index, display name and card text |
| `boons[].when` | `Start` (the starting boon, GP-27) or `Dominion` (a region's share reaching a `dominion.thresholds` entry, GP-24). There must be at least three of each (three Dominion boons only while `dominion.thresholds` is not empty) |
| `boons[].weight` | Relative chance to be offered. Three distinct cards are drawn by weight, without repeats. A Dominion boon's weight is first multiplied by its `favour` |
| `boons[].favour` | Optional (Start boons leave it out): `(holding, w)` pairs, each w at least 0. A Dominion offer for a region weighs the boon `weight x (1 + sum(w x amount))`, so the three on offer reflect what the player holds (GP-24). Holdings and their amounts: `Farms` (farms in the region's settlements the player owns, a count), `Smithy` (1 if one of them has a smithy), `LargestSettlement` (1 if the player owns the region's most populous settlement, ties to the lowest SpawnId), `Battles` (`Battle` chronicle entries of the player's faction; M4 step 6 writes them), `Kills` (enemies the player's soldiers have killed, `FactionStats.kills`). Checked by `holdings_weigh_the_dominion_draw` (dominion.rs) |
| `boons[].effects` | What the boon does: `FoodDays(d)` adds d days of food for the whole army to the champion's baggage; `ArmBand` gives the champion and every follower a weapon; `Goodwill(g)` changes the goodwill of the populated settlement nearest the champion by g (through the goodwill ledger, so it also spreads; villagers.ron `goodwill`); `Stat(stat, v)` is a modifier for the picker's faction, folded into `FactionModifiers` at the hourly rebuild (the start boon picked on tick 0 counts from tick 0; a `VeteranRate` modifier waits there for M4 step 6's veterans); `CaptainSlots(n)` gives the player n more captain slots from the pick on (see `dominion`). Effects apply when the card is picked; boons are kept for the run, even if the land that earned them is lost |
| `modifiers_per_run` | `(min, max)` run modifiers drawn per run (GP-27) |
| `modifiers[].id`, `modifiers[].name`, `modifiers[].text` | A run modifier card: its index, display name and text |
| `modifiers[].group` | Cards sharing a group (e.g. "harvest") never come in the same run; `None` for no group |
| `modifiers[].weight` | Relative chance to be drawn (without repeats) |
| `modifiers[].effects` | `Stores(f)` removes fraction f of the food in every storehouse at the start; `Stat(stat, v)` applies to everyone (the world row of `FactionModifiers`, rebuilt hourly); `RaiderPace(x)` divides the pacing director's `spacing_hours` by x (director.ron: low-tension raider bands come x times as often); `RivalStrength(x)` scales the rival's starting garrison, its levies (both rounded) and the seat's defence bonus by x (`rival`; several multiply; both factors finite and > 0); `Sellswords(n)` camps n armed men (captain included) near the start as an army of the independents, at peace with all, and offers them to the player for hire (`hire`; n >= 1, run modifiers only: a boon may not carry it) |
| `goodwill_deeds` | Goodwill change (-1..1) at a settlement, toward the doer's faction, for each deed done there (GP-22). Deeds post to `sim_agents::GoodwillLedger`; villagers.ron `goodwill` spreads a fraction to nearby settlements. Checked by sim_military's `deeds_move_goodwill_and_word_spreads` |
| `goodwill_deeds.trade` | Food bought with goods: a `Trade` order or a passive army's gathering |
| `goodwill_deeds.equip` | Arms bought with food (a passive `Equip`) |
| `goodwill_deeds.seize` | A hungry neutral army takes food from the reserve |
| `goodwill_deeds.plunder` | An aggressive army plunders food (it also provokes) |
| `goodwill_deeds.provoke` | The settlement is provoked into arms by the faction (once per provocation, not per tick) |
| `goodwill_deeds.killing` | Each of the settlement's militia killed, toward the faction they fought |
| `goodwill_deeds.request` | A request of the settlement answered (`requests`): toward the player, at the settlement that asked |
| `forage` | Wild food (SIM-38, `sim_military::wild`). The map is cut into 16-tile patches (`WildFood`); a `Forage { radius }` order makes the band stand where it is and gather from the patches with any tile within the radius, in patch order, until its baggage holds army.ron `gather_target_days` of food or no wild food is left there. The food is the least nourishing food good (grain). Checked by `patches_deplete_and_regrow` (wild.rs) and `a_foraging_band_lives_off_the_land` (orders.rs) |
| `forage.per_tile` | Food value a full patch holds per grass or forest tile (a 16 x 16 patch of grass: 2.56) |
| `forage.regrow_per_day` | Fraction (0..1) of a patch's capacity that grows back each day (the progress driver, once the patches exist: built at run setup, elsewhere by the first `Forage` order) |
| `forage.every_ticks` | Ticks between a foraging band's harvests (100 = a game hour) |
| `forage.per_member` | Food value each member (the captain too) gathers per harvest, times the faction's `ForageYield` modifier; units are rounded stochastically. 0.05 per hour is about 1.3 days of food per head per day of foraging |
| `forage.max_radius` | The largest `Forage` radius; a radius of 0 or above this is refused ("bad forage radius") |
| `forage.default_radius` | The radius the game's Forage button and F key use |
| `requests` | Village requests (GP-22, `sim_military::requests`), addressed to the player and run-only. Once a game hour (100 ticks), populated settlements in SpawnId order that are independent or the player's (never another warlord's, never the rival's seat), with no open request and no cooldown, may post one: **drive off** when an army at war with the player (raiders, the rival, beasts) and not the owner's stands within army.ron `threat.radius` of the centre, else **bring food** after `low_samples` hourly samples in a row below `food_days`. The sim detects completion; the reward comes from the real storehouse and residents and goes to the nearest player captain within `reward_radius`, and goodwill rises by `goodwill_deeds.request`. Checked by `a_hungry_village_asks_for_food_and_rewards_the_giver`, `raiders_near_a_village_ask_to_be_driven_off` and `an_unanswered_request_lapses_and_the_village_waits` (requests.rs) |
| `requests.food_days` | A settlement asks for food when its food security (`FoodSecurity.days`: food in the storehouse **and** homes over the daily need, the measure loyalty reads, not `spare_food`'s storehouse only) is below this |
| `requests.low_samples` | ... at this many hourly samples in a row (hysteresis: food security dips through the work day, so one low sample doesn't flap a request). At least 1 |
| `requests.need_days` | A bring-food request asks for this many days of the settlement's daily need (people x villagers.ron `needs.hunger_per_day`), in food value ... |
| `requests.max_need` | ... but at most this much, so a band can answer a town. Food delivered by the player's `Give` orders counts; the request is done at the first hourly sample with the need met |
| `requests.deadline_hours` | Game hours a request stays open; unanswered at the deadline, it fails (`RequestFailed`). Above 0 |
| `requests.cooldown_hours` | Game hours a settlement waits after a request ends (done or failed) before asking again |
| `requests.clear_hours` | Drive off: game hours with no hostile army within the threat radius before the danger counts as gone. Then the request is done if a player captain stands within `reward_radius`, and fails if none does (the danger passed without the player) |
| `requests.reward_radius` | The reward goes to the nearest player captain (ties: lowest SpawnId) within this many tiles of the centre; with none there, a bring-food request still counts and raises goodwill but pays nothing |
| `requests.reward_food_days` | Drive off: the settlement promises this many days of its own daily need in food, times a seeded 0.5..1 (stream `sim_military::requests ^ settlement` at the posting tick) ... |
| `requests.reward_max_food` | ... at most this much food value, and no more than its storehouse holds when posting. Paid (when done) as far as the storehouse and baggage room allow |
| `requests.reward_goods` | Every kind may promise up to this many units (seeded) of one non-food good the armies value (army.ron `trade_value`) that the storehouse holds; 0 promises none |
| `requests.volunteers` | `(min, max)` residents promised (seeded); on completion the first of recruiting's candidates (working-age civilians, idle first) join the rewarded captain, enlisted like recruits |
| `requests.camp_radius` | **Clear the camp** (GP-23), on its own track beside drive off and bring food (a settlement may have one of each open): each monument whose guard camp still has a captain and still holds it, in SpawnId order, is named by at most one open request, posted by the nearest settlement (ties: lowest SpawnId) within this many tiles that may ask (the rules above) and has no open clear-the-camp request and no clear-the-camp cooldown (`cooldown_hours`, kept apart from the other requests'). Done once the camp has no captain left, or has lost the monument (routed while someone held it), with a player captain within `reward_radius` of the monument (it gets the reward: food as for drive off, goods, volunteers; drawn from stream `sim_military::requests::camp ^ settlement`); failed if the camp went without one, or at the deadline. Monuments stand at least 52 tiles from every centre (worldgen.ron). Checked by `a_cleared_camp_answers_its_request_and_frees_the_monument` (monuments.rs unit tests) and `beating_a_guard_camp_takes_its_monument_and_clears_the_camp` (tests/tests/monuments.rs) |
| `invitations` | Invitations (GP-22, `sim_military::invitations`), run-only. Once a game hour, a populated settlement that isn't the player's or the rival's seat, with no invitation pending or cooling down, offers lordship to the player (an `Invitation` offer, choices accept/decline) when its goodwill toward the player is at least `goodwill`, a player captain stands within `radius` of its centre, and its owner is weak. Accepting (`ChooseOffer` choice 0) hands it over with residents' loyalty seeded from goodwill (`Invited` event). Checked by `a_village_that_likes_the_band_invites_it_in` (invitations.rs) |
| `invitations.goodwill` | Goodwill toward the player (-1..1) at or above which a settlement may invite it in |
| `invitations.radius` | A player captain must stand within this many tiles of the centre; the player's soldiers within it are also weighed against an owner's garrison |
| `invitations.weak_loyalty` | An owned settlement's owner is weak when its residents' mean loyalty is below this (0..1) and its garrison (the owner's soldiers within army.ron `garrison.radius`) is smaller than the player's soldiers within `radius`. An independent settlement always counts as weak |
| `invitations.expiry_hours` | Game hours the offer stands (`Offer.expires`); it can be answered up to and including that tick, then lapses at the hourly step. Above 0 |
| `invitations.cooldown_days` | Days a settlement waits after its invitation is accepted, declined or lapses before offering again (`RunProgress.invite_cooldowns`) |
| `drifters` | Drifters (GP-22, `sim_military::drifters`), run-only. At each day tick, for each player captain in SpawnId order, every independent or player-owned settlement within `radius` whose goodwill toward the player is at least `goodwill` sends a seeded number of volunteers (stream `sim_military::drifters ^ captain` at the day tick): real villagers, recruiting's candidates (working-age civilians, idle first), enlisted with `enlist_into` like recruits (`DriftersJoined` event). Checked by `a_village_that_likes_the_band_sends_it_drifters` (drifters.rs) |
| `drifters.radius` | Settlements whose centre is within this many tiles of the captain |
| `drifters.goodwill` | Goodwill toward the player (-1..1) a settlement needs to send drifters |
| `drifters.per_settlement` | `(min, max)`: the seeded count each such settlement sends per day (fewer if it has fewer candidates) |
| `drifters.max_per_day` | At most this many drifters join one captain per day |
| `hire` | A band for hire (GP-27, the `Sellswords(n)` run modifier, `sim_military::sellswords`), run-only. Optional: progression text without it (a save made before it existed) reads the shipped values. At the very end of run setup, after every other spawn, each drawn modifier with `Sellswords` effects camps its n men (captain included) `distance` tiles from the start, in a direction drawn from stream `sim_military::sellswords ^ card` (tick 0), snapped to the nearest tile the champion can walk to: an ordinary army of `FactionId::INDEPENDENT` (at peace with everyone), passive, without orders, every man carrying `NoUpkeep` (never hungry, never eats, never gathers). A `Hire { captain, men, price }` offer goes to the player, choices hire/decline. Hiring (choice 0) needs the champion within `radius` of the band's captain and the price in food in its baggage, else it is refused and the offer stands; the men then join the champion's army as followers (no captain slot used), eat from its baggage and grow it by army.ron `baggage_per_head` each. Declined, or lapsed after `expiry_hours`, the band leaves the world. Guarded by `sellswords` ([tests/tests/sellswords.rs](../../tests/tests/sellswords.rs)), the sellswords.rs unit tests, `hire_defaults_match_the_shipped_data` and `rejects_bad_data` |
| `hire.distance` | Tiles from the start to the band's camp (at least 0; the camp snaps to the nearest reachable tile, so on a coast it may be nearer) |
| `hire.unit` (`health`, `attack`, `defense`, `morale`) | Every sellsword's fighting stats, the captain's included (health above 0). Morale still recovers towards army.ron's `soldier.morale` |
| `hire.armed` | The sellswords carry weapons |
| `hire.price_days` | The hire's price, paid from the champion's baggage (least nourishing food first): this many days of villagers.ron `needs.hunger_per_day` per sellsword, rounded up (12 men x 0.9 x 1 = 10.8, so 11 food value). At least 0 |
| `hire.radius` | The champion must stand within this many tiles of the band's captain to hire it. Above `distance` |
| `hire.expiry_hours` | Game hours the Hire offer stands (`Offer.expires`, answerable before that tick); it lapses at the hourly step (`invitations::step`'s expiry pass) and the band moves on. Above 0 |
| `monuments` | Monuments (GP-23, `sim_military::monuments`, [ADR 0015](../adr/0015-monuments-and-faction-modifiers.md)), run-only (monuments exist only on the run island). At run setup every monument gets a `Holder` and draws its guard. Once a game hour (100 ticks), monuments in SpawnId order: soldiers (not militia, not routed) within `hold_radius` are counted by faction; one faction alone there for `hold_hours` takes the monument (`MonumentTaken`, `MonumentLost`); a holder that has had no soldiers there for `drift_days` and owns no settlement within `drift_distance` loses it to nobody. Then `FactionModifiers` is rebuilt from held monuments (`kinds`), the player's boons, champions' traits and run modifiers. Checked by `caps_and_set_bonus` and `shipped_monument_data_fits_the_island` (progression) and the monuments.rs unit tests |
| `monuments.hold_radius` | Soldiers within this many tiles of a monument stand at it (and count toward its holder's presence). Above 0. A guard camp's men stand within it, so a guarded monument can only be taken once its guard is beaten |
| `monuments.hold_hours` | Game hours (100 ticks each) one faction's soldiers must be the only ones at a monument to take it; sampled hourly, so the transfer comes at the first sample at least this long after the first sample that saw them alone. Any other faction's soldiers there reset it |
| `monuments.drift_days` | A held monument falls back to nobody after this many days without its holder's soldiers at it ... |
| `monuments.drift_distance` | ... unless its holder owns a settlement within this many tiles of it (a held monument near your lands stays yours) |
| `monuments.set_min` | A set bonus needs at least this many monuments of the kind in the region (so a region's lone monument of a kind is no set). At least 1 |
| `monuments.kinds` | One entry per monument kind (`kind`, worldgen.ron's `MonumentKind`), each listed at most once; a kind not listed gives nothing |
| `monuments.kinds[].stat` | The `sim_core::Stat` it raises: `Morale` (soldiers' morale target), `Defence` (damage taken is divided by the multiplier), `FoodYield` (economy.ron recipes tagged `FoodYield`, in the holder's settlements), `MarchSpeed` (captains and their followers), `SmithSpeed` (recipes tagged `SmithSpeed`), `TradeValue` (what a barter's goods fetch), `LoyaltyDrift` (villagers.ron `loyalty.drift_per_day`), `ThreatRadius` (flat tiles added to army.ron `threat.radius` for the holder's settlements) |
| `monuments.kinds[].bonus`, `monuments.kinds[].cap` | Per faction: `min(monuments of the kind held x bonus, cap)` (fractions, 0.01 is +1%; flat tiles for `ThreatRadius`). Both at least 0 |
| `monuments.kinds[].set_bonus` | Added beyond the cap for each region in which one faction holds every monument of the kind (at least `set_min` of them); 0 for none. Keep `threat.radius` + the watchtower `cap` + `set_bonus` x 4 regions below worldgen.ron's run `monuments.min_settlement_distance` (52), or a guard camp frightens a village; `shipped_monument_data_fits_the_island` checks it |
| `monuments.guards` | Who may guard a monument, drawn once per monument at setup (stream `sim_military::monuments::guards ^ monument`, tick 0): the entries' `chance`s laid end to end, past their sum none. A guard is an army spawned at the monument (`spawn_army`, posture neutral, no food) whose members all carry `Guard` and `NoUpkeep`: they never get hungry or eat, gather nothing, aren't hunted by raiders, and hold their tile, fighting anything at war within army.ron `engage_radius`. The guard faction holds the monument |
| `monuments.guards[].kind` | `RaiderCamp` (`FactionId::RAIDERS`, without the raider AI) or `Beasts` (`FactionId::BEASTS`) |
| `monuments.guards[].chance` | Chance (0..1) a monument gets this guard; all chances add up to at most 1 |
| `monuments.guards[].base`, `monuments.guards[].per_tile`, `monuments.guards[].max` | Followers (besides the captain): `base + per_tile x tiles from the start spot`, rounded, at most `max` (`base` <= `max`): camps far from the start are stronger |
| `monuments.guards[].armed` | The guards carry weapons |
| `dominion` | Dominion boons (GP-24) and captain slots (GP-25), `sim_military::progression::dominion`, run-only. Once a game hour (100 ticks; slot 8 of the progress driver) each region's people (`SettlementEconomy.population` of its settlements) and the part of them in settlements the player owns are tallied into `RunProgress.dominion`. The first time the player's share of a region reaches a `thresholds` entry, a `DominionBoon` offer of three distinct Dominion boons is posted (drawn by `boons[].weight` and `favour`, stream `sim_military::progression::boons ^ offer id` at the posting tick). Each threshold offers once per region: its flag is never cleared, so losing land and taking it back offers nothing new. Guarded by `dominion` ([tests/tests/dominion.rs](../../tests/tests/dominion.rs)) and the dominion.rs unit tests |
| `dominion.thresholds` | Shares (fractions of a region's people) that each offer one boon: ascending, each in (0, 1], at most 8. Shipped 25% and 50% |
| `dominion.per_captain` | One captain slot per this many people in the settlements the player holds (anywhere on the island), rounded down. Above 0 |
| `dominion.cap` | At most this many captain slots, whatever the land and boons (8, GP-01). Keep it below army.ron `max_captains` (9, which counts the champion and stays the hard limit) |
| `dominion.first_village_slots` | Slots that come with the first settlement held, until `per_captain` gives more: the first lieutenant comes with the first village. At most `cap`. The player's slots are `min(cap, max(first_village_slots if a settlement is held, floor(held / per_captain)) + CaptainSlots boons + the flat CaptainSlots modifier)`, and `AppointCaptain` is refused ("no captain slot") once the player's captains other than the champion fill them. Outside a run, and for other factions, only `max_captains` applies |
| `champion` | The champion's ranks (SIM-35, run only). The champion's `ChampionRank` earns points for deeds; at each hourly step (`champion::rank_ups`) every rank its points reach posts an offer of three traits from `champion_traits` (`Offer { Trait { person } }`, drawn from `sim_military::progression::traits ^ person` at that tick) and a `RankUp` event. Guarded by `champion` ([tests/tests/champion.rs](../../tests/tests/champion.rs)) and the champion.rs unit tests |
| `champion.points` (`battle`, `monument`, `settlement`, `request`) | Points per deed: a battle the champion's own army lived through (army.ron `veterans`), a monument the faction took, a settlement it conquered or was invited into, a village request it answered |
| `champion.ranks` | Total points for rank 1, 2, ... (strictly ascending from 1). Empty: no ranks, no trait offers |
| `rival` | The rival warlord's clock (GP-28, `sim_military::rival`, run only): one row per `difficulty` (`Steady`, `Chaotic`, `Harsh`, exactly one each), the run's `RunConfig.difficulty` picks it. At run setup the rival gets every populated settlement of its seat's region (quietly: no `Conquest` is reported) and a garrison at the seat with a standing `Garrison` order; then once a game hour (`rival::clock`, slot 11 of the progress driver) while it holds its seat it sets the seat's defence bonus, levies and raids when due. The stronger and weaker rival run modifiers (`RivalStrength`) scale `garrison`, `levy_size` and the defence bonus. Days count from tick 0 (a day is 2,400 ticks); a game hour is 100 ticks. Guarded by `rival` ([tests/tests/rival.rs](../../tests/tests/rival.rs)), the rival.rs unit tests and `rejects_bad_data` |
| `rival[].difficulty` | The difficulty this row is for |
| `rival[].start_loyalty` | Loyalty (0..1) of the rival's people at the start; it then drifts like any owned settlement's (villagers.ron `loyalty`) |
| `rival[].garrison` | Armed followers of the seat's garrison at the start (rounded after `RivalStrength`) |
| `rival[].garrison_food_days` | Days of food in the garrison's baggage at the start (it then lives on the seat's spare food, army.ron `garrison`) |
| `rival[].seat_radius` | The seat's defence bonus covers the rival's people (soldiers and militia) within this many tiles of its centre |
| `rival[].levy_every_days` | Days between levies (the first on this day). A levy enlists real villagers of the rival's settlements into the garrison (`enlist_into`), one per settlement in SpawnId order round and round (idle people first), and reports `RivalLevy`; they walk to the seat. A garrison that was wiped out is raised again at the seat by the levy. Greater than 0 |
| `rival[].levy_size` | Villagers enlisted at each levy (rounded after `RivalStrength`) |
| `rival[].defence_steps` | `(day, bonus)` pairs, ascending days: from that day, blows on the rival's people near the seat are divided by their `Defence` multiplier plus `bonus` (x `RivalStrength`), so 0.5 takes half again as many blows. It stands in for walls until M5's sieges. Before the first step the bonus is 0 |
| `rival[].raid_from_day` | The first raid leaves on this day (about day 10, minute 40 at 1x, on steady) |
| `rival[].raid_every_days` | Days between raids after the first. Greater than 0 |
| `rival[].raid_size` | People in a raiding band, its captain included (at least 2), but at most half the garrison's followers: the band is split off the garrison (`orders::split_army`, its share of the baggage by head count), so raids weaken the seat. A garrison of fewer than four followers sends nobody. The band (`RivalBand`, aggressive) goes for the nearest of the player's settlements (`Attack`) and monuments (stands at it), else hunts the player's nearest army, else goes home; it reports `RivalRaid` |
| `chronicle` | How the chronicle words its entries (UI-09, [ADR 0017](../adr/0017-events-and-chronicle.md)): read only by `sim_military::chronicle::describe`, never by the simulation, so changing a phrase changes no outcome (only the hashed RON text). Guarded by `chronicle` ([tests/tests/chronicle.rs](../../tests/tests/chronicle.rs)), the chronicle.rs unit tests and `rejects_bad_data` |
| `chronicle.causes` | One phrase per `Cause`, added after the sentence and a comma ("Day 3: Your warband took Saltholt Stones, by holding the ground."). Each must be non-empty, and may use only the placeholders `{faction}` (the event's faction), `{settlement}` (the settlement concerned: the asking village for a request), `{captain}` (who led a conquest or raid), `{boon}` (the boon's name) and `{foe}` (the killer's faction, or every faction a battle fought) |
| `chronicle.causes.player` | The player chose it (a trait picked, the run abandoned) |
| `chronicle.causes.request` | A request answered (`RequestDone`) |
| `chronicle.causes.conquest` | A settlement taken by force (`Conquest`, the run won) |
| `chronicle.causes.invitation` | A settlement's invitation accepted (`Invited`) |
| `chronicle.causes.starvation` | Death by hunger (the champion's fall) |
| `chronicle.causes.plunder` | Plunder |
| `chronicle.causes.revolt` | A rising (`Revolt`) |
| `chronicle.causes.raid` | A raid: the rival's raiding bands and the director's raiders |
| `chronicle.causes.hold` | Holding the ground: a monument taken |
| `chronicle.causes.boon` | A boon's effect |
| `chronicle.causes.death` | Death in battle (the champion's fall, a succession, the run lost) |
| `chronicle.causes.lapsed` | A request nobody answered in time (`RequestFailed`) |
| `chronicle.causes.neglect` | A monument left unwatched until it drifted back to nobody (`MonumentLost`) |
| `chronicle.causes.fight` | A battle (`Battle`), against every faction it fought |

## director.ron (`sim_military::DirectorData`)
The pacing director (GP-16, `sim_military::director`, run only). Once a game hour (100 ticks; slot 12 of the progress driver) it measures the player's tension: `threat x weights.threat + hunger x weights.hunger + fights x weights.fights - quiet x weights.quiet`, where threat is 1 with an army at war with the player within army.ron `threat.radius` of one of the player's armies (else the highest `Threat` level of the player's settlements), hunger is 1 with no food in the player's baggage and 0 at two days or more, fights is 1 while one of the player's armies is in battle, and quiet is the time since the last notable event (`Chronicle::last_notable`: every chronicled event and a posted request) as a share of `quiet_limit_hours`, each 0..1. It brings a band of raiders (faction `RAIDERS`, a `Raider` with `target`, aggressive) when the next hourly check would pass `quiet_limit_hours` without a notable event, when no request, raid or fight has come by `first_event_within_hours`, or when tension is below 0, `spacing_hours` have passed since its last band and fewer than `max_bands` raider bands roam. The band appears `spawn_distance` tiles from the populated settlement nearest the champion that the player or nobody owns (the player's first; outside the rival's region; stream `sim_military::director` at the tick), raids it first (on arrival the village asks the player to drive it off), then hunts only the player's armies no bigger than itself and raids only the player's settlements. Each band reports `RaidersArrived`. The run's `RunConfig.difficulty` picks the personality. Guarded by `director` ([tests/tests/director.rs](../../tests/tests/director.rs)) and the director.rs unit tests.

| Field | Meaning |
| --- | --- |
| `personalities` | One per `difficulty` (`Steady`, `Chaotic`, `Harsh`), exactly one each |
| `personalities[].difficulty` | The difficulty that picks this personality |
| `personalities[].weights` (`threat`, `hunger`, `fights`, `quiet`) | Tension weights (>= 0), see above. Quiet is subtracted |
| `personalities[].quiet_limit_hours` | Never longer than this many game hours without a notable event: a band comes on the last hourly check before (at least 2) |
| `personalities[].first_event_within_hours` | The run's first event (a request, raiders, a rival raid, a fight: `director::eventful`) comes within this many hours; if nothing has come, a band arrives on the last hourly check before it (at least 1) |
| `personalities[].spacing_hours` | At least this many hours (divided by the restless roads' `RaiderPace`) between bands brought for low tension, and before the first one (> 0). Bands against quiet ignore it |
| `personalities[].raider_band` (`min`, `max`, `per_day`, `armed`) | A band's people, its captain included: `min` on day 0, `per_day` more per day of the run (rounded down), at most `max` (1 <= min <= max); `armed` gives every raider a weapon |
| `personalities[].max_bands` | Bands for low tension come only while fewer than this many raider bands roam (bands against quiet always come) |
| `personalities[].auto_pause` | `SimEventKind`s the game pauses on when one arrives (UI, M4 step 10) |
| `spawn_distance` | A band appears between these many tiles (min, max) from its target village (or the champion), on the nearest passable tile in a region other than the rival's; 24 seeded tries, else no band that hour |
| `band_food_days` | Days of food in a band's baggage |

## render.ron (`game_render::RenderSettings`)
RND-03 and RND-06 ([ADR 0009](../adr/0009-art-direction-and-detail-bands.md)). Guarded by `game_render` band tests.

| Field | Meaning |
| --- | --- |
| `detail_distance` | Within this camera distance, textured figures and buildings |
| `billboard_distance` | Beyond this, camera-facing silhouettes (between the two: flat low-poly) |
| `dot_distance` | Beyond this, dots |
| `hysteresis` | Switch out at (1 + h) x a threshold and back in at (1 - h) x, so nothing flickers |
| `terrain_texture_tiles` | Tiles per repeat of the terrain detail texture |
