# rc-spec-monskill — hand-back (2026-10-10)

Branch `claude/rc-spec-monskill` (from `claude/specs-staging-7` + `claude/integ-r23`). REC-2600.

## Checks (family `monskill`, 64 rows `skill.monster.*`, state channel, 500 ticks)
| | before | after |
|---|---|---|
| ledger rows with a check | 0 (all NO-CHECK) | 64 |
| EQUAL (PARTIAL by REC-2055, 500/500 ticks) | 0 | 51 |
| DIVERGED | - | 13 (first run 17; 4 fixed below) |

## What changed
- `specs/skills/monster-skills.md` (new): per-skill index of the 64 rows (start /
  do slot, 1.14d address, body section, missile, monster users with `Sk<k>mode` /
  `Sk<k>lvl`, check name) and how a monster cast reaches the bodies (`use.md` §5.4).
- `tools/check-gen/check_gen.py` family `monskill` -> `traces/checks/gen/gen-monskill-<Id>.check`
  (64 files): the first enabled non-boss class that lists the skill, spawned
  at (+3, -2) of a level-70 expansion Amazon in the Blood Moor; the AI casts on its
  own. `ledger-areas.tsv` gets the 64 area rows. The level-1 character died in
  about 60 frames and stopped most casts (first try: 29 of 64 never cast).
- Fix, fire head (`missiles/bodies-2.md` §41, 3 checks 169/171/333): the wired
  host never gave `max_life` / `max_mana` to `MissileBodies`, so the heal clamped
  the owner's life to 0 (`wiring/action/missiles.rs`).
- Fix, Mosquito (206): the event-index store `0x006212C0` is the sequence position
  of a unit in a sequence; wired into `Sequence.pos` (`wiring/interaction/skill_use.rs`);
  spec note in `bodies-4.md` §3.2.
- Ledger part `docs/handoff/ledger/rc-spec-monskill.tsv` (64 rows) and the merged ledger.

## Open (sizes)
- No cast reached in 1.14d for 22 checks (156, 158, 166, 173, 176, 180, 210, 211,
  214, 215, 284, 290, 291, 293, 295, 300, 308, 321, 323, 328, 335, 352): idle and
  think frames only, PROVISIONAL REC-2600 (M). Needs a poke that sets the used skill.
- DIVERGED, causes not mine (route them):
  - player `sp` 80 vs 40 / 128 vs 64 after a hit: 210, 211, 212, 335 (S; the
    `rc-gen-mon-triage` "player sp" cluster).
  - player mode 4 vs 5 on a hit: 301, 339-343 (M); player seed differs at the hit:
    348 (M); the "draw at `0x005A55BA` on hit" cluster of `rc-gen-mon-triage`.
  - fetish pack member (class 141) walks in 1.14d, attacks in d2rs at frame 90:
    177, 178 (M; monster AI, `monsters/ai-bodies.md`).
- No skill body was found unimplemented by these checks; the 22 no-cast rows are
  untested, not known good.
- `/list.py` (stray scratch file at the container root, `$TMPDIR` empty); not in the repo.
- `traces/orig-cache/gen-monskill-*` recordings are left uncommitted by the rule.
