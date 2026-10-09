# q-tool-interact-pokes: hand-back (2026-10-09)

Task: `docs/handoff/q-tool-interact-pokes-task.md` (fidelity gaps §4,
`interact-pokes`). Branch `claude/q-tool-interact-pokes`. No REC ids used:
every 1.14d address comes from specs, nothing is provisional.

## Done

- **Spec** `specs/tools/poke.md`: rows `operate <ref>` and `talk <ref>
  [<choice>...]` (§1), §4 rule 10 (1.14d: the C→S dispatcher `0x0054D750`,
  ECX game, EDX player, [ESP+4] message, [ESP+8] size, EAX the result code;
  `sim/intents-events.md` §2.3), §5 rule 4 (d2rs), §5 rule 5 (the tick-end
  point), Edge case 5, Constants, Test vectors with the live runs.
  Choices: `trade` / `gamble` / `hire` / `action:<n>` → 0x38, `quest:<n>` →
  0x31, `close` → 0x30; `talk` always starts with 0x13 then 0x2F. The first
  call whose result is not 0 ends the directive (`failed` "0x2f returned 1").
- **1.14d** `tools/trace-recorder/poke.py`: `CALL_FORMS["dispatch"]`, parser,
  canonical text, `_interact` (bytes at scratch +0x300 plus 4 zero bytes; the
  record carries `codes` and `bytes`). `--selftest` 881+ checks green.
- **d2rs** `d2_sim::poke`: `Directive::{Operate, Talk}`, `TalkChoice`,
  `interact_calls` / `interact_calls_with` (no transport knowledge: ids and
  values in layout order). `d2_server::host::Host::dispatch_now` (the
  dispatcher called now, outside the queues) and `Host::frame_with` (a hook
  after the tick, before the flush). `d2-client`: `apply_on_host` /
  `interact_on_host`, `LocalLink::set_tick_end`, `state-dump` runs every poke
  of a frame holding an `operate` / `talk` at the tick end, after taking that
  frame's snapshot there (as `record_state.py`), and before the `tick_end`
  packet record (as `record_packets.py`).
- **Tests**: d2-sim (round-trip, malformed lines, call order), d2-client
  (bytes, the tick-end split), test-fixtures `a_poked_operate_is_the_clients_0x13`
  (a poked operate = the queued 0x13: same code, chest state, control seed,
  S→C). Full `nextest` of d2-sim, d2-server, d2-client, test-fixtures: 7731
  passed.
- **Live checks** (cloud, Wine, ScnAma seed 1234), `traces/checks/`:

| Check | 1.14d | d2rs | Verdict |
|---|---|---|---|
| `interact-operate-waypoint` | `ok` GUID 10, codes [0] | `ok` GUID 10 | state no difference (PARTIAL: snapshot gaps only), **packets MATCH** |
| `interact-talk-akara` | `ok` GUID 12, codes [0,0,0] | `ok` GUID 12 | state no difference; packets first difference frame 15: S→C 0x27 text list order (below) |
| `interact-operate-stash` | `ok` GUID 17; runs to the stash, 0x77 at frame 13 | `ok` GUID 17; the player never moves | DIVERGED frame 4 (player mode 3 vs 5): d2rs object walk gap (below) |

  The first Akara / waypoint runs showed the tool's own divergence: d2rs ran
  pokes after the frame's flush, 1.14d's hook is before it, so handler
  replies left one frame later. Fixed (§5 rule 5); waypoint packets now MATCH.

## Open (routed, not mine)

1. **d2rs 0x13 object case never walks** (`world/objects.md` §7.3 rule 4,
   `0x00548A50`): `View::object_message` uses the play preview's reach
   (`object_preview_range`, d2rs-own) and returns 0 out of range; the server
   walk and operate-on-arrival are not implemented. Same with the 0x13 sent
   through the client queue (`state-dump --send '4 InteractWithEntity type=2
   id=@2:267'`). Owner: objects (no row in `tools/coord/owners.tsv`) → the
   coordinator.
2. **S→C 0x27 NpcInfo text list order** (Akara, frame 15): bytes 10 and 14
   swapped (1.14d `40 … 0b`, d2rs `0b … 40`; `world/quests.md` §7.1). Owner:
   quests / NPC interact (claude/q-fix-pc1-day3-a-r2). Then the store
   messages: 1.14d frame 16, d2rs frame 15 together with the chat open
   (vendors, claude/q-fix-server-store-fill) — after the first difference,
   so not yet settled.
3. `play --poke` still runs every poke between frames (its `operate` / `talk`
   replies reach the client a frame late); a play aid, documented in §5 r5.
4. `@wp` stays a gap on 1.14d (poke.py does not read the objects table):
   checks name the waypoint class (`@2:119`).
5. Setup note: `session-setup.sh` run from `/tmp` computes its root as `/`
   (wine, winpy and saves steps fail); run it from inside the repo.

## Ledger

`docs/handoff/ledger/q-tool-interact-pokes.tsv`, 9 rows:
`object.operate.23.waypoint`, `object.operate.32.bank`, `npc.akara`,
`net.c2s.0x13`, `net.c2s.0x2f`, `net.c2s.0x38`, `net.s2c.0x63`,
`net.s2c.0x27`, `net.s2c.0x77`. `ledger.py` warns on two rows (`npc.akara`,
`net.s2c.0x27`: DIVERGED@15 here, @16 in `checks-status.md`, which does not
know the new check yet).

## Repro

```
python3 tools/trace-recorder/poke.py --selftest
cargo nextest run -p d2-sim -E 'test(poke)'
cargo nextest run -p d2-client -E 'test(poke)'
cargo nextest run -p test-fixtures --test e2e_night_flows -E 'test(poked)'
D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py traces/checks/interact-operate-waypoint.check
D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py traces/checks/interact-talk-akara.check
D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py traces/checks/interact-operate-stash.check
```
