# q-fix-monster-death — hand-back (2026-10-09, branch `claude/q-fix-monster-death`)

Area: monster death and its clean-up (d2-sim units / combat death path).
Queue rows: `q-fix-p4-death-cleanup`, `q-fix-real-monster-drop-spot`.
REC block 1090–1099 (used: 1090).

## Done

- **Death clean-up `0x005A6520` in full** (`sim/units.md` §4.6 rule
  1.2, new `crates/d2-sim/src/wiring/action/monster_death.rs`), in the
  rule's order: overhead record freed (flag 0x100, update queue),
  pack-leader handover `0x0058F6C0` on the AI store's controls, target
  node leave (+0xD0 := 11), flags &= ~0x800C, stat-list death
  `0x00627540` (+ its remove callbacks), state keep mask `0x00639FB0`
  (`monstaydeath` / `bossstaydeath` / `plrstaydeath`; new
  `StatLists::clear_states_except`), `hide` → flags &= ~0x2, dead-body
  footprint, path direction snap `0x006488A0`, then `0x005738D0` (thinks
  and regeneration cancelled). The play host
  (`d2-client/src/app/monster_drop.rs`) runs it between mode 0 and the
  treasure gate (1.14d order); for a host that does not call it, the
  wiring runs it after the host's start (`ActionHooks::death_cleaned`).
- **`ActionHooks` answers `0x0063A4A0`** (stays on death) from the
  states table (it returned false for every state).
- **Room dead-GUID ring `0x0061AFA0`** (rule 1.3; `RoomEntry::dead_guids`
  / `dead_next`), read by the Fallen corpse check (`ai-bodies.md` §9.4
  step 2) through the AI host's `last_dead` (was the always-empty
  `Pending::last_dead`, removed).
- **DD start `0x005A7390`** (rule 4): clean-up when not in mode 0, mode
  12, events 8 / 9 cancelled (a mode-12 request through `0x005A7C20` did
  nothing before).
- **Mode request on a dead monster** (§4.6 "What keeps a dead monster
  dead"): a start that returns 0 for a request other than 0 / 12 leaves
  the mode unchanged (no neutral start). No dead guard added: a mode-1
  request still revives the anim mode, as in 1.14d (test documents it).
- **Drop spot** (`q-fix-real-monster-drop-spot`): the play game already
  runs the floor drop `0x00555DA0` on the real `ExpField.D2` field
  (REC-108's `StartSpot` is only the field-less seam). Checked against
  1.14d: the new real-data test
  `a_monster_on_the_start_cell_moves_the_drop_one_cell_west` reproduces
  the recorded "+1/+3" case (a unit's NO_PATH centre on (x+2, y+3), ring
  search takes the west cell), next to the open "+2/+3" case.

Tests: `a_killed_monster_with_a_pending_think_stays_dead`,
`a_monster_killed_while_frozen_stays_dead`,
`mode_requests_on_a_dead_monster_follow_the_starts`,
`the_death_keeps_only_the_stay_on_death_states` (wiring/action/tests/
death.rs), `the_dead_monster_faces_its_killer` (unit_update/tests.rs:
1.14d d = 32 for a killer at (−2, −2), `combat-kill-fallen` frame 36).
Two 0x69 expectations (`kill_sends_code_8_then_the_death_end_code_9`,
test-fixtures `a_kill_sends_0x69_code_8_then_code_9_at_the_death_end`)
pinned "never turned (d = 0)"; 1.14d turns the dead monster to its
killer, so they now expect the snapped direction.

## Evidence (1.14d under Wine, `combat-kill-fallen`, state channel)

The check diverges from frame 5 on the warp's population (seed order,
not this area), so the poked fallen does not die on the d2rs side.
The 1.14d side alone settles two points used here: the fallen walks
with d = 0 and has d = 32 from its death (frame 36, killer at (−2, −2))
through mode 12 (frame 146); its gold lands at (5146, 4268) = (x + 1,
y + 3) with fallen 1:20 standing on (x + 2, y + 3).

Playthrough (`--build` binary of this branch):
`classes.play --only main-skill-kill --class all --difficulty all`:
11/21 reached (ama n/nm, sor, nec, dru all); no cell has a monster
back in mode 1 with hp 0. The 10 others are Quill Rats never hit (hp
256): pal / bar / ass skill rows and ama Hell (other sessions).
`act1.play --only kill-zombie`: 1/1.

## Open

- REC-1090 (PROVISIONAL, `units.md` §4.6 r3.1 note): the kill's
  direction is taken between path positions in sub-tiles; one 1.14d
  diagonal matches (test above); more geometries settle it.
- Rule 1.4 (`deathDmg`: bone fetish explosion, siege beast rider,
  death damage to adjacent players) and the evil-killed counter
  `0x00547E50` are still not run (population / skills owners).
- The pack handover acts on the AI store only; the worldgen
  `minions` / `owners` maps are not updated, and the owner-data flags
  (bits 0x1 / 0x2, REC-892) are never set by d2rs, so it does nothing
  in play yet. Skipped while the AI store is lent (a kill inside a
  think, e.g. a monster killing a hireling).
- `combat-kill-fallen` cannot compare the death until the Blood Moor
  population matches (seed-order owners).

## Repro

```sh
cargo nextest run -p d2-sim -E 'test(/dead|death|kill_sends|faces/)'
D2_GAME_DIR=$HOME/game cargo nextest run -p d2-client --test app_floor_drop --run-ignored all
python3 tools/playthrough/playthrough.py traces/playthrough/classes.play --only main-skill-kill --class all --difficulty all --build
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-kill-fallen.check
```
