# Split of the oversized specs (`split-large-specs`)

Branch `claude/split-large-specs`, from `main` at `674996d`. Spec maintenance
only: no rule reworded or renumbered, no code behaviour changed.

## 1. What was split

Line counts on `main` before the split: `skills/bodies-2.md` 2,259,
`monsters/ai.md` 1,737, `items/inventory.md` 1,651, `world/quests.md` 1,643,
`missiles/bodies.md` 943. The four files over ~1,500 lines were split along
their top-level `###` sections; `missiles/bodies.md` was left as is.

Old → new section map (section numbers and every rule id are unchanged; each
old file and its new file share no rule id, so a bare `§` reference resolves
to exactly one of them):

| Old file | Sections moved | New file | Lines after (old / new) |
|---|---|---|---|
| `specs/world/quests.md` | §10 Act I quests (§10.1–§10.8) | `specs/world/quests-act1.md` | 852 / 828 |
| `specs/monsters/ai.md` | §9 Per-AI behaviours (§9.1–§9.32, with the pointers to `ai-bodies-2.md`..`-5.md`) | `specs/monsters/ai-bodies.md` | 941 / 827 |
| `specs/items/inventory.md` | §6 Deferred item messages, §7 Intents, §8 Pickup from the ground, §9 Drop to the ground, §10 Gold, §11 Message layouts | `specs/items/inventory-moves.md` | 920 / 763 |
| `specs/skills/bodies-2.md` | §6 Bodies, required level 18; §7 … 24; §8 … 30 | `specs/skills/bodies-2b.md` | 1,439 / 853 |

Sections that stay in the old file: quests §1–§9, §11; ai §1–§8, §10;
inventory §1–§5; bodies-2 §1–§5. Constants, Randomness, Edge cases, Test
vectors, Provenance and Open questions stayed in the old files for both
halves (moving them would have renumbered their items).

Each new file has the template's metadata (Status copied from the old file),
a two-sentence Summary naming its origin, `## Rules`, and the moved sections
verbatim. Each old file's Summary ends with a "Split:" paragraph pointing to
the new file. The pointer sits in Summary, not under `## Rules`, so it adds no
`§rules text` unit (the unit count stays the same).

Check that the text is unchanged: the moved and kept text, with the inserted
`` `<file>.md` `` qualifiers removed and new file names mapped back, is
identical to `main` (difflib over all four pairs; only the Summary pointers
and index blocks differ).

## 2. References updated

- `Covers:` claims: 234 lines split per group (`specs/x.md §a; specs/x-new.md §b`).
- `// Spec:` headers: 21 headers with moved § refs rewritten; 41 files whose
  header named only one half but whose comments cite the other half got a
  second `// Spec:` line naming it.
- Prose, comments, string literals and TSV cells that named an old file with
  a moved § (388 occurrences, plus 15 where the § list continues on the next
  line, plus 34 hand-split mixed lists) across `specs/`, `docs/`, `crates/`.
- Bare § refs inside the four old and four new specs that point across the
  split were qualified with the other file's name (e.g. `` `quests.md` §6.3 ``
  inside `quests-act1.md`).
- `skills/functions.tsv` `body:` notes for §6–§8 now say `bodies-2b.md`;
  `skills/use_/tests/bodies.rs` accepts `body: bodies-2b.md §` (the
  `bodies_match_tsv_notes` check and its perturbation).
- `monsters/ai-functions.tsv` summaries, `d2-proto` S→C audit notes and
  `docs/handoff/s2c-builders.md` (the test compares the two), the
  `d2-server` handler id → spec tables.

Left on purpose:

- `world/object-functions.tsv` column `owner` keeps `world/quests.md`: it is
  a route key that `wiring/economy/quest_objects.rs` and
  `world/objects/tests.rs` match on, not a § reference.
- `docs/handoff/coverage-claims.md` (uncovered-unit snapshot lists) and
  `docs/handoff/impl-quests-act1-rest.md` line 5 (a change log of
  `quests.md` at that date) are dated records.
- Three bare refs in `bodies-2.md` that mean another spec's section, not the
  moved one: line ~110 `§8.1` (`monsters/init.md`), ~702 `§7.1`
  (`combat/damage.md`), ~765 `§6.5` (`bodies.md` skill stats). They were
  ambiguous before; left unqualified rather than pointed at the wrong file.
- Bare § refs in `docs/` prose that follow an old file name only several
  sentences earlier were not chased; the disjoint ids keep them resolvable.

## 3. Checks

| Check | Before (`main`) | After |
|---|---|---|
| `py tools/coverage.py --check` | 8149 claims, 0 errors | 8149 claims, 0 errors |
| rule units (`--summary` total) | 7342 | 7342 |
| covered units, any tier | 5934 | 5934 |

Per pair, the set of covered units of old + new after equals the old file's
set before. `py tools/coverage.py --selftest`, `py tools/spec_index.py
--check`, `cargo fmt --check` pass; `cargo test -p d2-proto` passes;
`cargo test -p d2-sim --lib`: 3145 passed, 0 failed, 8 ignored. `d2-server`
and `d2-client` tests were not run (only comments, spec-name string
literals and one-line `// Spec:` additions changed there).
