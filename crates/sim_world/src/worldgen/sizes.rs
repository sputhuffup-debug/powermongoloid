//! Settlement size draws (GP-21): every settlement of the run island gets a
//! tier and a number of people; the largest becomes the rival's seat.

use rand::RngExt;
use sim_core::SimSeed;
use sim_core::rng::rng_key;

use crate::settings::SizeSettings;
use crate::{SettlementSize, SizeTier};

/// Draws a size for each of `n` settlements, in index order (D9 stream
/// `sim_world::gen::sizes`, tick 0). With no tiers it draws nothing and
/// returns `None` for every settlement (the `developed` preset).
///
/// - Tier counts follow the weights as shares of `n` (largest remainder,
///   ties to the earlier tier), so the island's total population is
///   stable from seed to seed. The tiers are then shuffled over the
///   settlements.
/// - People are uniform in the tier's `min..=max`.
/// - The seat is the largest settlement (ties to the lowest index). It is
///   raised to at least `seat_min` and one more than any other settlement,
///   and gets the top tier (the one with the largest `max`), so it is
///   always the unique largest.
pub fn draw_sizes(seed: SimSeed, s: &SizeSettings, n: usize) -> Vec<Option<SettlementSize>> {
    if s.tiers.is_empty() || n == 0 {
        return vec![None; n];
    }
    let total: f32 = s.tiers.iter().map(|t| t.weight).sum();
    let quotas: Vec<f32> = s
        .tiers
        .iter()
        .map(|t| n as f32 * t.weight / total)
        .collect();
    let mut counts: Vec<usize> = quotas.iter().map(|q| q.floor() as usize).collect();
    let assigned: usize = counts.iter().sum();
    let mut by_remainder: Vec<usize> = (0..quotas.len()).collect();
    by_remainder.sort_by(|&a, &b| {
        let (ra, rb) = (quotas[a] - quotas[a].floor(), quotas[b] - quotas[b].floor());
        rb.total_cmp(&ra).then(a.cmp(&b))
    });
    for &i in by_remainder.iter().cycle().take(n.saturating_sub(assigned)) {
        counts[i] += 1;
    }

    let mut deck: Vec<usize> = Vec::with_capacity(n);
    for (i, &c) in counts.iter().enumerate() {
        deck.extend(std::iter::repeat_n(i, c));
    }
    deck.truncate(n);
    let mut rng = seed.stream(rng_key("sim_world::gen::sizes"), 0);
    for i in (1..deck.len()).rev() {
        let j = rng.random_range(0..=i);
        deck.swap(i, j);
    }
    let mut sizes: Vec<SettlementSize> = deck
        .iter()
        .map(|&t| {
            let rule = &s.tiers[t];
            SettlementSize {
                tier: rule.tier,
                people: rng.random_range(rule.min..=rule.max),
            }
        })
        .collect();

    // The seat: the largest, ties to the lowest index.
    let seat = (0..n)
        .max_by(|&a, &b| sizes[a].people.cmp(&sizes[b].people).then(b.cmp(&a)))
        .expect("n > 0");
    let others = (0..n)
        .filter(|&i| i != seat)
        .map(|i| sizes[i].people)
        .max()
        .unwrap_or(0);
    let top: SizeTier = s
        .tiers
        .iter()
        .enumerate()
        .max_by(|(ia, a), (ib, b)| a.max.cmp(&b.max).then(ib.cmp(ia)))
        .map(|(_, r)| r.tier)
        .expect("tiers non-empty");
    sizes[seat] = SettlementSize {
        tier: top,
        people: sizes[seat].people.max(s.seat_min).max(others + 1),
    };
    sizes.into_iter().map(Some).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SizeTierRule;

    fn settings() -> SizeSettings {
        let rule = |tier, weight, min, max| SizeTierRule {
            tier,
            weight,
            min,
            max,
        };
        SizeSettings {
            tiers: vec![
                rule(SizeTier::Hamlet, 0.2, 20, 60),
                rule(SizeTier::Village, 0.45, 60, 200),
                rule(SizeTier::Town, 0.35, 200, 400),
            ],
            seat_min: 450,
        }
    }

    #[test]
    fn no_tiers_draws_nothing() {
        let s = SizeSettings {
            tiers: Vec::new(),
            seat_min: 0,
        };
        assert_eq!(draw_sizes(SimSeed(1), &s, 5), vec![None; 5]);
    }

    #[test]
    fn tiers_follow_their_shares_and_the_seat_is_the_largest() {
        let s = settings();
        for seed in 0..20 {
            let sizes: Vec<SettlementSize> = draw_sizes(SimSeed(seed), &s, 25)
                .into_iter()
                .map(|z| z.expect("drawn"))
                .collect();
            let count = |t| sizes.iter().filter(|z| z.tier == t).count();
            // 25 x (0.2, 0.45, 0.35) = 5, 11.25, 8.75 -> 5, 11, 9.
            assert_eq!(
                (
                    count(SizeTier::Hamlet),
                    count(SizeTier::Village),
                    count(SizeTier::Town)
                ),
                (5, 11, 9)
            );
            let max = sizes.iter().map(|z| z.people).max().unwrap();
            let seats: Vec<_> = sizes.iter().filter(|z| z.people == max).collect();
            assert_eq!(seats.len(), 1, "seed {seed}: the seat is unique");
            assert!(max >= s.seat_min);
            for z in &sizes {
                if z.people != max {
                    let rule = s.tiers.iter().find(|r| r.tier == z.tier).unwrap();
                    assert!((rule.min..=rule.max).contains(&z.people));
                }
            }
        }
    }

    #[test]
    fn draws_are_seeded() {
        let s = settings();
        assert_eq!(
            draw_sizes(SimSeed(7), &s, 25),
            draw_sizes(SimSeed(7), &s, 25)
        );
        assert_ne!(
            draw_sizes(SimSeed(7), &s, 25),
            draw_sizes(SimSeed(8), &s, 25)
        );
    }
}
