//! The chronicle in words, and the run summary (UI-05, UI-09; ADR 0017).
//!
//! The sim stores no text: [`Chronicle`] entries carry ids, numbers and a
//! [`Cause`]. [`describe`] turns an entry into a sentence for the event
//! feed, the chronicle and the end screen: the day, who did what to whom
//! (settlement, monument and people names from the `NameTable`, faction
//! names from army.ron), and why, worded by progression.ron's `chronicle`
//! phrases. A `Battle` entry is told from its [`BattleReport`]: the foes,
//! the losses on both sides, the turning point and the ranks earned, with
//! Elites named.
//!
//! [`RunSummary`] is the run's numbers for the end screen: how it ended,
//! days played, settlements and monuments taken and held, battles,
//! veterans. It is a pure function of the chronicle, `ArmyStats` and
//! `RunProgress` ([`RunSummary::of`]); [`run_summary`] reads them from a
//! world.

use std::collections::{BTreeMap, HashMap};

use bevy_ecs::prelude::*;
use bevy_math::IVec2;
use serde::{Deserialize, Serialize};
use sim_core::{
    BattleReport, Calendar, Cause, Chronicle, ChronicleEntry, FactionId, NameId, NameTable,
    OfferKind, RunOutcome, RunState, SimEventKind, SimTick, SpawnId,
};
use sim_world::{Monument, Settlement};

use crate::components::{ArmyStats, Soldier};
use crate::data::Army;
use crate::progression::{Progression, RunProgress};

/// How the chronicle words a [`Cause`] (progression.ron `chronicle.causes`,
/// UI-09). Each phrase follows the sentence after a comma; `{faction}`
/// (the event's faction), `{settlement}`, `{captain}`, `{boon}` and `{foe}`
/// are filled in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CausePhrases {
    /// `Cause::Player`.
    pub player: String,
    /// `Cause::Request`: `{settlement}` is the asking village.
    pub request: String,
    /// `Cause::Conquest`: `{captain}` led the attack.
    pub conquest: String,
    /// `Cause::Invitation`.
    pub invitation: String,
    /// `Cause::Starvation`.
    pub starvation: String,
    /// `Cause::Plunder`.
    pub plunder: String,
    /// `Cause::Revolt`.
    pub revolt: String,
    /// `Cause::Raid`: `{captain}` led the raid.
    pub raid: String,
    /// `Cause::Hold`.
    pub hold: String,
    /// `Cause::Boon`: `{boon}` is its name.
    pub boon: String,
    /// `Cause::Death`: `{foe}` is the killer's faction.
    pub death: String,
    /// `Cause::Lapsed`.
    pub lapsed: String,
    /// `Cause::Neglect`.
    pub neglect: String,
    /// `Cause::Fight`: `{foe}` is every faction fought.
    pub fight: String,
}

impl CausePhrases {
    /// The phrase for `cause` ("" for `Cause::None`).
    pub fn phrase(&self, cause: Cause) -> &str {
        match cause {
            Cause::None => "",
            Cause::Player => &self.player,
            Cause::Request { .. } => &self.request,
            Cause::Conquest { .. } => &self.conquest,
            Cause::Invitation => &self.invitation,
            Cause::Starvation => &self.starvation,
            Cause::Plunder => &self.plunder,
            Cause::Revolt => &self.revolt,
            Cause::Raid { .. } => &self.raid,
            Cause::Hold => &self.hold,
            Cause::Boon { .. } => &self.boon,
            Cause::Death { .. } => &self.death,
            Cause::Lapsed => &self.lapsed,
            Cause::Neglect => &self.neglect,
            Cause::Fight { .. } => &self.fight,
        }
    }

    fn all(&self) -> [&str; 14] {
        [
            &self.player,
            &self.request,
            &self.conquest,
            &self.invitation,
            &self.starvation,
            &self.plunder,
            &self.revolt,
            &self.raid,
            &self.hold,
            &self.boon,
            &self.death,
            &self.lapsed,
            &self.neglect,
            &self.fight,
        ]
    }
}

/// progression.ron `chronicle` (UI-09): the chronicle's wording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChronicleData {
    /// One phrase per cause.
    pub causes: CausePhrases,
}

impl ChronicleData {
    /// Placeholders a phrase may use.
    pub const PLACEHOLDERS: [&'static str; 5] =
        ["{faction}", "{settlement}", "{captain}", "{boon}", "{foe}"];

    /// Every phrase is non-empty and uses only [`Self::PLACEHOLDERS`].
    pub fn validate(&self) -> Result<(), String> {
        for p in self.causes.all() {
            if p.trim().is_empty() {
                return Err("progression: chronicle cause phrases must not be empty".into());
            }
            let mut rest = p;
            while let Some(i) = rest.find('{') {
                let end = rest[i..]
                    .find('}')
                    .ok_or_else(|| format!("progression: unclosed placeholder in {p:?}"))?;
                let name = &rest[i..i + end + 1];
                if !Self::PLACEHOLDERS.contains(&name) {
                    return Err(format!(
                        "progression: unknown placeholder {name} in chronicle phrase {p:?}"
                    ));
                }
                rest = &rest[i + end + 1..];
            }
        }
        Ok(())
    }
}

/// UI-09: one chronicle entry as a sentence, "Day 4: Your warband took
/// Oakworth from ..., taken by force under Brand." It names the cause
/// (progression.ron `chronicle.causes`). Works on any entry of the world's
/// [`Chronicle`] (or its `EventFeed`); for many entries use
/// [`describe_entries`], which looks names up once.
pub fn describe(world: &World, e: &ChronicleEntry) -> String {
    Teller::new(world).tell(e)
}

/// UI-09: [`describe`] for each of `entries`, in order.
pub fn describe_entries(world: &World, entries: &[ChronicleEntry]) -> Vec<String> {
    let teller = Teller::new(world);
    entries.iter().map(|e| teller.tell(e)).collect()
}

/// What a SpawnId names, for the chronicle.
#[derive(Clone, Copy)]
enum Named {
    Settlement(NameId),
    Monument(NameId),
    Person(NameId),
}

/// Looks names up once for a batch of sentences.
struct Teller<'w> {
    names: &'w NameTable,
    army: &'w Army,
    progression: &'w Progression,
    calendar: &'w Calendar,
    chronicle: &'w Chronicle,
    by_id: HashMap<u64, Named>,
    /// Settlement centres and names, by SpawnId, for "near ...".
    places: Vec<(u64, IVec2, NameId)>,
}

impl<'w> Teller<'w> {
    fn new(world: &'w World) -> Self {
        let mut by_id = HashMap::new();
        let mut places = Vec::new();
        if let Some(mut q) = world.try_query::<(&SpawnId, &Settlement)>() {
            for (id, s) in q.iter(world) {
                by_id.insert(id.0, Named::Settlement(s.name));
                places.push((id.0, s.center, s.name));
            }
        }
        places.sort_unstable_by_key(|p| p.0);
        if let Some(mut q) = world.try_query::<(&SpawnId, &Monument)>() {
            for (id, m) in q.iter(world) {
                by_id.insert(id.0, Named::Monument(m.name));
            }
        }
        if let Some(mut q) = world.try_query::<(&SpawnId, &sim_agents::Villager)>() {
            for (id, v) in q.iter(world) {
                by_id.insert(id.0, Named::Person(v.name));
            }
        }
        if let Some(mut q) = world.try_query::<(&SpawnId, &Soldier)>() {
            for (id, s) in q.iter(world) {
                by_id.insert(id.0, Named::Person(s.name));
            }
        }
        Self {
            names: world.resource::<NameTable>(),
            army: world.resource::<Army>(),
            progression: world.resource::<Progression>(),
            calendar: world.resource::<Calendar>(),
            chronicle: world.resource::<Chronicle>(),
            by_id,
            places,
        }
    }

    /// A faction's name inside a sentence: army.ron's, except "the
    /// villagers" for the independents and a lower-case "your" for the
    /// player (the sentence's first letter is raised in [`Self::tell`]).
    fn faction(&self, f: FactionId) -> String {
        let name = self.army.faction_name(f);
        if f == FactionId::INDEPENDENT {
            "the villagers".into()
        } else if f == FactionId::PLAYER {
            lower_first(name)
        } else {
            name.to_string()
        }
    }

    fn name(&self, id: u64, missing: &str) -> String {
        match self.by_id.get(&id) {
            Some(Named::Settlement(n) | Named::Monument(n) | Named::Person(n)) => {
                self.names.get(*n).to_string()
            }
            None => missing.to_string(),
        }
    }

    /// The settlement nearest `at` (ties: lowest SpawnId).
    fn near(&self, at: IVec2) -> String {
        self.places
            .iter()
            .min_by_key(|(id, c, _)| ((*c - at).length_squared(), *id))
            .map_or_else(|| "the wilds".into(), |(_, _, n)| self.names.get(*n).into())
    }

    fn boon(&self, card: u64) -> String {
        self.progression
            .data
            .boons
            .get(card as usize)
            .map_or_else(|| format!("#{card}"), |b| b.name.clone())
    }

    fn trait_name(&self, id: u64) -> (String, String) {
        self.progression.data.traits.get(id as usize).map_or_else(
            || (format!("#{id}"), String::new()),
            |t| (t.name.clone(), t.phrase.clone()),
        )
    }

    fn rank_name(&self, rank: i32) -> String {
        self.army.data.veterans.name(rank.max(0) as u8).to_string()
    }

    fn tell(&self, e: &ChronicleEntry) -> String {
        let ev = e.event;
        let day = self.calendar.at(e.tick).day + 1;
        let who = self.faction(ev.faction);
        let report = self.chronicle.report_for(e);
        let request_kind = |tag: i32| match tag {
            0 => "drive off raiders",
            1 => "bring food",
            _ => "clear the camp",
        };
        let what = match ev.kind {
            SimEventKind::RunStarted => format!(
                "{} set out with a band of {}, far from the rival's seat at {}",
                self.name(ev.subject, "the champion"),
                self.progression
                    .data
                    .bands
                    .get(ev.amount.max(0) as usize)
                    .map_or_else(|| "followers".into(), |b| lower_first(&b.name)),
                self.name(ev.other, "?")
            ),
            SimEventKind::OfferPosted => format!("{who} has a choice to make"),
            SimEventKind::OfferChosen => match ev.amount {
                t if t == OfferKind::StartBoon.tag() as i32 => {
                    format!("{who} chose the boon {}", self.boon(ev.other))
                }
                t if t == (OfferKind::DominionBoon { region: 0, step: 0 }).tag() as i32 => {
                    format!(
                        "{who} chose the boon {} for the land held",
                        self.boon(ev.other)
                    )
                }
                t if t == (OfferKind::Trait { person: 0 }).tag() as i32 => {
                    format!("{who} chose the trait {}", self.trait_name(ev.other).0)
                }
                t if t == (OfferKind::Invitation { settlement: 0 }).tag() as i32 => {
                    if ev.other == 0 {
                        format!("{who} accepted an invitation")
                    } else {
                        format!("{who} declined an invitation")
                    }
                }
                t if t
                    == (OfferKind::Hire {
                        captain: 0,
                        men: 0,
                        price: 0,
                    })
                    .tag() as i32 =>
                {
                    if ev.other == 0 {
                        format!("{who} hired a band of sellswords")
                    } else {
                        format!("{who} sent the sellswords away")
                    }
                }
                _ => format!("{who} answered an offer"),
            },
            SimEventKind::MonumentTaken => {
                let from = if ev.other == 0 {
                    String::new()
                } else {
                    format!(" from {}", self.faction(FactionId(ev.other as u16)))
                };
                format!("{who} took {}{from}", self.name(ev.subject, "a monument"))
            }
            SimEventKind::MonumentLost => {
                let m = self.name(ev.subject, "a monument");
                if ev.other == 0 {
                    format!("{m} slipped out of {who}'s hands")
                } else {
                    format!(
                        "{who} lost {m} to {}",
                        self.faction(FactionId(ev.other as u16))
                    )
                }
            }
            SimEventKind::RequestPosted => format!(
                "{} asked {who} to {}",
                self.name(ev.other, "a village"),
                request_kind(ev.amount)
            ),
            SimEventKind::RequestDone => format!(
                "{who} did as {} asked and {}",
                self.name(ev.other, "a village"),
                match ev.amount {
                    0 => "drove off the raiders",
                    1 => "brought food",
                    _ => "cleared the camp",
                }
            ),
            SimEventKind::RequestFailed => format!(
                "{} asked in vain to {}",
                self.name(ev.other, "a village"),
                request_kind(ev.amount)
            ),
            SimEventKind::Conquest => {
                let from = if ev.other == 0 {
                    String::new()
                } else {
                    format!(" from {}", self.faction(FactionId(ev.other as u16)))
                };
                format!(
                    "{who} took {}{from} ({} people)",
                    self.name(ev.subject, "a settlement"),
                    ev.amount
                )
            }
            SimEventKind::Invited => format!(
                "{} invited {who} in ({} people)",
                self.name(ev.subject, "a settlement"),
                ev.amount
            ),
            SimEventKind::Revolt => format!(
                "{} rose against {who} and is free again",
                self.name(ev.subject, "a settlement")
            ),
            SimEventKind::RankUp => format!(
                "{} of {who} rose to rank {} for their deeds",
                self.name(ev.subject, "the champion"),
                ev.amount
            ),
            SimEventKind::TraitGained => {
                let (name, phrase) = self.trait_name(ev.amount.max(0) as u64);
                let person = self.name(ev.subject, "the champion");
                if phrase.is_empty() {
                    format!("{person} became {name}")
                } else {
                    format!("{person} became {name}, {phrase}")
                }
            }
            SimEventKind::VeteranRank => format!(
                "{} of {who}'s soldiers became {}",
                ev.other,
                self.rank_name(ev.amount)
            ),
            SimEventKind::ChampionFell => format!(
                "{who}'s champion {} fell",
                self.names.get(NameId(ev.amount as u32))
            ),
            SimEventKind::Succession => format!(
                "{} took up the fallen champion's banner, keeping {} trait{}",
                self.name(ev.subject, "a captain"),
                ev.amount,
                if ev.amount == 1 { "" } else { "s" }
            ),
            SimEventKind::RivalLevy => {
                format!("{who} levied {} men at {}", ev.amount, self.near(ev.at))
            }
            SimEventKind::RivalRaid => format!(
                "{who} sent {} men raiding out of {}",
                ev.amount,
                self.near(ev.at)
            ),
            SimEventKind::RaidersArrived => {
                let target = if ev.other == 0 {
                    self.near(ev.at)
                } else {
                    self.name(ev.other, "a village")
                };
                format!("{} {who} came down on the roads near {target}", ev.amount)
            }
            SimEventKind::Battle => format!("{who} fought near {}", self.near(ev.at)),
            SimEventKind::DriftersJoined => format!(
                "{} drifters from {} joined {who}",
                ev.amount,
                self.name(ev.other, "a village")
            ),
            SimEventKind::RunEnded => match ev.subject {
                0 => format!("The run was won by {}", self.faction(FactionId::PLAYER)),
                1 => "The run was lost".to_string(),
                _ => "The run was abandoned".to_string(),
            },
        };
        // An answered offer names its chooser already.
        let why = if ev.kind == SimEventKind::OfferChosen {
            String::new()
        } else {
            self.cause(e, report)
        };
        let mut out = format!("Day {day}: {}", upper_first(&what));
        if !why.is_empty() {
            out.push_str(", ");
            out.push_str(&why);
        }
        if let Some(r) = report {
            out.push_str(&self.battle(r));
        }
        out.push('.');
        out
    }

    fn cause(&self, e: &ChronicleEntry, report: Option<&BattleReport>) -> String {
        let ev = e.event;
        let phrase = self
            .progression
            .data
            .chronicle
            .causes
            .phrase(ev.cause)
            .to_string();
        if phrase.is_empty() {
            return phrase;
        }
        let settlement = match ev.kind {
            SimEventKind::RequestDone
            | SimEventKind::RequestFailed
            | SimEventKind::RequestPosted => self.name(ev.other, "a village"),
            _ => self.name(ev.subject, "a village"),
        };
        let captain = match ev.cause {
            Cause::Conquest { captain } | Cause::Raid { captain } => {
                self.name(captain, "their captain")
            }
            _ => "their captain".into(),
        };
        let boon = match ev.cause {
            Cause::Boon { card } => self.boon(card as u64),
            _ => self.boon(ev.other),
        };
        let foe = match (report, ev.cause) {
            (Some(r), _) if !r.foes.is_empty() => {
                let foes: Vec<String> = r.foes.iter().map(|f| self.faction(*f)).collect();
                join_and(&foes)
            }
            (_, Cause::Death { faction } | Cause::Fight { faction }) => self.faction(faction),
            _ => "their foes".into(),
        };
        phrase
            .replace("{faction}", &self.faction(ev.faction))
            .replace("{settlement}", &settlement)
            .replace("{captain}", &captain)
            .replace("{boon}", &boon)
            .replace("{foe}", &foe)
    }

    /// A battle report's details: numbers, the turning point, ranks earned
    /// and the Elites by name.
    fn battle(&self, r: &BattleReport) -> String {
        let mut out = format!(
            ": {} fought, {} fell and {} slain",
            r.fought,
            r.losses,
            match r.kills {
                1 => "1 foe was".to_string(),
                n => format!("{n} foes were"),
            }
        );
        match r.turning_point {
            Some(t) => {
                let c = self.calendar.at(t);
                let (h, m) = c.hours_minutes();
                out.push_str(&format!(
                    "; the line broke on day {} at {h:02}:{m:02}",
                    c.day + 1
                ));
            }
            None => out.push_str("; the line held"),
        }
        if !r.promotions.is_empty() {
            out.push_str(&format!("; {} rose in rank", r.promotions.len()));
            let top = self.army.data.veterans.thresholds.len() as u8;
            let elites: Vec<String> = r
                .promotions
                .iter()
                .filter(|p| p.rank >= top && top > 0)
                .map(|p| self.names.get(p.name).to_string())
                .collect();
            if !elites.is_empty() {
                out.push_str(&format!(
                    ", and {} became {}",
                    join_and(&elites),
                    self.rank_name(top as i32)
                ));
            }
        }
        out
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_lowercase().chain(c).collect())
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

fn join_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// A run's numbers (UI-09), for the end screen and `sim_headless`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RunSummary {
    /// How the run ended (`None` while it goes on).
    pub outcome: Option<RunOutcome>,
    /// Why (`Cause::None` while it goes on).
    pub cause: Cause,
    /// Days played: to the end, or so far.
    pub days: f32,
    /// Settlements the player took by force or was invited into.
    pub settlements_taken: u32,
    /// Settlements the player lost (revolts, conquests by others).
    pub settlements_lost: u32,
    /// Settlements the player holds at the end (`taken - lost`).
    pub settlements_held: u32,
    /// People in the player's settlements at the last hourly tally.
    pub people_held: u32,
    /// Monuments the player took.
    pub monuments_taken: u32,
    /// Monuments the player holds at the end (`taken - lost`).
    pub monuments_held: u32,
    /// Battles the player's armies fought.
    pub battles: u32,
    /// Foes the player's armies killed.
    pub kills: u32,
    /// The player's soldiers lost.
    pub lost: u32,
    /// The player's soldiers who rose in veteran rank at least once.
    pub veterans: u32,
    /// The player's soldiers who reached the top rank (Elites).
    pub elites: u32,
    /// Village requests the player answered.
    pub requests_done: u32,
    /// Boons picked.
    pub boons: u32,
}

impl RunSummary {
    /// The summary of a run from its chronicle, the army totals and the
    /// run's progress (a pure function). `ticks_per_day` and `now` (the
    /// current tick) give the days played while the run goes on;
    /// `top_rank` is army.ron's highest veteran rank (Elite).
    pub fn of(
        chronicle: &Chronicle,
        stats: &ArmyStats,
        progress: &RunProgress,
        run: &RunState,
        ticks_per_day: u64,
        now: u64,
        top_rank: u8,
    ) -> Self {
        let player = FactionId::PLAYER;
        let mut s = RunSummary {
            outcome: run.outcome,
            cause: run.cause,
            days: run.ended_at.map_or(now, |t| t + 1) as f32 / ticks_per_day.max(1) as f32,
            people_held: progress.dominion.iter().map(|t| t.held_by_player).sum(),
            kills: stats.get(player).kills,
            lost: stats.get(player).lost,
            boons: progress.boons.len() as u32,
            ..Default::default()
        };
        let mut monuments_lost = 0u32;
        for e in chronicle.entries() {
            let ev = e.event;
            let mine = ev.faction == player;
            let from_me = ev.other == player.0 as u64;
            match ev.kind {
                SimEventKind::Conquest | SimEventKind::Invited if mine => s.settlements_taken += 1,
                SimEventKind::Conquest | SimEventKind::Invited if from_me => {
                    s.settlements_lost += 1
                }
                SimEventKind::Revolt if mine => s.settlements_lost += 1,
                SimEventKind::MonumentTaken if mine => s.monuments_taken += 1,
                SimEventKind::MonumentLost if mine => monuments_lost += 1,
                SimEventKind::Battle if mine => s.battles += 1,
                SimEventKind::RequestDone if mine => s.requests_done += 1,
                _ => {}
            }
        }
        s.settlements_held = s.settlements_taken.saturating_sub(s.settlements_lost);
        s.monuments_held = s.monuments_taken.saturating_sub(monuments_lost);
        let mut best: BTreeMap<u64, u8> = BTreeMap::new();
        for r in chronicle.reports().iter().filter(|r| r.faction == player) {
            for p in &r.promotions {
                let b = best.entry(p.soldier).or_default();
                *b = (*b).max(p.rank);
            }
        }
        s.veterans = best.len() as u32;
        s.elites = best
            .values()
            .filter(|r| top_rank > 0 && **r >= top_rank)
            .count() as u32;
        s
    }

    /// The summary as lines of text (sim_headless, logs).
    pub fn lines(&self) -> Vec<String> {
        let ended = match self.outcome {
            Some(o) => format!("{} ({})", o.name(), self.cause.describe()),
            None => "in progress".into(),
        };
        vec![
            format!("run {ended} after {:.1} days", self.days),
            format!(
                "settlements: {} taken, {} lost, {} held ({} people)",
                self.settlements_taken,
                self.settlements_lost,
                self.settlements_held,
                self.people_held
            ),
            format!(
                "monuments: {} taken, {} held; requests answered: {}; boons: {}",
                self.monuments_taken, self.monuments_held, self.requests_done, self.boons
            ),
            format!(
                "battles: {}, {} foes slain, {} soldiers lost; veterans {}, elites {}",
                self.battles, self.kills, self.lost, self.veterans, self.elites
            ),
        ]
    }
}

/// UI-09: [`RunSummary::of`] for the world's run so far.
pub fn run_summary(world: &World) -> RunSummary {
    RunSummary::of(
        world.resource::<Chronicle>(),
        world.resource::<ArmyStats>(),
        world.resource::<RunProgress>(),
        world.resource::<RunState>(),
        world.resource::<Calendar>().ticks_per_day,
        world.resource::<SimTick>().0,
        world.resource::<Army>().data.veterans.thresholds.len() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progression::tests::run_app;
    use sim_core::{Promotion, SimEvent};

    fn entry(tick: u64, event: SimEvent) -> ChronicleEntry {
        ChronicleEntry { tick, event }
    }

    /// UI-09: sentences carry the day, the names and the cause, worded by
    /// progression.ron; a battle is told from its report, Elites by name.
    #[test]
    fn entries_read_as_sentences_with_their_causes() {
        let (mut app, start) = run_app(42);
        let world = app.world_mut();
        let mut q = world.query::<(&SpawnId, &Settlement)>();
        let (sid, sname) = q
            .iter(world)
            .map(|(id, s)| (id.0, s.name))
            .min()
            .expect("a settlement");
        let mut q = world.query::<(&SpawnId, &Monument)>();
        let (mid, mname) = q
            .iter(world)
            .map(|(id, m)| (id.0, m.name))
            .min()
            .expect("a monument");
        let champion = world.get::<SpawnId>(start.champion).unwrap().0;
        let names = world.resource::<NameTable>();
        let (sname, mname) = (names.get(sname).to_string(), names.get(mname).to_string());
        let champ_name = names
            .get(world.get::<Soldier>(start.champion).unwrap().name)
            .to_string();
        let day2 = world.resource::<Calendar>().ticks_per_day + 5;

        let taken = entry(
            day2,
            SimEvent::new(SimEventKind::Conquest, FactionId::PLAYER)
                .subject(sid)
                .amount(40)
                .cause(Cause::Conquest { captain: champion }),
        );
        let text = describe(world, &taken);
        println!("{text}");
        assert!(text.starts_with("Day 2: Your warband took "), "{text}");
        assert!(text.contains(&format!("{sname} (40 people)")), "{text}");
        assert!(
            text.contains(&sname) && text.contains(&champ_name),
            "{text}"
        );
        assert!(text.contains("taken by force"), "{text}");

        let lost = entry(
            7,
            SimEvent::new(SimEventKind::MonumentLost, FactionId::PLAYER)
                .subject(mid)
                .cause(Cause::Neglect),
        );
        let text = describe(world, &lost);
        assert!(
            text.contains(&mname) && text.contains("unwatched"),
            "{text}"
        );

        // A battle with its report: foes, numbers, turning point, Elites.
        let elite = world.resource::<Army>().data.veterans.thresholds.len() as u8;
        let champ_name_id = world.get::<Soldier>(start.champion).unwrap().name;
        world.resource_mut::<Chronicle>().push_report(BattleReport {
            tick: 9,
            started: 2,
            faction: FactionId::PLAYER,
            captain: champion,
            foes: vec![FactionId::RAIDERS, FactionId::BEASTS],
            at: IVec2::ZERO,
            fought: 7,
            losses: 2,
            kills: 5,
            turning_point: None,
            promotions: vec![Promotion {
                soldier: champion,
                rank: elite,
                name: champ_name_id,
            }],
        });
        let battle = entry(
            9,
            SimEvent::new(SimEventKind::Battle, FactionId::PLAYER)
                .subject(champion)
                .other(7)
                .amount(2)
                .cause(Cause::Fight {
                    faction: FactionId::RAIDERS,
                }),
        );
        let text = describe_entries(world, &[battle]).remove(0);
        println!("{text}");
        assert!(text.contains("against Raiders and Beasts"), "{text}");
        assert!(
            text.contains("7 fought, 2 fell and 5 foes were slain"),
            "{text}"
        );
        assert!(text.contains("the line held"), "{text}");
        assert!(
            text.contains(&format!("{champ_name} became Elite")),
            "{text}"
        );

        // GP-27: a band for hire, hired or sent away.
        let hire = OfferKind::Hire {
            captain: 0,
            men: 0,
            price: 0,
        }
        .tag() as i32;
        let answered = |choice: u64| {
            entry(
                11,
                SimEvent::new(SimEventKind::OfferChosen, FactionId::PLAYER)
                    .subject(1)
                    .other(choice)
                    .amount(hire)
                    .cause(Cause::Player),
            )
        };
        let text = describe(world, &answered(0));
        assert_eq!(text, "Day 1: Your warband hired a band of sellswords.");
        let text = describe(world, &answered(1));
        assert_eq!(text, "Day 1: Your warband sent the sellswords away.");
    }

    /// UI-05: the military log numbers its lines, so the feed can read
    /// new ones by cursor after the oldest have dropped off.
    #[test]
    fn the_log_numbers_its_lines() {
        let mut log = crate::MilitaryLog::default();
        for i in 0..70 {
            log.push(i, format!("line {i}"));
        }
        assert_eq!(log.seq, 70);
        let all: Vec<_> = log.since(0).collect();
        assert_eq!(all.len(), 64);
        assert_eq!(all[0], (6, 6, "line 6"));
        assert_eq!(log.since(69).map(|l| l.0).collect::<Vec<_>>(), vec![69]);
        assert_eq!(log.since(70).count(), 0);
    }

    /// UI-09: the summary counts what the chronicle and the totals say.
    #[test]
    fn the_summary_counts_the_run() {
        let mut chronicle = Chronicle::default();
        let me = FactionId::PLAYER;
        let mut push = |tick, e: SimEvent| chronicle.push(ChronicleEntry { tick, event: e });
        push(10, SimEvent::new(SimEventKind::Conquest, me).subject(1));
        push(20, SimEvent::new(SimEventKind::Invited, me).subject(2));
        push(
            30,
            SimEvent::new(SimEventKind::Revolt, me)
                .subject(1)
                .cause(Cause::Revolt),
        );
        push(
            40,
            SimEvent::new(SimEventKind::Conquest, FactionId::RIVAL).subject(5),
        );
        push(
            50,
            SimEvent::new(SimEventKind::MonumentTaken, me).subject(9),
        );
        push(
            60,
            SimEvent::new(SimEventKind::MonumentTaken, me).subject(8),
        );
        push(70, SimEvent::new(SimEventKind::MonumentLost, me).subject(9));
        push(80, SimEvent::new(SimEventKind::Battle, me).subject(3));
        push(
            90,
            SimEvent::new(SimEventKind::Battle, FactionId::RAIDERS).subject(4),
        );
        push(95, SimEvent::new(SimEventKind::RequestDone, me).subject(0));
        let promo = |soldier, rank| Promotion {
            soldier,
            rank,
            name: NameId(0),
        };
        for (faction, promotions) in [
            (me, vec![promo(11, 1), promo(12, 1)]),
            (me, vec![promo(11, 2), promo(12, 3)]),
            (FactionId::RAIDERS, vec![promo(13, 3)]),
        ] {
            chronicle.push_report(BattleReport {
                tick: 80,
                started: 70,
                faction,
                captain: 3,
                foes: vec![],
                at: IVec2::ZERO,
                fought: 2,
                losses: 0,
                kills: 1,
                turning_point: None,
                promotions,
            });
        }
        let mut stats = ArmyStats::default();
        stats.of(me).kills = 6;
        stats.of(me).lost = 1;
        let progress = RunProgress {
            boons: vec![3, 7],
            ..Default::default()
        };
        let run = RunState {
            outcome: Some(RunOutcome::Won),
            ended_at: Some(4799),
            cause: Cause::Conquest { captain: 3 },
            ..Default::default()
        };
        let s = RunSummary::of(&chronicle, &stats, &progress, &run, 2400, 9999, 3);
        assert_eq!(s.outcome, Some(RunOutcome::Won));
        assert_eq!(s.days, 2.0);
        assert_eq!(
            (s.settlements_taken, s.settlements_lost, s.settlements_held),
            (2, 1, 1)
        );
        assert_eq!((s.monuments_taken, s.monuments_held), (2, 1));
        assert_eq!((s.battles, s.kills, s.lost), (1, 6, 1));
        assert_eq!(
            (s.veterans, s.elites),
            (2, 1),
            "Raiders' Elites aren't ours"
        );
        assert_eq!((s.requests_done, s.boons), (1, 2));
        assert_eq!(s.lines().len(), 4);
        assert_eq!(
            s.lines()[0],
            "run won (conquest) after 2.0 days",
            "the cause is worded, not printed as a variant"
        );
    }
}
