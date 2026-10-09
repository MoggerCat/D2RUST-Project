# q-ledger-world

Done: `docs/handoff/ledger/world.tsv` (385 rows: 132 levels, 39 waypoints, 92 object operate/populate/preset groups, 23 shrines, 43 NPCs, 41 quests, 4 hireling groups, 11 systems/drlg groups).
States: NO-CHECK 359, DIVERGED 26, EQUAL 0, UNKNOWN 0.

Open / limits: `exercised` is `?` (coverage sessions fill it); `provisional` is 0 (not matched per row); `owner` is `-` for rows owners.tsv does not route; last_verdict is the worst channel of the listed checks from checks-status.md (17:34 UTC). No game logic changed. Level->check mapping is by arrival warp name only.

Repro: `python3 tools/coord/ledger_world_gen.py` (needs the private repo clone at /home/user/d2rust-private-repo with extracted/d2exp.mpq/data/global/excel, and checks-status.md saved to /tmp/cs.md).
