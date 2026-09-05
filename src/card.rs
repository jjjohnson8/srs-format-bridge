use crate::html;

/// A flashcard as it moves between the two formats. `due`/`interval_days`/
/// `ease`/`reps`/`lapses` only ever come from the JSON Lines side - Anki's
/// plain-text export has no columns for them, so they're `None` for any
/// card read from Anki. `media` is derived from `front`/`back` (the
/// filenames an `<img>` or `[sound:]` reference points at); it's metadata
/// about what those fields already contain, not a separate source of
/// truth. `plain_front`/`plain_back` are likewise derived: `front`/`back`
/// with Anki's HTML stripped out and entities decoded, so a scheduler can
/// show review text without a markup renderer.
#[derive(Debug, Clone)]
pub struct Card {
    pub front: String,
    pub back: String,
    pub tags: Vec<String>,
    pub media: Vec<String>,
    pub plain_front: String,
    pub plain_back: String,
    pub due: Option<String>,
    pub interval_days: Option<u32>,
    pub ease: Option<f64>,
    pub reps: Option<u32>,
    pub lapses: Option<u32>,
}

impl Card {
    pub fn new_unscheduled(front: String, back: String, tags: Vec<String>, media: Vec<String>) -> Self {
        let plain_front = html::to_plain_text(&front);
        let plain_back = html::to_plain_text(&back);
        Card {
            front,
            back,
            tags,
            media,
            plain_front,
            plain_back,
            due: None,
            interval_days: None,
            ease: None,
            reps: None,
            lapses: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_unscheduled_derives_plain_text_from_html_fields() {
        let card = Card::new_unscheduled(
            "what is the <b>capital</b> of France?".to_string(),
            "Paris &amp; environs".to_string(),
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(card.plain_front, "what is the capital of France?");
        assert_eq!(card.plain_back, "Paris & environs");
    }
}
