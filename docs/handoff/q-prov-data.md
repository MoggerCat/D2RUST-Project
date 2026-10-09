# q-prov-data hand-back (branch claude/q-prov-data)

Session `q-prov-data`, cloud, 2026-10-09. REC ids 560-569: none used.

## Done (all pushed)

- **Data points settled on the real install** (batch 1): REC-208 (Patch_D2
  `patchstring.tbl` 97/98 = Druid/Assassin), REC-220 (Patch_D2 has no
  loading-screen override), native-assets OQ6 (21 string tables, C-TBL and
  hash rebuild equal), REC-166 `FrameCnt1` = txt x 256. Tests: `d2-native`
  `real_tbl`, `d2-client` `prov_data_tables`, `front_host_real`.
- **Fixed from the data**: front-end logo and button file names, draw
  mode 3 as the PL2 additive table in index space, art frame offsets,
  stash (classic top 273/333) and cube (118,139) grid fallbacks.
- **Comment cleanup** for REC points settled by PC 1 specs: REC-289 (5),
  410, 415 (0x93), 400, 460, 406 (single client). The rest differ in d2rs
  and belong to q-fix rows.
- **q-fix-p6-object-door-step** (door step 0x004BCB20) and
  **q-fix-p6-npc-intro-record** (Malah, Nihlathak, Qual-Kehk).
- **Act IV**: player act set at game entry (save-start in Acts II-V now has
  the right act); object footprints stamped at the unit add when
  `HasCollision[mode]` (found by `a4-warp-plains-ama`: populated objects
  were placed on each other); `HostQuests::view` keeps seed, items and
  uniques (Act III save-start game seed now equals 1.14d).
- Checks added: `a4-fortress-arrival-ama`, `a4-warp-plains-ama`,
  `a4-start-noquest-sor`, `a3-start-noquest-sor`, `a5-harrogath-arrival-ama`
  (not run). Playthrough: `act4.play` izual-present sweep step 40;
  `act4-blockers.play` probes.

## State

act4.play: 7 of 12 milestones reached (3 consecutive) as of the last run.

## Open (next steps)

1. `izual-killed` (and the same kill recipe): harness issue, not d2rs. The
   `pos` poke is a raw teleport; the sweep leaves the player in a wall cell
   (collision bits 0x5 at (5143,5263), level 105), so the Fire Bolt's first
   step returns Stopped. Fix in the harness: after `find`, move the player
   to a walkable cell at the found unit (q-tool-checkpoints `goto`).
2. `hellforge`, `hephasto-present`: need River of Flame preset positions
   (`goto preset`, PC1-C recording); sweeps cannot reach them (level is
   200x200 tiles, `pos` refused on lava).
3. `a4-warp-plains-ama` first divergence is frame 24: three Fortress NPCs
   (405, 257, 246) run the Npc map AI (2 draws each) after the player left
   the level; 1.14d does not. PC 1 Step 4 item "Monster think in a room
   with no clients".
4. Hratli's unit seed is two steps behind 1.14d at f2 (`a3-start-noquest-sor`):
   PC 1 Step 4 item "Hratli's unit seed two steps at creation".
5. Town-start predicates in act2/3/5.play expect too many NPCs (only the
   NPCs in the player's active rooms exist at f40 on both sides).

## Open RECs / PC 1 items

- Two `[prov-data]` items in `docs/handoff/pc1-data.md` Step 4 (above, 3 and 4).
- Points left in the provisional index for q-fix rows: REC-100, 111, 115,
  265, 401, 412, 414, 415 (0x20), 416, 420, rest of 289.
- Spec gap: 0x3E null item GUID (spec 0xFFFFFFFF; `skill_use.rs` sends 0).

## Repro

```
export D2_GAME_DIR=$HOME/game CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 tools/scenario-diff/scenario_diff.py traces/checks/a4-warp-plains-ama.check
python3 tools/scenario-diff/scenario_diff.py traces/checks/a3-start-noquest-sor.check
python3 tools/playthrough/playthrough.py traces/playthrough/act4.play
cargo nextest run -p d2-sim -p d2-server -p d2-client -p test-fixtures
```
Disk note: the cloud container has little free space; delete
`target/debug/incremental` and large test binaries in `target/debug/deps`
between full runs.
