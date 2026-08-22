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
