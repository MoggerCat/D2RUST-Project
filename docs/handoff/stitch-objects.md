# Stitch: world objects (`claude/stitch-objects`)

> Stitching session, 2026-10-07. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Nothing here is verified against 1.14d (rule 10); every
> preview fill is marked `// d2rs-own, unverified`.

## The path, and where it was cut

Click → `world_clicks` → `Bridge::world_click_at` → C→S → `SimGame::handle`
→ `world::handle` (0x13, type 2) → `ActionSim::object_message` →
`operate_object` → object module → S→C 0x0E → client model → unit draw.

| # | Link | Was | Now |
|---|---|---|---|
| 1 | Client hover: `ModelClick::hover` is `None` (`bridge/click.rs`), so a click on an object is a ground click and never an interact | cut | `bridge/object_hover.rs` (`ModelClick::object_at`, `Bridge::object_under`): the object nearest the mouse's world sub-tile within 3 sub-tiles. `world_view/object_click.rs` keeps it as the pending interact target; the walk itself is the normal ground click |
| 2 | Pending interact: `ClickWorld::pending()` is `None`, so nothing sends 0x13 after the walk (`controls/click.rs` `interact`: walk, then pend code 2) | cut | `ObjectClick::tick`: once the predicted player is within 7 sub-tiles, `Bridge::object_interact` sends 0x13, again every 8 frames (max 30) until the object changes mode |
| 3 | Server reach: `Pending::object_in_range` defaults to false and `object_approach` to operate, and `LocalSeams` overrides neither, so the operate entry returned before any dispatch | cut | New `Pending::object_preview_range` (d2-sim `wiring/action/pending.rs`, default `None`); `LocalSeams` answers 5 sub-tiles. 0x13 beyond 50 → 1, beyond 5 → 0 (client retries), within → the operate entry. Path positions come from the path provider |
| 4 | Server player position: the point parser (`SimGame::point_state`) needs a staged `UnitFacts` position; the play host never staged one, so **every C→S walk was silently dropped and the server player never moved** (the walk the user saw was the client prediction only) | cut | The join stages `UnitFacts`; `SimGame::tick` moves staged positions to the path's (`WorldHost::unit_positions`, implemented by `ActionWorld`/`WiredWorld`) |
| 5 | S→C 0x0E → model → draw | wired | unchanged (checked by the new test: the chest's model mode changes) |

## Test

`crates/d2-client/tests/app_play_objects.rs` (synthetic fixtures; new
`single_player::start_with_chests` puts a synthetic chest, objects row 1,
operate 4 / init 3, in the town room). The click on the chest sets the
target, 0x13 is sent, the server operates, and the chest's mode changes in
the client model. It fails without links 1–3.

## Seams and what is left

- The synthetic `charstats` give the server player no walk speed, so the
  server walk to a chest is not exercised by the test (the chest is put in
  reach). Link 4 is why the live walk should now move the server player;
  that is the first thing to check locally.
- Drops: the chest drop needs the drop tables (`drops: None` in synthetic).
  The items reach the model only through the item messages; their art is
  stitch-items. Seam: S→C 0x15/0x9C for the dropped units reach the
  client dispatch; nothing item-specific was added here.
- Doors: operate and mode go through the same path; the footprint change
  (`apply_object_footprint`) and whether the server path provider reads it
  after an open/close are not checked here (no door in the synthetic game).
- Wells, shrines, urns and corpses use the same operate entry; nothing
  object-specific beyond the above. Barrel "break" art is the object
  composite's mode art (`unit_assets` objects/objmode), unverified.
- Hover has no cursor feedback (no highlight) and ignores `flag_4`
  (selectable is not set for every object in the model).
- Pre-existing, unrelated: `d2-server` `world_data_tables` tests fail on
  `expansionstring.tbl` in the synthetic install (string source switch,
  native-n4's area).

## The user's local check (Windows, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/stitch-objects; git checkout claude/stitch-objects
cargo run -p d2-client --release -- play --new sorceress Test
```

1. Left-click the ground a few tiles away: the player walks. This should
   now be the server's walk too (link 4); if it snaps back or never moves,
   copy the log into `docs/HANDOFF.md`.
2. Walk to the Blood Moor or the Den of Evil (the ground outside town may
   still be black, play-fix2). Left-click a chest, barrel or urn: the
   player walks to it, then the object plays its open / break mode
   (the object's mode art; no items on screen until stitch-items).
3. Click the waypoint in town: the server operates it (the menu is
   stitch-npc's).
