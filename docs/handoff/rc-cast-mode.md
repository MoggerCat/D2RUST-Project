# rc-cast-mode hand-back (REC-1830..1834: none used)

Task: d2rs sent two cast mode requests per local cast (code 0x15/0x16,
level 0 then level 20) where 1.14d sends one.

## Cause and fix

- Request 1 (level 0) is the client's own click request
  (`d2-client/src/bridge/click.rs` `skill_request`, `skills/sequences.md`
  local player rules 1-2): correct.
- Request 2 (level 20) was S->C 0x4D/0x4C from the player skill-mode row
  `0x00548090` (`d2-sim/src/wiring/path/walk.rs` `update_messages`), which
  d2rs sent to the caster's own client too (an old PROVISIONAL echo).
  `sim/pathing.md` §10 r2 and `sequences.md` local player r3 (settled):
  the own client gets it only when the used skill's E flags have bit 0x4
  (dodge/avoid). Fixed: `own_skill_message` gate; other clients unchanged.
- Tests: new `wiring::path::tests::a_cast_skips_the_casters_own_client_unless_e_flag_4`
  (fails on the old code: own client got 0x4D level 20);
  `unit_update::tests::a_player_attacking_a_monster_sends_0x4c` updated
  from the old echo to the spec rule (nothing without E flag 0x4, the
  same 0x4C/0x4D with it). d2-sim 4738/4738, d2-server 397/397.

## Checks (gen-skill-*, orig-cache, state channel)

- Before: 150 PARTIAL (no difference), 60 DIVERGED, 12597/14700 ticks.
- After: identical (150 / 60 / 12597). The state channel compares
  server state, and this was a server-to-own-client message, so no first
  divergence moved. The brief's guess that this sits in the gen-skill
  first divergences is not borne out.
- Ledger part `q-run-gen-skills.tsv` is stale (199 DIVERGED, mostly @2
  `q`); the current run is 150 PARTIAL / 60 DIVERGED. Its owner should
  refresh it (my part can't override it: name order).

## Open (for others)

- q-fix-audio REC-1683 (`claude/q-fix-audio`, not on integ-r10) filters
  out the local player's level-0 request and plays the cast sound at the
  level-20 echo. With this fix, there is no echo: that filter must flip
  (local start uses its own level, `msg-skills.md` §7 r4.3) or the local
  cast sound goes silent. Owner: q-fix-audio / client audio. Size S.
- Next shared gen-skill first divergences (after): 16 missile present
  in 1.14d / missing in d2rs (sor 36-55 odd, 64; nec 67, 84; pal 101;
  dru 225, 230; ass 266), 10 player `tx`, 5 monster `m`, 5 game `seed`.
  Not started (one root cause per session). Size M.
- d2-client debug tests not run (disk 3.7 GB free); the client crate is
  unchanged and the 210-check release suite exercises it.
