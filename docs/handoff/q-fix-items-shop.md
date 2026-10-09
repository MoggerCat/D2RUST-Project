# q-fix-items-shop (`claude/q-fix-items-shop`)

REC block 1030–1039: REC-1030 used (monster and summon equipment).

## Done

- **Task 3, monster item creation `0x00573B20` and summon equipment
  `0x005D6B60`.** `InitHost::create_equip_item` / `has_item_at` /
  `has_inventory` / `new_inventory` are now real on `WorldHost`
  (`wiring/worldgen/init_units.rs`), and the summon skills' `BodyEffect::Equipment`
  is consumed (`wiring/interaction/skill_use.rs` ->
  `wiring::economy::summon_equipment`). Core: `wiring/economy/monster_equip.rs`
  (item through `Economy::create_item`, flags, mode 1, stat link, durability,
  static-path place). `monequip_rows` (`monsters/init/create.rs`) is the shared
  row walk (cutoff level, item level, oninit test, owner code for `"    "`).
  `ActionTables::monequip` carries the rows. REC-1030 (PROVISIONAL, in
  `docs/HANDOFF.md`).
  Check: `traces/checks/ass-shadow-master.check` was DIVERGED from frame 28
  (6 items missing, game seed, hp / mana); now equal through frame 47.
- **Task 2 (belt potions) does not reproduce on this branch.** `soak --new
  <class> --steps 120` (all six classes) reports no desync, the checkpoint save
  loads its belt in mode 2 (state-dump), and the repro test
  `save_roundtrip::a_new_characters_belt_is_in_the_model` passes; its `#[ignore]`
  is now the standard real-data one. The fix had landed earlier on staging
  (`5a2826a9`, `9a194c0a`).

## Open

- **Task 1, shop buy "no room": not reproduced.** C->S 0x32 on Akara / the
  `a1-den` checkpoint (belt, body gear) and on a save with scattered backpack
  items: every buy answers 0x2A result 4 until the 10x4 backpack is really
  full, then result 10 (checked: the bought copies fill all rows). No code path
  was found that answers 10 with room left (`InvDesk::place` ->
  `find_free_position` agrees with `inventory.md` §2.3). If it still shows in
  play, send the exact save, the NPC and the item (class, size) with the
  failing frame.
- **`ass-shadow-master` frame 48:** the Shadow Master leaves mode 1 (1.14d mode
  7, unit seed differs): its first AI action. Skills / monster AI, not items.
- Four-space monequip codes (owner item copy) need the owner's inventory model
  (`InitHost::owner_item_code` default `None`).
- A Blood Raven (`sbw`, oninit 1) recording would settle the init-path half.

## Repro

```sh
export D2_GAME_DIR=/home/user/game            # assembled install
# 1.14d side under Wine (tools/cloud-setup.sh, setup_winpy.sh, prepare_saves.sh):
python3 tools/scenario-diff/scenario_diff.py traces/checks/ass-shadow-master.check --orig-only
# d2rs side + diff (variant install built by the first command):
D2_GAME_DIR=$HOME/variants/blood-moor-empty target/debug/d2-client state-dump \
  --save traces/raw/check-ass-shadow-master/ScnAss.d2s --seed 1234 --difficulty normal \
  --poke '4 warp 2' --ticks 70 --out traces/raw/check-ass-shadow-master/d2rs.state.jsonl \
  --input 'frame 20; rclick 330 300'
python3 tools/trace-recorder/state_diff.py traces/raw/check-ass-shadow-master/orig.state.jsonl \
  traces/raw/check-ass-shadow-master/d2rs.state.jsonl --next 20
cargo test -p d2-sim monsters::init::tests::summon_equipment_rows
```
