//! M4a step 6 (GP-31, GP-33, UI-12): what a taking leaves behind, in words.
//! The rival's pending vow of reprisal, the broken men abroad and the
//! rival's realm with the seat's defence.
//!
//! [`FalloutView`] is refreshed twice a second by one exclusive system
//! ([`refresh`]): `sim_military::rival::realm` reads the whole world and so
//! cannot run in an ordinary system. The words are pure functions (tested
//! without a world); the run bar, the labels, the seat's panel and the feed
//! read the view. Nothing here writes simulation state.

use bevy::prelude::*;
use sim_core::{Calendar, NameTable, RunState, SpawnId};
use sim_military::rival::{self, Realm};
use sim_military::{Bandits, RivalState};
use sim_world::Settlement;

/// How often the view is recomputed (real seconds).
const REFRESH_SECS: f64 = 0.5;

/// Adds the [`FalloutView`] and its refresh.
pub(crate) struct FalloutViewPlugin;

impl Plugin for FalloutViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FalloutView>()
            .add_systems(Update, refresh.in_set(crate::UiSet::Hud));
    }
}

/// The words the UI shows about the fallout, ready to place (UI side only).
#[derive(Resource, Default, Debug, PartialEq)]
pub(crate) struct FalloutView {
    stamp: f64,
    /// The town (SpawnId) a pending vow names, and its label mark.
    vow_town: Option<(u64, String)>,
    /// The run bar's vow line.
    pub vow_line: Option<String>,
    /// "Broken men abroad: N", while bands are alive.
    pub bands_line: Option<String>,
    /// The rival's realm, for the run bar and the seat's panel.
    pub realm_line: Option<String>,
    /// The rival's seat (SpawnId).
    pub seat: Option<u64>,
}

impl FalloutView {
    /// The mark over town `id`'s label while a vow names it.
    pub fn vow_mark(&self, id: u64) -> Option<&str> {
        self.vow_town
            .as_ref()
            .filter(|(t, _)| *t == id)
            .map(|(_, m)| m.as_str())
    }

    /// The realm line if town `id` is the rival's seat.
    pub fn seat_realm(&self, id: u64) -> Option<&str> {
        if self.seat == Some(id) {
            self.realm_line.as_deref()
        } else {
            None
        }
    }
}

/// "day 7, 18:00": the game day (counted from 1) and hour of `tick`.
pub(crate) fn when(tick: u64, cal: &Calendar) -> String {
    let reading = cal.at(tick);
    let (hour, _) = reading.hours_minutes();
    format!("day {}, {hour:02}:00", reading.day + 1)
}

/// The run bar's line for a pending vow.
pub(crate) fn vow_line(town: &str, due: &str) -> String {
    format!("Vow: the rival will retake {town} by {due}")
}

/// The mark over a vowed town's label.
pub(crate) fn vow_mark(due: &str) -> String {
    format!("[vow: {due}]")
}

/// The run bar's count of bands abroad (none: no line).
pub(crate) fn bands_line(bands: usize) -> Option<String> {
    (bands > 0).then(|| format!("Broken men abroad: {bands}"))
}

/// 1150 as "1,150".
fn thousands(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "The rival's realm: 4 of 9 towns, 1,150 people (45%) · seat defence
/// +0.50 (its clock says +0.75)". The clock's figure is left out while the
/// realm is whole and the two agree.
pub(crate) fn realm_line(realm: &Realm, defence: f32, clock: f32) -> String {
    let mut s = format!(
        "The rival's realm: {} of {} towns, {} people ({:.0}%) \u{b7} seat defence {defence:+.2}",
        realm.towns_held,
        realm.towns,
        thousands(realm.people_held),
        realm.share() * 100.0
    );
    if (defence - clock).abs() >= 0.005 {
        s += &format!(" (its clock says {clock:+.2})");
    }
    s
}

/// The vow's feed sentence: the chronicle says "within N hours"; the feed
/// gives the hour it is due instead ("by day 7, 18:00").
pub(crate) fn vow_feed_text(text: &str, hours: i32, due: &str) -> String {
    text.replacen(&format!(" within {hours} hours"), &format!(" by {due}"), 1)
}

/// Recomputes the [`FalloutView`] twice a second, in a run.
fn refresh(world: &mut World) {
    let _span = info_span!("game_ui::fallout::refresh").entered();
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if now - world.resource::<FalloutView>().stamp < REFRESH_SECS {
        return;
    }
    let ready = world.contains_resource::<RivalState>()
        && world.contains_resource::<Calendar>()
        && world.contains_resource::<NameTable>()
        && world.contains_resource::<RunState>();
    let mut view = FalloutView {
        stamp: now,
        ..default()
    };
    if ready && world.resource::<RunState>().in_run() {
        let cal = world.resource::<Calendar>().clone();
        let state = world.resource::<RivalState>();
        let (reprisal, seat, defence) = (state.reprisal, state.seat, state.defence_bonus);
        if let Some(vow) = reprisal {
            let mut q = world.query::<(&SpawnId, &Settlement)>();
            let name = q
                .iter(world)
                .find(|(id, _)| id.0 == vow.settlement)
                .map(|(_, s)| s.name)
                .map(|n| world.resource::<NameTable>().get(n).to_string());
            let due = when(vow.due, &cal);
            view.vow_line = Some(vow_line(name.as_deref().unwrap_or("a town"), &due));
            view.vow_town = Some((vow.settlement, vow_mark(&due)));
        }
        let mut bands = world.query_filtered::<(), With<Bandits>>();
        view.bands_line = bands_line(bands.iter(world).count());
        view.seat = seat;
        if let (Some(realm), Some(data)) = (rival::realm(world), rival::clock_data(world)) {
            let tick = world.resource::<sim_core::SimTick>().0;
            let day = tick as f32 / cal.ticks_per_day as f32;
            let clock = data.defence_on(day) * rival::strength(world);
            view.realm_line = Some(realm_line(&realm, defence, clock));
        }
    }
    *world.resource_mut::<FalloutView>() = view;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vow_names_its_town_and_hour() {
        let cal = Calendar::default_data();
        let tick = cal.ticks_per_day * 6 + cal.ticks_per_day * 3 / 4;
        let due = when(tick, &cal);
        assert!(due.starts_with("day 7, "), "{due}");
        assert!(due.ends_with(":00"), "{due}");
        assert_eq!(
            vow_line("Fenstead", "day 7, 18:00"),
            "Vow: the rival will retake Fenstead by day 7, 18:00"
        );
        assert_eq!(vow_mark("day 7, 18:00"), "[vow: day 7, 18:00]");
    }

    #[test]
    fn the_feed_gives_the_hour_a_vow_is_due() {
        let told =
            "The rival vowed to retake Fenstead within 30 hours (Fenstead was taken by the sword)";
        assert_eq!(
            vow_feed_text(told, 30, "day 7, 18:00"),
            "The rival vowed to retake Fenstead by day 7, 18:00 (Fenstead was taken by the sword)"
        );
        // Words that don't match are left alone.
        assert_eq!(
            vow_feed_text("The rival sulked", 30, "day 7, 18:00"),
            "The rival sulked"
        );
    }

    #[test]
    fn bands_abroad_are_counted_or_silent() {
        assert_eq!(bands_line(0), None);
        assert_eq!(bands_line(3).as_deref(), Some("Broken men abroad: 3"));
    }

    #[test]
    fn the_realm_reads_towns_people_and_defence() {
        let realm = Realm {
            towns_held: 4,
            towns: 9,
            people_held: 450,
            people: 1000,
        };
        assert_eq!(
            realm_line(&realm, 0.5, 0.75),
            "The rival's realm: 4 of 9 towns, 450 people (45%) \u{b7} seat defence +0.50 (its clock says +0.75)"
        );
        let whole = Realm {
            towns_held: 9,
            towns: 9,
            people_held: 1150,
            people: 1150,
        };
        let s = realm_line(&whole, 0.75, 0.75);
        assert!(
            s.contains("1,150 people (100%)") && !s.contains("clock"),
            "{s}"
        );
    }

    #[test]
    fn thousands_get_commas() {
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(1150), "1,150");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn the_view_marks_only_the_vowed_town_and_the_seat() {
        let v = FalloutView {
            vow_town: Some((5, "[vow: day 2, 06:00]".into())),
            realm_line: Some("realm".into()),
            seat: Some(9),
            ..default()
        };
        assert_eq!(v.vow_mark(5), Some("[vow: day 2, 06:00]"));
        assert_eq!(v.vow_mark(6), None);
        assert_eq!(v.seat_realm(9), Some("realm"));
        assert_eq!(v.seat_realm(5), None);
    }
}
