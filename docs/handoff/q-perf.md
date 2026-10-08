# Handoff: q-perf (performance pass on the play preview)

Branch `claude/q-perf`. Output is unchanged; no spec behavior touched.

## What was found (static trace of `world_view_frame`, no profiler run)

Per frame, per UI draw, the old path rebuilt strings and re-validated paths:

| Where | Cost per draw per frame | Fix |
|---|---|---|
| `panel_art::PanelArtLoader::ensure` -> `image_set` | `format!` + `CanonicalPath::new` + `FrameSetKey::new` (3-4 allocations, two validations) | `PanelArtLoader::set_of`: key memoized by file id, valid while `files` names the id the same; built by `PanelArtLoader::new` |
| `ui_bind::original_text_font` (called by `ensure`, `text_font`, `text_rules`) | two `CanonicalPath` validations and a key build | memoized per font id (14 fonts), thread-local; `build_text_font` is the old body |
| `present.rs` `last_tags` / `last_ui` | fresh `Vec` per frame | `clear` + `extend`, so the buffers are reused |

## Tests

- `panel_art::tests::the_memoized_set_key_equals_image_set`
- `ui_bind::tests::the_memoized_font_equals_the_built_one`

Both assert the memoized value equals the freshly built one (output identical).

## Not done

- No measured frame times: the cloud container has no GPU/window, and the toolchain build ate the session's time. The per-system timings below are the user's local check.
- Not touched: `build_frame`, `feed.prepare`, atlas `pack_cycle`, the per-frame `Arc::new(palette.clone())` (768 bytes), `text_sprites` layout allocations. Profile before changing them.

## The user's local check

```
cargo run --release -p d2-client --features bevy/trace_tracy -- play --new sorceress Test --seed 1 --frames 600   # or any Bevy frame-time diagnostic
cargo nextest run -p d2-client
```

Expect the same first frame as before (pixel-identical) and fewer allocations per frame in the UI path.
