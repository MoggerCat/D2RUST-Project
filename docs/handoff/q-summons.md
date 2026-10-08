# q-summons: summoned pets exist, are listed, follow and fight (`claude/q-summons`)

Nothing here is verified against 1.14d (rule 10); fills are `d2rs-own, unverified`, spec gaps PROVISIONAL (REC-123). Synthetic fixtures only. Sound not wired.

## Links traced and connected

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Skill body (`srvdo` 119 druid summon, golem, raise skeleton) → `BodyWorld::create_monster` | `Pending::create_monster` default `None`: every summon returned 0, no unit | `UseView::create_monster` falls back to the allocator (`interaction/summon.rs` `alloc_monster`), on the aimed point |
| 2 | `BodyEffect::PetAdd` → pet lists | `Pending::body_effect` default: nothing | `UseView::pet_effect` runs `player::pets::add` (group eviction, max trim, oldest first) over `ActionHooks::pet_lists` (new field) |
| 3 | S→C 0x7A add / remove | not sent | `PetView::send` → `Pending::send` to every player; the client already has `bridge/msg/pets.rs` |
| 4 | `BodyTables` (pettype count / group) | never installed (`ActionHooks::bodies` was `None`): the pet type check refused every summon | `BodyTables::pettype_group`, `GameTables::body_tables`, installed in `single_player` on live data |
| 5 | Follow and fight | none | `hireling_drive.rs` `think` is shared: pets of `pet_lists` get the hirelings' stand-in think (walk to owner, attack hostile within sight) |
| 6 | Dead pet leaves the list | none | `View::pet_sweep`, each frame before the think (0x7A remove) |
| 7 | Drawn | monsters arrive by 0xAC and the client already draws them | unchanged |

## Tests (fail before the change)

- `d2-sim` `wiring::interaction::tests::summon` (3): a summon makes a real monster, lists it, sends 0x7A add; a summon past the maximum removes the oldest (0x7A remove); a dead pet leaves the list.
- `d2-server/tests/e2e_night_world.rs` `a_summoned_pet_attacks_a_hostile_monster_beside_it`.

## PROVISIONAL / d2rs-own (REC-123)

Spawn on the aimed point (no spread search); resync `0x00575900` no-op; sweep instead of `0x005751A0`; the hirelings' stand-in think for pets (REC-100).

## Left

1. Real AI for pets: `OwnerData`, `NodeInsert`, `AiRefresh`, `SetLinked` still reach `Pending::body_effect` (nothing); the owner link feeds `ai_owner` for the real Necro/Druid pet AIs.
2. Golem equipment/stats, raise-skeleton corpse removal, Valkyrie: bodies run but nothing was checked on live data for them.
3. Player death clearing pets (`pets.md` §10), unsummon, mana reserve.
4. Client: pet life bar / name colour, the 0x7A list is not shown.
5. Live-data check below: the Sorceress has no summon, use a Necromancer or Druid.

## Your local check (Windows, 1.14d files)

```powershell
git fetch origin claude/q-summons; git checkout claude/q-summons
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
cargo run -p d2-client --release -- play --new necromancer Test
```
Leave town, select Raise Skeleton on the right button (or Raven / Wolf for a Druid), right-click near a corpse / the ground. Expect: a unit appears at the cursor and walks after you; it attacks monsters that come near; casting more than the skill's maximum removes the oldest (it disappears). Report the server log's `0x7A` lines and any `Pet(` wiring error.
