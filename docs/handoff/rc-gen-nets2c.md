# rc-gen-nets2c hand-back

Generator: `tools/check-gen/check_gen.py --family nets2c` (table `NETS2C_GROUPS`, one check per scenario
group, several ledger areas per check as the item family does). 8 checks in `traces/checks/gen/gen-nets2c-*`.

Result (suite.py, packets channel): arrival, hotkey, overhead, ping, stat, trade, warp MATCH; leave PARTIAL
(1.14d ends the game at frame 10, window 10 frames, all equal).
Ledger EQUAL: net.s2c rows 27 -> 29 (0x05, 0x06); net.c2s unchanged (78). Part: `ledger/rc-gen-nets2c.tsv`
(2 settled rows, 9 NO-CHECK rows with the reason in `note`).

Open (sizes):
- 0x04/0x5B/0x65/0x8D carried by 1.14d but never paired: d2rs packets window excludes join-time sends (M, comparator).
- 0x20, 0x58, 0x7B, 0x8F, c2s 0x14: the trigger I tried does not make 1.14d send the id; need the real cause (S each, read sender in Ghidra).
- Not reachable, left NO-CHECK as before: unused ids (s2c 0x17,0x2B,0x2D-0x39..., c2s 0x00,0x2B,...), multiplayer-only
  (0x5C, 0x78, 0x10), state no poke makes (death 0x74, merc 0x9E-0xA2, town portal 0x60/0x82, quest 0x50/0x61, 0x89).
- Harness: parallel suites collide on `/root/.wine-d2-suite-N.tmp`; a deleted tmp prefix gave "could not load kernel32.dll" (workers 1 fixed it).
No game code changed.
