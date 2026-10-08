# q-flaky-transport: `prop_transport::duplicate_filter_any`

Reported by q-move-anims: failed once in a full nextest run, passed 3/3 on rerun.

## Root cause

**A wrong oracle in the test.** The transport was not at fault.

`DuplicateFilter` (`crates/d2-server/src/transport.rs`) follows
`specs/sim/intents-events.md` §2.1 rule 1 exactly. The sender at
`0x00478350` compares the new message, over its own size, with a
0x200-byte store (`0x007BB3B8`) and a time (`0x007BB5B8`). Both start
zeroed. A send overwrites the store only over its own length, so bytes
past a shorter message stay behind from earlier ones.

The property's oracle modelled the store as "the last sent message"
(`last.starts_with(m)`, with `last = None` at the start). Two correct
drops broke that model:

- **Zeroed start.** An all-zero message (`[0x00]`, id 0, window 200 ms)
  sent before 200 ms matches the zeroed store, so the filter drops it
  with nothing sent yet. The oracle then panics on
  `expect("a filtered message repeats one")`.
- **Leftover tail.** Send `20 01 02 03`, then `21`. The store is now
  `21 01 02 03`, so `21 01 02` sent within 200 ms is a repeat. No sent
  message starts with it, so the oracle rejects the drop.

## Reproduction

The run is deterministic with a fixed seed. Copy the old test (HEAD
before this branch) and run:

    PROPTEST_RNG_SEED={1,2,3} PROPTEST_CASES=150000 cargo test --release -p d2-server --test <old copy> duplicate_filter_any

| Seed | Cases before the failure |
|---|---|
| 1 | 60018 |
| 2 | 38846 |
| 3 | 23117 |

All three shrink to `sends = [([0x00], 0)]`. That is about one failing
case in 40k, which explains a rare failure at the default 256 cases per
run. The seed is random per run because `failure_persistence` is None.
Timing, temp dirs, ports, ordering and HashMap iteration are not
involved: the test is pure and in-memory.

## Fix (test only, property made stricter)

- `FilterModel` in `crates/d2-server/tests/prop_transport.rs` models the
  spec independently of `d2_server::transport`: a zeroed 0x200-byte store
  overwritten only over each message's length, the start time, and the
  window table (50 ms for 0x05–0x0A and 0x0C–0x11, never for 0x3A,
  200 ms for everything else).
- `duplicate_filter_any` now checks the **exact** pass/drop result
  against that model on every send. The old oracle only checked a
  necessary condition on drops, and checked nothing on passes.
- Two new regression tests:
  - `duplicate_filter_zeroed_store` starts with the shrunk input.
  - `duplicate_filter_store_tail` covers the leftover tail.

The new property passed 300 000 cases in release.

## PROVISIONAL / REC

None. The spec is explicit and the code matches it, so REC-285 was not
used.

## Local check

None needed: the change does not need game files.
`cargo nextest run -p d2-server -E 'test(duplicate_filter)'` should pass.
