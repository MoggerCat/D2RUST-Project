# q-shrines-labels: shrine states and object labels

Stitching session 2026-10-08. Nothing here is verified against 1.14d
(rule 10); preview fills are marked `d2rs-own, unverified`. PROVISIONAL
note: REC-239 (docs/HANDOFF.md §7).

## Links connected

| # | Link | Was | Now |
|---|---|---|---|
| 1 | Shrine operate → `effect` → `ShrineWorld::apply_state` | default `None`: no shrine gave a state, no stat list | `wiring/action/objects.rs`: state list on the player, expiry frame + duration, the row's stat/value, `set_list_stat` for the extra stats, state on, type-12 timer. A list of the same state is replaced |
| 2 | Expiry | none | the type-12 timer frees the list on frame start + duration (stat leaves with it) |
| 3 | Hover pick → label | the pick only highlighted | `world_view/object_label.rs`: `ObjectLabels::draw` gives a centred text draw (name of the object's row) above the hovered object; `present.rs` pushes it into the UI frame's draws (+ glyph residency); `play.rs` hands in the `objects.txt` names |

Tests: `a_shrine_state_carries_its_stats_and_ends_on_its_tick` (d2-sim
`wiring/action/tests/objects.rs`; armor shrine state 129, stat 25 and to-hit
on the list, ends on frame start + 5) and the two `object_label` tests
(hover pick → draw; name fallback; monsters / no hover / blank row draw
nothing).

## PROVISIONAL / left

- Helper refusals and the skill / stamina remove callbacks are not wired
  (REC-239); those states end as plain states.
- The label's font, colour, position and string source are made up.
  The string lookup is the UI's `StringLookup` (`NoStrings` in play until
  q-strings), so today the label shows the raw `Name` (e.g. `Chest`-like
  internal names).
- The shrine overhead text (0x26 type 5, decimal string id) reaches the
  UI overhead store but nothing draws it.
- No app-level (Bevy) test of the label: only the draw function is tested.

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-shrines-labels; git checkout claude/q-shrines-labels
cargo run -p d2-client --release -- play --new sorceress Test
```

1. Move the mouse over a chest, barrel or shrine: a text with the object's
   `objects.txt` name should appear above it and go when the mouse leaves.
2. Click a shrine (Blood Moor / outside town): the player should get its
   effect (e.g. refill, or a timed state for the row's duration). The
   state's stats are in the server's stat list; the HUD does not show
   state icons yet.
