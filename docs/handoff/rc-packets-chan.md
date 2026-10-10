# rc-packets-chan hand-back
EQUAL 2023 -> 2026 (87 rows promoted under REC-2055: state equal, every channel incl. packets MATCH).
Changes: check-gen adds `packets` to gen-skill (210) and gen-obj (571, not yet run); packets added to 164 hand-written
input checks behind PARTIAL rows (traces/checks/*.check); d2-server: a full-save join now sends the post-load
0x23 selections (left hand 1, then right hand 0, owner -1) right after 0x94 (d2s.md §2.4 rule 6.2-6.3).
That one cause was 200/210 of the gen-skill packets first differences; gen-skill packets now MATCH 91/210.
Open first-difference causes (counts over gen-skill + hand-written re-run):
- ~60: s2c 0xA8 SetState at frame 2 #71-73, byte 9 (1.14d 61/135 vs d2rs 1): state-buff bytes after join (class skills with an aura/buff on load)
- 14: c2s 0x0C (d2rs sends an extra at frame 20; rclick intent) 
- 10: 0xA8 size 14 vs 17; 7+4: 0xA7 missing / 0x5D vs 0x07 at frame 4; 6: 0x6B bytes[6]
- state PARTIAL with differences or an ignore line: 10 rows
Not done: gen-obj (571 checks, 42 object.operate rows), system.sim, monster.superunique (not run; time).
sys-statpoints/sys-pets-skeletons: rng/packets ERROR (suite-bin binary missing for that channel), rows left unchanged.
Ledger part: docs/handoff/ledger/rc-packets-chan.tsv (188 rows; 9 DIVERGED@ rows read PARTIAL per ledger.py reconcile).
