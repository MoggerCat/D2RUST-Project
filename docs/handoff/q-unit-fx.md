# q-unit-fx: unit state tint, 0x26 system lines, overhead bubbles

> Stitching session, 2026-10-08, branch `claude/q-unit-fx`. Nothing here is
> verified against 1.14d (rule 10). Preview fills are marked
> `d2rs-own, unverified`; the spec gap is REC-245 (`docs/HANDOFF.md` §7).

## Links connected

| # | Link | Before | Now |
|---|---|---|---|
| 1 | State `colorshift` / `colorpri` → unit palette shift | `PreviewLook::look` always answered `remap: None`; the `states` table's colour columns were read nowhere | New `world_view/state_tint.rs` (`StateTints`, from the `states` table by `app/missile_art.rs::state_tints`, passed to the preview by `app/play.rs::add_preview_tinted`). `PreviewLook::look` gives the unit's `P` = PL2 remap map `colorshift − 1` of the highest-`colorpri` state the unit has; `LitRules` puts it in the unit's shade chain |
| 2 | S→C 0x26 type 4 → message list | Rules, model and draw were already there (`ui/game_messages.rs`) | Test only: raw 0x26 bytes through the bridge draw at the top left (`tests/app_unit_fx.rs`) |
| 3 | Overhead text records → bubbles | The records (`msg_ui_more.rs`) and the rules (`ui/messages/overhead.rs`) existed, but nothing drew a bubble | New `ui/overhead_ui.rs` (`OverheadUi`, open for good, id 0x112): units with a record get a framed bubble (point, text, box, placement, draw per `messages.md` §5 r3–r6); a record is freed on the frame after its counter passes the end (end = counter + 8·len + 125). `OriginalUi::set_overhead` and 0x76 mirror into it, so 0x26 type 5 and 0x27 kind 3 both show |

## PROVISIONAL (REC-245)

- The body of the state colour call `0x004D97F0` (see HANDOFF).
- The overhead counter steps per client frame; the unit point is the model cell with the local player's model-cell camera.
- No server code emits a type 4 system line; only the receiving side is wired.

## Tests

- `world_view/state_tint.rs`: a tinted state changes the unit's shade chain (the remap map is in it, and gone when the state is off); priority and tie rules.
- `ui/overhead_ui.rs`: a player's bubble draws for its whole life and is freed on the spec's frame; a monster's record shows its string; 0x76 clears.
- `tests/app_unit_fx.rs`: a 0x26 type 4 line through the bridge shows; a type 5 for an absent unit shows nothing.

## What is left

- Colour of a tinted state in the HUD / item colours is not touched.
- Overhead bubbles follow the model cell, not the predicted walk.
- Server sources of system lines.

## Local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-unit-fx
git checkout claude/q-unit-fx
cargo run -p d2-client --release -- play --new sorceress Test
```

1. Talk to Akara/Warriv in town: a bubble with the NPC's line should appear above them (if the server sends a 0x27 overhead number) and vanish after about 6 s.
2. Cast Frozen Armor / any buff whose `states.txt` row has a `colorshift`: the sorceress's palette should shift while it lasts and return when it ends. Note which states look wrong in `docs/HANDOFF.md`.
