# Handoff: UI panels (`ui/panels.md`) in `d2-client`

Branch `claude/impl-ui-panels`, based on `claude/specs-staging` at
`5844674` (2026-10-06, cloud). Spec: `specs/ui/panels.md` (draft) with
`ui-states.tsv`, `panel-layout.tsv`, `npc-menus.tsv`. Status:
**implemented where the spec is exact, unverified** (no capture, no game
files in the cloud; checks queued in §4). UI state is client
presentation only: every outcome leaves as an encoded C→S message
(`ClientIntent`) for the root to forward through `bridge`, or as a
`SetUIState` request / effect for the caller.

## 1. What changed

| Path | What | Spec |
|---|---|---|
| `ui/layout.rs` | Strict loaders for the three TSVs (embedded from `specs/ui/`), the position grammar (`Expr`: `W`, `H`, `W2`, `sx`, `sy`, ints joined by one sign), `cond` words (`Cond`, `CondEnv`), `hit` rectangles (incl. `tab1_bottom`), `Screen` (`sx`/`sy` §1.1, slots §1.5, quads §1.4) | §1, §16 |
| `ui/states.rs` | `UiStates::set` = `SetUIState(ui, mode, jump)`: fatal checks, expansion-only states, the gate (chat / escape / modal text / conflict table actions 0–4, closes are full recursive calls), flag update by player life, hooks, open mode, cursor jump; side effects returned as `UiEffect` in call order (`Opened`, `Closed`, `InventoryHook`, `OpenMode` (`rules::camera::OpenMode`), `CursorX`, `Respawn`, `EndNpcInteraction`) | §2–§4 |
| `ui/panels/mod.rs` | `PanelTables`, `UiFiles` (file ids for `ImageRef`, `C` expanded to the class letters), `TextMeasure` (width A, measured by the sink), `cel` / `text` / `centered_in`, `emit_static_draws`, `PanelEnv`, `PanelOutput` (`Intent`, `SetUi`, `ClickSound`) | §1.6, §7 |
| `ui/panels/character.rs` | §8: art, close button, stat-points block, add buttons (0x3A in chunks of 32, shift = all), labels with the LF split, values with cmp colors, resist clamp, Font8 fallback | §8, §4.4 |
| `ui/panels/inventory.rs` | §9.3 art + close, §9.4 empty-slot backgrounds (two-handed rule, table order), §9.5 dead tab code as no-op | §9.3–§9.5 |
| `ui/panels/skilltree.rs` | Back art per class and tab, icons (position, strict hit, remap k in the stated order, returned beside the draw since `ImageRequest` has no remap), level numbers, tab mouse-down (tab-1 bottom without `sy`, click sound), 0x3B on release of the pressed icon, close button (amazon only), x ≤ W/2 not consumed | §10 |
| `ui/panels/waypoint.rs` | Background, tabs (exp 5 / classic 4, frame rule), close button and hover query, rows (icon frames, text colors), title; `open` / `choose_row` (0x49 guid + level) / `close` (0x49 level 0 + `SetUIState(0x14, off, jump 1)`) / `close_hook` / `self_close` latch | §13 |
| `ui/panels/stash_cube.rs` | Stash art, GoldMax line, close (0x4F 0x12, inventory mode 0); cube art, buttons, cube-gone close, 0x4F 0x17 / 0x18; Horadric animation as a pure step function | §11, §12 |
| `ui/panels/npc.rs` | `NpcMenus`: the 48 records, the per-interaction edits, Kashya's hire insert, option → intent (trade, gamble, travel / sail west, identify, resurrect) | §14.1–§14.2 |
| `ui/panels/shop.rs` | Shop art, tabs and captions, button positions, close → 0x30 | §14.4–§14.5 |
| `ui/panels/border.rs` | 800 × 600 border by open mode, control panel base (y = H, GDI) | §6 |
| `ui/root.rs` | `UiRoot::sync_states`: a root mirrors `UiStates` for panels whose id is a UI state; `PanelRules` docs (the original's rules are `UiStates`, which can refuse) | §2 |
| `ui/draw.rs`, `ui/panel.rs` | `ImageRequest.at` is the cel draw position (§1.3); `StringLookup::get_id` (default `None`) | §1.3 |
| `tests/game_panels.rs` | `#[ignore]` game-file test of the panel DC6 files | §1.4, §7.2, §Constants |

Tests: 28 in `ui::panels::tests` (tables, §1, the state machine with
every gate / open-mode / cursor-jump vector, strict parsers with
perturbations), plus the inline tests of each panel module, 1 in
`ui::tests` (root sync), 2 ignored in `tests/game_panels.rs`.

Decisions taken here (no spec choice involved): `UiStates` is the
authority and `UiRoot` mirrors it, instead of forcing the gate into
`PanelRules::on_open` (which cannot refuse or report effects). The 0x3A
message is built as `AddStatPoint { stat: stat | (n − 1) << 8 }`: the
proto TSV types the field as a u16 at byte 1, the spec writes it as two
bytes `[stat][n − 1]`; the bytes are identical (vectors pass), but the
proto field name should become two u8 fields (`combat/vitals.md` §2 owner).

## 2. Not wired

The panels are not added to the app's `UiRoot` and no input binding opens
them (key table: `ui/controls.md`, spec OQ 2). The sink needs: text
measuring (`TextMeasure` from the loaded fonts), string lookup by id
(`get_id`, `d2-data::strings`), file ids → DC6 (`UiFiles::name` gives
`data\global\ui\<name>.dc6`), the `panel\inv_*` and class skill-icon
files (not in `panel-layout.tsv`).

## 3. Open questions (spec gaps; nothing was guessed)

Character (§8): thousands grouping `0x00525350` (level, experience,
next-level not drawn); popup width `0x00502520` (Font8 fallback values
below 1,000 not drawn until given); number format of the other values
(assumed `%i` as §8.4); cmp on unshifted vs shifted values for 7, 9, 11;
color of 6, 8, 10; which resist effect wins when both are active; clamp
order when the cap < −100; add buttons with 0 points (follows the
`statpts` row condition); damage block, name / class lines, hover texts
(§8.10, spec OQ 3).

Inventory (§9): does mouse-up clear `[0x007BCE90]`; the inventory click
area bottom edge (§4.4); the `panel\inv_*` files have no layout rows.

Stash / cube (§11, §12): GoldMax font ("current font"); stash and cube
button press / release rectangles (only hover rectangles given); does a
stash close send 0x4F 0x12 once (release + close hook) or twice (sent
once); does the cube close button also call `SetUIState(0x1A, off)`;
Horadric frame 30 drawn or not, draw mode 3 has no field in
`ImageRequest`.

NPC / shop (§14): Resurrect insert / remove slot (`0x004B6440`); Cain's
count reset (`0x004B5640`); talk message bytes; hire sender (OQ 8); the
lookup of NPC 257 (two records: first used); the flag byte; the menu box
(OQ 8); shop button file, mode derivation, tab / button hit rectangles,
`0x004B3500` meaning; buy / sell / repair client fields (price, 0x35 u16,
transaction mapping) — not built.

Skill tree (§10): class icon file prefix (`CC`) — the caller supplies
the icon file; remap k unverified (OQ 4); close-button offsets for
classes other than amazon (`0x00724CE4`); the no-points mouse-down
message, free-points box, tab tool tips (OQ 4); close press rectangle and
its test order against the overlapping column 3 / row 6 icon; whether
"free points" is re-checked at release; draw order of icons / numbers /
close (section order used).

Waypoint (§13): tab and row click rectangles and tab switch (OQ 7: the
caller calls `choose_row` / `close`); filled hover rectangle extent; mode
and jump of the self-close (off, 0 used, from §4.3's call-site scan) and
when its latch resets; does a close via `0x0049CEC0` send 0x49 twice
(release + hook); color of an unknown selected row; tab / row on open.

Border (§6.3): control panel overlays (OQ 1).

## 4. Gate

`cargo fmt`, `cargo clippy -p d2-client --lib --tests -D warnings`,
`coverage.py --check`, `spec_index.py --check` pass. `cargo test -p
d2-client --lib`: every `ui::` test of this branch passes (86 in
`ui::panels::`); **47 tests fail on the base already**, outside this
work: every test that builds a `Bridge` (`bridge::msg::tests_*`,
`bridge::local_tests::*`, `ui::tests::only_the_root_forwards_intents_in_order`,
`ui::tests::refused_intent_stays_queued`) panics with
`Table(Mismatch([NoHandler { id: 122 }, NoHandler { id: 129 }]))`:
S→C 0x7A and 0x81 rows came in with the staging merge of the spec
branches and the client handler table has no entry for them. This branch
changes nothing in `bridge/`, `d2-proto` or `specs/sim/` (bridge/model
session's area).

## 5. Local run queue

1. HANDOFF §5 C71: `D2_GAME_DIR=<install> cargo test -p d2-client --test
   game_panels -- --ignored` (panel DC6 sizes, counts, offsets).
2. Capture cases `placement-0001` (inventory frames), `ui-0001`, `ui-0002`
   (spec §Test vectors) once the panels are wired into the app.
