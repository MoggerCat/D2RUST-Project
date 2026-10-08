# q-objects-merge: object clicks on one path

Merged `claude/stitch-objects` (90febd75) into the stitch-npc line.

## Links connected
- Object clicks now use stitch-npc's path only: `bridge::hover::pick` (types 1, 2, 4)
  names the object, `ClickOut::Pend` → `world_view/interact.rs` sends C→S 0x13 once
  when the predicted walk ends.
- Removed: `ObjectClick` (resend-every-8-frames loop), `world_clicks_objects`,
  `bridge/object_hover.rs`. `world_view/object_click.rs` keeps only `INTERACT_RANGE`.
- Kept exactly: `Pending::object_preview_range`, the UnitFacts staging, `SimGame::tick`
  path following (d2-server / d2-sim parts).
- `tests/app_play_objects.rs` finds its click point with `hover::feet` (as the npc test
  does) and asserts `interact.pending` targets the object; intent unchanged
  (click → walk → 0x13 → server operates → S→C mode change).

## PROVISIONAL
- 0x13 is sent once at walk end. If the server still counts the player out of range
  (`object_preview_range` = 5 sub-tiles) the click is lost; the old resend loop covered
  that. The e2e test passes without it.

## Local check
`cargo run -p d2-client -- play ...` (see docs/local/2026-10-07/PLAYABLE.md): click the
town waypoint or a chest; the player walks up and the object opens/operates once.
