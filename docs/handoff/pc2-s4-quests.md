# PC 2 session 4 — quests lane (spec-complete pass)

Branch `claude/spec-quests-s4` (base `origin/claude/pc2-spec-gaps` dd480ad).
Every behaviour below was read from the 1.14d `Game.exe` (exports +
`tools/ghidra/disasm.py`); no recording.

## Written (spec § → behaviour, addresses)

| Spec § | Behaviour | 1.14d |
|---|---|---|
| `quests-status.md` §12 (+ Inputs row, vectors) | client quest check used by the level-entry lines (the `TODO(spec: world/quests.md)` in `d2-client/src/audio/environment/mod.rs`): 0x5E byte read by **row** index c, entry found by **chain** c, G.13 / P.1 / P.0 / P.14 tests, Den of Evil row-status < 5 (rewrites `last[1]`) | `0x004A4180`, `0x004B92E0`, `0x004B92B0`, caller `0x004CC322` |
| `quests.md` §11 | pointers to `quests-status.md` §1/§12 and `quests-helpers.md`; "not yet specified" line reworded (OQ8 answered) | — |
| `quests.md` §8.1 | act completion runs **after** the act change (relay from world-econ) | `0x0057A67A`/`0x0057A688`, `…6FF`/`…70D`, `…786`/`…794` |
| `quests.md` §6.3 | 0x5D status byte = 0 when a status_fn returns 0 (not the record status); no room / level → nothing sent | `0x00544190` (`0x005442AF`) |
| `quests.md` §8.4 | every cow-portal failure (refusal, no spot, no portal) plays the sound | `0x00594140` exit `0x0059424B` |
| `quests.md` §9.1 / §9.2 | pointers: `items/generation.md` §10.2 (`0x00559CE0`), item level helper, item search | — |
| `quests-act2-2.md` §5.1 | quest-chest gate (mode test, `Mode1` → mode 1 + ENDANIM at f + fc1>>8, no +1; else mode 2; flag 0x2 cleared) | `0x00545850` |
| `quests-act2-2.md` §5.2 | Tainted Sun start / end on the act environment record | `0x0061C450`, `0x0061C4D0` |
| `quests-act2-2.md` §5.3 | remove a unit for everyone (0x0A to clients whose room list holds it, then free) | `0x0052E050`, `0x0052DFB0` |
| `quests-act2-2.md` §5.4 | scroll text 0x27 type 2 | `0x005456A0` |
| `quests-act2.md` §5.3, §6.9, §4.9, §8.7, §8.9, §10 table; `quests-act2-2.md` §2 item 1 | independent darkening tests; sanctuary portal init (only the stored u16 becomes 2); Arcane hook independent tests + flags := 0; staff hand-in null reads; Arcane dummy list for a tomb outside 66–72; blocker walk stores the **first** player; init 18 without a free spot | `0x0059EE00`, `0x0059BA40`, `0x0059B660`, `0x0059DD80`, `0x0059D720`, `0x0059B820`/`0x0059F75A`, `0x0059F380` |
| `quests-helpers.md` (new) §1 | quest free-spot search (lattice order, room choice incl. the y-vs-x-end bug) | `0x00545340` |
| §2 | critical monster spawn and retry ladder (spreads −1, 5, 10×20, 15; (y, y+21) bug) | `0x005459A0` |
| §3 | superunique spawn at a point (class = id + monstats rows, `0x0054E600`) | `0x00545C30`, `0x00659B80` |
| §4.1–§4.3 | Ancient missile 541, missile-at-point wrapper (Baal 625), orb missile 368 | `0x0058C8D0`, `0x0056EDE0`, `0x005DFEE0` |
| §5 | end a player's interaction by kind, S→C 0x62 (byte 6 stale) | `0x005351C0` (+ `0x00579130`, `0x00572E00`, `0x005854D0`, `0x00567330`, `0x0053D6D0`) |
| §6 | end the game (host; owner decided `d2-server`, `d2-sim` raises a host request) | `0x00530590` |
| §7 | close a player's town portal (+ partner) | `0x00535430`, `0x005353F0`, `0x00553720` |
| §8 | find a player's item by code (cursor first, list order, page 1 skipped, quest-difficulty test) | `0x00558110` |
| `quests-act3.md` §4.7, §7.7 | stairs warp pointer; stairs ENDANIM frame (no +1); orb kill pointer | `0x005B85E0`, `0x005DFEE0` |
| `quests-act3-2.md` §11.4 | Golden Bird chooser test order; a class with no monstats row fails the whole test | `0x00544E80` |
| `quests-act4.md` §5 table, OQ10 | `0x005B5210` without chain 23 → 0; host ownership | `0x005B5226` |
| `quests-act5.md` OQ5; `quests-act5-2.md` OQ3, OQ5, OQ6, §7.6, §7.8, §8.5, §8.8 | state 118 = `corpse_noselect`; BaalToStairs (AI 138) calls `0x0058E600`; `0x0058E920` called by missile 625 bodies; pointers to the helpers | `0x005EF67B`, `missiles/bodies-2.md` §60 |

Open questions answered in place: act2 2, 3, 4; act4 10; act5 5; act5-2 5, 6
(and the owner half of 3).

Code `TODO(quests…)` hooks now answered by the specs (implementation
follow-up, not spec gaps): act2 OQ4 (q4.rs:345), act2 §5.3 / §6.9 / §4.9 /
§8.7 / §8.9 / §10 (q3.rs, q4.rs, q6.rs), act2-2 §2.1 (q4.rs:580; the reading
"reported" is wrong: spawn with room 0), act3 §4.7 (q2.rs:497), act3-2 §11.4
(q4.rs:427: skip entirely), act4 OQ3 / OQ4 / §8 (q2.rs, q3.rs), act5 §4.7 /
§5.7 / §5.9 ×2 (already in the spec), act5-2 OQ6 / §7.8 / §8.4 / §8.8
(q5.rs, q6.rs: already in the spec or `quests-helpers.md`), quests §6.3
(quests.rs:1734: send 0, not `r.status`), §8.4 (quests.rs:2117: confirmed),
§9.1 / §9.2 (quest_items.rs), chain 37 (`quests-act1-rest.md` §9 item 6).
Two readings in the code are contradicted: `jerhyn_near_blocker` must
store the first qualifying player and stop; the sanctuary portal init
must not set the object's mode to 2.

## Pending

- `quests-helpers.md` OQ1: whether the classic end reaches `0x00530590`
  once per game or per client (the `quests-act4.md` OQ2 recording).
- Unchanged recording items (not blocking): act1-rest 3, 5; act2 10, 31;
  act2-2 1; act3 8; act4 2, 13; act5 7; act5-2 7; quests 2, 4, 14;
  quests-status 1. act1-rest 4 (progression readers) belongs to the save
  spec.

## CODE-TABLE CHANGE commits

None (`quests.tsv` and `quest-messages.tsv` needed no change: no
`catalogued` or `?` cell is left).

## Cross-file requests

- `render/lighting.md` §9.1: "+0x2C has no writer, so 128" is wrong in
  1.14d: the Tainted Sun start `0x0061C450` writes +0x2C := 1 (speed
  `[0x007443E8]` = 4, `0x0061C465`) and the end `0x0061C4D0` writes 0
  (speed 128); point to `quests-act2-2.md` §5.2.
- `audio/environment.md` §4 r2: the owner of the quest check
  `0x004A4180(q)` is `world/quests-status.md` §12 (not `world/quests.md`);
  q is a chain id for the entry lookup but indexes the 0x5E bytes by
  init-table row (Act II lines q 12 / 13 read the byte of chain 11 / 12).
- `client/msg-ui.md` §14 r2: the 37 bytes are the not-intro bytes in
  init-table row order (`world/quests.md` §3 step 5); their only reader
  is `world/quests-status.md` §12.
- `sim/server-messages.tsv` 0x62 (owner sim): layout kind u8@1, GUID u32@2,
  byte 6 never written by `0x0053D6D0` (`quests-helpers.md` §5 step 3).
- `crates/d2-sim/src/world/quests.rs` seam docs (implementation owner):
  `stairs_warp` / `object_stairs_warp` are `0x0059D9D0` = warp every
  type-5 tile of the object's room; `create_object_at` is not
  `0x0056EDE0` (that is the missile wrapper, `quests-helpers.md` §4.2).
