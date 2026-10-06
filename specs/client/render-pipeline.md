# Spec: Client — Render pipeline (sprites, shading, composition, verify)

- **Status:** draft; d2rs-own design draft (2026-10-06, architecture
  session). Part (a) is our own architecture and binds implementations.
  Part (b) lists original-game behavior the pipeline must reproduce; none
  of it is specified yet, each item names its owner spec (to be written
  locally, RE) and its exactness comparison. Nothing here is implemented.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::render` (GPU), `d2-client::scene` (draw
  list, CPU reference; plain Rust, no Bevy types), `d2-client::verify`
  (harness)
- **Related specs:** `render/map-preview.md` (the proven pattern this
  generalizes), `formats/dcc.md`, `formats/dc6.md`, `formats/cof.md`,
  `formats/palette.md`, `formats/animdata.md`, `client/assets.md`,
  `client/ui.md`

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 34–50 |
| Inputs | 51–60 |
| Outputs / state changes | 61–65 |
| Rules | 66–67 |
|   A. d2rs design (ours) | 68–248 |
|   B. Original behavior to reproduce (not specified here) | 249–267 |
| Constants & data dependencies | 268–274 |
| Randomness | 275–280 |
| Edge cases & original bugs | 281–286 |
| Test vectors | 287–301 |
| Provenance | 302–309 |
| Open questions | 310–322 |
<!-- /index -->

## Summary

Every frame the client turns the latest server snapshot (via
`d2-client::bridge`) into one ordered **draw list** of indexed sprite
items. A plain-Rust **CPU reference compositor** and a GPU **compute
compositor** both turn that list into the same 800×600 image. Fidelity is
proven in two independent links, each by an exact pixel comparison:

1. *original → CPU reference:* the CPU reference's frame equals a frame
   captured from 1.14d for the same recorded game state (owner specs in
   §B decide every rule the reference uses);
2. *CPU reference → GPU:* the GPU output is byte-identical to the CPU
   reference for the same draw list (`d2-client verify`, as Phase 1b).

Link 2 is pure infrastructure and can be built and proven now. Link 1
needs the RE specs of §B.

## Inputs

| Name | Type | Source |
|---|---|---|
| snapshot | bridge view of units, rooms, tiles, missiles, overlays at tick `t` | `d2-client::bridge` (Phase 5) |
| decoded frames | 8-bit index images + offsets | `d2-formats::{dcc,dc6,dt1}` via `client/assets.md` |
| COF | layer list, per-frame draw order, events | `d2-formats::cof` |
| palette, PL2 | 256 colors; 1,714 + 2T index maps | `d2-formats::palette` |
| UI draw items | same item type as the world | `client/ui.md` |

## Outputs / state changes

An 800×600 RGBA8 sRGB image per presented frame (alpha always 255). The
CPU reference produces the same image as `Vec<u8>`. No game state changes.

## Rules

### A. d2rs design (ours)

#### A1. Layers of the pipeline

| Stage | Module | Bevy? | Output |
|---|---|---|---|
| 1. Scene build | `scene::build` | no | `Vec<DrawItem>` + `FrameParams` |
| 2. Order | `scene::order` | no | items sorted by `DrawKey` (stable) |
| 3. Residency | `client/assets.md` §A4 | yes (handles) | every frame of the list resident in the atlas |
| 4a. CPU compose | `scene::cpu` | no | reference image |
| 4b. GPU compose | `render::compose` | yes | GPU image |
| 5. Present | `render::present` | yes | window (scaled, §A9) |

Stages 1, 2 and 4a are plain Rust: unit-testable in CI without a GPU and
shared by `view`, `verify` and the game. The GPU stage takes the exact
same `Vec<DrawItem>` the CPU stage takes; nothing is re-derived on the
GPU side.

#### A2. Indexed frames

Every DCC/DC6/DT1 frame decodes once to an `IndexFrame { width, height,
x_off, y_off, pixels: Vec<u8> }`, pixels row-major, top row first, index
0 = transparent. Offsets are the format's own values unchanged
(`dc6.md` §Frame, `dcc.md` frame box); how they turn into a screen
position is §B1, not decided here. DT1 tiles use the `map-preview.md`
tile image. Frames carry no color: color comes only from the item's
shade chain and the frame palette (§A4).

Frames live in a GPU **atlas**: R8Uint texture pages of 2048×2048, packed
by a deterministic shelf packer (insertion order, no randomness, no hash
iteration), 1-pixel gutter of index 0. An atlas slot is
`(page, x, y, w, h)`. The CPU reference reads the same `IndexFrame`
bytes, never the atlas, so packing errors show up as verify mismatches.

#### A3. Draw item

```
DrawItem {
  frame: FrameRef,          // atlas slot + IndexFrame id
  x: i32, y: i32,           // screen position of the image's top-left, integer pixels
  flip_x: bool,             // reserved; set only if an owner spec requires it
  clip: Rect,               // integer screen rect (UI panels, view edge)
  shade: ShadeChain,        // §A4
  blend: BlendOp,           // §A5
  key: DrawKey,             // §A6
  tag: ItemTag,             // debug only: unit GUID / tile cell / UI id
}
```

Positions are integers in the 800×600 frame. Sub-pixel interpolation does
not exist in the item: if the bridge interpolates (Phase 5), it rounds
to an integer before stage 1 under a rule owned by §B7.

#### A4. Shade chain and palette slots

All PL2 index maps of all loaded PL2 files go into one **map table**
(storage buffer of u8 rows of 256; the 256×256 blend tables are rows of
it too). A `MapId` is a row number. `ShadeChain` is an ordered list of up
to 4 `MapId`s; the composed index is

```
i' = map[k3][ map[k2][ map[k1][ map[k0][i] ] ] ]   (unused slots skipped)
```

Index 0 is tested **before** the chain (transparent pixels stay
transparent). Whether a mapped result of 0 is transparent is §B3. Which
maps go in which slot for a unit, tile, missile or UI element (light
level, colormap variation, selection highlight, text color) is §B3 and
§B4; the mechanism is ours. Each frame has one **frame palette** (256
colors) per screen region; regions exist only if §B3 requires different
palettes on one screen (e.g. world vs UI), otherwise one palette.

#### A5. Blend ops

`BlendOp` is a closed enum. Each op is defined by an integer formula on
(source index after the chain, destination value) and is implemented
twice from one table-driven description: CPU in Rust, GPU in WGSL.

| Op | Domain | Meaning |
|---|---|---|
| `Opaque` | either | dest = src |
| `IndexTable(MapId base)` | indexed | dest_index = map[base + src][dest_index] (PL2 256×256 tables) |
| `Rgb(RgbFormula)` | rgb | dest_rgb = integer formula of (palette[src], dest_rgb) |

Which domain the original composes in, the formulas and which op each
draw uses are §B2 and §B5. The compositor's **framebuffer domain** is a
build-time choice made once §B2 is answered: an index framebuffer (u8 per
pixel, palette applied at the end) or an RGB framebuffer (u32 per pixel).
Both are supported by the same compositor structure; no float appears in
either.

#### A6. Draw order

`DrawKey` is a `u64`; items are stably sorted by it (ties keep build
order). The key layout is ours: `pass:4 | major:28 | minor:24 | sub:8`,
where `pass` separates screen passes (floor, shadows, world, roof, UI,
cursor) and `major/minor/sub` are filled by the rule functions of §B6.
Build order of items within a unit is the COF slot order (§A7). The
compositor itself never reorders: draw order is entirely the list order.

#### A7. Composite units (COF)

For a unit in (token, mode, weapon class, direction, frame):

1. The COF is resolved by the path rule of §B4.
2. For frame `f` and COF direction `d`, slots `s = 0..L-1` in the order
   `cof.component_at(d, f, s)` (back to front, `cof.md` §Frame events and
   draw order) give components.
3. Each component maps to one DCC (component file path, §B4) and one
   frame of it, placed by §B1, shaded by §B3/§B4 (per-component
   colormaps, COF translucency override fields → §B5).
4. One `DrawItem` per drawn component, keys sharing the unit's
   `major/minor` with `sub` = slot index.

Which direction and frame to show come from the snapshot (the simulation
owns animation state; the client never advances an animation that the
sim can see). Mapping the unit's direction to a file's direction count is
§B4.

#### A8. CPU reference compositor

`scene::cpu::compose(items, frames, maps, palette, view) -> Vec<u8>`:
for each item in list order, for each pixel of its image inside
`clip ∩ view`, apply §A4 then §A5. Straightforward loops; speed is not a
goal. It is the only definition of the d2rs image; the GPU must match it.

#### A9. GPU compute compositor

Not fixed-function blending: GPU blend units are not specified to be
bit-exact, and indexed blends need the destination index. Design:

- The screen is split into 32×32 **bins**. Stage 1 also emits, per bin,
  the ordered list of item indices touching it (CPU, plain Rust).
- One workgroup per bin, one invocation per pixel. Each invocation walks
  its bin's list in order and updates a private integer pixel value with
  the same formulas as §A8; then writes it once to a storage texture
  (R8Uint or R32Uint by domain).
- A final pass maps the framebuffer to RGBA8 sRGB through the frame
  palette with `textureLoad` (indexed domain) or a bit copy (rgb domain).
- Presentation scales the 800×600 image to the window by integer factor,
  nearest neighbor, centered with black bars (outside the verify
  boundary; verify reads the 800×600 image before presentation).

Feasibility (checked 2026-10-06 in the pinned registry): Bevy 0.19.1's
`bevy_render` exposes compute pipelines (`ComputePipelineDescriptor`,
`PipelineCache`) and storage buffers/textures, so the compositor is a
render-graph node in `d2-client` only. All per-pixel math is u32/i32.

The Phase 1b `Material2d` palette path stays for `map-preview.md` until
its case is ported to the compositor (§A10 case `map`), then is removed.

#### A10. Verify harness extension

`d2-client verify` becomes a runner over **cases**. A case file
(`crates/d2-client/render-cases/*.toml`, versioned, M20, field `version = 1`)
references game paths and parameters only, never game data:

| Kind | Builds | Needs |
|---|---|---|
| `synthetic` | items from frames defined inline (index grids, maps) | repo only |
| `map` | `map-preview.md` layout (port of today's verify) | `game/` |
| `sprite` | one DCC/DC6 frame with a given shade chain and blend | `game/` |
| `unit` | one COF composite at (token, mode, wclass, dir, frame) | `game/` |
| `scene` | a recorded snapshot (bridge format) → full frame | `game/`, Phase 5 |
| `ui` | a panel or text item set (`client/ui.md`) | `game/` |

Per case: build items, CPU compose, GPU compose (headless, offscreen,
the stable-capture rule of Phase 1b), compare byte for byte; report
mismatched pixel count and first mismatch; `--perturb N` per case must
fail with exactly N (M08). `synthetic` cases run the CPU half in CI
(`cargo test -p d2-client`); the GPU half needs a GPU and runs locally.

Link 1 (original → CPU reference) uses `traces/render/` entries: per
capture, the recorded game state and the SHA-256 and size of the
captured 800×600 frame. The captured images themselves show Blizzard art
and stay in `game/captures/` (gitignored; CLAUDE.md rule 1). The check
renders the CPU reference for the recorded state and compares to the
image in `game/captures/` (local only), then also compares hashes.
Capturing frames from 1.14d is §B9 (`render/capture.md`; committed
format `traces/FORMAT.md` §Render captures).

### B. Original behavior to reproduce (not specified here)

Each item: owner spec to be written locally (RE), what must be measured,
and the exactness comparison. Until the owner exists, implementations
leave a hook that panics or a `TODO(spec: …)`, never a guess.

| # | Behavior | Owner spec (to write) | Measure | Comparison |
|---|---|---|---|---|
| B1 | Sprite placement: DCC/DC6 frame offsets → screen pixel (the one-row question, HANDOFF §7 #3), DT1 tile vs unit anchor | **written:** `render/sprite-placement.md` (draft; bottom row = `Y + offset_y` inclusive) | client draw path; captures of a unit at known coordinates | identical pixels on a `sprite`/`unit` capture (`capture.md` case `placement-0001`) |
| B2 | Composition domain of 1.14d's renderer (indexed with PL2 tables vs RGB) and which video mode is the reference | **written:** `render/composition.md` (draft; indexed, GDI reference) | 1.14d video modes and their draw path | identical pixels of one capture with translucent sprites (`composition-0001`) |
| B3 | Meaning of each PL2 table (`palette.md` OQ 2); light-level map selection; selected-unit shift; whether mapped index 0 is transparent; palettes per screen region | **written:** `render/shading.md` (draft; light map `v >> 3`, tile gradients, highlight map computed ×1.7, mapped 0 drawn as index 0, one palette) | client shading path; captures at known light levels | identical pixels |
| B4 | Unit composites: COF/DCC path rules, component variants (armor class letters), colormaps per component, direction mapping (unit dirs → file dirs), frame source (animdata vs COF rate) | `render/unit-composite.md` | client unit draw path; `animdata.md` use | identical pixels on `unit` captures across dirs/frames |
| B5 | Blend modes: COF translucency override, missiles, overlays, shadows (the darkening blend), formulas | `render/blend-modes.md` | blend path; captures | identical pixels |
| B6 | Draw order: floors, shadows, walls vs units (the isometric rules, `map-preview.md` OQ 2), roofs, missiles, overlays, UI | `render/draw-order.md` | client sort and passes | identical pixels on scenes with occlusion |
| B7 | Camera: world (subtile) → screen, view size and centering, interpolation between ticks (if any), screen shake | **written:** `render/camera.md` (draft; no interpolation) | client view path; captures while walking | identical pixels per tick (`camera-0001`) |
| B8 | Lighting: light radius, light sources, day/night, per-tile or per-pixel light level | `render/lighting.md` | client lighting path; captures at night/with torches | identical pixels |
| B9 | Frame capture of 1.14d: which surface, at which point in the frame, how frames are tied to ticks | **written:** `render/capture.md` (draft), `tools/trace-recorder/record_frames.py`, `traces/FORMAT.md` §Render captures | debugger hook on the present call | capture of a static scene repeats identically (`stability-0001`, first) |
| B10 | Tile variants by rarity and the invisible-collision-tile skip (`map-preview.md` OQ 3) | DRLG spec (sim) + `render/draw-order.md` | DRLG; tile draw path | identical pixels on town captures |

## Constants & data dependencies

Ours: frame 800×600 (EARLY_DECISIONS 9; whether 640×480 must also exist
is ui.md OQ), bins 32×32, atlas pages 2048×2048, shade chain ≤ 4,
`DrawKey` layout §A6. Each is a constant in `scene`/`render`, changeable
without format impact (no persisted format depends on them).

## Randomness

None in the pipeline. Any visual randomness of the original (tile
variants, effects) must arrive as data from the sim or from a client RNG
owned by a §B spec.

## Edge cases & original bugs

To be listed by the §B owners. Design rule: a draw item whose frame is
not resident is an error in verify and a stall in play (`client/assets.md`
§A4), never a skipped draw.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| synthetic 2×2 frame `[0,5;7,0]` at (10,10), `Opaque`, empty chain | pixels (11,10) = palette[5], (10,11) = palette[7], others background | §A3, §A8 |
| same frame, chain `[m]` with `m[5]=9` | (11,10) = palette[9] | §A4 |
| chain `[m1, m2]`, `m1[5]=9`, `m2[9]=3` | palette[3] | §A4 order |
| two overlapping opaque items, keys k1 < k2 | second visible on overlap | §A6 |
| equal keys | build order decides | §A6 stable sort |
| `IndexTable(base)`, synthetic 256×256 table | dest = table[src][dest] | §A5 |
| item clipped by `clip` | no pixel outside clip | §A3 |
| frame spanning 4 bins | same image as without binning | §A9 |
| any case with `--perturb N` | verify fails with exactly N | §A10, M08 |
| `map` case on `townN1.ds1` | identical to today's `d2-client verify` | §A9 port |

## Provenance

Design decided in the 2026-10-06 architecture session from the Phase 1b
result (`map-preview.md`: CPU reference + byte-exact GPU, requirements in
the PLAN decisions log) and the repo's format specs. No original-game
behavior is stated here; §B lists it as unwritten owner specs. Bevy
compute availability checked in the pinned `bevy_render 0.19.1` source.

## Open questions

1. §B3–§B6, §B8, §B10 (each an owner spec to write locally); §B1, §B2,
   §B7, §B9 written as drafts.
2. Whether the compute compositor reaches 60 frames per second at
   800×600 with a full town scene on the developer GPU: measure once
   implemented; if not, bins per item list may be culled by item bounding
   boxes first (same result, verify unchanged).
3. ~~Whether the original ever draws a sprite at a non-integer or scaled
   size.~~ Not in the reference renderer: perspective (scaling) exists only
   in the Glide/Direct3D drivers (`render/composition.md` §1); §A3 keeps
   integer positions and no scale.
