//! Runs (GP-21): the run's configuration, its outcome and the gate that stops
//! the clock when it ends, plus the queue of offers waiting for the player
//! (GP-27: boons, traits, invitations).
//!
//! - [`RunState`] is always present. Its `config` is `Some` only in a run (the
//!   `run` world preset); the developed world and the M2/M3 scenarios have
//!   `None`, and nothing here changes how they play.
//! - A serial step ends a run with [`RunState::finish`], which only records a
//!   pending outcome. [`settle_outcome`] makes it final at the end of that
//!   tick, so the ending tick completes normally; from the next tick every
//!   [`SimSet`](crate::SimSet) is gated off by [`run_in_progress`], so the tick
//!   counter and the state hash freeze.
//! - [`Offers`] holds the pending offers. sim_military posts them (with
//!   [`post_offer`]) and resolves `ChooseOffer` commands against them.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::components::FactionId;
use crate::events::{Cause, Chronicle, EventFeed, SimEvent, SimEventKind, record};
use crate::hash::{StateHash, StateHasher};
use crate::schedule::SimTick;

/// How hard a run is: it picks the pacing director's personality and the
/// rival's clock (GP-16, GP-28). Data rows are chosen by difficulty; RNG
/// streams never depend on it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum Difficulty {
    /// Events spaced evenly; the default.
    #[default]
    Steady,
    /// Events bunch up and quiet stretches are longer.
    Chaotic,
    /// More pressure, earlier.
    Harsh,
}

impl Difficulty {
    /// Every difficulty.
    pub const ALL: [Difficulty; 3] = [Difficulty::Steady, Difficulty::Chaotic, Difficulty::Harsh];

    /// Lower-case name, as on the command line (`--difficulty`).
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Steady => "steady",
            Difficulty::Chaotic => "chaotic",
            Difficulty::Harsh => "harsh",
        }
    }

    /// Parses a [`Difficulty::name`].
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == s)
    }
}

/// Everything besides the seed that sets up a run (GP-21). Stored in replay
/// headers and (M4 step 8) save headers. Must stay `Copy` (the headless
/// config and scenarios are `Copy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RunConfig {
    /// The director's personality and the rival's clock.
    pub difficulty: Difficulty,
}

impl StateHash for RunConfig {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.difficulty as u8);
    }
}

/// How a run ended (GP-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RunOutcome {
    /// The rival's seat changed owner to the player.
    Won,
    /// The champion died with no successor.
    Lost,
    /// The player gave up.
    Abandoned,
}

impl RunOutcome {
    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            RunOutcome::Won => "won",
            RunOutcome::Lost => "lost",
            RunOutcome::Abandoned => "abandoned",
        }
    }
}

/// The run in progress, if any (GP-21). Always present, hashed.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct RunState {
    /// `Some` in a run; `None` in the developed world and M2/M3 scenarios.
    pub config: Option<RunConfig>,
    /// An outcome recorded this tick by [`RunState::finish`], made final by
    /// [`settle_outcome`] at the end of the tick.
    pub pending: Option<(RunOutcome, Cause)>,
    /// The final outcome. Once set, the simulation stops advancing.
    pub outcome: Option<RunOutcome>,
    /// Why the run ended ([`Cause::None`] while it goes on).
    pub cause: Cause,
    /// The tick the run ended on.
    pub ended_at: Option<u64>,
}

impl RunState {
    /// Whether this is a run (rather than the developed world).
    pub fn in_run(&self) -> bool {
        self.config.is_some()
    }

    /// Whether the run has ended or will at the end of this tick.
    pub fn is_over(&self) -> bool {
        self.outcome.is_some() || self.pending.is_some()
    }

    /// Ends the run with `outcome` at the end of this tick. The first caller
    /// in a tick wins; returns false (and changes nothing) if a later caller,
    /// if the run is already over, or outside a run.
    pub fn finish(&mut self, outcome: RunOutcome, cause: Cause) -> bool {
        if !self.in_run() || self.is_over() {
            return false;
        }
        self.pending = Some((outcome, cause));
        true
    }
}

fn hash_outcome(h: &mut StateHasher, outcome: Option<RunOutcome>) {
    match outcome {
        Some(o) => {
            h.u8(1);
            h.u8(o as u8);
        }
        None => h.u8(0),
    }
}

impl StateHash for RunState {
    fn hash_state(&self, h: &mut StateHasher) {
        match self.config {
            Some(c) => {
                h.u8(1);
                c.hash_state(h);
            }
            None => h.u8(0),
        }
        match self.pending {
            Some((o, cause)) => {
                h.u8(1);
                h.u8(o as u8);
                cause.hash_state(h);
            }
            None => h.u8(0),
        }
        hash_outcome(h, self.outcome);
        self.cause.hash_state(h);
        match self.ended_at {
            Some(t) => {
                h.u8(1);
                h.u64(t);
            }
            None => h.u8(0),
        }
    }
}

crate::save_serde!(RunConfig, RunOutcome, OfferKind, Offer);
crate::save_fields!(RunState {
    config,
    pending,
    outcome,
    cause,
    ended_at
});

/// Run condition: the simulation advances (no run, or a run that hasn't
/// ended). Gates every [`SimSet`](crate::SimSet).
pub fn run_in_progress(state: Option<Res<RunState>>) -> bool {
    state.is_none_or(|s| s.outcome.is_none())
}

/// Run condition: this is a run (the `run` preset), not the developed world.
/// Gates run-only systems (M4_PLAN default 6).
pub fn in_run(state: Option<Res<RunState>>) -> bool {
    state.is_some_and(|s| s.config.is_some())
}

/// Run condition: [`RunState::finish`] was called this tick.
pub(crate) fn outcome_pending(state: Res<RunState>) -> bool {
    state.pending.is_some()
}

/// GP-21: makes a pending outcome final, records `ended_at` and the
/// [`SimEventKind::RunEnded`] event (always the last entry of the run).
/// Runs in [`BookkeepingSet::Record`](crate::BookkeepingSet::Record) after
/// `record_events`, so the ending tick completes and every later tick is
/// gated off.
pub fn settle_outcome(
    tick: Res<SimTick>,
    mut state: ResMut<RunState>,
    mut chronicle: ResMut<Chronicle>,
    mut feed: ResMut<EventFeed>,
) {
    let _span = tracing::info_span!("sim_core::settle_outcome").entered();
    let Some((outcome, cause)) = state.pending.take() else {
        return;
    };
    state.outcome = Some(outcome);
    state.cause = cause;
    state.ended_at = Some(tick.0);
    let event = SimEvent::new(SimEventKind::RunEnded, FactionId::PLAYER)
        .subject(outcome as u64)
        .cause(cause);
    record(tick.0, event, &mut chronicle, &mut feed);
}

/// What an offer is for (GP-27). Append only (saves store the tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OfferKind {
    /// The starting boon, before the first tick. Choices index the boon deck.
    StartBoon,
    /// Holding 25% (`step` 0) or 50% (`step` 1) of `region` (GP-24).
    /// Choices index the boon deck.
    DominionBoon { region: u8, step: u8 },
    /// A champion rank-up (SIM-35): `person` picks a trait. Choices index the
    /// trait list.
    Trait { person: u64 },
    /// `settlement` offers lordship (GP-22). Choices are `[0 = accept, 1 = decline]`.
    Invitation { settlement: u64 },
    /// A band for hire led by `captain`. Choices are `[0 = hire, 1 = decline]`.
    Hire { captain: u64 },
}

impl OfferKind {
    /// Stable tag of the variant (saves, hashing).
    pub fn tag(self) -> u8 {
        match self {
            OfferKind::StartBoon => 0,
            OfferKind::DominionBoon { .. } => 1,
            OfferKind::Trait { .. } => 2,
            OfferKind::Invitation { .. } => 3,
            OfferKind::Hire { .. } => 4,
        }
    }

    /// What the offer concerns, as a number: the region, person, settlement
    /// or captain (0 for the start boon). Goes into `OfferPosted` events.
    pub fn subject(self) -> u64 {
        match self {
            OfferKind::StartBoon => 0,
            OfferKind::DominionBoon { region, .. } => region as u64,
            OfferKind::Trait { person } => person,
            OfferKind::Invitation { settlement } => settlement,
            OfferKind::Hire { captain } => captain,
        }
    }
}

impl StateHash for OfferKind {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u8(self.tag());
        match *self {
            OfferKind::StartBoon => {}
            OfferKind::DominionBoon { region, step } => {
                h.u8(region);
                h.u8(step);
            }
            OfferKind::Trait { person } => h.u64(person),
            OfferKind::Invitation { settlement } => h.u64(settlement),
            OfferKind::Hire { captain } => h.u64(captain),
        }
    }
}

/// A choice waiting for a faction (GP-27). Answered by a recorded
/// `ChooseOffer` command, so picks replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    /// Unique per run, increasing.
    pub id: u64,
    /// Who may answer it.
    pub faction: FactionId,
    /// What it is for.
    pub kind: OfferKind,
    /// The options, as indices into the deck for `kind` (progression.ron).
    pub choices: Vec<u16>,
    /// The tick it was posted.
    pub posted: u64,
    /// The tick it lapses (invitations); `None` waits forever.
    pub expires: Option<u64>,
}

impl StateHash for Offer {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.id);
        h.u16(self.faction.0);
        self.kind.hash_state(h);
        h.u32(self.choices.len() as u32);
        for &c in &self.choices {
            h.u16(c);
        }
        h.u64(self.posted);
        match self.expires {
            Some(t) => {
                h.u8(1);
                h.u64(t);
            }
            None => h.u8(0),
        }
    }
}

/// Offers waiting for an answer, oldest first (GP-27). Always present, hashed.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct Offers {
    /// The id the next offer gets.
    pub next_id: u64,
    /// Unanswered offers, in id order.
    pub pending: Vec<Offer>,
}

impl Offers {
    /// Queues an offer posted at `tick` and returns its id. Emits no event:
    /// exclusive code should call [`post_offer`], which also reports it.
    pub fn post(
        &mut self,
        faction: FactionId,
        kind: OfferKind,
        choices: Vec<u16>,
        tick: u64,
        expires: Option<u64>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.push(Offer {
            id,
            faction,
            kind,
            choices,
            posted: tick,
            expires,
        });
        id
    }

    /// The pending offer `id`.
    pub fn get(&self, id: u64) -> Option<&Offer> {
        self.pending.iter().find(|o| o.id == id)
    }

    /// Removes and returns the pending offer `id` (answering it). A second
    /// answer to the same offer finds nothing.
    pub fn take(&mut self, id: u64) -> Option<Offer> {
        let i = self.pending.iter().position(|o| o.id == id)?;
        Some(self.pending.remove(i))
    }

    /// Pending offers for `faction`, oldest first.
    pub fn for_faction(&self, faction: FactionId) -> impl Iterator<Item = &Offer> + '_ {
        self.pending.iter().filter(move |o| o.faction == faction)
    }

    /// Removes and returns the offers whose `expires` is at or before `tick`.
    pub fn expire(&mut self, tick: u64) -> Vec<Offer> {
        let (gone, kept) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|o| o.expires.is_some_and(|t| t <= tick));
        self.pending = kept;
        gone
    }
}

crate::save_fields!(Offers { next_id, pending });

impl StateHash for Offers {
    fn hash_state(&self, h: &mut StateHasher) {
        h.u64(self.next_id);
        h.u32(self.pending.len() as u32);
        for o in &self.pending {
            o.hash_state(h);
        }
    }
}

/// GP-27: posts an offer at the current tick and reports it with an
/// [`SimEventKind::OfferPosted`] event. For exclusive steps and setup code
/// (before the first tick, the event is recorded at tick 0). Returns the id.
pub fn post_offer(
    world: &mut World,
    faction: FactionId,
    kind: OfferKind,
    choices: Vec<u16>,
    expires: Option<u64>,
) -> u64 {
    let tick = world.resource::<SimTick>().0;
    let count = choices.len() as i32;
    let id = world
        .resource_mut::<Offers>()
        .post(faction, kind, choices, tick, expires);
    world.write_message(
        SimEvent::new(SimEventKind::OfferPosted, faction)
            .subject(id)
            .other(kind.subject())
            .amount(count),
    );
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_is_first_caller_wins_and_only_in_a_run() {
        let mut s = RunState::default();
        assert!(!s.finish(RunOutcome::Lost, Cause::None), "not a run");
        assert_eq!(s.pending, None);

        s.config = Some(RunConfig::default());
        assert!(s.finish(RunOutcome::Won, Cause::Hold));
        assert!(!s.finish(RunOutcome::Lost, Cause::None));
        assert_eq!(s.pending, Some((RunOutcome::Won, Cause::Hold)));
        assert!(s.is_over());
    }

    #[test]
    fn offers_are_taken_once_and_expire() {
        let mut o = Offers::default();
        let a = o.post(
            FactionId::PLAYER,
            OfferKind::StartBoon,
            vec![3, 1, 4],
            0,
            None,
        );
        let b = o.post(
            FactionId::PLAYER,
            OfferKind::Invitation { settlement: 7 },
            vec![0, 1],
            5,
            Some(100),
        );
        assert_eq!((a, b), (0, 1));
        assert_eq!(o.for_faction(FactionId::PLAYER).count(), 2);
        assert_eq!(o.for_faction(FactionId::RIVAL).count(), 0);
        assert_eq!(o.take(a).map(|x| x.choices), Some(vec![3, 1, 4]));
        assert_eq!(o.take(a), None, "a second answer finds nothing");
        assert!(o.expire(99).is_empty());
        assert_eq!(o.expire(100).len(), 1);
        assert!(o.pending.is_empty());
        assert_eq!(o.next_id, 2, "ids are never reused");
    }

    /// GP-27: an offer posted before the first tick is reported at tick 0,
    /// in the feed only.
    #[test]
    fn posting_an_offer_reports_it() {
        use crate::{SimCorePlugin, SimStepper};
        let mut app = bevy_app::App::new();
        app.add_plugins(SimCorePlugin {
            seed: 1,
            run: Some(RunConfig::default()),
        })
        .prepare_sim();
        let id = post_offer(
            app.world_mut(),
            FactionId::PLAYER,
            OfferKind::StartBoon,
            vec![0, 2, 5],
            None,
        );
        app.step(1);
        let feed = app.world().resource::<EventFeed>();
        let e = feed.iter().next().expect("one event").event;
        assert_eq!(
            (e.kind, e.subject, e.amount),
            (SimEventKind::OfferPosted, id, 3)
        );
        assert!(app.world().resource::<Chronicle>().is_empty());
        assert_eq!(app.world().resource::<Offers>().get(id).unwrap().posted, 0);
    }

    #[test]
    fn difficulty_names_round_trip() {
        for d in Difficulty::ALL {
            assert_eq!(Difficulty::parse(d.name()), Some(d));
        }
        assert_eq!(Difficulty::parse("easy"), None);
    }
}
