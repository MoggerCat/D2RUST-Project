# rc-succubus-m hand-back (2026-10-10)

Checks (gen-mon-469..473, 634, 635, 719): before 0/8 EQUAL (first divergence
frame 122: player m 4 vs 5, missing draw at 0x57DB38). After 8/8: state 150/150
(PARTIAL: q, seed ignored by the check), rng MATCH.

## Root cause (one)
`mode_at` (`0x005DDF90`, plain A1/A2) left the used skill of the previous skill
mode (the curse, S2) set. Event 0 `0x005A7670` then took its used-skill branch
(do + animation refresh, no melee strike), so the melee to-hit `0x0057D9B0`
(site 0x57DB38) never ran. 1.14d: the request builder `0x005A7E60` clears the
used skill, so a plain attack has none.

## Changed
- `monsters/ai/tactics.rs`: `mode_at` calls `clear_current_skill` first.
- New seam `clear_current_skill` (`ai/seams.rs`, `wiring/action/pending.rs` and
  `ai.rs`, client `single_player.rs` -> `monsters.clear_current`).
- `specs/monsters/ai.md` §7.1: note with the address (REC-2325).
- Ledger part `docs/handoff/ledger/rc-succubus-m.tsv`.

## Open
- Other plain requests built through `request_mode` without `use_skill`
  (not `mode_at`) do not clear the used skill; not exercised by a check yet.
- `d2-sim` nextest 4770 pass; clippy clean for d2-sim, d2-client.
