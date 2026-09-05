//! Anki note fields are small fragments of HTML - a bold word, a cloze
//! span, the occasional `&nbsp;`. A scheduler reading review text doesn't
//! want to render any of that, so this turns a field into plain text by
//! dropping the tags and decoding the entities that are left behind.

/// Strips tags, decodes entities, and collapses the whitespace tags leave
/// behind so words that were split across a `<br>` or `<div>` don't end up
/// jammed together.
pub fn to_plain_text(field: &str) -> String {
    collapse_whitespace(&decode_entities(&strip_tags(field)))
}

/// Drops everything between `<` and `>`, replacing each tag with a single
/// space rather than nothing - `a<br>b` should read as "a b", not "ab".
/// An unterminated tag (a stray `<` with no closing `>`) consumes the rest
/// of the field, matching how a browser would treat it.
fn strip_tags(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c == '<' {
            for c in chars.by_ref() {
                if c == '>' {
                    break;
                }
            }
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Decodes the handful of named entities Anki fields actually contain,
/// plus numeric character references (`&#39;`, `&#x27;`). An `&` that
/// isn't the start of a recognized entity is left as-is, since plain text
/// can legitimately contain a bare ampersand.
fn decode_entities(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut rest = field;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        // No real entity runs longer than this, so bail out early rather
        // than scanning the rest of the field for a stray ';'.
        let scan_limit = after.len().min(10);
        match after[..scan_limit].find(';') {
            Some(end) => match decode_named(&after[..end]) {
                Some(decoded) => {
                    out.push(decoded);
                    rest = &after[end + 1..];
                }
                None => {
                    out.push('&');
                    rest = after;
                }
            },
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_named(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(dec) = name.strip_prefix('#') {
                dec.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                None
            }
        }
    }
}

/// Turns any run of whitespace (including the spaces left by `strip_tags`)
/// into a single space, and trims the ends.
fn collapse_whitespace(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut last_was_space = true;
    for c in field.chars() {
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_passes_through_unchanged() {
        assert_eq!(to_plain_text("mitochondria"), "mitochondria");
    }

    #[test]
    fn strips_simple_tags() {
        assert_eq!(to_plain_text("<b>mitochondria</b>"), "mitochondria");
    }

    #[test]
    fn replaces_tags_with_a_space_so_words_dont_merge() {
        assert_eq!(to_plain_text("one<br>two"), "one two");
        assert_eq!(to_plain_text("<div>one</div><div>two</div>"), "one two");
    }

    #[test]
    fn decodes_named_entities() {
        assert_eq!(to_plain_text("Q &amp; A"), "Q & A");
        assert_eq!(to_plain_text("&lt;tag&gt;"), "<tag>");
        assert_eq!(to_plain_text("say &quot;hi&quot;"), "say \"hi\"");
        assert_eq!(to_plain_text("it&apos;s"), "it's");
        assert_eq!(to_plain_text("a&nbsp;b"), "a b");
    }

    #[test]
    fn decodes_numeric_entities() {
        assert_eq!(to_plain_text("&#39;"), "'");
        assert_eq!(to_plain_text("&#x27;"), "'");
        assert_eq!(to_plain_text("&#x2603;"), "\u{2603}");
    }

    #[test]
    fn leaves_a_bare_ampersand_alone() {
        assert_eq!(to_plain_text("Marks & Spencer"), "Marks & Spencer");
        assert_eq!(to_plain_text("A&B testing"), "A&B testing");
    }

    #[test]
    fn collapses_whitespace_left_by_removed_tags_and_trims_ends() {
        assert_eq!(to_plain_text("  <p>one</p>\n<p>two</p>  "), "one two");
    }

    #[test]
    fn an_unterminated_tag_consumes_the_rest_of_the_field() {
        assert_eq!(to_plain_text("keep this <b drop this"), "keep this");
    }
}
