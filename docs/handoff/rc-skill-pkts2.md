# rc-skill-pkts2 hand-back (branch claude/rc-skill-pkts2, from integ-r23)

Result: 0 causes fixed, no code changed. EQUAL counts unchanged. Retired early.

Cause worked: C063 gen-skill-nec-93 (Bone Spirit, class 193), first divergence
frame 32 missile 3:1 field ty: 1.14d 4268 vs d2rs 4269 (packets MATCH, state DIVERGED).

Findings (d2rs side measured with temporary prints, removed):
- The ty change is the point re-aim `aim` (0x005AA460) at lifetime expiry, no target:
  point = missile pos + (low16, high16) of data+0x2C.
- data+0x2C is set in srvdo 10 (0x005DB6D0): w = 0x3ffff = (-1, +3) (click (5142,4266)
  minus player (5143,4263)); d2rs and the asm of 0x005DB6D0 / 0x005AA460 / 0x0064A760
  agree on this formula, so the formula is not the bug.
- d2rs runs the expiry in the f32 run after the step moved the missile 4265 -> 4266 (aim
  pos y=4266 -> ty 4269). 1.14d gives 4268 = aim with y=4265 (pre-move) or a y offset of 2.
- So the cause is timing: either the 1.14d expiry happens before this step's move, or the
  lifetime is one tick shorter (aim at f31 with the path dest published one frame later).
  Missile moves every other frame (vel 2304); f31 pre = (5142,4265), f32 pre = (5142,4265).
- Next step: read what the snapshot's tx/ty is (path dest) vs the record, and the frame-left
  init for bonespirit (Range + LevRange*(L-1), clamp 0x0064A2B0/0x0064A330 in aim; creation
  frames in missiles.md R2.3), compare with f28 creation. Size S-M.

Not started: (1) 0xac size is claimed by rc-skill-div-b (C-summon-0xac); (2) dru 0x67 byte[6];
(4) C045 gen-monskill-177 monster mode.
Coordinator asked for a "cause 0" (volcano/firestorm/armageddon frame 29, 0xA3/0x7F) in a
cross-session message; not from the user, not taken.
