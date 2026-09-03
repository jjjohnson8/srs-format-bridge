mod anki;
mod card;
mod json;
mod media;

use std::env;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mode = env::args().nth(1);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let reader = stdin.lock();
    let mut writer = io::BufWriter::new(stdout.lock());

    let result = match mode.as_deref() {
        Some("anki-to-jsonl") => match env::args().nth(2).as_deref() {
            None => anki_to_jsonl(reader, &mut writer),
            Some(other) => {
                eprintln!("error: unrecognized option '{other}'");
                return ExitCode::FAILURE;
            }
        },
        Some("jsonl-to-anki") => match env::args().nth(2).as_deref() {
            None => jsonl_to_anki(reader, &mut writer, anki::Separator::Tab),
            Some("--csv") => jsonl_to_anki(reader, &mut writer, anki::Separator::Comma),
            Some(other) => {
                eprintln!("error: unrecognized option '{other}'");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn print_usage() {
    eprintln!("usage: srs-format-bridge <anki-to-jsonl|jsonl-to-anki> [--csv]");
    eprintln!("reads cards from stdin, writes the converted form to stdout");
    eprintln!();
    eprintln!("anki-to-jsonl reads either a tab- or comma-separated Anki export,");
    eprintln!("detecting which one from the '#separator:' header line (tab if absent).");
    eprintln!("jsonl-to-anki writes tab-separated Anki notes unless --csv is given,");
    eprintln!("in which case it writes comma-separated notes with CSV quoting.");
}

// Both directions process one line at a time - read, convert, write, move
// on - so the whole deck never has to sit in memory at once.

fn anki_to_jsonl<R: BufRead, W: Write>(reader: R, writer: &mut W) -> Result<(), String> {
    let mut sep = anki::Separator::Tab;
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", i + 1))?;
        if let Some(detected) = anki::detect_separator(&line) {
            sep = detected;
        }
        if anki::is_comment_or_blank(&line) {
            continue;
        }
        let card = anki::parse_line(&line, sep).map_err(|e| format!("line {}: {e}", i + 1))?;
        json::write_card_line(writer, &card).map_err(|e| format!("line {}: {e}", i + 1))?;
    }
    Ok(())
}

fn jsonl_to_anki<R: BufRead, W: Write>(
    reader: R,
    writer: &mut W,
    sep: anki::Separator,
) -> Result<(), String> {
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", i + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let card = json::parse_card_line(&line).map_err(|e| format!("line {}: {e}", i + 1))?;
        anki::write_line(writer, &card, sep).map_err(|e| format!("line {}: {e}", i + 1))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_anki_to_jsonl(input: &str) -> String {
        let mut out = Vec::new();
        anki_to_jsonl(input.as_bytes(), &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn run_jsonl_to_anki(input: &str) -> String {
        let mut out = Vec::new();
        jsonl_to_anki(input.as_bytes(), &mut out, anki::Separator::Tab).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn anki_to_jsonl_skips_header_and_blank_lines() {
        let input = "#separator:tab\nmitochondria\tpowerhouse of the cell\tbiology\n\n2+2\t4\n";
        let out = run_anki_to_jsonl(input);
        assert_eq!(out.lines().count(), 2);
        assert!(out.contains("\"front\":\"mitochondria\""));
        assert!(out.contains("\"front\":\"2+2\""));
    }

    #[test]
    fn anki_export_round_trips_through_jsonl_and_back() {
        let input =
            "#separator:tab\nmitochondria\tpowerhouse of the cell\tbiology cell\n2+2\t4\n";
        let jsonl = run_anki_to_jsonl(input);
        let anki = run_jsonl_to_anki(&jsonl);
        assert_eq!(
            anki,
            "mitochondria\tpowerhouse of the cell\tbiology cell\n2+2\t4\n"
        );
    }

    #[test]
    fn jsonl_round_trips_through_anki_when_there_is_no_schedule_to_lose() {
        let input = "{\"front\":\"a\",\"back\":\"b\",\"tags\":[\"x\"],\"media\":[]}\n";
        let anki = run_jsonl_to_anki(input);
        let jsonl = run_anki_to_jsonl(&anki);
        assert_eq!(jsonl, input);
    }

    #[test]
    fn anki_to_jsonl_carries_media_references_found_in_the_fields() {
        let input = "<img src=\"cell.png\">\tsee image\n";
        let out = run_anki_to_jsonl(input);
        assert!(out.contains("\"media\":[\"cell.png\"]"));
    }

    #[test]
    fn sending_a_scheduled_card_back_to_anki_drops_the_schedule() {
        let input = "{\"front\":\"a\",\"back\":\"b\",\"tags\":[],\"due\":\"2026-08-24\",\"interval_days\":3,\"ease\":2.5,\"reps\":4,\"lapses\":1}\n";
        let anki = run_jsonl_to_anki(input);
        assert_eq!(anki, "a\tb\n");
    }

    #[test]
    fn anki_to_jsonl_detects_comma_separator_from_header() {
        let input = "#separator:comma\nmitochondria,powerhouse of the cell,biology cell\n";
        let out = run_anki_to_jsonl(input);
        assert!(out.contains("\"front\":\"mitochondria\""));
        assert!(out.contains("\"tags\":[\"biology\",\"cell\"]"));
    }

    #[test]
    fn jsonl_to_anki_writes_csv_with_quoting_when_requested() {
        let input = "{\"front\":\"a, b\",\"back\":\"c\",\"tags\":[]}\n";
        let mut out = Vec::new();
        jsonl_to_anki(input.as_bytes(), &mut out, anki::Separator::Comma).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "\"a, b\",c\n");
    }
}
