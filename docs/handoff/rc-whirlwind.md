# rc-whirlwind hand-back

Checks (Wine run 2026-10-10, state channel):
- bar-whirlwind: DIVERGED@20 (sp 304 vs 256) -> PARTIAL (no difference over 70 frames).
- bar-whirlwind-unit (new, right-click ON the cow, 4 sub-tiles away, 110 ticks): DIVERGED@20.

Changed (d2-sim wiring): `0x00623F50` velocity-mode test (`0x006214A0`) read the used skill entry's flags
(bit 0, set by Whirlwind's start `0x005D8F50`) from the host seam; the skill list owns them
(`ActionHooks::entry_flags_of`). The velocity half (pathing.md §8.1) now runs for players too, at mode
set and in the rate refresh. d2-sim nextest 4749 passed; clippy clean.

Open:
- bar-whirlwind-unit: 1.14d casts at once (mode 18, mana spent at frame 20); d2rs walks toward the cow
  (mode 2, mp unchanged). The unit-form out-of-range start path is not taken (M). Not yet read in Ghidra.
- Player `fr` after a Swing and a dead monster's `d`: not reached (no check isolates them yet).
- Not run: monster/walk regression checks after the refresh change, coverage/spec_index/ledger --check,
  spec text for pathing.md §8.1 / units.md §4.7 (flags from the skill list).
