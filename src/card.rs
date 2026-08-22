/// A flashcard as it moves between the two formats. `due`/`interval_days`/
/// `ease`/`reps`/`lapses` only ever come from the JSON Lines side - Anki's
/// plain-text export has no columns for them, so they're `None` for any
/// card read from Anki.
#[derive(Debug, Clone)]
pub struct Card {
    pub front: String,
    pub back: String,
    pub tags: Vec<String>,
    pub due: Option<String>,
    pub interval_days: Option<u32>,
    pub ease: Option<f64>,
    pub reps: Option<u32>,
    pub lapses: Option<u32>,
}

impl Card {
    pub fn new_unscheduled(front: String, back: String, tags: Vec<String>) -> Self {
        Card {
            front,
            back,
            tags,
            due: None,
            interval_days: None,
            ease: None,
            reps: None,
            lapses: None,
        }
    }
}
