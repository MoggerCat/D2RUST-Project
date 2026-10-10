# rc-su-mode (2026-10-10)

Cause: ~19 gen-su-* / gen-boss-* rows DIVERGED on "monster ... field m".

Checks before -> after (EQUAL rows of the 19): 0 -> 17 (ledger part
docs/handoff/ledger/rc-su-mode.tsv; global EQUAL 2080 -> 2097).

Fresh re-run (gen-su-* 66 checks, gen-boss-* 25): 15 of the 19 rows had no
divergence any more (fixed upstream this morning); they are EQUAL under
REC-2055 (pokes-only checks, state PARTIAL only for the client gap).

What still diverged on m and what changed:
- gen-su-27 (Geleb, council members, 1.14d frame 63 mode 8 vs walk):
  `BodyEffect::Alignment` (0x005543B0, Hydra skill step "O a monster -> m's
  alignment := O's") was a no-op, so a council member's Hydras stayed good
  (alignment 2) and the HighPriest heal scan (0x005E0430) never found them
  (life percent 0, evil); 1.14d casts S1 at one at frame 63. Now applied
  (`skill_use.rs` effect -> `View::set_alignment`). The client's
  "listed pet = player side" fudge (single_player.rs) now only covers pets of
  players / allied owners. Spec: specs/skills/bodies-2b.md §8.5.
- After it, gen-su-26..31 (council) are equal through frame 69 (seeds,
  missiles); next divergence is a different cause below.

Open:
- Hydra fire bolt damage (missile 247 `hydra`, fired by a monster-owned
  hydra, skill lvl 3): at frame 70 1.14d takes 2593 (10.1 life) per hit,
  d2rs kills the 50-life player (hp 0, mode 0 vs 1.14d mode 4).
  d2rs builds the hit record with level 3, skill row 62 (Hydra), fire
  min/max 6400/8320 (25-32.5 life, 3 hydra missiles in one frame); 1.14d's
  per-hit loss is 2593 (10.1 life). Read first: which level the monster's
  Hydra uses in 1.14d (SetSkill level 1 vs skill_stats L=3) and the
  monster-side skill damage path.
  Rows: gen-su-26,27,28,30,31 (monster.superunique council, size S-M).
- gen-su-34 taintbreeder frame 73 monster m 2 vs 1 (not looked at).
- Player-mode m rows gen-su-12/18, gen-boss-250/544, gen-umod-30,
  gen-su-65 (tx 5143 vs 5140): player-side, not monster mode change.
- Next layer on remaining rows is seed / spawn order (gen-su-6,10,15,37,60,
  gen-boss-242,333,707), not m.

Verified: d2-sim nextest 4776 pass; clippy d2-sim d2-client clean.
