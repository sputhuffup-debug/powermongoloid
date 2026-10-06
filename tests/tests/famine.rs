//! M2 acceptance: cutting off a village's farmers causes measurable hunger
//! within 2 game days, and only in that village.

use sim_core::{Calendar, SimStepper};
use sim_headless::stats::{VillageStats, collect};
use sim_headless::{HeadlessConfig, cut_off_farmers, headless_app};

const SEED: u64 = 42;

fn villages_after_two_days(cut: Option<usize>) -> (Vec<VillageStats>, u32) {
    let mut app = headless_app(HeadlessConfig::new(SEED));
    let mut affected = 0;
    if let Some(i) = cut {
        let target = collect(&mut app).villages[i]
            .settlement
            .expect("settlement");
        affected = cut_off_farmers(&mut app, target);
    }
    app.step(2 * Calendar::default_data().ticks_per_day);
    (collect(&mut app).villages, affected)
}

#[test]
fn cutting_off_farmers_starves_that_village_within_two_days() {
    let (control, _) = villages_after_two_days(None);
    // The largest village: the most farmers to lose.
    let target = control
        .iter()
        .enumerate()
        .max_by_key(|(i, v)| (v.population, std::cmp::Reverse(*i)))
        .map(|(i, _)| i)
        .unwrap();
    let (cut, farmers) = villages_after_two_days(Some(target));
    assert!(farmers > 50, "only {farmers} farmers cut off");

    let (c, x) = (&control[target], &cut[target]);
    println!(
        "{}: control hunger {:.2} food {:.2} days; cut off hunger {:.2} food {:.2} days",
        c.name, c.mean_hunger, c.food_days, x.mean_hunger, x.food_days
    );
    assert!(
        x.food_days < 0.5,
        "{} still has {:.2} days of food",
        x.name,
        x.food_days
    );
    assert!(
        x.mean_hunger >= 0.4,
        "{} isn't hungry: {:.2}",
        x.name,
        x.mean_hunger
    );
    assert!(
        x.mean_hunger >= c.mean_hunger + 0.15,
        "hunger {:.2} vs control {:.2}",
        x.mean_hunger,
        c.mean_hunger
    );

    // The famine stays local: other villages are untouched.
    for (i, (c, x)) in control.iter().zip(&cut).enumerate() {
        if i != target {
            assert!(
                (c.mean_hunger - x.mean_hunger).abs() < 0.02,
                "{} changed: {:.3} vs {:.3}",
                c.name,
                c.mean_hunger,
                x.mean_hunger
            );
        }
    }
}
