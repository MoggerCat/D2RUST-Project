# Coordinator hand-over: 6-hour ledger push (2026-10-10)

Goal (owner): **70% EQUAL** (3133 of ~4476 rows) and **90% checked** (rows not NO-CHECK/UNKNOWN,
4028) by the deadline the owner gives you. Owner also wants **high token efficiency**: same speed,
fewer tokens. Start: EQUAL 1486, checked 2471 (staging f61de2413 + integ-r23).

## Branches
- `claude/specs-staging-7` = f61de2413 (gated, 7842 tests). Advance only by fast-forward to a gated sha.
- `claude/integ-r23` = staging + rc-packets-join-order (S->C from frame 3: 74 -> 522 of 524 MATCH)
  + rc-render-ui (cursor start). Not gated yet. Not merged: `claude/rc-drop-nor-timing`
  (conflict in crates/d2-sim/src/wiring/action/ai.rs; resolve, keep both behaviours).
- Briefs: `claude/rc-briefs`, `rc/<name>.md`, rules in `rc/RULES-template.txt`.
  **First edit:** replace the old coordinator id `session_01KcnkwCTXbuv5ZbToEUpBSj` with yours.

## Decisions already made by the owner (don't re-ask)
- REC-2055/2056 DECIDED: the d2rs `RUN_GAPS` client gap is not a game difference (PLAN.md log).
- Coverage rows (`cov.*`) are checked against the data tables; no written spec needed.
- Every session may read the Ghidra exports read-only (CLAUDE.md rule 3; private repo `re/`).
- Child sessions may `add_repo` MoggerCat/D2RUST-private-repo (allow rule on staging).
- Usage limits: ignore. Token efficiency: required (rules below).
- Next free REC id: 2400 (blocks of 10 per session).

## Live sessions you inherit
| session | branch | what to do |
|---|---|---|
| session_01WHXnRjNCbZeXetmd79RoQt | gate runner (context ~370K) | use for gates, or replace with a fresh gate session (cheaper) |
| session_01VTrAPGPvhpQA6L2XPng1QM | rc-damage-draws | wait for "done" |
| session_0116oypHfMyBVmCYij9dte4P | rc-mon-pos2 | tell it: reading re/exports IS allowed (rule 3); an earlier coordinator message said otherwise by mistake |
| session_01CrzoEcfJAo9NNQ6Xyavv1j | rc-gen-obj-six | idle since 08:12 with 3 fixes unpushed: tell it to push + hand back now |

## Waves (token-lean: few, bigger sessions; Sonnet unless marked)
Wave 1, launch at once (~14):
1. **runner-1..4**: re-run every existing check on staging (traces/checks/gen ~2170 + scenario checks),
   sharded 4 ways, 3 Wine workers each, reuse orig-cache; output only ledger parts
   `docs/handoff/ledger/runner-N.tsv` + a 20-line hand-back. No investigation.
2. **link-cov**: link NO-CHECK rows to checks that already exist (422 rows name a gen check in
   `note`); implement the data-table check for `cov.*` rows and run the 269 UNKNOWN + cov NO-CHECK;
   audit `needs_pc1=y` rows (517) and clear the flag where a Wine recording works.
3. **ghidra-tools**: build in `tools/ghidra/` (read the private repo `re/` at run time, commit no
   decompiled text): `lookup.py` (name/address/xref/string search over functions.tsv + index/),
   `site2fn.py` (rng draw site or first-difference address -> containing 1.14d function + callers),
   `decomp1.sh` (decompile one function with current names via cloud_setup.sh). Selftests. Then add
   one line to RULES-template pointing to them.
4. **join-burst (Opus)**: frame-2 join burst (S->C 0xAA AddUnit 39 vs 12 bytes, 0x59..0x7E order);
   first diff in 450 packets checks and all 184 gen-state rows. Read 1.14d first.
5. **gen-a**: generators for system.formats (55) + skills by class (164). **gen-b**: ui (174) +
   client (85). **gen-c**: render (89) + audio (37). **gen-d**: net.s2c (82) + monster.ai (107).
   Each writes the check family, runs it, writes the ledger part.
6. **spec-monskill**: specs for skill.monster (64 rows, no spec) from re/exports, then checks.
7. **mon-causes-1..2**: top unowned clusters in docs/handoff/rc-gen-mon-causes.tsv (5 each).
8. **a5-nihlathak**: Act V playthrough stall at nihlathak-killed (bisect r11..r12 was unfinished).

Wave 2 (after runners report, ~T+1.5h): one session per top cause in the refreshed
`docs/handoff/rc-queue.tsv` (largest row count first, max ~12 at once, Opus only for shared/hard).

## Merging and gates
- Merge each "done" branch into the open `claude/integ-rN` with
  `tools/coord/coordinator/mergeone.sh <branch>` (exit 2 = code conflict: resolve by hand, keep both).
  It regenerates the ledger. Then `cargo test -q -p d2-sim --lib`, `ledger.py --check`, push.
- Gate every ~45-60 min (or after ~5 merges): message the gate session with branch + sha; on
  `fail=0` fast-forward staging. New sessions always branch from staging.
- Freeze merges 45 min before the deadline; final gate; regenerate ledger; report EQUAL / checked.

## Token rules for you (the coordinator)
- Never call list_sessions with a large limit (100 rows = 250K chars). Use get_session per id and
  rely on "done" messages; parse saved tool output with python, print one line per session.
- Never print files, logs or diffs; `tail -3`, `grep -c`, one-line summaries.
- Short launch prompts: "Read rc/<name>.md on claude/rc-briefs and rc/RULES-template.txt; do it."
- Keep a running log in your scratchpad; one line per event.

## Owner's PC (pc1) runs in parallel
Branch `claude/local-pc1-today`: pc1-data.md Step 4 items + needs_pc1 rows Wine can't do.
Merge it like any session branch when it pushes (every ~45 min).
