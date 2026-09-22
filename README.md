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
{"front":"mitochondria","back":"powerhouse of the cell","tags":["biology"],"media":[]}
```

If a field's HTML references media Anki would normally bundle alongside the
export - an `<img src="...">` tag or Anki's `[sound:...]` syntax - the
filenames show up in `media`:

```
{"front":"<img src=\"cell.png\">","back":"cell","tags":[],"media":["cell.png"]}
```

`front`/`back` keep the raw HTML as Anki wrote it; `media` is just a list of
what's referenced in there, so a scheduler can know which files it needs
without re-parsing HTML itself. This tool never touches the media files -
Anki's plain-text export doesn't include them anyway.

Every card also carries `plain_front`/`plain_back`: the same fields with
tags stripped and entities decoded, for a scheduler that wants to show
review text without rendering HTML:

```
{"front":"what is the <b>capital</b>?","back":"Paris &amp; environs","tags":[],"media":[],"plain_front":"what is the capital?","plain_back":"Paris & environs"}
```

These are always derived from `front`/`back` - `jsonl-to-anki` ignores
them on the way back in, so hand-editing them in a JSONL file has no
effect.

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

## Reviewing cards

`review` runs the SM-2 algorithm forward by one step. Feed it JSONL where
each card also carries a `grade` field - the SM-2 recall quality for that
review, 0 (total blackout) through 5 (perfect recall), with 3 and up
counting as a pass - plus a date to treat as "today":

```
cargo run --release -- review 2026-09-18 < reviewed.jsonl > deck.jsonl
```

It reads each card's existing `due`/`interval_days`/`ease`/`reps`/`lapses`
(a brand new card with none of these is treated as unscheduled: ease
starts at 2.5, reps and lapses at 0), applies the grade, and writes the
card back out with those fields advanced and `grade` dropped. A pass
pushes the interval out (1 day, then 6, then scaled by ease); a grade
below 3 resets the interval to 1 day and counts a lapse. `due` is always
`today` plus the new interval - this tool doesn't read the system clock,
so the caller decides what date a review happened on.

## Format notes

**Anki side:** one note per line, `front<sep>back<sep>tags`, where
`<sep>` is a tab by default or a comma for the CSV variant. The tags
column is optional and space-separated. Lines starting with `#` (Anki
writes header lines like `#separator:tab`) and blank lines are skipped
on the way in. In the comma-separated form, a field containing a comma,
a quote, or a newline is wrapped in double quotes, with embedded quotes
doubled - standard CSV quoting, matching what Anki itself writes.

**JSON Lines side:** one object per line with `front`, `back`, `tags`,
`media`, `plain_front`, `plain_back`, and optionally `due`,
`interval_days`, `ease`, `reps`, `lapses`.

Both directions stream: each line is read, converted, and written
before the next one is touched, so converting a deck with a few
hundred thousand cards doesn't pull the whole file into memory.

All three commands default to stdin/stdout, but accept `--input`/`-i`
and `--output`/`-o` to read or write a file path instead, in case
that's more convenient than shell redirection:

```
cargo run --release -- anki-to-jsonl --input deck.txt --output deck.jsonl
cargo run --release -- review 2026-09-18 -i reviewed.jsonl -o deck.jsonl
```

## Known limitations right now

- A field containing a literal tab character can't round-trip through
  the tab-separated Anki format; the converter errors out instead of
  silently mangling the card.
- CSV parsing assumes one note per input line - a quoted field that
  contains a literal newline (rare, but legal CSV) won't round-trip,
  since this tool reads and converts line by line.
- Only the plain-text Anki export is supported (tab or comma
  delimited), not the `.apkg` SQLite format.
- `due` is a plain `YYYY-MM-DD` string. `review` is the only command that
  writes it, by adding the new interval to the date it's told is "today" -
  it doesn't read a card's existing `due`, so the other two commands pass
  whatever is already there through untouched.
- `media` is only filled in on the way from Anki: `<img src="...">` and
  `[sound:...]` references are found and listed, but `jsonl-to-anki` doesn't
  use it - the front/back HTML already carries those references, so it just
  gets dropped like the scheduling fields do.
- `plain_front`/`plain_back` strip tags and decode the entities Anki
  actually writes (`&amp;`, `&nbsp;`, numeric references, and the like);
  they're a plain-text approximation, not a full HTML parser, so unusual
  markup may not come out exactly as a browser would render it.
