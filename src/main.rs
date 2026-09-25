mod anki;
mod card;
mod date;
mod html;
mod json;
mod media;
mod sm2;

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).cloned();
    let extra = if args.len() > 2 { &args[2..] } else { &[][..] };

    let (input_path, output_path, rest) = match parse_io_paths(extra) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let reader: Box<dyn BufRead> = match &input_path {
        Some(path) => match fs::File::open(path) {
            Ok(f) => Box::new(io::BufReader::new(f)),
            Err(e) => {
                eprintln!("error: couldn't open '{path}' for reading: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => Box::new(io::stdin().lock()),
    };

    let mut writer: Box<dyn Write> = match &output_path {
        Some(path) => match fs::File::create(path) {
            Ok(f) => Box::new(io::BufWriter::new(f)),
            Err(e) => {
                eprintln!("error: couldn't open '{path}' for writing: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => Box::new(io::BufWriter::new(io::stdout().lock())),
    };

    let result = match mode.as_deref() {
        Some("anki-to-jsonl") => match rest.first().map(String::as_str) {
            None => anki_to_jsonl(reader, &mut writer),
            Some(other) => {
                eprintln!("error: unrecognized option '{other}'");
                return ExitCode::FAILURE;
            }
        },
        Some("jsonl-to-anki") => match rest.first().map(String::as_str) {
            None => jsonl_to_anki(reader, &mut writer, anki::Separator::Tab),
            Some("--csv") => jsonl_to_anki(reader, &mut writer, anki::Separator::Comma),
            Some(other) => {
                eprintln!("error: unrecognized option '{other}'");
                return ExitCode::FAILURE;
            }
        },
        Some("review") => {
            let today = match rest.first() {
                Some(today) => today.clone(),
                None => {
                    eprintln!("error: 'review' requires a date argument, e.g. 'review 2026-09-18'");
                    return ExitCode::FAILURE;
                }
            };
            if let Some(other) = rest.get(1) {
                eprintln!("error: unrecognized option '{other}'");
                return ExitCode::FAILURE;
            }
            review(reader, &mut writer, &today)
        }
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

// Pulls --input/-i and --output/-o (and their path arguments) out of the
// argument list, wherever they appear, leaving the rest (subcommand-specific
// flags and positional args) in order for the caller to interpret.
fn parse_io_paths(
    args: &[String],
) -> Result<(Option<String>, Option<String>, Vec<String>), String> {
    let mut input_path = None;
    let mut output_path = None;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            flag @ ("--input" | "-i") => {
                let path = args
                    .get(i + 1)
                    .ok_or_else(|| format!("'{flag}' requires a path argument"))?;
                input_path = Some(path.clone());
                i += 2;
            }
            flag @ ("--output" | "-o") => {
                let path = args
                    .get(i + 1)
                    .ok_or_else(|| format!("'{flag}' requires a path argument"))?;
                output_path = Some(path.clone());
                i += 2;
            }
            other => {
                rest.push(other.to_string());
                i += 1;
            }
        }
    }
    Ok((input_path, output_path, rest))
}

fn print_usage() {
    eprintln!("usage: srs-format-bridge <anki-to-jsonl|jsonl-to-anki|review> [options]");
    eprintln!("reads cards from stdin, writes the converted form to stdout, unless");
    eprintln!("--input/-i or --output/-o give a file path to use instead.");
    eprintln!();
    eprintln!("anki-to-jsonl reads either a tab- or comma-separated Anki export,");
    eprintln!("detecting which one from the '#separator:' header line (tab if absent).");
    eprintln!("jsonl-to-anki writes tab-separated Anki notes unless --csv is given,");
    eprintln!("in which case it writes comma-separated notes with CSV quoting.");
    eprintln!("review <date> reads jsonl cards that each carry a \"grade\" field (an");
    eprintln!("SM-2 quality score, 0-5) and writes them back out with due/interval_days/");
    eprintln!("ease/reps/lapses advanced as of <date> (YYYY-MM-DD).");
}

// Both directions process one line at a time - read, convert, write, move
// on - so the whole deck never has to sit in memory at once.

// Reports progress on stderr every REPORT_INTERVAL lines, so a large deck
// doesn't run silently for minutes with no sign of life. Stays quiet for
// small inputs (including everything the test suite feeds it) since the
// count never reaches the first interval.
const REPORT_INTERVAL: u64 = 10_000;

struct Progress {
    count: u64,
}

impl Progress {
    fn new() -> Self {
        Progress { count: 0 }
    }

    fn tick(&mut self) {
        self.count += 1;
        if self.count % REPORT_INTERVAL == 0 {
            eprintln!("... {} lines processed", self.count);
        }
    }

    fn finish(&self) {
        if self.count >= REPORT_INTERVAL {
            eprintln!("done: {} lines processed", self.count);
        }
    }
}

fn anki_to_jsonl<R: BufRead, W: Write>(reader: R, writer: &mut W) -> Result<(), String> {
    let mut sep = anki::Separator::Tab;
    let mut progress = Progress::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", i + 1))?;
        progress.tick();
        if let Some(detected) = anki::detect_separator(&line) {
            sep = detected;
        }
        if anki::is_comment_or_blank(&line) {
            continue;
        }
        let card = anki::parse_line(&line, sep).map_err(|e| format!("line {}: {e}", i + 1))?;
        json::write_card_line(writer, &card).map_err(|e| format!("line {}: {e}", i + 1))?;
    }
    progress.finish();
    Ok(())
}

fn jsonl_to_anki<R: BufRead, W: Write>(
    reader: R,
    writer: &mut W,
    sep: anki::Separator,
) -> Result<(), String> {
    let mut progress = Progress::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", i + 1))?;
        progress.tick();
        if line.trim().is_empty() {
            continue;
        }
        let card = json::parse_card_line(&line).map_err(|e| format!("line {}: {e}", i + 1))?;
        anki::write_line(writer, &card, sep).map_err(|e| format!("line {}: {e}", i + 1))?;
    }
    progress.finish();
    Ok(())
}

fn review<R: BufRead, W: Write>(reader: R, writer: &mut W, today: &str) -> Result<(), String> {
    let mut progress = Progress::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", i + 1))?;
        progress.tick();
        if line.trim().is_empty() {
            continue;
        }
        let value = json::parse(&line).map_err(|e| format!("line {}: {e}", i + 1))?;
        let grade = value
            .get("grade")
            .and_then(json::Value::as_f64)
            .ok_or_else(|| format!("line {}: missing \"grade\" field", i + 1))?;
        if grade < 0.0 || grade > 5.0 || grade.fract() != 0.0 {
            return Err(format!(
                "line {}: \"grade\" must be a whole number from 0 to 5",
                i + 1
            ));
        }
        let mut card = json::parse_card_line(&line).map_err(|e| format!("line {}: {e}", i + 1))?;
        sm2::review(&mut card, grade as u8, today).map_err(|e| format!("line {}: {e}", i + 1))?;
        json::write_card_line(writer, &card).map_err(|e| format!("line {}: {e}", i + 1))?;
    }
    progress.finish();
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

    fn run_review(input: &str, today: &str) -> String {
        let mut out = Vec::new();
        review(input.as_bytes(), &mut out, today).unwrap();
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
        let input = "{\"front\":\"a\",\"back\":\"b\",\"tags\":[\"x\"],\"media\":[],\"plain_front\":\"a\",\"plain_back\":\"b\"}\n";
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
    fn anki_to_jsonl_strips_html_into_plain_text_fields() {
        let input = "what is the <b>capital</b>?\tParis &amp; environs\n";
        let out = run_anki_to_jsonl(input);
        assert!(out.contains("\"plain_front\":\"what is the capital?\""));
        assert!(out.contains("\"plain_back\":\"Paris & environs\""));
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

    #[test]
    fn review_advances_a_new_card_and_drops_the_grade_field() {
        let input = "{\"front\":\"a\",\"back\":\"b\",\"tags\":[],\"grade\":4}\n";
        let out = run_review(input, "2026-09-18");
        assert!(out.contains("\"due\":\"2026-09-19\""));
        assert!(out.contains("\"interval_days\":1"));
        assert!(out.contains("\"reps\":1"));
        assert!(!out.contains("\"grade\""));
    }

    #[test]
    fn review_requires_a_grade_field() {
        let input = "{\"front\":\"a\",\"back\":\"b\",\"tags\":[]}\n";
        let mut out = Vec::new();
        assert!(review(input.as_bytes(), &mut out, "2026-09-18").is_err());
    }

    #[test]
    fn review_rejects_an_out_of_range_grade() {
        let input = "{\"front\":\"a\",\"back\":\"b\",\"tags\":[],\"grade\":9}\n";
        let mut out = Vec::new();
        assert!(review(input.as_bytes(), &mut out, "2026-09-18").is_err());
    }

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_io_paths_finds_flags_anywhere_and_leaves_the_rest_in_order() {
        let args = strs(&["--csv", "--input", "deck.txt", "extra", "-o", "out.jsonl"]);
        let (input, output, rest) = parse_io_paths(&args).unwrap();
        assert_eq!(input.as_deref(), Some("deck.txt"));
        assert_eq!(output.as_deref(), Some("out.jsonl"));
        assert_eq!(rest, vec!["--csv", "extra"]);
    }

    #[test]
    fn parse_io_paths_with_no_io_flags_returns_everything_as_rest() {
        let args = strs(&["2026-09-18"]);
        let (input, output, rest) = parse_io_paths(&args).unwrap();
        assert!(input.is_none());
        assert!(output.is_none());
        assert_eq!(rest, vec!["2026-09-18"]);
    }

    #[test]
    fn parse_io_paths_errors_when_a_path_flag_is_missing_its_argument() {
        let args = strs(&["--input"]);
        assert!(parse_io_paths(&args).is_err());
    }

    #[test]
    fn progress_ticks_at_each_interval_boundary() {
        let mut progress = Progress::new();
        for _ in 0..REPORT_INTERVAL - 1 {
            progress.tick();
        }
        assert_eq!(progress.count, REPORT_INTERVAL - 1);
        progress.tick();
        assert_eq!(progress.count, REPORT_INTERVAL);
        progress.tick();
        assert_eq!(progress.count, REPORT_INTERVAL + 1);
    }
}
