# Coverage report

How every spec rule is traced to the checks that prove it (Phase 4,
"the 99.x% number"). Tool: `py tools/coverage.py` (§4).

## 1. Rules and their IDs

A rule is the smallest numbered or headed piece of a spec's behavior
text. IDs come from how specs are written today (`specs/_TEMPLATE.md`):
numbered headings (`### 5.`, `#### 5.2`), numbered lists under them
(`1.`, `2.`, …) and, in small format specs, unnumbered headings
(`### Header (21 bytes)`). No anchors are added to specs; the IDs are
read from the text.

| ID | Names | Example |
|---|---|---|
| `§<n>` | a heading numbered `5.2 Title` or `1. Title` | `specs/sim/tick.md §5.2` |
| `§<slug>` | an unnumbered heading: lower case, runs of other characters → `-` | `specs/formats/tbl.md §header-21-bytes`, `§edge-cases-original-bugs` |
| `§<s> r<N>` | item `N.` of the first numbered list (at column 0) directly under section `s` | `specs/sim/tick.md §5.2 r4` |
| `§<s> l<K> r<N>` | item `N.` of the K-th list of section `s` (a list restarting at `1.` starts list K ≥ 2) | `specs/sim/tick.md §5.5 l2 r1` (the "Consequences" list) |
| `§<s> text` | the section's own text (lines at column 0 outside its items: tables, prose, formulas) when the section also has items or subsections | `specs/sim/rng.md §3 text` (the helper table) |
| `§<s> row<N>` / `§<s> t<K> row<N>` | data row `N` (after the header and separator lines) of a table that an opt-in marker line makes row-numbered (below); `t<K>` names the K-th marked table of the section, K ≥ 2 | `specs/sim/rng.md §5.4 row2` |

Which sections hold rules: every `##` section of a spec except
Summary, Inputs, Outputs / state changes, Constants & data dependencies,
Randomness, Test vectors, Provenance, Open questions, Example, Survey
(1.14d data) and Observations (1.14d install). So Rules, Edge cases &
original bugs, policy sections and spec-specific sections (`mpq.md`
Archive set, `mpq-tables.md` A–D) count. Randomness restates the draw
order its Rules own; Constants are data the rules use; Test vectors are
the inputs of the checks, not rules.

**Row units (opt-in).** A table is plain text (part of `§<s> text`) unless
the line directly above it is `<!-- rows -->`. A marked table gives one
unit per data row, numbered from 1 by position; the marker line, header
and separator are not rule text. The section id and `§<s> text` keep
their meaning (the section still covers its rows; `text` stays the
unmarked prose), so no existing claim changes and a spec opts in table
by table. A marker not directly above a table is an error. Row numbers
are positional like list numbers: inserting a row renumbers the later
ones and the tool reports claims that no longer exist (§4).

**Units** (what the metric counts): every list item; every section
without items or subsections; every `§<s> text`. A claim on a section
covers every unit inside it, so a claim names the narrowest ID that is
fully true.

**Stability.** An ID changes when a spec renumbers a heading or a list.
The tool then reports every claim on the old ID as dangling (§4), so the
claims are fixed in the same change. A spec whose IDs repeat (two
headings with one number, a list numbered 1, 2, 2) is an error until it
is renumbered; nothing is guessed.

Not counted: rows of unmarked tables and of machine-readable spec tables (`fields.tsv`,
`tables.tsv`, `*-messages.tsv`). Their checks are whole-table
(`data-tool tables`, `tables_match_tsv`); a row-level metric would need
row IDs and is left for when it is needed (M10).

## 2. Claims

A test or check claims rules with one comment line directly above it
(only comments, attributes and blank lines in between):

```rust
// Covers: specs/sim/rng.md §3 text, §3 r2
#[test]
fn roll_vectors() {
```

```python
# Covers: specs/sim/rng.md §2, §3 text
def main(path):
```

Grammar: `Covers: <spec> §<id>[, §<id>…][; <spec> §<id>…]`. The spec is a
repository path under `specs/`.

Why comments, not a separate map (M05, M10): the project already names
specs in source comments (`// Spec: specs/...`, one per module, M03); a
claim beside its test moves, renames and dies with the test, so there is
one owner per claim and nothing to keep in sync. A TSV map would need its
own check that each named test still exists (parsing the same sources
anyway) and drifts when tests are renamed. The comment is checked
mechanically (§4): that is the M05 part.

**Tier** (what kind of proof), decided by the tool from where the claim
sits, never written by hand:

| Tier | Claim on | Runs against |
|---|---|---|
| `unit` | a `#[test]` without `#[ignore]` (spec test vectors, synthetic) | the spec |
| `game` | an `#[ignore]` test (reads `D2_GAME_DIR`), or a check function in `tools/data-tool` / `tools/mpq-tool` | the 1.14d files |
| `trace` | any function in `crates/conformance/` or `tools/trace-recorder/` (replay or check of a recording) | recordings of 1.14d |

A claim anywhere else (a non-test function in a library, Python outside
`tools/trace-recorder/`) is an error.

## 3. Metric

Per spec and in total, the number of units covered by each tier, plus:

- **verified**: covered by a `game` or `trace` check. Only these prove a
  rule against 1.14d (`CLAUDE.md` hard rule 10, M01). The "99.x%" number
  is verified / units.
- **any**: covered by any tier (a rule with only `unit` claims is
  implemented and unverified, M02).

A claim says what a check covers; that the check passes comes from
running it. `unit` checks run in CI. `game` and `trace` checks run
locally (`docs/HANDOFF.md` §4–§5); a claim on one counts as verified
while its latest recorded local run passes, and a failing one is fixed
or its claim removed in the same session.

## 4. Tool

`tools/coverage.py` (Python, standard library, like `spec_index.py` and
`methods.py`: it reads markdown and source text, needs no build, and CI
already runs Python before Cargo).

| Command | Does |
|---|---|
| `py tools/coverage.py` | per-spec and total table, then the uncovered units of each spec |
| `py tools/coverage.py --summary` | the table only |
| `py tools/coverage.py --rules specs/<file>.md` | every unit ID of one spec with its line |
| `py tools/coverage.py --check` | exit 1 on a dangling claim (no such spec or rule), a malformed claim, a claim on a non-check, or a repeated ID in a spec; never on low coverage (CI) |
| `py tools/coverage.py --selftest` | perturbation tests (M08): a synthetic spec and test file, each perturbed claim reported exactly; then a real claim of the repository renamed to a missing rule and reported at its file and line |

Strict (M07): unknown claim syntax is an error, never skipped.
