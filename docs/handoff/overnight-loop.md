# Overnight loop: toward 1.14d match, token-light (2026-10-08 22:00 → 2026-10-09 06:00 UTC)

Set by the user before sleeping: run until 06:00 UTC or until the work is
done, spending as few tokens as the work allows. The coordinator
(session_01QeN5r8PoLZsUhwAH2iDzLJ) follows this file at every wake (a
session report or the hourly check-in).

## Work sources, in priority order
1. Crashes and freezes on the real install (any row or report naming one).
2. Differences from 1.14d found by real comparisons: `q-fix-real-*` rows
   (q-realdata-run, q-fixture-migrate), REC-290 and facts-compare results
   (q-cloud-game).
3. Open `q-fix-*` rows in `docs/handoff/build-queue.tsv` from the audits
   (ui, render, seam, flow, audio, items, proto, save, prov).
Cloud REC ids for new provisional points: REC-400..499, next free REC-490; real-skills: next 462 (480s: q-fix-audit-rest, 470s: q-fix-real-next, 460s: q-fix-real-skills (items-play used REC-289), 420s and 450s: q-fix-render-rest, 430s: q-fix-audio-sounds, 440s: q-cloud-game) (below 300 is
used up; 300..399 belong to PC 1). Rows that need the binary or a spec
decision are not cloud work: list
them for PC 1 in `docs/handoff/pc1-data.md` §Step 4 and skip.

## Token rules
- At most 8 sessions running at once (fix + data sessions). Do not launch
  while 8 run; launch the next row when one finishes.
- New fix sessions: Sonnet by default. Opus only for: rendering seams,
  movement / prediction, RNG order or wire layouts, or a row a Sonnet
  session already failed.
- One session = one small group of rows in one area (≤ 6 rows), with the
  list given explicitly; it pushes, reports in ≤ 5 lines, and stops.
- The coordinator does not read diffs beyond the removed-assert check,
  does not re-explain, and replies to session reports only when they need
  an answer. Reports to the user: none overnight except the final one.

## Merge rules (unchanged gates)
- Batch every finished branch into the local integration branch, one full
  gate per batch (fmt, clippy -D warnings, nextest on d2-sim / d2-server /
  d2-client / test-fixtures, coverage, spec_index; debug info off). Push
  staging only on green. A red batch: drop the failing branch from the
  batch, send it back to its session (or a new Sonnet session) with the
  failing test names.
- Never weaken a test; a changed expectation only follows the spec or the
  real data and is named in the merge commit.
- `cargo clean` when the disk passes 85 %.

## End (06:00 UTC, or earlier when sources 1–3 are empty)
1. Stop launching. Ask running sessions to push what they have.
2. Last batch merge and gate.
3. Ship one release snapshot to main (claude/release-8m: workspace clippy
   without d2-client, depcheck, PR, merge on green CI).
4. Update `docs/handoff/coordinator-resume.md` and write
   `docs/handoff/overnight-report.md`: what merged, what was found against
   real 1.14d, what is left (M24: remaining work with sizes and
   uncertainty), what waits on PC 1.
5. Archive finished sessions; disable the hourly check-in.

## Day run 2026-10-09 (from 06:30 UTC, until the work sources are empty)

Same rules as the overnight run (gate, Sonnet by default, at most 8
sessions, hourly check-in). The 400s are used up; cloud REC blocks move
to 500..599, ten per session:

| Session | REC block | Model |
|---|---|---|
| q-fix-real-unit-seed-order | 500–509 | Opus (RNG order) |
| q-scenes-compare (merge q-facts-scenes, then compare) | 510–519 | Opus (rendering) |
| q-fix-server-store-fill | 520–529 | Sonnet |
| q-fixture-migrate-2 (G4–G7, F1–F3, gate snap-back) | 530–539 | Sonnet |
| q-fix-client-missiles-rest | 540–549 | Opus (rendering) |
| q-prov-recording (settle `recording` points under Wine) | 550–559 | Opus |
| q-prov-data (settle `data` points from the install) | 560–569 | Sonnet |
| q-prov-recording-2 (the d2-client `recording` points; q-prov-recording takes the rest) | 570–579 | Opus |
| q-tool-state-diff (per-tick unit state diff; one-command scenario diff) | 580–589 | Opus |
| q-tool-poke (state injection on both sides; test-install patch variants) | 590–599 | Opus |

Next free cloud id: REC-700 (PC 1 took REC-600..654 as its day block). Session cap raised to 13 for the PC 1 rows (2026-10-09 09:20).

Speed (user, 2026-10-09 08:00): sessions work in parallel with subagents,
run only the changed crate's tests while iterating and the full gate before
each push, push every 2–4 fixes, report in one short message per push, and
send binary-only questions to pc1-data.md instead of stopping.

PC 1 items: sessions add new `pc1-data.md` Step 4 items without a number, as
"- **[session] title**"; the coordinator numbers them at merge (two
sessions picking the same next number collided twice on 2026-10-09).
