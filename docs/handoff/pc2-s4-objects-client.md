# Handoff: PC 2 spec worker, session 4 — client object functions (`objects-client`)

Spec session, 2026-10-07. Worktree branch `claude/spec-objects-client-wt`
from `origin/claude/spec-objects-hirelings-s4` (`9b45740`), merged
`origin/claude/spec-audio-s4` (read-only: `audio/triggers-2.md` §20) and
`origin/claude/pc2-spec-gaps`; pushed to
`refs/heads/claude/spec-objects-hirelings-s4`. Evidence: the 1.14d
`Game.exe` (exports, `tools/ghidra/disasm.py`), live `patch_d2`
`objects.txt`. No recordings, no Ghidra runs, no cargo.

## Written

| Spec § | Behaviour | 1.14d addresses |
|---|---|---|
| `world/objects-client.md` §25 r1 | dispatcher, 19-entry table, fatal 0x546 on ≥ 19, live row counts per `ClientFn` | `0x004BDEE0`, `0x007277F0` |
| §25 r2–r3 | call site A (every update, after the generic step; ≤ 3 sound only, ≥ 4 function then sound on non-zero) and call site B (C objects, second call, result ignored) | `0x004BDFF0`, `0x00480810`, `0x00463CC0`, `0x00465AA0` |
| §25 r4 | which objects are C objects: river presets 40–42, 65 and `clientsmoke` 478; so `ClientFn` 1–3 are unreachable live | `0x00466820`, `0x00666170`, `0x006661A0`, `0x0066BF30`, `0x004B15B3` |
| §25 r5–r7 | notation (`range` `0x004BC500`, `set_mode`, refresh, reinit), wall-clock rule and the recorded d2rs clock choice (`now: u32` input), client-only modes | `0x00624690`, `0x00624390`, `0x00470610`, `[0x006CC260]` |
| §26.1–§26.18 | every `ClientFn` 1–18: ripple counter, drinker / gesturer / turner timers, skeleton spawn and bubble timers, orifice / altar graphics preloads, smoke self-removal, invisible Ancient auto-interact (C→S 0x13), bonfire 500 ms day refresh, Anya `npcalert` overlay, Baal's portal shortcut, zoo chickens, Keeper grunt | `0x004BD730` … `0x004BDE40` (table in §25 r1), `0x004A30A0`, `0x004A30C0`, `0x004A3150`, `0x00481030`, `0x00480930` |
| §27 | latches `[0x007C025D]`–`[0x007C0261]`, their resets | `0x004A2390`, `0x004A3410`, `0x004A42A0`, `0x004A3100` |
| §28 | what d2rs must model and output | — |
| `world/objects.md` §15, §16.–18. | pointers to part 3 | — |

`object-functions.tsv`: unchanged. Its grammar (`world/objects.md`
Constants, kinds `init`, `operate`, `populate`, `preset`) has no client
kind, so the `ClientFn` table is prose only (§25 r1). No CODE-TABLE
CHANGE commit.

## Pending

- §26.16 (open question 1): whether `ClientFn` 16 can fire, i.e. the
  object speed +0x4C in mode 1 for 563 / 569 (source `0x00470610`, not
  traced). Settled by a recording of Baal's portal opening.
- Open question 2: what flag 1 of `0x0046F870` changes (same as
  `client/msg-ui.md` open question 1).

## CODE-TABLE CHANGE commits

None.

## Cross-file requests

1. `audio/triggers-2.md` §20 r2 ("owner: the client object functions,
   Cross-file request") and §21 row `ClientFn` → owner
   `world/objects-client.md` §25–§26. Add to r4: in 1.14d only C objects
   reach call site B and no live C object has `ClientFn` 3–6 (C objects
   are classes 40–42, 65, 478; `world/objects-client.md` §25 r4), so the
   second call happens only for `clientsmoke` (ClientFn 9, no sound
   call of its own).
2. `client/model.md` §5 rule 2/3 (PC 1): link the per-object client
   function: object update `0x004BDFF0` = generic step `0x004BCBB0`,
   then `world/objects-client.md` §25 r2; the C-set walk `0x00463CC0`
   runs `0x004BDEE0` again for type 2 (§25 r3) and `0x0046D780` for
   type 1. The client update needs a `now: u32` millisecond input
   (§25 r6).
3. `client/model.md` §8 rule 4 (code 0x02, PC 1): `0x00480930(type,
   GUID)` for an object in set S sends C→S 0x13 {0x13, type u32, GUID
   u32} (`0x004786A0`) after a path reset `0x00648B90` and, when U's
   flag +0xC4 bit 0x4 is set, P facing U (`0x00621C00`) and the skill
   start `0x004C6EB0` (classes 404 / 376 take other branches); for a
   monster it sends 0x13 only when `GetTickCount` − U+0xD4 ≥ 200 and
   then sets U+0xD4 (`0x00480AA7`). Caller seen here: `ClientFn` 13
   (`world/objects-client.md` §26.13).
4. `audio/sound-table-2.md` row "`0x004A3150` … owner none" → owner
   `world/objects-client.md` §26.17.
5. `render/lighting.md` open question 11 (answered `0x004BC5E0`): add
   that object class 39 also runs it every 500 ms through `ClientFn` 14
   (`0x004BDCC0`, `world/objects-client.md` §26.14), not only on a day
   period change.
6. `render/unit-composite.md` (overlay owner): `ClientFn` 15
   (`0x004BDCF0`) creates overlay 72 type 3 on object 558 every 500 ms
   in mode 0 and removes it in other modes; `0x00470390` has no
   duplicate test for type 3. Needs: lifetime of a type-3 overlay.
7. `client/msg-ui.md` §7 code 36: the latch `[0x007C025F]` drives
   `world/objects-client.md` §26.17 (client chickens, class 149).
8. `world/quests-act5-2.md` §7.8 object 561: the client sends the 0x13
   that triggers operate 69 by itself (`ClientFn` 13, distance < 25,
   39.0 set, 39.4 clear; `world/objects-client.md` §26.13).
