//! Offer cards (GP-27): one card per choice of an offer, shown on the
//! new-game screen (the start boon) and from the run bar (dominion boons,
//! traits, invitations, sellswords for hire). Clicking a card sends
//! `ChooseOffer` through [`crate::army::send`]; the sim validates it, and
//! an offer whose pick it refused (a hire the champion can't make yet)
//! shows again a tick later ([`SentOffers`]).

use std::collections::BTreeMap;

use bevy::prelude::*;
use sim_core::{CommandKind, Offer, OfferKind, Stat};
use sim_military::Progression;
use sim_military::invitations::SwornTerms;

use crate::style;
use crate::{UiScreen, UiSet};

pub struct CardsPlugin;

impl Plugin for CardsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SentOffers>()
            .add_systems(
                Update,
                forget_sent
                    .run_if(game_render::epoch_changed)
                    .in_set(UiSet::Screens),
            )
            .add_systems(Update, choice_buttons.in_set(UiSet::Hud));
    }
}

/// A new world (`WorldEpoch`): offer ids restart per run, so the answered
/// ones are forgotten.
fn forget_sent(mut sent: ResMut<SentOffers>) {
    let _span = info_span!("game_ui::cards::forget_sent").entered();
    sent.0.clear();
}

/// Offers the player has answered this session, with the `SimTick` at
/// which the answer was sent, so the UI hides them until the sim takes them
/// (a double click sends one command, not two). The sim applies the answer
/// on that tick; if it refused it (GP-27: a hire too far away or too dear),
/// the offer is still pending afterwards and shows again.
#[derive(Resource, Default)]
pub struct SentOffers(pub BTreeMap<u64, u64>);

impl SentOffers {
    /// Whether offer `offer` is hidden at `tick`: answered, and the tick
    /// that applies the answer hasn't run yet.
    pub fn hides(&self, offer: u64, tick: u64) -> bool {
        self.0.get(&offer).is_some_and(|&t| tick <= t)
    }
}

/// A clickable card.
#[derive(Component, Clone, Copy)]
pub struct ChoiceButton {
    pub offer: u64,
    pub choice: u8,
}

/// Formats a stat change: percentages, except flat stats.
pub fn stat_text(stat: Stat, v: f32) -> String {
    match stat {
        Stat::CaptainSlots => format!("{v:+.0} {}", stat.name()),
        Stat::ThreatRadius => format!("{v:+.0} tiles {}", stat.name()),
        _ => format!("{:+.0}% {}", v * 100.0, stat.name()),
    }
}

/// A short title for an offer's button in the run bar.
pub fn offer_title(offer: &Offer, place: &dyn Fn(u64) -> String) -> String {
    match offer.kind {
        OfferKind::StartBoon => "Start boon".into(),
        OfferKind::DominionBoon { region, .. } => format!("Boon (region {region})"),
        OfferKind::Trait { .. } => "New trait".into(),
        OfferKind::Invitation { settlement } => format!("Invitation: {}", place(settlement)),
        OfferKind::Hire { men, .. } => format!("Sellswords ({men})"),
    }
}

/// The (title, body) of one card: `card` is the offer's choice value.
/// `sworn` gives what accepting a town's invitation would make it (M4a,
/// GP-32), when the view has it.
pub fn card_text(
    kind: OfferKind,
    choice: u8,
    card: u16,
    progression: &Progression,
    place: &dyn Fn(u64) -> String,
    sworn: &dyn Fn(u64) -> Option<SwornTerms>,
) -> (String, String) {
    match kind {
        OfferKind::StartBoon | OfferKind::DominionBoon { .. } => match progression.boon(card) {
            Some(b) => (b.name.clone(), b.text.clone()),
            None => (format!("Boon {card}"), String::new()),
        },
        OfferKind::Trait { .. } => match progression.trait_data(card) {
            Some(t) => {
                let effects: Vec<String> =
                    t.effects.iter().map(|&(s, v)| stat_text(s, v)).collect();
                (
                    t.name.clone(),
                    format!("One {}.\n{}", t.phrase, effects.join(", ")),
                )
            }
            None => (format!("Trait {card}"), String::new()),
        },
        OfferKind::Invitation { settlement } => {
            if choice == 0 {
                let body = match sworn(settlement) {
                    Some(terms) => crate::panels::take::sworn_words(&place(settlement), &terms),
                    None => format!("Become the lord of {}.", place(settlement)),
                };
                ("Accept".into(), body)
            } else {
                ("Decline".into(), "Leave them be, for now.".into())
            }
        }
        OfferKind::Hire { men, price, .. } => {
            if choice == 0 {
                let r = progression.data.hire.radius;
                let armed = if progression.data.hire.armed {
                    "armed "
                } else {
                    ""
                };
                (
                    "Hire".into(),
                    format!(
                        "{men} {armed}sellswords join your champion's army for {price} food \
                         from its baggage. Your champion must be within {r:.0} tiles of \
                         their camp. Then they eat with you."
                    ),
                )
            } else {
                (
                    "Decline".into(),
                    "Send them on their way. They leave at once.".into(),
                )
            }
        }
    }
}

/// Spawns the cards of `offer` side by side under `parent`.
pub fn spawn_cards(
    parent: &mut ChildSpawnerCommands,
    offer: &Offer,
    progression: &Progression,
    place: &dyn Fn(u64) -> String,
    sworn: &dyn Fn(u64) -> Option<SwornTerms>,
) {
    parent
        .spawn(Node {
            column_gap: Val::Px(12.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            for (i, &card) in offer.choices.iter().enumerate() {
                let (title, body) = card_text(offer.kind, i as u8, card, progression, place, sworn);
                row.spawn((
                    Button,
                    ChoiceButton {
                        offer: offer.id,
                        choice: i as u8,
                    },
                    Node {
                        width: Val::Px(200.0),
                        min_height: Val::Px(120.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BorderColor::all(style::BUTTON_ACTIVE),
                    BackgroundColor(style::CARD),
                ))
                .with_children(|c| {
                    c.spawn((Text::new(title), style::font(17.0), TextColor(style::GOLD)));
                    c.spawn((Text::new(body), style::font(13.0), TextColor(style::TEXT)));
                });
            }
        });
}

/// A card was clicked: send the choice once, and leave the new-game screen
/// if that is where it was.
fn choice_buttons(
    buttons: Query<(&Interaction, &ChoiceButton), Changed<Interaction>>,
    screen: Res<State<UiScreen>>,
    mut next: ResMut<NextState<UiScreen>>,
    mut sent: ResMut<SentOffers>,
    tick: Option<Res<sim_core::SimTick>>,
    mut time: ResMut<Time<Virtual>>,
    mut out: MessageWriter<sim_core::SimCommand>,
) {
    let _span = info_span!("game_ui::cards::choice_buttons").entered();
    let tick = tick.map_or(0, |t| t.0);
    for (interaction, b) in &buttons {
        if *interaction != Interaction::Pressed || sent.hides(b.offer, tick) {
            continue;
        }
        sent.0.insert(b.offer, tick);
        crate::army::send(
            &mut out,
            CommandKind::ChooseOffer {
                offer: b.offer,
                choice: b.choice,
            },
        );
        if *screen.get() == UiScreen::NewGame {
            next.set(UiScreen::Playing);
            // The pick applies on the first tick (tick 0).
            time.unpause();
            time.set_relative_speed(1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_read_as_percentages_or_flat() {
        assert_eq!(stat_text(Stat::Morale, 0.05), "+5% morale");
        assert_eq!(stat_text(Stat::LoyaltyDrift, -0.05), "-5% loyalty drift");
        assert_eq!(stat_text(Stat::CaptainSlots, 1.0), "+1 captain slots");
    }

    #[test]
    fn every_shipped_boon_and_trait_has_a_card() {
        let p = Progression::default_data();
        let place = |_: u64| "Ashmere".to_string();
        let none = |_: u64| None;
        for (i, _) in p.data.boons.iter().enumerate() {
            let (t, _) = card_text(OfferKind::StartBoon, 0, i as u16, &p, &place, &none);
            assert!(!t.starts_with("Boon "), "{t}");
        }
        for (i, _) in p.data.traits.iter().enumerate() {
            let (t, b) = card_text(
                OfferKind::Trait { person: 1 },
                0,
                i as u16,
                &p,
                &place,
                &none,
            );
            assert!(!t.starts_with("Trait ") && b.contains('%'), "{t}: {b}");
        }
        let (t, b) = card_text(
            OfferKind::Invitation { settlement: 3 },
            0,
            0,
            &p,
            &place,
            &none,
        );
        assert_eq!(t, "Accept");
        assert!(b.contains("Ashmere"));
        // GP-32: with the view's terms in hand, the card says what sworn means.
        let terms = |_: u64| {
            Some(SwornTerms {
                seed: 0.75,
                bond: 0.05,
            })
        };
        let (_, b) = card_text(
            OfferKind::Invitation { settlement: 3 },
            0,
            0,
            &p,
            &place,
            &terms,
        );
        assert_eq!(
            b,
            "Ashmere offers lordship. Sworn: loyal (starts at 0.75); they will expect you \
             to defend them."
        );
        let (t, _) = card_text(
            OfferKind::Invitation { settlement: 3 },
            1,
            0,
            &p,
            &place,
            &terms,
        );
        assert_eq!(t, "Decline");
        // GP-27: the hire card says how many, for how much, and how near.
        let hire = OfferKind::Hire {
            captain: 9,
            men: 12,
            price: 11,
        };
        let (t, b) = card_text(hire, 0, 0, &p, &place, &none);
        assert_eq!(t, "Hire");
        assert!(
            b.contains("12 armed sellswords") && b.contains("11 food") && b.contains("40 tiles"),
            "{b}"
        );
        assert_eq!(card_text(hire, 1, 1, &p, &place, &none).0, "Decline");
        // Unarmed in the data, unarmed on the card.
        let mut bare = p.clone();
        bare.data.hire.armed = false;
        let (_, b) = card_text(hire, 0, 0, &bare, &place, &none);
        assert!(b.starts_with("12 sellswords"), "{b}");
    }

    /// GP-27: an answered offer hides until the tick that applies the
    /// answer has run; refused, it shows again.
    #[test]
    fn a_refused_offer_shows_again_after_its_tick() {
        let mut sent = SentOffers::default();
        assert!(!sent.hides(4, 10));
        sent.0.insert(4, 10);
        assert!(sent.hides(4, 10), "paused: not applied yet");
        assert!(
            !sent.hides(4, 11),
            "the tick ran and the offer is still there"
        );
        assert!(!sent.hides(5, 10));
    }
}
