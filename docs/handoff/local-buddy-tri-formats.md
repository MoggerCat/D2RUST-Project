# local-buddy-tri-formats (lane A, 1.14d install, 2026-10-07)

Re-run of the triage batch rows for `mpq-tool formats` and `game_sweep`
(origin/main 06a9726). No expected value was changed.

## 1. `mpq-tool formats` (fixed, case-insensitive names)

```
cof           3606    3605  0 0   (1 known unused: am\cof\amblxbow.cof, 72 bytes)
dc6           1653    1653  0 0   flip0 x26177, flip1 x140; term 00 x60, CD x400, EE x1193
ds1           2372    2372  0 0   version 18 x1926 (3 x1, 8 x6, 12 x14, 13 x36, 15 x13, 16 x229, 17 x147)
dt1            256     250  0 0   6 known unused (version 4); minor version 6 x250
              block format 0x0001 x226996, 0x1001 x108905, 0x2005 x15712
font.tbl        14      14  0 0
tbl             29      29  0 0   version 1 x29
```
tbl rows: 29 files under `data\local\LNG\{CHI,DEU,ENG(BETA),ESP,FRA,ITA,JPN,KOR,POL,POR}` plus
`lng\eng\{expansionstring,patchstring x2,string}`; 10 language folders.

## 2. `game_sweep` (7 pass, 4 fail, 45 s)

PASS: dc6_every_file_decodes (1,653 / 26,317 frames / EE 1,193 / CD 400 / 00 60),
dcc_every_file_decodes, palettes_every_file_parses, font_tables_every_file_parses,
animdata_real_vectors, animdata_matches_every_cof, expfield_layout.
FAIL (findings, expectations untouched):
- `dt1_every_live_file_decodes`: `dt1: 250 live files, 15873 tiles, version-4 [barracks, gargtrap,
  catacombs, cathedrl, court, outdoor1 (all d2data.mpq)], block formats {0001: 226996, 1001: 108905,
  2005: 15712}`. Live 250 and 6 v4 pass; expected 0x1001 = 110,259, actual 108,905 (-1,354). 226,996
  and 15,712 match.
- `ds1_every_file_parses`: 2,372 files (expected 2,456); versions {3:1, 8:6, 12:14, 13:36, 15:13,
  16:229, 17:147, 18:1926}; the version set assertion was not reached (count assert is first).
- `string_tables_every_key_resolves`: 29 tables, 10 languages {chi, deu, eng, esp, fra, ita, jpn,
  kor, pol, por}, 63,167 keys, 4,236 resolve to an earlier duplicate. Expected 33 / 11.
- `cof_every_live_file_parses`: 3,605 parse; only failure `d2char.mpq:...\am\cof\amblxbow.cof` (72
  bytes); then `amblxbw.cof` not found. Printed `ambl*` names (all in d2char.mpq only):
  `ambl1hs.cof`, `ambl1ht.cof`, `amblhth.cof`, `amblxbow.cof`.

## Triage questions settled
- ds1 (Q11): the fixed tool prints 2,372 / 1,926 at v18, equal to the sweep. Old 2,456 / 1,997
  was the double-counting tool. The ds1 expectation should become 2,372 (versions as above).
- dc6 (Q11): tool and sweep agree on 1,653 / EE 1,193 / CD 400 / 00 60 (frames 26,317 from sweep;
  flips 140).
- dt1 (Q11): tool and sweep agree on 250 live; fixed block counts are 226,996 / 108,905 / 15,712,
  so 110,259 was wrong (old double-counted tool).
- tbl (Q10): the fixed tool also finds 29 tables in 10 languages; there is no 11th language in
  1.14d. The "33 / 11" came from the case-sensitive tool. (The sweep's EXTRA_NAMES are among the 29.)
- cof (Q9): no `amblxbw.cof`; `ambl*` files are ambl1hs, ambl1ht, amblhth, amblxbow. `amblxbow.cof`
  (72 bytes, known unused) is the only `ambl*x*` file. The Amazon block/bow COF by pattern is not
  settled by this output (the `ambl*` set has no `amblbow`).

## Covers
The six passing tests with intended claims (dc6, dcc, palettes, font tables, both animdata) now carry
`// Covers:` lines; dt1, ds1, cof and tbl keep "Intended claim" until they pass.
`py tools/coverage.py --check`: 5259 claims, 0 errors.
Logs: `C:\Users\zffit\Desktop\D2test\out-tri-formats\{formats,sweep}.log`.
