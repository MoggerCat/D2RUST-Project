# q-fix-pc1-client-ui (`claude/q-fix-pc1-client-ui`)

> Hand-back, 2026-10-09. Everything below is pushed; the working tree is
> clean and staging (f17073a9) is merged.

## Done

- PC 1 client rows: seam-check-own-position, screen-to-world-y (the pick
  now follows REC-514, -4 rows; `camera.md` §4's "no -8" is superseded by
  the recording), click-seq-mode (`can_act` mode 18), preview-use-state
  (tests 1-4), preview-class-skill (comment only), object-generic-step
  (REC-725), hire-list-stats, anim-key-weapon-class, ui-globe-x87,
  ui-drop-cell, ui-npc-talk-facts, greeting-open, gossip, dialog-pass-abort,
  dialog-text-100-lf, chat-filter-utf8, shop-gamble-flag (reverted in code,
  REC-729), shop-repair-all, socket-left-down, automap x3, quest tab/icon,
  minipanel layout, render-bg-seed.
- ui-* audit rows: draw-sink (partial), input-pass / close-hooks / pause
  (already in), keys (partial), grid-hover / grid-msgs, item-tips (partial).
- q-fix-b41-skill-button-state (REC-720 settled): `ui/hud.rs`
  `skill_button_state`, `icon_draw` takes k.
- Esc / Space during an NPC interaction (autoplay blocker, REC-746):
  `ui/npc_talk.rs` `esc_key`, `ui/npc_box.rs`; sends C->S 0x30.

## In progress / partial (nothing half-pushed)

- draw-sink: HUD skill-button state is done (b41); text.md §8 framed text
  and the remaining callers of the draw modes are not switched.
- keys: missing belt / mini-panel tip short key names (`Key::name` still
  used), `controls.md` §4.2 r1 (last entry on one button wins), removing a
  wheel binding that has an up handler. Next: `controls/key_names.rs` to
  the HUD tips (HUD looks up text by string key, needs a `string.tbl` id).
- item-tips: one pop-up at the item anchor via `draw_tip_at`; the
  inventory / stash / cube / shop callers still pass the mouse point until
  `hover_lines` returns the hovered item's cell and size (§2 r4).
- Esc: the "box drawn more than 4 times" guard (REC-746) is not modelled;
  the topic-box and dialog-skip paths have no tests (only the menu box).

## Open RECs

REC-720 settled; open: 721, 722, 724 (preview use state tests 5-10 and the
HUD use state's missing mana / item / cooldown), 725, 726, 727, 728, 729,
740, 745, 746.

## PC 1 items (pc1-data.md Step 4, `[q-fix-pc1-client-ui]`)

- Real Gamble buy: what C->S 0x32 u32@9 carries (settles
  q-fix-shop-gamble-flag-dead; the click env keeps the OR 2 meanwhile).
- `0x00486BD0` (the 0x18 drop cell, REC-745) needs a spec task.
- Item 41 (skill button state) is answered by control-panel.md §7 r2.

## Repro / checks

```
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p d2-client --lib
# integration targets, in batches (disk is short; delete big test binaries between batches):
cargo test -p d2-client --no-fail-fast --test seam_world_screen --test e2e_vendor
python3 tools/coverage.py --check; python3 tools/spec_index.py --check
rustfmt +stable --edition 2021 <files>     # cargo fmt on the pinned toolchain is broken here
cargo +stable clippy -p d2-client --all-targets   # the pinned 1.99 clippy is not runnable in this container
```
d2-server `world_data_tables` races under default test threads (shared temp
dir); run it with `-- --test-threads=1`. `git stash list` holds two old
entries (stopped-agent partial work, already applied; an old WIP): safe to
drop.
