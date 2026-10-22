//! Timing of the flip animation, independent of any clock or GPU.
//!
//! A flip has two phases. First the top flap, still showing the old value,
//! falls forward until it stands edge-on at 90°. Then its back, showing the
//! bottom half of the new value, continues down until it lies flat on the
//! lower half of the card.

use std::f32::consts::PI;

/// Length of one complete flip in seconds.
pub const FLIP_DURATION: f64 = 0.6;

/// Progress at which the flap strikes the card, before its small rebound
/// (see [`flip_angle`]); the moment a flip is heard.
pub const LANDING: f32 = 0.875;

/// Angle of the flap in radians, from `0` (top flap flat, old value) to `π`
/// (flap folded down, new value), for a progress between `0` and `1`.
pub fn flip_angle(progress: f32) -> f32 {
    let t = progress.clamp(0.0, 1.0);
    if t < 0.5 {
        // Falling forward: starts slowly, like a flap tipping over its hinge.
        let u = t / 0.5;
        PI / 2.0 * u * u
    } else {
        // Coming down: fast at first, then easing out with a small rebound
        // as the flap lands on the card.
        let u = (t - 0.5) / 0.5;
        let eased = 1.0 - (1.0 - u) * (1.0 - u);
        let rebound = if u > 0.75 {
            0.06 * ((u - 0.75) / 0.25 * PI).sin()
        } else {
            0.0
        };
        PI / 2.0 + PI / 2.0 * (eased - rebound)
    }
}

/// What a card shows: its two digits and, on the hours card in 12-hour
/// mode, the AM/PM marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardContent {
    pub digits: String,
    pub marker: Option<&'static str>,
}

impl CardContent {
    /// Splits a clock face into its cards: hours, minutes and optional seconds.
    pub fn from_face(face: &openflip_core::ClockFace) -> Vec<Self> {
        let marker = face.pm.map(|pm| if pm { "PM" } else { "AM" });
        let mut cards = vec![
            Self {
                digits: face.hours.iter().collect(),
                marker,
            },
            Self {
                digits: face.minutes.iter().collect(),
                marker: None,
            },
        ];
        if let Some(seconds) = face.seconds {
            cards.push(Self {
                digits: seconds.iter().collect(),
                marker: None,
            });
        }
        cards
    }
}

/// One card at one moment: the value it flips from, the value it flips to
/// and how far the flip has progressed (`1.0` when it is at rest).
#[derive(Debug, Clone, PartialEq)]
pub struct CardFlip {
    pub from: CardContent,
    pub to: CardContent,
    pub progress: f32,
}

impl CardFlip {
    pub fn at_rest(content: CardContent) -> Self {
        Self {
            from: content.clone(),
            to: content,
            progress: 1.0,
        }
    }

    pub fn is_flipping(&self) -> bool {
        self.progress < 1.0
    }
}

struct CardState {
    from: CardContent,
    to: CardContent,
    /// Start of the current flip, in seconds on the caller's clock.
    started: Option<f64>,
    /// Whether the landing of the current flip was already reported.
    landed: bool,
}

/// Remembers what each card shows and starts a flip whenever that changes.
#[derive(Default)]
pub struct Flipper {
    cards: Vec<CardState>,
}

impl Flipper {
    /// Feeds the current card contents at time `now` (seconds, any origin,
    /// monotonic). The first update and any change in the number of cards
    /// show the contents immediately, without flipping.
    pub fn update(&mut self, contents: &[CardContent], now: f64) {
        if self.cards.len() != contents.len() {
            self.cards = contents
                .iter()
                .map(|c| CardState {
                    from: c.clone(),
                    to: c.clone(),
                    started: None,
                    landed: true,
                })
                .collect();
            return;
        }
        for (card, content) in self.cards.iter_mut().zip(contents) {
            if card.to != *content {
                // A change in the middle of a flip restarts from the value
                // the card was heading to; at one flip per second at most
                // this only happens after the clock was adjusted.
                card.from = std::mem::replace(&mut card.to, content.clone());
                card.started = Some(now);
                card.landed = false;
            }
        }
    }

    pub fn cards(&self, now: f64) -> Vec<CardFlip> {
        self.cards
            .iter()
            .map(|card| {
                let progress = card.started.map_or(1.0, |start| progress(start, now));
                CardFlip {
                    from: card.from.clone(),
                    to: card.to.clone(),
                    progress,
                }
            })
            .collect()
    }

    /// How many flaps struck their card since the last call, up to `now`.
    /// Each landing is reported once, however often this is called.
    pub fn take_landings(&mut self, now: f64) -> usize {
        let mut landings = 0;
        for card in &mut self.cards {
            if let Some(start) = card.started {
                if !card.landed && progress(start, now) >= LANDING {
                    card.landed = true;
                    landings += 1;
                }
            }
        }
        landings
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.cards
            .iter()
            .any(|c| c.started.is_some_and(|start| progress(start, now) < 1.0))
    }
}

fn progress(start: f64, now: f64) -> f32 {
    ((now - start) / FLIP_DURATION).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(digits: &str) -> CardContent {
        CardContent {
            digits: digits.into(),
            marker: None,
        }
    }

    #[test]
    fn angle_runs_from_flat_to_folded() {
        assert_eq!(flip_angle(0.0), 0.0);
        assert!((flip_angle(0.5) - PI / 2.0).abs() < 1e-5);
        assert!((flip_angle(1.0) - PI).abs() < 1e-5);
        assert_eq!(flip_angle(-1.0), 0.0);
        assert!((flip_angle(2.0) - PI).abs() < 1e-5);
    }

    #[test]
    fn angle_never_passes_through_the_card() {
        for i in 0..=1000 {
            let angle = flip_angle(i as f32 / 1000.0);
            assert!(
                (0.0..=PI + 1e-5).contains(&angle),
                "angle {angle} at step {i}"
            );
        }
    }

    #[test]
    fn first_update_does_not_flip() {
        let mut flipper = Flipper::default();
        flipper.update(&[card("12"), card("34")], 0.0);
        assert!(!flipper.is_animating(0.0));
        assert!(flipper.cards(0.0).iter().all(|c| !c.is_flipping()));
    }

    #[test]
    fn each_landing_is_reported_once_when_the_flap_strikes() {
        let mut flipper = Flipper::default();
        flipper.update(&[card("12"), card("34")], 0.0);
        assert_eq!(flipper.take_landings(0.0), 0, "no flip, no landing");

        flipper.update(&[card("13"), card("35")], 10.0);
        let strike = 10.0 + FLIP_DURATION * f64::from(LANDING);
        assert_eq!(flipper.take_landings(strike - 0.01), 0);
        assert_eq!(flipper.take_landings(strike + 0.001), 2);
        assert_eq!(flipper.take_landings(strike + 0.01), 0);
        assert_eq!(flipper.take_landings(20.0), 0);
    }

    #[test]
    fn the_flap_strikes_the_card_at_the_landing() {
        let flat = |p: f32| (flip_angle(p) - PI).abs();
        // Nearly flat at the landing, then lifts a little in the rebound.
        assert!(flat(LANDING) < 0.07 * PI);
        assert!(flat(LANDING) < flat(LANDING - 0.05));
        assert!(flat(LANDING + 0.05) > flat(LANDING));
    }

    #[test]
    fn only_changed_cards_flip() {
        let mut flipper = Flipper::default();
        flipper.update(&[card("12"), card("34")], 0.0);
        flipper.update(&[card("12"), card("35")], 10.0);

        let cards = flipper.cards(10.3);
        assert!(!cards[0].is_flipping());
        assert_eq!(cards[1].from, card("34"));
        assert_eq!(cards[1].to, card("35"));
        assert!((cards[1].progress - 0.5).abs() < 1e-3);
        assert!(flipper.is_animating(10.3));

        assert!(!flipper.is_animating(10.0 + FLIP_DURATION + 0.01));
        assert_eq!(flipper.cards(11.0)[1].progress, 1.0);
    }

    #[test]
    fn changing_the_number_of_cards_resets_without_flipping() {
        let mut flipper = Flipper::default();
        flipper.update(&[card("12"), card("34")], 0.0);
        flipper.update(&[card("12"), card("34"), card("56")], 1.0);
        assert!(!flipper.is_animating(1.0));
        assert_eq!(flipper.cards(1.0).len(), 3);
    }

    #[test]
    fn splits_a_face_into_cards() {
        let face = openflip_core::ClockFace {
            hours: ['0', '9'],
            minutes: ['4', '1'],
            seconds: Some(['0', '7']),
            pm: Some(true),
        };
        let cards = CardContent::from_face(&face);
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].digits, "09");
        assert_eq!(cards[0].marker, Some("PM"));
        assert_eq!(cards[2].digits, "07");
    }
}
