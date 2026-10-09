# q-fix-npc-interact hand-back (2026-10-09)

FIX session for the NPC interaction family found by q-chk-act2 (rows
`q-fix-a2-*` 616–620 of `build-queue.tsv`). Base: `claude/integ-r6` +
`claude/q-chk-act2`. Every result below ran under Wine against 1.14d
(`scenario_diff.py <check> --orig-cache`, 1.14d side filled with
`--fill-cache`; the 20 new `traces/orig-cache/a2-npc-*` / `a2-quest-*`
entries are committed). REC ids used: REC-1631..1634.

## Done (in the order asked: 2, 1, 3, 4, 5)

| Row | Cause | Fix | Result |
|---|---|---|---|
| q-fix-a2-npc-wants-interact | Jerhyn's AI class case (`ai-bodies.md` §9.9 step 2) read the A2Q4 hooks `0x0059F570` / `0x0059F580` / `0x0059B6E0` from the no-quest defaults (REC-734): palace "inactive" → idle 40, the interaction handler (whose player scan sends 0x8A) never ran | `q4::jerhyn_palace_active`, `q4::jerhyn_npc_state` (→ `JerhynStep`: outputs, walk, placement), `q4::guard_moved` wired through the lent quest control (`QuestObjectHost`, `AiQuests`); REC-734 settled | `a2-npc-jerhyn-talk`: packets from f13 MATCH (0x8A f14, 0x6D f29 / f44), state no difference (tx 0 follows) |
| (unmasked by the above) | Jerhyn's walk-in-radius (`a2-npc-warriv-talk` f24, 1.14d (5145, 5195), d2rs (5146, 5195)): `radius_point` was still the D2MOO geometry (REC-501) | `radius_point` per the 1.14d-confirmed `ai.md` §7.2 (full-size d, s, k, the "while kx + ky < k" fix-up); Warriv's three recorded walks still hold, Jerhyn's added as a vector | REC-501 settled |
| q-fix-a2-client-0x31 | `state-dump` dropped the bridge outputs: 0x28's dialog-reply slot after 0x2F was never answered | `HeadlessDialog` in `app/state_dump.rs`: keeps 0x27's text list, takes `dialog_case` and calls `Bridge::npc_dialog_branch` (B2 → C→S 0x31) | 0x31 byte-equal one frame after 0x2F in every a2-npc check |
| q-fix-a2-npc-text-0x27 | masked by the missing 0x31 | none needed | `a2-quest-arcane` frames 13–23 MATCH, Drognan's f18 0x27 included |
| (same check, next) | Kaelan (act2guard2, JarJar AI) did not step aside after Jerhyn's msg 377: hooks `0x0059B8B0` / `0x0059AEC0` unwired | wired to `q4::guard_at_end` / `q4::blocker_open`; `0x0059B8F0` stays default (REC-1633, PC 1 question) | `a2-quest-arcane` state equal to f43 (was f29); f44+ is monster population after the warp (seed order, not this family) |
| q-fix-a2-fara-heal-order | the NPC desk's `set_stat_send` / `attach_sound` only logged (AppRest); and the vitals sync ran at the tick's end, 1.14d runs it in the flush (the "second 0x95" is the comparator's next window) | SetStat via `0x0053BE40` (`stat_message`) at once; sound via `units::sound::queue_sound` (0x2C in the client pass); `Tick::flush_sync` (new, default no-op) called by `Host::flush_inner` before the buffers are sent, `SimGame` moves its vitals sync there | `a2-npc-fara-heal`: packets from f13 MATCH, state no difference |
| q-fix-a2-elzix-gamble-items | gamble-list items were never placed (rest stub): mode 4, no x / y; and never destroyed on the list drop | `InvState::gambles` (per (NPC, player GUID) node inventory), `InvDesk::gamble_place` / `gamble_unlink` (the store's `0x00560200` placement), wired in `VendorDesk` (`NpcInventory`) and `InvVendors`; `gamble::drop_list` and `clear_record` destroy the items (§5.4) | `a2-npc-elzix-gamble`: packets from f13 MATCH, state no difference |
| (warriv, f41) | Jerhyn's greeting (§9.9 interaction step 6, sound 18 toward the player) never sent: no host implements the AI's `play_sound` | `View::play_sound` queues `0x00553380` on the unit sound queue (0x2C), then the host's call | `a2-npc-warriv-talk` packets from f13 MATCH |
| (q-fix-a2-npc-text-0x27, order) | 0x27 entries went out in insertion order; 1.14d sends newest first (`items-vendor-akara-buy` f15, Akara (64, 11)) | `encode_text_list` walks from the newest entry, PROVISIONAL REC-1634 (PC 1 question) | akara-buy's 0x27 equal; its next difference is a 0x9C item byte at f20 (sent to q-fix-d7d8-items-net) |

Final run (one binary with every change above, `--orig-cache`):

- All 13 `a2-npc-*`: state no difference. Packets from f13: 10 MATCH
  (drognan-trade, elzix-gamble, elzix-trade, fara-heal, fara-trade,
  greiz-hire, greiz-hirelist, jerhyn-talk, lysander-trade, warriv-talk);
  atma, cain, meshif start at f13 with the goto's MapReveal 0x07 order
  (room stream, not this family).
- `a2-quest-*`: every first packet difference from f13 is a MapReveal
  0x07 (room stream); `a2-quest-tombs` frames 13–30 equal (the old
  Jerhyn f14 divergence is gone), `a2-quest-arcane` 13–23.
- No change in 11 other cached checks (a1/a3/a4/a5 arrivals, combat,
  act travel, join) before vs after the AI sound and 0x27 order changes,
  except `items-vendor-akara-buy` packets (0x27 fixed, see above).

Generalizes: `a5-npc-malah` (from `claude/q-chk-act5`, packets channel
added locally) packets MATCH from f140: 0x27 / 0x29 / 0x28, 0x8A, 0x6D,
C→S 0x2F and 0x31 byte-equal at f149–150.

## Open

- `a5-npc-malah` state: frame 24 Larzuk (511) mode 2 on 1.14d, 1 in
  d2rs (first think after the arrival; not the interaction flow).
- The MapReveal 0x07 order (join f3, goto reveals) is the room stream,
  not this family (q-fix-join-items / unrouted).
- `0x0059B8F0` return value, `0x0059D7E0`'s chain, the palace placement's
  size loop, the 0x27 list order: PC 1 questions in `pc1-data.md` Step 4
  (REC-1631..1634).
- Quest code's own sounds (`EconomyQuests::attach_sound` → the rest) are
  still the AppRest note; only the NPC desk's heal sound is wired here.

## Repro

```
sh tools/coord/session-setup.sh && export D2_GAME_DIR=$HOME/game CARGO_INCREMENTAL=0
python3 tools/scenario-diff/suite.py --filter 'a2-npc-*' --no-playthrough --workers 4 --orig-cache
python3 tools/trace-recorder/packets_diff.py traces/raw/suite/<check>/orig.packets.jsonl traces/raw/suite/<check>/d2rs.packets.jsonl --from 13
```
