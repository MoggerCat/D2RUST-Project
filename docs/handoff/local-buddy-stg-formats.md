# local-buddy-stg-formats (lane A, 1.14d install, 2026-10-07)

Base: origin/claude/specs-staging at 06db38f (NOT main). Expected values untouched.
Logs: `C:\Users\zffit\Desktop\D2test\out-stg-formats\{sweep,c65}.log`.

## 1. game_sweep (release): 9 pass, 2 FAIL (queue expected 11), 37 s

| test | result | printed counts |
|---|---|---|
| dc6_every_file_decodes | PASS | 1,653 files, 26,317 frames, 140 flipped, term 00 x60 / CD x400 / EE x1,193 |
| dcc_every_file_decodes | PASS | 21,717 files, 271,176 directions, 3,305,132 frames |
| palettes_every_file_parses | PASS | - |
| font_tables_every_file_parses | PASS | 14 |
| animdata_real_vectors | PASS | - |
| animdata_matches_every_cof | PASS | 3,558 records, 3,529 with a .cof (3,500 distinct names), 9 non-matching copies skipped |
| expfield_layout | PASS | - |
| cof_every_live_file_parses | PASS (now) | 3,605 parse; failed only d2char.mpq amblxbow.cof (72 bytes); 3 files 42-byte padded; ambl* = ambl1hs, ambl1ht, amblhth, amblxbow |
| string_tables_every_key_resolves | PASS (now) | 29 tables (20 paths), 10 languages {chi,deu,eng,esp,fra,ita,jpn,kor,pol,por}, 63,167 used entries, 4,236 earlier-duplicate, 16,786 non-ASCII, 0 raw FF, 130 C3 BF |
| dt1_every_live_file_decodes | FAIL (finding) | 250 live files, 15,873 tiles, 6 v4; block formats {0x0001: 226,996, 0x1001: 108,905, 0x2005: 15,712}; expected 0x1001 = 110,259 (-1,354) |
| ds1_every_file_parses | FAIL (finding) | 2,372 files, versions {3:1, 8:6, 12:14, 13:36, 15:13, 16:229, 17:147, 18:1,926}; expected 2,456 |

cof and tbl now pass as expected after the staging fixes. dt1 and ds1 still fail on stale
expectations, same actuals as the tri-formats run (2,372 and 108,905); 2,456 / 110,259 were
double-counting-tool values. Findings for the owner: change expected to the printed actuals
(not done here, per the rules).

## 2. C65 (d2-sim game_drlg_tables act3_act5): 2 PASS / 0 FAIL
`act3_act5_table_values` ok, `act3_act5_placement_on_live_tables` ok: every expected value in the
C65 entry matched (leveldefs 76..78, 111/112, 110, 117; lvlprest 573/574, 530..572, 575..604, 865,
652, 653..658; derived Act III / Act V vectors). No finding for `outdoor-act3-act5.md`.
(The test already carried its `// Covers:` line.)

## 3. Covers
Eight `Intended claim` lines in `crates/d2-formats/tests/game_sweep.rs` became `// Covers:`
(dc6, dcc, cof, palette, font-tbl, tbl, two animdata). dt1 and ds1 keep "Intended claim".
`py tools/coverage.py --check`: 5,637 claims, 0 errors.
