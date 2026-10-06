# frame-cycle — HANDOFF §2 step 7l (cloud part) + RN3

Branch `claude/frame-cycle`. Scope: `d2-client` world view, one doc line in
`d2-formats`. No `d2-sim` / `d2-server` change.

## What changed

- **World view runs the frame cycle** (`render/composition.md` §3):
  - `WorldViewState::cycle: FrameCycle` (800 × 600, starts all 0) holds the
    index framebuffer between frames.
  - `ViewFeed::blank_screen(world)` (new hook, `TODO(spec: the S→C owner
    spec of the player's current level)`) gives BlankScreen of the player's
    level; `world_view::blank_screen(&Levels)` reads the row (`+0x218`,
    non-zero = clear). `NoFeed` answers `true` (all 137 live rows are 1).
  - CPU: `world_view::compose_cycle_cpu(cycle, blank, frame, assets)` =
    `cycle.compose(..)` then `to_rgba` through the frame palette.
  - GPU (headless): `GpuAtlas::pack_cycle` = `pack(..)?.with_frame(
    cycle.pixels(), plan)` with `plan = cycle.plan(blank)`;
    `GpuAtlas::compose_cycle` composes, reads the indices back and
    `cycle.commit(plan, indices)`.
  - GPU (in-app node): `Gpu::encode_rgba` now also returns the index
    buffer; the node copies it to a staging buffer, `map_indices`
    (`Render`, `RenderSystems::Cleanup`, after submission, as Bevy's own
    `gpu_readback`) maps it into the shared `NodeIndices` slot. The main
    world commits job `seq`'s indices to the cycle before building the
    next frame; until they arrive it waits (no frame built that Bevy
    frame, like a Bevy frame with no new tick). The presented image still
    never leaves the GPU.
- **Palette**: `ViewAssets::from_pl2(pl2)` builds the presented palette
  with `scene::present_palette` (§4). `ViewAssets::palette`'s old
  `TODO(spec: render/shading.md)` "one palette per frame until per-region
  palettes are specified" is answered by §4 (one palette) and removed.
  `play` still uses the all-black placeholder: the model states no level,
  so no act `pal.pl2` (TODO on `unspecified_palette`).
- `scene::compose` (single frame from index 0) is unchanged; its
  `composition.md` §6 reading stands (a single-frame case without a
  recorded previous frame starts from all 0).
- **RN3**: `d2_formats::palette::Pl2::alpha_blend` doc now says
  `[level][dest][src]` (row = destination, column = source,
  `composition.md` §5, `formats/palette.md` OQ2).

## Tests (synthetic, CPU path; GPU via shader emulation)

- `world_view::tests::cpu_frames_run_the_frame_cycle`: first frame from 0
  equals the single-frame image; BlankScreen 1 over a frame of 5 keeps
  rows 553–599; BlankScreen 0 keeps everything; the next frame starts from
  the last; post-draw clear blanks the frame and steps the counter; a
  wrong-size cycle is refused unchanged.
- `gpu_cycle_packing_emulates_to_the_cpu_cycle`: `pack_cycle` + emulate +
  commit equals the CPU cycle for (BlankScreen 1/0, counter 0/1).
- `view_assets_present_the_pl2_palette`; `feed::tests::
  blank_screen_from_the_levels_row`.
- `bevy_frame_presents_the_cpu_image` now also checks the presented frame
  is the cycle's framebuffer on two frames.
- `tests/app_frame_loop.rs::gpu_node_composes_the_frame_into_the_presented_texture`
  (llvmpipe when present) runs the in-app readback path.

## Open questions / for the coordinator

- FC1 (spec): which level/act the client knows the player is in — no S→C
  owner spec yet. Until then BlankScreen comes from the feed (`NoFeed`:
  1) and `play` has no act palette. Both are `TODO(spec: …)` at their
  sites.
- FC2: `formats/palette.md` Layout table still reads "[level][source
  index]"; OQ2 says the reverse. The same row = destination reading
  presumably applies to the additive / multiplicative tables (OQ2 names
  all three); their `d2-formats` docs do not state an order. Spec edit is
  a local spec task.
- FC3: the in-app GPU path builds the next frame only after the previous
  frame's indices are read back; if Bevy's render world lags by more than
  a tick interval, intermediate ticks are not drawn (as before: only the
  latest tick of a Bevy frame is drawn). The original draws every tick
  (camera §9); not a regression, recorded.
- The capture `SceneSource` (CP1) is not part of this task.

## Local run queue

None new: no game-file check is introduced. (C-entries of
`composition.md`'s capture case `composition-0001` stay as queued.)
