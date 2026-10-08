# q-preview-skill-seams: throw skill and item-granted skills in play

## Links connected
- The preview item-move rest (`PreviewMoveRest`, d2-server `adapters/handlers/world.rs`) answered the §5.8 skill seams with defaults. They now run on the player's list in `ActionHooks::skill_lists` (d2-sim `SkillList`; no second copy): the lists are lent to the rest for one move call (`WiredWorld::moves`, `world/wired.rs`) and put back. Logic: `items/moves/preview_skills.rs` (`mouse_skill`, `select_skill` + S→C 0x23, `has_skill_owned`, `throw_skill_row` from `itypea1` is-a `thro` and `range` 2, saved mouse skills, skill quantity).
- The weapon bookkeeping saw no weapon: inventory +0x1C had no setter in play. `InvState::weapon_hand_fallback` (on in `preview_inv_parts`) reads the right-hand item as the weapon in use.
- `skills/levels.md` §7.1 for stats 97 / 107: after each move call `sync_oskills` gives a skill with a positive item total a native base-0 entry (+ S→C 0x21) and removes it again (hands to Attack) when the item leaves. `SkillList::remove` is new.
- Tests (`crates/test-fixtures/tests/preview_skill_seams.rs`, 2): wearing a throwing weapon puts Throw on the left and sends 0x23; a worn `item_singleskill` item adds the entry (0x21), it can be selected, and it goes when the item is taken off. Both failed before.

## PROVISIONAL: REC-266 (see `docs/HANDOFF.md` §7).

## What's left
`use_state` stays "usable"; 0x22 quantity send; the callback runs per move call, not per stat change, so already-worn items from a loaded save are synced at the first move; stat 98 / 151 / 204 handlers.

## The user's local check
`cargo run -p d2-client --release -- play --new barbarian Test`, press **I** and wear an item with a +skill (e.g. a magic item with "+1 to <skill>" of your class): the skill appears in the skill list (S menu / right-click) and can be put on a mouse slot; take the item off and it is gone. Wear a throwing weapon (javelin / throwing potion, if you have one): the left skill becomes Throw.
