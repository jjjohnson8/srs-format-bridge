use crate::card::Card;
use crate::media;
use std::io;

/// Anki's plain-text export lets you pick the field delimiter at export
/// time; tab is the default, but comma (proper CSV, with quoting) is the
/// other option people actually reach for since it opens cleanly in a
/// spreadsheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Separator {
    Tab,
    Comma,
}

impl Separator {
    fn as_char(self) -> char {
        match self {
            Separator::Tab => '\t',
            Separator::Comma => ',',
        }
    }
}

/// Anki writes header lines like `#separator:tab` before the actual
/// notes, and plain-text exports can have trailing blank lines.
pub fn is_comment_or_blank(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// Reads the `#separator:` header line, if this line is one. Returns
/// `None` for anything else, including comment lines we don't recognize,
/// so callers should keep whatever separator they already had.
pub fn detect_separator(line: &str) -> Option<Separator> {
    match line.strip_prefix("#separator:")?.trim() {
        "tab" => Some(Separator::Tab),
        "comma" => Some(Separator::Comma),
        _ => None,
    }
}

pub fn parse_line(line: &str, sep: Separator) -> Result<Card, String> {
    let mut fields = split_fields(line, sep)?.into_iter();
    let front = fields.next().ok_or("missing front field")?;
    let back = fields.next().ok_or("missing back field")?;
    let tags = match fields.next() {
        Some(raw) if !raw.trim().is_empty() => {
            raw.split_whitespace().map(|s| s.to_string()).collect()
        }
        _ => Vec::new(),
    };
    let media = media::extract_references(&[&front, &back]);
    Ok(Card::new_unscheduled(front, back, tags, media))
}

fn split_fields(line: &str, sep: Separator) -> Result<Vec<String>, String> {
    match sep {
        Separator::Tab => Ok(line.split(Separator::Tab.as_char()).map(String::from).collect()),
        Separator::Comma => split_csv_fields(line),
    }
}

/// Splits one CSV record, honoring double-quoted fields where a literal
/// `"` is written as `""` and a quoted field may contain commas that
/// don't act as separators. Doesn't handle a comma-separated field that
/// spans multiple lines (a quoted newline) - each line coming in is
/// treated as one record, matching how the rest of this tool streams.
fn split_csv_fields(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut chars = line.chars().peekable();
    loop {
        let field = if chars.peek() == Some(&'"') {
            chars.next();
            let mut out = String::new();
            loop {
                match chars.next() {
                    Some('"') => {
                        if chars.peek() == Some(&'"') {
                            chars.next();
                            out.push('"');
                        } else {
                            break;
                        }
                    }
                    Some(c) => out.push(c),
                    None => return Err("unterminated quoted field".to_string()),
                }
            }
            out
        } else {
            let mut out = String::new();
            while let Some(&c) = chars.peek() {
                if c == ',' {
                    break;
                }
                out.push(c);
                chars.next();
            }
            out
        };
        fields.push(field);
        match chars.next() {
            Some(',') => continue,
            None => break,
            Some(c) => return Err(format!("unexpected character '{c}' after field")),
        }
    }
    Ok(fields)
}

pub fn write_line<W: io::Write>(w: &mut W, card: &Card, sep: Separator) -> io::Result<()> {
    match sep {
        Separator::Tab => write_tab_line(w, card),
        Separator::Comma => write_csv_line(w, card),
    }
}

fn write_tab_line<W: io::Write>(w: &mut W, card: &Card) -> io::Result<()> {
    if card.front.contains('\t') || card.back.contains('\t') {
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

fn write_csv_line<W: io::Write>(w: &mut W, card: &Card) -> io::Result<()> {
    write!(w, "{}", csv_quote(&card.front))?;
    write!(w, ",{}", csv_quote(&card.back))?;
    if !card.tags.is_empty() {
        write!(w, ",{}", csv_quote(&card.tags.join(" ")))?;
    }
    writeln!(w)
}

/// Quotes a field only when it needs it (contains a comma, a quote, or a
/// newline), matching how Anki's own CSV export stays readable for the
/// common case of plain text.
fn csv_quote(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        let mut out = String::with_capacity(field.len() + 2);
        out.push('"');
        for c in field.chars() {
            if c == '"' {
                out.push('"');
            }
            out.push(c);
        }
        out.push('"');
        out
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(line: &str, sep: Separator) -> String {
        let card = parse_line(line, sep).unwrap();
        let mut buf = Vec::new();
        write_line(&mut buf, &card, sep).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn round_trips_a_note_with_tags() {
        let line = "mitochondria\tpowerhouse of the cell\tbiology cell";
        assert_eq!(round_trip(line, Separator::Tab), format!("{line}\n"));
    }

    #[test]
    fn round_trips_a_note_without_tags() {
        let line = "2+2\t4";
        assert_eq!(round_trip(line, Separator::Tab), format!("{line}\n"));
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
        assert!(parse_line("front only", Separator::Tab).is_err());
    }

    #[test]
    fn write_line_rejects_embedded_tab() {
        let card = Card::new_unscheduled("has\ttab".to_string(), "back".to_string(), Vec::new(), Vec::new());
        let mut buf = Vec::new();
        assert!(write_line(&mut buf, &card, Separator::Tab).is_err());
    }

    #[test]
    fn detects_known_separator_headers() {
        assert_eq!(detect_separator("#separator:tab"), Some(Separator::Tab));
        assert_eq!(detect_separator("#separator:comma"), Some(Separator::Comma));
        assert_eq!(detect_separator("#separator:pipe"), None);
        assert_eq!(detect_separator("mitochondria,cell"), None);
    }

    #[test]
    fn round_trips_a_csv_note_with_tags() {
        let line = "mitochondria,powerhouse of the cell,biology cell";
        assert_eq!(round_trip(line, Separator::Comma), format!("{line}\n"));
    }

    #[test]
    fn csv_fields_with_commas_and_quotes_are_quoted_on_write_and_parsed_back() {
        let card = Card::new_unscheduled(
            "what does \"CPU\" stand for?".to_string(),
            "central, processing, unit".to_string(),
            vec!["hardware".to_string()],
            Vec::new(),
        );
        let mut buf = Vec::new();
        write_line(&mut buf, &card, Separator::Comma).unwrap();
        let written = String::from_utf8(buf).unwrap();
        assert_eq!(
            written,
            "\"what does \"\"CPU\"\" stand for?\",\"central, processing, unit\",hardware\n"
        );
        let reparsed = parse_line(written.trim_end(), Separator::Comma).unwrap();
        assert_eq!(reparsed.front, card.front);
        assert_eq!(reparsed.back, card.back);
        assert_eq!(reparsed.tags, card.tags);
    }

    #[test]
    fn csv_parse_rejects_unterminated_quoted_field() {
        assert!(parse_line("\"unterminated,back", Separator::Comma).is_err());
    }

    #[test]
    fn parse_line_collects_media_references_from_both_fields() {
        let line = "<img src=\"cell.png\">\tlisten: [sound:cell.mp3]\tbiology";
        let card = parse_line(line, Separator::Tab).unwrap();
        assert_eq!(card.media, vec!["cell.png".to_string(), "cell.mp3".to_string()]);
    }

    #[test]
    fn parse_line_leaves_media_empty_for_plain_text_fields() {
        let card = parse_line("2+2\t4", Separator::Tab).unwrap();
        assert!(card.media.is_empty());
    }
}
