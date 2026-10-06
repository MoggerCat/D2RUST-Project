# Client d2rs-own gaps: sound pool, prefetch, text layout

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/client-own-gaps`, based on `claude/tender-meitner-mphas3`
at `4b5b0bf`. Cloud session (repo only, no game files). Task class:
implementation from a clear spec, medium effort (M14). Input:
`docs/handoff/gaps-client-formats.md` §4 (seams with no code).

## 1. State

The d2rs-own rules that §4 of `gaps-client-formats.md` listed as "no
code" now have code and unit tests. Everything here is **ours and
unverified** in the M02 sense only where it touches original behavior,
and it does not: each such point is still a `TODO(spec: …)` hook.

| Rule | Was | Now |
|---|---|---|
| `client/audio.md` §a1-decode-path r1 | no sound pool | `audio::pool::SoundPool` reads through `FileSource` (`ArchiveSet` in the client) and hands the bytes to the decoder unchanged |
| `client/audio.md` §a1-decode-path r3 | no sound pool | one decode per canonical path, `Arc<Sound>` kept as decoded, charged samples × 2 bytes under the `sounds` budget (`assets.md` §A5) |
| `client/assets.md` §a4-residency r3 | prefetch not implemented | `assets::prefetch::PrefetchQueue` + `Pool::offer` |
| `client/ui.md` §a3-text | no `layout_text` | `ui::text::layout_text` with the `TextRules` hook |

Coverage (`python3 tools/coverage.py`), this branch: audio 6 → 8 of 10
units, assets 8 → 9 of 12, ui 9 → 10 of 13. All `unit` tier.

Still uncovered in these four specs, and why:
- audio §a1-decode-path text: exactness check 1 compares decoded samples
  with 1.14d; needs `formats/wav.md` (§B1) and a debugger dump. Not
  claimable by a unit test.
- assets §a4 r1: the stage-1 key list is the render stage's (not owned).
- assets §a6-writes, ui §a1, ui §edge-cases, bridge §1 r2, §3 r4, §5 r1,
  §7 r4, §9 r2, §9 r3: unchanged from `gaps-client-formats.md` §6.
- every §B section: original behavior, not specified yet.

## 2. Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/audio/pool.rs` (new) | `SoundPool` (load, peek, begin_frame, decodes, used, drain_events), `WavDecoder` hook, `SoundPoolError`, `sound_bytes`, `POOL_NAME` | audio §A1 r1, r3; assets §A5 `sounds` |
| `crates/d2-client/src/audio/tests.rs` (`mod pool`) | `wav_bytes_come_from_the_archive_read_unchanged` (§a1 r1), `each_file_is_decoded_once_and_kept_as_decoded` (§a1 r3), `missing_and_undecodable_files_are_errors_naming_the_path`, `sound_pool_evicts_by_last_frame_used` (assets §a5) | |
| `crates/d2-client/src/assets/prefetch.rs` (new) | `PrefetchQueue<K>` (request in order without duplicates, start up to N skipping resident keys, finish); tests `prefetch_changes_stalls_not_what_is_drawn`, `offers_never_evict_or_overrun` (both §a4 r3), `queue_keeps_request_order_without_duplicates` | assets §A4 r3 |
| `crates/d2-client/src/assets/cache.rs` | `Pool::offer` and `Offer { Taken, Resident, NoRoom }` | assets §A4 r3, §A5 |
| `crates/d2-client/src/ui/text.rs` (new) | `layout_text`, `TextRules` hook, `NoTextRules` placeholder, `GlyphLookup`, `GlyphPlacement`, `GlyphDraw`, `TextOpts` (no fields yet), `TextError` | ui §A3 |
| `crates/d2-client/src/ui/tests.rs` (`mod text`) | `glyphs_resolve_code_to_record_to_frame`, `missing_code_is_an_error_not_a_fallback_glyph`, `text_reaches_the_rules_as_utf16_units_unchanged`, `layout_rules_are_unspecified_until_ui_text_md` (all §a3-text) | |

Design choices (ours, within the specs' latitude):
- **Sound pool.** Paths are canonicalized (`assets.md` §A1), so two
  spellings share one entry and one decode. A missing file, a read
  failure and a decode failure are errors naming the path; a failed load
  leaves nothing resident. Eviction is the shared `Pool` (LRU by last
  frame used); a playing voice holds its own `Arc<Sound>`, so eviction
  never cuts a voice.
- **Prefetch.** Keys are queued in request order, once while pending or
  in flight. `Pool::offer` takes a prefetched value only if the key is
  not resident and it fits the budget as is: a prefetch never evicts and
  never overruns. A taken entry counts as last used by the frame before
  the current one, so it is not protected by the current frame; a frame
  that lists it marks it used as usual. The test renders the same frame
  lists with and without prefetch: identical values drawn, fewer stalls.
- **Text.** Glyph lookup is format-level: the record whose `code` field
  equals the unit (not the record index), then its `frame`. No record:
  `MissingGlyph`; two records with one code: `AmbiguousGlyph` (which one
  the original uses is not specified). The rules decide which code units
  are drawn and where (color codes, advance, wrap, alignment are §B3);
  `layout_text` resolves every placement and keeps the rules' order.

## 3. Signature changes

New public items only; no existing signature changed.
- `d2_client::audio::pool` (module), re-exported from `audio`:
  `SoundPool`, `SoundPoolError`, `WavDecoder`, `sound_bytes`.
- `d2_client::assets::prefetch` (module): `PrefetchQueue`.
- `d2_client::assets::cache`: `Pool::offer`, `Offer`.
- `d2_client::ui::text` (module), re-exported from `ui`: `layout_text`,
  `TextRules`, `NoTextRules`, `GlyphLookup`, `GlyphPlacement`,
  `GlyphDraw`, `TextOpts`, `TextError`.

## 4. Seams reached (stopped here)

- `WavDecoder`: nothing implements it. `TODO(spec: formats/wav.md §B1)`;
  the implementation will be `d2-formats::wav` behind this trait.
- A `SoundBank` over the pool needs the sound id → file mapping
  (`audio/sound-table.md`, §B3). Not written.
- `TextRules`: only `NoTextRules`. `TODO(spec: ui/text.md §B3)`.
  `world_view::ui_bind::UiRules::ui_text` (not owned here) can call
  `layout_text` once the rules exist.
- Prefetch wiring (not owned here): the bridge has no "new active room /
  new unit type" report yet, and the Bevy task that runs the queued loads
  and calls `Pool::offer` belongs to `app.rs`.
- No dependency was needed.

## 5. Local checks to queue

None: no rule here reads game files. The §B1 decoded-samples check
(audio exactness check 1) stays queued in `p6-audio.md` until
`formats/wav.md` exists.

## 6. Questions

1. Prefetched entries rank as "last used by the previous frame". Should a
   prefetched entry rank below every used entry instead (always evicted
   first)? Either keeps the image unchanged; it only changes stalls.
2. Should `TextOpts` carry the clip rect now (it is on `TextRequest`), or
   wait for `ui/text.md` to say what the original's text call takes?

## 7. Gate

All pass on this branch:
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p d2-client`
- `cargo run -p depcheck`
- `python3 tools/spec_index.py --check`
- `python3 tools/methods.py check`
- `python3 tools/coverage.py --check`
- `python3 tools/coverage.py --selftest`

Non-source files in the diff: this note only.
