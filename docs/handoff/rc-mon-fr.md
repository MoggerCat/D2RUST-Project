# rc-mon-fr hand-back (2026-10-10, REC-2065)

Task: monster `fr` (current frame, unit +0x44) divergences, ~60 rows.

## Root cause (one)
d2rs had no plain (non-sequence) frame advance `0x00623E00`: the monster
`refresh_animation` hook was a no-op everywhere, so +0x44 and +0x4E never
moved after the mode start. 1.14d advances it after the used skill's do in
the attack-family event 0 `0x005A7670` (and after the step in S3, KB, SQ
and the stepping death). Example gen-mon-507 (bloodlord2 A2): enters the
mode at fr 7936 (> count 5120), event 0 at frame 41 wraps it to 3024.

## Changed
- `d2-sim/units/anim.rs`: `advance_plain` (spec `sim/units.md` §4.2
  "Frame advance and +0x4E"), unit tests with the gen-mon-507 vector.
- `ActionHooks::refresh_animation` (sequence branch, else plain; monster
  frame bonus 0) replaces the empty `Pending` hook at 6 call sites.
- 4 d2-sim tests that relied on the no-op were given real animation data.
- Spec `sim/units.md` §4.2: callers and the 7936 -> 3024 case.
- Ledger part `docs/handoff/ledger/rc-mon-fr.tsv` (65 rows).

## Checks (gen checks of the `fr` cluster, 65, `--no-playthrough`)
- first divergence on `fr`: 65 before -> 3 after (gen-mon-117 f100,
  gen-mon-509/510 f97; the latter were f51 before).
- MATCH: 0 -> 0 (each check has a further divergence; the cluster
  moved to other fields: lvl 18, s 9, seed 4, m 4, hp 4, tx 4, sp 3).
- d2-sim nextest 4756 pass; clippy, coverage, spec_index, ledger clean.
- Orig recordings reused from traces/orig-cache; none re-recorded.

## Open
- gen-mon-117 (mosquito4, f100, 3328 vs 4608) and gen-mon-509/510
  (bloodlord4/5, f97, 23296 vs 3077): probably a mode that starts with
  another frame/speed rule (re-init `0x00624390` or a get-hit rate); not
  read yet. Size: 3 rows.
- gen-su-48 and Megademon gen-su-362/686 were not in the filter (not
  rechecked); rc-mon-frame31 hand-back not read.
- Next fields per cluster: `lvl` (18), `s` (9): other owners.
- The WL/RN walk events were not checked for the same advance.
