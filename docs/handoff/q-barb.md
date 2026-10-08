# q-barb: Barbarian skills in `play` (`claude/q-barb`)

Nothing here is verified against 1.14d (rule 10); fills are `d2rs-own, unverified`, spec gaps PROVISIONAL (REC-152, `docs/HANDOFF.md` §7). Synthetic fixtures only. Sound not wired. Reuses q-amazon's passive refresh (`bodies/passive.rs`, `skills/world.rs` `add_skill_level`) and weapon seams (`app/weapons.rs`), the cast path (`q-skills-cast`) and states (`q-states-auras`).

## Why Bash did nothing (the unfinished session's failure)

Three faults, all in the test rig, none in the product (the path already worked):

1. The spawned monster lacked unit flag bit 1 (`FLAG_TARGETABLE`, `missiles::unit_flag::BIT1`): `target_checks` (`use.md` §5.3 step 2) cleared the target, so `bash` found none and returned 0, and no mana was taken.
2. The skill row had `manashift` 0: the cost was 2/256 of a mana point, lost to regeneration. Real rows use 8.
3. The synthetic player is level 0: `hit_chance` gives a level-0 attacker 5% (`hit.md` §3). The rig sets level 20.

## Links connected

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Bash (`srvst 32` / `srvdo 2`) on a monster | worked; the rig hid it | test `bash_on_a_monster_costs_mana_and_hurts_it` |
| 2 | Shout, Battle Orders (`srvdo 68`: missile ring + state on the caster, `shout_state`) | worked given a states table | test `shout_and_battle_orders_put_their_state_on_the_barbarian` (the rig now installs a 200-row states table) |
| 3 | Double Swing (`srvdo 70`), second swing on the next monster | `line_clear` "blocked" emptied the target scan (filter 0x200); `frame_event_index` always 0; `body_path_op` no-op | `skill_rest.rs`: `line_clear` clear, `frame_event_index` / `set_frame_event_index` on `event_arg`; `UseView::path_op` (d2-sim, 5 lines) also tells the host of a `TargetUnit`, the preview keeps it as the mode target (q-paladin's path provider handled it but never the kept target); test `double_swing_hits_two_monsters_in_two_swings` |
| 4 | Masteries / passives learned through C→S 0x3B | q-amazon's refresh; no server-level test (its Left #7) | test `a_learned_mastery_puts_its_passive_stats_in_its_state` |

Tests: `crates/d2-client/tests/app_barbarian.rs` (4), rig in `app_barbarian/rig.rs`.

## PROVISIONAL / d2rs-own (REC-152)

- `line_clear` is always clear in the preview: skills with `lineofsight` start through walls.
- Frame event index = `event_arg`; monster bodies that set it now see it.
- `PathOp::TargetUnit` also stored as the mode target; the other path ops are q-paladin's provider's.
- Warcry state ids and lengths in the tests are made up; Howl is shaped like Shout.

## Left

1. **Leap and Whirlwind do nothing** in play (probed again after merging q-paladin's path provider: no mana, no move; the start returns 0 before any path op). Next: find which start check refuses (`leap_room_test` / `leap_clamp`, `has_path`, `free_point`) with a probe on the start bodies.
2. Howl (scares monsters), Taunt, Find Item, Grim Ward: bodies exist, not exercised. The client side of the warcry state (S→C state message, status icon) is q-states-auras; not checked here for the caster.
3. Weapon requirement of a skill (`itypea1`) is not checked (q-amazon Left #2): Bash with a bow equipped is allowed.
4. Masteries by item type (`passiveitype` > 0) set their stat on layer 0 (q-amazon's choice); saved Barbarians' passives are not turned on at load.

## The user's local check (Windows, PowerShell, 1.14d files)

```powershell
git fetch origin claude/q-barb; git checkout claude/q-barb
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
cargo run -p d2-client --release -- play --new barbarian Test
```

Walk out of the camp. Put a point into Bash (needs experience for a skill point), select it on the right button, right-click a monster: it swings, mana drops, the monster loses life. Shout / Battle Orders (clvl 6 / 30; cheat the level with a save): right-click the ground, mana drops and the state appears (`RUST_LOG=d2_server=debug`). Leap and Whirlwind: expected to do nothing yet. Headless: `cargo nextest run -p d2-client --test app_barbarian`.
