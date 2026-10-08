# q-druid: Druid skills in play (`claude/q-druid`)

> Stitching session, 2026-10-08. Nothing here is verified against 1.14d
> (rule 10); fills are `d2rs-own, unverified`, spec gaps PROVISIONAL
> (REC-155). Synthetic fixtures only. Sound not wired.

## Links traced

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Firestorm (srvdo 117), Tornado (118) from the right button: cast path, mana, `fan` of server missiles | already connected by q-skills-cast / q-missiles-draw | unchanged; `tests/app_druid.rs` pins it (passed before the change) |
| 2 | Werewolf / Werebear (srvdo 116): the state on the player | body ran | unchanged; test `werewolf_turns_the_player_into_the_shape...` |
| 3 | `UseRest::shapeshifted` (free recast while shifted, `use.md` §5.3) | always false | **connected**: `skill_rest::sync_shapes` (from `sync_seams`) |
| 4 | Client draws the shape | player kept its human COF | **connected**: `world_view/disguise.rs` (`unit-composite.md` §1.1), applied in `unit_cof`, `component_codes`, the loader; `UnitLooks::live` reads `states` gfxtype/gfxclass and the `monstats2` mode bits |
| 5 | Summons (Raven 114, vines 115, wolves / bears / spirits 119) | q-summons connected pets, list, follow, fight | unchanged; not re-tested per skill |

## PROVISIONAL (REC-155)

Substitution without the flag-ex bit 3 test; states tested in ascending id;
shifted players found by the `aurastate` of 116 rows.

## Left

- Molten Boulder's trail (srvdo 6 maker): the synthetic world has no
  collision, so a missile dies after one tick there; untestable headless.
- Recast to revert a shape needs `BodyTables` (`state_group`) in the
  synthetic test; live data has them.
- Raven / Oak Sage / vines per skill on live data, werewolf animation
  speed and attack mode, client missile art for the Druid rows.
- Tornado / Firestorm art names (`missile_art.rs`) unchecked.

## The user's local check (Windows, PowerShell, 1.14d files)

```powershell
git fetch origin claude/q-druid; git checkout claude/q-druid
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
cargo run -p d2-client --release -- play --new druid Test
```
Level the Druid cheaply if the preview allows, leave the camp and:
1. Right skill Firestorm / Tornado, right-click the ground: several missiles.
2. Werewolf: the player is drawn as the wolf (monster art, WL when walking);
   casting it again costs no mana. Copy any `unit art:` error to HANDOFF.
3. Raven / Wolf / Oak Sage: pets appear and follow (see q-summons).

Headless: `cargo test -p d2-client --test app_druid` and
`cargo test -p d2-client --lib disguise`.
