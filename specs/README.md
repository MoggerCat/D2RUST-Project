# How to write a spec

Specs are the source of truth for the code and the project's memory
between sessions. An implementer reads only `specs/`, `docs/` and
`crates/`, so a spec must be complete enough to build the behavior exactly
without opening `re/` or `../refs/`. Template: `_TEMPLATE.md`.

## The bar

1. **Confirmed against 1.14d.** Every fact names its evidence: a
   measurement on the 1.14d files (file, count, size, value), a 1.14d
   `Game.exe` address, an executable check (`data-tool tables`, `mpq-tool
   formats`, `d2-client verify`) or a trace id. D2MOO documents 1.10f: a
   hint, never proof. Say how each D2MOO-derived rule was confirmed and
   note where 1.14d differs.
2. **Guesses are open questions.** Anything unconfirmed goes in "Open
   questions": numbered, one line each, saying what would settle it. Never
   state a guess as fact.
3. **One owner per rule.** Each rule lives in exactly one spec; others
   link to it (`loading.md` §6) instead of restating it. Restated rules
   drift apart. Data-layer owners: `data/loading.md` (which file, load
   order, checks), `data/txt-format.md` (parsing), `data/field-types.md`
   (cell → bytes), `data/calc-expressions.md` (formulas),
   `data/fields.tsv` + `tables.tsv` (layouts), `data/patch-layers.md`
   (patches).
4. **Dense and plain.** Short sentences, tables for anything structured,
   exact numbers, units and byte orders. No filler, no history of how the
   work went (that goes in git and `docs/HANDOFF.md`). If a sentence
   carries no fact or rule, cut it.
5. **Machine-readable when code consumes it.** Lists and tables that code
   needs (field layouts, constant tables, opcode lists) go in a TSV with a
   header row, documented by a sibling `.md`, embedded with `include_str!`
   and checked mechanically. See `data/schema.md` + `data/fields.tsv`.
6. **Test vectors that can run.** Concrete inputs and exact outputs.
   Synthetic vectors for edge cases (CI-safe unit tests), real 1.14d
   values for game-file tests (`#[ignore]`, `D2_GAME_DIR`). Give the
   source of each vector.
7. **Strict about the real data.** Quirks in live files become rules (e.g.
   the case-sensitive `Expansion` row). Unused leftovers are listed with
   the reason they're unused. Original bugs are reproduced by default.
8. **Clean room.** Describe behavior the way you'd explain it to another
   programmer. Never paste decompiler output, assembly or near-verbatim
   D2MOO code. Small game facts (names, counts, a few values) are fine;
   whole tables are not.

## Status line

`draft` → `implemented` → `verified` → `conformance-passing`, followed by
one sentence of evidence, e.g. "verified: `data-tool tables` reproduces all
73 live tables (69 byte-identical, 4 explained)".

| Status | Meaning |
|---|---|
| draft | written; facts confirmed as far as the spec says |
| implemented | code exists and passes the spec's test vectors |
| verified | an executable check against the 1.14d files passes |
| conformance-passing | traces recorded from the original game match |

## Process

- One writer per spec (spec role: may read `re/`, `../refs/`, `game/`).
  Then prove it with an executable check rather than extra review rounds.
- When a check proves or corrects a fact, update the owning spec and add
  "confirmed by <check>" to Provenance.
- Implementation sessions that find a gap add an open question to the spec
  and let the real data decide; they don't peek at `re/` or `../refs/`.
- A spec past ~60 KB usually mixes several owners: split it.

## Good examples

| Spec | Shows |
|---|---|
| `data/loading.md` | inventory tables, provenance with 1.14d addresses, binding policy |
| `data/schema.md` + `fields.tsv` | machine-readable layouts with mechanical checks |
| `formats/tbl.md` | a small, complete format spec with test vectors |
| `render/map-preview.md` | rules decided from evidence (renders, probes) |
