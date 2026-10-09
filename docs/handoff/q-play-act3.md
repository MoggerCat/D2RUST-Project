# q-play-act3: Act III played end to end (d2rs headless)

Session `q-play-act3`, branch `claude/q-play-act3`, REC ids 780–789
(used: 780, 781). Scope: Kurast Docks through the portal to Act IV.

## How it is played

- `crates/d2-client/tests/app_play_act3.rs` (ignored, real data): the
  Act III start save is made by `d2s-tool new` (level 25 sorceress, Acts
  I–II done, waypoints to Kurast Docks, town byte act 2), loaded as
  `play --save`, joined through `add_live_client` (the play client
  headless). Talks are clicks on the NPC through the client's own click
  path; far states come from pokes (`warp`, `pos`, `stat`, `spawn`).
  Every call the host had no provider for (`AppRest::log`) fails a test.
- `python3 tools/playthrough/playthrough.py traces/playthrough/act3.play`
  (q-tool-state-diff): 12/14 milestones on the install (2026-10-09).
- `traces/checks/milestone-act3-entry.check` against 1.14d
  (`scenario_diff.py`, 1.14d side recorded under Wine).

## Fixed

| Blocker | Fix |
|---|---|
| NPC chat start: the NPC's path stop and AI hold (`npc.md` §2 r2) only logged | `UnitHooks::stop_path_now` / `set_ai_param0` on the action hooks; the desk calls them first |
| Golden Bird: no boss ever carried the figurine (`quests-act3.md` §6.2 hook unwired) | `WorldPending::boss_quest_hook` → `QuestEvent::BossCreated` → `choose_bird_boss` (REC-780) |
| Travincal council never registered (`0x00545B50` → §7.5) | `InitHost::quest_preset_boss` → `QuestEvent::PresetBoss` → `council_preset` below level 108 |
| Durance of Hate 1 open from the start (`quests.md` §8.2 `0x005BBFA0`) | `Pending::set_durance_open`, published per tick; `warp_quest_gate` closes level 100 except from 101 |
| Alkor's and Ormus' map-AI quest calls (`ai-bodies.md` §9.9) answered false/None | published per tick (`set_act3_npc_answers`); reset / altar activation queued (REC-781) |
| Quest object inits that read their own position got none (the object is not in its room during its init) — Lut Gholein's start Jerhyn never spawned | `QuestWorld::set_init_point`: the quest host answers `unit_position` for the object under init from its init point |
| A quest spawn's unit-seed step was lost (the economy's game seed was not handed to the allocator) | `HostQuests::view` syncs `fields.seed` |

`milestone-act3-entry.check` after these: game seed and every unit field
equal to 1.14d on all 40 frames except the unit seed `s` of the two
quest-spawned NPCs (below).

## Left (in order)

1. **Quest-spawned monsters skip `0x005B2F20`'s monster init.** The quest
   host allocates them raw (`HostQuests::spawn_unit`): no normal mods, so
   their own seed stays `[lo, 666]` (1.14d: stepped). Seen on Jerhyn (201)
   and Hratli (253) in `milestone-act3-entry`. Needs the population
   placement (`placement::place`, monster data in the worldgen world)
   reachable from the quest economy, or a queued post-init.
2. **Ormus' menu does not open** (`app_play_act3`, NPC 255): the client
   sends C→S 0x59 then 0x13 at once (it reads the distance as ≤ 2) and
   the server answers nothing (distance 9..50, `npc.md` §2 r3). Next:
   the server vs model positions printed by the test's `talk`.
3. **`flayer-jungle-altar`**: the Gidbinn altar (object 251) is not seen
   in Flayer Jungle by the harness sweep (R 800); a wider sweep was cut
   short by the disk. The altar comes from a clearing preset (Pygmy
   595–604, `outdoor-act3-act5.md` §3).
4. **`town-start`**: 2 of the 7 town NPC classes in the snapshot at f40
   (1.14d count not recorded yet; `milestone-act3-entry` arrives by warp,
   not by the save's town).
5. Unwired, lower: `will_cubed` (counters only), the Dark Wanderer's walk
   target and minions (`wanderer_target`, `wanderer_minions`),
   `bird_boss_removed` (monster storage `0x005421A0` has no quest hook).
