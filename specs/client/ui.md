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
| B8 | UI sounds triggered by panels and clicks | `client/audio.md` §B owners | audio trace | identical file and trigger tick |

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
facts come from `font-tbl.md` and `tbl.md`. No original behavior is
stated here.

## Open questions

1. Does the original's UI layout at 800×600 differ from 640×480 beyond
   centering, and must 640×480 be offered? (decision + `ui/panels.md`)
2. §B1–§B8.
3. Text input (chat) IME/clipboard: ours to decide once chat exists;
   no fidelity impact beyond the characters sent.
