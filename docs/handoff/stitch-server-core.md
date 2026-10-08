# Stitch: server core for the play preview (`claude/stitch-server-core`)

> Stitching session, 2026-10-07/08. Read only `specs/`, `docs/`,
> `crates/`, `tools/`. Base: `daf861b8`. Synthetic fixtures only.
> Nothing here is verified against 1.14d (rule 10); preview fills are
> marked `// d2rs-own, unverified` (decision D1), spec gaps
> `PROVISIONAL` (M22). Sound not wired. Follows
> `stitch-combat.md` §3 (items 1–6).

## 1. Fixes

| # | Fix | Where | Test (synthetic, fails before) |
|---|---|---|---|
| 1 | **UnitFacts from the sim.** `SimGame::set_facts_source(world_sim_facts)`: a unit without staged facts gets its act from the unit record and its position from the path record (`path-placement.md` §2.1). The point parse (walks 0x01–0x05, 0x0C) and the unit-target parse (0x02, 0x04, 0x06, 0x0D, …) now see the player and every unit (`intents-events.md` §2.4 r3–r4). Before: `point_state` → `None` → `Invalid`, `unit_target` → `Missing` → `Refused`, so the 2026-10-07 walk was the client prediction only. | `d2-server/src/adapters/sim.rs` (`FactsSource`, `world_sim_facts`, `SimGame::facts`); `single_player.rs` `build_with` | `d2-client/tests/app_server_core.rs` `the_server_side_player_walks_on_a_client_walk` |
| 1b | **Host sync.** `SimGame::set_host_sync(sync_seams)` runs before each handled intent and at the start of each tick; the play host copies the players and monsters (type, allied flag, path position) into `LocalSeams::sides` for the seams that get no `Game`. | `sim.rs`; `single_player.rs` `sync_seams` | (used by 3) |
| 2 | **Server skills.** The server player gets skill 0 + its `charstats` Skill 1–10 at the join (`client/msg-skills.md` §2 r8 mirrored: server init `0x005348C0` → `0x00647EE0`), Attack in both hands; the world's skill slot is `WiredSkills` (`UseRest` / `LearnRest` on `LocalSeams`, `app/skill_rest.rs`); `has_skill` / `set_mouse_skills` / `add_skill_level` of the join have providers; S→C 0x94 at the join after 0x59 / 0xAA / 0x76 (`intents-events.md` §8.2 r3.1). play-fix2 had no 0x94 work on its branch (checked at start). | `d2-server/src/adapters/{character.rs,session.rs}`; `d2-sim/src/wiring/action/pending.rs` (`select_hand_skill`, `assign_skill_level`, default false); `d2-client/src/app/{skill_rest.rs,single_player.rs}` | `tests/app_server_skills.rs` (2 tests) |
| 3a | **Combat seams** on `LocalSeams` (all d2rs-own): `may_attack` (player side = players + allied monsters vs the other monsters, never itself), `alignment` (player side 2, else 0), `target_nodes` (one slot per player, ≤ 8: evil monsters' AI now finds the player, `ai.md` §5.2 step 5), `melee_range` 2 and `in_melee_range` (larger axis distance ≤ reach + extra + 1, no line test). | `single_player.rs` | `app_server_core.rs` `a_left_skill_on_a_monster_next_to_the_player_starts_the_attack` |
| 3b | **S→C 0x4C / 0x4D / 0x0C builders.** Monster skill modes send 0x4C / 0x4D (was a logged `ModeMessage(SkillMessage)` error); player skill modes too (`path/walk.rs` `update_messages`); unit flag 0x8000 sends 0x0C (§7.3 r2 step 7, not provisional). Missiles have no own S→C message (§7.6 r1): the client creates them from 0x4C / 0x4D. Client: codes 0x15 / 0x16 set the unit's mode from the skill's `anim` / `monanim`. | `d2-sim/src/wiring/action/unit_update/skill_message.rs` (new), `unit_update.rs`, `path/walk.rs`; `d2-client/src/bridge/modes.rs` | `unit_update/tests.rs` (6 tests), `modes_tests.rs` `skill_codes_set_the_skill_animation_mode`; `e2e_night_world.rs` now expects the two 0x4D |
| 3c | **Vitals sync on** in the preview build (`hooks.enable_vitals_sync()`): life / mana / stamina / position at the end of each tick (`combat/vitals.md` §5.1). | `single_player.rs` | existing vitals-sync tests |

## 2. PROVISIONAL / d2rs-own points

- 0x4C / 0x4D level byte; which player modes send them, own client included (`intents-events.md` §7.4 r3, `pathing.md` §10 r2): **REC-94** (new, HANDOFF §7 P0).
- Client mode from codes 0x15 / 0x16 (`client/model.md` OQ1): REC-51.
- 0x94 contents on the new-character path (`msg-skills.md` §3 r1; `d2s-load.md` OQ4): REC-02. On live data a new Sorceress's right hand stays 0 (her `StartSkill` 36 is not in the r8 list), two 0x23 not three.
- `skill_rest.rs`: `find_entry` (first entry, any owner), `use_state` (usable when it has a level; no mana / cooldown), `srvst` / `srvdo` logged and 0 (`use.md` OQ10), `LearnRest::is_class_skill` false (0x3B refused), `run_to` and timer-path `start_mode` only logged, passive states logged.
- Hostility, alignment, target nodes, melee reach (§1 row 3a): REC-94 notes what to observe; `0x00554200` / `0x00622870` are not specified.

## 3. What is left

1. `Pending::reaction` stays a no-op: no get-hit mode and nothing sets unit flag 0x8000, so 0x0C never fires in the preview. Life is taken by the damage code; the kill and death mode run in `wiring/action/reaction.rs`.
2. `run_to` (out of melee reach) only logs: a click on a far monster does not walk to it on the server (the client's stitch-combat "always in range" sends 0x06 at any distance). Needs the path spec's run-to.
3. 0x15 resync is still recorded, not sent (layout, `sim.rs` `queue_resync`).
4. Synthetic data cannot show a hit: no animdata (`Anim(NoRecord)`), no `skills` / `charstats` / `monstats` rows, no `isSpawn`. The tests use test-local table copies; the kill is the local check below.
5. Monster AI uses `good_target_search` = none for good monsters; town NPCs stay out (players in town are skipped by §5.2 step 5).
6. 0x3B skill points (stat seam), server passive states; the client draws no missiles yet.
7. `stitch-combat.md` rows 5–9, 12–14 are superseded by this note.
8. **Merging stitch-objects (`90febd75`).** Its server half fits with this branch: it stages the player's facts at the join and moves them along the path each tick. Staged facts win over the live source here, and the live source answers every other unit.
   - Two files conflict, `single_player.rs` and `world_view/present.rs`. Resolution: keep both `Pending` blocks, and call `object_click::world_clicks_objects` into `let outs = …`.
   - After that merge, `app_play_objects.rs` needs `pick: false` in its `ClickView`.
   - Even resolved, `app_play_npc` `clicking_the_waypoint_walks_there_and_interacts` fails. stitch-objects' object click and stitch-npc2's pending interaction both send C→S 0x13 for the waypoint, so the interaction is left pending. One of the two client systems must own object clicks. That choice is outside this server branch, so stitch-objects is not merged here.

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/stitch-server-core
git checkout claude/stitch-server-core
cargo run -p d2-client --release -- play --new barbarian Test
```

What to see:

1. **Walk out of town.** Left-click the ground: the player walks as before, but now the server moves too: on stopping there is no snap back to the start (the 0x15 / vitals position matches the drawn one). Walk east through the camp gate.
2. **Rooms stream.** Past the gate the Blood Moor ground appears room by room as you walk (S→C 0x07), and its monsters arrive (Fallen, Quill Rats, Zombies). If the ground stays black, copy any `warn` line and the last `frame N:` line into `docs/HANDOFF.md`.
3. **Monsters engage.** A Fallen that sees you walks toward you (glides, stitch-combat motion) and attacks.
4. **Attack a Fallen and kill it.** Stand next to it and left-click its body: the Barbarian plays the attack animation, the Fallen its get-hit / death animation after enough hits, and its corpse stays. Clicking a far monster does nothing on the server yet (§3 item 2): walk up to it first.
5. Life globe: drops when the Fallen hits you (vitals sync, 0x95).

Copy the outcome (each of 1–5: yes / no, plus any `warn` / panic line) into
`docs/HANDOFF.md` under a local-run entry. Headless:
`cargo nextest run -p d2-client --test app_server_core --test app_server_skills`.
