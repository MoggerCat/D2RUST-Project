# rc-player-hit — hand-back (2026-10-10)

Branch `claude/rc-player-hit` (from `claude/integ-r10`). REC-1835, 1836.

## Cause
The player *was* hit (life equal per frame in `combat-fallen-hits-player`),
but 1.14d's hits are soft (no GH), and a soft hit reaches the client only
as S→C 0x0D code 0x13, which d2rs never sent; the client's impact and
get-hit sounds hang on it (`client/model.md` §8 r4 → `0x004CC5B0(U, 0x13, 1)`).
1. `intents-events.md` §7.3 r1 step 4 (flag 0x8000 → 0x0D) added
   (`wiring/path/walk.rs` `soft_hit_message`, from `dispatch.rs`).
2. `damage.md` §7.1 step 2 (element hit class, D +0xB0 := R +0x60) was a
   no-op seam: +0xB0 is now `UnitRecord::hit_class`; `Pending::unit_b0`
   removed (mode messages, 0x0C, stop rows read the record). Monster
   `0x00623C20` = monstats2 `HitClass` (REC-1836, read in the 1.14d export,
   written into `damage.md` §5.1 step 4.4; Fallen 3 = 1.14d's byte).
3. Client: the audio feed runs the explicit 0x13 mode sound on a new
   request (REC-1835 PROVISIONAL, `model.md` §8); +0xB0 reaches the unit
   sound record; the monster model stores +0xB0 for 0x06 / 0x13 / 0x14.

## Checks (EQUAL ticks, orig-cache)
| check | before | after |
|---|---|---|
| combat cluster (17 checks, 19 runs) | 2722 | 2726 (potion-midfight packets 127 → 131) |
| combat-fallen-hits-player as packets (probe) | 180/200 | 185/200, five 0x0D byte-equal (frames 77, 98, 124, 137, 173) |
| audio-monster-hit-ama voices (1.14d 36) | 28 | 34: sword*.wav + get-hit voice at every hit |
No check lost a tick.

## Open
- Audio ticks +1 (T 76 vs 75 …): the whole re-capture is +1 (town1, thrust). (S)
- First hit: 1.14d skips the voice (entry line T 64 still speaking); d2rs
  plays no entry line in this run (audio environment §4). (S)
- 0x95 life sync of the hit: 1.14d frame 77, d2rs 80. (M)
- Variant picks: shared client seed (q-fix-audio item 1).
- Player branch of `0x00623C20` (weapon item hit class, 1 bare) still the
  `Pending::weapon_hit_class` seam (default 0). (S)
- Player `fc` 256 vs 0 after a hit (rc-difficulty, gen-boss-*,
  combat-pop-cold-plains frame 251): unchanged here, another path. (M)
- d2-client integration tests run in batches (111 binaries don't fit disk).
