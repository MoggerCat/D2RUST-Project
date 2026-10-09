# q-fix-real-next (overnight fix session, 2026-10-09)

Docs-only session; no code changed, no REC ids used (block REC-470..479 unspent).

| Row | Result |
|---|---|
| q-fix-real-controls-default-order | Skipped: needs a spec decision (slot order of command 1). Listed in `pc1-data.md` Step 4 item 11. |
| q-fix-real-stash-repro | Superseded, not fixed: `crates/d2-client/tests/smoke_town.rs` (synthetic `Rig::with_stash`) was deleted by q-fixture-migrate Wave 1 (8adab19), so the failing test no longer exists on staging. Its likely cause, the click distance measured from the model cell (`q-fix-seam-click-distance`), is done. `play_smoke.rs` has no stash case; a real-install replacement is q-fixture-migrate's. |

Open `q-fix-real-*` rows left: none for the cloud.
Not run: the real-install fetch and the code gate, since nothing in `crates/` changed.
