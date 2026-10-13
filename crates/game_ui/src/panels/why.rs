//! The settlement panel's "Why they feel this way" section (UI-11, SIM-60,
//! SIM-61, GP-32; M4a, ADR 0020): how the town came to its owner, its
//! loyalty target term by term, the dated deeds behind its goodwill, and its
//! risk of revolt, in plain words.
//!
//! A line on the town's influence (GP-31, M4a step 3) follows the tenure
//! line: the shares of the pull at the town and what they make of it.
//!
//! The words are built from state copied out of the sim into a [`Why`], so
//! they are tested without a world; the panel only reads. The rules that
//! decide what is named live in sim_agents (`loyalty_parts`,
//! `loyalty_target`, `loyalty_pulls`, `deciding_term`), not here.

use sim_agents::data::LoyaltyData;
use sim_agents::loyalty::{Pull, deciding_term, loyalty_parts, loyalty_pulls, loyalty_target};
use sim_agents::{Deed, DeedLog, Goodwill, LoyaltyTerm, LoyaltyTerms, Owner, SettlementLoyalty};
use sim_core::FactionId;
use sim_military::influence::{Influence, InfluenceData};
use sim_military::{TakenBy, Tenure};

use super::capitalise;

/// Longest line of the section, indent included: the panel is 380 px wide
/// in a 13 px monospace font.
const WIDTH: usize = 44;

/// A town is "close" to the revolt line within this much above it.
const CLOSE: f32 = 0.10;

/// A target this far from the mean is rising or falling, not steady.
const STEADY: f32 = 0.005;

/// Everything the section reads, copied out of the sim.
pub(super) struct Why<'a> {
    /// Who holds the town and since when.
    pub owner: Owner,
    /// Its goodwill toward each faction.
    pub goodwill: Option<&'a Goodwill>,
    /// Its food security, in days.
    pub food_days: Option<f32>,
    /// Its loyalty, summarised.
    pub loyalty: Option<&'a SettlementLoyalty>,
    /// Its itemised loyalty terms (a run's towns).
    pub terms: Option<&'a LoyaltyTerms>,
    /// Its dated deeds (a run's towns).
    pub log: Option<&'a DeedLog>,
    /// How it was taken (a run's towns).
    pub tenure: Option<&'a Tenure>,
    /// villagers.ron `loyalty`.
    pub data: &'a LoyaltyData,
    /// progression.ron `tenure.scar_fade_per_day`.
    pub scar_fade_per_day: f32,
    /// The town's influence (a run's towns, with influence on).
    pub influence: Option<InfluenceFacts<'a>>,
    /// The current tick.
    pub tick: u64,
    /// Ticks in a game day.
    pub ticks_per_day: u64,
    /// A faction's display name (army.ron).
    pub name: &'a dyn Fn(FactionId) -> String,
}

/// What the influence line reads, copied out of the sim.
pub(super) struct InfluenceFacts<'a> {
    /// The town's power and shares, as the sim last computed them.
    pub influence: &'a Influence,
    /// progression.ron `influence`.
    pub data: &'a InfluenceData,
    /// progression.ron `invitations.goodwill`: what an invitation needs.
    pub invite_goodwill: f32,
    /// Whether this is the rival's seat.
    pub seat: bool,
}

impl Why<'_> {
    /// The day number the panel shows for `tick` (days count from 1).
    fn day(&self, tick: u64) -> u64 {
        tick / self.ticks_per_day.max(1) + 1
    }

    /// Goodwill toward `faction` (0.0 when none is recorded).
    fn goodwill_to(&self, faction: FactionId) -> f32 {
        self.goodwill.map_or(0.0, |g| g.get(faction))
    }

    fn owned(&self) -> bool {
        self.owner.faction != FactionId::INDEPENDENT
    }

    /// A term's name from the owner's side, given its value `v`: the
    /// Contest term is the other side's influence, so at a rival town it is
    /// your pull; where the owner leads it is a bonus, the heartland.
    fn term_name(&self, term: LoyaltyTerm, v: f32) -> &'static str {
        match term {
            LoyaltyTerm::Contest if v > 0.0 => "heartland",
            LoyaltyTerm::Contest if self.owner.faction == FactionId::RIVAL => "your pull",
            _ => term.name(),
        }
    }

    /// A pull's name (of value `v`), its term named from the owner's side.
    fn pull_name(&self, pull: Pull, v: f32) -> &'static str {
        match pull {
            Pull::Term(term) => self.term_name(term, v),
            _ => pull.name(),
        }
    }
}

/// The section's text, or nothing for a town with nothing to explain (no
/// owner, no run, no goodwill, no log). Lines are indented two spaces and
/// end in a newline.
pub(super) fn section(w: &Why) -> String {
    let (you, rival) = (
        w.goodwill_to(FactionId::PLAYER),
        w.goodwill_to(FactionId::RIVAL),
    );
    let in_run = w.tenure.is_some();
    if !w.owned() && !in_run && w.log.is_none() && you == 0.0 && rival == 0.0 {
        return String::new();
    }
    let mut s = String::from("Why they feel this way\n");
    if let Some(line) = tenure_line(w) {
        s += &format!("  {line}\n");
    }
    s += &influence_lines(w);
    let target = match w.food_days {
        Some(food) if w.owned() => {
            let (text, target) = loyalty_block(w, food);
            s += &text;
            Some(target)
        }
        _ => None,
    };
    s += &goodwill_block(w, you, rival, in_run);
    if let (Some(food), Some(target)) = (w.food_days, target) {
        s += &risk_block(w, food, &target);
    }
    s
}

/// How a town's loyalty target came out.
struct Target {
    /// The target with its terms.
    value: f32,
    /// The target without them: food, treatment and time.
    base: f32,
}

/// "Yours since day 6, taken by the sword (resentment fades in 5 days)".
fn tenure_line(w: &Why) -> Option<String> {
    let t = w.tenure?;
    let (_, poss) = who(w.owner.faction, w.name);
    let holder = if !w.owned() {
        "Free".to_string()
    } else if w.owner.faction == FactionId::PLAYER {
        "Yours".to_string()
    } else {
        capitalise(&poss)
    };
    let when = if t.how == TakenBy::Native {
        "from the start".to_string()
    } else {
        format!("since day {}", w.day(t.since))
    };
    let how = match t.how {
        TakenBy::Native => None,
        TakenBy::Conquered => Some("taken by the sword".to_string()),
        TakenBy::Sacked => Some("sacked".to_string()),
        TakenBy::Submitted => Some("it knelt to a demand".to_string()),
        TakenBy::Invited => Some("by invitation".to_string()),
        TakenBy::Subverted => Some("turned from within".to_string()),
        TakenBy::Granted => Some("granted at the start".to_string()),
        // The taking before this one is the lord it threw off.
        TakenBy::Freed => Some(
            match t
                .history
                .iter()
                .rev()
                .nth(1)
                .map(|p| p.by)
                .filter(|f| *f != FactionId::INDEPENDENT)
            {
                Some(lord) => format!("it threw off {}", who(lord, w.name).0),
                None => "it broke free".to_string(),
            },
        ),
    };
    let mut line = format!("{holder} {when}");
    if let Some(how) = how {
        line += &format!(", {how}");
    }
    if w.owned() && t.scar > 0.0 && w.scar_fade_per_day > 0.0 {
        let days = (t.scar / w.scar_fade_per_day).ceil().max(1.0) as u32;
        let unit = if days == 1 { "day" } else { "days" };
        line += &format!(" (resentment fades in {days} {unit})");
    }
    Some(line)
}

/// The town's influence (GP-31): the shares of the pull at the town, from
/// its owner's side, and what they make of it. Nothing outside a run or
/// with influence off.
///
/// ```text
///   Influence: you 58%, the rival 4%
///     inside your sphere
///     likes you +0.21 of +0.50 for envoys
/// ```
fn influence_lines(w: &Why) -> String {
    let Some(f) = &w.influence else {
        return String::new();
    };
    if !f.data.enabled {
        return String::new();
    }
    let (you, them) = (f.influence.player, f.influence.rival);
    let net = f.influence.net();
    if f.seat {
        return "  Influence: the seat is never swayed\n".to_string();
    }
    let mut notes: Vec<String> = Vec::new();
    let head = match w.owner.faction {
        FactionId::PLAYER => {
            if net >= f.data.sphere {
                notes.push("your heartland".to_string());
            } else if net < 0.0 {
                notes.push("contested: the rival's pull leads".to_string());
            }
            format!("your pull {}, the rival's {}", pct(you), pct(them))
        }
        FactionId::RIVAL => {
            if -net >= f.data.sphere {
                notes.push("the rival's heartland".to_string());
            } else if net > 0.0 {
                notes.push("contested: your pull leads".to_string());
            }
            format!("the rival's pull {}, yours {}", pct(them), pct(you))
        }
        _ => {
            if you + them < 0.005 {
                notes.push("out of reach of both sides".to_string());
            } else if f.influence.envoy {
                notes.push("inside your sphere".to_string());
                notes.push("its envoys set out today".to_string());
            } else if net >= f.data.sphere {
                notes.push("inside your sphere".to_string());
                let likes = w.goodwill_to(FactionId::PLAYER);
                if likes >= f.invite_goodwill {
                    notes.push("liking you enough: envoys may come".to_string());
                } else {
                    notes.push(format!(
                        "likes you {} of {} for envoys",
                        signed(likes),
                        signed(f.invite_goodwill)
                    ));
                }
            } else if net >= f.data.warming {
                notes.push("warming to you".to_string());
            } else if -net >= f.data.sphere {
                notes.push("inside the rival's sphere".to_string());
            } else if net.abs() < f.data.front {
                notes.push("on the front".to_string());
            }
            format!("you {}, the rival {}", pct(you), pct(them))
        }
    };
    let mut s = format!("  Influence: {head}\n");
    for note in notes {
        s += &format!("    {note}\n");
    }
    s
}

/// The loyalty target, term by term:
///
/// ```text
///   Loyalty 47%, rising to 53%:
///     food +0.55, treatment +0.12, time +0.06,
///     resentment -0.14 (fading 0.02 a day),
///     rival's pull -0.06
/// ```
fn loyalty_block(w: &Why, food_days: f32) -> (String, Target) {
    let owner_goodwill = w.goodwill_to(w.owner.faction);
    let (food, treatment, time) = loyalty_parts(
        w.data,
        w.owner,
        owner_goodwill,
        food_days,
        w.tick,
        w.ticks_per_day,
    );
    let bias = w.terms.map_or(0.0, LoyaltyTerms::bias);
    let value = loyalty_target(
        w.data,
        w.owner,
        owner_goodwill,
        food_days,
        w.tick,
        w.ticks_per_day,
        bias,
    );
    let head = match w.loyalty.filter(|l| l.residents > 0) {
        Some(l) if value > l.mean + STEADY => {
            format!("Loyalty {}, rising to {}:", pct(l.mean), pct(value))
        }
        Some(l) if value < l.mean - STEADY => {
            format!("Loyalty {}, falling to {}:", pct(l.mean), pct(value))
        }
        Some(l) => format!("Loyalty {}, steady:", pct(l.mean)),
        None => format!("Loyalty target {}:", pct(value)),
    };
    let mut items = vec![
        format!("food {}", signed(food)),
        format!("treatment {}", signed(treatment)),
        format!("time {}", signed(time)),
    ];
    if let Some(terms) = w.terms {
        for (term, v) in terms.nonzero() {
            let mut item = format!("{} {}", w.term_name(term, v), signed(v));
            if term == LoyaltyTerm::Resentment && w.scar_fade_per_day > 0.0 {
                item += &format!(" (fading {:.2} a day)", w.scar_fade_per_day);
            }
            items.push(item);
        }
    }
    let base = food + treatment + time;
    if bias != 0.0 && base + bias > 1.0 {
        items.push("capped at 100%".to_string());
    } else if bias != 0.0 && base + bias < 0.0 {
        items.push("floored at 0%".to_string());
    }
    let text = format!("  {head}\n{}", wrap(&items, 4));
    (text, Target { value, base })
}

/// Goodwill toward you and the rival, then the dated deed log, newest
/// first:
///
/// ```text
///   Thinks of you +0.21, of the rival -0.10
///     day 8: your influence +0.03
///     day 7: a gift from you +0.09
/// ```
fn goodwill_block(w: &Why, you: f32, rival: f32, in_run: bool) -> String {
    let mut s = format!("  Thinks of you {}", signed(you));
    if in_run || rival != 0.0 {
        s += &format!(", of the rival {}", signed(rival));
    }
    s += "\n";
    let Some(log) = w.log else {
        return s;
    };
    if log.entries.is_empty() {
        s += "    nothing remembered yet\n";
    }
    for e in log.entries.iter().rev() {
        let (subject, poss) = who(e.faction, w.name);
        s += &format!(
            "    day {}: {} {}{}\n",
            e.day + 1,
            deed_words(e.deed, &subject, &poss),
            signed(e.delta),
            if e.heard { " (hearsay)" } else { "" }
        );
    }
    s
}

/// The revolt risk against villagers.ron's `revolt_threshold`, the term
/// that decides it when one does, and what pulls the target down:
///
/// ```text
///   Revolt risk: close. 38% against a line of 35%
///     Awe (+0.10) holds it above the line
///     Pulled down by hunger -0.28 and resentment -0.14
/// ```
fn risk_block(w: &Why, food_days: f32, target: &Target) -> String {
    let line = w.data.revolt_threshold;
    let loyalty = w.loyalty.filter(|l| l.residents > 0);
    let now = loyalty.map_or(target.value, |l| l.mean);
    let against = format!("{} against a line of {}", pct(now), pct(line));
    let (head, low) = if let Some(since) = loyalty.and_then(|l| l.revolt_since) {
        (
            format!("a rising is under way (since day {})", w.day(since)),
            false,
        )
    } else if let Some(since) = loyalty.and_then(|l| l.below_since) {
        (
            format!(
                "high. In unrest since day {}; a day of it can bring a rising",
                w.day(since)
            ),
            false,
        )
    } else if target.value < line {
        (
            format!(
                "rising. Heading for {}, under the line of {}",
                pct(target.value),
                pct(line)
            ),
            false,
        )
    } else if now < line + CLOSE {
        (format!("close. {against}"), false)
    } else {
        (format!("low. {against}"), true)
    };
    let mut s = format!("  Revolt risk: {head}\n");
    let none = LoyaltyTerms::default();
    let terms = w.terms.unwrap_or(&none);
    if let Some((term, v)) = deciding_term(target.base, terms, line) {
        let verb = if v > 0.0 {
            "holds it above"
        } else {
            "pulls it under"
        };
        s += &format!(
            "    {} ({}) {verb} the line\n",
            capitalise(w.term_name(term, v)),
            signed(v)
        );
    }
    if !low {
        let owner_goodwill = w.goodwill_to(w.owner.faction);
        let pulls = loyalty_pulls(w.data, w.owner, owner_goodwill, food_days, terms);
        let named: Vec<String> = pulls
            .iter()
            .take(2)
            .map(|(p, v)| format!("{} {}", w.pull_name(*p, *v), signed(*v)))
            .collect();
        if !named.is_empty() {
            s += &format!("    Pulled down by {}\n", named.join(" and "));
        }
    }
    s
}

/// What a deed reads as in a town's memory, from the town's side:
/// `subject` is "you", "the rival" or a faction's name and `poss` its
/// possessive.
fn deed_words(deed: Deed, subject: &str, poss: &str) -> String {
    match deed {
        Deed::Trade => format!("{subject} traded fairly with us"),
        Deed::Equip => format!("{subject} paid us for arms"),
        Deed::Seize => format!("{subject} seized our food"),
        Deed::Plunder => format!("{subject} plundered us"),
        Deed::Provoke => format!("{subject} drove us to arms"),
        Deed::Killing => format!("{subject} killed our men"),
        Deed::Request => format!("{subject} answered our call"),
        Deed::Boon => format!("{poss} name is known here"),
        Deed::Gift => format!("a gift from {subject}"),
        Deed::Defended => format!("{subject} fought for us"),
        Deed::Influence => format!("{poss} influence"),
        Deed::Whisper => format!("{poss} agent's whispers"),
        Deed::Festival => format!("a feast from {subject}"),
        Deed::Forsaken => format!("{subject} left our call unanswered"),
        Deed::Threat => format!("{subject} threatened us"),
        Deed::Plot => format!("{poss} plot, found out"),
    }
}

/// How the panel speaks of a faction: ("you", "your"), ("the rival", "the
/// rival's"), or its army.ron name.
fn who(f: FactionId, name: &dyn Fn(FactionId) -> String) -> (String, String) {
    if f == FactionId::PLAYER {
        ("you".to_string(), "your".to_string())
    } else if f == FactionId::RIVAL {
        ("the rival".to_string(), "the rival's".to_string())
    } else if f == FactionId::RAIDERS {
        ("the raiders".to_string(), "the raiders'".to_string())
    } else {
        let n = name(f);
        let poss = format!("{n}'s");
        (n, poss)
    }
}

/// A value with its sign and two decimals; a value that rounds to zero
/// reads "0.00", never "-0.00".
fn signed(v: f32) -> String {
    let s = format!("{v:+.2}");
    if s == "+0.00" || s == "-0.00" {
        "0.00".to_string()
    } else {
        s
    }
}

/// A 0..1 value as a whole percentage.
fn pct(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}

/// `items` joined with commas and broken into lines of at most [`WIDTH`]
/// characters, each indented by `indent`; an item is never split.
fn wrap(items: &[String], indent: usize) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::new();
    let mut line = pad.clone();
    let mut fresh = true;
    for (i, item) in items.iter().enumerate() {
        let comma = if i + 1 < items.len() { "," } else { "" };
        let piece = format!("{item}{comma}");
        if !fresh && line.len() + 1 + piece.len() > WIDTH {
            out += &line;
            out += "\n";
            line = pad.clone();
            fresh = true;
        }
        if !fresh {
            line += " ";
        }
        line += &piece;
        fresh = false;
    }
    out += &line;
    out += "\n";
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_agents::Villagers;
    use sim_core::Calendar;

    const DAY: u64 = 2_400;

    fn data() -> LoyaltyData {
        Villagers::default_data(&Calendar::default_data())
            .data
            .loyalty
    }

    fn name(f: FactionId) -> String {
        match f.0 {
            1 => "Your warband".to_string(),
            3 => "Rival".to_string(),
            5 => "Wolf Lord".to_string(),
            _ => "?".to_string(),
        }
    }

    fn owner(faction: FactionId, since_day: u64) -> Owner {
        Owner {
            faction,
            since: since_day * DAY,
        }
    }

    fn loyalty(mean: f32) -> SettlementLoyalty {
        SettlementLoyalty {
            mean,
            min: mean,
            residents: 40,
            below_since: None,
            revolt_since: None,
        }
    }

    /// A town of `owner` at day `day`, fed for `food_days`, with no terms,
    /// log or tenure; tests fill in what they read.
    fn town<'a>(
        data: &'a LoyaltyData,
        owner: Owner,
        day: u64,
        food_days: f32,
        goodwill: &'a Goodwill,
    ) -> Why<'a> {
        Why {
            owner,
            goodwill: Some(goodwill),
            food_days: Some(food_days),
            loyalty: None,
            terms: None,
            log: None,
            tenure: None,
            data,
            scar_fade_per_day: 0.02,
            influence: None,
            tick: day * DAY,
            ticks_per_day: DAY,
            name: &name,
        }
    }

    fn goodwill(you: f32, rival: f32) -> Goodwill {
        let mut g = Goodwill::default();
        g.set(FactionId::PLAYER, you);
        g.set(FactionId::RIVAL, rival);
        g
    }

    /// Signs and rounding: never "-0.00", always two decimals and a sign.
    #[test]
    fn signed_values_never_read_minus_zero() {
        assert_eq!(signed(0.0), "0.00");
        assert_eq!(signed(-0.001), "0.00");
        assert_eq!(signed(0.004), "0.00");
        assert_eq!(signed(0.1), "+0.10");
        assert_eq!(signed(-0.14), "-0.14");
        assert_eq!(pct(0.474), "47%");
    }

    /// Items break between items, never inside one, within the width.
    #[test]
    fn long_lists_wrap_between_items() {
        let items: Vec<String> = [
            "food +0.55",
            "treatment +0.12",
            "time +0.06",
            "resentment -0.14 (fading 0.02 a day)",
            "rival's pull -0.06",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let text = wrap(&items, 4);
        assert_eq!(
            text,
            "    food +0.55, treatment +0.12, time +0.06,\n\
             \x20   resentment -0.14 (fading 0.02 a day),\n\
             \x20   rival's pull -0.06\n"
        );
        assert!(text.lines().all(|l| l.len() <= WIDTH));
        assert_eq!(wrap(&items[..1], 4), "    food +0.55\n");
    }

    /// SIM-60: the target is read term by term, each part at its weight,
    /// each non-zero term by name, and the parts add up to the target.
    #[test]
    fn the_target_is_read_term_by_term() {
        let d = data();
        let g = goodwill(-0.2, 0.0);
        let mut w = town(&d, owner(FactionId::PLAYER, 0), 4, d.food_days, &g);
        let mut terms = LoyaltyTerms::default();
        terms.set(LoyaltyTerm::Resentment, -0.14);
        terms.set(LoyaltyTerm::Contest, -0.06);
        let l = loyalty(0.47);
        w.terms = Some(&terms);
        w.loyalty = Some(&l);
        let (text, target) = loyalty_block(&w, d.food_days);
        assert_eq!(
            text,
            "  Loyalty 47%, rising to 53%:\n\
             \x20   food +0.55, treatment +0.12, time +0.06,\n\
             \x20   resentment -0.14 (fading 0.02 a day),\n\
             \x20   rival's pull -0.06\n"
        );
        assert!((target.base - 0.73).abs() < 1e-5, "{}", target.base);
        assert!((target.value - 0.53).abs() < 1e-5, "{}", target.value);

        // Falling, steady and no summary at all.
        let l = loyalty(0.7);
        w.loyalty = Some(&l);
        assert!(loyalty_block(&w, d.food_days).0.contains("falling to 53%"));
        let l = loyalty(0.53);
        w.loyalty = Some(&l);
        assert!(
            loyalty_block(&w, d.food_days)
                .0
                .contains("Loyalty 53%, steady:")
        );
        w.loyalty = None;
        assert!(
            loyalty_block(&w, d.food_days)
                .0
                .contains("Loyalty target 53%:")
        );

        // Terms past the ends are named as capped.
        let mut big = LoyaltyTerms::default();
        big.set(LoyaltyTerm::Awe, 0.5);
        w.terms = Some(&big);
        let (text, target) = loyalty_block(&w, d.food_days);
        assert_eq!(target.value, 1.0);
        assert!(text.contains("awe +0.50, capped at 100%"), "{text}");
        let mut low = LoyaltyTerms::default();
        low.set(LoyaltyTerm::Stirred, -0.4);
        let w0 = town(&d, owner(FactionId::PLAYER, 0), 0, 0.0, &g);
        let w0 = Why {
            terms: Some(&low),
            ..w0
        };
        let (text, target) = loyalty_block(&w0, 0.0);
        assert_eq!(target.value, 0.0);
        assert!(text.contains("stirred -0.40, floored at 0%"), "{text}");
    }

    /// LORDSHIP_DESIGN section 4, "terms are named when they decide": the
    /// panel says which term keeps a town above the revolt line, or tips it
    /// under, and what pulls a town toward it.
    #[test]
    fn a_term_is_named_when_it_decides() {
        let d = data();
        // Awe: 0.00 food, neutral goodwill, ten days: 0.30 before the term.
        let g = goodwill(0.0, 0.0);
        let mut awe = LoyaltyTerms::default();
        awe.set(LoyaltyTerm::Awe, 0.1);
        let l = loyalty(0.38);
        let mut w = town(&d, owner(FactionId::PLAYER, 0), 10, 0.0, &g);
        w.terms = Some(&awe);
        w.loyalty = Some(&l);
        let (_, target) = loyalty_block(&w, 0.0);
        let text = risk_block(&w, 0.0, &target);
        assert!(
            text.contains("Awe (+0.10) holds it above the line"),
            "{text}"
        );
        assert!(
            text.starts_with("  Revolt risk: close. 38% against a line of 35%"),
            "{text}"
        );
        assert!(text.contains("Pulled down by hunger -0.55"), "{text}");

        // The same town with the Bond term instead.
        let mut bond = LoyaltyTerms::default();
        bond.set(LoyaltyTerm::Bond, 0.08);
        w.terms = Some(&bond);
        let (_, target) = loyalty_block(&w, 0.0);
        assert!(risk_block(&w, 0.0, &target).contains("Bond (+0.08) holds it above the line"));

        // Resentment tips a fed town under the line: 0.455 less 0.14.
        let g = goodwill(0.2, 0.0);
        let mut scar = LoyaltyTerms::default();
        scar.set(LoyaltyTerm::Resentment, -0.14);
        let mut w = town(&d, owner(FactionId::PLAYER, 0), 0, d.food_days / 2.0, &g);
        w.terms = Some(&scar);
        let (_, target) = loyalty_block(&w, d.food_days / 2.0);
        let text = risk_block(&w, d.food_days / 2.0, &target);
        assert!(
            text.contains("Resentment (-0.14) pulls it under the line"),
            "{text}"
        );
        assert!(
            text.starts_with("  Revolt risk: rising. Heading for 32%, under the line of 35%"),
            "{text}"
        );

        // A term that decides nothing is not named.
        let g = goodwill(0.5, 0.0);
        let w = town(&d, owner(FactionId::PLAYER, 0), 10, d.food_days, &g);
        let w = Why {
            terms: Some(&awe),
            ..w
        };
        let (_, target) = loyalty_block(&w, d.food_days);
        let text = risk_block(&w, d.food_days, &target);
        assert!(
            !text.contains("holds it above") && !text.contains("pulls it under"),
            "{text}"
        );
        assert!(text.contains("Revolt risk: low"), "{text}");
    }

    /// Two largest pulls are named, hunger first when it is the larger.
    #[test]
    fn the_two_largest_pulls_are_named() {
        let d = data();
        let g = goodwill(-0.4, 0.0);
        let mut terms = LoyaltyTerms::default();
        terms.set(LoyaltyTerm::Resentment, -0.14);
        terms.set(LoyaltyTerm::Contest, -0.05);
        let mut w = town(&d, owner(FactionId::PLAYER, 0), 1, 0.0, &g);
        w.terms = Some(&terms);
        let (_, target) = loyalty_block(&w, 0.0);
        let text = risk_block(&w, 0.0, &target);
        assert!(
            text.contains("Pulled down by hunger -0.55 and resentment -0.14\n"),
            "{text}"
        );
        assert!(!text.contains("rival's pull -0.05"), "only two: {text}");
    }

    /// GP-32: each way a town was taken reads in its tenure line, with the
    /// resentment that will fade.
    #[test]
    fn the_tenure_line_tells_how_a_town_was_taken() {
        let d = data();
        let g = goodwill(0.0, 0.0);
        let you = FactionId::PLAYER;
        let rival = FactionId::RIVAL;
        let line = |owner: Owner, mut t: Tenure, scar: f32| {
            t.scar = scar;
            let mut w = town(&d, owner, 12, d.food_days, &g);
            w.tenure = Some(&t);
            tenure_line(&w).expect("a tenure")
        };
        let mut conquered = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        conquered.retake(TakenBy::Conquered, you, 5 * DAY, 5, 0.12);
        assert_eq!(
            line(owner(you, 5), conquered.clone(), 0.12),
            "Yours since day 6, taken by the sword (resentment fades in 6 days)"
        );
        assert_eq!(
            line(owner(you, 5), conquered.clone(), 0.02),
            "Yours since day 6, taken by the sword (resentment fades in 1 day)"
        );
        assert_eq!(
            line(owner(you, 5), conquered, 0.0),
            "Yours since day 6, taken by the sword"
        );
        let cases = [
            (TakenBy::Sacked, "Yours since day 7, sacked"),
            (
                TakenBy::Submitted,
                "Yours since day 7, it knelt to a demand",
            ),
            (TakenBy::Invited, "Yours since day 7, by invitation"),
            (TakenBy::Subverted, "Yours since day 7, turned from within"),
        ];
        for (how, expected) in cases {
            let mut t = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
            t.retake(how, you, 6 * DAY, 6, 0.0);
            assert_eq!(line(owner(you, 6), t, 0.0), expected, "{how:?}");
        }
        // The start: native and independent, then granted to the rival.
        let native = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        assert_eq!(line(Owner::default(), native, 0.0), "Free from the start");
        let mut granted = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        granted.retake(TakenBy::Granted, rival, 0, 0, 0.0);
        assert_eq!(
            line(owner(rival, 0), granted, 0.0),
            "The rival's since day 1, granted at the start"
        );
        // A town that threw off its lord names him; a freed one with no
        // lord on record just broke free.
        let mut freed = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        freed.retake(TakenBy::Conquered, you, 2 * DAY, 2, 0.1);
        freed.retake(TakenBy::Freed, FactionId::INDEPENDENT, 9 * DAY, 9, 0.0);
        assert_eq!(
            line(
                Owner {
                    faction: FactionId::INDEPENDENT,
                    since: 9 * DAY
                },
                freed,
                0.0
            ),
            "Free since day 10, it threw off you"
        );
        let mut lone = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        lone.retake(TakenBy::Freed, FactionId::INDEPENDENT, 9 * DAY, 9, 0.0);
        assert_eq!(
            line(
                Owner {
                    faction: FactionId::INDEPENDENT,
                    since: 9 * DAY
                },
                lone,
                0.0
            ),
            "Free since day 10, it broke free"
        );
        // A faction with no word of its own is named from army.ron.
        let mut wolf = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        wolf.retake(TakenBy::Conquered, FactionId(5), 3 * DAY, 3, 0.0);
        assert_eq!(
            line(owner(FactionId(5), 3), wolf, 0.0),
            "Wolf Lord's since day 4, taken by the sword"
        );
    }

    /// SIM-61: the deed log reads newest first, dated from day 1, with the
    /// deed in the town's words, its size, and hearsay marked.
    #[test]
    fn the_deed_log_reads_newest_first_with_days() {
        let d = data();
        let g = goodwill(0.21, -0.10);
        let mut log = DeedLog::default();
        assert_eq!(log.entries.len(), 0);
        let (you, rival) = (FactionId::PLAYER, FactionId::RIVAL);
        log.record(5, you, Deed::Plunder, false, -0.10);
        log.record(6, you, Deed::Gift, false, 0.09);
        log.record(7, rival, Deed::Killing, true, -0.04);
        log.record(7, you, Deed::Influence, false, 0.03);
        let t = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        let mut w = town(&d, Owner::default(), 8, d.food_days, &g);
        w.log = Some(&log);
        w.tenure = Some(&t);
        assert_eq!(
            goodwill_block(&w, 0.21, -0.10, true),
            "  Thinks of you +0.21, of the rival -0.10\n\
             \x20   day 8: your influence +0.03\n\
             \x20   day 8: the rival killed our men -0.04 (hearsay)\n\
             \x20   day 7: a gift from you +0.09\n\
             \x20   day 6: you plundered us -0.10\n"
        );
        // An empty log says so; no log (not a run) is one line.
        let empty = DeedLog::default();
        w.log = Some(&empty);
        assert_eq!(
            goodwill_block(&w, 0.0, 0.0, true),
            "  Thinks of you 0.00, of the rival 0.00\n    nothing remembered yet\n"
        );
        w.log = None;
        assert_eq!(
            goodwill_block(&w, 0.2, 0.0, false),
            "  Thinks of you +0.20\n"
        );
        assert_eq!(
            goodwill_block(&w, 0.2, -0.3, false),
            "  Thinks of you +0.20, of the rival -0.30\n",
            "the rival's goodwill shows once it is non-zero"
        );
    }

    /// Every deed has words that name who did it, and none reads as code.
    #[test]
    fn every_deed_has_words() {
        let mut seen = std::collections::BTreeSet::new();
        for deed in Deed::ALL {
            let you = deed_words(deed, "you", "your");
            let wolf = deed_words(deed, "Wolf Lord", "Wolf Lord's");
            assert!(!you.contains(['{', '}']), "{deed:?}: {you}");
            assert!(
                you.contains("you") || you.contains("your"),
                "{deed:?}: {you}"
            );
            assert!(wolf.contains("Wolf Lord"), "{deed:?}: {wolf}");
            assert!(seen.insert(you), "{deed:?} reads like another deed");
        }
    }

    /// UI-11: the risk is graded and says why: a rising, unrest, a target
    /// under the line, close, low.
    #[test]
    fn revolt_risk_is_graded() {
        let d = data();
        let g = goodwill(0.0, 0.0);
        let head = |l: SettlementLoyalty, food: f32, day: u64| {
            let mut w = town(&d, owner(FactionId::PLAYER, 0), day, food, &g);
            w.loyalty = Some(&l);
            let (_, target) = loyalty_block(&w, food);
            risk_block(&w, food, &target)
                .lines()
                .next()
                .unwrap()
                .to_string()
        };
        let mut rising = loyalty(0.3);
        rising.below_since = Some(3 * DAY);
        rising.revolt_since = Some(5 * DAY);
        assert_eq!(
            head(rising, 0.0, 6),
            "  Revolt risk: a rising is under way (since day 6)"
        );
        let mut unrest = loyalty(0.3);
        unrest.below_since = Some(3 * DAY);
        assert_eq!(
            head(unrest, 0.0, 6),
            "  Revolt risk: high. In unrest since day 4; a day of it can bring a rising"
        );
        // Not yet in unrest, but heading under: no food, day 1.
        assert_eq!(
            head(loyalty(0.5), 0.0, 1),
            "  Revolt risk: rising. Heading for 16%, under the line of 35%"
        );
        // Fed, time under the owner: 0.55 + 0.15 + 0.015.
        assert_eq!(
            head(loyalty(0.40), d.food_days, 1),
            "  Revolt risk: close. 40% against a line of 35%"
        );
        assert_eq!(
            head(loyalty(0.8), d.food_days, 10),
            "  Revolt risk: low. 80% against a line of 35%"
        );
    }

    /// A free town has no loyalty or revolt lines, but its goodwill, deeds
    /// and tenure; a developed-world town with nothing to explain gets no
    /// section at all.
    #[test]
    fn a_free_town_explains_only_its_goodwill() {
        let d = data();
        let g = goodwill(0.1, 0.0);
        let t = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        let log = DeedLog::default();
        let mut w = town(&d, Owner::default(), 3, d.food_days, &g);
        w.tenure = Some(&t);
        w.log = Some(&log);
        let text = section(&w);
        assert_eq!(
            text,
            "Why they feel this way\n\
             \x20 Free from the start\n\
             \x20 Thinks of you +0.10, of the rival 0.00\n\
             \x20   nothing remembered yet\n"
        );
        let nothing = Goodwill::default();
        let w = Why {
            goodwill: Some(&nothing),
            tenure: None,
            log: None,
            ..town(&d, Owner::default(), 3, d.food_days, &g)
        };
        assert_eq!(section(&w), "");
        // Outside a run an owned town still shows its target and risk.
        let w = town(&d, owner(FactionId::PLAYER, 0), 3, d.food_days, &g);
        let text = section(&w);
        assert!(
            text.starts_with("Why they feel this way\n  Loyalty target"),
            "{text}"
        );
        assert!(text.contains("Thinks of you +0.10\n"), "{text}");
        assert!(text.contains("Revolt risk:"), "{text}");
    }

    /// The whole section for a conquered town: tenure, target, goodwill,
    /// deeds and risk, in that order.
    #[test]
    fn the_section_reads_in_order() {
        let d = data();
        let g = goodwill(0.21, -0.10);
        let you = FactionId::PLAYER;
        let mut t = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        t.retake(TakenBy::Conquered, you, 5 * DAY, 5, 0.14);
        let mut terms = LoyaltyTerms::default();
        terms.set(LoyaltyTerm::Resentment, -0.14);
        let mut log = DeedLog::default();
        log.record(5, you, Deed::Plunder, false, -0.10);
        let l = loyalty(0.47);
        let mut w = town(&d, owner(you, 5), 6, d.food_days, &g);
        w.tenure = Some(&t);
        w.terms = Some(&terms);
        w.log = Some(&log);
        w.loyalty = Some(&l);
        let text = section(&w);
        println!("{text}");
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("{needle}: {text}"))
        };
        let order = [
            at("Why they feel this way"),
            at("Yours since day 6"),
            at("Loyalty 47%"),
            at("Thinks of you"),
            at("day 6: you plundered us -0.10"),
            at("Revolt risk"),
        ];
        assert!(order.windows(2).all(|p| p[0] < p[1]), "{text}");
        assert!(text.lines().all(|l| l.len() <= 80), "{text}");
        assert!(text.ends_with('\n'));
    }

    /// The settlement panel's system parameters don't conflict (Bevy checks
    /// when a system is first set up, which no other test does), so the new
    /// queries can't panic the game on its first frame.
    #[test]
    fn the_settlement_panel_system_sets_up() {
        use bevy::ecs::system::{IntoSystem, System};
        let mut world = bevy::ecs::world::World::new();
        let mut system = IntoSystem::into_system(super::super::settlement_panel);
        system.initialize(&mut world);
    }

    fn facts<'a>(inf: &'a Influence, data: &'a InfluenceData, seat: bool) -> InfluenceFacts<'a> {
        InfluenceFacts {
            influence: inf,
            data,
            invite_goodwill: 0.5,
            seat,
        }
    }

    fn shares(player: f32, rival: f32) -> Influence {
        Influence {
            power: 0.0,
            player,
            rival,
            envoy: false,
        }
    }

    /// The influence line of a town of `owner` that thinks `likes` of you.
    fn influence_of(owner_faction: FactionId, inf: &Influence, likes: f32, seat: bool) -> String {
        let (d, idata) = (data(), InfluenceData::default());
        let g = goodwill(likes, 0.0);
        let mut w = town(&d, owner(owner_faction, 0), 3, d.food_days, &g);
        w.influence = Some(facts(inf, &idata, seat));
        influence_lines(&w)
    }

    /// GP-31, UI-12: a free town reads its shares and where it stands:
    /// warming, inside your sphere (and how near its envoys are), on the
    /// front, in the rival's sphere, or out of reach.
    #[test]
    fn a_free_town_reads_its_standing() {
        let free = FactionId::INDEPENDENT;
        let lines = |inf: Influence, likes: f32| influence_of(free, &inf, likes, false);
        assert_eq!(
            lines(shares(0.4, 0.1), 0.0),
            "  Influence: you 40%, the rival 10%\n    warming to you\n"
        );
        assert_eq!(
            lines(shares(0.6, 0.05), 0.21),
            "  Influence: you 60%, the rival 5%\n    inside your sphere\n    \
             likes you +0.21 of +0.50 for envoys\n"
        );
        assert_eq!(
            lines(shares(0.6, 0.05), 0.55),
            "  Influence: you 60%, the rival 5%\n    inside your sphere\n    \
             liking you enough: envoys may come\n"
        );
        let mut sending = shares(0.7, 0.0);
        sending.envoy = true;
        assert_eq!(
            lines(sending, 0.5),
            "  Influence: you 70%, the rival 0%\n    inside your sphere\n    \
             its envoys set out today\n"
        );
        assert_eq!(
            lines(shares(0.05, 0.6), 0.0),
            "  Influence: you 5%, the rival 60%\n    inside the rival's sphere\n"
        );
        assert_eq!(
            lines(shares(0.3, 0.25), 0.0),
            "  Influence: you 30%, the rival 25%\n    on the front\n"
        );
        assert_eq!(
            lines(shares(0.0, 0.0), 0.0),
            "  Influence: you 0%, the rival 0%\n    out of reach of both sides\n"
        );
        // The warming line is inclusive; just under it, only the numbers.
        assert_eq!(
            lines(shares(0.2, 0.0), 0.0),
            "  Influence: you 20%, the rival 0%\n    warming to you\n"
        );
        assert_eq!(
            lines(shares(0.19, 0.0), 0.0),
            "  Influence: you 19%, the rival 0%\n",
            "a net of 0.19 is not yet warming"
        );
    }

    /// An owned town reads the shares from its owner's side: its heartland
    /// where the owner leads by the sphere's width, a contest where the
    /// other side leads or (at the rival's town) you do.
    #[test]
    fn an_owned_town_reads_from_its_owners_side() {
        let (you, them) = (FactionId::PLAYER, FactionId::RIVAL);
        let mine = |inf: Influence| influence_of(you, &inf, 0.8, false);
        assert_eq!(
            mine(shares(0.7, 0.05)),
            "  Influence: your pull 70%, the rival's 5%\n    your heartland\n"
        );
        assert_eq!(
            mine(shares(0.3, 0.2)),
            "  Influence: your pull 30%, the rival's 20%\n"
        );
        assert_eq!(
            mine(shares(0.1, 0.4)),
            "  Influence: your pull 10%, the rival's 40%\n    \
             contested: the rival's pull leads\n"
        );
        let theirs = |inf: Influence| influence_of(them, &inf, -0.2, false);
        assert_eq!(
            theirs(shares(0.02, 0.7)),
            "  Influence: the rival's pull 70%, yours 2%\n    the rival's heartland\n"
        );
        assert_eq!(
            theirs(shares(0.4, 0.3)),
            "  Influence: the rival's pull 30%, yours 40%\n    contested: your pull leads\n"
        );
        // The seat is never swayed, whatever the numbers.
        assert_eq!(
            influence_of(them, &shares(0.5, 0.2), 0.0, true),
            "  Influence: the seat is never swayed\n"
        );
        // No line is wider than the panel.
        for inf in [shares(0.7, 0.05), shares(0.1, 0.4), shares(0.4, 0.3)] {
            for owner in [you, them, FactionId::INDEPENDENT] {
                for l in influence_of(owner, &inf, 0.21, false).lines() {
                    assert!(l.len() <= WIDTH, "{l:?}");
                }
            }
        }
    }

    /// No line without a run's influence, or with influence switched off.
    #[test]
    fn no_influence_line_without_influence() {
        let (d, idata) = (data(), InfluenceData::default());
        let g = goodwill(0.0, 0.0);
        let mut w = town(&d, owner(FactionId::INDEPENDENT, 0), 3, d.food_days, &g);
        assert_eq!(influence_lines(&w), "");
        let inf = shares(0.6, 0.0);
        let off = InfluenceData {
            enabled: false,
            ..idata
        };
        w.influence = Some(facts(&inf, &off, false));
        assert_eq!(influence_lines(&w), "");
    }

    /// The influence line sits right under the tenure line.
    #[test]
    fn the_influence_line_follows_the_tenure_line() {
        let (d, idata) = (data(), InfluenceData::default());
        let g = goodwill(0.0, 0.0);
        let t = Tenure::new(TakenBy::Native, FactionId::INDEPENDENT, 0, 0);
        let inf = shares(0.4, 0.1);
        let mut w = town(&d, owner(FactionId::INDEPENDENT, 0), 3, d.food_days, &g);
        w.tenure = Some(&t);
        w.influence = Some(facts(&inf, &idata, false));
        let text = section(&w);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "Why they feel this way");
        assert_eq!(lines[1], "  Free from the start");
        assert_eq!(lines[2], "  Influence: you 40%, the rival 10%");
        assert_eq!(lines[3], "    warming to you");
        assert!(lines[4].starts_with("  Thinks of you"), "{text}");
    }

    /// The Contest term is the other side's pull: the rival's at your
    /// towns, yours at the rival's.
    #[test]
    fn the_contest_is_named_from_the_owners_side() {
        let d = data();
        let g = goodwill(0.0, 0.0);
        let mut terms = LoyaltyTerms::default();
        terms.set(LoyaltyTerm::Contest, -0.06);
        let mut w = town(&d, owner(FactionId::PLAYER, 0), 4, d.food_days, &g);
        w.terms = Some(&terms);
        assert!(
            loyalty_block(&w, d.food_days)
                .0
                .contains("rival's pull -0.06")
        );
        w.owner = owner(FactionId::RIVAL, 0);
        let text = loyalty_block(&w, d.food_days).0;
        assert!(text.contains("your pull -0.06"), "{text}");
        assert!(!text.contains("rival's pull"), "{text}");
        assert_eq!(
            w.pull_name(Pull::Term(LoyaltyTerm::Contest), -0.06),
            "your pull"
        );
        assert_eq!(w.pull_name(Pull::Hunger, -0.5), "hunger");
    }

    /// Where the owner leads the other side by the sphere's width, the
    /// Contest term is a bonus and reads "heartland", not a pull, at either
    /// side's towns.
    #[test]
    fn a_positive_contest_is_the_heartland() {
        let d = data();
        let g = goodwill(0.0, 0.0);
        let mut terms = LoyaltyTerms::default();
        terms.set(LoyaltyTerm::Contest, 0.05);
        for faction in [FactionId::PLAYER, FactionId::RIVAL] {
            let mut w = town(&d, owner(faction, 0), 4, d.food_days, &g);
            w.terms = Some(&terms);
            let text = loyalty_block(&w, d.food_days).0;
            assert!(text.contains("heartland +0.05"), "{text}");
            assert!(!text.contains("pull"), "{text}");
            assert_eq!(w.term_name(LoyaltyTerm::Contest, 0.05), "heartland");
            assert_eq!(w.term_name(LoyaltyTerm::Awe, 0.05), "awe");
        }
    }
}
