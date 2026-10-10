# rc-su-fc-tx (REC-2790)

gen-su-* (66 checks, state channel, pokes only): EQUAL (state PARTIAL 150/150, only
the d2rs client gap, REC-2055) 47 -> 48 at the fresh run; ledger part
`ledger/rc-su-fc-tx.tsv` settles 54 superunique rows (38 EQUAL, 16 DIVERGED with fresh
first divergences). Before this session's fixes the fresh run was already 47: the
"fc" rows (gen-su-11/25/49/50/61) and most "tx" rows (47/8/22/36/51/52/64) were fixed upstream.

## Changed
1. Hydra summon alignment (`0x005543B0(m, alignment(owner), 1)`, skills/bodies-3.md §8.5):
   the `Alignment` body effect was not wired, so a Council member's Hydras kept the
   monstats class alignment (good) and targeted the Council member; now they take the
   owner's (evil) and aim the Hydra missile at the player (gen-su-26..31 tx 5153 -> 5143).
   `wiring/interaction/skill_use.rs`.
2. Knockback request ignored when the *requested unit* is in mode 19 (`0x0057F190` reads
   its own unit +0x10, not the target's): gen-su-65 tx 5140 -> 5143. `path/walk/request.rs`,
   spec pathing.md §1.2 r5, mutant test corrected.

## Open
- gen-su-26..31: first divergence now player `m` 4 vs 0 (gen-su-29: seed) after the Hydra
  missile hit (hit-reaction / will-die flag on the player), size M, not investigated.
- Other DIVERGED gen-su rows are seed / monster mode `m` (rc-su-mode) causes.
- gen-mon regression sample not run: no gen-mon 1.14d recordings cached here (fix is
  limited to Council Hydras and a knockback double-request).

## Second round (REC-3180..3182; owner goal 99/99)
gen-su-* EQUAL (state PARTIAL, REC-2055) 48 -> 59 of 66 on integ-r23 + fixes. Causes:
1. Hydra missile hit: combat `is_revived()` (unit flag 0x80000000) was always false, so the
   17% pet-vs-player damage percent (damage.md §4.2) never applied (gen-su-18, 26..31, 63).
2. Radament (hcIdx 10): the closing umod-22 tail repeated the case-10 `roll(5)` (and the
   hcIdx 42/60/62 spawns, no-ops) that population §11.4 already ran: `superunique_finish_spawned`.
3. Overseer whip class change (`BodyEffect::ClassChange`) was not wired: now `reinit` of the
   monster world (init.md §27), gen-su-56.
4. `0x005A5490` melee set-up returns without a path target unit (no hit test, no draw): the
   strike now checks `path_target` first (gen-su-56 frame 142).
5. AI `skill_entry` (`0x006439F0`) read only the summon entries; the init entries
   (`natural_skills`) were invisible to the AI, so Nihlathak's E1/E3/E4 tests failed (gen-su-60).
Regression: gen-su all 66 (no row left PARTIAL), 106 cached gen-mon/ai/nec/pal checks: same
verdicts as the ledger (nec-93, pal packets, nec-75/78 unchanged; gen-mon-688 rng ERROR is an
empty 1.14d rng recording, state PARTIAL). d2-sim 4781 pass.
Checks-status rows of gen-su-* refreshed; `ledger.py --fix` reconciled rc-gen-monai/rc-su-mode.

## Open (7 rows)
- gen-su-6 Countess: special-state-13 think `0x005E5C50` (home leash; AI commands, region ids
  `0x0061B130`, walk-home `0x005DEDE0`) is unimplemented; 1.14d draws at frame 51. Size L.
- gen-su-8: second quill hit on a player in GH: 1.14d doubles the damage and draws on the
  player's seed (monster crit? roll 15 vs Crit 5), d2rs does not. Size M.
- gen-su-12: Serpent Charge stops 2 sub-tiles short in 1.14d (path end 5147 vs 5145). Size M.
- gen-su-15 / -37: cast do event (MagottLay, MonBoneSpirit) at the S1 action frame is missing
  in d2rs (fr stays at start frame; 1.14d advances at frame 66/43). Size M.
- gen-su-34: monster mode m 2 vs 1 (rc-su-mode area). gen-su-45: Ancient 3 max life +36 hp
  (base 4300 vs 4264; equipment order?). Size M.
