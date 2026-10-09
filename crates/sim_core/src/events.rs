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
    /// picked (its index in the deck); `amount`: the choice.
    OfferChosen,
    /// A monument changed to `faction` (GP-23). `subject`: the monument;
    /// `other`: the previous holder's faction id (0 for nobody); `at`: the
    /// monument; `amount`: its kind (`MonumentKind` index).
    MonumentTaken,
    /// `faction` lost a monument. `subject`: the monument; `other`: the
    /// faction that took it (0 when it drifted back to nobody); `at`: the
    /// monument; `amount`: its kind.
    MonumentLost,
    /// A settlement asks the player for help (GP-22). `subject`: the
    /// request id; `other`: the settlement; `at`: its centre; `amount`: the
    /// request kind's tag (0 drive off, 1 bring food, 2 clear the camp).
    RequestPosted,
    /// A request was completed (cause `Cause::Request`). `subject`: the
    /// request id; `other`: the settlement; `at`: its centre; `amount`: the
    /// kind's tag.
    RequestDone,
    /// A request expired, or its danger passed without the player.
    /// `subject`: the request id; `other`: the settlement; `at`: its
    /// centre; `amount`: the kind's tag.
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
    /// its rank points; `amount`: the rank.
    RankUp,
    /// Someone gained a trait (SIM-34). `subject`: the person; `amount`: the trait id.
    TraitGained,
    /// A soldier reached a veteran rank (SIM-36). `subject`: the soldier;
    /// `at`: its army's captain's tile; `amount`: the rank.
    VeteranRank,
    /// The champion died (SIM-35). `subject`: the champion; `at`: where;
    /// `cause`: `Death` (the striker's faction) or `Starvation`.
    ChampionFell,
    /// A captain became champion (SIM-35). `subject`: the new champion;
    /// `other`: the fallen one; `at`: the new champion's tile; `amount`:
    /// traits kept; `cause`: the fallen's.
    Succession,
    /// The rival enlisted men at its seat (GP-28). `amount`: how many.
    RivalLevy,
    /// The rival sent a raiding band. `subject`: its captain.
    RivalRaid,
    /// The pacing director brought raiders (GP-16). `subject`: their captain.
    RaidersArrived,
    /// An army's battle ended (SIM-36). `faction`: the army's; `subject`:
    /// its captain; `other`: the survivors who fought; `at`: the captain's
    /// tile; `amount`: the army's dead.
    Battle,
    /// Volunteers joined a band (GP-22). `subject`: the captain; `other`:
    /// the settlement they came from; `at`: the captain's tile; `amount`:
    /// how many.
    DriftersJoined,
    /// The run ended. `subject`: the [`RunOutcome`](crate::RunOutcome) as a number.
    RunEnded,
}

impl SimEventKind {
    /// Every kind, in discriminant order.
    pub const ALL: [SimEventKind; 22] = [
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
        }
    }

    /// Whether events of this kind go into the [`Chronicle`] (UI-09). The
    /// rest (offers posted, requests posted, levies, single soldiers' ranks,
    /// drifters) are feed-only.
    pub fn chronicled(self) -> bool {
        !matches!(
            self,
            SimEventKind::OfferPosted
                | SimEventKind::RequestPosted
                | SimEventKind::VeteranRank
                | SimEventKind::RivalLevy
                | SimEventKind::DriftersJoined
        )
    }

    /// Whether this kind resets the pacing director's quiet clock
    /// ([`Chronicle::last_notable`], GP-16): every chronicled kind, plus a
    /// posted request (something for the player to do).
    pub fn notable(self) -> bool {
        self.chronicled() || self == SimEventKind::RequestPosted
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
        }
    }

    /// The variant's payload as a number (0 for none).
    pub fn payload(self) -> u64 {
        match self {
            Cause::Request { id } => id,
            Cause::Conquest { captain } | Cause::Raid { captain } => captain,
            Cause::Boon { card } => card as u64,
            Cause::Death { faction } => faction.0 as u64,
            Cause::None
            | Cause::Player
            | Cause::Invitation
            | Cause::Starvation
            | Cause::Plunder
            | Cause::Revolt
            | Cause::Hold => 0,
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

/// The run's history (UI-09): every chronicled event in recording order.
/// Append only. Hashed (as its length, a running digest of every entry and
/// the last notable tick, so hashing is O(1)) and saved.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Chronicle {
    entries: Vec<ChronicleEntry>,
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
}
