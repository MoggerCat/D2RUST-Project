# rc-client-seed-2 hand-back

Task: make d2rs' client seed match 1.14d before the first rain update.

Checks (tools/audio-diff, orig under Wine; cloud capture, so rain T 4 there):
- audio-town-ambience-ama: DIVERGED 38 -> 40 differences (rain no longer first; footsteps)
- audio-cast-frost-nova-sor: DIVERGED, 4 (after; no before run)
- audio-monster-hit-ama: DIVERGED, 37 (after; no before run)
- audio-walk-town-ama: DIVERGED, 65 (after; no before run)
- draws-town-arrival-ama: DIVERGED row 111 (object vs monster shadow, not weather; no before run)
- No EQUAL gained among the checks. Rain voice: d2rs T 3 = Windows capture T 3
  (REC-1363 ground truth, traces/audio/win); before T 7 here. The rain ramp equals
  facts/client/weather/a1-town-rain-start.tsv (target 1@C3, 2@C5, 3@C7, 4@C9, 5@C11).

Root cause (read in the Ghidra export):
- 0x00472890 (act load) runs a one-time 0x00472610 (flag 0x007A8A30): 3 steps.
- S->C 0x15 for the local player ends in 0x00472C20(flag) -> 0x004726F0(flag): 3 steps
  at the join (flag 1). 0x004726F0 draws nothing while the player has no room.
- d2rs updated the weather only on drawn frames; GPU-skipped ticks lost updates and the
  sound tick ran ahead of the weather.

Changed (d2-client): `Weather::wind_ready`; `ClientWorld::local_places` replayed by
`WeatherView` before the act load (REC-1901, PROVISIONAL); weather replayed per skipped
client update and run for undrawn ticks (`ViewFeed::weather_update`, REC-1900,
PROVISIONAL). Spec: draw-order-2.md §11.8. Test: first_act_load_adds_the_one_time_wind_init.
Gates: fmt, clippy, d2-client lib tests (2290), coverage, spec_index pass.

Open:
- Footsteps (M): extra at T22/30/46/54, missing T65 in ambience. Candidates: idle cursor step
  (render/capture.md 3.3, own seed copy in ui/cursor_ui.rs, wall-clock), overlay creates,
  room-change draws 0x00472C20 for flag 0 placements. The Windows capture has no seed values;
  pc1-data.md Step 4 "[rc-client-seed]" still needs a roll-hook recording.
- Not examined: birdie05 at T58, warcry variants.
