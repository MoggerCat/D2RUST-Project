# q-diff-combat-a1 — hand-back (2026-10-09, branch `claude/q-diff-combat-a1`)

Area B: outdoor monster combat in Act I, diff-driven against 1.14d under
Wine (`docs/handoff/diff-driven.md`). REC block 750–759 (used: 750–754).

## Done (pushed, gate green)

- **Killed monsters stand back up** (playthrough blocker): the monster DT
  start skipped the death clean-up's last call `0x005738D0` (cancel type-2
  thinks and type-3 regeneration, `sim/units.md` §4.6 r1.2); a think
  scheduled before the kill ran on the dead unit (mode 5 / 1, hp 0).
  `monsters::ai::cancel_think_and_regen`, called in
  `wiring/action/units.rs` after the host's DT start. Test
  `the_death_start_cancels_the_pending_think_and_regeneration`; three e2e
  expectations that pinned the pending think now follow the spec.
  `playthrough --all`: kill-zombie, duriel-, council-, mephisto-,
  baal-killed reached.
- **Poke `msg <id> <values>`** (`tools/poke.md` §1, §4 r9, §5 r3): one
  C→S message on an exact frame on both sides (1.14d client sender
  `0x00478350`; d2rs `Host::send_game`, duplicate filter included).
  Checked on 1.14d: `0x06` attacks and `0x03` / `0x49` work.
- **umods 38 / 39 life** (REC-752, `monsters/init.md` §19.6): maxhp and
  hitpoints both written with the new value; `combat-umod-life` 24/24.
- **Objects stamp collision when the object state is lent** (REC-750,
  with staging's own stamp) and **run speed includes the run list**
  (REC-751, `pathing.md` §8.2): the 0x03 run matches 1.14d.
- **Path preparation onto the start keeps the target** (REC-753,
  PROVISIONAL, `pathing.md` §4 r3; pc1-data item "preparation probe at
  the start").

## Checks (`traces/checks/`)

combat-kill-fallen, -champion-pack, -unique-pack, -random-boss,
-elements (Blood Moor via `warp 2`; town allows no damage), -umod-life
(town, spawn init only), -melee-fallen-msg (player attack by `msg 0x06`),
-monster-missiles (quill rat, fallen shaman), -pop-blood-moor,
-pop-cold-plains, -pop-stony-field (warp, natural population, 400
frames), -cold-plains-wp (run + waypoint by `msg`). 1.14d sides recorded
locally (`traces/raw/`, not committed; re-record with `--orig-only`).

## State at stop / next steps

1. **First divergence of the Blood Moor checks** was the warp tile vs
   object creation order (frame 4); fixed in staging by q-diff-skills-2
   (merged here). Re-run every combat check (d2rs side:
   `rm traces/raw/check-<n>/d2rs.*; python3
   tools/scenario-diff/scenario_diff.py traces/checks/<n>.check
   --reuse`) and fix the next first divergence. Seen on staging before
   that fix: the Blood Moor population differs from frame 5.
2. **Fallen think seed step** (from q-diff-skills-2, my area): a poked
   fallen out of town, each AI think steps its unit seed once less in
   d2rs (frame 11: 1.14d 1 step, d2rs 0; frames 15/25/35: 3 vs 2);
   check `variant blood-moor-empty` on claude/q-diff-skills-2.
3. **andariel-killed** (playthrough act1 #14): Andariel never dies
   (no death mode by f200): a damage problem, not the stand-up bug.
4. **radament-killed** (act2 #6): Radament stays at hp 256 (mode 4
   get-hit) to f1000.
5. **combat-pop-stony-field**: frame 4 object GUID 18 class 37 vs 119
   (object order); **combat-pop-cold-plains**: frame 4 player arrival x
   5168 vs 5183 (level warp arrival, `path-placement.md` §11/§12.2).
   Re-check after (1).

## Open rows (assigned by the coordinator, not started)

q-fix-p3-death-path-end, q-fix-p4-mon-frame-code,
q-fix-p4-range-meleeonly, q-fix-p4-evade-sound-only,
q-fix-p4-mon-missile-weapon (`docs/handoff/build-queue.tsv`; each row
states the spec rule, the file and the test).

## Open RECs and PC 1 items

- REC-753 PROVISIONAL: pc1-data "[q-diff-combat-a1] preparation probe at
  the start" (`0x00648120`).
- REC-754: pc1-data "[q-diff-combat-a1] town NPC thinks while the player
  runs" (Gheed / Charsi stop thinking once the player has run off).

## Repro

```
tools/cloud-game/README.md setup; export D2_GAME_DIR=$HOME/game
python3 tools/playthrough/playthrough.py --all
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-pop-blood-moor.check
```

Notes: two Wine prefixes made by `cp -al` share `.run.lock` (runs
serialize); a `run.sh` killed by `timeout` can leave a `sleep` holding
the lock (kill it). Disk is about 40 GB per session: one debug target
plus one release build fit; `rm -rf target/debug` before a clean gate if
a merge brought much code.
