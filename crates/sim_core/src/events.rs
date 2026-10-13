//! Typed simulation events, the event feed and the chronicle (UI-05, UI-09).
//!
//! Any sim crate reports something notable by writing a [`SimEvent`] message:
//! exclusive steps with `world.write_message`, systems with a
//! `MessageWriter<SimEvent>`. A single reader, [`record_events`] in
//! [`BookkeepingSet::Record`](crate::BookkeepingSet::Record), sorts each
//! tick's events (writers in one set run in executor order), stamps them with
//! the tick and appends them to:
//! - [`EventFeed`]: a ring of recent events for the UI (not hashed);
//! - [`Chronicle`]: the run's append-only history, for kinds that
//!   [`SimEventKind::chronicled`] (hashed, saved).
//!
//! Messages are not part of the state hash and live only two ticks, so tests
//! assert on [`Chronicle`] or [`EventFeed`], never on a `MessageReader`. The
//! sim stores no text: events carry ids and a [`Cause`], and the UI formats
//! them.

use std::collections::{HashMap, VecDeque};

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use serde::{Deserialize, Serialize};

use crate::components::FactionId;
use crate::hash::{StateHash, StateHasher};
use crate::names::NameId;
use crate::schedule::SimTick;

/// What happened. Append only: the discriminant is stored in saves and is
/// part of the sort order of a tick's events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SimEventKind {
    /// A run began (GP-21). `subject`: the champion's SpawnId; `other`: the
    /// rival's seat; `at`: the start spot; `amount`: the band template.
    RunStarted,
    /// An offer is waiting (GP-27). `subject`: the offer id; `other`: what it
    /// concerns ([`OfferKind::subject`](crate::OfferKind::subject));
    /// `amount`: the number of choices.
    OfferPosted,
    /// An offer was answered. `subject`: the offer id; `other`: the card
    /// picked (its index in the deck); `amount`: the offer kind's tag
    /// ([`OfferKind::tag`](crate::OfferKind::tag): 0 start boon, 1 dominion
    /// boon, 2 trait, 3 invitation, 4 hire), which says which deck.
    OfferChosen,
    /// A monument changed to `faction` (GP-23). `subject`: the monument;
    /// `other`: the previous holder's faction id (0 for nobody); `at`: the
    /// monument; `amount`: its kind (`MonumentKind` index).
    MonumentTaken,
    /// `faction` lost a monument. `subject`: the monument; `other`: the
    /// faction that took it (0 when it drifted back to nobody, cause
    /// [`Cause::Neglect`]); `at`: the monument; `amount`: its kind.
    MonumentLost,
    /// A settlement asks the player for help (GP-22). `subject`: the
    /// request id; `other`: the settlement; `at`: its centre; `amount`: the
    /// request kind's tag (0 drive off, 1 bring food, 2 clear the camp).
    RequestPosted,
    /// A request was completed (cause `Cause::Request`). `subject`: the
    /// request id; `other`: the settlement; `at`: its centre; `amount`: the
    /// kind's tag.
    RequestDone,
    /// A request expired, or its danger passed without the player (cause
    /// [`Cause::Lapsed`]). `subject`: the request id; `other`: the
    /// settlement; `at`: its centre; `amount`: the kind's tag.
    RequestFailed,
    /// `faction` took a settlement by force (GP-05). `subject`: the
    /// settlement; `other`: its previous owner's faction id; `at`: its
    /// centre; `amount`: residents handed over.
    Conquest,
    /// A settlement invited `faction` in (GP-22). `subject`: the
    /// settlement; `other`: its previous owner's faction id; `at`: its
    /// centre; `amount`: residents handed over.
    Invited,
    /// A settlement rose against `faction`, its owner, and is independent
    /// again (GP-06). `subject`: the settlement; `other`: the new owner's
    /// faction id (0); `at`: its centre; `amount`: residents.
    Revolt,
    /// The champion ranked up (SIM-35). `subject`: the champion; `other`:
    /// its rank points; `at`: its tile; `amount`: the rank.
    RankUp,
    /// Someone gained a trait (SIM-34). `subject`: the person; `amount`: the trait id.
    TraitGained,
    /// Soldiers of one army reached a veteran rank after a battle (SIM-36),
    /// grouped so a big battle doesn't flood the feed (the names are in its
    /// [`BattleReport`]). `subject`: the army's captain; `other`: how many;
    /// `at`: the captain's tile; `amount`: the rank.
    VeteranRank,
    /// The champion died (SIM-35). `subject`: the champion; `at`: where;
    /// `amount`: the champion's name (a `NameId`, since the person is gone);
    /// `cause`: `Death` (the striker's faction) or `Starvation`.
    ChampionFell,
    /// A captain became champion (SIM-35). `subject`: the new champion;
    /// `other`: the fallen one; `at`: the new champion's tile; `amount`:
    /// traits kept; `cause`: the fallen's.
    Succession,
    /// The rival enlisted men at its seat (GP-28). `faction`: the rival;
    /// `subject`: the garrison's captain; `at`: the seat's centre;
    /// `amount`: how many.
    RivalLevy,
    /// The rival sent a raiding band (GP-28). `faction`: the rival;
    /// `subject`: its captain; `at`: the seat's centre; `amount`: people in
    /// the band, its captain included; `cause`: `Raid` (its captain).
    RivalRaid,
    /// The pacing director brought raiders (GP-16). `faction`: the raiders;
    /// `subject`: their captain; `other`: the settlement they were sent to
    /// raid (0 for none); `at`: where they appeared; `amount`: people in
    /// the band, its captain included; `cause`: `Raid` (their captain).
    RaidersArrived,
    /// An army's battle ended (SIM-36). `faction`: the army's; `subject`:
    /// its captain; `other`: the survivors who fought; `at`: the captain's
    /// tile; `amount`: the army's dead; `cause`: `Fight` (its first foe).
    /// Its [`BattleReport`] is in [`Chronicle::reports`] (UI-09).
    Battle,
    /// Volunteers joined a band (GP-22). `subject`: the captain; `other`:
    /// the settlement they came from; `at`: the captain's tile; `amount`:
    /// how many.
    DriftersJoined,
    /// The run ended. `subject`: the [`RunOutcome`](crate::RunOutcome) as a number.
    RunEnded,
    /// A settlement knelt to `faction` after a demand (GP-32, M4a, tag 22;
    /// cause [`Cause::Submission`]). `subject`: the settlement; `other`:
    /// its previous owner's faction id; `at`: its centre; `amount`:
    /// residents handed over. A take: no `conquests` count.
    Submitted,
    /// A submitted settlement defied `faction`, its owner, and is free
    /// again (GP-32, M4a, tag 23; cause [`Cause::Defiance`]), told like a
    /// [`SimEventKind::Revolt`]. `subject`: the settlement; `other`: the new
    /// owner's faction id (0); `at`: its centre; `amount`: residents.
    Defied,
    /// A settlement an agent turned opened its gates to `faction` (GP-32,
    /// M4a, tag 24; cause [`Cause::Subversion`]). `subject`: the
    /// settlement; `other`: its previous owner's faction id; `at`: its
    /// centre; `amount`: residents handed over. A take: no `conquests`.
    Subverted,
    /// Broken men of a taken settlement took to the woods as bandits
    /// (GP-33, M4a, tag 25; cause [`Cause::Fallout`] naming the
    /// settlement). `faction`: who took it; `subject`: the band's captain;
    /// `other`: the settlement (its home); `at`: where they rose;
    /// `amount`: men in the band, its captain included.
    BanditsRose,
    /// The rival vowed to retake a settlement (GP-33, M4a, tag 26).
    /// `faction`: the rival; `subject`: the settlement it names; `other`:
    /// the tick the reprisal is due; `at`: its centre; `amount`: game
    /// hours until then; `cause`: what provoked it (the taking's cause, or
    /// [`Cause::Caught`]).
    ReprisalVowed,
    /// The rival sent a reprisal band (GP-33, M4a, tag 27; cause
    /// [`Cause::Reprisal`] naming its target). `faction`: the rival;
    /// `subject`: its captain; `other`: the settlement it is pinned to;
    /// `at`: the seat's centre; `amount`: people in the band, its captain
    /// included.
    RivalReprisal,
    /// An agent was caught and killed (GP-32, M4a, tag 28; cause
    /// [`Cause::Caught`]). `faction`: the agent's lord; `subject`: the
    /// agent's SpawnId; `other`: the settlement; `at`: its centre;
    /// `amount`: the agent's name (a `NameId`, since the person is gone).
    AgentCaught,
    /// A sworn settlement's call went unanswered (GP-32, M4a, tag 29;
    /// cause [`Cause::Lapsed`]). `faction`: its lord, who forsook it;
    /// `subject`: the settlement; `other`: the request id; `at`: its
    /// centre; `amount`: times it has been forsaken. Chronicled, quiet.
    Forsaken,
    /// An ordered building was finished (SIM-63, M4a, tag 30). `faction`:
    /// the settlement's owner; `subject`: the building; `other`: the
    /// settlement; `at`: the building's tile; `amount`: the
    /// [`BuildKind`](crate::BuildKind) tag. Chronicled, quiet.
    Built,
    /// A settlement has a new steward, or lost one (GP-34, M4a, tag 31).
    /// `faction`: the owner; `subject`: the steward; `other`: the
    /// settlement; `at`: its centre; `amount`: 0 a resident named, 1 a
    /// follower appointed, 2 deposed (cause [`Cause::Deposed`]); `cause`
    /// otherwise `None` or `Player`. Chronicled, quiet.
    StewardNamed,
    /// A dial of a settlement's edicts turned (GP-34, M4a, tag 32).
    /// `faction`: the owner; `subject`: the settlement; `other`: the
    /// [`Dial`](crate::Dial) tag; `at`: its centre; `amount`: the new
    /// setting's tag; `cause`: `Player` when the player set it,
    /// [`Cause::Edict`] when the steward's habit did. Feed only.
    EdictSet,
    /// An owner's lands sent its army food, arms and men (GP-34, M4a, tag
    /// 33; cause [`Cause::Edict`]), one per receiving captain a day.
    /// `faction`: the owner; `subject`: the captain; `other`: food value
    /// delivered (whole units) in the low 32 bits and arms (weapons and
    /// tools) in the high 32 bits; `at`: the captain's tile; `amount`: men.
    /// Feed only.
    Tribute,
}

impl SimEventKind {
    /// Every kind, in discriminant order.
    pub const ALL: [SimEventKind; 34] = [
        SimEventKind::RunStarted,
        SimEventKind::OfferPosted,
        SimEventKind::OfferChosen,
        SimEventKind::MonumentTaken,
        SimEventKind::MonumentLost,
        SimEventKind::RequestPosted,
        SimEventKind::RequestDone,
        SimEventKind::RequestFailed,
        SimEventKind::Conquest,
        SimEventKind::Invited,
        SimEventKind::Revolt,
        SimEventKind::RankUp,
        SimEventKind::TraitGained,
        SimEventKind::VeteranRank,
        SimEventKind::ChampionFell,
        SimEventKind::Succession,
        SimEventKind::RivalLevy,
        SimEventKind::RivalRaid,
        SimEventKind::RaidersArrived,
        SimEventKind::Battle,
        SimEventKind::DriftersJoined,
        SimEventKind::RunEnded,
        SimEventKind::Submitted,
        SimEventKind::Defied,
        SimEventKind::Subverted,
        SimEventKind::BanditsRose,
        SimEventKind::ReprisalVowed,
        SimEventKind::RivalReprisal,
        SimEventKind::AgentCaught,
        SimEventKind::Forsaken,
        SimEventKind::Built,
        SimEventKind::StewardNamed,
        SimEventKind::EdictSet,
        SimEventKind::Tribute,
    ];

    /// Display name (debug output, logs).
    pub fn name(self) -> &'static str {
        match self {
            SimEventKind::RunStarted => "run started",
            SimEventKind::OfferPosted => "offer posted",
            SimEventKind::OfferChosen => "offer chosen",
            SimEventKind::MonumentTaken => "monument taken",
            SimEventKind::MonumentLost => "monument lost",
            SimEventKind::RequestPosted => "request posted",
            SimEventKind::RequestDone => "request done",
            SimEventKind::RequestFailed => "request failed",
            SimEventKind::Conquest => "conquest",
            SimEventKind::Invited => "invited",
            SimEventKind::Revolt => "revolt",
            SimEventKind::RankUp => "rank up",
            SimEventKind::TraitGained => "trait gained",
            SimEventKind::VeteranRank => "veteran rank",
            SimEventKind::ChampionFell => "champion fell",
            SimEventKind::Succession => "succession",
            SimEventKind::RivalLevy => "rival levy",
            SimEventKind::RivalRaid => "rival raid",
            SimEventKind::RaidersArrived => "raiders arrived",
            SimEventKind::Battle => "battle",
            SimEventKind::DriftersJoined => "drifters joined",
            SimEventKind::RunEnded => "run ended",
            SimEventKind::Submitted => "submitted",
            SimEventKind::Defied => "defied",
            SimEventKind::Subverted => "subverted",
            SimEventKind::BanditsRose => "bandits rose",
            SimEventKind::ReprisalVowed => "reprisal vowed",
            SimEventKind::RivalReprisal => "rival reprisal",
            SimEventKind::AgentCaught => "agent caught",
            SimEventKind::Forsaken => "forsaken",
            SimEventKind::Built => "built",
            SimEventKind::StewardNamed => "steward named",
            SimEventKind::EdictSet => "edict set",
            SimEventKind::Tribute => "tribute",
        }
    }

    /// Whether events of this kind go into the [`Chronicle`] (UI-09). The
    /// rest (offers posted, requests posted, levies, single soldiers' ranks,
    /// drifters, and M4a's edicts set and tribute) are feed-only.
    pub fn chronicled(self) -> bool {
        !matches!(
            self,
            SimEventKind::OfferPosted
                | SimEventKind::RequestPosted
                | SimEventKind::VeteranRank
                | SimEventKind::RivalLevy
                | SimEventKind::DriftersJoined
                | SimEventKind::EdictSet
                | SimEventKind::Tribute
        )
    }

    /// Chronicled kinds that are not news (M4a, ADR 0020): they go into the
    /// [`Chronicle`] but don't reset the director's quiet clock or stop
    /// fast-forward: `Forsaken`, `Built` and `StewardNamed`.
    pub fn quiet(self) -> bool {
        matches!(
            self,
            SimEventKind::Forsaken | SimEventKind::Built | SimEventKind::StewardNamed
        )
    }

    /// Whether this kind resets the pacing director's quiet clock
    /// ([`Chronicle::last_notable`], GP-16): every chronicled kind that
    /// isn't [`quiet`](Self::quiet), plus a posted request (something for
    /// the player to do). No kind before M4a is quiet, so their
    /// classification is unchanged.
    pub fn notable(self) -> bool {
        (self.chronicled() && !self.quiet()) || self == SimEventKind::RequestPosted
    }
}

/// Why something happened, for the chronicle's "after they drove off the
/// raiders" (UI-09). Ids are SpawnIds or offer, request and card ids.
/// Append only: the tag is stored in saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Cause {
    /// No particular cause.
    #[default]
    None,
    /// The player chose it (a command).
    Player,
    /// Completing request `id`.
    Request { id: u64 },
    /// Conquest by the army of `captain`.
    Conquest { captain: u64 },
    /// An accepted invitation.
    Invitation,
    /// Hunger.
    Starvation,
    /// Plunder.
    Plunder,
    /// A revolt.
    Revolt,
    /// A raid led by `captain`.
    Raid { captain: u64 },
    /// Holding ground (a monument or a settlement centre) long enough.
    Hold,
    /// The effect of boon `card` (an index into progression.ron's boons).
    Boon { card: u16 },
    /// A death at the hands of `faction`.
    Death { faction: FactionId },
    /// Nobody answered in time (a request that lapsed, UI-09).
    Lapsed,
    /// Left unwatched too long (a monument that drifted back to nobody).
    Neglect,
    /// Fighting `faction` (a battle; its first foe by faction id).
    Fight { faction: FactionId },
    /// A town sacked by the army of `captain`: a conquest at Aggressive
    /// posture that plunders the storehouse (GP-32, M4a, tag 15).
    Sack { captain: u64 },
    /// A town that knelt to the army of `captain` after a demand (GP-32,
    /// M4a, tag 16).
    Submission { captain: u64 },
    /// A town an agent turned, its gates opened (GP-32, M4a, tag 17).
    Subversion,
    /// A submitted town that defied its lord (GP-32, M4a, tag 18).
    Defiance,
    /// The fallout of taking `settlement` (a SpawnId): its broken men
    /// (GP-33, M4a, tag 19).
    Fallout { settlement: u64 },
    /// The rival's reprisal for `settlement` (a SpawnId) (GP-33, M4a,
    /// tag 20).
    Reprisal { settlement: u64 },
    /// By a settlement's edict, or its steward's order (GP-34, M4a,
    /// tag 21).
    Edict,
    /// An agent caught at his work (GP-32, M4a, tag 22).
    Caught,
    /// A steward deposed when the town changed hands (GP-34, M4a, tag 23).
    Deposed,
}

impl Cause {
    /// Stable tag of the variant (saves, hashing, sorting).
    pub fn tag(self) -> u8 {
        match self {
            Cause::None => 0,
            Cause::Player => 1,
            Cause::Request { .. } => 2,
            Cause::Conquest { .. } => 3,
            Cause::Invitation => 4,
            Cause::Starvation => 5,
            Cause::Plunder => 6,
            Cause::Revolt => 7,
            Cause::Raid { .. } => 8,
            Cause::Hold => 9,
            Cause::Boon { .. } => 10,
            Cause::Death { .. } => 11,
            Cause::Lapsed => 12,
            Cause::Neglect => 13,
            Cause::Fight { .. } => 14,
            Cause::Sack { .. } => 15,
            Cause::Submission { .. } => 16,
            Cause::Subversion => 17,
            Cause::Defiance => 18,
            Cause::Fallout { .. } => 19,
            Cause::Reprisal { .. } => 20,
            Cause::Edict => 21,
            Cause::Caught => 22,
            Cause::Deposed => 23,
        }
    }

    /// The cause in a few plain words, with no names (a summary line, a log
    /// line: "by your choice", "conquest", "killed in battle"). The ids in
    /// the payload are not looked up; the chronicle's sentences name them
    /// through progression.ron's `chronicle.causes`.
    pub fn describe(self) -> &'static str {
        match self {
            Cause::None => "no cause",
            Cause::Player => "by your choice",
            Cause::Request { .. } => "a request",
            Cause::Conquest { .. } => "conquest",
            Cause::Invitation => "an invitation",
            Cause::Starvation => "starvation",
            Cause::Plunder => "plunder",
            Cause::Revolt => "a revolt",
            Cause::Raid { .. } => "a raid",
            Cause::Hold => "held long enough",
            Cause::Boon { .. } => "a boon",
            Cause::Death { .. } => "killed in battle",
            Cause::Lapsed => "left unanswered",
            Cause::Neglect => "neglect",
            Cause::Fight { .. } => "a battle",
            Cause::Sack { .. } => "a sack",
            Cause::Submission { .. } => "a submission",
            Cause::Subversion => "an agent's work",
            Cause::Defiance => "defiance",
            Cause::Fallout { .. } => "the fallout of a taking",
            Cause::Reprisal { .. } => "a reprisal",
            Cause::Edict => "an edict",
            Cause::Caught => "caught spying",
            Cause::Deposed => "deposed",
        }
    }

    /// The variant's payload as a number (0 for none).
    pub fn payload(self) -> u64 {
        match self {
            Cause::Request { id } => id,
            Cause::Conquest { captain }
            | Cause::Raid { captain }
            | Cause::Sack { captain }
            | Cause::Submission { captain } => captain,
            Cause::Boon { card } => card as u64,
            Cause::Death { faction } | Cause::Fight { faction } => faction.0 as u64,
            Cause::Fallout { settlement } | Cause::Reprisal { settlement } => settlement,
            Cause::None
            | Cause::Player
            | Cause::Invitation
            | Cause::Starvation
            | Cause::Plunder
            | Cause::Revolt
            | Cause::Hold
            | Cause::Lapsed
            | Cause::Neglect
            | Cause::Subversion
            | Cause::Defiance
            | Cause::Edict
            | Cause::Caught
            | Cause::Deposed => 0,
        }
    }
}

impl StateHash for Cause {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.tag());
        h.u64(self.payload());
    }
}

/// Something notable happened (UI-05). A message: not hashed, gone after two
/// ticks. [`record_events`] turns it into feed and chronicle entries.
/// Written inside ticks: run setup writes none, because messages are not
/// saved (a save before the first tick would lose them).
/// Each [`SimEventKind`] documents what `subject`, `other` and `amount` mean.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct SimEvent {
    /// What happened.
    pub kind: SimEventKind,
    /// The faction it happened to or for.
    pub faction: FactionId,
    /// The main thing involved: a SpawnId, or an offer or request id.
    pub subject: u64,
    /// A second thing involved (0 if none).
    pub other: u64,
    /// Where, in tiles (the camera jumps here from the feed).
    pub at: IVec2,
    /// A count, rank, choice or id, per kind.
    pub amount: i32,
    /// Why.
    pub cause: Cause,
}

impl SimEvent {
    /// An event of `kind` for `faction`, with every other field zero.
    pub fn new(kind: SimEventKind, faction: FactionId) -> Self {
        Self {
            kind,
            faction,
            subject: 0,
            other: 0,
            at: IVec2::ZERO,
            amount: 0,
            cause: Cause::None,
        }
    }

    /// Sets `subject`.
    pub fn subject(mut self, subject: u64) -> Self {
        self.subject = subject;
        self
    }

    /// Sets `other`.
    pub fn other(mut self, other: u64) -> Self {
        self.other = other;
        self
    }

    /// Sets `at`.
    pub fn at(mut self, at: IVec2) -> Self {
        self.at = at;
        self
    }

    /// Sets `amount`.
    pub fn amount(mut self, amount: i32) -> Self {
        self.amount = amount;
        self
    }

    /// Sets `cause`.
    pub fn cause(mut self, cause: Cause) -> Self {
        self.cause = cause;
        self
    }

    /// The order [`record_events`] records a tick's events in: by kind,
    /// faction, subject, other, place, amount, then cause.
    #[allow(clippy::type_complexity)] // A plain lexicographic key.
    pub fn sort_key(&self) -> (u8, u16, u64, u64, i32, i32, i32, u8, u64) {
        (
            self.kind as u8,
            self.faction.0,
            self.subject,
            self.other,
            self.at.x,
            self.at.y,
            self.amount,
            self.cause.tag(),
            self.cause.payload(),
        )
    }
}

impl StateHash for SimEvent {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.kind as u8);
        h.u16(self.faction.0);
        h.u64(self.subject);
        h.u64(self.other);
        h.ivec2(self.at);
        h.i32(self.amount);
        self.cause.hash_state(h);
    }
}

/// An event and the tick it happened on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChronicleEntry {
    /// The tick the event was recorded on.
    pub tick: u64,
    /// What happened.
    pub event: SimEvent,
}

impl ChronicleEntry {
    /// Folds this entry into a running digest.
    fn extend(&self, digest: u64) -> u64 {
        let no_entities = HashMap::new();
        let mut h = StateHasher::new(&no_entities);
        h.u64(digest);
        h.u64(self.tick);
        self.event.hash_state(&mut h);
        h.finish()
    }
}

/// A soldier who rose in veteran rank in a battle (SIM-36), named so the
/// chronicle can tell of its Elites after they are gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Promotion {
    /// The soldier's SpawnId.
    pub soldier: u64,
    /// The rank reached.
    pub rank: u8,
    /// The soldier's name.
    pub name: NameId,
}

/// One army's account of a battle (UI-09), written when its battle ends
/// (with its `Battle` event, same tick, captain and faction): who fought
/// whom, the losses on both sides, the turning point and the ranks earned.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleReport {
    /// The tick the battle ended (its `Battle` entry's tick).
    pub tick: u64,
    /// The tick of its first blow.
    pub started: u64,
    /// The army's faction.
    pub faction: FactionId,
    /// The army's captain (SpawnId).
    pub captain: u64,
    /// The factions it fought, ascending (0: a village's militia).
    pub foes: Vec<FactionId>,
    /// Where its captain stood at the end.
    pub at: IVec2,
    /// Survivors who struck or were struck.
    pub fought: u32,
    /// Its dead.
    pub losses: u32,
    /// Foes its people killed.
    pub kills: u32,
    /// The turning point: the tick its first man broke and ran (`None`:
    /// it held to the end).
    pub turning_point: Option<u64>,
    /// Its people who rose in veteran rank, in SpawnId order: the ranks
    /// earned (traits from triggers come in a later milestone).
    pub promotions: Vec<Promotion>,
}

impl BattleReport {
    fn hash_into(&self, h: &mut StateHasher) {
        h.u64(self.tick);
        h.u64(self.started);
        h.u16(self.faction.0);
        h.u64(self.captain);
        h.u32(self.foes.len() as u32);
        for f in &self.foes {
            h.u16(f.0);
        }
        h.ivec2(self.at);
        h.u32(self.fought);
        h.u32(self.losses);
        h.u32(self.kills);
        h.u64(self.turning_point.map_or(u64::MAX, |t| t));
        h.u32(self.promotions.len() as u32);
        for p in &self.promotions {
            h.u64(p.soldier);
            h.u8(p.rank);
            h.u32(p.name.0);
        }
    }
}

/// The run's history (UI-09): every chronicled event in recording order,
/// and a [`BattleReport`] per battle. Append only. Hashed (as its lengths,
/// a running digest of every entry and report, and the last notable tick,
/// so hashing is O(1)) and saved.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Chronicle {
    entries: Vec<ChronicleEntry>,
    reports: Vec<BattleReport>,
    digest: u64,
    last_notable: u64,
}

impl Chronicle {
    /// Appends an entry and updates the digest. Only [`record_events`] and
    /// [`settle_outcome`](crate::run::settle_outcome) call this.
    pub fn push(&mut self, entry: ChronicleEntry) {
        self.digest = entry.extend(self.digest);
        self.entries.push(entry);
    }

    /// Appends a battle report and updates the digest (sim_military's
    /// battle step, in captain SpawnId order).
    pub fn push_report(&mut self, report: BattleReport) {
        let no_entities = HashMap::new();
        let mut h = StateHasher::new(&no_entities);
        h.u64(self.digest);
        report.hash_into(&mut h);
        self.digest = h.finish();
        self.reports.push(report);
    }

    /// Every battle report, oldest first.
    pub fn reports(&self) -> &[BattleReport] {
        &self.reports
    }

    /// The report of a `Battle` entry: same tick, faction and captain.
    pub fn report_for(&self, entry: &ChronicleEntry) -> Option<&BattleReport> {
        (entry.event.kind == SimEventKind::Battle)
            .then(|| {
                self.reports.iter().find(|r| {
                    r.tick == entry.tick
                        && r.faction == entry.event.faction
                        && r.captain == entry.event.subject
                })
            })
            .flatten()
    }

    /// Every entry, oldest first.
    pub fn entries(&self) -> &[ChronicleEntry] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been chronicled.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A digest of every entry so far (part of the state hash).
    pub fn digest(&self) -> u64 {
        self.digest
    }

    /// The tick of the last [notable](SimEventKind::notable) event (0 before
    /// any). The pacing director measures quiet stretches from it (GP-16).
    pub fn last_notable(&self) -> u64 {
        self.last_notable
    }
}

impl StateHash for Chronicle {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.entries.len() as u64);
        h.u64(self.reports.len() as u64);
        h.u64(self.digest);
        h.u64(self.last_notable);
    }
}

/// One event in the [`EventFeed`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedEntry {
    /// Increases by one per entry, never reused; the UI reads by cursor.
    pub seq: u64,
    /// The tick the event was recorded on.
    pub tick: u64,
    /// What happened.
    pub event: SimEvent,
}

/// The most recent events, chronicled or not, for the UI's feed (UI-05).
/// Exempt from the hash: it is derived from the events, and the chronicle
/// keeps what matters.
#[derive(Resource, Debug, Clone, Default)]
pub struct EventFeed {
    entries: VecDeque<FeedEntry>,
    next_seq: u64,
}

impl EventFeed {
    /// Entries kept; older ones drop off.
    pub const CAPACITY: usize = 256;

    /// Appends an event and returns its `seq`.
    pub fn push(&mut self, tick: u64, event: SimEvent) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        if self.entries.len() == Self::CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(FeedEntry { seq, tick, event });
        seq
    }

    /// Entries with `seq >= cursor`, oldest first. Pass the last seen `seq + 1`.
    pub fn since(&self, cursor: u64) -> impl Iterator<Item = &FeedEntry> + '_ {
        self.entries.iter().filter(move |e| e.seq >= cursor)
    }

    /// Every entry kept, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &FeedEntry> + '_ {
        self.entries.iter()
    }

    /// The `seq` the next entry will get.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Number of entries kept.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the feed is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// GP-12: the chronicle and the feed are saved (the feed so the UI's
// cursor carries on after a load).
crate::save_serde!(SimEventKind, Cause);
crate::save_fields!(SimEvent {
    kind,
    faction,
    subject,
    other,
    at,
    amount,
    cause
});
crate::save_fields!(ChronicleEntry { tick, event });
crate::save_fields!(Promotion {
    soldier,
    rank,
    name
});
crate::save_fields!(BattleReport {
    tick,
    started,
    faction,
    captain,
    foes,
    at,
    fought,
    losses,
    kills,
    turning_point,
    promotions
});
crate::save_fields!(Chronicle {
    entries,
    reports,
    digest,
    last_notable
});
crate::save_fields!(FeedEntry { seq, tick, event });
crate::save_fields!(EventFeed { entries, next_seq });

/// Records one event at `tick`: always to the feed, and to the chronicle if
/// its kind is chronicled.
pub(crate) fn record(tick: u64, event: SimEvent, chronicle: &mut Chronicle, feed: &mut EventFeed) {
    feed.push(tick, event);
    if event.kind.chronicled() {
        chronicle.push(ChronicleEntry { tick, event });
    }
    if event.kind.notable() {
        chronicle.last_notable = tick;
    }
}

/// UI-05, UI-09: the single reader of [`SimEvent`]s. Sorts this tick's events
/// by [`SimEvent::sort_key`], so the result doesn't depend on which writer ran
/// first, stamps them with the tick and records them. Runs in
/// [`BookkeepingSet::Record`](crate::BookkeepingSet::Record).
pub fn record_events(
    tick: Res<SimTick>,
    mut events: MessageReader<SimEvent>,
    mut chronicle: ResMut<Chronicle>,
    mut feed: ResMut<EventFeed>,
) {
    let _span = tracing::info_span!("sim_core::record_events").entered();
    let mut batch: Vec<SimEvent> = events.read().copied().collect();
    if batch.is_empty() {
        return;
    }
    batch.sort_by_key(SimEvent::sort_key);
    for event in batch {
        record(tick.0, event, &mut chronicle, &mut feed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SimCorePlugin, SimStepper, compute_state_hash};
    use bevy_app::App;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(SimCorePlugin { seed: 1, run: None })
            .prepare_sim();
        app
    }

    /// UI-09: every cause has plain words of its own, with no variant name or
    /// id in them, so a summary line never prints `Player` or `Conquest { .. }`.
    #[test]
    fn every_cause_is_worded() {
        let f = FactionId::RIVAL;
        let causes = [
            Cause::None,
            Cause::Player,
            Cause::Request { id: 3 },
            Cause::Conquest { captain: 4 },
            Cause::Invitation,
            Cause::Starvation,
            Cause::Plunder,
            Cause::Revolt,
            Cause::Raid { captain: 5 },
            Cause::Hold,
            Cause::Boon { card: 2 },
            Cause::Death { faction: f },
            Cause::Lapsed,
            Cause::Neglect,
            Cause::Fight { faction: f },
            Cause::Sack { captain: 6 },
            Cause::Submission { captain: 7 },
            Cause::Subversion,
            Cause::Defiance,
            Cause::Fallout { settlement: 8 },
            Cause::Reprisal { settlement: 9 },
            Cause::Edict,
            Cause::Caught,
            Cause::Deposed,
        ];
        for (i, c) in causes.iter().enumerate() {
            assert_eq!(c.tag() as usize, i, "tags are positions: {}", c.describe());
        }
        let mut words: Vec<&str> = causes.iter().map(|c| c.describe()).collect();
        assert!(
            words
                .iter()
                .all(|w| !w.is_empty() && !w.contains(['{', '}']))
        );
        assert_eq!(Cause::Player.describe(), "by your choice");
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), causes.len(), "each cause reads differently");
    }

    fn events() -> [SimEvent; 4] {
        [
            SimEvent::new(SimEventKind::Battle, FactionId::PLAYER)
                .subject(9)
                .amount(3),
            SimEvent::new(SimEventKind::OfferPosted, FactionId::PLAYER).subject(0),
            SimEvent::new(SimEventKind::Conquest, FactionId::PLAYER)
                .subject(4)
                .cause(Cause::Conquest { captain: 9 }),
            SimEvent::new(SimEventKind::Conquest, FactionId::RAIDERS).subject(2),
        ]
    }

    /// UI-05, UI-09: a tick's events are recorded in a fixed order whatever
    /// order they were written in, stamped with the tick; the chronicle keeps
    /// only chronicled kinds and its hash doesn't depend on write order.
    #[test]
    fn events_are_recorded_in_sorted_order() {
        let mut forward = app();
        let mut backward = app();
        forward.step(3);
        backward.step(3);
        for e in events() {
            forward.world_mut().write_message(e);
        }
        for e in events().into_iter().rev() {
            backward.world_mut().write_message(e);
        }
        forward.step(2);
        backward.step(2);

        let feed = forward.world().resource::<EventFeed>();
        let kinds: Vec<_> = feed
            .iter()
            .map(|e| (e.seq, e.tick, e.event.kind, e.event.faction))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (0, 3, SimEventKind::OfferPosted, FactionId::PLAYER),
                (1, 3, SimEventKind::Conquest, FactionId::PLAYER),
                (2, 3, SimEventKind::Conquest, FactionId::RAIDERS),
                (3, 3, SimEventKind::Battle, FactionId::PLAYER),
            ],
            "events are sorted by kind, then faction, and recorded once"
        );
        assert_eq!(feed.since(2).count(), 2);

        let chronicle = forward.world().resource::<Chronicle>();
        assert_eq!(chronicle.len(), 3, "OfferPosted is feed-only");
        assert!(chronicle.entries().iter().all(|e| e.tick == 3));
        assert_eq!(chronicle.last_notable(), 3);
        assert_eq!(
            chronicle.entries()[0].event.cause,
            Cause::Conquest { captain: 9 }
        );

        let a = forward.world().resource::<Chronicle>().clone();
        let b = backward.world().resource::<Chronicle>().clone();
        assert_eq!(a, b, "write order doesn't change the chronicle");
        assert_ne!(a.digest(), 0);
        assert_eq!(
            compute_state_hash(forward.world_mut()),
            compute_state_hash(backward.world_mut())
        );
    }

    #[test]
    fn the_feed_keeps_the_most_recent_entries() {
        let mut feed = EventFeed::default();
        let e = SimEvent::new(SimEventKind::Battle, FactionId::PLAYER);
        for tick in 0..(EventFeed::CAPACITY as u64 + 10) {
            feed.push(tick, e);
        }
        assert_eq!(feed.len(), EventFeed::CAPACITY);
        assert_eq!(feed.iter().next().map(|e| e.seq), Some(10));
        assert_eq!(feed.next_seq(), EventFeed::CAPACITY as u64 + 10);
    }

    #[test]
    fn every_kind_is_listed_in_order() {
        for (i, kind) in SimEventKind::ALL.iter().enumerate() {
            assert_eq!(*kind as usize, i, "{}", kind.name());
        }
    }

    /// M4a (ADR 0020): every kind's classification, as a table. The kinds
    /// before M4a keep theirs; Forsaken, Built and StewardNamed are
    /// chronicled but quiet; EdictSet and Tribute are feed only; the other
    /// new kinds are notable.
    #[test]
    fn every_kind_is_classified() {
        use SimEventKind as K;
        // (kind, chronicled, quiet, notable)
        let table = [
            (K::RunStarted, true, false, true),
            (K::OfferPosted, false, false, false),
            (K::OfferChosen, true, false, true),
            (K::MonumentTaken, true, false, true),
            (K::MonumentLost, true, false, true),
            (K::RequestPosted, false, false, true),
            (K::RequestDone, true, false, true),
            (K::RequestFailed, true, false, true),
            (K::Conquest, true, false, true),
            (K::Invited, true, false, true),
            (K::Revolt, true, false, true),
            (K::RankUp, true, false, true),
            (K::TraitGained, true, false, true),
            (K::VeteranRank, false, false, false),
            (K::ChampionFell, true, false, true),
            (K::Succession, true, false, true),
            (K::RivalLevy, false, false, false),
            (K::RivalRaid, true, false, true),
            (K::RaidersArrived, true, false, true),
            (K::Battle, true, false, true),
            (K::DriftersJoined, false, false, false),
            (K::RunEnded, true, false, true),
            (K::Submitted, true, false, true),
            (K::Defied, true, false, true),
            (K::Subverted, true, false, true),
            (K::BanditsRose, true, false, true),
            (K::ReprisalVowed, true, false, true),
            (K::RivalReprisal, true, false, true),
            (K::AgentCaught, true, false, true),
            (K::Forsaken, true, true, false),
            (K::Built, true, true, false),
            (K::StewardNamed, true, true, false),
            (K::EdictSet, false, false, false),
            (K::Tribute, false, false, false),
        ];
        assert_eq!(table.len(), SimEventKind::ALL.len());
        for (i, (kind, chronicled, quiet, notable)) in table.into_iter().enumerate() {
            assert_eq!(kind as usize, i, "{}", kind.name());
            assert_eq!(kind.chronicled(), chronicled, "{}", kind.name());
            assert_eq!(kind.quiet(), quiet, "{}", kind.name());
            assert_eq!(kind.notable(), notable, "{}", kind.name());
        }
        let mut names: Vec<&str> = SimEventKind::ALL.iter().map(|k| k.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SimEventKind::ALL.len(), "names are distinct");
    }
}
