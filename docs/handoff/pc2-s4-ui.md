# Handoff: PC 2 spec worker, session 4, lane `ui`

Branch `claude/spec-ui-s4` (base `origin/claude/pc2-spec-gaps`
`dd480ad`). Files: `specs/ui/panels.md`, `panels-2.md`, new
`panels-3.md`, `controls.md`, `inventory.md`, `control-panel.md`. RE on
the 1.14d `Game.exe` (exports, `tools/ghidra/disasm.py`, image reads and
DC6 headers by scripts outside the repo). No capture, no trace.

## Written (spec § → behaviour, addresses)

| Spec § | Behaviour | 1.14d addresses |
|---|---|---|
| `panels.md` §9 r6 | corrected: `0x004845A0` is the equipment draw; draw order after the grid (shop extras, gold line, close button, hover box) | `0x0048EDF0` |
| `panels.md` OQ 2, OQ 5 | answered (controls §3; panels-2 §21) | — |
| `panels-2.md` §21 | gold lines (stash / inventory / shop), `goldcoinbtn` buttons, hover, hit rects, press / release, gold dialog kinds 0–4, OK → C→S 0x50 / 0x4F 0x13 / 0x4F 0x14 | `0x00488100`, `0x00486820`, `0x00486DA0`, `0x00492310`, `0x00489AC0`, `0x00454150`, `0x00453EE0`, `0x00454080` |
| `panels-2.md` §22 | d2rs answers: button frames (press only), label pen, no wheel scrolling (scan of every 0x20A handler entry), UI image request = cel draw point + shading + blend per draw mode, open mode source | handler tables `0x0070F2C8`, `0x006D6028`, `0x00724790`, `0x007273E8`, `0x0072DDCC` |
| `panels-3.md` §23 | cursor: 7 types, init, move / down / up transitions, item cursor, shop cursors, step (client seed draw), draw (item graphic centred), dead copy | `0x004680B0`–`0x00468840`, table `0x00712010` |
| `panels-3.md` §24 | character values: state masks (`armblue` … `rpred`), exact resist penalty, defense color (Holy Shield), language id, popup width | `0x0063A130`, `0x0063A550`–`0x0063A670`, `0x00611D30`, `0x004A86BB` |
| `panels-3.md` §25 | skill tree: learnability per frame, level number (`skill_level` with bonus), remap order (corrects `panels.md` §10.3 last clause), `[0x006CE270]` = 4 (`InGame`) | `0x004AC200`, `0x004ABF60`, `0x006447D0`, `0x00644920` |
| `panels-3.md` §26 | waypoint open sequence and row rebuild (tab level ranges, index walk, used byte, other-known count) | `0x0049CF90`, `0x0049C7F0`, table `0x007224AC` |
| `panels-3.md` §27 | scroll panel ui 0x10 (Inifuss / deciphered / Horadric: base, symbols animation), recipe scroll ui 0x25, guild states, anvil link | `0x0049FF10`, `0x0049FA10`, `0x0049FBA0`, `0x0048BC10` |
| `inventory.md` §2 r3 | palette byte order (red first) and tint blend (rect mode 0 = `A2`) | `0x0081E668` |
| `inventory.md` §5 r3 | cursor cell formula with a cursor item | `0x00487000` |
| `inventory.md` §8 | item graphic: file choice (set / unique / variant / invfile), path, visibility, draw point, ethereal mode 1, inventory color map, overlay call | `0x0046EE80`, `0x004DABC0`, `0x0062E920`, `0x005FE610` |
| `inventory.md` §9 | the tint checks (`0x0062A4E0`, `0x004C2240`, `Transmogrify`, `Shoots` / `Quiver`, quest test) | `0x00483F80` … |
| `controls.md` §6 | world clicks: event → kind 0–5, dispatcher gates and per-pass latch, click record, press / held / release filter, per-kind steps, held repeat per client loop pass, skill codes = C→S ids | `0x00462D00`, `0x00462930`, `0x004629A0`–`0x00462CA0`, `0x00461700`, `0x00481030`, `0x00480B40` |
| `controls.md` §7 | gates (`[0x007A0620]` game-exit flag, dead / absent player, game type), belt keys and belt use (C→S 0x26), key-mode latch = gold dialog, pointer button meanings (§B4) | `0x00498C50`, `0x00498A90`, `0x00498A20` |
| `control-panel.md` §5 r10–r12 | belt box hit, belt click (C→S 0x23 / 0x24 / 0x25), hover tracking | `0x004987E0`, `0x00498870`, `0x00498930` |
| `control-panel.md` §8 r4–r5 | new-stats / new-skills press and release | `0x004A66E0`, `0x004A6790`, `0x004A6840`, `0x004A6920` |

Code hooks answered: `ui/panel.rs` `PointerButton` (controls §7 r5),
`world_view/present.rs` and `ui_bind.rs` unhandled events (controls §6),
`ui/widget.rs` Button / Label / ScrollList (panels-2 §22 r1–r3),
`CellGrid` (inventory §7, §B5), `world_view/feed.rs` open mode (panels-2
§22 r5), `ui_bind.rs` `ui_image` (panels-2 §22 r4). `ui::original::PENDING`:
character values (panels-3 §24), skill tree icons / levels (§25),
waypoint panel (§26) are now fully specified; inventory equipment
backgrounds wait on the item stream decode (`client/msg-stats-items.md`
OQ 3, not a UI gap); stash / cube panels are specified (`panels.md`
§11–§12, `panels-2.md` §20–§21): wiring only; NPC menu openers are the
NPC interaction messages (`ui/messages.md` §14): wiring; the cursor jump
is an app edge.

## Pending (binary did not settle it here)

- `panels-2.md` OQ 6: gold dialog controls (edit box `0x004BBD80`,
  buttons `0x004BB0F0`, `0x004BC480`, box `0x004B7CD0`) art and input.
- `panels-3.md` OQ 1–4: writers of the scroll symbol slots
  `[0x007BF098]` / `[0x007BF254]`; openers of ui 0x1B–0x1D, 0x20; player
  trade panel (multiplayer); the cursor handler table messages.
- `inventory.md` OQ 7: gold picture frame from the amount class.
- `controls.md` OQ 8: the predicate order inside `0x004625B0` and its
  senders; OQ 2 send ticks (recording).
- `control-panel.md` OQ 5 rest: `[0x007BEFD4]`, `0x004B3470`.

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

- `render/capture.md` (PC 1); §3.3: the cursor rule is now owned by
  `ui/panels-3.md` §23 (adds the move / down / up transitions
  `0x00468840`, `0x00467F20`, `0x00467FA0`, the item and shop cursors
  `0x00468070`, `0x00468010`, `0x00468040`, the dead copy `0x004685C0`);
  keep the recorded-globals table and replace the rule text with that
  link.
- `client/ui.md` (PC 1); §B table rows B1, B2, B6: owners written
  (`ui/panels-2.md` §22, `ui/panels-3.md` §23); B4 rest: `ui/controls.md`
  §6–§7; B5: `ui/inventory.md` §8–§9.
- `client/msg-ui.md` (PC 1); OQ 4: the client quest record
  `0x004B32D0()` is read by the waypoint tab gates (`ui/panels-3.md` §26
  r4: records 7, 15, 23, 26, 28 bit 0) and the inventory quest tint
  (`ui/inventory.md` §9 r5: records 9, 20 bit 5, 37 bits 7–8); its
  writer is needed for both.
- `client/msg-stats-items.md` (PC 1); belt column-ready bytes
  `[0x007BEFB0 + c]` are written by `0x00498D50(c, v)` from
  `0x004C4130`, `0x004C42A0`, `0x004C4C70`; the belt keys require v = 1
  (`ui/controls.md` §7 r2): state which messages set / clear them.
- Owner of the C→S 0x4F handler (`0x0054C7C0` → `0x00568060`; candidate
  `world/vendors.md` or `world/cube.md`): buttons 0x13 (withdraw `v`
  from the stash) and 0x14 (deposit `v`), p1 = `v >> 16`, p2 = `v &
  0xFFFF` (`ui/panels-2.md` §21 r8), and 0x12 (stash close,
  `ui/panels.md` OQ 6).

## Follow-up (lane `ui-2`, branch `claude/spec-ui-s4`)

### Written

| Spec § | Behaviour | 1.14d addresses |
|---|---|---|
| `controls.md` §6 r8–r10 (OQ 8 partly answered) | world-click decision order (unit / point split, corpse and item-skill tests, "act on it", town gates, press-only monster walk), every sender (interact by unit type with reach 4 / 2 / 5 / 3, NPC stop C→S 0x59, town-portal `just_portaled` gate, warp 500-unit throttle; attack with melee test, Inferno / Arctic Blast approach; object; town player), use check and refusal sound, walk clamp (min step, 7-update held throttle), client hostility test, target re-pick | `0x004625B0`, `0x004621D0`, `0x004610C0`, `0x00461DC0` (table `0x004621AC`), `0x00461C70`, `0x00461B40`, `0x00461890`, `0x004619E0`, `0x00461840`, `0x004623C0`, `0x00465C60`, `0x00467880` |
| `panels.md` OQ 6; `panels-2.md` §21 r8 | links to `world/vendors-2.md` §10.2 (orchestrator relay) | — |
