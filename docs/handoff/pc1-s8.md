# PC 1 session s8 (2026-10-08)

Branch `claude/local-pc1-s8` from `origin/claude/specs-staging-7`
(`docs/handoff/pc1-loop.md`). REC block REC-300..349; none used yet.

## Lane A — provisional points from the binary

| REC | Result | Specs changed | Code | Queue row |
|---|---|---|---|---|
| REC-177 (3) | settled statically (`0x005616A0`–`0x00561AF9`): no requirement recheck or refusal; durability untouched; moves 4→11, 5→12, 11→4, 12→5 (all four leave first); mouse-skill swap sets trade; S→C 0x97, direct 0x23s, then next update 0x9D action 0x17 per item, 0x47, 0x48, two queued 0x23 | `items/inventory-moves.md` §7.25 (new owner), `sim/intents-events.md` §9 r14 + owner table + OQ16 answered, `items/inventory.md` §5.8 pointer | `swap_key.rs` matches (1-byte 0x60); `wiring/inventory/swap.rs` + `handlers/items/moves.rs` do not (gates bypassed at `adapters/sim.rs:485`, no 0x9D 0x17, no skill trade / 0x23, fail result 1 not 3) | `q-fix-weapon-switch` |
| REC-188 | settled: 0x1D–0x1F base only; client sums item lists itself; 0xFE/0xFD pseudo states d2rs-own; set lists are parked state 165+i children, the client runs the set update `0x00663CC0` at 8 sites; no 0xA8 from set lists | `client/stat-lists.md` §2 r1.1, r5, r6, OQ2 answered, new OQ8; `items/bitstream.md` §4.6 r6 | `vitals_sync.rs:172-260` d2rs-own transport (q-item-bonus-wire removes it); `item_bits` decode fine | `q-fix-client-set-lists` |
| REC-232, REC-233, REC-173 (bin part), REC-176 (2 parts) | Whirlwind strikes on the way (`0x00580460` falls through unless step returns 2; restart at frame 3, seq 10 events 3/7; 0–2 strikes per do, radius 5 round robin); Leap landing = knockback only; Talon complete; dual claws: seq 16 `ht2` events 6/10, even → right, odd → left, only that hand's stats (`0x00535BC0`, `0x00535D10`/`0x00535E20`); Lightning Sentry 10 shots, no lifetime; Leap pattern = path +0x48; Smite shield = first usable shld at loc 4 then 5; item-type test `0x00643F80` | `skills/use.md` §2, §5.2, OQ12–13; `skills/bodies-2.md` §2.13, §2.25, §2.27 new, §3.4, §4.7; `skills/bodies.md` §8.3, §8.10 | Leap pattern matches (drop note at `skill_rooms.rs:67`); others do not | `q-fix-moving-skill-do`, `q-fix-item-type-test`, `q-fix-smite-shield`, `q-fix-dual-weapon-damage`, `q-fix-sentry-ai` |
| REC-237, REC-204, REC-211, REC-229 (static), REC-212 (verify only); REC-183/222 partly | Esc menu: DC6 label art Options/Exit/ReturnToGame, pentspin 8 frames, no strings/backdrop/hover colour, y-only bands 188–237 / 238–287 / 288–334 at 800×600, Options → Sound/Video/Automap/Controls/Previous; Esc close-all `0x00456300` with flag table `0x006D6378`; SP pause while ui 9/11 open; char-select wheel exists (`0x004FA340`); hardcore always offered; no `blizno` screen. d2rs-own (left): REC-180 (1)–(5), 181, 182, 184, 185, 186 | `ui/frontend-options.md` §O1, §O2 r4, §O4 r5–r6; `ui/panels.md` §2 r9, §3 r1; `ui/frontend-menus.md` §F1.3, §F2.5 r5, §F3.5 r2; `ui/frontend-credits.md` C1 r3 | hardcore + no logo match; Esc menu, pause, wheel do not | `q-fix-esc-menu`, `q-fix-sp-pause`, `q-fix-charselect-wheel` |
| REC-174 (1), (3), cross-act TP, lair placement, door 153; REC-175 (1), (2) | act change `0x0053ACC0` writes unit +0x18 / +0x1C after placement; Tyrael's portal = ordinary pair `0x0056D130`/`0x0056CF40` (73 → 40 tile-12 point, mask 0xBE11 step 5, owner −1); a cross-act portal is a fatal error (0xE5C); Tyrael/Duriel/door 153 only from `duriel.ds1` presets (78,88), (135,218), (110,207); door opens on Duriel's death only; old-room leave happens before the client room switch; callers pass a spawn tile index (Harrogath portal (109,5), cain6 (103,5)). REC-174 (2) d2rs-own | `world/waypoints.md` §11 new, §7 r5, OQ9; `world/objects-2.md` §25 new; `world/quests-act2.md` §8.11; `world/npc.md` §8.3; `sim/units.md` §2 | unit act set matches (drop note `act_change.rs:109`); `activate_waypoint` matches; placement, tile index, portal pair do not | `q-fix-act-change`, `q-fix-portal-pair` |
| REC-46, REC-50 (model part); model OQ17, ui OQ5; properties §13 callers; generation §12.2 | 0x67 u32@0x27 = 4 / 0x804 / 0x100004 / 0x100804 (hardcore bit 0x800); pet +0x1C always 100 (setter `0x00479010` dead); recharge walks runeword list (171) before main; skill fallback `0x00496CF0` runs only in tick passes and once per paused pass; `[0x007BC850]` = chat input line, gold dialog `0x00454150` | `client/model.md` §7 r9, §14 r3, §17 r4, OQ10, OQ17; `client/bridge.md` §8 r5; `client/ui.md` §B8.1, OQ5; `items/properties.md` §13; `items/inventory.md` OQ1; `items/generation.md` §12.2 | +0x1C matches; create flags, recharge order do not; fallback not implemented | `q-fix-create-flags`, `q-fix-recharge-order`, `q-skill-fallback` |

Follow-ups for a later worker (other owners): `client/msg-stats-items.md` §5 r4 (0x7D) and the 0x92 rule have the (1,1)/(0,0) labels swapped ((1,1) = removed / owner set list freed); `items/properties.md` §13 caller list should name the 7 other client sites (stat-lists §2 r5); `items/inventory.md` OQ1 and `client/msg-stats-items.md` OQ3 can point to stat-lists §2 r1.1. `combat/damage.md` §3.1 step 1 should link `skills/bodies-2.md` §2.27; `client/stat-lists.md` §2 r8 should link the `skills/use.md` §2 item type test. `sim/path-placement.md` ~line 610 "the act-change spec" → `world/waypoints.md` §11.

HANDOFF REC lines to update when HANDOFF is free (Lane B is editing it): REC-237, REC-204, REC-211, REC-212, REC-229 → "settled from the binary 2026-10-08 (pc1-s8), capture verifies only", pointing at the sections above; REC-177 (3), REC-188, REC-232, REC-233 likewise.

## Lane C — recordings PC 2 needs (to move into HANDOFF §7)

- **R-SWAP-1** [MANUAL] Settles: REC-177 (3) message order and fields.
  Expansion character, set 1 sword + shield, set 2 a two-hander that fails
  a requirement, a different left/right skill chosen in each set. Press W
  twice, then once more with both sets empty. Capture S→C in order: 0x97,
  the direct 0x23s, 0x9D action 0x17 ×n, 0x47, 0x48, the two queued 0x23;
  the 0x23 fields and the 0x9D bit streams (body location, flag 0x4000 on
  the unusable two-hander).
- **R-SET-1** [MANUAL] Settles: REC-188 (stat-lists OQ8). Equip a
  two-hander (action 0x07) over a worn set shield while a partial set bonus
  shows; record 0x9C/0x9D and what follows, then read the character panel:
  does anything refresh the taken-off item's set list?
- **R-SET-2** [MANUAL] Settles: REC-188 (stat-lists §2 r6, set test
  vectors). Equip 2 then 3 pieces of one set, unequip one; record the
  0x9C/0x9D streams (set mask, lists), confirm no 0xA8 for states 165–170
  and base-only 0x1D–0x1F; compare panel totals with base + item lists +
  client-computed set bonuses.
- **R-WW-1** [MANUAL] Settles: REC-173, REC-232 (tick timing). Whirlwind
  with one and with two weapons, and one Leap: per tick the unit position,
  0x4C/0x4D, type-0 timer args, E param 4, hit ticks.
- **R-CLAW-1** [MANUAL] Settles: REC-233. Fists of Fire or Dragon Claw with
  two claws vs one claw on one monster: type-0 events per attack, unit flag
  0x40, damage per hit with different claws.
- **R-SENTRY-1** [MANUAL] Settles: REC-233 / REC-176 sentry part. One
  Lightning Sentry laid near a monster: shots fired (expect 10), think
  ticks, death tick.
- **R-PAUSE-1** [AUTO] Verifies `ui/frontend-options.md` §O1 r6 (REC-237).
  1.14d single player: log the server frame counter and the client
  update-clock global `0x007A0490` each loop pass; open the Esc menu 10 s,
  close it. Expect no server tick while open and no catch-up after.
- **R-ACT-1** [MANUAL] Settles: REC-175, REC-174 (Meshif part), waypoints
  OQ9. Tyrael's travel and the Harrogath portal (operate 73) to level 109,
  Meshif to level 75: packets and RNG (level-seed `roll(n)` at `0x0066ACB3`),
  order 0x05 / 0x03 / 0x53 / 0x07… / 0x15, arrival point, whether a 0x0A for
  the player's own GUID is sent. Needs saves at those quest stages.
- **R-TYRPORT-1** [MANUAL] Settles: REC-174. Talk to Tyrael (msg 302), walk
  through the portal: its 0x51, position, the two game-seed steps, arrival
  point in Lut Gholein (expect near the tile-12 spawn), removal / ENDANIM on use.
- **R-HCFLAG-1** [AUTO] Confirms `client/model.md` §7 r9 (REC-46). 0x67
  bytes of a classic-hardcore and an expansion-hardcore character joining
  single player; expect u32@0x27 = 0x804 and 0x100804.
