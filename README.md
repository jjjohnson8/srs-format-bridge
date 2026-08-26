# srs-format-bridge

Converts flashcards between Anki's plain-text export format and a JSON
Lines format that carries spaced-repetition scheduling state (due date,
interval, ease factor, reps, lapses).

## Why

I keep my flashcards in Anki but I'm writing my own scheduler on the
side, and the two don't speak the same format. Anki's plain-text export
is just front/back/tags - it has no column for review history. My
scheduler reads one JSON object per line, one per card, including the
SM-2 style state it needs to compute the next due date. This tool moves
cards between the two:

- `anki-to-jsonl`: import a fresh Anki export, cards come out with no
  scheduling state (they're new).
- `jsonl-to-anki`: send scheduler cards back to Anki, scheduling state
  is dropped because Anki's plain-text format has nowhere to put it.

## Usage

Export a deck from Anki as plain text (Notes menu > Export Notes >
"Notes in Plain Text") and convert it:

```
cargo run --release -- anki-to-jsonl < deck.txt > deck.jsonl
```

Each output line looks like:

```
{"front":"mitochondria","back":"powerhouse of the cell","tags":["biology"]}
```

Going the other way, from scheduler state back to something Anki can
import:

```
cargo run --release -- jsonl-to-anki < deck.jsonl > deck.txt
```

If your Anki export uses commas instead of tabs (the other delimiter
Anki's export dialog offers), `anki-to-jsonl` picks that up automatically
from the `#separator:comma` header line Anki writes at the top of the
file. Going the other way, pass `--csv` to get comma-separated output
back:

```
cargo run --release -- jsonl-to-anki --csv < deck.jsonl > deck.csv
```

## Format notes

**Anki side:** one note per line, `front<sep>back<sep>tags`, where
`<sep>` is a tab by default or a comma for the CSV variant. The tags
column is optional and space-separated. Lines starting with `#` (Anki
writes header lines like `#separator:tab`) and blank lines are skipped
on the way in. In the comma-separated form, a field containing a comma,
a quote, or a newline is wrapped in double quotes, with embedded quotes
doubled - standard CSV quoting, matching what Anki itself writes.

**JSON Lines side:** one object per line with `front`, `back`, `tags`,
and optionally `due`, `interval_days`, `ease`, `reps`, `lapses`.

Both directions stream: each line is read, converted, and written
before the next one is touched, so converting a deck with a few
hundred thousand cards doesn't pull the whole file into memory.

## Known limitations right now

- A field containing a literal tab character can't round-trip through
  the tab-separated Anki format; the converter errors out instead of
  silently mangling the card.
- CSV parsing assumes one note per input line - a quoted field that
  contains a literal newline (rare, but legal CSV) won't round-trip,
  since this tool reads and converts line by line.
- Only the plain-text Anki export is supported (tab or comma
  delimited), not the `.apkg` SQLite format.
- `due` is carried through as an opaque string - nothing here parses
  or validates it as a date yet.
