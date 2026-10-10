# q-fix-skills-4cls hand-back (REC-1640..1644 used)

Input: q-chk-skills-4cls (107 Ama/Sor/Nec/Pal skill checks, 27 DIVERGED).
All runs under Wine (`scenario_diff.py <check> --orig-cache --fill-cache`,
state channel; orig-cache filled for every check run). Branch
`claude/q-fix-skills-4cls`.

## Result

25 of the 27 DIVERGED checks now compare equal for all 70 frames (PARTIAL
only for the known `own` / client gaps); 2 remain DIVERGED with their cause
found and a PC 1 question queued. 8 unrelated PARTIAL skill checks re-run
as a regression sample: unchanged. `dru-tornado` (q-fix-skills-bda) also
stays PARTIAL with the missile-snap change below.

| Cluster | Checks now PARTIAL | Cause → fix |
|---|---|---|
| 1 cast-start tx (shared) | nec-raise-skeleton, -raise-skeletal-mage, -bloodgolem, -irongolem, -firegolem, -revive, -corpse-explosion, -poison-explosion; ass-psychic-hammer, ass-dragon-flight | 1.14d's client sends no C→S: the click's use check (`ui/controls.md` §6 r9.1) refuses on mana, and the client start's `cltstfunc` refuses without its target kind (`client/model.md` §8 r7 step 5). d2-client `bridge/use_state.rs`; `mode_request` returns the request result, `click::apply` sends only on it. cltstfunc 5 read from the 1.14d export (`0x004F2010`). |
| 2 summon mode | ama-dopplezon, nec-clay-golem | `delete_timers(…, 0)` = any argument (`0x00540E60`), so the spawn's F + 25 think is first; pets with the owner link left to their real monstats AI (d2-server `hireling_drive`); `AiSummons` path final / target point and coordinate index on the live `View`; `BodyEffect::AiParams` wired (Hydra expiry); target searches skip units without unit flag 0x4 (scan 5: REC-1642). |
| 3 paladin | pal-vigor, -fanaticism, -holy-fire, -prayer (holy-freeze / -shock / sanctuary same cause), pal-charge, pal-blessed-hammer | the save load's right-skill selection runs the aura start (`d2s.md` §2.4 r6.3, `0x0056FF10`); point-path missiles reach their points (below). |
| 4 | sor-charged-bolt, sor-inferno, sor-telekinesis, nec-bone-prison, nec-bone-spirit, nec-bone-wall | one-step point snap for every missile but path type 4 (REC-1643 with staging's REC-1391); a failed start sets TN only in town, else NU (REC-1644); Inferno / Blade Fury remove callbacks run at list expiry (state off, flag 0x40); Bone Wall maker summon seams on the live missile host; skill summon spawn through population's placement (`0x005B2F20`). |

## Still DIVERGED

- `sor-hydra` frame 42: at the end of their S2 spawn mode the three hydras'
  path target becomes (0, 0) in 1.14d (every other monster mode end in the
  orig cache keeps it or sets its own position). Likely the per-class mode
  records (`0x006E22D0`…, monstats +0x1A5). PC 1 item queued.
- `ama-valkyrie` frame 51: the Valkyrie's wander step is 17200/frame in
  1.14d (path velocity 1075) vs 31536 (1971 = 11·256·70/100) in d2rs.
  `0x00623F50` / `0x00621360` (read from the export) give base = monstats
  `Velocity`·256 unless disguised, so the stat-67 total differs; needs the
  Valkyrie's stat 67 at frame 50 (PC 1 item queued).

## Provisional (REC ids)

- REC-1640 client use state tests 5, 7–10 and code 8 read as passing.
- REC-1641 cltstfunc 20/21/24 "dead monster target", 23 "item target"
  (5 is now read from the export).
- REC-1642 scan 5 skips units without unit flag 0x4.
- REC-1643 one-step snap skips only the straight missile path (type 4).
- REC-1644 failed start: TN in town, else NU.

## Notes for others

- Synthetic click fixtures need `SkillRow::ingame = true` and a native
  level ≥ 1 entry, or the use check refuses (no C→S).
- `traces/orig-cache` now holds the state (and some packets / rng) sides of
  the checks above.
- Owners: `tools/coord/owners.tsv` row "skills cast/right-click …" is this
  session's.
