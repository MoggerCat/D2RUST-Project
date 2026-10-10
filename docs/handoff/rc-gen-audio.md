# rc-gen-audio: generator for the system.audio ledger rows

Branch claude/rc-gen-audio. REC ids used: none.

## Checks
- `tools/check-gen/check_gen.py --family aud` writes 10 audio-diff checks to
  `traces/audio/gen/` (`gen-aud-*`; scenario table `AUD_SCEN`). Audio checks
  stay out of `traces/checks/gen/`: `scenario_diff.CHANNELS` and `suite.py`
  have no `audio` channel, so they run with
  `python3 tools/audio-diff/audio_diff.py run traces/audio/gen/<n>.check --json F`
  (no orig-cache; each run records 1.14d again). `check_gen.py --audio-out DIR`.
- EQUAL before -> after: 0 -> 0 of the 37 NO-CHECK rows. All 10 checks ran
  end to end (1.14d capture + d2rs + compare) and are DIVERGED.
- 24 rows now carry a DIVERGED verdict (`DIVERGED@1`); 13 stay NO-CHECK with
  the reason in `note` (`AUD_NOCHECK`): EAX env (mode 2 only), quest
  stingers (no poke raises quest events), front-end music, table loading,
  d2rs mapping, client-seed users, options sliders, cache residency,
  >16 voices, wall-clock fixed requests, 0x2C senders, ProgSound, driver table.
- Ledger part: `docs/handoff/ledger/rc-gen-audio.tsv` (ledger.py --check clean).

## Results (paired voices / differences, mixed ticks equal)
town-idle 8 pairs/38 diffs, 15/249; warp-levels 9/122, 1/259; npc-talk-akara
4/3, 7/119 (closest: only start ticks differ); item-drop 8/43; object-chest
9/47, 39/139; monster-idle 40/213; monster-attack 10/50; ui-panels 5/45;
player-chat-event 6/37; hit-sequence 22/123.

## Open (causes for others; not root-caused here, game code untouched)
- Start tick differs by one on most paired voices (as the existing audio
  ledger rows say): sound-tick phase, owner of audio/sound-table.md §6.
- Device volume / pan differ on most pairs (§8 volume and pan).
- Unpaired voices both ways (d2rs plays 3 more in every capture; variant picks
  differ with the shared client seed): triggers §6/§8 owners.
- Size per cause: S for tick phase, M each for vol/pan and variant seed.
- The scenarios were not individually verified to raise the intended sound
  (e.g. gen-aud-player-chat-event, ui-panels key I/C): read the voice list
  per check before trusting a row's DIVERGED as being about that rule.

## Harness notes
- `check_gen.py --selftest` fails in the netc2s step with or without these
  changes (its args omit the client-messages file); the aud asserts pass.
- Adding `audio` to suite.py would be a real change (cache, workers), not done.
