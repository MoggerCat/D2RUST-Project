# rc-ancient-tx hand-back

Cause: rc-run-1-causes row 2 (state tx 4324 vs 4321, "class 522"). Class 522 is
act5barb1 (NpcBarb, Align 1), not an Ancient. Its run target is the unit found
by the not-evil main search; d2rs picked Death Mauler 1:18, 1.14d 1:19.

Checks: EQUAL before 0 -> after 0 (of the 20 + a5-warp-l110-siege-1-ama).
The tx difference is gone in all of them; the first difference moved:
- 14 checks (ancient1-3, axe-dweller, blaze-ripper, eyeback, frozenstein,
  megaflow-rectifier, nihlathak-boss, sharp-tooth-sayer, shenk, snapchip,
  threash-socket, siege-1-ama): 6 -> 35 frames equal; next: frame 36 Mauler
  1:18 hp 1.14d 55808 vs d2rs 54409 (d2rs applies the barbarian's melee hit,
  1.14d does not at that tick; siege-1-ama also rng site 0x5a55ba missing).
- 6 checks (anodized-elite, bonesaw-breaker, dac-farren, magma-torquer,
  pindleskin, vinvear-molech): now frame 16 game seed differs (an RNG draw
  count at the superunique spawn tick; not examined). Size S-M, d2-sim/host.
gen-boss-*, gen-mon-540..542, gen-ai-ancient*, gen-ai-npcbarb, gen-su-*: verdicts
unchanged (state PARTIAL where they were); they are not in checks-status.md.
The chain was cut by the time limit before every gen check finished.

Changed: d2-client LocalSeams::good_target_search (crates/d2-client/src/app/
single_player.rs): full-size distance with the candidate's size, 35 inclusive;
spec note in specs/monsters/ai.md §5.4. The unit test was updated but not run
(d2-client test build died when the disk filled).

Open: scan 5 `threat` main/alt split and walk-order tie-break; barb melee hit
at frame 36; the frame-16 seed cause. Fix lives in d2-client (preview seams),
not d2-sim: LocalSeams is the Pending impl the state-dump uses.
