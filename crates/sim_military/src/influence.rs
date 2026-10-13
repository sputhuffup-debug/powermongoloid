//! Influence: a sphere you can see grow (GP-31, M4a; LORDSHIP_DESIGN
//! section 6, ADR 0020). Run only.
//!
//! - **The field is pure.** A held town (the player's or the rival's)
//!   projects [`power`]: `sqrt(people / people_unit) x (loyalty_floor + (1 -
//!   loyalty_floor) x mean loyalty) x (1 - scar)`, times `seat_power` at the
//!   rival's seat; people and loyalty only, no specialty or building
//!   multiplier. Its [`pull`] falls linearly to 0 at `reach` tiles (centre
//!   to centre; a town counts for itself at distance 0). [`field_at`] sums
//!   the pulls at a point into the player's and the rival's [`Shares`]:
//!   `player = I_P / (I_P + I_R + independence)`, likewise the rival's. The
//!   renderer draws the same field from each town's [`Influence::power`]
//!   ([`Source::of`]).
//! - [`Influence`] sits on every populated settlement of a run (inserted by
//!   `progression::setup_run`): its power and the shares at its centre, and
//!   the day's envoy flag.
//! - [`step`] is progress slot 13 (hourly, run only, nothing when
//!   `influence.enabled` is false): it refreshes every town's [`Influence`]
//!   in SpawnId order; then, on the day tick:
//!   1. **Warmth.** Every town the player doesn't own, the rival's seat
//!      excepted, with a net above `warm_above` and goodwill toward the
//!      player under `goodwill_cap` gets a `Deed::Influence` toward the
//!      player ([`warmth`]: `goodwill_per_day x (net - warm_above)`, never
//!      past the cap). Influence deeds never spread (`Deed::spreads`).
//!   2. **Envoys.** Up to `max_envoys_per_day` independent towns (not the
//!      seat) whose net is at least `envoy_net` and whose goodwill toward
//!      the player, with this dawn's warmth, is at least
//!      `invitations.goodwill`, with no invitation pending or cooling down,
//!      are flagged ([`Influence::envoy`]), the highest net first (ties by
//!      SpawnId); every other flag is cleared.
//!      `invitations::step` (slot 7) posts a flagged town's invitation in
//!      the hours that follow with no captain near.
//!   3. **The realm's rank.** The player's champion gains
//!      `influence.rank_points` per `influence.rank_people` people the
//!      player holds ([`realm_units`]), through `champion::credit_times`
//!      with `Feat::Realm`.
//! - Read elsewhere: [`contest_term`] (the Contest loyalty term
//!   `tenure::step` sums, slot 14), the envoy branch of
//!   `invitations::step` (slot 7) and the sphere's ease in `drifters::draw`
//!   (slot 5, [`in_sphere`]).
//! - **A taking refreshes the field at once** (`tenure::retake` calls
//!   `refresh_field` before it rebuilds the town's terms), so the
//!   Contest term follows the new owner in the same tick rather than
//!   counting the town's own power for its old one for up to an hour.
//! - **The seat is never swayed:** no influence deed, no envoys and no
//!   Contest term at the rival's seat.

use bevy_ecs::prelude::*;
use bevy_math::{IVec2, Vec2};
use serde::{Deserialize, Serialize};
use sim_agents::{Deed, Goodwill, GoodwillLedger, Owner, SettlementLoyalty};
use sim_core::{
    Calendar, FactionId, OfferKind, Offers, RunState, SimTick, SpawnId, StateHash, StateHasher,
};
use sim_economy::SettlementEconomy;
use sim_world::Settlement;

use crate::progression::champion::{Feat, credit_times};
use crate::progression::{Progression, RunProgress};
use crate::tenure::Tenure;

/// A run settlement's influence (GP-31, M4a): `power` it projects (held
/// towns only) and the shares of the pull at its centre, each in 0..1, and
/// the day's envoy flag. Written by [`step`] and, at a taking, by
/// `refresh_field` (power and shares only); `invitations::step` spends
/// the envoy flag when it posts the invitation. Hashed and saved.
#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct Influence {
    /// The power the town projects, if held: people and loyalty
    /// ([`power`]). 0 for a town nobody holds.
    pub power: f32,
    /// The player's share of the pull at the town.
    pub player: f32,
    /// The rival's share of the pull at the town.
    pub rival: f32,
    /// Set on the day tick: this independent town lies inside the player's
    /// sphere (net at least `influence.envoy_net`), likes the player enough
    /// to invite (`invitations.goodwill`) and sends envoys today, one of at
    /// most `influence.max_envoys_per_day` across the island.
    /// `invitations::step` posts its invitation with no captain near and
    /// clears the flag; the next day tick sets the flags afresh.
    pub envoy: bool,
}

impl Influence {
    /// The player's share minus the rival's, in -1..1.
    pub fn net(&self) -> f32 {
        self.player - self.rival
    }

    /// The shares at the town.
    pub fn shares(&self) -> Shares {
        Shares {
            player: self.player,
            rival: self.rival,
        }
    }
}

impl StateHash for Influence {
    fn hash_state(&self, h: &mut StateHasher) {
        h.f32(self.power);
        h.f32(self.player);
        h.f32(self.rival);
        h.bool(self.envoy);
    }
}

sim_core::save_fields!(Influence {
    power,
    player,
    rival,
    envoy
});

/// Influence (GP-31): `influence` in progression.ron. See docs/dev/DATA.md
/// and LORDSHIP_DESIGN section 6.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InfluenceData {
    /// `false` makes influence a no-op (scenario tests opt out).
    pub enabled: bool,
    /// A town's pull falls to 0 at this many tiles.
    pub reach: f32,
    /// Power grows with the square root of people over this.
    pub people_unit: f32,
    /// Power's loyalty factor at loyalty 0 (1.0 at loyalty 1).
    pub loyalty_floor: f32,
    /// The rival's seat projects this times its power.
    pub seat_power: f32,
    /// A town's own independence in the shares.
    pub independence: f32,
    /// A town you don't own warms when the net is above this ...
    pub warm_above: f32,
    /// ... by this times the excess, a day ...
    pub goodwill_per_day: f32,
    /// ... until its goodwill toward you reaches this.
    pub goodwill_cap: f32,
    /// An independent town at this net (and liking you) sends envoys.
    pub envoy_net: f32,
    /// Captain-free invitations a day, across the island.
    pub max_envoys_per_day: u32,
    /// The Contest term per share the other side leads by.
    pub contest: f32,
    /// The Contest term in the player's heartland (a town of the player's
    /// where the player's share leads the rival's by at least `sphere`;
    /// never at the rival's towns).
    pub heartland: f32,
    /// The overlay's "warming" line (net).
    pub warming: f32,
    /// The sphere's edge (net): the overlay's dashed line, the heartland's
    /// lead and the drifters' ease.
    pub sphere: f32,
    /// The overlay's front (|net| under this).
    pub front: f32,
    /// The champion gains `rank_points` per this many people held, a day.
    pub rank_people: f32,
    /// See `rank_people`.
    pub rank_points: u32,
    /// Drifters' goodwill line drops by this inside your sphere.
    pub drifter_ease: f32,
}

impl Default for InfluenceData {
    /// The shipped values.
    fn default() -> Self {
        Self {
            enabled: true,
            reach: 200.0,
            people_unit: 100.0,
            loyalty_floor: 0.25,
            seat_power: 2.0,
            independence: 0.5,
            warm_above: 0.1,
            goodwill_per_day: 0.12,
            goodwill_cap: 0.6,
            envoy_net: 0.5,
            max_envoys_per_day: 1,
            contest: 0.25,
            heartland: 0.05,
            warming: 0.2,
            sphere: 0.5,
            front: 0.1,
            rank_people: 500.0,
            rank_points: 1,
            drifter_ease: 0.1,
        }
    }
}

impl InfluenceData {
    /// Checks the block.
    pub fn validate(&self) -> Result<(), String> {
        let positive = |v: f32| v.is_finite() && v > 0.0;
        let non_negative = |v: f32| v.is_finite() && v >= 0.0;
        let unit = |v: f32| (0.0..=1.0).contains(&v);
        if !positive(self.reach)
            || !positive(self.people_unit)
            || !positive(self.rank_people)
            || !positive(self.independence)
            || !non_negative(self.seat_power)
            || !non_negative(self.goodwill_per_day)
            || !non_negative(self.contest)
            || !non_negative(self.heartland)
            || ![
                self.loyalty_floor,
                self.warm_above,
                self.goodwill_cap,
                self.envoy_net,
                self.warming,
                self.sphere,
                self.front,
                self.drifter_ease,
            ]
            .into_iter()
            .all(unit)
        {
            return Err(
                "progression: influence needs reach, people_unit, rank_people and independence > 0, \
                 weights >= 0 and shares in 0..1"
                    .into(),
            );
        }
        Ok(())
    }
}

/// One held town's push on the field (GP-31): where it stands, whose it is
/// and its [`power`]. `id` (its SpawnId) lets a caller leave a town out
/// (the Demand watch counts only a town's neighbours, M4a step 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    /// The town's SpawnId.
    pub id: u64,
    /// Its centre.
    pub at: Vec2,
    /// Its owner: only the player's and the rival's count in the shares.
    pub faction: FactionId,
    /// Its [`power`].
    pub power: f32,
}

impl Source {
    /// The source town `id` at `center`, held by `owner`, is, from its
    /// [`Influence`] (the renderer's way in): `None` unless the player or
    /// the rival holds it and it projects some power.
    pub fn of(id: u64, center: IVec2, owner: FactionId, influence: &Influence) -> Option<Self> {
        (matches!(owner, FactionId::PLAYER | FactionId::RIVAL) && influence.power > 0.0).then_some(
            Self {
                id,
                at: center.as_vec2(),
                faction: owner,
                power: influence.power,
            },
        )
    }
}

/// The player's and the rival's shares of the pull at a point (GP-31),
/// each in 0..1, their sum under 1 (the rest is the place's own
/// independence).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Shares {
    /// The player's share.
    pub player: f32,
    /// The rival's share.
    pub rival: f32,
}

impl Shares {
    /// The player's share minus the rival's, in -1..1.
    pub fn net(&self) -> f32 {
        self.player - self.rival
    }

    /// `faction`'s share (0 for anyone but the player and the rival).
    pub fn of(&self, faction: FactionId) -> f32 {
        match faction {
            FactionId::PLAYER => self.player,
            FactionId::RIVAL => self.rival,
            _ => 0.0,
        }
    }
}

/// GP-31: the power a held town projects: `sqrt(people / people_unit) x
/// (loyalty_floor + (1 - loyalty_floor) x loyalty) x (1 - scar)`, times
/// `seat_power` for the rival's seat. People and loyalty only (the scar
/// is how fresh and resented the holding is). Loyalty and scar are clamped
/// to 0..1.
pub fn power(data: &InfluenceData, people: u32, loyalty: f32, scar: f32, seat: bool) -> f32 {
    let floor = data.loyalty_floor;
    let p = (people as f32 / data.people_unit).sqrt()
        * (floor + (1.0 - floor) * loyalty.clamp(0.0, 1.0))
        * (1.0 - scar.clamp(0.0, 1.0));
    if seat { p * data.seat_power } else { p }
}

/// GP-31: a town of `power`'s pull at `distance` tiles: `power x max(0, 1 -
/// distance / reach)`.
pub fn pull(data: &InfluenceData, power: f32, distance: f32) -> f32 {
    power * (1.0 - distance / data.reach).max(0.0)
}

/// GP-31, pure: the player's and the rival's [`Shares`] of the pull at
/// `at` from `sources` (summed in the order given; the sim passes them in
/// SpawnId order): `player = I_P / (I_P + I_R + independence)`, likewise
/// the rival's. Sources of any other faction add nothing. The sim and the
/// renderer draw the same field with it.
pub fn field_at<'a>(
    data: &InfluenceData,
    sources: impl IntoIterator<Item = &'a Source>,
    at: Vec2,
) -> Shares {
    let (mut p, mut r) = (0.0f32, 0.0f32);
    for s in sources {
        let w = pull(data, s.power, (s.at - at).length());
        match s.faction {
            FactionId::PLAYER => p += w,
            FactionId::RIVAL => r += w,
            _ => {}
        }
    }
    let total = p + r + data.independence;
    Shares {
        player: p / total,
        rival: r / total,
    }
}

/// GP-31: the Contest loyalty term at a town held by `owner`, whose share
/// is `own` and the other side's `other`: `-contest x (other - own)` where
/// the other side leads; `heartland` in the player's heartland (the player
/// holds it and leads by at least `sphere`; the design's "your heartland",
/// never at the rival's towns, whose loyalty the rival's own field must not
/// prop up); 0 otherwise.
pub fn contest_value(data: &InfluenceData, owner: FactionId, own: f32, other: f32) -> f32 {
    if other > own {
        0.0 - data.contest * (other - own)
    } else if owner == FactionId::PLAYER && own - other >= data.sphere {
        data.heartland
    } else {
        0.0
    }
}

/// GP-31: a day's `Deed::Influence` toward the player at a town the player
/// doesn't own (not the seat) with net `net` and goodwill `goodwill`
/// toward the player: `goodwill_per_day x (net - warm_above)` when the net
/// is above `warm_above`, never past `goodwill_cap`; 0 otherwise.
pub fn warmth(data: &InfluenceData, net: f32, goodwill: f32) -> f32 {
    if net <= data.warm_above || goodwill >= data.goodwill_cap {
        return 0.0;
    }
    (data.goodwill_per_day * (net - data.warm_above)).min(data.goodwill_cap - goodwill)
}

/// GP-31: whether a town lies inside the player's sphere (its net is at
/// least `influence.sphere`).
pub fn in_sphere(data: &InfluenceData, influence: &Influence) -> bool {
    influence.net() >= data.sphere
}

/// GP-31 (design question 16): how many times a day the player's champion
/// earns `influence.rank_points` for `people` held: one per
/// `influence.rank_people`, rounded down.
pub fn realm_units(data: &InfluenceData, people: u32) -> u32 {
    (people as f64 / f64::from(data.rank_people)).floor() as u32
}

/// One town as the step sees it.
struct Town {
    id: u64,
    e: Entity,
    at: Vec2,
    owner: FactionId,
    people: u32,
    goodwill: f32,
    power: f32,
    seat: bool,
    shares: Shares,
}

/// GP-31: progress slot 13, hourly, run only (span
/// `sim_military::influence::step`): see the module docs. Nothing when
/// `influence.enabled` is false.
pub fn step(world: &mut World) {
    let _span = tracing::info_span!("sim_military::influence::step").entered();
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let data = world.resource::<Progression>().data.influence;
    if !data.enabled {
        return;
    }
    let towns = refresh(world, &data);
    let tick = world.resource::<SimTick>().0;
    if world.resource::<Calendar>().on_day(tick) {
        warm(world, &data, &towns);
        flag_envoys(world, &data, &towns, tick);
        credit_realm(world, &data, &towns);
    }
}

/// GP-31: refreshes every town's power and shares now (a taking calls it
/// through `tenure::retake`, so the new owner's Contest term reads the
/// field it makes). In a run with influence enabled only; the envoy flags
/// are left as the day tick set them.
pub(crate) fn refresh_field(world: &mut World) {
    if !world.resource::<RunState>().in_run() {
        return;
    }
    let data = world.resource::<Progression>().data.influence;
    if data.enabled {
        refresh(world, &data);
    }
}

/// The field: every run town's power from its people, loyalty and scar,
/// then the shares at each centre from every held town, written to its
/// [`Influence`] where they changed. Returns the towns in SpawnId order.
fn refresh(world: &mut World, data: &InfluenceData) -> Vec<Town> {
    let seat = world.resource::<RunProgress>().rival_seat;
    let mut q = world.query_filtered::<(
        Entity,
        &SpawnId,
        &Settlement,
        &Owner,
        &SettlementEconomy,
        &SettlementLoyalty,
        &Goodwill,
        Option<&Tenure>,
    ), With<Influence>>();
    let mut towns: Vec<Town> = q
        .iter(world)
        .map(|(e, id, st, o, se, sl, g, t)| {
            let seat = Some(id.0) == seat;
            let held = matches!(o.faction, FactionId::PLAYER | FactionId::RIVAL);
            let power = if held {
                let scar = t.map_or(0.0, |t| t.scar);
                let at_seat = seat && o.faction == FactionId::RIVAL;
                power(data, se.population, sl.mean, scar, at_seat)
            } else {
                0.0
            };
            Town {
                id: id.0,
                e,
                at: st.center.as_vec2(),
                owner: o.faction,
                people: se.population,
                goodwill: g.get(FactionId::PLAYER),
                power,
                seat,
                shares: Shares::default(),
            }
        })
        .collect();
    towns.sort_unstable_by_key(|t| t.id);
    let sources: Vec<Source> = towns
        .iter()
        .filter(|t| matches!(t.owner, FactionId::PLAYER | FactionId::RIVAL) && t.power > 0.0)
        .map(|t| Source {
            id: t.id,
            at: t.at,
            faction: t.owner,
            power: t.power,
        })
        .collect();
    for t in &mut towns {
        t.shares = field_at(data, &sources, t.at);
        let Some(inf) = world.get::<Influence>(t.e) else {
            continue;
        };
        if (inf.power, inf.player, inf.rival) != (t.power, t.shares.player, t.shares.rival) {
            let mut inf = world.get_mut::<Influence>(t.e).expect("checked");
            (inf.power, inf.player, inf.rival) = (t.power, t.shares.player, t.shares.rival);
        }
    }
    towns
}

/// The day's warmth: a `Deed::Influence` toward the player at every town
/// the player doesn't own (not the seat) with [`warmth`] to give.
fn warm(world: &mut World, data: &InfluenceData, towns: &[Town]) {
    let posts: Vec<(u64, f32)> = towns
        .iter()
        .filter(|t| t.owner != FactionId::PLAYER && !t.seat)
        .map(|t| (t.id, warmth(data, t.shares.net(), t.goodwill)))
        .filter(|(_, d)| *d > 0.0)
        .collect();
    let mut ledger = world.resource_mut::<GoodwillLedger>();
    for (id, delta) in posts {
        ledger.push(id, FactionId::PLAYER, delta, Deed::Influence);
    }
}

/// The day's envoys: flags up to `max_envoys_per_day` independent towns
/// (not the seat) at a net of at least `envoy_net`, liking the player
/// enough to invite and with no invitation pending or cooling down, the
/// highest net first (ties by SpawnId); clears every other flag. "Liking
/// enough" counts this dawn's own warmth, which [`warm`] has just posted
/// and the ledger applies on the next tick, before slot 7 next looks.
fn flag_envoys(world: &mut World, data: &InfluenceData, towns: &[Town], tick: u64) {
    let line = world.resource::<Progression>().data.invitations.goodwill;
    let pending: Vec<u64> = world
        .resource::<Offers>()
        .pending
        .iter()
        .filter_map(|o| match o.kind {
            OfferKind::Invitation { settlement } => Some(settlement),
            _ => None,
        })
        .collect();
    let progress = world.resource::<RunProgress>();
    let mut chosen: Vec<(f32, u64)> = towns
        .iter()
        .filter(|t| {
            t.owner == FactionId::INDEPENDENT
                && !t.seat
                && t.shares.net() >= data.envoy_net
                && t.goodwill + warmth(data, t.shares.net(), t.goodwill) >= line
                && !pending.contains(&t.id)
                && tick >= progress.invite_cooldown(t.id)
        })
        .map(|t| (t.shares.net(), t.id))
        .collect();
    chosen.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    chosen.truncate(data.max_envoys_per_day as usize);
    for t in towns {
        let flag = chosen.iter().any(|c| c.1 == t.id);
        if let Some(mut inf) = world.get_mut::<Influence>(t.e)
            && inf.envoy != flag
        {
            inf.envoy = flag;
        }
    }
}

/// The realm's rank (design question 16): the player's champion earns
/// `rank_points` per `rank_people` people the player holds.
fn credit_realm(world: &mut World, data: &InfluenceData, towns: &[Town]) {
    let people = towns
        .iter()
        .filter(|t| t.owner == FactionId::PLAYER)
        .fold(0u32, |n, t| n.saturating_add(t.people));
    let units = realm_units(data, people);
    if units > 0 && data.rank_points > 0 {
        credit_times(world, FactionId::PLAYER, Feat::Realm, units);
    }
}

/// GP-31: the Contest loyalty term at owned town `s` (summed by
/// `tenure::step`): [`contest_value`] of its owner's share against the
/// other side's, from its [`Influence`]. 0.0 at the rival's seat (never
/// swayed), at a town nobody holds, outside a run and when
/// `influence.enabled` is false.
pub fn contest_term(world: &World, s: Entity) -> f32 {
    let data = &world.resource::<Progression>().data.influence;
    if !data.enabled || !world.resource::<RunState>().in_run() {
        return 0.0;
    }
    let (Some(inf), Some(owner), Some(id)) = (
        world.get::<Influence>(s),
        world.get::<Owner>(s),
        world.get::<SpawnId>(s),
    ) else {
        return 0.0;
    };
    if Some(id.0) == world.resource::<RunProgress>().rival_seat {
        return 0.0;
    }
    let (own, other) = match owner.faction {
        FactionId::PLAYER => (inf.player, inf.rival),
        FactionId::RIVAL => (inf.rival, inf.player),
        _ => return 0.0,
    };
    contest_value(data, owner.faction, own, other)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> InfluenceData {
        InfluenceData::default()
    }

    fn source(id: u64, x: f32, faction: FactionId, power: f32) -> Source {
        Source {
            id,
            at: Vec2::new(x, 0.0),
            faction,
            power,
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// GP-31: power is people and loyalty only: it grows with the square
    /// root of people, the loyalty factor runs from the floor (0.25 at
    /// loyalty 0) to 1, the scar takes its share off, and the rival's seat
    /// projects twice. The design's figures: a loyal town of 300 (0.7)
    /// has about 1.34; the seat (450 people, 0.85) about 3.8.
    #[test]
    fn power_is_people_and_loyalty_only() {
        let d = data();
        assert!(near(power(&d, 300, 0.7, 0.0, false), 3f32.sqrt() * 0.775));
        assert!((power(&d, 300, 0.7, 0.0, false) - 1.34).abs() < 0.01);
        assert!((power(&d, 450, 0.85, 0.0, true) - 3.77).abs() < 0.01);
        // The floor: a town at loyalty 0 still projects a quarter.
        let full = power(&d, 400, 1.0, 0.0, false);
        assert!(near(full, 2.0));
        assert!(near(power(&d, 400, 0.0, 0.0, false), 0.25 * full));
        // Four times the people, twice the power.
        assert!(near(power(&d, 1600, 1.0, 0.0, false), 2.0 * full));
        // The scar takes its share; the seat doubles.
        assert!(near(power(&d, 400, 1.0, 0.2, false), 0.8 * full));
        assert!(near(power(&d, 400, 1.0, 0.0, true), d.seat_power * full));
        // Nobody there, no power; loyalty and scar are clamped.
        assert_eq!(power(&d, 0, 1.0, 0.0, false), 0.0);
        assert_eq!(power(&d, 400, 2.0, -1.0, false), full);
        assert_eq!(power(&d, 400, 1.0, 1.5, false), 0.0);
    }

    /// GP-31: a pull falls linearly from the full power at the centre to 0
    /// at `reach` and stays 0 beyond.
    #[test]
    fn the_pull_falls_off_with_distance() {
        let d = data();
        assert_eq!(pull(&d, 2.0, 0.0), 2.0);
        assert!(near(pull(&d, 2.0, d.reach / 2.0), 1.0));
        assert!(near(pull(&d, 2.0, d.reach * 0.75), 0.5));
        assert_eq!(pull(&d, 2.0, d.reach), 0.0);
        assert_eq!(pull(&d, 2.0, d.reach * 3.0), 0.0);
    }

    /// GP-31: the shares count the player's and the rival's pulls against
    /// the place's own independence (0.5), so they never reach 1; with no
    /// source in reach both are 0; other factions add nothing. The design's
    /// scale: one loyal town of 300 gives a neighbour 145 tiles off a net
    /// of about 0.43, two give about 0.6, and the rival's pull mirrors the
    /// player's.
    #[test]
    fn field_at_shares_the_pull() {
        let d = data();
        let at = Vec2::ZERO;
        assert_eq!(field_at(&d, &[], at), Shares::default());
        let far = [source(1, d.reach + 1.0, FactionId::PLAYER, 5.0)];
        assert_eq!(field_at(&d, &far, at), Shares::default(), "out of reach");
        let own = [source(1, 0.0, FactionId::PLAYER, 1.0)];
        let s = field_at(&d, &own, at);
        assert!(near(s.player, 1.0 / 1.5) && s.rival == 0.0, "{s:?}");

        let town = power(&d, 300, 0.7, 0.0, false);
        let one = [source(1, 145.0, FactionId::PLAYER, town)];
        let net1 = field_at(&d, &one, at).net();
        assert!((net1 - 0.43).abs() < 0.01, "one town: {net1}");
        let two = [
            source(1, 145.0, FactionId::PLAYER, town),
            source(2, -145.0, FactionId::PLAYER, town),
        ];
        let net2 = field_at(&d, &two, at).net();
        assert!((net2 - 0.6).abs() < 0.02, "two towns: {net2}");
        assert!(net2 >= d.sphere, "inside the sphere");

        // The rival's pull mirrors the player's.
        let theirs = [source(1, 145.0, FactionId::RIVAL, town)];
        let r = field_at(&d, &theirs, at);
        assert!(near(r.rival, field_at(&d, &one, at).player) && r.player == 0.0);
        assert!(near(r.net(), -net1));

        // Both sides: the shares split the pull and stay under 1.
        let both = [
            source(1, 50.0, FactionId::PLAYER, 2.0),
            source(2, -50.0, FactionId::RIVAL, 2.0),
        ];
        let b = field_at(&d, &both, at);
        assert!(near(b.player, b.rival) && b.net().abs() < 1e-6);
        assert!(b.player + b.rival < 1.0);
        assert_eq!(
            (b.of(FactionId::PLAYER), b.of(FactionId::RIVAL)),
            (b.player, b.rival)
        );
        assert_eq!(b.of(FactionId::INDEPENDENT), 0.0);

        // Independents and raiders hold no land: they add nothing.
        let others = [
            source(1, 0.0, FactionId::INDEPENDENT, 9.0),
            source(2, 0.0, FactionId::RAIDERS, 9.0),
        ];
        assert_eq!(field_at(&d, &others, at), Shares::default());
        // Leaving a town out (the Demand watch): a filtered iterator.
        let without_1 = field_at(&d, two.iter().filter(|s| s.id != 1), at);
        assert!(near(without_1.net(), net1));
    }

    /// GP-31: a source comes only from a town the player or the rival holds
    /// with some power.
    #[test]
    fn a_source_is_a_held_town_with_power() {
        let inf = Influence {
            power: 1.5,
            ..Default::default()
        };
        let c = IVec2::new(10, -4);
        let s = Source::of(7, c, FactionId::RIVAL, &inf).expect("held");
        assert_eq!(
            (s.id, s.at, s.faction, s.power),
            (7, c.as_vec2(), FactionId::RIVAL, 1.5)
        );
        assert!(Source::of(7, c, FactionId::INDEPENDENT, &inf).is_none());
        assert!(Source::of(7, c, FactionId::PLAYER, &Influence::default()).is_none());
    }

    /// GP-31: the Contest term is `-0.25 x` the other side's lead at either
    /// side's towns, +0.05 in the player's heartland (a lead of at least the
    /// sphere's 0.5 at a town of the player's; never at the rival's) and 0
    /// between.
    #[test]
    fn the_contest_follows_the_lead() {
        let d = data();
        for owner in [FactionId::PLAYER, FactionId::RIVAL] {
            assert!(near(contest_value(&d, owner, 0.1, 0.5), -0.1));
            assert!(near(contest_value(&d, owner, 0.0, 0.9), -0.225));
            assert_eq!(contest_value(&d, owner, 0.3, 0.3), 0.0);
            assert_eq!(contest_value(&d, owner, 0.4, 0.1), 0.0);
            assert!(contest_value(&d, owner, 0.1, 0.5).is_sign_negative());
            assert!(
                contest_value(&d, owner, 0.0, 0.0).is_sign_positive(),
                "no -0.0"
            );
        }
        // The heartland is the player's: the rival's towns deep in its own
        // sphere get nothing (LORDSHIP_DESIGN section 6, "your heartland").
        let you = FactionId::PLAYER;
        assert_eq!(contest_value(&d, you, 0.6, 0.1), d.heartland);
        assert_eq!(contest_value(&d, you, 0.7, 0.0), d.heartland);
        assert_eq!(contest_value(&d, FactionId::RIVAL, 0.6, 0.1), 0.0);
        assert_eq!(contest_value(&d, FactionId::RIVAL, 0.9, 0.0), 0.0);
        assert!(contest_value(&d, FactionId::RIVAL, 0.9, 0.0).is_sign_positive());
    }

    /// GP-31: a day's warmth is `0.12 x (net - 0.1)` above the warming
    /// line, never past the 0.6 cap: a free town at net 0.6 warms 0.06 a
    /// day, from 0 to the invitation's 0.5 in about eight days.
    #[test]
    fn warmth_stops_at_the_cap() {
        let d = data();
        assert!(near(warmth(&d, 0.6, 0.0), 0.06));
        assert_eq!(warmth(&d, d.warm_above, 0.0), 0.0);
        assert_eq!(warmth(&d, -0.4, 0.0), 0.0);
        assert!(near(warmth(&d, 0.6, 0.59), 0.01), "up to the cap");
        assert_eq!(warmth(&d, 0.9, d.goodwill_cap), 0.0);
        let mut g = 0.0;
        let mut days = 0;
        while g < 0.5 {
            g += warmth(&d, 0.6, g);
            days += 1;
        }
        assert_eq!(days, 9, "0.06 a day: past 0.5 on the ninth dawn");
        assert!(in_sphere(
            &d,
            &Influence {
                player: 0.6,
                rival: 0.1,
                ..Default::default()
            }
        ));
        assert!(!in_sphere(
            &d,
            &Influence {
                player: 0.6,
                rival: 0.2,
                ..Default::default()
            }
        ));
    }

    /// Design question 16: one rank point a day per 500 people held.
    #[test]
    fn the_realm_counts_whole_units() {
        let d = data();
        assert_eq!(realm_units(&d, 0), 0);
        assert_eq!(realm_units(&d, 499), 0);
        assert_eq!(realm_units(&d, 500), 1);
        assert_eq!(realm_units(&d, 1499), 2);
        assert_eq!(realm_units(&d, u32::MAX), u32::MAX / 500);
    }

    // ---- The step on the run island (seed 42). ----

    use crate::conquest::tests::settlements;
    use crate::conquest::transfer;
    use crate::progression::champion::ChampionRank;
    use crate::progression::tests::run_app;
    use crate::rival::RivalState;
    use sim_core::Cause;

    /// Hands `s` to the player (invited: no scar) with residents' mean
    /// loyalty `loyalty`.
    fn hold(world: &mut World, s: Entity, loyalty: f32) {
        transfer(world, s, FactionId::PLAYER, Cause::Invitation).expect("populated");
        world.get_mut::<SettlementLoyalty>(s).unwrap().mean = loyalty;
    }

    fn influence(world: &World, s: Entity) -> Influence {
        *world.get::<Influence>(s).expect("a run settlement")
    }

    fn goodwill(world: &World, s: Entity) -> f32 {
        world.get::<Goodwill>(s).unwrap().get(FactionId::PLAYER)
    }

    /// Every held town as a source, recomputed by hand, in SpawnId order.
    fn sources_by_hand(world: &mut World, d: &InfluenceData) -> Vec<Source> {
        let seat = world.resource::<RunProgress>().rival_seat;
        let mut q = world.query::<(
            &SpawnId,
            &Settlement,
            &Owner,
            &SettlementEconomy,
            &SettlementLoyalty,
            &Tenure,
        )>();
        let mut v: Vec<Source> = q
            .iter(world)
            .filter(|(_, _, o, ..)| matches!(o.faction, FactionId::PLAYER | FactionId::RIVAL))
            .map(|(id, st, o, se, sl, t)| Source {
                id: id.0,
                at: st.center.as_vec2(),
                faction: o.faction,
                power: power(d, se.population, sl.mean, t.scar, Some(id.0) == seat),
            })
            .filter(|s| s.power > 0.0)
            .collect();
        v.sort_by_key(|s| s.id);
        v
    }

    /// The Influence deeds waiting in the ledger: (settlement, delta).
    fn influence_deeds(world: &World) -> Vec<(u64, f32)> {
        let mut v: Vec<(u64, f32)> = world
            .resource::<GoodwillLedger>()
            .0
            .iter()
            .filter(|c| c.cause == Deed::Influence)
            .inspect(|c| assert_eq!(c.faction, FactionId::PLAYER))
            .map(|c| (c.settlement, c.delta))
            .collect();
        v.sort_by_key(|c| c.0);
        v
    }

    /// The free towns other than `of`, nearest `of`'s centre first.
    fn nearest_free(world: &mut World, of: IVec2) -> Vec<(u64, Entity, IVec2)> {
        let mut v = settlements(world);
        v.retain(|t| t.2 != of);
        v.sort_by(|a, b| {
            let (da, db) = ((a.2 - of).as_vec2().length(), (b.2 - of).as_vec2().length());
            da.total_cmp(&db).then(a.0.cmp(&b.0))
        });
        v
    }

    /// GP-31: the field follows the holdings. A taking refreshes it at once
    /// (the new holding projects for the player in the same tick); the
    /// hourly step writes every town's power and shares as `field_at` gives
    /// them; on the day tick every town the player doesn't own, the seat
    /// excepted, with warmth to give gets exactly that `Deed::Influence`;
    /// and `tenure::step` writes each owned town's Contest term as
    /// `contest_term` reads it (0 at the seat and at free towns).
    #[test]
    fn a_held_town_warms_its_neighbours() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        let d = world.resource::<Progression>().data.influence;
        let (tid, t, tc) = settlements(world)[0];
        let near = nearest_free(world, tc);
        let (a, b) = (near[0].1, near[1].1);
        hold(world, a, 0.9);
        let fresh = influence(world, a);
        assert!(
            fresh.power > 0.0 && fresh.player > 0.0,
            "a taking refreshes the field: {fresh:?}"
        );
        hold(world, b, 0.9);
        assert_eq!(world.resource::<SimTick>().0, 0, "tick 0 is a day tick");
        step(world);
        let sources = sources_by_hand(world, &d);
        let mut q = world.query_filtered::<(Entity, &SpawnId, &Settlement), With<Influence>>();
        let towns: Vec<(Entity, u64, IVec2)> = q
            .iter(world)
            .map(|(e, id, st)| (e, id.0, st.center))
            .collect();
        for &(e, id, c) in &towns {
            let inf = influence(world, e);
            assert_eq!(
                inf.shares(),
                field_at(&d, &sources, c.as_vec2()),
                "town #{id}"
            );
            let own = sources.iter().find(|s| s.id == id).map_or(0.0, |s| s.power);
            assert_eq!(inf.power, own, "town #{id}");
        }
        let at_t = influence(world, t);
        println!(
            "town #{tid}: net {:.3} between two held neighbours",
            at_t.net()
        );
        assert!(at_t.net() > d.warm_above, "warmed: {at_t:?}");

        // The day's warmth: every town not the player's, not the seat.
        let seat = world.resource::<RunProgress>().rival_seat;
        let mut want: Vec<(u64, f32)> = towns
            .iter()
            .filter(|&&(e, id, _)| {
                Some(id) != seat && world.get::<Owner>(e).unwrap().faction != FactionId::PLAYER
            })
            .map(|&(e, id, _)| {
                (
                    id,
                    warmth(&d, influence(world, e).net(), goodwill(world, e)),
                )
            })
            .filter(|(_, w)| *w > 0.0)
            .collect();
        want.sort_by_key(|w| w.0);
        assert!(want.iter().any(|w| w.0 == tid), "the neighbour warms");
        assert_eq!(influence_deeds(world), want);

        // The Contest term, hourly in slot 14.
        crate::tenure::step(world);
        for &(e, id, _) in &towns {
            let term = world
                .get::<sim_agents::LoyaltyTerms>(e)
                .unwrap()
                .get(sim_agents::LoyaltyTerm::Contest);
            assert_eq!(term, contest_term(world, e), "town #{id}");
            let owner = world.get::<Owner>(e).unwrap().faction;
            if Some(id) == seat || owner == FactionId::INDEPENDENT {
                assert_eq!(term, 0.0, "town #{id}");
            } else {
                let inf = influence(world, e);
                let other = if owner == FactionId::PLAYER {
                    FactionId::RIVAL
                } else {
                    FactionId::PLAYER
                };
                let (own, other) = (inf.shares().of(owner), inf.shares().of(other));
                assert_eq!(term, contest_value(&d, owner, own, other), "town #{id}");
                if owner == FactionId::RIVAL {
                    assert!(term <= 0.0, "no heartland at the rival's #{id}");
                }
            }
        }

        // Off the day tick the field still moves, but nobody warms.
        world.resource_mut::<GoodwillLedger>().0.clear();
        let tph = world.resource::<Calendar>().ticks_per_hour();
        world.resource_mut::<SimTick>().0 = tph;
        step(world);
        assert!(influence_deeds(world).is_empty());
        // Switched off, it does nothing at all, even at dawn.
        world.resource_mut::<Progression>().data.influence.enabled = false;
        let day = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = day;
        world.get_mut::<SettlementLoyalty>(a).unwrap().mean = 0.2;
        let before = influence(world, a);
        step(world);
        assert_eq!(influence(world, a), before, "no refresh");
        assert!(influence_deeds(world).is_empty());
        assert_eq!(contest_term(world, a), 0.0);
    }

    /// A run of seed 42 with the independence shrunk so every free town
    /// near a holding lies deep in the player's sphere: the free town
    /// farthest from the rival's seat is the player's (loyalty 1.0), and
    /// up to three free towns within 150 tiles of it and out of every
    /// captain's reach are returned, nearest first.
    fn sphere_with_candidates() -> (bevy_app::App, Vec<(u64, Entity, IVec2)>) {
        let (mut app, start) = run_app(42);
        let world = app.world_mut();
        world
            .resource_mut::<Progression>()
            .data
            .influence
            .independence = 0.01;
        let rules = world.resource::<Progression>().data.invitations;
        let home = world.get::<sim_core::Position>(start.champion).unwrap().0;
        let seat_at = world.resource::<RivalState>().seat_center;
        let (_, h, hc) = settlements(world)
            .into_iter()
            .max_by(|a, b| {
                let (da, db) = ((a.2 - seat_at).as_vec2(), (b.2 - seat_at).as_vec2());
                da.length().total_cmp(&db.length()).then(b.0.cmp(&a.0))
            })
            .unwrap();
        hold(world, h, 1.0);
        let far_from_captains = |c: IVec2| (c - home).as_vec2().length() > rules.radius + 5.0;
        let liked: Vec<(u64, Entity, IVec2)> = nearest_free(world, hc)
            .into_iter()
            .filter(|t| far_from_captains(t.2) && (t.2 - hc).as_vec2().length() < 150.0)
            .take(3)
            .collect();
        assert!(liked.len() >= 2, "candidates: {liked:?}");
        (app, liked)
    }

    /// The SpawnIds of the towns flagged to send envoys.
    fn flagged(world: &mut World) -> Vec<u64> {
        let mut q = world.query::<(&SpawnId, &Influence)>();
        let mut v: Vec<u64> = q
            .iter(world)
            .filter(|(_, i)| i.envoy)
            .map(|(id, _)| id.0)
            .collect();
        v.sort_unstable();
        v
    }

    /// GP-31: envoys come without a captain, at most one a day. Several
    /// free towns deep in the player's sphere and far from any captain
    /// like the player enough; the day tick flags only the highest net,
    /// and the invitation step posts its invitation alone ("sends
    /// envoys"), spending the flag, never a second that day. The next dawn
    /// flags another (the first is pending); two a day when the data says
    /// two.
    #[test]
    fn envoys_come_without_a_captain() {
        let (mut app, liked) = sphere_with_candidates();
        let world = app.world_mut();
        let rules = world.resource::<Progression>().data.invitations;
        for t in &liked {
            world
                .get_mut::<Goodwill>(t.1)
                .unwrap()
                .set(FactionId::PLAYER, rules.goodwill);
        }
        step(world);
        let nets: Vec<(u64, f32)> = liked
            .iter()
            .map(|t| (t.0, influence(world, t.1).net()))
            .collect();
        println!("candidates (id, net): {nets:?}");
        assert!(nets.iter().all(|n| n.1 >= 0.5), "all inside the sphere");
        let best = nets
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .unwrap()
            .0;
        assert_eq!(flagged(world), vec![best], "one a day, the highest net");

        let invitations = |world: &World| -> Vec<u64> {
            world
                .resource::<Offers>()
                .pending
                .iter()
                .filter_map(|o| match o.kind {
                    OfferKind::Invitation { settlement } => Some(settlement),
                    _ => None,
                })
                .collect()
        };
        let tph = world.resource::<Calendar>().ticks_per_hour();
        for hour in 1..4 {
            world.resource_mut::<SimTick>().0 = hour * tph;
            crate::invitations::step(world);
            assert_eq!(
                invitations(world),
                vec![best],
                "hour {hour}: not two in a day"
            );
        }
        assert!(flagged(world).is_empty(), "the flag is spent");
        let said = world
            .resource::<crate::components::MilitaryLog>()
            .entries
            .iter()
            .any(|(_, l)| l.contains("sends envoys"));
        assert!(said, "the log says envoys came");

        // The next dawn: the first is pending, so another is flagged.
        let day = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = day;
        step(world);
        let second = flagged(world);
        assert_eq!(second.len(), 1);
        assert_ne!(second, vec![best]);
        // Two a day when the data says so.
        world
            .resource_mut::<Progression>()
            .data
            .influence
            .max_envoys_per_day = 2;
        world.resource_mut::<SimTick>().0 = 2 * day;
        step(world);
        assert_eq!(flagged(world).len(), 2.min(liked.len() - 1));
        assert!(!flagged(world).contains(&best), "pending: {best}");
    }

    /// GP-31: the dawn's own warmth counts toward the invitation line. A
    /// town just under it (by less than the day's warmth) is flagged that
    /// dawn, since its warmth is applied before the invitation step next
    /// looks; one further under than a day's warmth is not.
    #[test]
    fn envoys_count_the_dawns_warmth() {
        let (mut app, liked) = sphere_with_candidates();
        let world = app.world_mut();
        let line = world.resource::<Progression>().data.invitations.goodwill;
        let d = world.resource::<Progression>().data.influence;
        let (close, far) = (liked[0], liked[1]);
        let set = |world: &mut World, s: Entity, g: f32| {
            world
                .get_mut::<Goodwill>(s)
                .unwrap()
                .set(FactionId::PLAYER, g);
        };
        // Any net past `envoy_net` warms at least 0.12 x 0.4 = 0.048 a day,
        // and no net warms more than 0.12 x 0.9 = 0.108.
        set(world, close.1, line - 0.02);
        set(world, far.1, line - 0.2);
        step(world);
        let net = influence(world, close.1).net();
        assert!(net >= d.envoy_net, "{net}");
        let w = warmth(&d, net, line - 0.02);
        assert!(w > 0.02, "{w}");
        assert!(influence_deeds(world).contains(&(close.0, w)), "posted");
        assert_eq!(flagged(world), vec![close.0], "just under, with its warmth");
    }

    /// GP-31: a flag is the day's. A town flagged yesterday that never sent
    /// its envoys (its goodwill stayed under the line all day) and comes to
    /// like the player just before dawn sends none at the dawn tick, where
    /// the invitation step runs before the new day's flags are set (else a
    /// stale flag at dawn and the new day's an hour later would make two
    /// in a day); off the day tick a flag is honoured.
    #[test]
    fn a_stale_envoy_flag_sends_no_one_at_dawn() {
        let (mut app, liked) = sphere_with_candidates();
        let world = app.world_mut();
        let line = world.resource::<Progression>().data.invitations.goodwill;
        let (id, town, _) = liked[0];
        world.get_mut::<Influence>(town).unwrap().envoy = true;
        world
            .get_mut::<Goodwill>(town)
            .unwrap()
            .set(FactionId::PLAYER, line);
        let invited = |world: &World| {
            world
                .resource::<Offers>()
                .pending
                .iter()
                .any(|o| o.kind == OfferKind::Invitation { settlement: id })
        };
        let day = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = day;
        crate::invitations::step(world);
        assert!(!invited(world), "a stale flag at the dawn tick");
        let tph = world.resource::<Calendar>().ticks_per_hour();
        world.resource_mut::<SimTick>().0 = day + tph;
        crate::invitations::step(world);
        assert!(invited(world), "a flag off the day tick sends envoys");
        assert!(!influence(world, town).envoy, "and is spent");
    }

    /// GP-31: the seat is never swayed. The player holds every other town
    /// within reach of the rival's seat, the seat projects nothing
    /// (`seat_power` 0) and a town's own independence is slight, so the
    /// player's pull would sway any other town there: it would warm, a
    /// rival's town would feel the pull, and a free one liking the player
    /// would send envoys. Still the seat gets no influence deed and no
    /// Contest term while the rival holds it, and, gone free by a revolt
    /// (no rule keeps it from one), no deed and no envoys either.
    #[test]
    fn the_seat_is_never_swayed() {
        let (mut app, _) = run_app(42);
        let world = app.world_mut();
        {
            let mut p = world.resource_mut::<Progression>();
            p.data.influence.independence = 0.01;
            p.data.influence.seat_power = 0.0;
        }
        let d = world.resource::<Progression>().data.influence;
        let line = world.resource::<Progression>().data.invitations.goodwill;
        let seat_at = world.resource::<RivalState>().seat_center;
        let seat_id = world.resource::<RunProgress>().rival_seat.expect("a seat");
        let seat = crate::orders::find_settlement(world, seat_id).expect("the seat");
        let mut q = world.query_filtered::<(Entity, &SpawnId, &Settlement), With<Influence>>();
        let mut around: Vec<(u64, Entity)> = q
            .iter(world)
            .filter(|(_, id, st)| {
                id.0 != seat_id && (st.center - seat_at).as_vec2().length() <= d.reach
            })
            .map(|(e, id, _)| (id.0, e))
            .collect();
        around.sort_unstable_by_key(|t| t.0);
        assert!(around.len() >= 3, "{around:?}");
        for &(_, e) in &around {
            hold(world, e, 1.0);
        }
        world
            .get_mut::<Goodwill>(seat)
            .unwrap()
            .set(FactionId::PLAYER, line + 0.05);
        step(world);
        let inf = influence(world, seat);
        println!("the seat, {} towns around it held: {inf:?}", around.len());
        // Left to the formulas, any other town there would be swayed.
        assert!(inf.net() >= d.envoy_net, "{inf:?}");
        assert!(warmth(&d, inf.net(), line + 0.05) > 0.0);
        assert!(contest_value(&d, FactionId::RIVAL, inf.rival, inf.player) < 0.0);
        // The rival's seat: no deed, no Contest term.
        assert!(influence_deeds(world).iter().all(|d| d.0 != seat_id));
        assert_eq!(contest_term(world, seat), 0.0);
        crate::tenure::step(world);
        let terms = *world.get::<sim_agents::LoyaltyTerms>(seat).unwrap();
        assert_eq!(terms.get(sim_agents::LoyaltyTerm::Contest), 0.0);

        // Gone free by a revolt, at the next dawn: still no deed and no
        // envoys, though it likes the player above the invitation line.
        transfer(world, seat, FactionId::INDEPENDENT, Cause::Revolt).expect("populated");
        assert!(
            world.resource::<RunState>().in_run(),
            "a revolt ends nothing"
        );
        world.resource_mut::<GoodwillLedger>().0.clear();
        let day = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = day;
        step(world);
        let inf = influence(world, seat);
        println!("the seat gone free: {inf:?}");
        assert!(inf.net() >= d.envoy_net && goodwill(world, seat) >= line);
        assert!(!inf.envoy, "the seat sends no envoys");
        assert!(influence_deeds(world).iter().all(|d| d.0 != seat_id));
        assert_eq!(contest_term(world, seat), 0.0);
    }

    /// Design question 16: a realm ranks the champion. Once a day (and only
    /// on the day tick), the player's champion earns `rank_points` per 500
    /// people the player holds.
    #[test]
    fn a_realm_ranks_the_champion() {
        let (mut app, start) = run_app(42);
        let world = app.world_mut();
        let d = world.resource::<Progression>().data.influence;
        let mut people = 0;
        for (_, e, _) in settlements(world) {
            if people >= 1_000 {
                break;
            }
            hold(world, e, 0.5);
            people += world.get::<SettlementEconomy>(e).unwrap().population;
        }
        let units = realm_units(&d, people);
        assert!(units >= 2, "{people} people");
        let points = |world: &World| world.get::<ChampionRank>(start.champion).unwrap().points;
        let before = points(world);
        let tph = world.resource::<Calendar>().ticks_per_hour();
        world.resource_mut::<SimTick>().0 = tph;
        step(world);
        assert_eq!(points(world), before, "not a day tick");
        let day = world.resource::<Calendar>().ticks_per_day;
        world.resource_mut::<SimTick>().0 = day;
        step(world);
        println!(
            "{people} people held: +{} rank points",
            points(world) - before
        );
        assert_eq!(points(world), before + d.rank_points * units);
    }

    /// `save_fields!` writes every field: an influence round-trips with its
    /// envoy flag.
    #[test]
    fn influence_round_trips() {
        let inf = Influence {
            power: 1.25,
            player: 0.5,
            rival: 0.125,
            envoy: true,
        };
        let (ids, entities) = Default::default();
        let mut w = sim_core::SaveWriter::new(&ids);
        sim_core::SaveState::save(&inf, &mut w);
        let bytes = w.into_bytes();
        let mut r = sim_core::SaveReader::new(&bytes, &entities);
        let back: Influence = sim_core::SaveState::load(&mut r).unwrap();
        assert_eq!(back, inf);
    }
}
