# rc-mon-s hand-back (2026-10-10, REC-2240)

Task: gen-mon checks whose first divergence is the unit seed `s`.

## Root cause (one)
`AiActs::has_list_flag` and `max_mana` for the AI host were stubs
(`Pending::ai_has_list_flag` always false, `ai_max_mana` 0). Succubus
(`AITHINK_Fn118` 0x005E1E00) checks "target cursed" (`0x00625760(T,0x20)`)
before its P(aip3) draw. 1.14d: gen-mon-469 frame 46 casts the curse, so
at frame 69 step 1 is skipped (one draw, site 0x5E20AC); d2rs saw "not
cursed" and drew twice.

## Changed
- `wiring/action/ai.rs`: has_list_flag = unit list extended + active chain
  of `0x006256E0`; max_mana from the stat lists. Stubs removed from
  `wiring/action/pending.rs`.
- Spec `monsters/ai-bodies-5.md` §4: cursed is live state.
- Ledger part `docs/handoff/ledger/rc-mon-s.tsv` (8 rows).

## Checks (gen-mon-469..473, 634, 635, 719; `--no-playthrough`)
- first divergence on `s` at frame 69: 8 before -> 0 after.
- EQUAL (MATCH) checks: 0 -> 0; each moved to frame 120-122 (state field
  `m` of the player 1.14d 4 vs d2rs 5; rng: a draw at 0x57DB38 on unit 1:8
  missing in d2rs). Frames equal 68 -> 119-121 of 150.
- d2-sim nextest 4768 pass; clippy clean.
- (The brief said 9 checks; the baseline run of gen-mon-* shows 8 on `s`.)

## Open
- Frame ~120 next divergence (player mode 4 vs 5 plus the 0x57DB38 draw
  from the succubus): not read. Size: 8 rows, one cause likely. Other owner.
