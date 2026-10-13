//! Taking towns (UI-11, GP-32; M4a step 5): what the player sees of the
//! three ways to take a town that need no new surface of their own. The
//! right-click menu's "Demand submission" and "Gift food" with their
//! forecasts, the settlement panel's "Ways to take it" block, a submitted
//! town's watch, an invited town's bond, the invitation card's sworn
//! words, and the label marks all read one UI-side [`TakeView`].
//!
//! [`TakeView`] is refreshed by one exclusive system ([`refresh`]) for the
//! few towns anything is showing (the selected one, the one under the
//! open menu, the pending invitations' and the towns of yours that
//! knelt), twice a second or when the selection changes: the sim's
//! `parley`, `watch` and `sworn_terms` read the whole world and so cannot
//! run in an ordinary system. The words are pure functions of [`Facts`]
//! (tested without a world); nothing here writes simulation state, and the
//! orders it enables go out as `SimCommand`s from `army.rs`.

use std::collections::BTreeMap;

use bevy::prelude::*;
use sim_agents::{
    Deed, DeedLog, FoodSecurity, Goodwill, Owner, SettlementLoyalty, Villagers as VillagersData,
};
use sim_core::{Calendar, FactionId, OfferKind, Offers, RunState, SimTick, SpawnId};
use sim_economy::{Economy, SettlementEconomy, Stockpile};
use sim_military::invitations::{SwornTerms, sworn_terms};
use sim_military::tenure::CourtData;
use sim_military::{
    Army, Follower, Parley, Progression, RunProgress, TakenBy, Tenure, bond_term, parley,
    paying_tribute, settles_at, watch,
};
use sim_world::Settlement;

use crate::army::OrderMenu;
use crate::selection::Selection;

/// How often the view is recomputed when nothing it keys on has changed
/// (real seconds).
const REFRESH_SECS: f64 = 0.5;

/// Longest line of the block, indent included: the settlement panel is
/// 380 px wide in a 13 px font.
const WIDTH: usize = 44;

/// Hours in a game day (the calendar's clock face).
const DAY_HOURS: u32 = 24;

/// Adds the [`TakeView`] and its refresh.
pub(crate) struct TakePlugin;

impl Plugin for TakePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TakeView>()
            .add_systems(Update, refresh.in_set(crate::UiSet::Hud));
    }
}

/// Whether a demand can be made at a town, and if not why.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Demand {
    /// Not offered: not a run, or the town is yours.
    Hidden,
    /// Can be made.
    Open,
    /// Greyed, with the reason.
    Refused(String),
}

/// What a gift of the menu's army would earn at the town now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Gift {
    /// This much goodwill.
    Goodwill(f32),
    /// The town was already given a gift today.
    Today,
    /// The army has nothing to spare.
    Nothing,
}

/// A submitted town of yours paying tribute: its watch.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tribute {
    /// Watched this hour (any of the three ways).
    pub watched_now: bool,
    /// Hours watched so far today.
    pub watched_hours: u8,
    /// Hours of the day gone.
    pub elapsed: u32,
    /// Watched hours that make a day (`demand.watch_hours`).
    pub need_hours: u8,
    /// Weak days in a row.
    pub weak_days: u8,
    /// Weak days that make it defy you (`demand.weak_days`).
    pub weak_limit: u8,
    /// The day it settles (counted from 1).
    pub settles_day: u64,
    /// Your strength within the watch radius.
    pub army: f32,
    /// The strength that watches it.
    pub hold: f32,
    /// A garrisoned town of yours lies near.
    pub garrison: bool,
    /// Your other towns' influence share there.
    pub share: f32,
    /// The share that watches it (`demand.watch_share`).
    pub share_need: f32,
}

/// An invited town of yours: its bond.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Bond {
    /// The Bond term now.
    pub value: f32,
    /// Raids driven off there.
    pub kept: u8,
    /// Calls left unanswered.
    pub broken: u8,
    /// Broken calls that end the bond for good.
    pub after: u8,
    /// The days (counted from 1) it was forsaken, oldest first.
    pub forsaken_days: Vec<u64>,
}

/// Everything the words about one town read, copied out of the sim.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Facts {
    /// Who holds it.
    pub owner: FactionId,
    /// How it came to its owner.
    pub how: TakenBy,
    /// The rival's seat.
    pub seat: bool,
    /// Whether the player can demand its submission.
    pub demand: Demand,
    /// The demand's comparison (towns not yours).
    pub parley: Option<Parley>,
    /// `demand.radius`.
    pub radius: f32,
    /// The resentment a conquest would leave, by the militia it would meet.
    pub scar_conquer: f32,
    /// The same for a sack.
    pub scar_sack: f32,
    /// Its goodwill toward you.
    pub goodwill: f32,
    /// Goodwill that makes it offer lordship.
    pub invite_goodwill: f32,
    /// Mean loyalty under which a rival town is weak enough to invite.
    pub weak_loyalty: f32,
    /// Its residents' mean loyalty.
    pub loyalty: f32,
    /// The menu army's gift forecast (only for the town under the menu).
    pub gift: Option<Gift>,
    /// What accepting its invitation would make it (a pending invitation).
    pub sworn: Option<SwornTerms>,
    /// Its watch (a submitted town of yours paying tribute).
    pub tribute: Option<Tribute>,
    /// Its bond (an invited town of yours).
    pub bond: Option<Bond>,
}

/// The facts the UI shows about a few towns, by SpawnId (UI-side only).
#[derive(Resource, Default)]
pub(crate) struct TakeView {
    towns: BTreeMap<u64, Facts>,
    stamp: f64,
    key: Key,
}

/// What the view was last computed for.
#[derive(Default, PartialEq)]
struct Key {
    selected: Option<Entity>,
    menu: Option<(u64, u64)>,
    invitations: Vec<u64>,
}

impl TakeView {
    /// The facts about town `id`, if it has been looked at.
    pub fn get(&self, id: u64) -> Option<&Facts> {
        self.towns.get(&id)
    }

    /// Whether town `id` is displaying "watched" or "unwatched" (a town of
    /// yours paying tribute): its watch.
    pub fn tribute(&self, id: u64) -> Option<&Tribute> {
        self.towns.get(&id)?.tribute.as_ref()
    }

    /// Whether a demand on town `id` can be sent now.
    pub fn demand_open(&self, id: u64) -> bool {
        self.get(id).is_some_and(|f| f.demand == Demand::Open)
    }

    /// What accepting town `id`'s invitation would make it.
    pub fn sworn(&self, id: u64) -> Option<SwornTerms> {
        self.get(id)?.sworn
    }
}

// ---------------------------------------------------------------- refresh

/// Recomputes the [`TakeView`] when the selection, the open menu or the
/// pending invitations change, and every [`REFRESH_SECS`]. Exclusive: the
/// sim's `parley` and `watch` read the whole world. Reads only.
fn refresh(world: &mut World) {
    let _span = info_span!("game_ui::take::refresh").entered();
    let ready = world.contains_resource::<Progression>()
        && world.contains_resource::<Calendar>()
        && world.contains_resource::<SimTick>()
        && world.contains_resource::<RunState>()
        && world.contains_resource::<Economy>()
        && world.contains_resource::<Army>()
        && world.contains_resource::<VillagersData>()
        && world.contains_resource::<sim_military::FighterGrid>();
    if !ready || !world.resource::<RunState>().in_run() {
        let mut view = world.resource_mut::<TakeView>();
        if !view.towns.is_empty() {
            view.towns.clear();
        }
        return;
    }
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |t| t.elapsed_secs_f64());
    let key = Key {
        selected: world.get_resource::<Selection>().and_then(|s| s.settlement),
        menu: world.get_resource::<OrderMenu>().and_then(|m| m.target()),
        invitations: world
            .get_resource::<Offers>()
            .map(|o| {
                o.pending
                    .iter()
                    .filter_map(|o| match o.kind {
                        OfferKind::Invitation { settlement } => Some(settlement),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    {
        let view = world.resource::<TakeView>();
        if view.key == key && now - view.stamp < REFRESH_SECS && now >= view.stamp {
            return;
        }
    }
    let mut towns: Vec<(Entity, u64)> = world
        .query_filtered::<(Entity, &SpawnId), With<Settlement>>()
        .iter(world)
        .map(|(e, id)| (e, id.0))
        .collect();
    towns.sort_unstable_by_key(|t| t.1);
    let captain = key.menu.and_then(|(c, _)| {
        world
            .query::<(Entity, &SpawnId)>()
            .iter(world)
            .find(|(_, id)| id.0 == c)
            .map(|(e, _)| e)
    });
    let mut facts = BTreeMap::new();
    for &(e, id) in &towns {
        let at_menu = key.menu.is_some_and(|(_, s)| s == id);
        let shown = key.selected == Some(e) || at_menu || key.invitations.contains(&id);
        let kneeling = world
            .get::<Tenure>(e)
            .is_some_and(|t| t.how == TakenBy::Submitted)
            && world
                .get::<Owner>(e)
                .is_some_and(|o| o.faction == FactionId::PLAYER);
        if !(shown || kneeling) {
            continue;
        }
        let gift_from = if at_menu { captain } else { None };
        if let Some(f) = facts_of(world, e, id, gift_from, key.invitations.contains(&id)) {
            facts.insert(id, f);
        }
    }
    let mut view = world.resource_mut::<TakeView>();
    view.towns = facts;
    view.key = key;
    view.stamp = now;
}

/// The facts about town `e` (SpawnId `id`), or `None` outside a run.
/// `gifter` is the menu's captain, whose gift to forecast; `invited` is
/// whether an invitation of the town waits.
fn facts_of(
    world: &mut World,
    e: Entity,
    id: u64,
    gifter: Option<Entity>,
    invited: bool,
) -> Option<Facts> {
    let tenure = world.get::<Tenure>(e)?.clone();
    let owner = world.get::<Owner>(e)?.faction;
    let tick = world.resource::<SimTick>().0;
    let cal = world.resource::<Calendar>().clone();
    let (demand_data, court, tenure_data, invitations, food_days) = {
        let d = &world.resource::<Progression>().data;
        (
            d.demand,
            d.court,
            d.tenure,
            d.invitations,
            d.requests.food_days,
        )
    };
    let seat = world
        .get_resource::<RunProgress>()
        .is_some_and(|r| r.rival_seat == Some(id));
    let mine = owner == FactionId::PLAYER;
    let day = cal.at(tick).day;
    let demand = if mine {
        Demand::Hidden
    } else if seat {
        Demand::Refused("the rival's seat never kneels".into())
    } else if tenure.cool_until > tick {
        Demand::Refused(refusal_cooldown(cal.at(tenure.cool_until).day + 1))
    } else {
        Demand::Open
    };
    let parley = (!mine)
        .then(|| parley(world, e, FactionId::PLAYER))
        .flatten();
    let able = parley.map_or(0, |p| p.able);
    let goodwill = world
        .get::<Goodwill>(e)
        .map_or(0.0, |g| g.get(FactionId::PLAYER));
    let loyalty = world.get::<SettlementLoyalty>(e).map_or(0.0, |l| l.mean);
    let gift = match (gifter, mine) {
        (Some(c), false) => Some(gift_for(
            world,
            c,
            e,
            tenure.gift_day == (day + 1) as u32,
            food_days,
            &court,
        )),
        _ => None,
    };
    let tribute = (mine && paying_tribute(world, &tenure, tick))
        .then(|| watch(world, e))
        .flatten()
        .map(|w| {
            let elapsed = (cal.at(tick).tick_of_day / cal.ticks_per_hour().max(1)) as u32;
            Tribute {
                watched_now: w.watched,
                watched_hours: tenure.watched_hours,
                elapsed: elapsed.min(DAY_HOURS),
                need_hours: demand_data.watch_hours,
                weak_days: tenure.weak_days,
                weak_limit: demand_data.weak_days,
                settles_day: cal.at(settles_at(world, tenure.since)).day + 1,
                army: w.strength,
                hold: w.hold,
                garrison: w.garrisoned_by.is_some(),
                share: w.share,
                share_need: demand_data.watch_share,
            }
        });
    let bond = (mine && tenure.how == TakenBy::Invited).then(|| Bond {
        value: bond_term(&tenure_data, &tenure),
        kept: tenure.kept,
        broken: tenure.broken,
        after: tenure_data.forsaken_after,
        forsaken_days: world
            .get::<DeedLog>(e)
            .map(|log| {
                log.entries
                    .iter()
                    .filter(|d| {
                        d.deed == Deed::Forsaken && d.faction == FactionId::PLAYER && !d.heard
                    })
                    .map(|d| d.day as u64 + 1)
                    .collect()
            })
            .unwrap_or_default(),
    });
    let sworn = if invited { sworn_terms(world, e) } else { None };
    Some(Facts {
        owner,
        how: tenure.how,
        seat,
        demand,
        parley,
        radius: demand_data.radius,
        scar_conquer: tenure_data.scar(TakenBy::Conquered, able),
        scar_sack: tenure_data.scar(TakenBy::Sacked, able),
        goodwill,
        invite_goodwill: invitations.goodwill,
        weak_loyalty: invitations.weak_loyalty,
        loyalty,
        gift,
        sworn,
        tribute,
        bond,
    })
}

/// The gift `captain`'s army would make at town `s` now (`orders::give`'s
/// spare food, `tenure::gift`'s goodwill).
fn gift_for(
    world: &mut World,
    captain: Entity,
    s: Entity,
    gifted_today: bool,
    food_days: f32,
    court: &CourtData,
) -> Gift {
    if gifted_today {
        return Gift::Today;
    }
    let hunger = world.resource::<VillagersData>().data.needs.hunger_per_day;
    let keep = world.resource::<Army>().data.give_keep_days;
    let people = world
        .query::<&Follower>()
        .iter(world)
        .filter(|f| f.captain == captain)
        .count() as f32
        + 1.0;
    let held = {
        let econ = world.resource::<Economy>();
        world
            .get::<Stockpile>(captain)
            .map_or(0.0, |b| b.food_value(econ))
    };
    let spare = (held - keep * hunger * people).max(0.0);
    let pop = world
        .get::<SettlementEconomy>(s)
        .map_or(0, |se| se.population) as f32;
    let hungry = world
        .get::<FoodSecurity>(s)
        .is_some_and(|f| f.days < food_days);
    gift_forecast(spare, hunger * pop, hungry, court)
}

// ------------------------------------------------------------------ words

/// The refusal a cooldown gives, `day` counted from 1.
fn refusal_cooldown(day: u64) -> String {
    format!("refused you; ask again on day {day}")
}

/// What a gift of `spare` food value to a town that eats `town_need` a
/// day earns (`tenure::gift`): `gift_per_day_of_need x spare / need`, at
/// most `max_gift`, times `hungry_mult` when the town is hungry.
pub(crate) fn gift_forecast(spare: f32, town_need: f32, hungry: bool, court: &CourtData) -> Gift {
    if spare <= 0.0 {
        return Gift::Nothing;
    }
    let mut v = (court.gift_per_day_of_need * spare / town_need.max(1e-3)).min(court.max_gift);
    if hungry {
        v *= court.hungry_mult;
    }
    if v > 0.0 && v.is_finite() {
        Gift::Goodwill(v)
    } else {
        Gift::Nothing
    }
}

/// The menu's "Give food" button: "Gift food" with its forecast at a town
/// that isn't yours (in a run), plain "Give food" otherwise.
pub(crate) fn gift_label(f: Option<&Facts>) -> String {
    let Some(f) = f.filter(|f| f.owner != FactionId::PLAYER) else {
        return "Give food".into();
    };
    match f.gift {
        None => "Gift food".into(),
        Some(Gift::Goodwill(g)) => format!("Gift food (+{g:.2} goodwill)"),
        Some(Gift::Today) => "Gift food (already gifted today)".into(),
        Some(Gift::Nothing) => "Gift food (nothing to spare)".into(),
    }
}

/// The menu's "Demand submission" button, or `None` to hide it.
pub(crate) fn demand_label(f: Option<&Facts>) -> Option<String> {
    let f = f?;
    match &f.demand {
        Demand::Hidden => None,
        Demand::Refused(why) => Some(format!("Demand submission ({why})")),
        Demand::Open => Some(match f.parley {
            Some(p) => format!(
                "Demand submission (need {} within {:.0}, have {})",
                p.need_shown(),
                f.radius,
                p.have_shown()
            ),
            None => "Demand submission".into(),
        }),
    }
}

/// The invitation card's body for town `place` (GP-32): what accepting
/// makes it.
pub(crate) fn sworn_words(place: &str, terms: &SwornTerms) -> String {
    let mood = if terms.seed >= 0.5 { "loyal" } else { "wary" };
    format!(
        "{place} offers lordship. Sworn: {mood} (starts at {:.2}); they will expect you \
         to defend them.",
        terms.seed
    )
}

/// A town's label mark from how it was taken (`None` for no mark): ASCII
/// tags the label font always renders, for the design's sword, broken
/// sword, bent knee, ribbon and eye. A submitted town of yours paying
/// tribute adds "watched" or "unwatched".
pub(crate) fn tenure_mark(owner: FactionId, how: TakenBy, watched: Option<bool>) -> Option<String> {
    if owner == FactionId::INDEPENDENT {
        return None;
    }
    let base = match how {
        TakenBy::Conquered => "sword",
        TakenBy::Sacked => "broken sword",
        TakenBy::Submitted => "bent knee",
        TakenBy::Invited => "ribbon",
        TakenBy::Subverted => "eye",
        TakenBy::Native | TakenBy::Granted | TakenBy::Freed => return None,
    };
    Some(match watched {
        Some(true) => format!("[{base}, watched]"),
        Some(false) => format!("[{base}, unwatched]"),
        None => format!("[{base}]"),
    })
}

/// Breaks `text` into lines of at most [`WIDTH`] characters: the first
/// line starts with `first`, the rest with `rest` (both indents).
fn wrap(text: &str, first: &str, rest: &str) -> String {
    let mut out = String::new();
    let mut line = first.to_string();
    let mut fresh = true;
    for word in text.split_whitespace() {
        if !fresh && line.len() + 1 + word.len() > WIDTH {
            out += &line;
            out.push('\n');
            line = rest.to_string();
            fresh = true;
        }
        if !fresh {
            line.push(' ');
        }
        line += word;
        fresh = false;
    }
    out += &line;
    out.push('\n');
    out
}

/// The settlement panel's block for a town that isn't yours: the ways to
/// take it, each with what it would take (design section 5.5). Empty for
/// your own towns. Subversion arrives with step 7.
pub(crate) fn ways_block(f: &Facts) -> String {
    if f.owner == FactionId::PLAYER {
        return String::new();
    }
    let mut s = String::from("Ways to take it\n");
    // Conquer.
    let (able, garrison) = f.parley.map_or((0, 0), |p| (p.able, p.garrison));
    s += &wrap(
        &format!("Conquer: militia about {able}, garrison {garrison}."),
        "  ",
        "    ",
    );
    s += &wrap(
        &format!(
            "Fast and bloody: resentment about {:.2} ({:.2} if sacked).",
            f.scar_conquer, f.scar_sack
        ),
        "    ",
        "    ",
    );
    // Demand.
    match (&f.demand, f.parley) {
        (Demand::Refused(why), _) => {
            s += &wrap(&format!("Demand: {why}."), "  ", "    ");
        }
        (_, Some(p)) => {
            s += &wrap(
                &format!(
                    "Demand: defence {:.0}. You need {} within {:.0} tiles and have {}.",
                    p.defence,
                    p.need_shown(),
                    f.radius,
                    p.have_shown()
                ),
                "  ",
                "    ",
            );
        }
        _ => {}
    }
    // Court.
    let court = if f.seat {
        "the rival's seat never invites.".to_string()
    } else if f.owner == FactionId::RIVAL {
        format!(
            "a rival town invites only when unhappy (loyalty under {:.0}%, now {:.0}%) and \
             fond of you ({:.2} of {:.2}), with your captain near.",
            f.weak_loyalty * 100.0,
            f.loyalty * 100.0,
            f.goodwill,
            f.invite_goodwill
        )
    } else {
        format!(
            "goodwill toward you {:+.2} of {:.2} needed for an invitation.",
            f.goodwill, f.invite_goodwill
        )
    };
    s += &wrap(&format!("Court: {court}"), "  ", "    ");
    s
}

/// The settlement panel's lines for a submitted town of yours paying
/// tribute: "Watched today: yes, 14 of 24 hours", its weak days and the
/// day it settles, and who watches it now. Empty without a watch.
pub(crate) fn tribute_block(f: &Facts) -> String {
    let Some(t) = &f.tribute else {
        return String::new();
    };
    let day = if t.watched_hours >= t.need_hours {
        "yes"
    } else if t.elapsed.saturating_sub(t.watched_hours as u32)
        > DAY_HOURS.saturating_sub(t.need_hours as u32)
    {
        "no"
    } else {
        "not yet"
    };
    let mut s = format!(
        "Watched today: {day}, {} of {} hours\n",
        t.watched_hours, t.elapsed
    );
    s += &format!(
        "  weak days {} of {} | settles on day {}\n",
        t.weak_days, t.weak_limit, t.settles_day
    );
    let garrison = if t.garrison { "near" } else { "none" };
    s += &wrap(
        &format!(
            "Now {}: army {:.0} of {:.0}, garrison {garrison}, sphere {:.0}% of {:.0}%.",
            if t.watched_now {
                "watched"
            } else {
                "unwatched"
            },
            t.army,
            t.hold,
            t.share * 100.0,
            t.share_need * 100.0
        ),
        "  ",
        "    ",
    );
    s
}

/// The settlement panel's lines for an invited town of yours: its bond,
/// and the days it was forsaken. Empty without a bond.
pub(crate) fn bond_block(f: &Facts) -> String {
    let Some(b) = &f.bond else {
        return String::new();
    };
    let mut s = format!(
        "Bond {:+.2} | {} kept, {} broken\n",
        b.value, b.kept, b.broken
    );
    if !b.forsaken_days.is_empty() {
        let days: Vec<String> = b.forsaken_days.iter().map(|d| format!("day {d}")).collect();
        s += &wrap(
            &format!("You forsook us ({})", days.join(", ")),
            "  ",
            "    ",
        );
    }
    if b.after > 0 && b.broken >= b.after {
        s += "  The bond is broken for good.\n";
    }
    s
}

/// All of a town's take lines for the settlement panel, newline-ended,
/// each block set off by a blank line from what precedes.
pub(crate) fn panel_block(f: &Facts) -> String {
    let mut s = String::new();
    for block in [ways_block(f), tribute_block(f), bond_block(f)] {
        if !block.is_empty() {
            s += "\n";
            s += &block;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn court() -> CourtData {
        CourtData {
            gift_per_day_of_need: 0.25,
            max_gift: 0.12,
            hungry_mult: 2.0,
        }
    }

    fn parley_of(need: f32, strength: f32) -> Parley {
        Parley {
            able: 38,
            garrison: 0,
            defence: need / 1.5,
            need,
            strength,
        }
    }

    fn facts(owner: FactionId) -> Facts {
        Facts {
            owner,
            how: TakenBy::Native,
            seat: false,
            demand: Demand::Open,
            parley: Some(parley_of(14.25, 11.9)),
            radius: 40.0,
            scar_conquer: 0.2,
            scar_sack: 0.34,
            goodwill: 0.21,
            invite_goodwill: 0.5,
            weak_loyalty: 0.4,
            loyalty: 0.52,
            gift: None,
            sworn: None,
            tribute: None,
            bond: None,
        }
    }

    /// Text with its line breaks undone, so wrapping never decides a test.
    fn flat(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn tribute() -> Tribute {
        Tribute {
            watched_now: true,
            watched_hours: 14,
            elapsed: 14,
            need_hours: 6,
            weak_days: 0,
            weak_limit: 2,
            settles_day: 9,
            army: 21.0,
            hold: 14.0,
            garrison: false,
            share: 0.3,
            share_need: 0.5,
        }
    }

    #[test]
    fn a_gift_is_a_fraction_of_the_towns_day() {
        // A fifth of a day's food: 0.25 x 0.2 = 0.05.
        assert_eq!(
            gift_forecast(20.0, 100.0, false, &court()),
            Gift::Goodwill(0.05)
        );
        // Capped at 0.12; doubled when hungry (the cap is before the double).
        assert_eq!(
            gift_forecast(500.0, 100.0, false, &court()),
            Gift::Goodwill(0.12)
        );
        assert_eq!(
            gift_forecast(500.0, 100.0, true, &court()),
            Gift::Goodwill(0.24)
        );
        assert_eq!(gift_forecast(0.0, 100.0, false, &court()), Gift::Nothing);
        // An empty town doesn't divide by zero.
        assert!(matches!(
            gift_forecast(5.0, 0.0, false, &court()),
            Gift::Goodwill(_)
        ));
    }

    #[test]
    fn the_gift_button_reads_its_forecast() {
        let mut f = facts(FactionId::INDEPENDENT);
        assert_eq!(gift_label(Some(&f)), "Gift food");
        f.gift = Some(Gift::Goodwill(0.08));
        assert_eq!(gift_label(Some(&f)), "Gift food (+0.08 goodwill)");
        f.gift = Some(Gift::Today);
        assert_eq!(gift_label(Some(&f)), "Gift food (already gifted today)");
        f.gift = Some(Gift::Nothing);
        assert_eq!(gift_label(Some(&f)), "Gift food (nothing to spare)");
        // Your own town, or no run: the old words.
        let mine = facts(FactionId::PLAYER);
        assert_eq!(gift_label(Some(&mine)), "Give food");
        assert_eq!(gift_label(None), "Give food");
    }

    #[test]
    fn the_demand_button_shows_its_numbers_or_its_reason() {
        let mut f = facts(FactionId::INDEPENDENT);
        assert_eq!(
            demand_label(Some(&f)).unwrap(),
            "Demand submission (need 15 within 40, have 11)"
        );
        f.demand = Demand::Refused(refusal_cooldown(6));
        assert_eq!(
            demand_label(Some(&f)).unwrap(),
            "Demand submission (refused you; ask again on day 6)"
        );
        f.demand = Demand::Refused("the rival's seat never kneels".into());
        assert!(
            demand_label(Some(&f))
                .unwrap()
                .contains("seat never kneels")
        );
        // Not offered at your own town, or outside a run.
        f.demand = Demand::Hidden;
        assert_eq!(demand_label(Some(&f)), None);
        assert_eq!(demand_label(None), None);
    }

    #[test]
    fn the_view_knows_whether_a_demand_can_go() {
        let mut view = TakeView::default();
        assert!(!view.demand_open(3));
        view.towns.insert(3, facts(FactionId::INDEPENDENT));
        assert!(view.demand_open(3));
        let mut f = facts(FactionId::INDEPENDENT);
        f.demand = Demand::Refused("x".into());
        view.towns.insert(4, f);
        assert!(!view.demand_open(4));
    }

    #[test]
    fn the_card_says_what_sworn_means() {
        let terms = SwornTerms {
            seed: 0.75,
            bond: 0.05,
        };
        assert_eq!(
            sworn_words("Nettlebrook", &terms),
            "Nettlebrook offers lordship. Sworn: loyal (starts at 0.75); they will expect \
             you to defend them."
        );
        let wary = SwornTerms {
            seed: 0.4,
            bond: 0.05,
        };
        assert!(sworn_words("Fenstead", &wary).contains("wary (starts at 0.40)"));
    }

    #[test]
    fn marks_follow_how_a_town_was_taken() {
        let mark = |owner, how, w| tenure_mark(owner, how, w);
        let you = FactionId::PLAYER;
        assert_eq!(mark(you, TakenBy::Conquered, None).unwrap(), "[sword]");
        assert_eq!(mark(you, TakenBy::Sacked, None).unwrap(), "[broken sword]");
        assert_eq!(mark(you, TakenBy::Invited, None).unwrap(), "[ribbon]");
        assert_eq!(mark(you, TakenBy::Subverted, None).unwrap(), "[eye]");
        assert_eq!(
            mark(you, TakenBy::Submitted, Some(true)).unwrap(),
            "[bent knee, watched]"
        );
        assert_eq!(
            mark(you, TakenBy::Submitted, Some(false)).unwrap(),
            "[bent knee, unwatched]"
        );
        // No mark for the start, a grant, a freed or an ownerless town.
        assert_eq!(mark(you, TakenBy::Native, None), None);
        assert_eq!(mark(FactionId::RIVAL, TakenBy::Granted, None), None);
        assert_eq!(mark(FactionId::INDEPENDENT, TakenBy::Conquered, None), None);
        // Every mark is plain ASCII.
        for how in TakenBy::ALL {
            if let Some(m) = mark(you, how, Some(true)) {
                assert!(m.is_ascii(), "{m}");
            }
        }
    }

    #[test]
    fn ways_to_take_a_free_town() {
        let f = facts(FactionId::INDEPENDENT);
        let s = ways_block(&f);
        let flat_s = flat(&s);
        assert!(flat_s.starts_with("Ways to take it"), "{s}");
        assert!(
            flat_s.contains("Conquer: militia about 38, garrison 0."),
            "{s}"
        );
        assert!(
            flat_s.contains("resentment about 0.20 (0.34 if sacked)"),
            "{s}"
        );
        assert!(
            flat_s.contains("Demand: defence 10. You need 15 within 40 tiles and have 11."),
            "{s}"
        );
        assert!(
            flat_s.contains("Court: goodwill toward you +0.21 of 0.50 needed for an invitation."),
            "{s}"
        );
        assert!(s.lines().all(|l| l.len() <= WIDTH), "{s}");
    }

    #[test]
    fn ways_to_take_a_rival_town_and_the_seat() {
        let mut f = facts(FactionId::RIVAL);
        f.demand = Demand::Refused(refusal_cooldown(7));
        let s = ways_block(&f);
        let flat_s = flat(&s);
        assert!(
            flat_s.contains("Demand: refused you; ask again on day 7."),
            "{s}"
        );
        assert!(flat_s.contains("invites only when unhappy"), "{s}");
        assert!(flat_s.contains("loyalty under 40%, now 52%"), "{s}");
        assert!(s.lines().all(|l| l.len() <= WIDTH), "{s}");
        f.seat = true;
        f.demand = Demand::Refused("the rival's seat never kneels".into());
        let s = ways_block(&f);
        let flat_s = flat(&s);
        assert!(
            flat_s.contains("Demand: the rival's seat never kneels."),
            "{s}"
        );
        assert!(
            flat_s.contains("Court: the rival's seat never invites."),
            "{s}"
        );
        assert!(
            flat_s.contains("Conquer:"),
            "the seat can still be conquered: {s}"
        );
    }

    #[test]
    fn your_own_town_has_no_ways_to_take_it() {
        assert_eq!(ways_block(&facts(FactionId::PLAYER)), "");
    }

    #[test]
    fn a_submitted_town_reports_its_watch() {
        let mut f = facts(FactionId::PLAYER);
        f.how = TakenBy::Submitted;
        f.tribute = Some(tribute());
        let s = tribute_block(&f);
        let flat_s = flat(&s);
        assert!(
            flat_s.starts_with("Watched today: yes, 14 of 14 hours"),
            "{s}"
        );
        assert!(
            flat_s.contains("weak days 0 of 2 | settles on day 9"),
            "{s}"
        );
        assert!(flat_s.contains("Now watched: army 21 of 14"), "{s}");
        assert!(flat_s.contains("garrison none, sphere 30% of 50%"), "{s}");
        assert!(s.lines().all(|l| l.len() <= WIDTH), "{s}");
    }

    #[test]
    fn today_is_not_yet_then_no() {
        let mut f = facts(FactionId::PLAYER);
        let mut t = tribute();
        // Early: 2 hours gone, none watched: still 22 left for 6.
        t.watched_hours = 0;
        t.elapsed = 2;
        t.watched_now = false;
        f.tribute = Some(t.clone());
        let s = tribute_block(&f);
        let flat_s = flat(&s);
        assert!(
            flat_s.starts_with("Watched today: not yet, 0 of 2 hours"),
            "{s}"
        );
        assert!(flat_s.contains("Now unwatched"), "{s}");
        // Late: 21 hours gone, 2 watched: 3 left cannot make 6.
        t.watched_hours = 2;
        t.elapsed = 21;
        f.tribute = Some(t);
        let s = tribute_block(&f);
        let flat_s = flat(&s);
        assert!(
            flat_s.starts_with("Watched today: no, 2 of 21 hours"),
            "{s}"
        );
    }

    #[test]
    fn an_invited_town_remembers_who_forsook_it() {
        let mut f = facts(FactionId::PLAYER);
        f.how = TakenBy::Invited;
        f.bond = Some(Bond {
            value: 0.07,
            kept: 1,
            broken: 0,
            after: 2,
            forsaken_days: vec![],
        });
        let s = bond_block(&f);
        assert!(s.starts_with("Bond +0.07 | 1 kept, 0 broken\n"), "{s}");
        assert!(!s.contains("forsook"), "{s}");
        f.bond = Some(Bond {
            value: -0.1,
            kept: 1,
            broken: 2,
            after: 2,
            forsaken_days: vec![9, 13],
        });
        let s = bond_block(&f);
        assert!(s.contains("You forsook us (day 9, day 13)"), "{s}");
        assert!(s.contains("broken for good"), "{s}");
        assert!(s.starts_with("Bond -0.10"), "{s}");
    }

    #[test]
    fn the_panel_block_sets_blocks_apart() {
        let f = facts(FactionId::INDEPENDENT);
        let s = panel_block(&f);
        assert!(s.starts_with("\nWays to take it"), "{s}");
        assert_eq!(panel_block(&facts(FactionId::PLAYER)), "");
    }

    #[test]
    fn words_wrap_between_words() {
        let s = wrap(
            "one two three four five six seven eight nine ten eleven twelve thirteen",
            "  ",
            "    ",
        );
        assert!(s.lines().count() > 1);
        assert!(s.lines().all(|l| l.len() <= WIDTH), "{s}");
        assert!(s.lines().skip(1).all(|l| l.starts_with("    ")), "{s}");
    }
}
