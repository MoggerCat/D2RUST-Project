# Spec: Audio — Sound triggers (which code path requests which sound)

- **Status:** draft; every rule names its 1.14d `Game.exe` address;
  no rule is confirmed by a recording yet (Test vectors, "Checks", names
  the hook each check needs).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::audio::triggers` (the per-cause rule
  functions of `client/audio.md` §A2; plain Rust, fed by the bridge),
  tables `object-sounds.tsv`, `npc-speech.tsv` (this directory).
- **Related specs:** `audio/sound-table.md` (owner of the request call,
  variants, channels, volume/pan; this spec only says *when* and *with
  which arguments* it is called), `audio/environment.md` (music, quest
  stingers, level-entry lines, ambience), `client/audio.md` §B2, §B6,
  `client/model.md` §5, §8 (client update pass, mode requests),
  `sim/units.md` §4 (unit fields +0x44/+0x48/+0x4C), `sim/rng.md` §3,
  `sim/server-messages.tsv` (S→C 0x2C, 0x5D), `sim/client-messages.tsv`
  (C→S 0x3F), `data/fields.tsv` (column offsets quoted here),
  `skills/use.md`, `skills/bodies.md` (skill functions; not restated),
  `missiles/missiles.md`, `monsters/ai.md` (who sends events 16–18),
  `world/quests.md` (who sends quest events), `world/npc.md`,
  `items/inventory.md` (who sends item events), `client/ui.md` §B8,
  `audio/triggers-2.md` (part 2: §13–§17).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 52–66 |
| Inputs | 67–79 |
| Outputs / state changes | 80–85 |
| Rules | 86–87 |
|   1. Conventions and shared state | 88–164 |
|   2. Server sound events (S→C 0x2C) | 165–213 |
|   3. Player event sounds (`0x004CB9C0(U, event e)`) | 214–277 |
|   4. Mode sounds | 278–383 |
|   5. Footsteps (`0x004CAF60(U)`) | 384–435 |
|   6. Monster idle voices | 436–462 |
|   7. Object mode sounds (`0x004CB460`, objects) | 463–504 |
|   8. Skills, missiles, states | 505–547 |
|   9. Items | 548–571 |
|   10. NPC speech | 572–641 |
|   11. UI sounds | 642–671 |
|   12. Other fixed requests | 672–735 |
| Constants & data dependencies | 736–752 |
| Randomness | 753–776 |
| Edge cases & original bugs | 777–793 |
| Test vectors | 794–825 |
|   Checks (hook addresses for `record_sound.py`, `client/audio.md` §B7) | 826–839 |
| Provenance | 840–876 |
| Open questions | 877–988 |
<!-- /index -->

## Summary

Every sound the 1.14d client plays starts as a call to the request
entry `0x004B9A00(id, unit, delay, flags, offset)` (`sound-table.md`
§5). 222 call sites in 132 functions make that call. This spec groups
them by cause: server sound events (S→C 0x2C), the per-class player
voice table, unit mode changes (hit, death, attack, block, kick, skill
modes), footsteps and idle voices from the per-update unit code
(`Unit\UnitSnd.cpp`), object mode sounds, the sound columns of
skills/missiles/states/items, NPC greetings and dialog lines, and UI
clicks. For each it gives the sound id rule, the unit the sound is
attached to (or none: non-positional), the delay, the flags, the
client-side guards (timers, "already speaking"), and every client RNG
draw in order. Music and ambience are `audio/environment.md`.

## Inputs

| Name | Type | Source |
|---|---|---|
| S→C 0x2C PlaySound | unit type u8@1, GUID u32@2, event u16@6 | handler `0x0045E110` |
| S→C 0x5D (UI action) | code u8@1, flags u8@2, value i16@4 | handler `0x0045E540` |
| client mode sets | unit, new mode | `0x00480E70` (`client/model.md` §8) |
| client update pass | per unit, per update | `client/model.md` §5 |
| client unit fields | type +0x00, class +0x04, mode +0x10, frame +0x44, frame count +0x48, speed +0x4C, sound fields §1 r6 | `sim/units.md` §4 layout (client copy) |
| tables | `sounds`, `monsounds`, `monstats`, `monstats2`, `skills`, `missiles`, `states`, `weapons`/`armor`/`misc`, `uniqueitems`, `setitems`, `soundenviron`, `levels` | `data/fields.tsv` |
| static tables in `Game.exe` | §3 r1, §4.3 r1, §7, §10 | `object-sounds.tsv`, `npc-speech.tsv`, prose tables |
| client RNG | local player client unit seed | `sound-table.md` §4 r5 |

## Outputs / state changes

Requests (id, unit, delay, flags, offset) in a defined order per client
update, plus per-request volume sets (`0x004B9B50`), fades, stops and
the sound fields of §1 r6. Each client RNG draw listed in Randomness.

## Rules

### 1. Conventions and shared state

1. **Request.** `request(id, U, d, f, o)` is `0x004B9A00` with ECX =
   id, EDX = unit (0 = none), stack = delay `d` (sound ticks), flags
   `f` (bit 0 exact id, bit 1 no fade-in), start offset `o`; it returns
   a handle or 0 (`sound-table.md` §5). "On U" means the unit is passed
   (positional, tracks U); "none" means 0 (centre, no falloff). Unless
   a rule says otherwise d = f = o = 0.
2. **Volume set** `0x004B9B50(h, v)`: if a request with handle `h`
   exists, its volume (+0x1C) := v. Rules write "volume v" for it; it
   is applied by the next sound update (`sound-table.md` §8.2 r1).
3. **Detach** `0x004BA790(h, U, force)`: removes U from request h's
   unit list; if other units remain, nothing more. Otherwise the
   request is stopped (stop flag, fade-out per `sound-table.md` §5 r5)
   when `force` is set, or when its sound has `Loop` and no fade to 0 is
   running; a non-looping one-shot keeps playing where it is.
4. **Group stops** (each sets the stop flag and, if the request is
   playing and the row has `Fade Out`, starts a fade to 0 over `Fade
   Out` ticks): `0x004BA840(h)` one handle; `0x004BA890(id)` every
   request of that id; `0x004BA8F0` every request whose id is in the
   song range 4,657–4,684 (`sound-table.md` §1 r5); `0x004BA950(a, b)`
   ids 52–71 except group bases a, b; `0x004BA9D0(a, b)` ids 72–201
   except group bases a, b; `0x004BAA50` ids 2,934–4,656 (speech).
5. **Time bases.** T = sound tick `[0x007BC9BC]` (`sound-table.md`
   §6.1; the recorded offsets of the first frames: REC-1684). C = client update counter `[0x007A0498]`, +1 per client update
   `0x0044C790` (`render/capture.md` §2 table; one per server tick in
   single player, `client/model.md` §5 r1). All timers in this spec are
   in C unless they say T. Request delays are always in T.
6. **Unit sound fields** (client units): +0x70 u8 object mode seen,
   +0x74 object previous mode (§7); +0x78 list of requests attached to
   the unit; +0x7C last voice (C); +0x80 last idle voice (C); +0x84 last
   footstep (C); +0x88 sample-lock flag (`0x004CC160`, cache only,
   `sound-table.md` §10 r4); +0xB0 last hit class taken (u8, open
   question 3). Globals: `[0x007C88B8]` last idle voice of any monster
   (C), `[0x007C88BC]` last voice of any unit (C), `[0x007C88C0]` idle
   gap (C), `[0x007C88C4]`/`[0x007C88C8]` time (C) and id of
   the last player speech line (§3 r6). All five are reset at every
   game start by sound init (`0x004CA280` from `0x00482282`): 0, 0,
   **90** (corrected: an earlier draft said the gap starts at 0), 0,
   0.
7. **Draw helpers** on the client RNG (`sound-table.md` §4 r5,
   `0x004E40A0`): `roll(n)`; `uniform(lo, hi)` = lo + roll(hi − lo +
   1) (`0x004E4100`); `jitter(r)` = roll(2r + 1) − r (`0x004E4120`).
   Variant picks inside the request (`sound-table.md` §4 r3) happen at
   channel start, after the rule's own draws.
8. **Speaking.** `speaking(U)`: a playing request with id 2,934–4,656
   whose unit list holds U (`0x004B9E90` → `0x004B9700`). `any_speech`:
   a request not ended with id 2,934–4,656 (`0x004B9C20`).
9. **Swing delay** `swing(U, h)` (`0x004CA500`): frames `F` from the
   swing table §4.3 r1 at index `h & 0xF` (index ≥ 14 is fatal), ticks
   = (F × 256 + 128) / max(U+0x4C, 1), unsigned, truncating.
10. **Type and class.** "player"/"monster"/"object" = unit type 0/1/2.
    Monster class = monstats row (`0x004CA2C0`); "base class" = its
    `BaseId` (`0x00463860`).
11. **Id 0 and unguarded calls** (answers TR-6). Where a rule has no
    "id 0 → nothing" guard, the request is still made with id 0: the
    request entry returns 0 at once for `id < 1` (`0x004B9A21`), before
    any record read, list change or RNG draw (`sound-table.md` §5 r1).
    Recorded: the request log of `docs/handoff/local-buddy-q-rec.md`
    entry 74 holds many id-0 requests from `0x004D9BC7` (state sounds
    at unit creation, §8 r4) with return 0 and the seed unchanged. d2rs
    may make or skip such calls; the logs compare equal only if the
    request log keeps them (it records every entry call).
12. **Time comparisons.** Every timer test in this spec is an unsigned
    32-bit difference against C or T (`jb`/`jae`/`jbe` on the
    difference, e.g. `0x004CB5C4`–`0x004CB6A0`, `0x004CB043`,
    `0x004CC359`), so a stored time in the future reads as a huge
    elapsed time.
13. **Identity, lists, objects, inputs** (second pass): the type, class
    and mode the rules of §4–§6 read are the unit's *sound identity*
    (a transformed player sounds as its monster form) and its
    `monsounds` record is chosen by superunique / unique / minion rules:
    `audio/triggers-2.md` §18. The unit's request list (+0x78: order,
    lifetime, the group walk `0x004CA900`, unit free): §19 there. When
    object units make their §7 call: §20. Every client input the rules
    read and its owner spec: §21.

### 2. Server sound events (S→C 0x2C)

1. 0x2C (8 bytes): unit type u8@1, GUID u32@2, event u16@6. Handler
   `0x0045E110` calls `0x004CBDE0(GUID, type, event)`; the unit is
   looked up (`0x00463990`); none → nothing. The server side
   (`0x00553380(unit, event, target)`) is owned by the senders
   (`world/quests.md`, `world/cube.md`, `world/npc.md`,
   `items/inventory.md`, `monsters/ai.md`).
2. Event table (switch `0x004CC108`/`0x004CC0C4`); U = the event unit,
   P = local player `[0x007A6A70]`:

<!-- rows -->
| Event | Sound | Rule |
|---|---|---|
| 10 | 2,673 `shrine_refill` on U | then volume 120 |
| 12 | `stsound` of a skill read from U (`0x006256B0(U, 0x44)`, `0x00625D00(·, 350, 0)`, `0x006439F0`, `0x0045C4B0` +0xFC) | only if > 0 (open question 4) |
| 13 | 2,634 `object_trap_trigger` on U | |
| 14 | 2,635 `object_trap_release` on U | |
| 15 | 4,290 `rogue_confirm_a_1` on U | only if U's class is 271 (`roguehire`) |
| 16 | monster: `monsounds.Taunt` (+0x74) on U | non-monster or no record: id 0 (nothing) |
| 17 | monster flee voice | §6 r4 |
| 18 | NPC greeting of U (§10 r1, mode 1) on **P**, flags 1 | |
| 84 | 4,615 `hireable_female_thank_you` if U's class is 271, else 4,625 `hireable_male_thanks_1`, on U | + text, + r3 |
| 85 | 4,612 / 4,616 (`…cant_use_that_ever…`) | same |
| 86 | 4,613 / 4,619 (`…cant_use_that_yet…`) | same; 84–86 also come from the client hireling checks `0x0048B7C0` and `0x004934D0` (`ui/panels-3.md` §30) |
| 87 | 4,614 / 4,622 (`…i_will_use_that…`) | same |
| 90 | 4,379 `ancient_act5_reward_spiel` on U | |
| 91 | 8 `cursor_level_up_hireling` on P | |
| 92 | none: stinger speech re-arm `0x004DCE10(25, 1)` | `audio/environment.md` §3 r5 |
| 93 | 2,553 `object_corpse_loot` on U | |

3. Events 84–87 and every event not in the table (0–9, 11, 19–83,
   88, 89, ≥ 94) go on to: if U is a player, the player event sound
   (§3). Events 84–87 first show the overhead text of their id
   (`0x004A0200`, `client/ui.md`).
4. **Dispatch owner of 0x2C** (`client/bridge-dispatch.tsv`). Client
   model state written: none (`0x0045E110` and `0x004CBDE0` write no
   unit or global the model holds). The handler looks U up (r1; set S,
   `client/model.md` §2 rule 2); none → nothing. Else it emits one
   `ServerSound` output (`client/bridge.md` §10) with U's key (type
   u8@1, GUID u32@2), U's class and the event u16@6, captured at
   receive. The audio layer applies r2–r3 to it, with P = the local
   player at delivery. Events 84–87's overhead text is part of the
   same output: the audio consumer makes the overhead-text request
   (`0x004A0200`, `client/ui.md`) as `0x004CBDE0` does
   (`client/bridge.md` §10 rule 5). Recorded: `2c 01 26000000 1200`
   (`20261006-015956` seq 293508, event 18, an NPC) and `2c 00 01000000
   0200` (`-022633` seq 46370, event 2 on the player → §3).

### 3. Player event sounds (`0x004CB9C0(U, event e)`)

1. **Class record** (static, `[0x0072A008 + 4·class]`, 11 dwords;
   class ≥ 7 is fatal):

<!-- rows -->
| Class | hit | death | impossible | needmana | needkey | cantcarry | cantuseyet | notintown | chat base | quest base | footstep base |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 amazon | 2878 | 2883 | 2953 | 2957 | 2955 | 2934 | 2936 | 2959 | 2937 | 2961 | 2768 |
| 1 sorceress | 2910 | 2915 | 3265 | 3269 | 3267 | 3246 | 3248 | 3271 | 3249 | 3273 | 2720 |
| 2 necromancer | 2894 | 2899 | 3109 | 3113 | 3111 | 3090 | 3092 | 3115 | 3093 | 3117 | 2720 |
| 3 paladin | 2886 | 2891 | 3187 | 3191 | 3189 | 3168 | 3170 | 3193 | 3171 | 3195 | 2768 |
| 4 barbarian | 2902 | 2907 | 3031 | 3035 | 3033 | 3012 | 3014 | 3037 | 3015 | 3039 | 2816 |
| 5 druid | 2926 | 2931 | 3421 | 3425 | 3423 | 3402 | 3404 | 3427 | 3405 | 3429 | 2720 |
| 6 assassin | 2918 | 2923 | 3343 | 3347 | 3345 | 3324 | 3326 | 3349 | 3327 | 3351 | 2720 |

   Offsets +0x00 … +0x28 in column order. Footstep bases: 2,720
   `light_walk_dirt_1`, 2,768 `medium_walk_dirt_1`, 2,816
   `heavy_walk_dirt_1` (§5).
2. If U is P and P is missing or in mode 17 (dead) (`0x00463DF0`):
   nothing. Exact (`0x004CB9C0`–`0x004CBA23`): U null or U's class ≥ 7
   is fatal (`0xB22`) before this test, so "missing" never applies: the
   rule is U = P (`[0x007A6A70]`) and P's mode (+0x10) = 17. Another
   player's events play even when it is dead.
3. **Chat, e = 25–32** (sent for C→S 0x3F PlayAudio,
   `client-messages.tsv`): unless `speaking(U)`, id = chat base + 2·(e
   − 25) (8 lines, 2 variants each), on U.
4. **Quest lines, e = 33–83**: id = quest base + (e − 33) (51 lines;
   amazon 2,961 `amazon_act1_complete_andariel` … 3,011
   `amazon_act5_defeat_baal`), on U. Events 33, 34, 35, 37, 50, 52, 66,
   75, 80, 82, 83 instead start a quest stinger that plays the line
   later (`audio/environment.md` §3) and stop here. The others are
   requested at once with delay 0 for e ∈ {38, 40, 41, 42, 43, 44, 46,
   47, 55, 56} and 12 otherwise (table `0x004CBD68`).
5. **Fixed events, e = 1–24** (table `0x004CBD7C`):

<!-- rows -->
| e | Sound | Unit |
|---|---|---|
| 1 | 235 `item_pickup` | U |
| 2 | 7 `cursor_level_up` | none |
| 3 | 10 `cursor_convert_ready` | none |
| 4 | 11 `cursor_convert_item` | none |
| 5 | 12 `cursor_identify_ready` | none |
| 6 | 13 `cursor_identify_item` | none |
| 7 | 2,230 `player_townportal_cast` | U |
| 8 | 2,231 `player_townportal_enter` | U |
| 9 | 9 `cursor_durability_break` | U |
| 11 | 228 `item_key_used` | U |
| 19 | class `impossible` | U, speech |
| 20 | class `cantuseyet` | U, speech |
| 21 | class `needmana` | U, speech |
| 22 | class `needkey` | U, speech |
| 23 | class `cantcarry` | U, speech |
| 24 | class `notintown` | U, speech |
| 10, 12–18 | none | |

6. **Speech guard** (e = 19–24): nothing if `speaking(U)`; nothing if
   the id equals `[0x007C88C8]` and C − `[0x007C88C4]` < 75; else
   `[0x007C88C4]` := C, `[0x007C88C8]` := id, then the request. The
   guard is global, not per unit.
7. After any request of §3 the overhead text of the *requested* id is
   shown (`0x004B9CA0`, `0x004A0200`; not audio).

### 4. Mode sounds

#### 4.1 Dispatch (`0x004CC5B0(U, m, explicit)`)

1. Every client mode set that changes the mode (`0x00480E70` →
   `0x00624690` non-zero) calls it with explicit = 0: the mode used is
   U's current mode (`0x004CA380`). The player mode machine
   `0x00461250` also calls it with explicit = 1 and m = 3 or 19
   (`0x00461469`), and the monster machine `0x004AFF60` once
   (`0x004B046E`).
2. **Player**, by m: 0 DT, 4 GH, 19 KB → hit/death (§4.2); 7 A1, 8 A2 →
   attack (§4.3 r1, U, m); 9 BL → block (§4.4); 12 KK → 255
   `weapon_kick_1` on U, delay 4 for class 6 (assassin) else 0; 15 S3 →
   attack with m = 15 for class 4 (barbarian) only; 16 S4 → attack with
   m = 16 for class 6 only; other modes nothing (table `0x004CC74C`).
3. **Monster.** First the mode conversion, for i = 1, 2, 3 in order:
   if m = `CvtMo<i>` and `CvtSk<i>` < 0, m := `CvtTgt<i>`
   (monsounds +0x7C/+0x80/+0x7D, +0x84/+0x88/+0x85, +0x8C/+0x90/+0x8D).
   Then by m: 0 DT, 3 GH, 13 KB → hit/death; 4 A1, 5 A2 → attack
   (§4.3 r2); 8–11 S1–S4 → skill voice (§4.5); 12 DD → dead (§4.6);
   others nothing (table `0x004CC774`). No `monsounds` record
   (`0x004CA410`) → nothing.
4. Objects (type 2) are not handled here (§7); other types nothing.
5. Order in one mode set: these requests come before any skill start
   sounds of the same mode set (§8 r1).

#### 4.2 Hit and death (`0x004CC410(U, m)`)

1. Monster with state 146 (`0x00639DF0(U, 0x92)`): nothing at all.
2. **Impact** (`0x004CA560(U, h = U+0xB0, m)`): a = by `h & 0xF`: 1 →
   321 `impact_punch_1`; 2–5 → 341 `impact_blade_swing_1`; 6, 7 → 335
   `impact_blade_thrust_1`; 8, 9 → 329 `impact_blunt_1`; 10, 11 → 347
   `impact_arrow_1`; 12 → 325 `impact_claw_1`; else 0. b = by `h &
   0xF0`: 0x10 → 379 `impact_double_1` if a ≠ 0; 0x20 → 356
   `impact_fire_layer_1` if a ≠ 0 else 353 `impact_fire_1`; 0x30 → 359
   `impact_cold_1` if a = 0, else 362 `impact_cold_layer_1` if U is
   neither player nor monster or m ≠ 0; 0x40 → 372
   `impact_lightning_layer_1` if a ≠ 0 else 368; 0x50 → 376
   `impact_poison_1` if a = 0 or U is neither player nor monster or m ≠
   0; 0x60 → 382 `impact_stun_1`; 0x70 → 395 `impact_bash`; 0x80 →
   2,390 `paladin_thorns_hit`; 0xB0 → 391 `impact_goo_1`; else 0. A
   monster whose `monstats2.critter` is set (`0x004638A0(class, 13)`)
   gets a = b = 0.
3. request(a, U), then request(b, U); if a's handle ≠ 0 and U is P:
   volume 255.
4. Monster and m = 0: stop its own voices (§4.6 r1) and release its
   sample locks (`0x004CC160(U, −1)`).
5. Nothing more if U has state 1 (`freeze`) or `h & 0xF0` = 0xA0.
6. **Monster voice**: m = 3 or 13 → unless `speaking(U)`,
   request(`HitSound`, U, d = `HitDelay`); else (death)
   request(`DeathSound`, U, d = `DeaDelay`). Id 745 `diablo_death` also
   requests 746 (d 106) and 747 (d 193) on U. Id 0 → no request.
7. **Player voice**: m = 4 or 19 → unless `speaking(U)`, class `hit`
   on U with d = 2; else (death) class `death` with d = 1.
8. Always at the end (also when r6/r7 requested nothing): U+0x7C := C,
   `[0x007C88BC]` := C. (Not after r1 or r5.)

#### 4.3 Attack

1. **Player** (`0x004CB6A0(U, m, 1)`): h = U's weapon hit class
   (`0x00623C20`). Swing table (`0x00727ED8`, pairs id/frames, index h
   & 0xF): 0 → none; 1 → 251 `weapon_punch_1`/6; 2 → 262/6; 3 → 268/6;
   4 → 274/8; 5 → 280/8; 6 → 286/6; 7 → 292/8; 8 → 268/6; 9 → 298/6;
   10–13 → none. request(id, U, d = swing(U, h)) (d = 0 when the third
   argument is 0; all callers in §4.1 pass 1). U+0x7C := C.
2. **Monster** (same function), slot N = 1 for m = 4, else 2:
   `AttackN`, `AttNDel`, `AttNPrb`, `WeaponN`, `WeaNDel`, `WeaNVol`.
   1. ok := `AttackN` ≠ 0. If `AttNPrb` < 100: ok := false when C −
      `[0x007C88BC]` < 15; and if `speaking(U)` skip to r2.3.
   2. If ok: draw roll(100); if it is < `AttNPrb`: detach U's playing
      `Neutral` sounds (§6 r1 group, `0x004CB220(U, 1)`: force when the
      request still has units, else fade to 0 over 6), request(`AttackN`,
      U, d = `AttNDel`), U+0x7C := C, `[0x007C88BC]` := C. (Prb ≥ 100
      still draws.) Exact (answers TR-4): `0x004CB220(U, f)` walks U's
      request list (newest first, `sound-table.md` §5 r6) and takes
      every handle whose request's current id has the group base of
      `Neutral` (monsounds +0x68), playing or not. With f = 1: if that
      request has **more than one** unit (`0x004B9BE0`; possible
      through compound merges, `sound-table.md` §5 r2) → detach(h, U,
      force) (§1 r3: U leaves, the others keep it); else → fade(h, 0,
      delay 0, len 6) (`sound-table.md` §5 r7: len raised to `Fade
      Out`, stop flag set; U stays in its list). With f = 0 every match
      is detached with force. A handle whose request is gone reads id
      0 (group base 0) and matches only when `Neutral` is 0; detaching
      or fading a missing request does nothing.
   3. Always: request(`WeaponN`, U, d = (`WeaNDel` × 256 + 128) /
      max(U+0x4C, 1)); if a handle: volume `WeaNVol`.

#### 4.4 Block (player m = 9, `0x004CB860`)

By `U+0xB0 & 0xF`: 1–9 → 398 `block_weapon_1`; 10, 11 → 401
`block_arrow_1`; else 0. request(id, U); U+0x7C := C; `[0x007C88BC]`
:= C.

#### 4.5 Monster skill voice (`0x004CB890(U, m)`)

id = `Skill1..4` for m = 8..11. id 0 → nothing. id 2,692
`creature_chicken_1` only if C − U+0x7C ≥ 25. request(id, U); U+0x7C
:= C; `[0x007C88BC]` := C.

#### 4.6 Monster dead (m = 12, `0x004CB2C0`)

1. For each request in U's list whose group base equals one of the
   group bases of `Attack1`, `Attack2`, `HitSound`, `Neutral`,
   `Skill1..4`, `Init`, `Taunt`, `Flee`: detach with force (§1 r3).

### 5. Footsteps (`0x004CAF60(U)`)

1. Called every client update: for players from `0x00463390` only when
   the mode's movement entry (table `0x00711E00`, 12 bytes per mode,
   first dword) is 1: modes 2 WL, 3 RN, 6 TW, 19 KB; for monsters from
   `0x004B13A0` after the idle voice (§6 r1). Monsters step only in
   modes 2 WL and 15 RN, except class 15 (`foulcrow1`) also in mode 1,
   and class 110 (`vulture1`) only in mode 8.
2. s = U+0x4C; s = 0 → nothing. Defaults: count n = 2, offset o = 0,
   layer = 0, layer chance p = 100. Monster with record: n = `FsCnt`, o
   = `FsOff` × 256, layer = `FootstepLayer`, p = `FsPrb`; n = 0 →
   nothing.
3. F = U+0x48, f = U+0x44. period = F / (n × s), elapsed = C − U+0x84
   (unsigned). If n > 1 and elapsed < ⌊2·period / 3⌋ → nothing.
4. step = F / n. dist(x) = circular distance of x and o modulo step
   (`0x004E4180`: both reduced mod step, masked if step is a power of
   two; min of |b − a|, |b − a − step|, |a − b − step|). Fires only if
   dist(f) < dist(f + s) and dist(f) < dist(f − s).
5. id = footstep base (player: class record; monster: `Footstep`); id
   0 → nothing. Material k (§5 r8): + 4·(k − 1) for k = 2–6. Running
   (player mode 3, monster mode 15): + 24.
6. request(id, U); if a handle: v = 255 for P, else 200; if elapsed >
   ⌊3·period / 2⌋: v := trunc(v × 160 / 255) (255 → 160, 200 → 125);
   volume v.
7. If layer ≠ 0: draw roll(100); < p → request(layer, U). Then U+0x84
   := C (also when the request returned 0).
8. **Material** (`0x004CADB0`): default k = `soundenviron.Material 1`
   of the current environment (`audio/environment.md` §1) if 1–6, else
   1. Then the floor tile under U: U's room, tile ((x − room x) / 5, (y −
   room y) / 5) in its floor list (`0x004CACE0`); not found → the same
   search in the rooms near it (`0x00619790`) in their order; none →
   default. No room → k = 0 (dirt). A found tile's DT1 material flags
   (`formats/dt1.md` +0x06; `0x004CAD60`) give, first match: 0x20 → 1
   dirt, 0x08 → 2 istone, 0x10 → 3 ostone, 0x40 → 4 sand, 0x80 → 6
   wood, 0x400 → 5 snow; none → default.
9. **Integer types** (answers TR-5; `0x004CAF60`, `0x004E4180`). s is
   U+0x4C read as **signed 16-bit** (`movsx`); F = U+0x48 and f = U+0x44
   are 32-bit. `n × s` is a 32-bit product used as an **unsigned**
   divisor (period = F / (n·s), unsigned; a negative s gives a huge
   divisor, period 0); step = F / n unsigned; `f + s`, `f − s` are
   32-bit wrapping sums (signed values). dist(x): if step is a power of
   two (or 0), x and o are masked with step − 1 (two's complement, so a
   negative x reduces like a mathematical modulo; step 0 masks with
   0xFFFFFFFF, i.e. no reduction); otherwise both are reduced by
   **signed** division (`idiv`: the remainder has the sign of x, so f −
   s < 0 gives a negative remainder). Then a = x mod, b = o mod: the
   three candidates |b − a|, |b − a − step|, |a − b − step| are signed
   32-bit with absolute value, and the minimum is taken with signed
   compares; the step test of r4 compares signed (`jge`). The elapsed
   tests of r3 and r6 are unsigned (`C − U+0x84`, ⌊2·period/3⌋,
   ⌊3·period/2⌋ on u32).

### 6. Monster idle voices

1. **Neutral** (`0x004CB460`, monsters, every client update, before
   footsteps): base class b. Only in modes 1 NU, 2 WL, 15 RN, plus mode
   8 for b = 110 and mode 11 for b = 136 (`batdemon1`). With record R
   and `Neutral` ≠ 0, all of:
   1. no request of U is in `Neutral`'s group (`0x004CA900`; also
      false while the sound system is off);
   2. C − `[0x007C88B8]` ≥ `[0x007C88C0]`;
   3. C − U+0x7C ≥ `NeuTime` and C − U+0x80 ≥ `NeuTime`;
   4. U's level is not a town (`0x0061AB00`, levels 1, 40, 75, 103,
      109) or U has no room;
   5. U within 700 of P (`0x004B9E10`: x² + (2y)² < 490,000, positions
      as `sound-table.md` §8.1 r1);
   6. roll(15) = 0 (drawn only when 1–5 hold).
   Then request(`Neutral`, U); `[0x007C88B8]` := C; U+0x80 := C;
   `[0x007C88C0]` := uniform(30, 90).
2. **Init** (`0x004CC380`, from `0x00466360`): monster with record: if
   `Init` ≠ 0, U is not dying/dead (`0x00464820` = 0) and U+0x80 = 0:
   request(`Init`, U); `[0x007C88B8]` := C; U+0x80 := C;
   `[0x007C88C0]` := uniform(30, 90). Else if U's mode is 8: skill voice
   (§4.5).
3. **Taunt**: event 16 (§2 r2).
4. **Flee** (event 17, `0x004CB950`): monster with record and C −
   `[0x007C88BC]` ≥ 4: request(`Flee`, U, d = 6 + roll(3)); U+0x7C :=
   C; `[0x007C88BC]` := C.

### 7. Object mode sounds (`0x004CB460`, objects)

1. Called on object units (callers `0x004BCF60`, `0x004BD7C0`,
   `0x004BD860`, `0x004BD900`, `0x004BD9C0`, `0x004BDFF0`); m = current
   mode, class c. Record: `object-sounds.tsv` row c (453 classes; static
   `[0x007295F8 + 4·c]`, c ≤ 572 else fatal).
2. Class 26 (Cain's gibbet), m = 0, U+0x80 = 0, distance(U, P) < 20
   (`0x006416D0`, its units): request(3,671 `cain_act1_help`, U); U+0x80
   := C.
3. If U+0x70 is set and m = U+0x74: stop here.
4. **Mode loop** s (`0x004CB3E0`): class 61 → `cairn_stone_1..5` for
   m = 1..5 (table `0x00728338`; 0 for other m); else `loop_a` if
   `loop_a_mode` = m or 8, else `loop_b` if `loop_b_mode` = m or 8,
   else 0 (both 0 → 0). s ≠ 0: request(s, U) unless a request of U is
   already in s's group. s = 0: detach U from all its looping requests
   (`0x004CAA10`, no force).
5. **Transition** t (`0x004CB380`): only if U+0x70 is set or c = 59
   (town portal): t = `mode<m>` of the row (m ≤ 7), except for rows
   with `ordered` = 1 (wells) where t = 0 when m < U+0x74. t ≠ 0 →
   request(t, U).
6. U+0x74 := m, U+0x70 := 1. So the first call only sets the loop
   (and class 59's transition).

7. **Classes without a record** (answers TR-1): 120 of the 573 classes
   have a null pointer at `[0x007295F8 + 4·c]` (453 non-null, the TSV
   rows). `0x004CB3E0` and `0x004CB380` then return 0 (`0x004CB3FC`,
   `0x004CB39F`), so the class behaves as an all-zero row: no
   transition; loop s = 0 → U is detached from its looping requests
   (r4, normally none); U+0x74 and U+0x70 are still updated (r6). Class
   ≥ 573 or mode ≥ 8 is fatal (`0xE72`, `0xE86`). The class-61 cairn
   table is used only when the record is non-null (it is: `0x00728368`).
8. **Cairn loop table** (answers TR-2; `0x00728338`, 8 dwords by mode):
   mode 0 → 0, modes 1–5 → 413–417 (`cairn_stone_1` … `cairn_stone_5`),
   modes 6, 7 → 0. Class 61's record gives only the mode-6 transition
   418 (`object-sounds.tsv`). The table index is the current mode, so a
   stone in mode 1–5 loops the matching `cairn_stone_<m>`.

`object-sounds.tsv`: `class`; `mode0`–`mode7` transition ids;
`loop_a`, `loop_a_mode`, `loop_b`, `loop_b_mode` (mode 8 = any);
`ordered` (1 = the shared well record `0x00729328`). Ids are `sounds`
line indices, 0 = none. Dumped from `Game.exe`.

### 8. Skills, missiles, states

1. **Skill start** (`0x004C6140(skill S, unit U, item, …)`, from the
   mode machines via `0x004C6660`, `0x004C6EB0`, `0x004C6F40`): after
   the start function `cltstfunc` (+0xF2) and the mode set; if the
   start function returned 0 and `stsuccessonly`: no sounds. Then in
   order:
   1. s = `stsound`; cast from an item (item ≠ −1) with
      `ItemCastSound` > 0 → s = `ItemCastSound`. s > 0: request(s, U,
      d = `stsounddelay` ? swing(U, h) − 3 : 0), h = U's weapon hit
      class (signed: a negative delay is passed as is).
   2. `weaponsnd`: if the swing table gives an id for h
      (`0x004CA4C0`), request(it, U, d = swing(U, h)).
   3. `stsoundclass` > 0, not replaced by an item cast sound, U a
      player whose class = the skill's `charclass` (+0x0C):
      request(`stsoundclass`, U, d as in r1.1).
   d2rs (REC-1683): the start is taken from the unit's mode request,
   code 0x15 / 0x16 (`client/model.md` §8 r4), skill = record entry 0,
   after the mode sounds of a mode change; the start function result is
   taken as non-zero, and the local player's request with level 0
   (the server's first message of a cast) plays nothing.
2. **Skill do / target** (`0x004C6680`, `0x004C6930`, `0x004C6AC0`;
   from S→C 0xA3 `0x0045D5E0` and item casts `0x004CA060`): after the
   skill's client do function returns non-zero: `dosound` > 0 →
   request on the caster; `tgtsound` > 0 and the caster has a target
   (`0x004648F0`) → request on the target. `ItemCastSound` ≥ 0 on item
   casts (`0x004CA060`). `prgsound` (+0x10E) on the unit from the
   progressive function `0x004D9090`. `dosound a`/`dosound b`:
   `0x004C9B40`, `0x004F4590` (open question 5).
3. **Missiles**: `TravelSound` (+0x12) on the missile after its client
   init function, in the client missile create `0x004CD540` (185 call
   sites); `HitSound` (+0x14) ≥ 0 on the missile when its client hit
   function returns non-zero (`0x004D2D70`); `ProgSound` (+0x34) on the
   missile from the client progressive functions `0x004CE850` (missile
   315 only; corrected: elapsed 315 of function 29, `triggers-2.md`
   §16), `0x004D39C0`, `0x004D5950`, `0x004D5DD0` (conditions:
   open question 6).
4. **States** (S→C 0xA7, 0xA8, 0xAA → `0x004D9B20`; 0xA9 →
   `0x004D9C30`): state on: if `notondead` and U is a dead monster
   (mode 12) or player (mode 17), only the state bit is set; else, if U
   did not have the state, `onsound` ≥ 0 → request on U. State off: if
   U had the state, `offsound` ≥ 0 → request on U.

### 9. Items

1. **Item mode 4** (on cursor, `0x004C1910`): 235 `item_pickup`, none.
2. **Item mode 5** (dropping to the ground, `0x004C18B0`): request(216
   `item_flippy`, item); then the drop sound (`0x004C16D0`): base row
   `dropsound` > 0 with d = `dropsfxframe` (0 → 12); a unique
   (quality 7) row with `dropsound` > 0 replaces it (its
   `dropsfxframe`, 0 → keep); else a set (quality 5) row likewise. Drop
   sound > 0 → request(it, item, d); handle → volume 180.
3. **Place** (inventory, equip, belt, stash, cube, trade grids:
   `0x0048FFE0`, `0x00490780`, `0x00490BA0`, `0x00490FC0`,
   `0x004912A0`, `0x00498870`, `0x004B2650`, `0x004BFA70`,
   `0x004C2970`): request(place sound, none), place sound
   (`0x004C1D60`) = the drop sound of r2 without the frame. `0x0048FFE0`
   also plays 235 `item_pickup` (none) at one site (`0x004902CE`).
4. **Use** (`0x00487740`, `0x00498A90`, through `0x004C1E20`): use
   sound = base `usesound` if the base `dropsound` > 0, else 0; a
   unique row with `dropsound` > 0 gives its `usesound`; else a set row
   likewise. Request on none. (The `dropsound` test is reproduced.)
5. **Gold**: 221 `item_gold`, none (`0x004B53F0`, `0x004B6390`); and
   `[0x007C0D5B]` (a stored id, `0x004B545C`).
6. Server item events reach sounds as player events 1, 9, 11, 19–24
   (§3) and 0x2C event 93.

### 10. NPC speech

1. **Greeting** (`0x004E0590(N, mode)`, event 18 mode 1,
   `0x004B4FD0`, `0x004B66B0`): record by N's class (`0x004E0370`, 6
   dwords: greet, inactive, time, return, last, tick; 35 classes
   sharing 28 records, e.g. 148 `akara`: `akara_greeting_1`,
   `akara_greeting_inactive_1`, `akara_greeting_time_1`,
   `akara_greeting_return`; greet is 0 for Warriv and the five Act V
   records, time is 0 for Halbu and Jamella). No record → 0. Mode 2 → `return` (no draw,
   no variant). Else up to 20 attempts:
   1. s = greet; if mode = 0 and inactive ≠ 0: roll(2) = 0 → s =
      inactive.
   2. If time ≠ 0 and (s = 0 or roll(3) = 0): s = time + 0 / 1 / 2 for
      day phase 1 / 2–3 / other (`audio/environment.md` §1 r3).
   3. pick = variant(s) (`sound-table.md` §4 r3, its draws).
   4. pick = last → next attempt (the 20th pick is kept).
   last := pick, tick := C. The caller requests pick with flags 1
   (exact) on P: event 18 directly; `0x004B4FD0` and `0x004B66B0`
   (NPC interaction) first detach N's skill voices (`0x004CB190`,
   force); `0x004B66B0` keeps the handle (`[0x007C0DB8]`).
2. **Dialog line** (`0x004A10E0(N, key)`, `0x004A1320`): fade the
   previous dialog line to 0 over 4 (if playing) and stop all speech
   (§1 r4); if NPC speech is enabled (`[0x0072AE24]`, r3), s =
   `npc-speech.tsv` sound of the first row whose key = key (864 rows,
   table `0x0072B0E0`; key = the dialog text id; 1 duplicate key, first
   wins); s ≠ 0 → detach N's skill voices, request(s, P, d = 5);
   remember the handle and id.
3. **NPC Speech option** (`0x0047CEF0`, options menu): 0 → enabled,
   text off; 1 → disabled, text on; 2 → both on. `[0x0072AE24]` starts
   1 and changes only there (the stored setting is not applied at
   start; open question 7). Greetings (r1) ignore it.
4. Other fixed NPC lines: 4,603 `wussie_cheer_1` / 4,607
   `wussie_help_me` on class 534 (`0x004B3380`, by `0x004AE130`),
   3,983 `guard_halt`, 4,560 `nihlathak_hurryup` (`0x004B4380`);
   conditions open question 8.
5. **Greeting records as data** (answers TR-3): `npc-greetings.tsv`,
   one row per NPC class that `0x004E0370` maps to a record (35 rows,
   28 distinct records). Columns: `class` (monstats row, decimal),
   `monstats` (its `Id`, for reading only), `record` (the record's
   address in `Game.exe`, hex `0x…`; classes with the same address
   share the record's runtime `last` / `tick` dwords at +0x10 / +0x14,
   so e.g. all six Cain classes avoid each other's last pick), then
   `greet`, `inactive`, `time`, `return`: `sounds` line indices,
   decimal, 0 = none. Every other class has no record (r1: 0). The
   mapping is a switch on the class (`0x004E0386`–`0x004E0432`: 405
   direct, 146–297 through the byte table `0x004E04C4`, 511–521
   through `0x004E055C`). Five records repeat `greet` as `return`
   (Cain, Elzix, Halbu, Jamella, Tyrael). Dumped from `Game.exe` with
   `pefile` (our script). Parsing: header row as above; exactly 7
   tab-separated cells per row; numbers decimal except `record`; rows
   in ascending `class`, no duplicates.
6. **Dialog table lookup** (`0x004E0650(key)`, used by r2; answers
   TR-7's duplicate): when `[0x0072AE24]` is 0 → 0. Else the 8-byte
   entries at `0x0072B0E0` (sound u32, key u16, 2 pad bytes) are
   scanned from the first until an entry with sound 0; the first whose
   key equals the 16-bit key wins. Key 506 is listed twice (rows
   `order` 37 → 3,533 and 38 → 3,534): 3,533 is played, 3,534 never.
   The initial value of `[0x0072AE24]` in the image is 1; its only
   writer is the setter `0x004E0690`, called only from the options
   menu `0x0047CEF0` (`0x0047CF17`, `0x0047CF28`, `0x0047CF3C`)
   (answers open question 7: no start-up path applies the stored
   `NPC Speech` setting).
7. **Mode 2 and the record state** (`0x004E05AA`): mode 2 returns
   `return` before the attempt loop and writes neither `last` (+0x10)
   nor `tick` (+0x14); only modes 0 and 1 set them, after the last
   attempt (`0x004E062E`, `tick` := C). The day phase of r1.2 is read
   once per attempt that reaches it (`0x0061C220([0x007A0634])`).
   "N's skill voices" of r1 and r2 (`0x004CB190`): `audio/triggers-2.md`
   §19 r4.

### 11. UI sounds

All are requests with no unit, delay 0 (call sites whose id is a
constant in the disassembly; `client/ui.md` §B8 owns which control is
which). Id 6 is also requested by every screen message added
(`0x0049E3A0`, `ui/messages.md` §2 r3), recorded at T 0 for the empty
line of the local player's own join (S→C 0x5A code 2, return
`0x0049E58A`); d2rs requests it for the 0x5A lines (REC-1681):

<!-- rows -->
| Id | Sound | Sites |
|---|---|---|
| 1 | `cursor_pass` | 24 (menus, panels, `0x0047AA60` … `0x004C2C80`) |
| 2 | `cursor_select` | 7 |
| 3 | `cursor_error` | 4 (`0x00489360`, `0x004897E0`, `0x004BFC50`, `0x004C0550`) |
| 4 | `cursor_button_click` | 26 |
| 5 | `cursor_point_drop` | 1 (`0x004AB7E0`) |
| 6 | `cursor_switch` | 8 |
| 15 | `cursor_repair_item` | 1 (`0x004B2650`) |
| 16 | `cursor_hostile` | 1 (`0x0049E8F0`) |

S→C 0x5D (`0x004A2CB0`, flags byte f, code c, value v): f bit 0 and c
= 33 → 237 `item_potion`; f bit 1: c = 4 → 241 `item_ring`; c ∈ {8,
15, 18, 22, 35} → 7 `cursor_level_up`; c = 32 → 217 `item_gem`; c = 33
→ 243 `item_scroll`; f = 0x10: c = 10 → 2,456 then 2,474; c = 33 → id
v. All none (full dispatch: `client/msg-ui.md` §1; open question 9). Only the first matching row of
`client/msg-ui.md` §1 r2 runs: with f bit 0 set the bit-1 rows are not
reached (f = 3, c = 33 plays 237 only), and f = 0x10 rows only when bits
0 and 1 are clear.

### 12. Other fixed requests

| Id | Sound | Where | Unit |
|---|---|---|---|
| 396, 397 | `impact_steal_life`, `impact_steal_mana` | `0x00464E50` | yes |
| 202 | `event_thunder_1` | weather `0x00473910` | none |
| 452 | `andariel_quake_loop` | screen shake `0x004769D0` (`render/camera.md` §8) | none |
| 2,231 | `player_townportal_enter` | `0x0049D010` | none |
| 2,671 | `shrine_portal` | `0x0049FBA0` | none |
| 4,640 | `monster_diablo_taunt_ex` | `0x0049EB10` (missing file: silent, `sound-table.md` §11) | none |
| 4,638 | `monster_diablo_taunt_1` | `0x004D6540` | none (corrected: `0x004D6574` passes no unit) |
| 2,458 | `necromancer_corpseexp_1` | `0x004AD0C0`, `0x004AD1A0`, `0x004ADCE0` | yes |
| 1,308 / 1,311 / 1,314 / 1,317, 2,419, 790 | minion deaths, fireball impact, druid pod death | `0x004AFF60` (monster mode machine) | yes |
| 2,517 | `barbarian_leap_land` (+ a running footstep) | `0x004C8970` | yes |
| 1,830 | `spider_web_1` | `0x004E2D40` | yes |

Their conditions (open question 10, answered here and in
`triggers-2.md` §13):

1. 396 / 397: `0x00464E50(U, overlay o, n)` (from the monster mode
   machine `0x004AFF60`, 5 sites) first creates overlay o on U (type 1
   with n if n ≥ 1, else type 2; no seed draw), then o = 151 → 396
   `impact_steal_life`, o = 152 → 397 `impact_steal_mana`, on U.
2. 452: the shake level `l = trunc(a × 255 / 20)` clamped to 0–255 (a =
   the shake amplitude of `render/camera.md` §8, `0x004769D0`, called
   twice per drawn frame from `0x00476D40`). l > 0 with no loop request
   (`[0x007B8D2C]` = 0) → request 452 with no unit, then volume := l
   (`0x004B9B50`); with a request, its volume moves toward l by at most
   6 per call (`0x004B9B20` read); when the new volume is 0 the request
   is stopped (`0x004BA840`: stop flag, plus a `Fade Out` fade if it is
   playing, as `sound-table.md` §6.2 r3's switch-off) and
   `[0x007B8D2C]` := 0; otherwise volume := the new value. Wall-clock
   driven (the shake envelope).
3. 2,231 at `0x0049D010`: the waypoint panel row choice
   (`ui/menus.md` mouse-up rule), no unit.
4. 2,671 at `0x0049FBA0`: the deciphered Scroll of Inifuss panel (UI
   state 16, item code `bkd `, from `0x0049FF10`): a step counter
   advances when more than 50 ms of `GetTickCount` time passed; each of
   the 5 symbols plays 2,671 (no unit) on the step where counter − its
   start (`0x00722F08 + 4i`) = 1. Wall-clock driven.
5. 4,640 at `0x0049EB10`: S→C 0x5A EventMessage type 18 (`0x0049F35B`;
   handler `0x0045E070`, also `0x0048A630`), after the message, the
   level effect `0x0061C240` and a shake: no unit (file missing, silent).
6. 4,638 at `0x004D6540`: client missile function 37 (table
   `0x0072A398`, missile 372 `diablo appears`): at frames left 150 a
   screen shake (`0x00476A80`), at frames left 50 the request, no unit.

The rest — 2,458 (`0x004AD0C0`, `0x004AD1A0`, `0x004ADCE0`, reached
through tables `0x0072509C`, `0x00724EE4` and `0x004D93A0`), the
`0x004AFF60` death sounds, 2,517 (`0x004C8970`) and 1,830
(`0x004E2D40`, from `0x004807E9`) — is `audio/triggers-2.md` §13.

**Thunder, draws** (`0x00473910`, weather; the timer and when it runs
are the weather spec's): at a thunder step (`0x004739B4`) the code draws
on the **local player's client seed** with `0x00472280(seed, lo, n)` =
lo + roll(n) (the same generator as §1 r7; n ≤ 0 → lo, no step): first
the next thunder timer = 500 + roll(1500); then, if the flag
`[0x00712B4C]` is set (image value 1; when 0 it is set to 1 and no
sound plays this time), delay = 25 + roll(50), h = request(202, none,
d = delay); if h ≠ 0: y = −200 + roll(400), then x = −200 + roll(400)
(y is drawn first), and the position of h := (x, y, 0 + 640.0)
(`0x004B99A0`, `sound-table.md` §5 r8). These draws interleave with the
sound draws of the same tick (`sound-table.md` open question 3).

## Constants & data dependencies

Static `Game.exe` tables: class records `0x0072A008` (§3 r1), swing
table `0x00727ED8` (§4.3 r1), event tables `0x004CC108`, `0x004CBD68`,
`0x004CBD7C`, mode tables `0x004CC74C`, `0x004CC774`, player movement
`0x00711E00`, object records `0x007295F8` (`object-sounds.tsv`), cairn
`0x00728338`, NPC records `0x0072AE28`–`0x0072B0C8`, dialog lines
`0x0072B0E0` (`npc-speech.tsv`). Columns: `monsounds` (all),
`skills` +0xF2, +0xFC–+0x10E, +0x122, flag bits 12–14; `missiles`
+0x12, +0x14, +0x34; `states` +0x26, +0x28, bit 39; item rows +0x124,
+0x126, +0x128, uniques +0x84–+0x88, sets +0x82–+0x86;
`monstats2.critter`; `monstats.BaseId`; `soundenviron.Material 1`.
Fixed numbers: 75 (speech repeat), 15 (attack voice gap), 25 (chicken),
4 (flee gap), 6 + roll(3) (flee delay), 700 (neutral range), roll(15),
uniform(30, 90), 12 (quest-line delay, drop frame default), 120, 180,
200, 255, 160/255 (volumes), 20 (greeting attempts, Cain distance).

## Randomness

All draws are on the local player's client unit seed (§1 r7), in this
order within one call:

| Rule | Draws |
|---|---|
| §4.3 r2 (monster attack) | roll(100), only when the voice is allowed; then the variant draws of `AttackN` and `WeaponN` at their channel starts |
| §5 r7 (footstep layer) | roll(100), only if a layer exists, after the step request |
| §6 r1 (neutral) | roll(15) after the other conditions; if 0, uniform(30, 90) = roll(61) |
| §6 r2 (init) | roll(61) after the request |
| §6 r4 (flee) | roll(3) before the request |
| §10 r1 (greeting) | per attempt: roll(2) (mode 0 with inactive), roll(3) (time ≠ 0 and s ≠ 0), then the variant draws |

Within one client update the order follows the unit update order of
`client/model.md` §5 r3 (monster: neutral, then footsteps). Variant
draws of requests happen later, in the sound update's request order
(`sound-table.md` §6.2). How these interleave with other users of the
seed is `sound-table.md` open question 3 (answered statically in
`sound-table-2.md` §14). Also on this seed, outside `0x004E40A0`:
Nihlathak's hurry-up deadline (open question 8, `roll(30)` via
`0x0045C3E0`, once per interaction, during a drawn frame) and the
thunder draws (§12).

## Edge cases & original bugs

1. Usesound only when the row's dropsound is set (§9 r4).
2. A monster attack voice with `AttNPrb` ≥ 100 still draws roll(100)
   (§4.3 r2.2).
3. The impact sounds play even when the voice is suppressed by
   `freeze` (§4.2 r5) — and a monster in state 146 gets neither.
4. Stinger-held quest lines are not checked for `speaking` (§3 r4,
   `audio/environment.md` §3).
5. The global speech guard (§3 r6) is keyed only by id: two players'
   identical lines within 75 updates play once.
6. The level-up sound of event 2 and the S→C 0x5D level-up are
   independent (both can play).
7. Footsteps with an unknown floor give dirt (k = 0 adds nothing).
8. Greeting with no greet line and no time line (Warriv, Act V NPCs,
   mode 1) picks variant(0) → request 0 → silence.

## Test vectors

Synthetic (CI, d2rs rule functions):

| Input | Expected | Source |
|---|---|---|
| player event 21, amazon, not speaking, `[0x7C88C8]` = 0 | request(2,957, U); guard (C, 2,957) | §3 r5, r6 |
| same again 74 updates later | no request | §3 r6 |
| same again 75 updates later | request | §3 r6 |
| player event 26, sorceress | request(3,249 + 2 = 3,251, U) | §3 r3 |
| player event 38, amazon | request(2,966, U, d 0) | §3 r4 |
| player event 39, amazon | request(2,967, U, d 12) | §3 r4 |
| player A1, hit class 5, speed 256 | request(280, U, d = (8·256+128)/256 = 8) | §4.3 r1 |
| player A1, hit class 2, speed 0 | d = 1,664 | §1 r9 |
| impact h = 0x22 on a monster, m = 3 | requests 341 then 356 | §4.2 r2 |
| impact h = 0x30, player, m = 0 | 359 only | §4.2 r2 |
| impact h = 0x35, monster, m = 0 | 341, then nothing | §4.2 r2 |
| footstep F = 2048, s = 256, n = 2, o = 0, f = 1024, elapsed 2 | period 4, ⌊8/3⌋ = 2 ≤ 2; dist 0 < 256, < 256 → step | §5 r3, r4 |
| same, elapsed 1 | nothing | §5 r3 |
| footstep elapsed 7, period 4 | volume 200 → 125 (monster) | §5 r6 |
| object class 27 (door), m 0 → 1 | transition `object_door_wood_open`; no loop | §7, tsv |
| object class 39 (brazier), first call m 1 | loop `object_fire_loop_brazier` (a), no transition | §7 r6 |
| well (ordered) m 2 → 1 | no transition | §7 r5 |
| greeting class 148, mode 2 | `akara_greeting_return`, no draw | §10 r1 |

Real (`#[ignore]`, `D2_GAME_DIR`): `object-sounds.tsv` has 453 rows,
pointing at 115 distinct record addresses in `Game.exe` that hold 106
distinct contents (the TSV has 106 distinct rows; corrected for TR-7:
"115 distinct records" counted addresses); `npc-speech.tsv` 864 rows; every id in both
is < 4,699; class record ids resolve to the names in §3 r1
(`amazon_hit_1` … `light_walk_dirt_1`).

### Checks (hook addresses for `record_sound.py`, `client/audio.md` §B7)

| Check | Hook | Record | Proves |
|---|---|---|---|
| request log | entry `0x004B9A00` | T `[0x7BC9BC]`, C `[0x7A0498]`, ECX, EDX (type +0, GUID +0xC), the 3 stack words, return value | §2–§12: identical sequence of (C, id, unit, delay, flags, offset) |
| volume sets | entry `0x004B9B50` | handle, value | §1 r2 uses |
| RNG | seed at `[0x7A6A70]+0x20/+0x24` before/after each request and at `0x004E40A0` entry (ECX = n) | n, result | Randomness order |
| footsteps | `0x004CAF60` entry | U, +0x44, +0x48, +0x4C, +0x84, C | §5 timing |
| mode sounds | `0x004CC5B0` entry | U, EDX, explicit, mode | §4.1 |

A replay of a recorded session through d2rs must give the same request
log; voices then follow from `sound-table.md`. A static scene recorded
twice must give identical logs first (`client/audio.md` §B7).

## Provenance

Both TSVs re-checked against the 1.14d file image (second pass,
2026-10-07, scratch script): `npc-speech.tsv` equals the 864 8-byte
entries at `0x0072B0E0` (sound u32 at +0, key u32 at +4; entry 864 has
sound 0, the end); `object-sounds.tsv` equals, for each of the 453
classes with a non-null pointer at `[0x007295F8 + 4·c]` (the same 453
classes), the record's 12 dwords (`mode0`–`mode7`, `loop_a`,
`loop_a_mode`, `loop_b`, `loop_b_mode`), with `ordered` = 1 exactly when
the pointer is the well record `0x00729328`. 0 rows differ.

1.14d `Game.exe` (sha256 631066c1…adaaf), Ghidra decompile export and
`tools/ghidra/disasm.py` (register arguments: ECX = id, EDX = unit, three
stack words for every `0x004B9A00` call; call sites listed by our script
over `re/exports/all.asm`): event dispatch `0x0045E110`, `0x004CBDE0`;
player events `0x004CB9C0`; UnitSnd `0x004CA2C0`–`0x004CC72A`; mode set
`0x00480E70`; skill start `0x004C6140`; do/target `0x004C6680`–
`0x004C6DA0`; missiles `0x004CD540`, `0x004D2D70`; states `0x004D9B20`,
`0x004D9C30`; items `0x004C16D0`–`0x004C1E20`, `0x004C18B0`,
`0x004C1910`; NPC `0x004E0370`, `0x004E0590`, `0x004E0650`,
`0x004A10E0`, `0x0047CEF0`; 0x5D `0x0045E540`, `0x004A2CB0`. Static
tables read from the image with `pefile` (our script); names from P
`sounds.txt`, `monstats.txt`, `objects.txt`, `states.txt`; column
offsets from `data/fields.tsv`. Sound ids cross-checked: every table
id resolves to a sound whose name matches its role (e.g. class record
+0x24 = `*_act1_complete_andariel` for all 7 classes). D2MOO and
Riiablo not used (no client sound code there for these paths).
Second pass (2026-10-07, TR-1–TR-7 of `docs/handoff/impl-audio.md`):
`0x004CB220`, `0x004B9BE0`, `0x004BA790`, `0x004BA760` (TR-4);
`0x004CAF60`, `0x004E4180` (TR-5); `0x004CB380`, `0x004CB3E0`,
`0x004CB460` and the tables `0x007295F8`, `0x00728338` (TR-1, TR-2);
`0x004E0370` switch and records `0x0072AE28`–`0x0072B0C8` dumped to
`npc-greetings.tsv` (TR-3); `0x004E0650`, `0x004E0690` (TR-7, OQ 7);
`0x00466360`, `0x0045F190` (OQ 11); thunder `0x00473910`,
`0x00472280`. Recording facts: `docs/handoff/local-buddy-q-rec.md`
entry 74 (id-0 requests).

## Open questions

1. Every rule: confirm with the request log (Checks) on a recorded
   session (town walk, a fight, an NPC talk, item moves).
   Needs recording: the Checks hooks (every `0x004B9A00` call with
   caller, id, unit type/GUID, delay, flags, T, C) plus each S→C 0x2C,
   0xA9 and code-8 mode request with T, over: town walk on two floor
   materials, a fight with a melee and a caster monster (hits, a
   death, a block), an NPC talk and greeting, item pickup / drop /
   identify, a waypoint, a Leap, a fire-enchanted unique's death, the
   options menu sliders; each logged request must match one rule's
   site, id, unit and delay.
2. Where the 0x2C events come from per id and tick (server senders);
   compare the request log's C against the 0x2C packet tick.
   Answered (static, `triggers-2.md` §14): all 78 sites by event; the
   unit keeps one pending event (last wins), flushed in the
   per-client unit update; the client requests at receive.
3. Who writes client unit +0xB0 (hit class of the last hit; read by
   §4.2, §4.4); a write watch during a fight. Partly answered: +0xB0
   is a dword, written only by the client mode machines: player
   `0x00461250` (`0x0046140B`, `0x0046143F`, `0x00461498`, `0x004614EF`,
   `0x0046155E`), monster `0x004AFF60` (`0x004B03BB`, `0x004B043E`,
   `0x004B04A8`, `0x004B0549`, `0x004B05CD`, `0x004B0650`), plus
   `0x00450A99` (`0x00450950`) and the player update `0x004635F4`; the
   values come from the mode event records those machines consume
   (e.g. record +0x08 at `0x00461494`, +0x18 at `0x004B0546`). Which
   S→C message field fills them belongs to `client/msg-units.md`; the
   write watch still settles it end to end. The writer list is
   complete for client code: a scan of every store to `[reg + 0xB0]` in
   `0x00440000`–`0x0050FFFF` finds only these sites plus `0x00447BB0`,
   `0x00449050`, `0x004493E0`, which use +0xB0 as the next link of the
   list at `[0x00798F34]` (not a unit).
4. Event 12: which skill/record `0x006256B0(U, 0x44)` and
   `0x00625D00(·, 350, 0)` select. Answered (`0x004CBE6B`–`0x004CBEB9`):
   U's stat list of state 68 (`evade`); its stat 350
   (`modifierlist_skill`) = the skill that set the state; if U has that
   skill (`0x006439F0`) and its `skills.txt` record exists
   (`0x0045C4B0`), its `stsound` (+0xFC) > 0 is requested on U, delay
   0. Live: Dodge (13), Avoid (18) and Evade (29) all have `stsound`
   2,236 `amazon_dodge_1`; no state 68 or no skill → nothing.
5. `dosound a`/`dosound b` use in `0x004C9B40`, `0x004F4590`.
   Answered: `0x004F4590` is `cltdofunc` 16 (table `0x00727BA8`; live:
   Jab only): when the caster's action frame (unit byte +0x4E,
   `sim/units.md`) is 3, a player caster requests `dosound a` (+0x102)
   and a monster caster `dosound b` (+0x104), on the caster, delay 0,
   when > 0 (Jab: 286 `weapon_1ht_1` / 292 `weapon_2ht_1`). It
   returns 1 either way. `0x004C9B40` is `cltstfunc` 25 (table
   `0x00727A90`; live: Charge 107, SerpentCharge 352): unless it hands
   over to `0x004C7630` (player with a target in reach, `0x00622C40`),
   a player requests `dosound a` (> 0) and a monster the sound of its
   monstats skill slot holding this skill (`0x004F4F40`: slots 0–3 →
   `monsounds` Skill1–Skill4, +0x44–+0x50; other slots or −1 → none),
   on the caster, delay 0 (after its entry checks: skill record, path,
   and not the local player re-casting its current skill with the
   skill flag bit 0 set, `0x006446A0`). Both live skills have no
   `dosound a`.
6. ProgSound conditions per client progressive function (owner: the
   client part of `missiles/missiles.md`). Answered (`triggers-2.md`
   §16): functions 9, 29, 47, 51; 29 tests elapsed 315 (not missile
   315).
7. Answered (§10 r6): no; only the options menu calls the setter.
8. Conditions of `0x004B3380`, `0x004B4380` (Act II guard, Act V
   soldiers, Nihlathak). Answered: `0x004B3380` is the S→C 0x8A
   NpcWantsInteract handler (`0x0045EA40`; type u8@1, GUID u32@2). Unit
   not found → nothing. Class 534 (`act5pow`): monster data +0x3C ≠ −1
   (`0x004AE130`) → 4,603, else 4,607, on U; done. Any other U that is
   not the NPC of the active interaction (`[0x007C0D29]`,
   `[0x007C0D25]`): overlay 0x48 (type 3, no draw) on U; then if U is
   class 331 (`act2guard2`), no interaction is active (`0x004B1620`), the
   client quest record `[0x007C0D43]` exists, slot 12 (Arcane
   Sanctuary) has neither bit 8 nor bit 1 (`0x0065C310`), and no object
   of class 318 (`eunuch harem blocker`) in mode 2 is in the client
   object set (`0x004649D0` with callback `0x004B3360`): 3,983
   `guard_halt` on U. `0x004B4380` runs in the UI pass `0x00456EE0`
   (once per drawn frame); in its branch with the NPC menu flag
   `[0x007C0D67]` set (and `[0x007C0D6B]`, `[0x007C0D63]` clear,
   `ui/menus.md`), when the interaction NPC is class 514 (`nihlathak`)
   and `[0x007C0DBC]` = 0: the first time (`[0x007C0DC0]` = 0) it sets
   `[0x007C0DC0]`, draws `roll(30)` on the sound seed and sets a
   deadline = `GetTickCount` + (roll + 120) × 1,000; then if
   `GetTickCount` < deadline (true at once) it sets `[0x007C0DBC]` and
   requests 4,560 `nihlathak_hurryup` with no unit. So the line plays
   on the first such frame, not after the delay (original bug, kept);
   both flags are cleared by `0x004B23E0` (from `0x00456970`,
   `0x004B32F0`).
9. Answered by `client/msg-ui.md` §1 (full `0x004A2CB0` dispatch; the
   message is `world/quests.md` §6.3's one-quest status: code = chain,
   value = extra). §11's 0x5D paragraph is the sound subset of it.
10. Conditions of the §12 call sites. Answered: §12 r1–r6 and
    `triggers-2.md` §13 (2,458, the death sounds, 2,517, 1,830).
11. Answered: on client creation. `0x004CC380` is called once, at the
    end of the client monster create `0x00466360` (`0x00466716`), whose
    callers are the S→C 0xAC AssignMonster handler `0x0045F190`
    (`0x0045F40C`) and `0x00466730` (`0x00466796`; its callers are
    `0x004667F0`, `0x00466820`, `0x0046C1A0`, `0x0046C320`,
    `0x004A3150`, `0x004AFF60`, `0x004B13A0`, `0x004CD540`). No path
    plays `Init` on first sight; which client creations reach
    `0x00466730` belongs to `client/model.md`.
12. UI control → site mapping for §11 (owner `client/ui.md` §B8).
    Partly answered: the seven options-menu sites (`triggers-2.md`
    §17, `sound-table-2.md` §15 r5). ~~The other 65 sites of §11~~
    (more than two reads): moved to the recording list
    (`docs/handoff/pc2-rec-pc2-render-audio.md` RA-T1); the result goes
    to `client/ui.md` §B8 and the panel specs.
13. COF/AnimData frame event 3 ("sound", `formats/cof.md`): none of
    the 222 request sites reads it; mode sounds use the fixed delays of
    §4. A request log of an attack whose animation has event 3 settles
    whether any other path plays it. Answered (`triggers-2.md` §15):
    event 3 is stored in U +0x4E by the frame advance; it runs the
    skill do (with `dosound` / `tgtsound`) like events 1–2, and
    `cltdofunc` 16 and 37 act on it alone; nothing else reads it.
