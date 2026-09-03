//! Extracts filenames of media Anki embeds inline in a field's HTML: images
//! via `<img src="...">` and audio/video via the `[sound:...]` bracket
//! syntax Anki uses instead of an HTML tag. This only reads references
//! already present in field text - it doesn't touch the media files
//! themselves, since a plain-text Anki export never includes them (they
//! live in a separate media folder next to the real `.apkg`).

/// Collects references from the given fields in the order they first
/// appear, de-duplicated, since the same sound icon or image commonly shows
/// up more than once in a field.
pub fn extract_references(fields: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for field in fields {
        extract_sound_refs(field, &mut out);
        extract_img_refs(field, &mut out);
    }
    out
}

fn push_unique(out: &mut Vec<String>, filename: String) {
    if !filename.is_empty() && !out.contains(&filename) {
        out.push(filename);
    }
}

fn extract_sound_refs(field: &str, out: &mut Vec<String>) {
    let mut rest = field;
    while let Some(start) = rest.find("[sound:") {
        let after = &rest[start + "[sound:".len()..];
        match after.find(']') {
            Some(end) => {
                push_unique(out, after[..end].to_string());
                rest = &after[end + 1..];
            }
            None => break,
        }
    }
}

fn extract_img_refs(field: &str, out: &mut Vec<String>) {
    let lower = field.to_ascii_lowercase();
    let mut search_from = 0;
    while let Some(rel) = lower[search_from..].find("<img") {
        let tag_start = search_from + rel;
        let tag_end = match lower[tag_start..].find('>') {
            Some(rel_end) => tag_start + rel_end,
            None => break,
        };
        let tag = &field[tag_start..tag_end];
        if let Some(filename) = extract_src_attr(tag) {
            push_unique(out, filename);
        }
        search_from = tag_end + 1;
    }
}

/// Reads the value of a `src="..."`, `src='...'`, or unquoted `src=...`
/// attribute out of a single tag's source text.
fn extract_src_attr(tag: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let attr_start = lower.find("src=")? + "src=".len();
    let rest = &tag[attr_start..];
    match rest.chars().next()? {
        quote @ ('"' | '\'') => {
            let inner = &rest[1..];
            let end = inner.find(quote)?;
            Some(inner[..end].to_string())
        }
        _ => {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            Some(rest[..end].to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_sound_reference() {
        let refs = extract_references(&["what does a violin sound like? [sound:violin.mp3]"]);
        assert_eq!(refs, vec!["violin.mp3".to_string()]);
    }

    #[test]
    fn extracts_multiple_sound_references_without_duplicates() {
        let refs = extract_references(&["[sound:a.mp3] and again [sound:a.mp3] then [sound:b.mp3]"]);
        assert_eq!(refs, vec!["a.mp3".to_string(), "b.mp3".to_string()]);
    }

    #[test]
    fn extracts_an_image_reference_with_double_quotes() {
        let refs = extract_references(&["<img src=\"mitochondria.png\">"]);
        assert_eq!(refs, vec!["mitochondria.png".to_string()]);
    }

    #[test]
    fn extracts_an_image_reference_with_single_quotes_and_extra_attributes() {
        let refs = extract_references(&["<IMG alt='cell' src='cell.jpg' width=\"100\">"]);
        assert_eq!(refs, vec!["cell.jpg".to_string()]);
    }

    #[test]
    fn extracts_an_unquoted_image_reference() {
        let refs = extract_references(&["<img src=leaf.gif width=50>"]);
        assert_eq!(refs, vec!["leaf.gif".to_string()]);
    }

    #[test]
    fn collects_references_across_both_fields_in_order() {
        let refs = extract_references(&["<img src=\"front.png\">", "[sound:back.mp3]"]);
        assert_eq!(refs, vec!["front.png".to_string(), "back.mp3".to_string()]);
    }

    #[test]
    fn plain_text_fields_yield_no_references() {
        assert!(extract_references(&["mitochondria", "powerhouse of the cell"]).is_empty());
    }

    #[test]
    fn an_img_tag_missing_a_src_attribute_is_ignored() {
        assert!(extract_references(&["<img alt=\"no source here\">"]).is_empty());
    }
}
