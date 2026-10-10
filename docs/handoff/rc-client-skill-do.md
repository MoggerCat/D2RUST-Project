# rc-client-skill-do hand-back (REC-2205..2208 used; 2209 unused)

Base: claude/specs-staging-7 + claude/integ-r17, then staging with r18 (sync); all results re-measured on the final head.

## Checks
| check | before (r17) | after |
|---|---|---|
| audio-cast-frost-nova-sor | voices 1.14d 12 / d2rs 10, paired 10, 2 differences (novaice.wav T 25, T 65); mixed 2/99 | voices 12 / 12, paired 12, **0 differences**; mixed 2/99 (unchanged, first at T 2) |
| sor-frost-nova, sor-frost-nova-twice, sor-nova, gen-skill-sor-44, gen-skill-sor-48 (state) | 100% PARTIAL | 100% PARTIAL (no change) |
| fxfrostnova render scenes (cast / flight / hit / later) | 14.8 / 14.3 / 14.2 / 15.5 % pixels, first diff tick 3 (frame.tsv row 5 level) | 19.1 / 17.3 / 16.6 / 15.4 %, first diff tick 3 (row 8 tile_origin_x) |
EQUAL count: 0 -> 0 (the audio check stays DIVERGED on the mixed channel only).

## What changed (read in re/exports-typed; spec: specs/client/msg-skills.md §11)
- New `bridge::client_do`: `0x004C68F0` (used skill + level with bonuses) ->
  `0x004C6680` (cltdofunc table `0x00727BA8`, 130 entries, null entries 0 and
  97..129 only set flag 0x40) -> function 25 `0x004E34E0` (missile pick
  `0x004F21B0`, v = Vel + VelLev*L/8 `0x00663270`, ring `0x004C70D0`: 64 client
  creates, flags 3|4, offsets = the server ring, all 64 checked in the image).
- `player_anim::step` is the player update's do part (`client/model.md` §19 r2,
  r3): kind-2 modes (table `0x00711E00` read from the image; settles the rows
  REC-1000 assumed), flag 0x40 clear, +0x4E in 1..3 from the previous advance
  (now the `sim/units.md` §4.2 advance with the AnimData events), and the
  remote-player do at the mode end; used skill := none at the end.
- Mode set clears flag 0x40 (`0x00480E70`); 0x15/0x16 set the used skill
  (entry of r0/r1, `0x006439B0`). ClientUnit: `flag_40`, `action_frame`,
  `anim_events`; SkillRow: cltdofunc, cltmissile(a..c), cltcalc1,
  progressive, aurastate, aurastat1; PlayerAnim: events.
- Probe: the two rings are units 3/208.. and 3/320.., as 1.14d's 3/208..271.
- Private repo: 7 names appended to re/exports/names.tsv.

## Open
- Effect render scenes: fxfrostnova re-run (fxnova not, same blocker): the
  first difference is still the scene's world placement (tile origin, before
  any cast), so no nova sprite is compared; re-run after that (owner: client
  click / waypoint path, q-chk-render-effects.md). Size: re-run only.
- Other cltdofunc bodies (26 Fire Wall, 28, 30 (13 skills), ...), the
  `cltmissile` create, dosound / tgtsound / overlay / delay UI of §11 r5 and
  the monster update's do (`0x004AF4C0`): size L, PROVISIONAL REC-2207.
- E-flags bit 0 path-step do (REC-2208), used skill for a skill the list
  lacks (REC-2206): size S each.
- Mixed channel of the audio check (first difference T 2): not this cause.
