mod anki;
mod card;
mod json;

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
        Some("anki-to-jsonl") => anki_to_jsonl(reader, &mut writer),
        Some("jsonl-to-anki") => jsonl_to_anki(reader, &mut writer),
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
    eprintln!("usage: srs-format-bridge <anki-to-jsonl|jsonl-to-anki>");
    eprintln!("reads cards from stdin, writes the converted form to stdout");
}

// Both directions process one line at a time - read, convert, write, move
// on - so the whole deck never has to sit in memory at once.

fn anki_to_jsonl<R: BufRead, W: Write>(reader: R, writer: &mut W) -> Result<(), String> {
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", i + 1))?;
        if anki::is_comment_or_blank(&line) {
            continue;
        }
        let card = anki::parse_line(&line).map_err(|e| format!("line {}: {e}", i + 1))?;
        json::write_card_line(writer, &card).map_err(|e| format!("line {}: {e}", i + 1))?;
    }
    Ok(())
}

fn jsonl_to_anki<R: BufRead, W: Write>(reader: R, writer: &mut W) -> Result<(), String> {
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", i + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let card = json::parse_card_line(&line).map_err(|e| format!("line {}: {e}", i + 1))?;
        anki::write_line(writer, &card).map_err(|e| format!("line {}: {e}", i + 1))?;
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
        jsonl_to_anki(input.as_bytes(), &mut out).unwrap();
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
        let input = "{\"front\":\"a\",\"back\":\"b\",\"tags\":[\"x\"]}\n";
        let anki = run_jsonl_to_anki(input);
        let jsonl = run_anki_to_jsonl(&anki);
        assert_eq!(jsonl, input);
    }

    #[test]
    fn sending_a_scheduled_card_back_to_anki_drops_the_schedule() {
        let input = "{\"front\":\"a\",\"back\":\"b\",\"tags\":[],\"due\":\"2026-08-24\",\"interval_days\":3,\"ease\":2.5,\"reps\":4,\"lapses\":1}\n";
        let anki = run_jsonl_to_anki(input);
        assert_eq!(anki, "a\tb\n");
    }
}
