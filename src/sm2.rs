//! The SM-2 spaced-repetition algorithm (Piotr Wozniak's original SuperMemo
//! 2 scheme): given how well a review went, decide how many days until the
//! card is due again.

use crate::card::Card;
use crate::date;

const DEFAULT_EASE: f64 = 2.5;
const MIN_EASE: f64 = 1.3;

/// Applies one review outcome to a card's scheduling state, advancing
/// `due`/`interval_days`/`ease`/`reps`/`lapses` in place. `quality` is the
/// standard SM-2 grade for how the review went, 0 (total blackout) through
/// 5 (perfect recall); 3 and above counts as a pass, below that as a
/// lapse. `today` is the review date, "YYYY-MM-DD" - `due` is computed
/// from it, not from the wall clock, so callers control what "today"
/// means. A card with no prior schedule (all `None`) is treated as new:
/// ease starts at 2.5, reps and lapses at 0.
pub fn review(card: &mut Card, quality: u8, today: &str) -> Result<(), String> {
    if quality > 5 {
        return Err(format!("quality {quality} is out of range - must be 0-5"));
    }

    let reps = card.reps.unwrap_or(0);
    let prev_interval = card.interval_days.unwrap_or(0);
    let ease = card.ease.unwrap_or(DEFAULT_EASE);
    let lapses = card.lapses.unwrap_or(0);

    let (new_reps, new_interval, new_lapses) = if quality >= 3 {
        let interval = match reps {
            0 => 1,
            1 => 6,
            _ => (f64::from(prev_interval) * ease).round() as u32,
        };
        (reps + 1, interval, lapses)
    } else {
        (0, 1, lapses + 1)
    };

    let q = f64::from(quality);
    let new_ease = (ease + (0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02))).max(MIN_EASE);

    card.due = Some(date::add_days(today, i64::from(new_interval))?);
    card.interval_days = Some(new_interval);
    card.ease = Some(new_ease);
    card.reps = Some(new_reps);
    card.lapses = Some(new_lapses);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_card() -> Card {
        Card::new_unscheduled("front".to_string(), "back".to_string(), Vec::new(), Vec::new())
    }

    #[test]
    fn first_pass_sets_a_one_day_interval() {
        let mut card = new_card();
        review(&mut card, 4, "2026-09-18").unwrap();
        assert_eq!(card.interval_days, Some(1));
        assert_eq!(card.reps, Some(1));
        assert_eq!(card.lapses, Some(0));
        assert_eq!(card.due.as_deref(), Some("2026-09-19"));
    }

    #[test]
    fn second_pass_sets_a_six_day_interval() {
        let mut card = new_card();
        review(&mut card, 4, "2026-09-18").unwrap();
        review(&mut card, 4, "2026-09-19").unwrap();
        assert_eq!(card.interval_days, Some(6));
        assert_eq!(card.reps, Some(2));
        assert_eq!(card.due.as_deref(), Some("2026-09-25"));
    }

    #[test]
    fn third_pass_scales_the_interval_by_ease() {
        let mut card = new_card();
        review(&mut card, 4, "2026-09-18").unwrap();
        review(&mut card, 4, "2026-09-19").unwrap();
        let ease_after_two = card.ease.unwrap();
        review(&mut card, 4, "2026-09-25").unwrap();
        assert_eq!(card.interval_days, Some((6.0 * ease_after_two).round() as u32));
        assert_eq!(card.reps, Some(3));
    }

    #[test]
    fn a_failing_grade_resets_reps_and_interval_and_counts_a_lapse() {
        let mut card = new_card();
        review(&mut card, 4, "2026-09-18").unwrap();
        review(&mut card, 4, "2026-09-19").unwrap();
        review(&mut card, 1, "2026-09-25").unwrap();
        assert_eq!(card.interval_days, Some(1));
        assert_eq!(card.reps, Some(0));
        assert_eq!(card.lapses, Some(1));
        assert_eq!(card.due.as_deref(), Some("2026-09-26"));
    }

    #[test]
    fn ease_never_drops_below_the_minimum() {
        let mut card = new_card();
        for _ in 0..20 {
            review(&mut card, 0, "2026-09-18").unwrap();
        }
        assert_eq!(card.ease, Some(MIN_EASE));
    }

    #[test]
    fn a_perfect_grade_raises_ease_above_the_default() {
        let mut card = new_card();
        review(&mut card, 5, "2026-09-18").unwrap();
        assert!(card.ease.unwrap() > DEFAULT_EASE);
    }

    #[test]
    fn rejects_a_quality_outside_zero_to_five() {
        let mut card = new_card();
        assert!(review(&mut card, 6, "2026-09-18").is_err());
    }

    #[test]
    fn rejects_an_invalid_today() {
        let mut card = new_card();
        assert!(review(&mut card, 4, "not-a-date").is_err());
    }
}
