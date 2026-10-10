# rc-client-seed hand-back

Task: weather and sound variant picks on one shared client seed.

Checks (tools/audio-diff, audio-town-ambience-ama only so far):
- before: DIVERGED, 32 differences (rain2 T 3 vs T 11; 7 footstep variants)
- after:  DIVERGED, 30 differences (rain2 T 3 vs T 10; 9 footstep variants)
- not run: the other three audio checks and the render-world rain scenes.
  No EQUAL gained.

Changed (d2-client):
- `SoundLink` holds one `Arc<Mutex<ClientSeed>>`; `SoundDriver` adopts it in `SoundLink::set`.
- `WeatherView::prepare` syncs it from the model, then steps it for the
  update; `WeatherFrame` writes the seed back on drop (`SeedCommit`), so floor
  and pass 4/9 draws step it too.
- REC-1845 (PROVISIONAL): draw order inside a frame is receive, update, weather,
  then sound tick (`sound-table-2.md` §14.2); frames dropped under load not modelled.
- REC-1846 (PROVISIONAL): `Seed::init_low(guid)` fallback while the model holds no seed.

Open: the seed value at game start and the draws before T 3 are unknown;
queued as the "[rc-client-seed]" item in pc1-data.md Step 4. Birdie05 at T 58
and the warcry variants were not examined. Screen shake, cursor and room-change
draws (sound-table-2.md §14.3) still do not share the seed.
