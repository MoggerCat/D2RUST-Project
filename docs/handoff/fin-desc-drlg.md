# fin-desc-drlg handoff

Branch `claude/fin-desc-drlg`. Short session (45 min limit); most of it went to the first Bevy build of `d2-client`.

## Covered counts (any tier, rules claimed / claimable)

| Spec | Before | After |
|---|---|---|
| specs/skills/descriptions.md | 0 / 78 | 11 / 78 |
| specs/drlg/levels.md | 59 / 74 | 59 / 74 (not touched) |
| specs/drlg/rooms.md | 92 / 102 | 92 / 102 (not touched) |
| specs/drlg/preset.md | 54 / 64 | 54 / 64 (not touched) |

## Work done

- New `crates/d2-client/src/ui/skill_desc.rs` (spec crate/module): pure formula
  parts of the description functions: table bounds, `x*s/128`, range text
  (§2.1, §2.2 text), EType color, kick (entry 2), entries 8, 9, 10 sums, AR
  sum and color. Tests in `ui/tests_fdesc.rs` use the spec's test vectors.
- Claims: §1 r1, §1 r5, §2.1 r1, §2.1 r2, §2.11, §3 rows 2/8/9/10, Edge cases r1, r2.
- No code fixes needed; no spec changed.

## Left, with reasons

- descriptions §2.2 draw layout, §2.3 to §2.10 helpers, §3 rows 1, 3 to 7, 11 to 24,
  §4 rows 1 to 5, §2.9 charges, §2.10 hand swaps, §2.8 drivers: large new wiring
  (stat reads from the local player's client-side stat lists, weapon stat-list
  toggles, hand swap). Needs a client-side unit/stat-list read seam; not
  started. Table rows that are "computes and draws" also need the panel
  caller (`ui/panels-2.md` §17.5).
- drlg/levels, rooms, preset: not reached (15 / 10 / 10 uncovered rules; see
  `python3 tools/coverage.py`).

## Verification caveat

`d2-client` could not be built in this container (`wayland-client` system
library missing for `wayland-sys`). The module and its tests were compiled
and run standalone with `rustc --test` (9/9 pass); clippy and nextest on
`d2-client` were not run. The local run should do
`cargo nextest run -p d2-client ui::tests_fdesc` and clippy.
