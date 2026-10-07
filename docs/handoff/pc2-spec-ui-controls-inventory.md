# Handoff: PC 2 spec worker — `ui/controls.md`, `ui/inventory.md` (Wave D)

Branch `claude/spec-ui-controls-inventory`, based on
`origin/claude/specs-staging-4` (2026-10-07). Two new specs, draft, RE on
the 1.14d `Game.exe` (exports + `tools/ghidra/disasm.py`, tables dumped by
scripts outside the repo); no capture, no packet trace.

## Answered

- `client/ui.md` §B4 (owner `ui/controls.md`): command table (57
  commands), compiled default keys `0x00712220`, `.key` / `default.key`
  formats and load / write order, keyboard and mouse dispatch, key modes,
  modifiers → `ui/controls.md` §1–§5; the original-defaults check is
  `ui/controls.md` §B4. Finding: the archives' `default.key` files are
  version 0x22 / 0x24 and are rejected by 1.14d, so the compiled table is
  the clean-install default; skills 9–16 have no default key.
- `ui/panels.md` §Open questions 2 (which key toggles which state, jump)
  → `ui/controls.md` §3 table.
- `client/ui.md` §B5 (owner `ui/inventory.md`): grid geometry, tint
  colours, grid item tint rules, placement tint, equipment box centring
  and tint, hover anchor → `ui/inventory.md` §1–§6, summary §B5.
- p6-controls OQ 1 (Action list) and OQ 2 (wheel / modifiers):
  `ui/controls.md` §3, §4.2, §4.3.

## Still open

- controls OQ 1 command labels for cmds 9, 10, 11, 41, 43, 45, 56
  (config-screen row table not found); OQ 2 left / right click semantics
  and repeat ticks (Needs recording); OQ 3–7 gates, belt use, key mode
  latch, Accept write, primary slot.
- inventory OQ 1 palette byte order + fill blend (Needs recording);
  OQ 2 item graphic draw `0x0046EE80`; OQ 3 cursor-cell formula with a
  cursor item; OQ 4 belt panel; OQ 5 cursor item drawing; OQ 6 helper
  checks.

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

- `ui/panels.md` (to PC 2 panels owner); §Open questions 2: answered by
  `ui/controls.md` §3 (CmdTbl `0x00712698`, defaults `0x00712220`); mark
  it "Answered" with that link.
- `ui/panels.md` (to PC 2 panels owner); §9 r6: the grid / equipped-item
  owner is now written, link `ui/inventory.md` §3–§6 (equipped items are
  `0x004845A0`, not `0x00483FF0`; `0x004845A0` is the equipment draw, so
  the "gold and other buttons (`0x004845A0`)" attribution there and in
  its §Open questions 5 looks wrong: recheck).
- `client/ui.md` (to PC 1); §B table rows B4 and B5: owner specs now
  exist (`ui/controls.md`, `ui/inventory.md`, draft); §A4 "Action list
  completed from §B4" can point to `ui/controls.md` §3.
- `crates/d2-client/src/controls/names.rs` / `mod.rs` (to PC 1 impl):
  the `original` preset can now be built from `ui/controls.md` §3 and
  checked by §B4.

## Recording list

- `ui/controls.md` OQ 2: packet trace of a scripted input: left click on
  ground, left click held 2 s, right click with a skill, Shift + left
  click, Ctrl + left click; record C→S messages and their ticks.
- `ui/inventory.md` OQ 1: 800 × 600 capture of the inventory with one
  identified usable item, one unidentified item, one unusable (red) item,
  a hovered item and a cursor item over free cells; read the tint pixels
  and compare with the palette.
