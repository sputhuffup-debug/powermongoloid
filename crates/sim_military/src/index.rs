//! Per-tick read-only summaries built serially at the start of each tick
//! (Knowledge set), so the parallel per-soldier systems never read each
//! other's components: one entry per army, and a grid of everyone who can
//! fight. Both are derived from hashed state, so neither is hashed.

use std::collections::HashMap;
use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use sim_agents::{ActionKind, Activity, CombatStats, Stance};
use sim_core::{
    Allegiance, FactionId, Motion, Position, Posture, SaveError, SaveReader, SaveState, SaveWriter,
    SpawnId,
};
use sim_economy::{Economy, Stockpile};
use sim_world::{FlowCache, FlowField, WorldMap, world_pos};

use crate::components::{Captain, Follower};
use crate::data::Army;

/// One army, as the soldiers see it this tick.
#[derive(Debug, Clone)]
pub struct ArmyInfo {
    pub entity: Entity,
    pub id: SpawnId,
    pub faction: FactionId,
    pub posture: Posture,
    pub pos: Vec2,
    pub goal: Option<IVec2>,
    /// Toward the goal (the captain walks it).
    pub field: Option<Arc<FlowField>>,
    /// Toward the captain (followers far from their place walk it).
    pub rally: Option<Arc<FlowField>>,
    pub followers: u32,
    pub fighting: u32,
    /// Followers well outside the formation.
    pub stragglers: u32,
    /// Food value in the baggage.
    pub food: f32,
}

/// Every army this tick, by SpawnId, with a lookup by captain entity.
#[derive(Resource, Debug, Default)]
pub struct ArmyIndex {
    pub armies: Vec<ArmyInfo>,
    by_entity: HashMap<Entity, usize>,
    /// Membership signature per army when its slots were last numbered.
    signatures: HashMap<Entity, (u32, u64, u64)>,
}

impl ArmyIndex {
    /// The army led by `captain`.
    pub fn get(&self, captain: Entity) -> Option<&ArmyInfo> {
        self.by_entity.get(&captain).map(|&i| &self.armies[i])
    }
}

/// Which side someone fights on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Side {
    pub faction: FactionId,
    /// Militia defending against this faction.
    pub against: Option<FactionId>,
}

impl Side {
    /// Whether these sides fight each other.
    pub fn hostile_to(self, other: Side, army: &Army) -> bool {
        match (self.against, other.against) {
            (None, None) => army.at_war(self.faction, other.faction),
            (Some(f), None) => other.faction == f,
            (None, Some(f)) => self.faction == f,
            (Some(_), Some(_)) => false,
        }
    }

    /// This side's bit in a cell's mask: soldiers by faction (bits 0..32),
    /// militia by the faction they fight (bits 32..64).
    fn bit(self) -> u64 {
        match self.against {
            None => 1 << (self.faction.0 as u64 & 31),
            Some(f) => 1 << (32 + (f.0 as u64 & 31)),
        }
    }

    /// Bits of every side this side could fight.
    fn hostile_mask(self, army: &Army) -> u64 {
        match self.against {
            None => {
                let mut m = 1u64 << (32 + (self.faction.0 as u64 & 31));
                for g in 0..32u16 {
                    if army.at_war(self.faction, FactionId(g)) {
                        m |= 1 << g;
                    }
                }
                m
            }
            Some(f) => 1 << (f.0 as u64 & 31),
        }
    }

    /// The side of someone who can fight; `None` for civilians.
    pub fn of(allegiance: &Allegiance, stats: &CombatStats) -> Option<Side> {
        match stats.stance {
            Stance::Civilian => None,
            Stance::Soldier => Some(Side {
                faction: allegiance.0,
                against: None,
            }),
            Stance::Militia { against } => Some(Side {
                faction: allegiance.0,
                against: Some(against),
            }),
        }
    }
}

/// One fighter in the grid.
#[derive(Debug, Clone, Copy)]
pub struct FighterEntry {
    pub id: SpawnId,
    pub entity: Entity,
    pub pos: Vec2,
    pub side: Side,
    pub routed: bool,
}

/// Everyone who can fight, bucketed in 8-tile cells, SpawnId order within
/// a cell.
#[derive(Resource, Debug, Default)]
pub struct FighterGrid {
    per_side: i32,
    cells: Vec<Vec<FighterEntry>>,
    /// Sides present in each cell (see `Side::bit`).
    masks: Vec<u64>,
    used: Vec<u32>,
    pub count: usize,
    /// Every fighter (not sorted: users sort what they pick out).
    pub fighters: Vec<(SpawnId, Entity)>,
    /// Fighting villagers (militia), in SpawnId order.
    pub militia: Vec<(SpawnId, Entity)>,
    /// Living fighters per tile, at the start of the tick (lookups only).
    occupancy: HashMap<IVec2, u16>,
}

const CELL: f32 = 8.0;

impl FighterGrid {
    fn cell(&self, p: Vec2) -> Option<usize> {
        let c = (p / CELL).floor().as_ivec2();
        (self.per_side > 0 && c.x >= 0 && c.y >= 0 && c.x < self.per_side && c.y < self.per_side)
            .then(|| (c.y * self.per_side + c.x) as usize)
    }

    /// How many living fighters stood on `tile` when the tick began.
    pub fn fighters_on(&self, tile: IVec2) -> u16 {
        self.occupancy.get(&tile).copied().unwrap_or(0)
    }

    /// Visits fighters within `radius` of `p` in a fixed order.
    pub fn for_each_within(&self, p: Vec2, radius: f32, mut f: impl FnMut(&FighterEntry, f32)) {
        if self.per_side == 0 {
            return;
        }
        let lo = ((p - radius) / CELL).floor().as_ivec2().max(IVec2::ZERO);
        let hi = ((p + radius) / CELL)
            .floor()
            .as_ivec2()
            .min(IVec2::splat(self.per_side - 1));
        let r2 = radius * radius;
        for cy in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                for e in &self.cells[(cy * self.per_side + cx) as usize] {
                    let d2 = e.pos.distance_squared(p);
                    if d2 <= r2 {
                        f(e, d2.sqrt());
                    }
                }
            }
        }
    }

    /// The nearest fighter hostile to `side` within `radius`, ties by
    /// SpawnId. Cells are visited nearest first; cells holding no possible
    /// foe, or none closer than the best found so far, are skipped, so in a
    /// packed melee only a cell or two is read. The result is the same as
    /// a full search ([`Self::nearest_hostile_slow`]).
    pub fn nearest_hostile(
        &self,
        me: Entity,
        p: Vec2,
        side: Side,
        radius: f32,
        army: &Army,
    ) -> Option<Foe> {
        if self.per_side == 0 {
            return None;
        }
        let hostile = side.hostile_mask(army);
        let lo = ((p - radius) / CELL).floor().as_ivec2().max(IVec2::ZERO);
        let hi = ((p + radius) / CELL)
            .floor()
            .as_ivec2()
            .min(IVec2::splat(self.per_side - 1));
        // (distance from p to the cell's square, cell index), nearest first.
        let mut cells: Vec<(f32, usize)> = Vec::with_capacity(9);
        for cy in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                let c = (cy * self.per_side + cx) as usize;
                if self.masks[c] & hostile == 0 {
                    continue;
                }
                let min = IVec2::new(cx, cy).as_vec2() * CELL;
                let gap = (min - p).max(p - (min + CELL)).max(Vec2::ZERO);
                cells.push((gap.length(), c));
            }
        }
        cells.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let r2 = radius * radius;
        let mut best: Option<Foe> = None;
        for (gap, c) in cells {
            // Everyone in this cell is at least `gap` away; the margin
            // keeps rounding from ever skipping a tie.
            if best.is_some_and(|b| gap > b.2 + 1e-3) || gap > radius + 1e-3 {
                break;
            }
            for e in &self.cells[c] {
                let d2 = e.pos.distance_squared(p);
                if d2 > r2 || e.entity == me || !side.hostile_to(e.side, army) {
                    continue;
                }
                let d = d2.sqrt();
                let better = match best {
                    None => true,
                    Some((_, _, bd, bid)) => d < bd || (d == bd && e.id < bid),
                };
                if better {
                    best = Some((e.entity, e.pos, d, e.id));
                }
            }
        }
        best
    }

    /// How many hostiles and friends are within `radius`. Cells holding no
    /// possible foe are skipped (so friends are counted only where there
    /// are foes). Reads every fighter in range: call it only when the
    /// numbers are needed (the decision trace), not every tick.
    pub fn count_nearby(
        &self,
        me: Entity,
        p: Vec2,
        side: Side,
        radius: f32,
        army: &Army,
    ) -> Nearby {
        let mut out = Nearby::default();
        if self.per_side == 0 {
            return out;
        }
        let hostile = side.hostile_mask(army);
        let lo = ((p - radius) / CELL).floor().as_ivec2().max(IVec2::ZERO);
        let hi = ((p + radius) / CELL)
            .floor()
            .as_ivec2()
            .min(IVec2::splat(self.per_side - 1));
        let r2 = radius * radius;
        for cy in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                let c = (cy * self.per_side + cx) as usize;
                if self.masks[c] & hostile == 0 {
                    continue;
                }
                for e in &self.cells[c] {
                    if e.entity == me || e.pos.distance_squared(p) > r2 {
                        continue;
                    }
                    if side.hostile_to(e.side, army) {
                        out.enemies += 1;
                    } else if e.side == side {
                        out.friends += 1;
                    }
                }
            }
        }
        out
    }

    /// The nearest hostile and the counts, visiting every cell in range
    /// (tests: the reference for the two searches above).
    pub fn nearest_hostile_slow(
        &self,
        me: Entity,
        p: Vec2,
        side: Side,
        radius: f32,
        army: &Army,
    ) -> (Option<Foe>, Nearby) {
        let mut nearest: Option<Foe> = None;
        let mut out = Nearby::default();
        self.for_each_within(p, radius, |e, d| {
            if e.entity == me {
                return;
            }
            if side.hostile_to(e.side, army) {
                out.enemies += 1;
                let better = match nearest {
                    None => true,
                    Some((_, _, bd, bid)) => d < bd || (d == bd && e.id < bid),
                };
                if better {
                    nearest = Some((e.entity, e.pos, d, e.id));
                }
            } else if e.side == side {
                out.friends += 1;
            }
        });
        (nearest, out)
    }
}

/// A foe found by [`FighterGrid::nearest_hostile`]: entity, position,
/// distance and SpawnId.
pub type Foe = (Entity, Vec2, f32, SpawnId);

/// How many fighters a fighter sees around it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Nearby {
    pub enemies: u32,
    pub friends: u32,
}

// GP-12: the index and the grid are saved, although they are exempt from
// the hash: the next tick's input step (raiders, the rival, an attack's
// hold) reads them before `build_index` refreshes them, so a resumed game
// must see exactly what the saved one would have.
sim_core::save_fields!(Side { faction, against });
sim_core::save_fields!(FighterEntry {
    id,
    entity,
    pos,
    side,
    routed
});

/// GP-12: writes the index's armies (their flow fields are not saved: the
/// next `build_index` looks them up again) and slot signatures, by SpawnId.
pub(crate) fn save_index(world: &World, w: &mut SaveWriter) {
    let index = world.resource::<ArmyIndex>();
    w.count(index.armies.len());
    for a in &index.armies {
        w.entity(a.entity);
        a.id.save(w);
        a.faction.save(w);
        w.serde(&a.posture);
        w.vec2(a.pos);
        a.goal.save(w);
        w.u32(a.followers);
        w.u32(a.fighting);
        w.u32(a.stragglers);
        w.f32(a.food);
    }
    let mut signatures: Vec<(SpawnId, Entity, (u32, u64, u64))> = index
        .signatures
        .iter()
        .filter_map(|(e, s)| world.get::<SpawnId>(*e).map(|id| (*id, *e, *s)))
        .collect();
    signatures.sort_unstable_by_key(|(id, ..)| *id);
    w.count(signatures.len());
    for (_, e, s) in signatures {
        w.entity(e);
        s.save(w);
    }
}

/// GP-12: restores what [`save_index`] wrote.
pub(crate) fn load_index(world: &mut World, r: &mut SaveReader) -> Result<(), SaveError> {
    let n = r.count()?;
    let mut armies = Vec::with_capacity(n);
    for _ in 0..n {
        armies.push(ArmyInfo {
            entity: r.entity()?,
            id: SpawnId::load(r)?,
            faction: FactionId::load(r)?,
            posture: r.serde()?,
            pos: r.vec2()?,
            goal: Option::<IVec2>::load(r)?,
            field: None,
            rally: None,
            followers: r.u32()?,
            fighting: r.u32()?,
            stragglers: r.u32()?,
            food: r.f32()?,
        });
    }
    let n = r.count()?;
    let mut signatures = HashMap::with_capacity(n);
    for _ in 0..n {
        let e = r.entity()?;
        signatures.insert(e, <(u32, u64, u64)>::load(r)?);
    }
    let by_entity = armies
        .iter()
        .enumerate()
        .map(|(i, a)| (a.entity, i))
        .collect();
    world.insert_resource(ArmyIndex {
        armies,
        by_entity,
        signatures,
    });
    Ok(())
}

/// GP-12: writes the grid's fighters, cell by cell in the order the cells
/// were filled, and its fighter and militia lists.
pub(crate) fn save_grid(world: &World, w: &mut SaveWriter) {
    let grid = world.resource::<FighterGrid>();
    w.i32(grid.per_side);
    w.save(&grid.count);
    w.save(&grid.used);
    for &c in &grid.used {
        w.save(&grid.cells[c as usize]);
    }
    w.save(&grid.fighters);
    w.save(&grid.militia);
}

/// GP-12: restores what [`save_grid`] wrote; the cells' side masks and the
/// per-tile counts are recomputed from the fighters.
pub(crate) fn load_grid(world: &mut World, r: &mut SaveReader) -> Result<(), SaveError> {
    let per_side = r.i32()?;
    if !(0..=1 << 14).contains(&per_side) {
        return Err(SaveError::Corrupt(format!("fighter grid of {per_side}")));
    }
    let n = (per_side * per_side) as usize;
    let count = usize::load(r)?;
    let used = Vec::<u32>::load(r)?;
    let mut cells = vec![Vec::new(); n];
    let mut masks = vec![0u64; n];
    let mut occupancy: HashMap<IVec2, u16> = HashMap::new();
    for &c in &used {
        let entries = Vec::<FighterEntry>::load(r)?;
        let slot = cells
            .get_mut(c as usize)
            .ok_or_else(|| SaveError::Corrupt(format!("fighter cell {c}")))?;
        for e in &entries {
            masks[c as usize] |= e.side.bit();
            *occupancy.entry(e.pos.floor().as_ivec2()).or_default() += 1;
        }
        *slot = entries;
    }
    let fighters = Vec::load(r)?;
    let militia = Vec::load(r)?;
    world.insert_resource(FighterGrid {
        per_side,
        cells,
        masks,
        used,
        count,
        fighters,
        militia,
        occupancy,
    });
    Ok(())
}

/// Knowledge set: formation slots, then the army index and fighter grid.
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Bevy system parameters.
pub(crate) fn build_index(
    tick: Res<sim_core::SimTick>,
    map: Res<WorldMap>,
    army: Res<Army>,
    econ: Res<Economy>,
    flows: Res<FlowCache>,
    mut index: ResMut<ArmyIndex>,
    mut grid: ResMut<FighterGrid>,
    mut captains: Query<(
        Entity,
        &SpawnId,
        &mut Captain,
        &Position,
        &Motion,
        Option<&Stockpile>,
    )>,
    mut followers: Query<(&SpawnId, &mut Follower, &Position, &Motion, &Activity)>,
    fighters: Query<(
        Entity,
        &SpawnId,
        &Allegiance,
        &CombatStats,
        &Position,
        &Motion,
        &Activity,
    )>,
) {
    let _span = tracing::info_span!("sim_military::build_index").entered();
    // Armies, in SpawnId order.
    let mut armies: Vec<ArmyInfo> = captains
        .iter()
        .map(|(e, id, c, pos, motion, baggage)| {
            let field = c.goal.and_then(|g| flows.get((g, c.window.0, c.window.1)));
            let rally = c
                .rally
                .and_then(|r| flows.get((r, c.rally_window.0, c.rally_window.1)));
            let p = world_pos(pos, Some(motion));
            ArmyInfo {
                entity: e,
                id: *id,
                faction: c.faction,
                posture: c.posture,
                pos: p,
                goal: c.goal,
                field,
                rally,
                followers: 0,
                fighting: 0,
                stragglers: 0,
                food: baggage.map_or(0.0, |s| s.food_value(&econ)),
            }
        })
        .collect();
    armies.sort_unstable_by_key(|a| a.id);
    // Lookups only; never iterated (determinism rule).
    index.by_entity = armies
        .iter()
        .enumerate()
        .map(|(i, a)| (a.entity, i))
        .collect();

    // Head counts, and a membership signature per army.
    let mut signature = vec![(0u64, 0u64); armies.len()];
    let mut routing = vec![false; armies.len()];
    for (id, f, _, _, act) in &followers {
        let Some(&i) = index.by_entity.get(&f.captain) else {
            continue;
        };
        let a = &mut armies[i];
        a.followers += 1;
        if act.kind == ActionKind::Fight {
            a.fighting += 1;
        }
        routing[i] |= act.kind == ActionKind::Rout;
        let s = &mut signature[i];
        s.0 = s.0.wrapping_add(id.0);
        s.1 ^= id.0.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    // UI-09: the turning point of an army's battle is the first tick one
    // of its men is seen running.
    for (a, _) in armies.iter().zip(&routing).filter(|(_, r)| **r) {
        if let Ok((.., mut c, _, _, _)) = captains.get_mut(a.entity)
            && c.battle_since.is_some()
            && c.battle_rout.is_none()
        {
            c.battle_rout = Some(tick.0);
        }
    }
    // Formation slots: followers of each army in SpawnId order, renumbered
    // every 50 ticks (5 s) so a battle's losses don't reshuffle the ranks
    // every tick. Armies whose membership hasn't changed are skipped: their
    // slots are already what renumbering would give.
    if tick.0.is_multiple_of(50) {
        let changed: Vec<bool> = armies
            .iter()
            .zip(&signature)
            .map(|(a, s)| index.signatures.get(&a.entity) != Some(&(a.followers, s.0, s.1)))
            .collect();
        if changed.iter().any(|c| *c) {
            let mut members: Vec<(usize, SpawnId)> = followers
                .iter()
                .filter_map(|(id, f, ..)| {
                    let &i = index.by_entity.get(&f.captain)?;
                    changed[i].then_some((i, *id))
                })
                .collect();
            members.sort_unstable();
            let mut by_id: Vec<(SpawnId, u16)> = Vec::with_capacity(members.len());
            let mut current = usize::MAX;
            let mut n = 0u16;
            for (i, id) in &members {
                if *i != current {
                    current = *i;
                    n = 0;
                }
                by_id.push((*id, n));
                n = n.saturating_add(1);
            }
            by_id.sort_unstable();
            for (id, mut f, ..) in &mut followers {
                if let Ok(k) = by_id.binary_search_by_key(id, |(s, _)| *s)
                    && f.slot != by_id[k].1
                {
                    f.slot = by_id[k].1;
                }
            }
        }
        index.signatures = armies
            .iter()
            .zip(&signature)
            .map(|(a, s)| (a.entity, (a.followers, s.0, s.1)))
            .collect();
    }
    for (_, f, pos, motion, _) in &followers {
        if let Some(&i) = index.by_entity.get(&f.captain) {
            let a = &mut armies[i];
            let limit = army.formation_radius(a.followers) + army.data.formation.catch_up;
            if world_pos(pos, Some(motion)).distance(a.pos) > limit {
                a.stragglers += 1;
            }
        }
    }
    index.armies = armies;

    // Fighter grid.
    let per_side = (map.size() as f32 / CELL).ceil() as i32;
    if grid.per_side != per_side {
        grid.per_side = per_side;
        grid.cells = vec![Vec::new(); (per_side * per_side) as usize];
        grid.masks = vec![0; (per_side * per_side) as usize];
        grid.used.clear();
    }
    let used = std::mem::take(&mut grid.used);
    for c in used {
        grid.cells[c as usize].clear();
        grid.masks[c as usize] = 0;
    }
    let mut count = 0;
    grid.fighters.clear();
    grid.militia.clear();
    grid.occupancy.clear();
    for (e, id, allegiance, stats, pos, motion, act) in &fighters {
        if !stats.alive() {
            continue;
        }
        let Some(side) = Side::of(allegiance, stats) else {
            continue;
        };
        let p = world_pos(pos, Some(motion));
        if let Some(c) = grid.cell(p) {
            if grid.cells[c].is_empty() {
                grid.used.push(c as u32);
            }
            grid.masks[c] |= side.bit();
            grid.cells[c].push(FighterEntry {
                id: *id,
                entity: e,
                pos: p,
                side,
                routed: act.kind == ActionKind::Rout,
            });
            count += 1;
            *grid.occupancy.entry(p.floor().as_ivec2()).or_default() += 1;
            grid.fighters.push((*id, e));
            if side.against.is_some() {
                grid.militia.push((*id, e));
            }
        }
    }
    grid.militia.sort_unstable();
    let used = std::mem::take(&mut grid.used);
    for &c in &used {
        grid.cells[c as usize].sort_unstable_by_key(|e| e.id);
    }
    grid.used = used;
    grid.count = count;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::Calendar;

    fn army() -> Army {
        Army::default_data(&Calendar::default_data(), &Economy::default_data())
    }

    #[test]
    fn skipping_cells_without_foes_finds_the_same_foe() {
        use bevy_app::App;
        use bevy_ecs::system::RunSystemOnce;
        use sim_world::Terrain;
        let a = army();
        let mut app = App::new();
        app.insert_resource(WorldMap::flat(128, 64, Terrain::Grass))
            .insert_resource(a.clone())
            .insert_resource(Economy::default_data())
            .init_resource::<FlowCache>()
            .init_resource::<sim_core::SimTick>()
            .init_resource::<ArmyIndex>()
            .init_resource::<FighterGrid>();
        let world = app.world_mut();
        let soldier = || CombatStats {
            health: 10.0,
            max_health: 10.0,
            attack: 1.0,
            defense: 0.0,
            morale: 1.0,
            stance: Stance::Soldier,
        };
        let mut people = Vec::new();
        for i in 0..400u64 {
            let f = 1 + (i % 3 == 0) as u16;
            let p = IVec2::new((i * 37 % 120) as i32, (i * 53 % 120) as i32);
            let e = world
                .spawn((
                    SpawnId(i),
                    Allegiance(FactionId(f)),
                    soldier(),
                    Position(p),
                    Motion::default(),
                    Activity::default(),
                ))
                .id();
            people.push((e, p, f));
        }
        world.run_system_once(build_index).unwrap();
        let grid = world.resource::<FighterGrid>();
        for (e, p, f) in people {
            let side = Side {
                faction: FactionId(f),
                against: None,
            };
            let at = p.as_vec2() + 0.5;
            for radius in [3.0, 8.0, 24.0] {
                let (nearest, counts) = grid.nearest_hostile_slow(e, at, side, radius, &a);
                assert_eq!(grid.nearest_hostile(e, at, side, radius, &a), nearest);
                let fast = grid.count_nearby(e, at, side, radius, &a);
                assert_eq!(fast.enemies, counts.enemies);
            }
        }
    }

    #[test]
    fn hostility_rules() {
        let a = army();
        let p1 = Side {
            faction: FactionId(1),
            against: None,
        };
        let p2 = Side {
            faction: FactionId(2),
            against: None,
        };
        let militia_vs_1 = Side {
            faction: FactionId(0),
            against: Some(FactionId(1)),
        };
        assert!(p1.hostile_to(p2, &a) && p2.hostile_to(p1, &a));
        assert!(militia_vs_1.hostile_to(p1, &a) && p1.hostile_to(militia_vs_1, &a));
        assert!(!militia_vs_1.hostile_to(p2, &a) && !p2.hostile_to(militia_vs_1, &a));
        assert!(!p1.hostile_to(p1, &a));
        assert!(!militia_vs_1.hostile_to(militia_vs_1, &a));
    }
}
