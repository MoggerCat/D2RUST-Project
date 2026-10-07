# Spec: Client — UI panels, text and input mapping

- **Status:** draft; d2rs-own design draft (2026-10-06, architecture
  session). Part (a) is our design, including the controls file format
  (`d2controls 1`, ours). Part (b) lists original behavior to reproduce;
  each item names an owner spec to be written locally (RE).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::ui` (panel tree, layout, hit tests, text
  layout; plain Rust, no Bevy types), `d2-client::input` (Bevy input →
  actions), `d2-client::controls` (controls file)
- **Related specs:** `client/render-pipeline.md` (UI items are draw
  items), `client/assets.md`, `formats/font-tbl.md`, `formats/dc6.md`,
  `formats/tbl.md`, `formats/palette.md` (PL2 text colors),
  `sim/intents-events.md` (intents the UI sends)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 33–41 |
| Inputs | 42–51 |
| Outputs / state changes | 52–56 |
| Rules | 57–58 |
|   A. d2rs design (ours) | 59–167 |
|   B. Original behavior to reproduce (not specified here) | 168–289 |
| Constants & data dependencies | 290–293 |
| Randomness | 294–297 |
| Edge cases & original bugs | 298–301 |
| Test vectors | 302–316 |
| Provenance | 317–328 |
| Open questions | 329–347 |
<!-- /index -->

## Summary

The UI is our own small immediate-layout framework in plain Rust: panels
produce `DrawItem`s for the same compositor as the world, so UI pixels are
held to the same CPU-reference/GPU and original-capture checks. The UI
never decides outcomes: a click on an inventory cell becomes an intent;
what happens is the server's answer. Input goes through a rebindable
action map stored in a versioned controls file.

## Inputs

| Name | Type | Source |
|---|---|---|
| UI state | open panels, hovered element, cursor item | client-local |
| game state | inventory, stats, belt, skills, chat lines | bridge snapshot |
| input events | keys, mouse buttons, cursor position in 800×600 | Bevy (`d2-client::input`) |
| font | `FontTable` + glyph DC6 | `client/assets.md` |
| strings | string tables | `formats/tbl.md` via `d2-data::strings` |

## Outputs / state changes

UI `DrawItem`s (pass `ui` and `cursor`, `render-pipeline.md` §A6),
client-local UI state changes, intents to the bridge.

## Rules

### A. d2rs design (ours)

#### A1. Why not `bevy_ui`

`bevy_ui` lays out in floats (flexbox) and renders through its own
pipeline: neither is provably pixel-exact. Our UI uses integer pixel
coordinates in the 800×600 frame only, and the shared compositor.

#### A2. Panel model

```
trait Panel {
    fn id(&self) -> PanelId;
    fn rect(&self) -> Rect;                      // integer, 800×600 frame
    fn draw(&self, ctx: &UiCtx, out: &mut Vec<DrawItem>);
    fn hit(&self, p: IVec2) -> Option<WidgetId>; // integer hit test
    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse;
}
```

- `UiCtx` holds read-only snapshot data, fonts, string lookup, frame tick.
- `UiResponse` = `Consumed`, `Ignored`, or `Intent(ClientIntent)`; only
  the root forwards intents to the bridge.
- Panels are owned by a `UiRoot` in a fixed `Vec` order; event dispatch
  walks top-most first, drawing bottom-most first. Opening, closing and
  stacking rules of the original (which panels exclude which, left/right
  slots) are §B2; the root has a `PanelRules` hook for them.
- Widgets (button, frame image, text, grid of cells, scroll list, text
  input) are plain structs with integer rects; they emit items and
  answer hit tests. No retained GPU state.
- Every panel is testable by drawing it to the CPU reference and
  comparing images (`render-pipeline.md` §A10 case kind `ui`).

#### A3. Text

```
fn layout_text(font: &Font, text: &[u16], origin: IVec2, color: TextColor,
               opts: TextOpts) -> Vec<DrawItem>
```

- Glyph lookup is format-level: code → record → DC6 frame
  (`font-tbl.md`). Missing code: `TODO(spec)` error, not a fallback glyph.
- Advance, kerning, line height, baseline (`font-tbl.md` unknown fields),
  word wrap, alignment, color codes inside strings (`ÿc`), and which PL2
  text-color map recolors glyphs are §B3; `layout_text` calls into one
  `TextRules` implementation written from that spec.
- Strings are UTF-16 code units as the string tables hold them
  (`tbl.md`), never re-encoded.

#### A4. Input → actions

- Bevy keyboard/mouse events are mapped to `Action` values in
  `d2-client::input` (the only Bevy-facing part); everything after is
  plain Rust.
- `Action` is a closed enum, ours: movement/attack clicks, skill slots,
  belt slots, panel toggles, chat, map, run toggle, etc. Its list is
  completed from §B4 (the original's configurable commands).
- Mouse position is converted to the 800×600 frame by the inverse of the
  presentation scale (`render-pipeline.md` §A9) with integer division,
  then clamped; positions in the black bars are outside the frame.
- Actions on the world (move, attack, use skill on a point/unit) become
  intents (`sim/intents-events.md`) at the client tick they occur; the
  client never predicts their outcome.

#### A5. Logical resolution

One logical size: 800×600 (EARLY_DECISIONS 9: what is on screen affects
gameplay; one size for everyone). Panel layouts are defined for it. If
the original's 800×600 layout differs from its 640×480 one, only 800×600
is reproduced (§Open questions 1).

#### A6. Controls file `d2controls 1` (M20)

Location: `<config_dir>/d2rs/controls.toml` (no Blizzard data,
`client/assets.md` §A6). TOML, strict (M07): unknown keys, unknown
action names, unknown key names, duplicate bindings within one context,
and a missing or unsupported `version` are errors with line numbers; the
client then refuses to start with that file (no silent defaults).

```toml
version = 1                      # format version, required
preset = "original"              # base binding set; entries below override it

[bindings]                       # action = list of inputs (max 2, like slots)
toggle_inventory = ["I", "B"]
skill_slot_1     = ["F1"]
show_items       = ["LeftAlt"]
move_attack      = ["MouseLeft"]

[unbind]                         # actions explicitly left without input
list = ["toggle_automap_fade"]
```

Rules:
1. Effective binding = preset, then `[bindings]` overrides per action,
   then `[unbind]` clears.
2. One input may trigger one action per context (`world`, `chat`,
   `panel`); a clash after step 1 is an error naming both actions.
3. Key names are our closed list (`Key` enum names: `A`–`Z`, `0`–`9`,
   `F1`–`F12`, `LeftAlt`, `MouseLeft`, `MouseWheelUp`, …), not
   platform scancodes, so files are portable.
4. The writer emits keys in `Action` enum order (deterministic diff).
5. Newer `version` than supported: error. Older: migrated in code
   (`controls::migrate`), file rewritten only on explicit user save.

`preset = "original"` is the original's default key configuration (§B4);
until that spec exists, only `preset = "dev"` (ours, marked unverified
in code) is accepted.

### B. Original behavior to reproduce (not specified here)

| # | Behavior | Owner spec (to write) | Measure | Comparison |
|---|---|---|---|---|
| B1 | Panel art and layout: which DC6 files and frames each panel draws, positions at 800×600, control panel, belt, orbs (fill rule), minipanel | `ui/panels.md` | client UI draw path; captures | identical pixels on `ui` captures per panel |
| B2 | Panel open/close/stack rules, which panels exclude each other, world view shift when panels open | `ui/panels.md` | UI state code; captures | identical pixels and identical intents per input sequence |
| B3 | Text layout: advance (font `width` vs cell), line height, baseline, the font `.tbl` unknowns (`font-tbl.md` OQ 1), wrap, alignment, `ÿc` color codes, text-color PL2 maps, hover/item-name boxes | `ui/text.md` | text draw path; captures of known strings | identical pixels |
| B4 | Default key configuration, configurable command list, mouse semantics (left/right/shift/alt), repeat behavior, at which tick held buttons send repeated intents | `ui/controls.md` | 1.14d key config and input path; packet trace with known inputs | identical action list; identical C→S messages and send ticks for a scripted input |
| B5 | Inventory/stash/cube/belt grids: cell sizes, item graphic placement (`invfile`, sizes), hover highlight, cursor item drawing | `ui/inventory.md` | UI draw path; captures | identical pixels |
| B6 | Cursor: images, hotspots, animation frames, when it changes | `ui/panels.md` | captures | identical pixels |
| B7 | Automap: drawing, fade, cells revealed | `ui/automap.md` | captures | identical pixels |
| B8 | UI sounds triggered by panels and clicks | `client/audio.md` §B owners; site → control map below | audio trace | identical file and trigger tick |

#### B8.1 UI sound request sites → controls

Answers `audio/triggers.md` open question 12 (the control part; the
request path, ids and sounds stay in `audio/triggers.md` §11). Every
call of the UI request `0x004B9A00(id, no unit, 0, 0, 0)` whose id is a
constant in the 1.14d disassembly and is one of 1–6, 15, 16. "Constant"
includes an id built as `lea ecx, [r + k]` where r was set to a fixed
value earlier in the same function (`xor r, r` or `mov r, 1`). "Down" /
"up" = the mouse handler for button down / up of that UI's handler
table; "pressed" = the press flag the handler sets before the request.

1. **Count: 77 sites, not 72.** The scan of `re/exports/all.asm` finds
   the 72 sites of `audio/triggers.md` §11 plus five whose base
   register is set far from the call: `0x0045FE8B` (id 4), `0x004913F5`
   (4), `0x004A5E9A` (2), `0x004A60F9` (1), `0x004A7896` (5). Ids: 1 ×
   25, 2 × 8, 3 × 4, 4 × 28, 5 × 2, 6 × 8, 15 × 1, 16 × 1. Site
   `0x004C05C1` requests 3 or 5 (`0x58` code 5, arg u8@6 = 0 → 3, else
   5) and is counted once, under 3. `audio/triggers.md` §11 needs these
   counts (cross-file request to PC 2).
2. The options-menu sites (7) are owned by `audio/sound-table-2.md`
   §15 r5 (`audio/triggers-2.md` §17); they are listed here only for
   completeness.

   | Site | Id | Function | Control / event |
   |---|---|---|---|
   | `0x0047D646` | 1 | `0x0047D5C0` | options menu: choice entry activated (Enter / click release), `sound-table-2.md` §15 r5 |
   | `0x0047D667` | 2 | `0x0047D5C0` | options menu: action entry activated |
   | `0x0047D7E1` | 1 | `0x0047D670` | options menu: slider dragged to a new stop |
   | `0x0047D8F6` | 1 | `0x0047D8A0` | options menu: down arrow (next enabled entry) |
   | `0x0047D971` | 1 | `0x0047D920` | options menu: up arrow (previous enabled entry) |
   | `0x0047DA69` | 1 | `0x0047D9A0` | options menu: left arrow changed the value |
   | `0x0047DB59` | 1 | `0x0047DA90` | options menu: right arrow changed the value |
   | `0x0047AC66` | 1 | `0x0047AA60` | IME composition result into the 32-unit text field `[0x007BC850]` that does not fit (cut to 31 units, `[0x007BC88E]` := 0) |
   | `0x0048A74E` | 1 | `0x0048A730` | inventory weapon swap button (`ui/panels.md` §9, swap 0x60) |
   | `0x004A5A1B` | 1 | `0x004A59D0` | configure controls (UI_CONFIG 11): mouse wheel moved the key list |
   | `0x004A5A8F` | 1 | `0x004A5A40` | configure controls: up arrow moved the selected row (separator rows, key code 0x39, skipped) |
   | `0x004A5B09` | 1 | `0x004A5AB0` | configure controls: down arrow moved the selected row |
   | `0x004A5B68` | 1 | `0x004A5B30` | configure controls: left arrow, column flag `[0x007246D4]` 0 → 1 |
   | `0x004A5BC8` | 1 | `0x004A5B90` | configure controls: right arrow (the other column) |
   | `0x004A5CCB` | 1 | `0x004A5C70` | configure controls: Esc or Space closes the screen (`SetUIState(11, off)`, `0x0047E090`) |
   | `0x004A60F9` | 1 | `0x004A60B0` | configure controls, waiting for a key: Esc cancels the assignment |
   | `0x004B3B0B` | 1 | `0x004B3870` | NPC shop item click (`ui/menus.md` §4: "every other case") |
   | `0x004B713E` | 1 | `0x004B7100` | NPC menu box (`ui/menus.md` §2): keyboard moved the selection to another selectable item |
   | `0x004B7251` | 1 | `0x004B7200` | NPC menu box: the pointer selected a different item |
   | `0x004BE946` | 1 | `0x004BE910` | text list widget (`ui\menu\textslid`, vtable `0x006DAAF8`; the message log, UI_MSGLOG 24): line up |
   | `0x004BE9EE` | 1 | `0x004BE990` | text list widget: line down |
   | `0x004BF3F9` | 1 | `0x004BF2F0` | text list widget: position set by the slider |
   | `0x004BFADF` | 1 | `0x004BFA70` | item-socket dialog (UI 0x0E, `ui/messages.md` §11 r5): the placed item taken back onto the cursor |
   | `0x004C032A` | 1 | `0x004C02F0` | item-socket dialog: close button, Esc or Space (`ui/messages.md` §11 r6) |
   | `0x004C049D` | 1 | `0x004C0450` | item-socket dialog: button released inside its armed rectangle |
   | `0x004C2E61` | 1 | `0x004C2C80` | not a control: S→C 0x9D action 0x05 (RemoveFromContainer, `client/msg-stats-items.md` §2) removing an item of the local player, with the handler's flag byte = 1 |
   | `0x004A5C46` | 2 | `0x004A5BF0` | configure controls: Delete or Backspace cleared the selected binding |
   | `0x004A5E9A` | 2 | `0x004A5E60` | configure controls, button up on a key row: start the assignment |
   | `0x004A5ED5` | 2 | `0x004A5E60` | configure controls, button up on a bottom button (function table `0x007246DA`, stride 0x1A) |
   | `0x004A5FC4` | 2 | `0x004A5F70` | configure controls: Enter starts the assignment |
   | `0x004A613E` | 2 | `0x004A60B0` | configure controls, waiting for a key: a key was bound |
   | `0x004A620C` | 2 | `0x004A61A0` | configure controls, waiting for a key: a mouse button was bound |
   | `0x004BFB09` | 2 | `0x004BFAF0` | item-socket dialog: button 0 (imbue / ok) with an item placed |
   | `0x00489387` | 3 | `0x00489360` | trade screen (`client/msg-ui.md` §3, multiplayer) |
   | `0x00489825` | 3 | `0x004897E0` | trade screen |
   | `0x004BFD7F` | 3 | `0x004BFC50` | item-socket dialog: cursor item refused |
   | `0x004C05C1` | 3 / 5 | `0x004C0550` | not a control: S→C 0x58 code 5 (`client/msg-ui.md` §8) |
   | `0x0045FE8B` | 4 | `0x0045FE30` | one-button popup (`UI\Bigmenu\popupok`, loop `0x00460500` / `0x004605A0`): OK down |
   | `0x0046024B` | 4 | `0x004601F0` | two-button popup (`UI\Menu\okcancelbtn`, loop `0x00460680`): OK or Cancel down |
   | `0x0047F091` | 4 | `0x0047EF30` | mini panel button down (`ui/control-panel.md` §9 r7) |
   | `0x0048B780` | 4 | `0x0048B680` | hireling inventory (UI_MERCINV 36): close button down (x `sx + 272`…`sx + 304`, y `H + sy − 95`…`H + sy − 63`, inclusive), `[0x007BCEA0]` := 1 |
   | `0x0049134A` | 4 | `0x004912A0` | inventory grid handler: inventory gold button down (`[0x007BCE30]` := 1; modes 0, 0x0B, 0x0C, 0x0F) |
   | `0x0049138B` | 4 | `0x004912A0` | inventory grid handler: inventory close button down (`[0x007BCE90]` := 1) |
   | `0x004913F5` | 4 | `0x004912A0` | inventory grid handler: inventory close button down, second path (after `SetUIState`) |
   | `0x00491D98` | 4 | `0x00491D20` | NPC shop down: inventory close button (`ui/panels-2.md` §16 r13) |
   | `0x0049241E` | 4 | `0x00492310` | trade screen (inventory mode 0x0B, multiplayer): button down (`[0x007BCE18]` := 1) |
   | `0x00492469` | 4 | `0x00492310` | trade screen: button down (`[0x007BCE24]`, `[0x007BCE1C]` := 1) |
   | `0x004924B0` | 4 | `0x00492310` | trade screen: button down (`[0x007BCE30]` := 1) |
   | `0x004925E4` | 4 | `0x00492510` | stash: stash close button down (`ui/panels-2.md` §20 r1) |
   | `0x00492696` | 4 | `0x00492510` | stash: inventory gold button down (`[0x007BCE30]`) |
   | `0x004926E1` | 4 | `0x00492510` | stash: stash gold button down (`[0x007BCE34]`) |
   | `0x00492858` | 4 | `0x004927C0` | cube: cube close button down (`ui/panels-2.md` §20 r2) |
   | `0x004928AB` | 4 | `0x004927C0` | cube: transmute button down |
   | `0x00492946` | 4 | `0x004927C0` | cube: inventory gold button down (`[0x007BCE30]`) |
   | `0x0049965D` | 4 | `0x00499500` | control panel: menu or run button down (`ui/control-panel.md` §10 r1) |
   | `0x0049D225` | 4 | `0x0049D160` | waypoint menu: close button down (`ui/menus.md` §1 r2) |
   | `0x0049D248` | 4 | `0x0049D160` | waypoint menu: row down |
   | `0x004A2A4C` | 4 | `0x004A2A20` | quest-log alert button (UI_QUESTLOG 17) down (`[0x007BF2B0]` := 1) |
   | `0x004A3F65` | 4 | `0x004A3E40` | quest panel (UI_QUESTSCREEN 15): a quest icon down (`[0x007BF2B5]` := index) |
   | `0x004A3FB3` | 4 | `0x004A3E40` | quest panel: close button down (`[0x007BF2B4]` := 1) |
   | `0x004A6749` | 4 | `0x004A66E0` | new-stats button down (`[0x007C02E4]`, `ui/control-panel.md` §8) |
   | `0x004A67F9` | 4 | `0x004A6790` | new-skills button down (`[0x007C02E8]`) |
   | `0x004A778B` | 4 | `0x004A7720` | character panel: close button down (`[0x007C02F4]`, `ui/panels.md` §8 r3) |
   | `0x004ABAD8` | 4 | `0x004AB7E0` | skill tree: close button down (`ui/panels-2.md` §19) |
   | `0x004BACB3` | 4 | `0x004BAC70` | `ButtonWrapper` widget (vtable `0x006DA6F4`; made by `0x004BB0F0`, e.g. at (250, 287) in `0x00454440`): down inside it |
   | `0x004A7896` | 5 | `0x004A7720` | character panel: a stat add button down (`[0x00724A50 + …]` := 1) |
   | `0x004ABBF1` | 5 | `0x004AB7E0` | skill tree: a learnable skill icon down (`[0x007C0838 + 4i]` := 3) |
   | `0x00491F7B` | 6 | `0x00491D20` | NPC shop: tab 0 chosen (not the current page) |
   | `0x00491FB6` | 6 | `0x00491D20` | NPC shop: tab 1 |
   | `0x00491FE6` | 6 | `0x00491D20` | NPC shop: tab 2 |
   | `0x0049201F` | 6 | `0x00491D20` | NPC shop: tab 3 |
   | `0x0049D1CD` | 6 | `0x0049D160` | waypoint menu: another act tab |
   | `0x0049E585` | 6 | `0x0049E3A0` | not a control: screen message added (`ui/messages.md` §2 r3) |
   | `0x0049E7D0` | 6 | `0x0049E5C0` | not a control: event-text screen message added (from 0x5A, `client/msg-ui.md` §19) |
   | `0x004ABA32` | 6 | `0x004AB7E0` | skill tree: another tab |
   | `0x004B26A2` | 15 | `0x004B2650` | NPC repair sent (`ui/menus.md` §4) |
   | `0x0049E9AC` | 16 | `0x0049E8F0` | not a control: event text "hostile" (string 0xFBA; from 0x5A, `client/msg-ui.md` §19) |

3. Sites marked "not a control" are message effects; their owner is the
   message's spec, and the UI layer requests them as part of that
   message's output (`client/bridge.md` §10 r5).
4. Not listed: requests whose id is computed (item, NPC and skill sound
   tables), owned by `audio/triggers.md`.

## Constants & data dependencies

Logical frame 800×600 (§A5); controls format version 1.

## Randomness

None in the UI.

## Edge cases & original bugs

To be listed by the §B owners (reproduced by default).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| controls file without `version` | error, line 1 | §A6 |
| `version = 2` | error "unsupported version 2" | §A6 rule 5 |
| unknown action `foo = ["A"]` | error naming `foo` and its line | §A6 |
| two actions bound to `I` in context `world` | error naming both | §A6 rule 2 |
| preset `dev` + override `toggle_inventory = ["B"]` | inventory on `B` only | §A6 rule 1 |
| `[unbind] list = ["x"]` with `x` in `[bindings]` | `x` unbound | §A6 rule 1 |
| write then parse | same effective bindings, keys in enum order | §A6 rule 4 |
| window 1600×1200, cursor (1599, 1199) | frame (799, 599) | §A4 (scale 2) |
| window 1700×1200 (bars 50 px), cursor (25, 0) | outside frame | §A4 |
| hit test, two overlapping panels | top-most gets the event | §A2 |

## Provenance

Design decided 2026-10-06 (architecture session). Font and string format
facts come from `font-tbl.md` and `tbl.md`. §B8.1 (2026-10-08): a scan
of every `0x004B9A00` call in `re/exports/all.asm` (ECX traced back in
the function), handler tables read from the 1.14d image (`0x006D60C0`
UI_CONFIG, `0x006D615C` UI_QUESTLOG, `0x0070F8D8` UI_QUESTSCREEN,
`0x006D633C` UI_MERCINV, `0x00711D40` / `0x00711D70` popups; open hook
`0x00455720` jump table `0x00455A40`), the functions read with
`tools/ghidra/disasm.py` and the Ghidra export; control names from the
PC 2 `ui/*` specs where they own the handler.

## Open questions

1. Does the original's UI layout at 800×600 differ from 640×480 beyond
   centering, and must 640×480 be offered? (decision + `ui/panels.md`)
2. §B1–§B8.
3. Text input (chat) IME/clipboard: ours to decide once chat exists;
   no fidelity impact beyond the characters sent.
4. OQ 1 is answered for the panels by `ui/panels.md` §1: the 800 × 600
   layout is the 640 × 480 one moved by (80, 60), plus the 800 × 600
   border and control panel (§6 there). §B1 (panel art) and §B2
   (open/close, exclusion, view shift) are written in `ui/panels.md`;
   its §Open questions 1 lists the §B1/§B6 parts still open.
5. *Answered (2026-10-08).* §B8 UI control → sound site map
   (`audio/triggers.md` OQ12): §B8.1 (77 constant-id sites of
   `0x004B9A00`; the 7 options-menu sites stay owned by
   `audio/sound-table-2.md` §15 r5). Open: the field `[0x007BC850]` of
   `0x0047AC66` and the `ButtonWrapper` users of `0x004BACB3` are named
   only by address; Phase 6 UI specs name them.
