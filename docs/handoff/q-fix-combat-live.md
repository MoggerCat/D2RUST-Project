# q-fix-combat-live (done)

Combat exactness against 1.14d, checked live: 1.14d under Wine in the
cloud (`tools/cloud-game/`), `tools/scenario-diff` state and rng
channels. REC ids used: REC-1260.

## How the checks were read

Every combat-* check still first diverges at **frame 5** (the Blood Moor
population on warp, game seed and monster creation draws:
q-fix-real-unit-seed-order / q-fix-seed-order). That divergence also
shifts GUIDs (the poked monster is GUID 19 in 1.14d, 17 in d2rs), so
`state_diff.py` (pairs by GUID) reports nothing useful after frame 5.
The combat part was compared with a pairing that follows the pokes:
player 0:1, the poked monster and its pack by GUID offset from the spawn
poke, poked missiles by poke order, other missiles by GUID. Script and
use under Repro.

## Done (all pushed to claude/q-fix-combat-live)

1. **Missile data flags 1/2 → hit flags 0x20/0x80** (`missiles.md`
   §R6.1 step 5). The missile store is lent out during a hit, so the
   wiring's `apply_missile_record` saw no missile data: the HitClass
   merge of q-fix-player-hit and these flags never ran on the live path.
   `MissileCombat::apply_damage` now gets the missile data from the
   caller. (q-fix-player-hit's own version, merged, read the lent store;
   this branch keeps the working one.)
2. **0x4000 soft-hit rule:** nothing to add. `damage.md` §7.1 steps
   4.5–4.7 / 5.3 / 5.7 are implemented in `wiring/action/reaction.rs`
   and server-damage function 3's soft hit reaches the record.
3. **Monster AI state** (`ai.md` §3.1; q-fix-c3-quillrat-choice): done
   here, then replaced by q-fix-monster-ai's identical staging version
   at the merge.
4. **Death direction toward the killer** (`units.md` §4.6 rule 1.2):
   done here, then replaced by staging's `monster_death.rs` at the merge.
5. **Missile collision unit search** `0x00641CB0` with sizes
   (`path-placement.md` §4 rule 6): the r × s shape overlap and the
   dead / object / item skips, through `path::collision::unit_at_point`.
   d2rs took only units on the exact sub-tile.
6. **Champion / unique flags of the death drop** from the monster data
   (`treasure.md` §3.2): the drop asked the unanswered Pending seam, so a
   champion dropped from column 1.
7. **Frame bonus** `0x00623B10` (`units.md` §4.7) computed in d2-sim from
   the draw identity, the dual-wield class rule and the attack weapon's
   type class (new seam `Pending::item_type_class`, answered by the
   d2-client weapon copy). It was an unanswered seam (always 0).
8. **Monster death by regeneration** (`stat-lists.md` §10.1 step 6):
   `UnitHooks::monster_death` had no implementation on the action
   wiring, so poison / open wounds left monsters alive at life 0. Now the
   kill (`reaction::kill_by`, killer optional) and the death events
   (PROVISIONAL REC-1260: killed 10 on the unit, kill 9 on the owner).

Tests (all fail without their fix): `missile_data_flags_set_the_domissiledamage_hit_flags`,
`missile_unit_search_uses_the_shapes_and_skips_the_dead`
(`wiring/action/tests/missiles.rs`); `a_champion_drops_from_column_2_by_its_monster_data`,
`player_attack_starts_at_the_frame_bonus`, `death_start_faces_the_killer`
(`tests/death.rs`); `poison_kills_a_monster_at_zero_life_by_its_owner`
(`tests/combat.rs`).

## Per check (combat units, paired as above)

"Before" = docs/handoff/checks-status.md (whole state, GUID-paired);
"first combat divergence" = the paired comparison at the start of this
session's work on that check; "after" = now.

| Check | Before (status) | First combat divergence | After |
|---|---|---|---|
| combat-arrow-quillrat | f5 seed | f59 rat AI (AI state) | equal to the end |
| combat-arrow-kill | f5 seed | f38 rat death direction | equal to the end |
| combat-champion-pack | f5 seed | f58 fire bolt misses the minion (unit search); then f85 champion drop | equal to the end |
| combat-elements | f5 seed | f45 missile misses (unit search); then f78 poison death | equal to the end |
| combat-kill-fallen | f5 seed | f74 fallen leader handover | equal to the end (staging's handover) |
| combat-fallen-hits-player | f5 seed | equal | equal to the end |
| combat-potion-midfight | f5 seed | equal | equal to the end |
| combat-random-boss | f5 seed | equal | equal to the end |
| combat-pop-blood-moor | f5 seed | equal (player only) | equal (player only) |
| combat-pop-stony-field | f4 object class | equal (player only) | equal; first divergence now f5 seed (staging D11) |
| combat-umod-life | PARTIAL | equal | equal |
| combat-melee-fallen-msg | f5 seed | f34 player +0x44 (frame bonus) | f40: GUID-addressed poke hits another fallen (frame-5 GUID skew) — **routed** |
| combat-melee-fallen | f5 seed (packets f1) | f46 | f46 — **routed** (below) |
| combat-monster-missiles | f5 seed | f32 Fallen Shaman | f32 — **routed** |
| combat-unique-pack | f5 seed | f51 Fallen Shaman | f51 — **routed** |
| combat-cold-plains-wp | f93 player m | f93 player TN vs TW | f93 — not combat, **routed** |
| combat-pop-cold-plains | f4 player x | f4 | f4 — q-scenes-compare (already routed) |

## Open (routed)

1. **combat-melee-fallen f46** (barbarian melee kill of a fallen; rng
   channel, draw by draw): 1.14d makes (a) a block draw on the fallen
   (`0x57E024`, `hit.md` §6.1), (b) the physical roll over 1664 (short
   sword 2–7 + 30 % strength bonus) where d2rs rolls over 1280, (c) a
   durability draw on the player (`0x559E9B`, `damage.md` §9). All three
   are unanswered seams on the real host: (a) `Pending::composit_shield`
   (the SH component code; d2-data's monstats2 lacks the per-component
   choice codes of `+0x26 + 12c + v`), (b) `Pending::str_dex_bonus`
   (`InvItemRec` lacks StrBonus/DexBonus), (c) `Pending::item_has_durability`.
   Owners: items (q-fix-server-store-fill) for (b) (c); monster data /
   composit for (a). The treasure draw that follows then lines up by itself.
2. **Fallen Shaman** (`ai-bodies.md` §9.6; monster-missiles f32,
   unique-pack f51): the draws agree up to step 5 (P(aip2) passes), then
   1.14d starts ShamanFire in `Sk2mode` (mode 14, `0x005DEAD0`) while
   d2rs's skill use at T fails and falls through to step 7 (P(aip3) and a
   circle draw at `tactics.rs:321`). Monster skill-use path → q-fix-monster-ai.
3. **Frame 5 population / GUIDs** (every check; melee-fallen-msg's
   `msg 0x06 1 19` then names another unit) → q-fix-real-unit-seed-order.
4. **combat-cold-plains-wp f93**: the player in town is TN (5) in 1.14d,
   TW (6) with a different target in d2rs → walk / client path
   (q-scenes-compare / q-fix-client-crash).
5. PC 1 question added to `docs/handoff/pc1-data.md` Step 4: what
   `0x005539B0` step 5 writes to +0x44 (spec `f · 256`; 1.14d reads 256 =
   the frame bonus through an Amazon A1).

## Repro

```sh
sh tools/cloud-setup.sh && tools/cloud-game/setup_winpy.sh && tools/cloud-game/fetch.sh
export D2_GAME_DIR=$HOME/game && tools/cloud-game/prepare_saves.sh
# both sides (≈ 3–5 min per check under Wine):
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-elements.check --channels state
# d2rs side again against the recorded 1.14d side:
rm traces/raw/check-combat-elements/d2rs.state.jsonl
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-elements.check --channels state --d2rs-only --reuse
# draw-by-draw: --channels rng, then orig.rng.jsonl / d2rs.rng.jsonl (field owner "unit T:G", site)
```

The pairing comparator used above (not committed; ~60 lines): load both
`*.state.jsonl`, pair player 0:1, each `poke` record with a `guid` by
order (spawn → type 1, missile → type 3), monsters at or past the first
spawn GUID by the offset of the two spawn GUIDs, other missiles by equal
GUID, and print per frame the fields that differ (ignoring `g`, `own`,
`q`). A tool for this belongs to q-tool-state-diff (a `--pair-pokes`
option of `state_diff.py`).
