# rc-cast-sound hand-back (REC-1965..1969 unused)

## Cause and fix
- q-fix-audio's REC-1683 filter in `d2-client/src/audio/unit_feed.rs`
  skipped the local player's mode request at level 0 (`record[4] == 0`) and
  played the skill start sound only at the server's level-20 echo.
  rc-cast-mode removed that echo (`skills/sequences.md` local player rules
  1-3: the local client starts its cast itself at the click; the own client
  gets 0x4C/0x4D only with E flags bit 0x4), so the local cast sound was
  silent.
- Fix: the filter is gone. The new `skill_start_request` takes the last 0x15
  / 0x16 request of any unit, local player included, whose count moved.
  The echo for E flag 4 skills (dodge/avoid) still arrives as a second
  request, as in 1.14d (its start sounds play at it too).
- Test: `audio::unit_feed::skill_start_tests::the_local_players_click_request_at_level_0_plays_its_start`
  (the old inline filter returned `None` for the local player's level-0
  request; the test calls the extracted function, so it asserts the new rule
  rather than failing on the old inline code).
- Gates run: `cargo fmt`, `clippy -p d2-client --all-targets -D warnings`
  clean; d2-client `--lib` test above passes.

## Checks
- Not run: the cloud disk was full (the private repo clone is 9.4 GB), so
  no game install and no `audio-cast-frost-nova-sor` run. Queued in
  `docs/handoff/pc1-data.md` Step 4 ("[rc-cast-sound]"). Before/after EQUAL
  counts: unknown.

## Open
- Run the queued check; if `coldcast.wav` is not at T 18 / 58, the cause is
  elsewhere in the start sound path (size S).
- Full `cargo nextest -p d2-client` and the integration tests were not run
  (disk).
