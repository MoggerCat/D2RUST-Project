# rc-render-wp-click: Act I waypoint click path (REC-2115, REC-2116)

Private repo attached (add_repo worked); game files + Wine used.

## Checks before -> after (EQUAL scenes: 0 -> 0 of 80; the gate moved)
- Before: all 80 effect scenes first differ at tick 3, `frame.tsv` row 5 `level` (1.14d 3, d2rs 1).
- After (fxfireball, fxbash, fxfrozenarmor re-rendered, 12 scenes): `level` equal in all.
  Fire Ball / Bash: first difference now `frame.tsv` row 8 `tile_origin_x`; Frozen Armor: `draws.tsv` row 97 (78 % pixels).

## Cause
1.14d hovers while it draws (`0x00467A10`): a press sees the cursor of the previous pass. The path's
third click (590,262) works because the cursor was still over the waypoint from click 2 (650,300).
d2rs picked at the click's own position, in a +-24 px box, so the waypoint was missed (28 px off at tick 121).

## Changed
- `ClickView::prev_hover`: hover from the previous pass (play `state.prev_hover`, `Headless::prev_pick`).
- Objects pick inside +-48 px (`hover::HIT_HALF_WIDTH_OBJECT`), PROVISIONAL REC-2116 (1.14d hit-tests sprite pixels).
- Test: monster click now moves a frame before the press; new test for same-pass press = point click.
- Spec note in `specs/tools/scenario-diff.md` §3 r8.2.
- Host clock (0x49 hostile delay, REC-2115 unused): not the cause; d2rs's clock starting at 0 would refuse a fast
  headless run before 10 s (state-dump starts at 1 s). Left alone.

## Open
- tile_origin_x differs after the waypoint: walk position/timing (`q-chk-render-world.md` 1-2), owner q-tool-state-diff. Size M.
- Frozen Armor: draws row 97 file name differs (aura/overlay art), S.
- Other 17 groups not re-rendered (same path; expect the same level fix).
- Not run: coverage.py, spec_index.py, ledger.py --check (see push).
