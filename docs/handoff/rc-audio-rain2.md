# rc-audio-rain2 hand-back (REC-2185 used for the spec note; 2186..2189 unused)

Base: claude/specs-staging-7 + claude/integ-r16 (merged). No Rust change this session.

## Checks (tools/audio-diff, traces/audio/*.check; orig recorded under Wine, none EQUAL)
| check | before (rc-audio-cast-run / rc-client-seed-2) | after (r16, measured now) |
|---|---|---|
| cast-frost-nova-sor | 4 differences, mixed 2/99 | 2 differences (voices 1.14d 12, d2rs 10, paired 10), mixed 2/99 |
| monster-hit-ama | 37 | 36 (mixed 2/199) |
| town-ambience-ama | 40 | 39 (mixed 167/249) |
| walk-town-ama | 65 | 66 (mixed 8/119) |
EQUAL count: 0 -> 0.

## Found
- rain2 start lag: gone. No rain difference in any of the four checks after the
  integ-r16 merge (weather replay + client seed, rc-client-seed-2). The rain step
  itself matches environment.md §6; nothing to change there.
- The unknown 35015-byte sound in the cast check is `skill\sorceress\novaice.wav`
  (audio_diff compare --sounds; Sounds.txt ESOUND_SORCERESS_FROSTNOVA, index 1845,
  logged request id 2422 = index + 577). 1.14d requests it 64 times at T 25 / 65
  (units 3/208..271, caller 0x004CDB04 = end of client missile create 0x004CD540),
  as the missile `TravelSound` of the 64 Frost Nova missiles.

## Cause (read in re/exports-typed; spec: specs/client/msg-skills.md §8 r4)
- Frost Nova (`cltdofunc` 25) has no `ClientSend`: the client makes its own 64
  missiles in the client skill do `0x004E34E0` -> ring `0x004C70D0` (flags 3|4, 64
  target offsets from 0x006DACC0 / 0x006DABC0, same values as the server ring) ->
  `0x004CD540`.
- d2rs runs the server side (nova at frame 26 / 66, missile 119) but the client has
  no client skill-do dispatcher: `Output::SkillDo` only feeds audio, and
  `client_missiles::create` is never called in this check (probed). So the 64 client
  missiles, and their TravelSound request, do not exist. This also means the ice
  nova is not drawn client-side.

## Open
- Client skill-do dispatcher + entry 25 (size M, owner: client missiles, see
  `tools/coord/route.py crates/d2-client/src/bridge/client_missiles.rs`, which is
  unrouted here). Needs skill-row columns cltdofunc / cltmissilea / calc, the
  missile rows and lights in the message handler, then a draw-check re-run (64 new
  missiles change draws). Expected audio gain: the 2 cast differences.
- Footsteps / warcry variant, pan, volume, start tick (size M, untouched): walk-town
  66, town-ambience 39, monster-hit 36 differences; unknown 19624-byte sound at T 98
  in monster-hit.
- Gates: ledger --check and spec_index --check pass. `coverage.py --check` has one
  error that is not from this change (d2-sim debug/state/tests.rs:409 malformed rule).
