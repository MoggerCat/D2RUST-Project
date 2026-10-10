# rc-player-hit — hand-back (2026-10-10)

Branch `claude/rc-player-hit` (from `claude/integ-r10`). REC-1835, 1836 used.

## Cause (one root cause, two missing pieces of the same hit)

The player *was* hit in d2rs (life equal frame by frame in
`combat-fallen-hits-player`); 1.14d's hits are soft (no GH mode), and the
client learns of a soft hit only through S→C 0x0D code 0x13, which d2rs
never sent. Its client sound (impact + get-hit voice) hangs on that
message (`client/model.md` §8 r4 code 0x13 → `0x004CC5B0(U, 0x13, 1)`).

1. Server: the player update had no `intents-events.md` §7.3 r1 step 4
   (flag 0x8000 → `0x00547F70` → 0x0D). Added (`wiring/path/walk.rs`
   `soft_hit_message`, called from `dispatch.rs`).
2. Server: `damage.md` §7.1 step 2 (element hit class, D +0xB0 := R
   +0x60) was a no-op seam; +0xB0 is now `UnitRecord::hit_class`, the
   `Pending::unit_b0` seam is gone (mode messages, 0x0C, stop rows read
   the record). Monster weapon hit class `0x00623C20` := monstats2
   `HitClass` (REC-1836, PROVISIONAL, measured: Fallen 3 = 1.14d's byte).
3. Client: the audio feed ran mode sounds only on a mode change; it now
   runs the explicit 0x13 sound on a new request (REC-1835), and +0xB0
   reaches the unit sound record. The monster model stores +0xB0 for
   0x06 / 0x13 / 0x14 (§19 r4).

## Checks (EQUAL ticks, orig-cache, same build machine)

| check | before | after |
|---|---|---|
| combat cluster (17 checks, 19 runs) | 2722 | 2726 (combat-potion-midfight packets 127 → 131) |
| combat-fallen-hits-player as packets (probe, not committed) | 180 / 200 | 185 / 200: the five 0x0D byte-equal at frames 77, 98, 124, 137, 173 |
| audio-monster-hit-ama voices (1.14d 36) | d2rs 28 | d2rs 34: sword*.wav + get-hit voice at every hit |

No check lost a tick.

## Open (not fixed here)

- Audio ticks: d2rs plays the hit sounds at T 76/97/123/136/172, 1.14d at
  75/96/122/135/171: the whole re-capture is +1 (town1 T 0 vs 1, thrust
  T 73 vs 74), a capture alignment, not this path. (S)
- First hit: 1.14d has no get-hit voice (player still speaking the Blood
  Moor entry line, T 64); d2rs does not play that line in this run. Owner:
  audio environment §4. (S)
- 0x95 life sync of the same hit: 1.14d frame 77 flush, d2rs frame 80. (M)
- Variant picks (soft4 vs hard5): shared client seed, q-fix-audio item 1.
- PC 1: `0x00623C20` read (pc1-data.md Step 4).
- d2-client tests: lib 2286 pass; integration tests run in batches
  (disk too small for all 111 test binaries at once), see the commit.
