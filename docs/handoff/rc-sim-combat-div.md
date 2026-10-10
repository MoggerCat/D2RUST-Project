# rc-sim-combat-div hand-back

Branch `claude/rc-sim-combat-div` (from integ-r23). REC ids not used.
Rows: system.sim.* / system.combat.* / other non-ui system.* DIVERGED.
Ledger part: `docs/handoff/ledger/rc-sim-combat-div.tsv`. EQUAL 2759 -> 3462 on
the merged branch (DIVERGED 1021 -> 642; part of that is parallel work on integ).

## Causes fixed (one coordinator message each)
1. `ignore q` dropped from sys-tick-idle-a2/a5, sys-units-census (q now equal):
   system.sim.tick.*, system.sim.units.*, system.flows.server-tick.* (20 rows).
   The 1.14d side of these checks needs a re-record (check text changed; verified
   here on the cached recording with `--reuse-orig`, 0 differences).
2. Join SetSkill (0x23): `session.rs` passed `hand` as the unit type; the load
   selection was sent without an existing (skill, owner -1) entry
   (`d2s.md` 2.4 r6.3). rng-town-idle-sor and sys-pets-skeletons packets MATCH:
   system.sim.rng.*, unit-order.*, pets.* (27 rows).
3. Vendor store recharge was a stub (store wands showed charges below max):
   `VendorDesk::recharge`; talk/trade/waypoint msg-ui rows (8).
4. Act change: 0x5D before the room removals (parallel fix on integ) and the
   queue-walk marks 0xFE/0xFD leaking into the poke tick-end drain
   (`queue_sent_now`): system.flows.act-change.*, msg-ui.20.
5. C->S 0x59 MakeEntityMove (`0x0054CA10`, read in the export) was unimplemented:
   `NpcControl::make_entity_move` (spec: ai-bodies.md 9.9). Replay
   `town-ama-10k` is now equal over 10,000 frames (412,326 unit records).
   system.act.travel EQUAL by the REC-2055/2056 rule (promote.py --list).
6. Replay `bloodmoor-bar-10k`: first divergence frame 5 -> 1461. Fixed the
   0x06 melee range (`0x00622C40` with the target-moving argument `0x00622DC0`;
   `moving_mode` `0x00622D00` was a stub = false everywhere, which also feeds
   the block/3 rule of hit.md 6.1) and the 0x16 pick-up distance (`0x00641530`
   with player size 2 / item size 1; it was a max-axis d2rs-own value).
   Combat checks (17) and items-ground still MATCH/PARTIAL.

## Open
- `system.replay.bloodmoor-bar-10k` (L): frame 1461, a population spawn (guids 22,
  24, 25): hit points differ (quill rat 5 vs 2, fallen 12 vs 9 and 9 vs 10) while
  every unit seed is equal. d2rs rolls hp with the second draw after the init
  seed; no single draw/range of the two reproduces 1.14d's three values. Needs
  `0x00573CB0` / the population caller read (draw order), or an rng trace.
- `system.seams.messages.warp-0x07-frame` (rc-map-reveal's), `system.seams.
  world-screen.*` and `system.client.*` draws (row 173 CelDraw): rendering, not mine.
- Akara/Halbu/Drognan stock checks: 1.14d sends the store 0x9C before the
  NPC's own 0x67 in the same frame (items sit in the room update queue before
  the NPC that moves later); d2rs sends them after the tick's unit messages.
- integ-r23 note: release `cargo test -p d2-sim --lib` crashes rustc (SIGSEGV)
  on this container; debug builds pass.
