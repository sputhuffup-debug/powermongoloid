//! Military components and resources (GP-01..04).

use std::collections::VecDeque;

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use sim_core::{FactionId, NameId, OrderSpec, Posture, StateHash, StateHasher};

/// An order in a captain's queue, with its progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order {
    pub spec: OrderSpec,
    /// Progress within the order (order-specific).
    pub step: u8,
    /// Tick the current step started.
    pub since: u64,
    /// Where the army was when the order started (scouting returns here).
    pub origin: IVec2,
    /// `Attack`, once the defenders broke: the tick the army's current
    /// uncontested hold of the centre began (`None` while contested).
    pub held_since: Option<u64>,
}

impl Order {
    /// A fresh order, not yet started.
    pub fn new(spec: OrderSpec) -> Self {
        Self {
            spec,
            step: 0,
            since: 0,
            origin: IVec2::ZERO,
            held_since: None,
        }
    }
}

fn hash_spec(spec: &OrderSpec, h: &mut StateHasher) {
    match *spec {
        OrderSpec::MoveTo { x, y } => {
            h.u8(0);
            h.i32(x);
            h.i32(y);
        }
        OrderSpec::Return => h.u8(1),
        OrderSpec::Recruit { settlement, count } => {
            h.u8(2);
            h.u64(settlement);
            h.u16(count);
        }
        OrderSpec::GatherFood { settlement } => {
            h.u8(3);
            h.u64(settlement);
        }
        OrderSpec::Equip { settlement } => {
            h.u8(4);
            h.u64(settlement);
        }
        OrderSpec::Attack { settlement } => {
            h.u8(5);
            h.u64(settlement);
        }
        OrderSpec::Trade { settlement } => {
            h.u8(6);
            h.u64(settlement);
        }
        OrderSpec::Scout { x, y } => {
            h.u8(7);
            h.i32(x);
            h.i32(y);
        }
        OrderSpec::Garrison { settlement } => {
            h.u8(8);
            h.u64(settlement);
        }
        OrderSpec::Forage { radius } => {
            h.u8(9);
            h.u16(radius);
        }
        OrderSpec::Give { settlement } => {
            h.u8(10);
            h.u64(settlement);
        }
        OrderSpec::Demand { settlement } => {
            h.u8(11);
            h.u64(settlement);
        }
        OrderSpec::Subvert { settlement } => {
            h.u8(12);
            h.u64(settlement);
        }
    }
}

/// Describes an order for the UI and the event log.
pub fn describe(spec: &OrderSpec) -> String {
    match *spec {
        OrderSpec::MoveTo { x, y } => format!("move to ({x}, {y})"),
        OrderSpec::Return => "return to camp".into(),
        OrderSpec::Recruit { count, .. } => format!("recruit {count}"),
        OrderSpec::GatherFood { .. } => "gather food".into(),
        OrderSpec::Equip { .. } => "equip".into(),
        OrderSpec::Attack { .. } => "attack".into(),
        OrderSpec::Trade { .. } => "trade".into(),
        OrderSpec::Scout { x, y } => format!("scout ({x}, {y})"),
        OrderSpec::Garrison { .. } => "garrison".into(),
        OrderSpec::Forage { radius } => format!("forage within {radius}"),
        OrderSpec::Give { .. } => "give food".into(),
        OrderSpec::Demand { .. } => "demand submission".into(),
        OrderSpec::Subvert { .. } => "send an agent".into(),
    }
}

/// A captain (SEED.md "Core data model"). The captain's own entity also
/// holds the army's baggage as a `Stockpile`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Captain {
    pub faction: FactionId,
    pub posture: Posture,
    pub orders: VecDeque<Order>,
    /// Where the army is marching; `None` = holding.
    pub goal: Option<IVec2>,
    /// The flow-field window for `goal`.
    pub window: (IVec2, IVec2),
    /// Followers rejoin along a field toward this tile (where the captain
    /// was when it was built) over `rally_window`.
    pub rally: Option<IVec2>,
    pub rally_window: (IVec2, IVec2),
    /// Last tick the army foraged from a settlement on its own.
    pub last_forage: u64,
    /// SIM-36: the tick the army's current battle began (its first blow
    /// struck or taken), `None` while it is not in one. A battle ends
    /// (`veterans::end_battles`) once no blow has landed for army.ron
    /// `veterans.quiet_hours`.
    pub battle_since: Option<u64>,
    /// SIM-36: the last tick a member of the army struck or was struck.
    pub last_engaged: u64,
    /// SIM-36: members of the army who died during its current battle.
    pub battle_losses: u32,
    /// UI-09: the factions the army has fought in its current battle, one
    /// bit per faction id (`1 << id`), for its battle report.
    pub battle_foes: u32,
    /// UI-09: foes its people killed in its current battle.
    pub battle_kills: u32,
    /// UI-09: the tick the first of its men broke and ran in its current
    /// battle (the report's turning point); `None` while it holds.
    pub battle_rout: Option<u64>,
}

impl Captain {
    /// A captain with no orders.
    pub fn new(faction: FactionId, posture: Posture) -> Self {
        Self {
            faction,
            posture,
            orders: VecDeque::new(),
            goal: None,
            window: (IVec2::ZERO, IVec2::ZERO),
            rally: None,
            rally_window: (IVec2::ZERO, IVec2::ZERO),
            last_forage: 0,
            battle_since: None,
            last_engaged: 0,
            battle_losses: 0,
            battle_foes: 0,
            battle_kills: 0,
            battle_rout: None,
        }
    }
}

impl StateHash for Captain {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u16(self.faction.0);
        h.u8(self.posture as u8);
        h.u32(self.orders.len() as u32);
        for o in &self.orders {
            hash_spec(&o.spec, h);
            h.u8(o.step);
            h.u64(o.since);
            h.ivec2(o.origin);
            match o.held_since {
                Some(t) => {
                    h.u8(1);
                    h.u64(t);
                }
                None => h.u8(0),
            }
        }
        match self.goal {
            Some(g) => {
                h.u8(1);
                h.ivec2(g);
            }
            None => h.u8(0),
        }
        h.ivec2(self.window.0);
        h.ivec2(self.window.1);
        match self.rally {
            Some(r) => {
                h.u8(1);
                h.ivec2(r);
            }
            None => h.u8(0),
        }
        h.ivec2(self.rally_window.0);
        h.ivec2(self.rally_window.1);
        h.u64(self.last_forage);
        match self.battle_since {
            Some(t) => {
                h.u8(1);
                h.u64(t);
            }
            None => h.u8(0),
        }
        h.u64(self.last_engaged);
        h.u32(self.battle_losses);
        h.u32(self.battle_foes);
        h.u32(self.battle_kills);
        match self.battle_rout {
            Some(t) => {
                h.u8(1);
                h.u64(t);
            }
            None => h.u8(0),
        }
    }
}

/// A follower of a captain (GP-04).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Follower {
    pub captain: Entity,
    /// Place in the loose formation (by SpawnId within the army).
    pub slot: u16,
    /// Decided to leave; applied by the serial desertion step.
    pub deserting: bool,
}

impl StateHash for Follower {
    fn hash_state(&self, h: &mut StateHasher) {
        h.entity(self.captain);
        h.u16(self.slot);
        h.bool(self.deserting);
    }
}

/// Arms from the smithy (SIM-32): a weapon adds attack, tools add a little
/// defense.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Equipment {
    pub weapon: bool,
    pub tools: bool,
}

impl StateHash for Equipment {
    fn hash_state(&self, h: &mut StateHasher) {
        h.bool(self.weapon);
        h.bool(self.tools);
    }
}

/// A soldier who isn't a villager (a starting army's men).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Soldier {
    pub name: NameId,
}

impl StateHash for Soldier {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.name.0);
    }
}

/// A captain driven by the scripted hostile band (M3 stand-in for rival
/// warlords, GP-09 in M5). It issues ordinary AI commands.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Raider {
    pub next_think: u64,
    /// The settlement (SpawnId) raided last.
    pub last_target: Option<u64>,
    /// GP-16: the settlement (SpawnId) the pacing director sent the band
    /// to raid first; cleared once it has been raided. `None` for bands
    /// the director didn't aim.
    pub target: Option<u64>,
}

impl StateHash for Raider {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.next_think);
        h.u64(self.last_target.map_or(u64::MAX, |t| t));
        match self.target {
            Some(t) => {
                h.u8(1);
                h.u64(t);
            }
            None => h.u8(0),
        }
    }
}

/// A band of broken men from a town the player took by force (GP-33, M4a
/// step 6): on a RAIDERS captain beside its [`Raider`], passed to the heir
/// with it. Hashed and saved.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bandits {
    /// The town (SpawnId) they came from.
    pub home: u64,
    /// Towns they have raided.
    pub raids: u32,
}

impl StateHash for Bandits {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.home);
        h.u32(self.raids);
    }
}

/// Per-faction state: the seat (starting camp) armies return to. Orders
/// travel from the seat by messenger in M5 (GP-07).
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct Factions {
    pub seats: Vec<Option<IVec2>>,
}

impl Factions {
    /// The faction's seat (where `Return` goes), if set.
    pub fn seat(&self, f: FactionId) -> Option<IVec2> {
        self.seats.get(f.0 as usize).copied().flatten()
    }

    /// Sets the faction's seat.
    pub fn set_seat(&mut self, f: FactionId, at: IVec2) {
        let i = f.0 as usize;
        if self.seats.len() <= i {
            self.seats.resize(i + 1, None);
        }
        self.seats[i] = Some(at);
    }
}

impl StateHash for Factions {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.seats.len() as u32);
        for s in &self.seats {
            match s {
                Some(p) => {
                    h.u8(1);
                    h.ivec2(*p);
                }
                None => h.u8(0),
            }
        }
    }
}

/// Running totals per faction, for tests, the UI and the event log.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FactionStats {
    pub recruited: u32,
    pub deserted: u32,
    /// Own soldiers killed.
    pub lost: u32,
    /// Enemies (soldiers or militia) killed.
    pub kills: u32,
    /// Villagers of other settlements killed.
    pub villagers_killed: u32,
    /// Food value seized, bought or given freely.
    pub seized: f32,
    pub bought: f32,
    pub given: f32,
    pub provocations: u32,
    pub starvation_damage: f32,
    pub first_desertion: Option<u64>,
    pub first_starvation: Option<u64>,
    pub rejected_commands: u32,
    /// Settlements taken by force (GP-05, M4).
    pub conquests: u32,
    /// Food value drawn, unpaid and unprovoked, from settlements the
    /// faction owns (garrison supply, M4). Not counted in `seized`,
    /// `bought` or `given`.
    pub supplied: f32,
    /// Food value handed to settlements by `Give` orders (GP-22, M4): what
    /// was actually delivered. Not `given`, which is village to army.
    pub donated: f32,
    /// Food value gathered from the wild by `Forage` orders (SIM-38, M4).
    pub foraged: f32,
}

/// Totals for every faction, by faction id. Hashed.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct ArmyStats {
    pub factions: Vec<FactionStats>,
}

impl ArmyStats {
    /// A faction's totals (zero if none yet).
    pub fn get(&self, f: FactionId) -> FactionStats {
        self.factions.get(f.0 as usize).copied().unwrap_or_default()
    }

    /// A faction's totals, to update.
    pub fn of(&mut self, f: FactionId) -> &mut FactionStats {
        let i = f.0 as usize;
        if self.factions.len() <= i {
            self.factions.resize(i + 1, FactionStats::default());
        }
        &mut self.factions[i]
    }
}

impl StateHash for ArmyStats {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u32(self.factions.len() as u32);
        for s in &self.factions {
            h.u32(s.recruited);
            h.u32(s.deserted);
            h.u32(s.lost);
            h.u32(s.kills);
            h.u32(s.villagers_killed);
            h.f32(s.seized);
            h.f32(s.bought);
            h.f32(s.given);
            h.u32(s.provocations);
            h.f32(s.starvation_damage);
            h.u64(s.first_desertion.unwrap_or(u64::MAX));
            h.u64(s.first_starvation.unwrap_or(u64::MAX));
            h.u32(s.rejected_commands);
            h.u32(s.conquests);
            h.f32(s.supplied);
            h.f32(s.donated);
            h.f32(s.foraged);
        }
    }
}

/// Recent military events for the UI (UI-01). History only.
#[derive(Resource, Debug, Clone, Default)]
pub struct MilitaryLog {
    /// `(tick, text)`, oldest first; the last 64.
    pub entries: VecDeque<(u64, String)>,
    /// Entries ever pushed (UI-05): entry `i` of `entries` has sequence
    /// number `seq - entries.len() + i`, so the event feed can read new
    /// lines by cursor, like [`sim_core::EventFeed`].
    pub seq: u64,
}

impl MilitaryLog {
    const MAX: usize = 64;

    /// Adds an entry, dropping the oldest when full.
    pub fn push(&mut self, tick: u64, text: impl Into<String>) {
        if self.entries.len() >= Self::MAX {
            self.entries.pop_front();
        }
        self.entries.push_back((tick, text.into()));
        self.seq += 1;
    }

    /// Entries with sequence number `>= cursor`, oldest first, as
    /// `(seq, tick, text)`. Pass the last seen `seq + 1`.
    pub fn since(&self, cursor: u64) -> impl Iterator<Item = (u64, u64, &str)> + '_ {
        let first = self.seq - self.entries.len() as u64;
        self.entries
            .iter()
            .enumerate()
            .map(move |(i, (tick, text))| (first + i as u64, *tick, text.as_str()))
            .filter(move |(seq, ..)| *seq >= cursor)
    }
}
