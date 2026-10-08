# q-doors: doors and object polish

Branch `claude/q-doors`. Touches `d2-sim` only (`wiring/action/objects.rs`
and its tests). Nothing in `object_click` / pending-interaction.

## Links connected

| Link | State |
|---|---|
| Door footprint (open = walkable, closed = blocks) | Already connected: C→S 0x13 → `View::object_message` → operate 8 → `MechWorld::free_footprint` / `stamp_footprint` on the DRLG collision grid. New end-to-end test `door_operate_frees_then_restamps_the_footprint` (mask 0x806, 500 ms debounce) pins it. No code change needed. |
| Shrine hover (name above the shrine) | Connected. `ShrineWorld::create_hover` / `hover_expiry` / `free_hover` on `ObjectView` keep the overhead record (`UnitRecord::hover` + `session.overheads`); `object_update` sends it to the receiver on flag 0x100 (0x26 form 5, `intents-events.md` §7.9 r3); event 6 expires it. Test `shrine_hover_is_kept_sent_and_expires`. The text is the decimal string id (3683 + shrine id), as the client's `OverheadRecord::from_string_id` already reads it. |

## PROVISIONAL / not done

- **Shrine timed-state effects** (armor, combat, resist, mana recharge,
  skill, stamina shrines) still do nothing: `ShrineWorld::apply_state` /
  `set_list_stat` stay defaults. Stat-only shrines (life/mana/stamina
  refill, experience) already run through `stat` / `add_base_stat`. The
  state helper `skills/use_/bodies/helpers.rs::apply_state` needs
  `BodyWorld`, implemented only by `UseView` (`X: UseRest`); `ObjectView`
  is generic over `X: Pending`. Next step: lend a `UseView` to the object
  operate (or add a `UseRest`-bound variant of `with_objects`) and map
  `StateRequest` to `helpers::StateRequest`.
- **Mouse-over name label / outline for objects** (client): not touched,
  to stay out of `q-objects-merge`'s object-click path. `bridge/hover.rs`
  picks monsters only; extending `hover::pick` to objects belongs after
  that merge.
- Door `Locked` (mode 6) key test: unchanged.

## Local check

`cargo test -p d2-sim wiring::action::tests::objects` : 3 new/changed
tests pass. In `play`, walk to a hover-able shrine and click it: its text
bubble (the shrine's string id) appears above it for ~8 s.
