# Handoff: client side of objects — `claude/impl-final-objclient`

Cloud implementation session, 2026-10-07. Base `claude/specs-staging-7`
at `431a3fd`. Repo only, no game files. Everything below is
**implemented, unverified** (METHODS M02): the spec is a draft read from
the 1.14d disassembly and no recording of these objects exists.

## 1. What landed

| Spec | Code | Tests (`d2-client`, `bridge::objects::tests`) |
|---|---|---|
| `world/objects-client.md` §25 r1–r3, r5–r7 | `bridge::objects` (`object_update` = `0x004BDFF0`: generic step, then `ClientFn` ≤ 3 → mode sound call, ≥ 4 → dispatch then sound when non-zero; `site_b` = the second call of a C object; `dispatch` = `0x004BDEE0`, fatal 0x546 at ≥ 19; `Cx` helpers `step`, `range` (`0x004BC500`), `set_mode`, `reinit`, `refresh`, `sound`, `end`) | `dispatch_zero_and_out_of_range`, `call_sites_a_and_b`, `update_order_c_objects_before_s_players` |
| §26.1–§26.18 (all 18 functions) | `bridge::objects::fns` | one test per function family with every spec vector (7, 10, 4, 5, 6, 9, 13, 17, 18, 2/3, 8/12, 14, 15, 16) |
| §27 (latches) | `objects::Latches` in `ClientWorld::objclient`; S→C 0x50 code 36 now also sets `[0x007C025F]` / `[0x007C0265]` in the model (`msg/ui_quest.rs`) | `preload_latches`, `clientfn_17_vector`, `quest_special_36_sets_the_zoo_latch` |
| `render/lighting.md` OQ11 (`0x004BC5E0`) | `fns::day_refresh` (run by `ClientFn` 14) | `clientfn_14_bonfire` |
| `client/model.md` §5 r2–r3 | `bridge/update.rs`: object update before each S object's drain; the C objects' walk (A then B) after the S missiles, before the S players | `update_order_c_objects_before_s_players`, `call_sites_a_and_b` |
| `client/model.md` §8 r4 code 0x02, §8 r7 | `objects::interact` (`send` = `0x00480930`, all five type cases; `command_13` with the action gate `0x00480BA0`); the player mode request code 0x02 of a queued message now calls `send` (`msg/units.rs`) | `interact_sender_cases`, `player_mode_request_code_2_sends_the_interact`, `clientfn_13_vectors` |
| `client/model.md` §2 r1 (set C), §2 r6 (flag-ex bit 0x2000000) | `ClientObjects::set_c`, `create_client_unit` (`0x00466730`), `remove_client_unit` (`0x00465F00`); `msg/units.rs` `create` sets flag-ex 0x2000000 in an expansion game | in the tests above |
| `render/overlay.md` §5 (`ClientFn` 15 row) | `ObjFx::OverlayCreate` / `OverlayRemove` (kind 3, overlay 72, every 500 ms) | `clientfn_15_overlay` |

Model and bridge changes:

- `ClientUnit`: `interact_ms` (+0xD4, model §8 r7's name; also the
  object timer T), `frame` (+0x44), `flag_ex` (+0xC8), `flag_4` (+0xC4
  bit 0x4; no writer is specified yet).
- `ClientWorld::objclient`: set C, the §27 latches, the client GUID
  counter.
- `ModelInputs::now` (wrapping ms, §25 r6) and `ModelInputs::objclient`
  (`ObjClientInputs`: object rows, the UI's client quest record, the
  hand item code, the skill gate bits, the 0x16 pick-up byte).
- `Bridge::set_now`, `set_object_rows`, `set_client_quest_flags`. The
  Bevy `bridge_frame` passes `Time<Real>` elapsed ms as `now` (the live
  host clock; an app without `Time` keeps the set value). `app/play.rs`
  hands the live `objects` rows over (`single_player::client_object_rows`;
  none for the synthetic game, so its object update runs nothing).
- Two update-pass outputs, `Output::ObjectSound(ObjSound)` (audio) and
  `Output::ObjectFx(ObjFx)` (effects), with their rows added to
  `specs/client/bridge.md` §10 (producer `update`) and to `ROWS`; §10 r11
  now says they stay in list order (only `TownExit` is delivered early).

## 2. Changed expectations

None of the existing tests changed their expectations. The exhaustive
field test `client_world_holds_only_stated_fields` names the new fields
with their owner specs.

## 3. PROVISIONAL (M22; grep `PROVISIONAL` in `crates/d2-client/src/bridge/objects`)

| Point | Choice | Settled by |
|---|---|---|
| generic step `0x004BCBB0` speed (`objects/mod.rs` `generic_step`) | speed = the class's `FrameDelta[mode]` | REC-45 (§26.16) |
| generic step, cycling mode | frame wraps modulo `FrameCnt[mode]` | REC-45 |
| generic step, `FrameCnt` 0 | no advance | REC-45 |
| generic step, end of non-cycling mode 1 | `set_mode(2)` + graphics refresh (the transition §26.2 / §26.7 / §26.16 name) | REC-45 |
| §26.16 `ClientFn` 16 | rule implemented as stated; expected never to fire through site A | REC-45 (spec's own PROVISIONAL) |
| client GUID counter `[0x00711F30]` (`create_client_unit`) | starts at 0; the GUID is the value before += 1 | needs a REC id: REC-objclient-1 (a capture of two client chickens' GUIDs, or the counter's writers in Ghidra) |
| interact sender "P's skill on side 1" (`interact.rs` `skill_id_of`) | `0x006439F0(P, 1)` = the highest entry of skill 1; "skill 0" = native entry of skill 0; no entry → `Unspecified` error | REC-objclient-2 (static: read the args of `0x006439F0` at the call in `0x00480930`) |

The two `REC-objclient-*` ids are placeholders for the coordinator to
number in HANDOFF §7.

## 4. Left (not done)

1. **Consumers of the new outputs.** No audio or effects consumer
   handles `ObjectSound` / `ObjectFx` yet (impl-triage-client owns the
   audio driver and render hooks): mode sound → `audio::triggers::objects::object_mode`;
   `Request` → `SoundCalls::request`; `Light` →
   `rules::lighting::sources::object_light`; overlays → the overlay
   owner (`render/overlay.md`, not implemented).
2. **Generic step pieces not specified**: the `Lit2` light of
   `0x004BCBB0`, unit flag 1 and the temporary stat lists of `set_mode`
   (`sim/units.md` §4.1), the object speed field +0x4C.
3. **Set C creators other than the zoo**: the river presets (room preset
   pass `0x00466820`, §25 r4) and `clientsmoke` from the monster update
   (`0x004B15B3`) belong to the client DRLG / monster update sessions;
   they should call `objects::create_client_unit`. The C missiles and C
   monsters walks run nothing (their updates and `0x0046D780` are Phase
   6).
4. **Inputs nobody supplies yet**: the UI layer's client quest record
   (`Bridge::set_client_quest_flags`; without it `ClientFn` 13 is fatal
   0x1F as in 1.14d once the Ancient is in mode 0 with a local player),
   the hand item code, the skill gate bits, the 0x16 byte.
5. **Latch reset**: §27 r1's resets (`0x004A2390`, `0x004A3410`,
   `0x004A42A0`) are `Latches::reset`; nothing calls it yet (a new
   `ClientWorld` per game starts cleared).
6. Unit sizes for monsters / missiles in `0x006416D0` (`unit_size`
   returns 0; only P and objects are measured by these rules).
7. The path reset `0x00648B90` of the interact sender (no client path
   record in the model).

## 5. Gate

`CARGO_INCREMENTAL=0 sh tools/gate.sh` — result in the session's final
message and the commit.
