# Handoff: run_scenario.py probes on 1.14d (lane T, `claude/local-buddy-scenarios`)

Local run, 2026-10-07, user's reference `Game.exe` (sha256 `631066c1…adaaf`,
checked by the tool), save `bdAma` (read only, `-nosave`). Base:
`origin/claude/tool-run-scenario` (63ae730) merged with
`origin/claude/scenario-harness`. Spec: `specs/tools/original-hooks.md`
(`origin/claude/specs-staging`, not edited here).

**Order.** The PC 1 "final report" with the probe order is not in the
repo; the order used is the one in the `run_scenario.py` docstring and
README: `start`, `inject`, `seed`, `savepath`, `ready`, `unit_fields`
(the `PROBES` dict lists them in another order; no probe depends on
another). `--selftest` first: `selftest ok` (also after every edit).

## Results

| Probe | Spec | Result | Key output |
|---|---|---|---|
| `--selftest` | — | PASS | `selftest ok` |
| `start` | OQ2, §5.2, §5.4 | PASS (after fix 3) | `0x67: {'type': 3, 'class': 0, 'difficulty': 0, 'name': 'bdAma'}`; `RESULT OQ2 0x67 sent: True; first tick frame: 1 (want 1)`; menu ended by the tool at 6.0 s |
| `inject` | OQ1, §1, §3 rule 1 | PASS (after fixes 1, 2, 3) | `inject` record `result: 1` (queued) at tick 30; `c2s` and `dispatch` both `frame 29` (= N−1, `game_frame` 29); `result` `code: 1, dispatched: True` (see finding 2) |
| `seed` | OQ3, §2.1 | PASS (after fix 3) | two `seed_override` records per run (time 0x1234567, init 0x89ABCDEF); two runs: 3023 rng records each, same sha256 `d2fceb67306fbd10`; `RESULT OQ3 two runs identical: True` |
| `savepath` | OQ4, §5.3 | PASS | `save path read at 0x534410: C:\Users\zffit\Saved Games\Diablo II\bdAma.d2s` |
| `ready` | §1 rule 5, §3 rule 2 | PASS | `frame 1: client state 1`, `frame 2: client state 4`, `RESULT first ready frame 2: inject at tick >= 3` |
| `unit_fields` | §4 | PASS (reads) | player unit record `{'id': 1, 't': 0, 'cl': 0, 'm': 5, 'x': 5473, 'y': 4708, 'life': 12800, 'mana': 3840}`; full stat array of 25 entries incl. (0,6,12800) (0,7,12800) (0,8,3840) (0,9,3840) (0,10,21504); 12800/256 = 50 life, 3840/256 = 15 mana, 84 stamina: the values of a level 1 amazon |

The first runs of `start` and `inject` (before fix 1) FAILED with
`RuntimeError: unexpected code at 0x52fd1e` (logs in `out-scenarios\probe-start.log`,
`probe-inject.log`); `seed`, `ready`, `savepath`, `unit_fields` do not hook the
packets stream and ran.

## Tool fixes (separate commits)

1. **Tick-return hook checked twice** (`install`, original-hooks §3: tick
   return `0x0052FD1E`, bytes `8B 76 18`). The packets hook set
   (`record_packets.HOOKS`) already plants INT3 at `0x52FD1E`; the extra
   address check then read the INT3 back and refused the game. Addresses the
   first loop hooked are skipped. (Every `packets` stream run died here.)
2. **Result after a reply** (§1 rule 6: `result` at `0x0053F45E`). A walk's
   handler queues an S→C message (id 0x15, caller `0x53bc44`) between the
   `dispatch` and the `result` hook; `record_packets.py` then called the result
   "not dispatched" and dropped its code. `ScenarioRecorder.on_hook` keeps the
   dispatch open from `dispatch` to the next `c2s`/`c2s_sys`/`drain`.
   (`record_packets.py` itself keeps the old behaviour; not this task.)
3. **Probe judgments** (§1 rule 6, §2.1, §5.2): `start` recorded nothing
   before the first seed override, so the 0x67 (drained before game creation)
   was never seen; `seed` hashed the clock value the override replaces
   (`seed_override.old`) and heap addresses (`seed` pointers), so two runs
   always differed (the draws were identical: 3023 records, only those fields
   differ); `inject` demanded result 0, but the probe's fixed target (5105,
   5045) is out of range for the character and is refused with code 1
   (intents-events.md §2.3: 1 = refused), which still answers OQ1.

Also (feature, not a bug): `run_scenario.py` reads `.scenario` scripts and
writes `scenario-trace` (see `local-buddy-scenarios.md`).

## Findings for PC 1 (spec `original-hooks.md`; not edited)

1. **OQ1 expectation "result 0"** is only true for a reachable target; the
   dispatch result is a diagnostic (intents-events §2.2 rule 5). The injection
   path itself matches §1 rule 3/6 and §3 rule 1 (`dispatch.game_frame` = N−1).
2. **`record_packets.py` `result` flag**: a handler that queues S→C between
   `dispatch` and `0x0053F45E` (walk does, id 0x15) makes `dispatched` false.
   The `result` semantics in §1 rule 6 should say "the first `result` after a
   `dispatch`, replies in between allowed".
3. **OQ3 first-draw check cannot be done from the rng stream**: the step at
   `0x0052C2C6` is inline and the game seed (`seed_set` 0x52c2bf, new
   `[T, 666]`) has no recorded draw in the first 5 ticks. The scenario traces
   answer it by reading game +0xD0 (see `local-buddy-scenarios.md`: 44 steps
   (sorceress) or 46 (amazon) from `{T, 666}` before tick 0).
4. **OQ4**: path read at `0x534410` is `<Saved Games>\Diablo II\<name>.d2s`
   with the registry value present; the no-registry fallback was not exercised.
5. **OQ6** (difficulty not reached): see `local-buddy-scenarios.md`
   (champion-pack, Hell on a save that has not reached it): the client never
   gets past the first tick; the run stops only at the time limit.
6. **§1 rule 5 / §3 rule 2** confirmed: client state 1 in frame 1, 4 in frame 2
   (first usable N = 3 for a step; scenario F0 = 2).
7. §5.2 confirmed: 0x67 carries game type 3, class, difficulty, name; tick 1
   runs with no input.
