use crate::card::Card;
use std::io;

const FIELD_SEP: char = '\t';

/// Anki writes header lines like `#separator:tab` before the actual
/// notes, and plain-text exports can have trailing blank lines.
pub fn is_comment_or_blank(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

pub fn parse_line(line: &str) -> Result<Card, String> {
    let mut fields = line.split(FIELD_SEP);
    let front = fields.next().ok_or("missing front field")?.to_string();
    let back = fields.next().ok_or("missing back field")?.to_string();
    let tags = match fields.next() {
        Some(raw) if !raw.trim().is_empty() => {
            raw.split_whitespace().map(|s| s.to_string()).collect()
        }
        _ => Vec::new(),
    };
    Ok(Card::new_unscheduled(front, back, tags))
}

pub fn write_line<W: io::Write>(w: &mut W, card: &Card) -> io::Result<()> {
    if card.front.contains(FIELD_SEP) || card.back.contains(FIELD_SEP) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "field contains a tab character, which the anki text format cannot represent",
        ));
    }
    write!(w, "{}\t{}", card.front, card.back)?;
    if !card.tags.is_empty() {
        write!(w, "\t{}", card.tags.join(" "))?;
    }
    writeln!(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(line: &str) -> String {
        let card = parse_line(line).unwrap();
        let mut buf = Vec::new();
        write_line(&mut buf, &card).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn round_trips_a_note_with_tags() {
        let line = "mitochondria\tpowerhouse of the cell\tbiology cell";
        assert_eq!(round_trip(line), format!("{line}\n"));
    }

    #[test]
    fn round_trips_a_note_without_tags() {
        let line = "2+2\t4";
        assert_eq!(round_trip(line), format!("{line}\n"));
    }

    #[test]
    fn comment_and_blank_lines_are_recognized() {
        assert!(is_comment_or_blank("#separator:tab"));
        assert!(is_comment_or_blank("   "));
        assert!(is_comment_or_blank(""));
        assert!(!is_comment_or_blank("front\tback"));
    }

    #[test]
    fn parse_line_rejects_missing_back_field() {
        assert!(parse_line("front only").is_err());
    }

    #[test]
    fn write_line_rejects_embedded_tab() {
        let card = Card::new_unscheduled("has\ttab".to_string(), "back".to_string(), Vec::new());
        let mut buf = Vec::new();
        assert!(write_line(&mut buf, &card).is_err());
    }
}
