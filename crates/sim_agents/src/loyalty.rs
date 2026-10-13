//! Villagers' loyalty to their settlement's owner (GP-06, SIM-24; ADR 0014).
//!
//! - [`Loyalty`] (0..1) on every villager, starting at
//!   `loyalty.independent_target` (villagers.ron).
//! - [`SettlementLoyalty`] on every populated settlement: the residents'
//!   mean and minimum, and how long the mean has been below
//!   `loyalty.revolt_threshold` (the revolt clock sim_military reads).
//! - Once a day, in [`AgentSet::Standing`](crate::AgentSet::Standing),
//!   [`drift_loyalty`] moves each villager toward [`loyalty_target`] and
//!   [`aggregate_loyalty`] refreshes the settlement summaries.
//!
//! Enlisted villagers are neither drifted nor counted. A deserter who
//! comes home takes the settlement's current mean loyalty (sim_military's
//! `apply_desertions`), since its owner may have changed meanwhile.
//!
//! M4a (Lordship, SIM-60, ADR 0020), run only: [`LoyaltyTerms`] itemise
//! what else pulls a town's target (resentment, awe, the rival's pull ...),
//! rebuilt hourly by sim_military's `tenure::step`; their sum is
//! [`loyalty_target`]'s `bias`. Outside a run the component is absent and
//! the bias is exactly 0.0, so nothing moves.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use sim_core::{
    Calendar, FactionId, FactionModifiers, SimTick, SpawnId, Stat, StateHash, StateHasher,
};

use crate::components::{Enlisted, Villager};
use crate::data::{LoyaltyData, Villagers};
use crate::food::FoodSecurity;
use crate::standing::{Goodwill, Owner};

/// A villager's loyalty to its settlement's owner, 0 (ready to rise) .. 1
/// (devoted) (GP-06). On every villager. Set by
/// [`set_owner`](crate::standing::set_owner) when the settlement changes
/// hands, then drifted daily.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Loyalty(pub f32);

impl StateHash for Loyalty {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.0);
    }
}

/// A settlement's loyalty, summarised daily from its residents who aren't
/// enlisted (GP-06, UI-02). On every populated settlement.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct SettlementLoyalty {
    /// Mean loyalty of the residents (0 with none).
    pub mean: f32,
    /// Lowest loyalty among them (0 with none).
    pub min: f32,
    /// Residents counted (not enlisted).
    pub residents: u32,
    /// The day tick since which `mean` has been below
    /// `loyalty.revolt_threshold` under the current owner. `None` while it
    /// isn't, and always for an independent or empty settlement. Restarts
    /// when the owner changes.
    pub below_since: Option<u64>,
    /// When a revolt began, for sim_military's revolt step (it sets and
    /// clears this). Cleared when the owner changes.
    pub revolt_since: Option<u64>,
}

// GP-12: saved field by field.
sim_core::save_newtype!(Loyalty);
sim_core::save_fields!(SettlementLoyalty {
    mean,
    min,
    residents,
    below_since,
    revolt_since
});

fn hash_tick(h: &mut StateHasher, t: Option<u64>) {
    match t {
        Some(t) => {
            h.u8(1);
            h.u64(t);
        }
        None => h.u8(0),
    }
}

impl StateHash for SettlementLoyalty {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.mean);
        h.f32(self.min);
        h.u32(self.residents);
        hash_tick(h, self.below_since);
        hash_tick(h, self.revolt_since);
    }
}

/// One itemised term of a run settlement's loyalty target (SIM-60, M4a;
/// LORDSHIP_DESIGN section 4). Append only: its position indexes
/// [`LoyaltyTerms`], which is saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum LoyaltyTerm {
    /// `-Tenure.scar`: a violent taking's resentment, fading daily.
    Resentment,
    /// A submitted town while it is watched (M4a step 5).
    Awe,
    /// The other side's influence there; your heartland (M4a step 3).
    Contest,
    /// An invited town's bond, kept or forsaken (M4a step 5).
    Bond,
    /// The Dues edict (M4a step 4).
    Dues,
    /// The Table edict: rationing, a feast's afterglow (M4a step 4).
    Table,
    /// The steward's traits: a native steward (M4a step 9).
    Steward,
    /// `-Tenure.stir`: an agent at work (M4a step 7).
    Stirred,
}

impl LoyaltyTerm {
    /// How many terms there are.
    pub const COUNT: usize = 8;

    /// Every term, in position order.
    pub const ALL: [LoyaltyTerm; Self::COUNT] = [
        LoyaltyTerm::Resentment,
        LoyaltyTerm::Awe,
        LoyaltyTerm::Contest,
        LoyaltyTerm::Bond,
        LoyaltyTerm::Dues,
        LoyaltyTerm::Table,
        LoyaltyTerm::Steward,
        LoyaltyTerm::Stirred,
    ];

    /// Display name ("why" lines, logs).
    pub fn name(self) -> &'static str {
        match self {
            LoyaltyTerm::Resentment => "resentment",
            LoyaltyTerm::Awe => "awe",
            LoyaltyTerm::Contest => "rival's pull",
            LoyaltyTerm::Bond => "bond",
            LoyaltyTerm::Dues => "dues",
            LoyaltyTerm::Table => "table",
            LoyaltyTerm::Steward => "steward",
            LoyaltyTerm::Stirred => "stirred",
        }
    }
}

/// A run settlement's itemised loyalty terms (SIM-60, M4a), one value per
/// [`LoyaltyTerm`], added to its [`loyalty_target`] as their sum
/// ([`LoyaltyTerms::bias`]). Inserted by sim_military's `setup_run` on
/// every populated settlement of a run (all zero) and rebuilt hourly by
/// its `tenure::step`; nothing else writes it. Hashed and saved.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct LoyaltyTerms(pub [f32; LoyaltyTerm::COUNT]);

impl LoyaltyTerms {
    /// The value of `term`.
    pub fn get(&self, term: LoyaltyTerm) -> f32 {
        self.0[term as usize]
    }

    /// Sets the value of `term`.
    pub fn set(&mut self, term: LoyaltyTerm, value: f32) {
        self.0[term as usize] = value;
    }

    /// The sum of the terms, in position order: `loyalty_target`'s `bias`.
    pub fn bias(&self) -> f32 {
        self.0.iter().sum()
    }

    /// The non-zero terms, in position order ("why" lines).
    pub fn nonzero(&self) -> impl Iterator<Item = (LoyaltyTerm, f32)> + '_ {
        LoyaltyTerm::ALL
            .into_iter()
            .map(|t| (t, self.get(t)))
            .filter(|(_, v)| *v != 0.0)
    }
}

impl StateHash for LoyaltyTerms {
    fn hash_state(&self, h: &mut StateHasher) {
        for v in self.0 {
            h.f32(v);
        }
    }
}

sim_core::save_newtype!(LoyaltyTerms);

/// The three weighted parts of an owned settlement's loyalty target before
/// its terms (UI-11, the "why" lines), as their contributions to the
/// target: `(food, treatment, time)`, each already multiplied by its weight
/// and divided by the weights' sum, so they add up to [`loyalty_target`]
/// with a bias of 0. All three are 0 for an independent settlement.
pub fn loyalty_parts(
    data: &LoyaltyData,
    owner: Owner,
    goodwill: f32,
    food_days: f32,
    tick: u64,
    ticks_per_day: u64,
) -> (f32, f32, f32) {
    if owner.faction == FactionId::INDEPENDENT {
        return (0.0, 0.0, 0.0);
    }
    let food = (food_days / data.food_days).clamp(0.0, 1.0);
    let treatment = ((goodwill + 1.0) / 2.0).clamp(0.0, 1.0);
    let days = tick.saturating_sub(owner.since) as f32 / ticks_per_day.max(1) as f32;
    let time = (days / data.settle_days).clamp(0.0, 1.0);
    let weights = data.w_food + data.w_goodwill + data.w_time;
    (
        data.w_food * food / weights,
        data.w_goodwill * treatment / weights,
        data.w_time * time / weights,
    )
}

/// The loyalty a settlement's villagers drift toward (GP-06, SIM-24).
///
/// Independent (`owner.faction` 0): `independent_target`. Under an owner:
/// the weighted mean of
/// - food security: `food_days / data.food_days`, capped at 1;
/// - treatment: goodwill toward the owner as `(goodwill + 1) / 2`;
/// - time under the owner: days since `owner.since` over `settle_days`,
///   capped at 1;
///
/// plus `bias`, the sum of the settlement's [`LoyaltyTerms`] (SIM-60, M4a),
/// clamped to 0..1. A bias of exactly 0.0 (outside a run) returns the
/// weighted mean untouched, so the developed world is an exact no-op.
pub fn loyalty_target(
    data: &LoyaltyData,
    owner: Owner,
    goodwill: f32,
    food_days: f32,
    tick: u64,
    ticks_per_day: u64,
    bias: f32,
) -> f32 {
    if owner.faction == FactionId::INDEPENDENT {
        return data.independent_target;
    }
    let food = (food_days / data.food_days).clamp(0.0, 1.0);
    let treatment = ((goodwill + 1.0) / 2.0).clamp(0.0, 1.0);
    let days = tick.saturating_sub(owner.since) as f32 / ticks_per_day.max(1) as f32;
    let time = (days / data.settle_days).clamp(0.0, 1.0);
    let weights = data.w_food + data.w_goodwill + data.w_time;
    let mean = (data.w_food * food + data.w_goodwill * treatment + data.w_time * time) / weights;
    if bias == 0.0 || !bias.is_finite() {
        return mean;
    }
    (mean + bias).clamp(0.0, 1.0)
}

/// One reason an owned settlement's loyalty target falls short (UI-11,
/// SIM-60, M4a): what the settlement panel names when it explains a town's
/// risk of revolt. Not saved or hashed; derived from state on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pull {
    /// The food weight left unmet: less than `loyalty.food_days` of food.
    Hunger,
    /// Goodwill toward the owner below neutral.
    IllWill,
    /// A negative itemised term (resentment, the rival's pull, a levy ...).
    Term(LoyaltyTerm),
}

impl Pull {
    /// Display name ("hunger", "ill will", a term's name).
    pub fn name(self) -> &'static str {
        match self {
            Pull::Hunger => "hunger",
            Pull::IllWill => "ill will",
            Pull::Term(t) => t.name(),
        }
    }
}

/// The negative pulls on an owned settlement's loyalty target, largest
/// first (ties in [`Pull`] order), each as its contribution to the target
/// (negative): hunger is the food weight left unmet, ill will goodwill
/// toward the owner below neutral, and every negative [`LoyaltyTerms`]
/// term as it stands. Time under the owner is not a pull: a new town is
/// not unhappy for being new. Empty for an independent settlement. The
/// first two are what a revolt's words would cite (LORDSHIP_DESIGN
/// section 4).
pub fn loyalty_pulls(
    data: &LoyaltyData,
    owner: Owner,
    goodwill: f32,
    food_days: f32,
    terms: &LoyaltyTerms,
) -> Vec<(Pull, f32)> {
    if owner.faction == FactionId::INDEPENDENT {
        return Vec::new();
    }
    let weights = data.w_food + data.w_goodwill + data.w_time;
    let food = (food_days / data.food_days).clamp(0.0, 1.0);
    let treatment = ((goodwill + 1.0) / 2.0).clamp(0.0, 1.0);
    let mut out = Vec::new();
    let hunger = 0.0 - data.w_food * (1.0 - food) / weights;
    if hunger < 0.0 {
        out.push((Pull::Hunger, hunger));
    }
    let ill_will = 0.0 - data.w_goodwill * (0.5 - treatment).max(0.0) / weights;
    if ill_will < 0.0 {
        out.push((Pull::IllWill, ill_will));
    }
    out.extend(
        terms
            .nonzero()
            .filter(|(_, v)| *v < 0.0)
            .map(|(t, v)| (Pull::Term(t), v)),
    );
    out.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    out
}

/// The term that decides whether an owned settlement stays on its side of
/// `line` (the revolt threshold), if one does (LORDSHIP_DESIGN section 4,
/// "terms are named when they decide"): the one whose removal alone would
/// carry the clamped target (`base`, the target without terms, plus
/// their bias) to the other side. A positive term returned holds the town
/// above the line; a negative one pulls it under. The largest by size wins,
/// ties by term order. `None` when no single term decides, and always
/// without terms.
pub fn deciding_term(base: f32, terms: &LoyaltyTerms, line: f32) -> Option<(LoyaltyTerm, f32)> {
    let sum = base + terms.bias();
    let above = sum.clamp(0.0, 1.0) >= line;
    terms
        .nonzero()
        .filter(|(_, v)| ((sum - v).clamp(0.0, 1.0) >= line) != above)
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()).then(b.0.cmp(&a.0)))
}

/// One day of drift: `loyalty` closes `rate` (clamped to 0..1) of the gap to
/// `target`. At the target it stays exactly where it is.
pub fn drift_toward(loyalty: f32, target: f32, rate: f32) -> f32 {
    let rate = rate.clamp(0.0, 1.0);
    (loyalty + (target - loyalty) * rate).clamp(0.0, 1.0)
}

/// Daily (GP-06, SIM-24): every villager who isn't enlisted drifts toward
/// its settlement's [`loyalty_target`], at `loyalty.drift_per_day` times
/// the owner's `LoyaltyDrift` modifier. In a run the target carries the
/// settlement's [`LoyaltyTerms`] as its bias (SIM-60); without them the
/// bias is 0.0. Parallel: each villager writes only its own `Loyalty` and
/// reads settlement components.
pub fn drift_loyalty(
    tick: Res<SimTick>,
    calendar: Res<Calendar>,
    villagers: Res<Villagers>,
    mods: Res<FactionModifiers>,
    settlements: Query<(&Owner, &Goodwill, &FoodSecurity, Option<&LoyaltyTerms>)>,
    mut people: Query<(&Villager, &mut Loyalty), Without<Enlisted>>,
) {
    let _span = tracing::info_span!("sim_agents::drift_loyalty").entered();
    let data = &villagers.data.loyalty;
    let (tick, tpd) = (tick.0, calendar.ticks_per_day);
    people.par_iter_mut().for_each(|(v, mut loyalty)| {
        let Ok((owner, goodwill, food, terms)) = settlements.get(v.settlement) else {
            return;
        };
        let target = loyalty_target(
            data,
            *owner,
            goodwill.get(owner.faction),
            food.days,
            tick,
            tpd,
            terms.map_or(0.0, LoyaltyTerms::bias),
        );
        let rate = data.drift_per_day * mods.mult(owner.faction, Stat::LoyaltyDrift);
        let next = drift_toward(loyalty.0, target, rate);
        if next != loyalty.0 {
            loyalty.0 = next;
        }
    });
}

#[derive(Default)]
struct Tally {
    sum: f64,
    min: f32,
    n: u32,
}

/// Daily, after [`drift_loyalty`] (GP-06): each settlement's
/// [`SettlementLoyalty`] from its residents, summed in SpawnId order. Sets
/// `below_since` when an owned settlement's mean falls below
/// `loyalty.revolt_threshold`, and clears it when the mean recovers.
pub fn aggregate_loyalty(
    tick: Res<SimTick>,
    villagers: Res<Villagers>,
    people: Query<(&SpawnId, &Villager, &Loyalty), Without<Enlisted>>,
    mut settlements: Query<(Entity, &Owner, &mut SettlementLoyalty)>,
) {
    let _span = tracing::info_span!("sim_agents::aggregate_loyalty").entered();
    let threshold = villagers.data.loyalty.revolt_threshold;
    let mut list: Vec<(SpawnId, Entity, f32)> = people
        .iter()
        .map(|(id, v, l)| (*id, v.settlement, l.0))
        .collect();
    list.sort_unstable_by_key(|(id, ..)| *id);
    // Each settlement's sum runs in its residents' SpawnId order.
    let mut tallies: BTreeMap<Entity, Tally> = BTreeMap::new();
    for (_, s, l) in list {
        let t = tallies.entry(s).or_insert(Tally {
            min: f32::INFINITY,
            ..Tally::default()
        });
        t.sum += l as f64;
        t.min = t.min.min(l);
        t.n += 1;
    }
    for (e, owner, mut sl) in &mut settlements {
        let (mean, min, n) = match tallies.get(&e) {
            Some(t) if t.n > 0 => ((t.sum / t.n as f64) as f32, t.min, t.n),
            _ => (0.0, 0.0, 0),
        };
        let below = n > 0 && owner.faction != FactionId::INDEPENDENT && mean < threshold;
        let below_since = if below {
            Some(sl.below_since.unwrap_or(tick.0))
        } else {
            None
        };
        let next = SettlementLoyalty {
            mean,
            min,
            residents: n,
            below_since,
            revolt_since: sl.revolt_since,
        };
        if *sl != next {
            *sl = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, TaskPoolPlugin};
    use bevy_ecs::system::RunSystemOnce;

    fn data() -> LoyaltyData {
        Villagers::default_data(&Calendar::default_data())
            .data
            .loyalty
    }

    fn owned(since: u64) -> Owner {
        Owner {
            faction: FactionId::PLAYER,
            since,
        }
    }

    /// SIM-24, GP-06: food, goodwill and time each pull the target up.
    #[test]
    fn the_target_weighs_food_goodwill_and_time() {
        let d = data();
        let tpd = 2_400;
        let independent = loyalty_target(&d, Owner::default(), -1.0, 0.0, 0, tpd, 0.0);
        assert_eq!(independent, d.independent_target);

        let starved = loyalty_target(&d, owned(0), 0.0, 0.0, 0, tpd, 0.0);
        let fed = loyalty_target(&d, owned(0), 0.0, d.food_days, 0, tpd, 0.0);
        let overfed = loyalty_target(&d, owned(0), 0.0, 10.0 * d.food_days, 0, tpd, 0.0);
        let liked = loyalty_target(&d, owned(0), 1.0, 0.0, 0, tpd, 0.0);
        let settled = loyalty_target(&d, owned(0), 0.0, 0.0, 100 * tpd, tpd, 0.0);
        let weights = d.w_food + d.w_goodwill + d.w_time;
        assert!((starved - 0.5 * d.w_goodwill / weights).abs() < 1e-6);
        assert!((fed - starved - d.w_food / weights).abs() < 1e-6);
        assert_eq!(overfed, fed, "food security is capped at food_days");
        assert!((liked - starved - 0.5 * d.w_goodwill / weights).abs() < 1e-6);
        assert!((settled - starved - d.w_time / weights).abs() < 1e-6);
        let everything = loyalty_target(&d, owned(0), 1.0, d.food_days, 100 * tpd, tpd, 0.0);
        assert!((everything - 1.0).abs() < 1e-6);
        // The tuning the conquest scenario relies on: a starved conquest
        // aims below the revolt threshold, a fed one above its seed.
        assert!(starved < d.revolt_threshold, "starved {starved}");
        assert!(fed > 0.5, "fed {fed}");
        // Treatment must count, or harsh owners could never lose a village.
        assert!(d.w_goodwill > 0.0, "shipped loyalty ignores goodwill");
    }

    /// GP-06, SIM-24: treatment reaches loyalty through goodwill toward the
    /// owner. Two villages, equally fed: the one that likes its owner ends
    /// the day more loyal than the one that hates it, and goodwill toward
    /// some other faction counts for nothing.
    #[test]
    fn goodwill_toward_the_owner_reaches_drift() {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        let world = app.world_mut();
        let calendar = Calendar::default_data();
        let d = data();
        world.insert_resource(Villagers::default_data(&calendar));
        world.insert_resource(calendar);
        world.insert_resource(SimTick(0));
        world.insert_resource(FactionModifiers::default());
        let fed = d.food_days;
        let liked = village(world, 0, owned(0), fed);
        let hated = village(world, 10, owned(0), fed);
        let neutral = village(world, 20, owned(0), fed);
        let elsewhere = village(world, 30, owned(0), fed);
        let mut set = |v: &Village, f: FactionId, g: f32| {
            world.get_mut::<Goodwill>(v.settlement).unwrap().set(f, g);
        };
        set(&liked, FactionId::PLAYER, 1.0);
        set(&hated, FactionId::PLAYER, -1.0);
        set(&elsewhere, FactionId::RIVAL, -1.0);
        day(world, 0);
        let l = |v: &Village| loyalty(world, v.people[0]);
        assert!(l(&liked) > l(&neutral), "{} vs {}", l(&liked), l(&neutral));
        assert!(l(&neutral) > l(&hated), "{} vs {}", l(&neutral), l(&hated));
        assert_eq!(l(&elsewhere), l(&neutral), "goodwill toward the rival");
    }

    /// SIM-60 (M4a): each term moves the target by exactly its value
    /// (clamped to 0..1); a bias of 0.0 is an exact no-op; an independent
    /// settlement ignores it; the parts add up to the unbiased target.
    #[test]
    fn terms_sum_into_the_target() {
        let d = data();
        let tpd = 2_400;
        let base = loyalty_target(&d, owned(0), -0.2, d.food_days, 3 * tpd, tpd, 0.0);
        let mut terms = LoyaltyTerms::default();
        assert_eq!(terms.bias(), 0.0);
        terms.set(LoyaltyTerm::Resentment, -0.12);
        let one = loyalty_target(&d, owned(0), -0.2, d.food_days, 3 * tpd, tpd, terms.bias());
        assert!((one - (base - 0.12)).abs() < 1e-6, "{one} vs {base}");
        terms.set(LoyaltyTerm::Awe, 0.1);
        terms.set(LoyaltyTerm::Stirred, -0.3);
        let all = loyalty_target(&d, owned(0), -0.2, d.food_days, 3 * tpd, tpd, terms.bias());
        assert!((all - (base - 0.32)).abs() < 1e-6, "{all}");
        assert_eq!(
            terms.nonzero().map(|(t, _)| t).collect::<Vec<_>>(),
            [
                LoyaltyTerm::Resentment,
                LoyaltyTerm::Awe,
                LoyaltyTerm::Stirred
            ]
        );
        assert_eq!(
            loyalty_target(&d, owned(0), 1.0, d.food_days, 100 * tpd, tpd, 0.5),
            1.0,
            "clamped"
        );
        assert_eq!(
            loyalty_target(&d, owned(0), -1.0, 0.0, 0, tpd, -0.9),
            0.0,
            "clamped"
        );
        assert_eq!(
            loyalty_target(&d, Owner::default(), 0.0, 0.0, 0, tpd, -0.4),
            d.independent_target,
            "nobody to be loyal to"
        );
        let (food, treatment, time) = loyalty_parts(&d, owned(0), -0.2, d.food_days, 3 * tpd, tpd);
        assert!((food + treatment + time - base).abs() < 1e-6);
        assert_eq!(
            loyalty_parts(&d, Owner::default(), 0.0, 1.0, 0, tpd),
            (0.0, 0.0, 0.0)
        );
        for (i, t) in LoyaltyTerm::ALL.iter().enumerate() {
            assert_eq!(*t as usize, i, "{}", t.name());
        }
    }

    /// UI-11 (M4a): the pulls on a target are the unmet food weight, goodwill
    /// below neutral and every negative term, largest first; an independent
    /// town, a fed liked town and a town with only positive terms have none.
    #[test]
    fn pulls_are_named_largest_first() {
        let d = data();
        let weights = d.w_food + d.w_goodwill + d.w_time;
        let mut terms = LoyaltyTerms::default();
        terms.set(LoyaltyTerm::Resentment, -0.14);
        terms.set(LoyaltyTerm::Contest, -0.05);
        terms.set(LoyaltyTerm::Awe, 0.1);
        let pulls = loyalty_pulls(&d, owned(0), -0.4, 0.0, &terms);
        let hunger = -d.w_food / weights;
        let ill_will = -d.w_goodwill * 0.2 / weights;
        let names: Vec<_> = pulls.iter().map(|(p, _)| p.name()).collect();
        assert_eq!(
            names,
            ["hunger", "resentment", "ill will", "rival's pull"],
            "{pulls:?}"
        );
        assert!((pulls[0].1 - hunger).abs() < 1e-6);
        assert!((pulls[2].1 - ill_will).abs() < 1e-6);
        assert!(pulls.iter().all(|(_, v)| *v < 0.0), "Awe is no pull");

        let fed = loyalty_pulls(&d, owned(0), 0.0, d.food_days, &LoyaltyTerms::default());
        assert!(fed.is_empty(), "neutral goodwill and full food: {fed:?}");
        let mut kind = LoyaltyTerms::default();
        kind.set(LoyaltyTerm::Bond, 0.05);
        assert!(loyalty_pulls(&d, owned(0), 0.5, 5.0 * d.food_days, &kind).is_empty());
        assert!(loyalty_pulls(&d, Owner::default(), -1.0, 0.0, &terms).is_empty());
        // Half the food is half the hunger; the sum of what is missing
        // from the food and treatment parts is what the parts leave out.
        let half = loyalty_pulls(&d, owned(0), 0.0, d.food_days / 2.0, &kind);
        assert_eq!(half.len(), 1);
        assert!((half[0].1 - hunger / 2.0).abs() < 1e-6);
    }

    /// UI-11 (M4a): a term decides when taking it away alone would carry the
    /// target across the revolt line; the biggest such term is named, a
    /// positive one holds the town above, a negative one pulls it under.
    #[test]
    fn the_deciding_term_flips_the_line() {
        let line = 0.35;
        let mut terms = LoyaltyTerms::default();
        assert_eq!(deciding_term(0.3, &terms, line), None, "no terms");
        terms.set(LoyaltyTerm::Awe, 0.1);
        // 0.30 + 0.10 = 0.40: awe holds the town above the line.
        assert_eq!(
            deciding_term(0.3, &terms, line),
            Some((LoyaltyTerm::Awe, 0.1))
        );
        // 0.50 + 0.10: it would be above without awe too.
        assert_eq!(deciding_term(0.5, &terms, line), None);
        // 0.20 + 0.10 = 0.30: below with it, below without it.
        assert_eq!(deciding_term(0.2, &terms, line), None);
        terms.set(LoyaltyTerm::Bond, 0.08);
        // 0.30 + 0.18 = 0.48: without either term alone it is still above.
        assert_eq!(deciding_term(0.3, &terms, line), None, "neither alone");
        // 0.20 + 0.18 = 0.38: without the awe 0.28, without the bond 0.30,
        // so either alone decides, and the bigger is named.
        assert_eq!(
            deciding_term(0.2, &terms, line),
            Some((LoyaltyTerm::Awe, 0.1))
        );
        // A bond alone, a negative term pulling under.
        let mut bond = LoyaltyTerms::default();
        bond.set(LoyaltyTerm::Bond, 0.08);
        assert_eq!(
            deciding_term(0.3, &bond, line),
            Some((LoyaltyTerm::Bond, 0.08))
        );
        let mut scar = LoyaltyTerms::default();
        scar.set(LoyaltyTerm::Resentment, -0.14);
        assert_eq!(
            deciding_term(0.45, &scar, line),
            Some((LoyaltyTerm::Resentment, -0.14)),
            "0.31 with the scar, 0.45 without it"
        );
        assert_eq!(deciding_term(0.8, &scar, line), None);
        // The clamp counts: a target pinned at 1.0 is decided by nothing.
        let mut big = LoyaltyTerms::default();
        big.set(LoyaltyTerm::Awe, 0.1);
        assert_eq!(deciding_term(1.5, &big, line), None);
    }

    /// SIM-60 (M4a): a settlement with `LoyaltyTerms` drifts toward the
    /// biased target; one without them (every preset but a run) drifts
    /// exactly as before.
    #[test]
    fn drift_reads_the_terms() {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        let world = app.world_mut();
        let calendar = Calendar::default_data();
        world.insert_resource(Villagers::default_data(&calendar));
        world.insert_resource(calendar);
        world.insert_resource(SimTick(0));
        world.insert_resource(FactionModifiers::default());
        let d = data();
        let plain = village(world, 0, owned(0), d.food_days);
        let resentful = village(world, 10, owned(0), d.food_days);
        let mut terms = LoyaltyTerms::default();
        terms.set(LoyaltyTerm::Resentment, -0.2);
        world.entity_mut(resentful.settlement).insert(terms);
        day(world, 0);
        let target = loyalty_target(&d, owned(0), 0.0, d.food_days, 0, 2_400, 0.0);
        let expect = |t: f32| drift_toward(0.5, t, d.drift_per_day);
        assert_eq!(loyalty(world, plain.people[0]), expect(target));
        assert_eq!(
            loyalty(world, resentful.people[0]),
            expect((target - 0.2).clamp(0.0, 1.0))
        );
    }

    #[test]
    fn drift_closes_part_of_the_gap() {
        assert_eq!(drift_toward(0.5, 0.1, 0.25), 0.4);
        assert_eq!(drift_toward(0.5, 0.9, 1.5), 0.9, "rate clamped to 1");
        assert_eq!(drift_toward(0.5, 0.9, -1.0), 0.5);
        assert_eq!(
            drift_toward(0.6, 0.6, 0.4),
            0.6,
            "exactly still at the target"
        );
    }

    struct Village {
        settlement: Entity,
        people: Vec<Entity>,
        soldier: Entity,
    }

    /// A settlement with four residents at loyalty 0.5 plus an enlisted one.
    fn village(world: &mut World, first: u64, owner: Owner, food_days: f32) -> Village {
        let settlement = world
            .spawn((
                SpawnId(first),
                owner,
                Goodwill::default(),
                FoodSecurity {
                    days: food_days,
                    food_value: 0.0,
                },
                SettlementLoyalty::default(),
            ))
            .id();
        let person = |world: &mut World, id: u64| {
            world
                .spawn((
                    SpawnId(id),
                    Villager {
                        name: sim_core::NameId(id as u32),
                        age_days: 9_000,
                        home: settlement,
                        settlement,
                    },
                    Loyalty(0.5),
                ))
                .id()
        };
        let people = (1..=4).map(|i| person(world, first + i)).collect();
        let soldier = person(world, first + 5);
        world.entity_mut(soldier).insert(Enlisted);
        Village {
            settlement,
            people,
            soldier,
        }
    }

    fn day(world: &mut World, n: u64) {
        let tpd = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = n * tpd;
        world.run_system_once(drift_loyalty).unwrap();
        world.run_system_once(aggregate_loyalty).unwrap();
    }

    fn loyalty(world: &World, e: Entity) -> f32 {
        world.get::<Loyalty>(e).unwrap().0
    }

    /// GP-06, SIM-24: a starved conquest loses loyalty day by day until its
    /// mean is below the revolt threshold, a fed one gains it, and an
    /// independent village settles at its target. Enlisted villagers are
    /// neither drifted nor counted.
    #[test]
    fn loyalty_drifts_toward_its_target_each_day() {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        let world = app.world_mut();
        let calendar = Calendar::default_data();
        let d = data();
        world.insert_resource(Villagers::default_data(&calendar));
        world.insert_resource(calendar);
        world.insert_resource(SimTick(0));
        world.insert_resource(FactionModifiers::default());
        let starved = village(world, 0, owned(0), 0.0);
        let fed = village(world, 10, owned(0), 2.0 * d.food_days);
        let free = village(world, 20, Owner::default(), 0.0);

        // (Later the growing time term slowly lifts the target again, but a
        // starved village never gets back to the threshold on time alone.)
        let mut last = 0.5;
        let mut fell_below = None;
        for n in 0..4 {
            day(world, n);
            let now = loyalty(world, starved.people[0]);
            assert!(now < last, "day {n}: {now} after {last}");
            for p in &starved.people {
                assert_eq!(loyalty(world, *p), now, "one settlement, one target");
            }
            let sl = *world.get::<SettlementLoyalty>(starved.settlement).unwrap();
            assert_eq!((sl.mean, sl.min, sl.residents), (now, now, 4));
            if now < d.revolt_threshold {
                let since = *fell_below.get_or_insert(n * 2_400);
                assert_eq!(sl.below_since, Some(since), "the clock keeps its start");
            } else {
                assert_eq!(sl.below_since, None);
            }
            last = now;
        }
        assert!(
            fell_below.is_some(),
            "a starved conquest falls below the threshold"
        );
        assert_eq!(loyalty(world, starved.soldier), 0.5, "enlisted: untouched");
        assert!(loyalty(world, fed.people[0]) > 0.5);
        let sl = *world.get::<SettlementLoyalty>(fed.settlement).unwrap();
        assert_eq!(sl.below_since, None);
        // Independence: from 0.5 toward independent_target, never "below".
        let f = loyalty(world, free.people[0]);
        assert!(f > 0.5 && f <= d.independent_target, "{f}");
        let sl = *world.get::<SettlementLoyalty>(free.settlement).unwrap();
        assert_eq!(sl.below_since, None);

        // Feeding the starved village lifts it back above the threshold, and
        // the clock clears.
        world
            .get_mut::<FoodSecurity>(starved.settlement)
            .unwrap()
            .days = 2.0 * d.food_days;
        for n in 6..12 {
            day(world, n);
        }
        let sl = *world.get::<SettlementLoyalty>(starved.settlement).unwrap();
        assert!(sl.mean > d.revolt_threshold, "{}", sl.mean);
        assert_eq!(sl.below_since, None);
    }

    /// GP-06: the owner's `LoyaltyDrift` modifier scales the daily rate.
    #[test]
    fn the_loyalty_drift_modifier_speeds_drift() {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        let world = app.world_mut();
        let calendar = Calendar::default_data();
        world.insert_resource(Villagers::default_data(&calendar));
        world.insert_resource(calendar);
        world.insert_resource(SimTick(0));
        let mut mods = FactionModifiers::default();
        mods.add(
            FactionId::PLAYER,
            Stat::LoyaltyDrift,
            sim_core::ModifierSource::Monuments { kind: 6 },
            0.5,
        );
        world.insert_resource(mods);
        let fast = village(world, 0, owned(0), 0.0);
        let slow = village(
            world,
            10,
            Owner {
                faction: FactionId::RIVAL,
                since: 0,
            },
            0.0,
        );
        day(world, 0);
        let d = data();
        let target = loyalty_target(&d, owned(0), 0.0, 0.0, 0, 2_400, 0.0);
        let expect = |rate: f32| 0.5 + (target - 0.5) * rate;
        assert_eq!(
            loyalty(world, fast.people[0]),
            expect(d.drift_per_day * 1.5)
        );
        assert_eq!(loyalty(world, slow.people[0]), expect(d.drift_per_day));
    }
}
