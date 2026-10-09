# PC 1 day 3, session B — hand-back (branch `claude/local-pc1-day3-b`)

REC block REC-815..829. Items from the PC1-B prompt (2026-10-09).

## REC-680 — Blood Golem life share (answered)

- `0x005C6870` (ECX game, EDX U, stack v; `ret 4`): U's first type-3 pet
  (`0x00574EC0(game, U, 3, 0)`), if a monster of class 290, takes
  x = p·v / 100 of a life amount, where p = `Param6` of monstats 290's
  `Skill1` (BloodGolem, 25). Then v −= Heal(golem, x) (`0x005C5F10`:
  capped at the golem's max, returns what was applied). It applies to
  the whole potion amount before the stat-74 list spreads it; callers
  are use entries 3 / 4 / 5.
- Specs: `items/use.md` §3.1, `combat/events.md` §1 "Heal".
- d2rs differs: new row `q-fix-b680-golem-potion-share`.

## REC-665 — mode request to the unit's own position (answered)

- A mode-2 request at U's own cell is still made. The path compute
  (`0x00649970`) hits start = target for type 13 and computes no path,
  so the path has 0 points. It retries as type 15 (flags |= 1, unit
  queued) with the same result.
- The walk start `0x005A7520` returns 0, so the unit goes to neutral
  (`0x005A73E0`, mode 1) and never enters walk.
- Next think at f + aidel (+45 with state 21). One S→C 0x67 code 7 is
  sent; no 0x68 and no RNG draws.
- Spec: `monsters/ai.md` §7.5 rule 8 ("Zero-length walk"), §7.2 row
  `0x005DE6D0`.
- d2rs differs (`walk_in_radius` makes no request): new row
  `q-fix-b665-zero-walk`, which depends on `q-fix-p3-walk-in-radius`.

## REC-670 — local input path for mode 18 (answered)

- The local client starts mode 18 itself at the click.
  - `0x00481030`: gate `0x00480BA0`; point codes → mode request 0x15,
    unit codes → 0x16.
  - `0x00480C10` → `0x004C6EB0` / `0x004C6F40` → `0x004C6140`: mode 18,
    frame 0.
  - Only then does stat 0x148 go up by 1 and the C→S send happen.
- The server sends its own client no 0x4C / 0x4D (`0x00548090` skips the
  own client unless E flags bit 0x4, dodge / avoid).
- First machine step: the first client update after the click. Other
  units are unchanged (the queued request loads frame 0, then the next
  update steps).
- Spec: `skills/sequences.md` §3 "Local player".
- d2rs differs: it takes the local mode 18 from the server echo, one or
  more updates late. New row `q-fix-b670-local-skill-start`.

## REC-671 — whirl end rule (answered)

- Count client updates from the request update (update 0). The last
  partial path step is at a = 2 + ceil((d << 16) / (walk << 12)). Mode 18
  lasts N = max(8, 4·ceil(a/4)) updates.
- Arrival (`0x004C9320`) only clears the E flags and stops the sound; it
  does not end the mode. The only end is machine step 5 of `0x00463390`
  (`0x006217C0`: +0x48 < 1 after the advance `0x00623E00`), which falls
  on every 4th update.
- Fits both recorded whirls: d = 8 → 24 and d ≈ 14 → 40. Both arrivals
  fall on a multiple of 4, so the current d2rs fit also gives 24 / 40.
- Spec: `skills/sequences.md` §3 Whirlwind r1–r3.
- d2rs: `whirl_updates` must return N − 3 (not a − 3); 5 when walk ≤ 0.
  New expected values: (9, 6) → 25, (1, 6) → 5, (6, 6) → 17. Row
  `q-fix-b-whirl-end-rule`, which replaces q-fix-p4-whirl-end's "check
  first".
- Still PROVISIONAL:
  - REC-815: why the path first steps on update 3 (updates 1–2 do not
    step).
  - REC-816: whether update N's mode change is machine step 5 or a
    queued server mode message. N is the same either way; a whirl whose
    arrival is not on a multiple of 4 would decide it.

## Item 41 — skill button state `0x004A8D30` (answered, round 2)

- `0x004A8D30(P, skill)` runs the use check `0x004D9FC0(P, skill)` → u.
  The state is 0 when u = 0 (usable), 4 when u = 6 (aura), and 1 for any
  other u.
- The caller `0x00496BE0` (`0x00496C24`–`0x00496C42`) forces state 1
  when the skills.txt row lacks `InTown` (byte +5 & `[0x006CE268]`, mask
  1) and P's room is a town.
- Spec: `ui/control-panel.md` §7 r2 (REC-720 settled).
- d2rs differs (it draws the plain cel): new row
  `q-fix-b41-skill-button-state`.

## Left / next

- Pending: any new Step 4 items tagged `[combat-a1]`, `[skills-1]` or
  `[skills-2]`, or about damage, monster AI, missiles or skills (none
  existed at the start of this session).
