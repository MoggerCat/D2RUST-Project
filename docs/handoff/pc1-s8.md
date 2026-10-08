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
| msg-units OQ6, OQ11; msg-ui OQ1, OQ2 (bit 0x10000), OQ9; msg-skills OQ1 (REC-09 check only); msg-stats-items OQ3, OQ7; 0x7D/0x92 set-update labels | pet pass t rule (`0x00478E70`); dead flag 0x10000 writers/clearer; chat ignore list filter; 0x0D f=1 preload only; charges only via stat callback `0x004609F0`; 0xA6 never sent; 0x16 check, 0x17 never dispatched | `client/msg-stats-items.md` §5 r4–r5, r7.1–r7.2; `client/msg-units.md` §4 r6, §7 r12–r13, §8 r10.1; `client/msg-ui.md` §1, §4 r3.2.1–r3.2.4; `client/msg-skills.md` §1 r5–r6. No PROVISIONAL left in `client/msg-*.md` | 5 markers match (row `q-drop-stale-provisional`); pet palette, dead flag, charges do not | `q-fix-pet-palette`, `q-fix-dead-flag`, `q-fix-client-stat-callback`, `q-drop-stale-provisional` |
| intents-events OQ2, OQ10, OQ12 (0x23 part), OQ15; REC-95, REC-02 (binary; captures confirm only) | leave queues 0x05, 0x06, sends 0xB0, no 0x5C to the leaver; 0xAF only at server start; state-change bit writers/clear; legacy 0x23 only for saves < 0x5C; 0x3D = mode 2↔5 by footprint query; 0x4C/0x4D level byte = low byte of `skill_level(…,1)`; new character: no 0x7B. sim/stats, unit-order: no bin questions left | `sim/intents-events.md` §2.5 r2, §3.5 r6, §7.3 r2, §7.4 r3, §8.2 r3.6, §9 r4 + table; `combat/damage.md` §3.1 step 1 → bodies-2 §2.27 | leave sequence + hot keys match; level byte, 0x3D, monster update steps do not (dual-weapon switch already `q-fix-dual-weapon-damage`; also `wiring/action/combat.rs:358`) | `q-fix-skill-msg-level`, `q-fix-door-highlight`, `q-fix-monster-update-steps` |
| REC-240 (1)–(4), REC-238 | belt labels: short name of the slot-1 key of commands 23–26 (`0x00722404`, `0x0046A530`), "None" when only slot 0 bound, key-name table 3762–3914; belt tip `0x00497A40` (stat lines colour 3 + name colour 0 + trade price); rectangles `0x0046EFD0` 29×29 mode 0; gold prompts 4049/4050, 0x4F 0x14/0x13, sound 0xDD; pop-up text `0x00502280`/`0x00503000` one slot per frame, Font16; globe numbers DrawText, no box | `ui/control-panel.md` §3 r6, §5 r4, r5, r8, r13–r14 new, §6 r1, OQ7–8 answered, OQ9 new | tip font matches; labels, tip, rects, prompts, pop-up box, globes do not | `q-fix-belt-ui`, `q-fix-gold-prompts`, `q-fix-hud-popup` |
| REC-230 | settled from the existing DRLG rules + 1.14d data: walk links (warp −1, shared edge, flag 0x10<<i) vs tile links (lvlwarp Id + DS1 exit cell); full connection lists for Act I fields, Act III (jungle adjacency seed-dependent: 76–77 always, 78 to 76/77), Act V; levels.txt = leveldefs.bin (137 rows), LvlWarp.txt = lvlwarp.bin | `drlg/levels.md` §12 new, OQ11 answered; `drlg/outdoor-act3-act5.md` §4 r3, §5 r1 refs | `synthetic_chains.rs` chains wrong (Act I chain does not exist, Act V order wrong, walk links as tiles, slots/ids made up) | `q-fix-level-connections` |
| REC-95 (pathing part), REC-90, path-placement OQ8 | mode table `0x007319E8` (20 rows) fully specified: own client gets the skill message only with E-flag 0x4 (dodge/avoid); unit form needs receiver in the target's room; NU/TN, GH/BL/DD, DT, KB rows; `0x00554EA0` never sees a static path (20 callers); 1.14d save act byte at `0x0056A1F7`. tick OQ2, rng OQ1 need captures; rng OQ5 Phase 7 | `sim/pathing.md` §10 r2; `sim/path-placement.md` §10 r1, §11 → waypoints §11, §13 r2, OQ8 | §10 r1 and §13 r2 match (REC-90 can close); §10 r2 does not | `q-fix-player-mode-msgs` |
| REC-82 (binary; PC 2 can drop it); missiles OQ7, OQ9, OQ11 | Shadow Master scan: beyond d² 1024 skips n/near/best/notable; aitype 1 lacks aurastate → no draw; aitype 12 non-progressive = pick +8/+12; server reads no InitSteps/Qty/SpecialSetup/ExplosionMissile; monster mode missile `0x005A6D50` level MonsterSkillBonus+1 (hireling stat 12), quill volley `0x005A6B70` inert in 1.14d; no missile is created before the queue run | `monsters/ai-bodies-7.md` §27 steps 8, 12; `missiles/missiles.md` §R2.2, §R7 r1, §R10, §R11, OQ7/9/11 | 3 markers match (added to `q-drop-stale-provisional`); scan and mode missile do not | `q-fix-shadowmaster-scan`, `q-fix-monster-mode-missile` |
| REC-60, REC-91, REC-92 (withdrawn, binary); text OQ10; tbl OQ1, OQ3 (Patch_D2); mpq OQ2; animdata OQ1, OQ4 (u16); partly animdata OQ3, cof OQ1 | edit-box scroll refit `0x004FE7C0`; patchstring 103–105 correct; tbl hash signed chars, probe skips non-1 used bytes; Storm lookup only locale 0 / platform 0; animdata u16 unused; d2s token slot classes from `0x00744CA8` (old 57–124 reconstruction wrong for 75 of 659 items); empty alternategfx passed as is | `ui/text.md` §15 r6, OQ10; `formats/tbl.md` §Key lookup, OQ1, OQ3; `formats/mpq.md` §5 r5; `formats/animdata.md` OQ1/3/4; `formats/cof.md` OQ1; `formats/d2s-appearance.md` §Constants, OQ3/4 | appearance OQ4 matches; scroll, token table, tbl, mpq do not | `q-fix-edit-scroll`, `q-fix-d2s-token-ref`, `q-fix-format-lookups` |
| REC-62 (binary; capture optional); camera OQ8, capture OQ8, draw-order OQ14, composition OQ3 | every caller of shake `0x00476A80` with trigger and parameters (missile client fns 29/31/36/37/38/54/66 via table `0x0072A398`, skill 301 Stomp via `0x00727BA8`); draws with no tick: paused passes and the catch-up hold `[0x007A04BC]` (10 s act load, 2 s act set-up, 3 s 0x15 move); world draw reaches no UI/text code; StartDraw callers mapped. Gap table for render was stale (most OQs already answered) | `render/camera.md` §8, §9, OQ8; `render/capture.md` OQ8; `render/draw-order.md` OQ14; `render/composition.md` OQ3 | shake does not match; no-tick draws deliberately not reproduced (model §17 r4) | `q-fix-shake-starts` |
| original-hooks OQ4, OQ6, OQ7, OQ8, OQ12 (part); scenario OQ2, OQ4, OQ3 (part) | save-dir fallback; join codes table (`0x00569D80`: 0xD/0xE difficulty, 8/9 expansion, 0xA–0xC hardcore/dead); class from the save; `-act` has no effect; new hook points §7.1–§7.5 for the R-* list | `tools/original-hooks.md` §5.2–§5.4, §6.2 r3, §7 new, OQ13 new; `tools/scenario.md` OQ2–4 | recorder lacks the §7 hooks | `q-rec-hooks`, `q-rec-joinflags`, `q-rec-d2sitems` |
| REC-239 (1)–(3) | shrine states via `0x0056E970` curse path (stat 109, state 57, first flag-0x20 list replaced, same state = refresh), flags 0x22, timer 12; remove callbacks `0x00583BD0` / `0x00583A40`; hover label `0x00454F30` text rules + pop-up draw; overhead "3683+id" bubble 157 draws, 0x76 clears | `world/objects-2.md` §26 new; `world/objects-client.md` §29 new, OQ3–4; `world/objects.md` §9.1–9.2 pointers | `create_hover` matches; state helper, label, overhead draw do not | `q-fix-shrine-states`, `q-fix-object-label`, `q-fix-object-overhead` |
| REC-80, REC-81 (now confirmation checks only); population PROVISIONALs | boss restore `0x005A09E0`: mode 1, room from `0x0064E840`, search origin = §8 point when §8 found a spot but creation failed; tentacle heads always get owner data (no SetBoss test); Radament `roll(5)`+2 on the boss seed, spawns via `0x005B23C0` without owner; dispatcher `0x0054E600` null ids. ai.md / units.md "unspec" hits are false matches; their OQs are capture-only | `monsters/population.md` §6.3, §10.2–§10.3, §11.4 r10, §11.5 r6, OQ9; no PROVISIONAL left | 4 markers match; restore origin does not | `q-fix-boss-restore-origin` |
| calc-expressions OQ1, 2, 4, 6, 7, 9; loading OQ2, 3, 12 (part); field-types OQ6; txt-format OQ1 | missile `rand` seed = (formula offset, missile-id arg) local to one evaluation; bytes ≥ 0x80 stop the tokenizer; missing code file → family evaluates 0; items context = target unit + item; special-value low byte; stat mode whole-name compare; loose files only with `-direct`; `experience` needs MaxLvl + 2 rows; `atol` = strtol saturating. calc "unspec" hits were false ("pending") | `data/calc-expressions.md` §1.5, §3.5, §4.2, §4.4, policy 6, OQs; `data/loading.md` §2, §4.3, OQs; `data/field-types.md` §8.2; `data/txt-format.md` OQ1 | stat mode, low byte, atol, loading policy match; missile rand does not | `q-fix-calc-missile-rand` |
| REC-230 follow-up (warp tiles on deactivate/reactivate), REC-99 (where tiles spawn); rooms OQ12, OQ14; preset OQ2 (reachability), OQ3, OQ5, OQ7; outdoor OQ1 (field part) | preset units spawn once per DRLG room (flag 0x4000000); tiles stored/freed on deactivation and re-created with new GUIDs by the restore `0x00542B40`; `0x005559A0` spawns non-monster presets first; item store always frees the item (old §8 r5 wrong); no animated shadow tiles; hardcoded units before transfer; DS1 priority Patch_D2 > d2exp > d2data (10 Patch_D2 files); waypoints always in grid; preset direction u32 at `[level+0x14]+0x04` | `drlg/rooms.md` §1, §8 r5–r6, OQ12, OQ14; `drlg/preset.md` §6 s9, OQs; `drlg/outdoor.md` OQ1; `drlg/levels.md` §1 | tile set after reactivation matches; order and once-rule do not | `q-fix-warp-tile-restore` |
| assets OQ1 (budgets); dc6 flip values; render-pipeline page fit | measured 23,595 DCC/DC6/DT1 files: decoded ≈ 2.8 GiB (eviction required), scene sizes (town ≈ 22 MiB, busy field ≈ 105 MiB); largest frames DCC 345×324, DC6 319×256, DT1 160×864 (all fit a 2048² page); DC6 flip only 0/1; no zero-size frames | `client/assets.md` §A5, OQ1; `client/render-pipeline.md` §A2; `formats/dc6.md` pixel decoding | `assets/cache.rs:43-46` defaults match | — |
| REC-245 (1)–(3), REC-247 (1)–(4) | tint: `0x004D97F0` picks colorpri strictly greater (state 0 never wins), sets colorshift and the local player's light colour; bubbles step per drawn frame from the unit's drawn point; S→C 0x26 type 4 never sent (builder `0x0054A9B0` unreachable); roofs take the floor drawer, opaque; wall fade = record +0x24; blocks-light: cells outside rooms → flag 1, only DT1 bits 0x02/0x20 | `render/lighting.md` §4 r1–r3, §8, §11, §13; `render/shading.md` §4, §6 r1.1, edge 5; `render/blend-modes.md` §2, §6; `ui/messages.md` §3 r4, §5 r1, r3 | type 4 and roof path match (drop `preview_blocks.rs:10`); tint, bubbles, wall fade, light map do not | `q-fix-state-tint`, `q-fix-preview-light` |
| sprite-placement OQ5; model OQ7, OQ15; msg-units OQ4; msg-ui OQ3; dt1 OQ1 (part) | DCC decoder never clips; frames > 256 hit fatal 0x58C, gargoyletrap reachable (PROVISIONAL skip until a capture); DCC coded bytes = DC6 re-encoding length in every live frame; cel context table; shrine on-use functions 16/19/21/22; 0x5B / 0x75 words; quest-log latch 0/1/2; DT1 unknown fields zero, 0x58 overwritten at load | `render/sprite-placement.md` §3, OQ5; `formats/dcc.md` §Coded bytes, §Frame size limit; `client/model.md` §13 r7, §15; `client/msg-units.md` §8 r3, r10; `client/msg-ui.md` §1 r6.3; `formats/dt1.md` §Unknown fields | roster words, DT1 fields match | `q-fix-dcc-limits`, `q-fix-questlog-latch`, `q-fix-shrine-onuse` |
| REC-117 (1)–(3), (5); REC-243 (1)–(4). REC-117 (4) d2rs-own | item-use dispatcher `0x005BF240` + table `0x00741790` (new `items/use.md`); TP cast `0x005BE290`: refused in town / level 136 (sound 24, item kept), old pair closed first, sound 7 always, owners only on success; removal 0x0A to every client with the room adjacent; 0x82 fields; state 102 new list each time; no timer, five closers; d2rs "last field level" pair contradicts the original | `items/use.md` new; `world/objects-2.md` §27 new, edge 8–10; `world/objects.md` §12 r13; `world/cube.md` §1, OQ7 | client 0x20 and 0x82 fields match; cast, use, state 102, vendor copy do not | `q-fix-portal-pair` (extended), `q-fix-tp-use`, `q-fix-just-portaled`, `q-fix-vendor-buy-copy` |
| REC-244 (cube source, cube leaving while open, tip font), REC-241 (town byte, status word, mouse skills on load, hireling block, golem item, stale runeword). d2rs-own: start_extra cube gift, cube_opened latch, read_gaps plumbing | original never gives a cube at start; panel does not close when the cube leaves (no cube-gone close in 1.14d), transmute still works; tips font 1 pop-up; town byte = act of the player's level; status dead bit and progression bits writers; mouse skills by exact (skill, owner) lookup, no fallback; hireling block rebuilt from the live hireling; un-recast golem item dropped; stale equipped runeword detached, not saved | `world/cube.md` §11 new; `formats/d2s.md` §2.1, §2.3, §2.4 r6, §2.5 r3, §8.5 r6, OQ13; `formats/d2s-load.md` §6 r1.2, OQ2 | town byte matches; others do not | `q-fix-cube-panel`, `q-fix-save-gaps`, `q-fix-stale-runeword`; `q-fix-weapon-switch` extended (save +0x10/+0x80/+0x84) |
| REC-248 | skill LOS `0x00645950` walks from the target point to the caster in the caster's room (asymmetric §13.3 walk), masks 4/0x1C09/0x180/0x804/0x805, both end cells tested, outside the room's adjacency → blocked; client target point substitute by facing octant; client grid built from its own rooms' DT1 flags at room creation, never rebuilt | `sim/pathing.md` §13.3–§13.4; `drlg/rooms.md` §10.2; `skills/use.md` §5.3 s6.4 | `umod_line_clear` matches; skill LOS direction does not | `q-fix-los-direction` |
| REC-234, REC-254, REC-235 (1)–(3), (5); REC-235 (4) d2rs-own; pointer fixes (items/use.md links, 0x82 → §27.5, hirelings save) | Tal Rasha's Chamber = `tomb_talrasha` stamp in the staff tomb; §8.1 corrected (boss tomb gets `tomb_kaa`, entrance object 100 next to the orifice); Lair has no warps, entry by operate 43 at spawn record 0, presets only; Duriel/Tyrael AI = generic preset start, first think frame + 2; Tyrael's portal lands via record 0 in Lut Gholein, never partner-less; quest item delete by mode; inventory +0x1C writers; quest drop floor search; Hellforge row matches | `world/quests-act2.md` §8.1, §8.11, §8.12 new; `world/objects-2.md` §25, §27.2; `world/quests.md` §9.2; `world/quests-act3-2.md` §11.5; `world/quests-act4.md` §4.7; `world/hirelings.md` §10; pointers in items/inventory-moves, items/inventory, data/calc-expressions, sim/intents-events | Hellforge row matches; the rest do not | `q-fix-preset-monster-ai`, `q-fix-lair-entrance`, `q-fix-quest-items`; `q-fix-portal-pair` extended |
| REC-187 (1),(3),(5),(7); REC-189 (1),(2),(4); REC-231 (1),(3),(5); REC-251; REC-256; d2rs-own: REC-187 (2),(4),(8), REC-231 (2),(4),(5) | options art fully specified, no extra rows, Light/Shadows/Automap rows have real effects, slider math 53-bit (REC-213 verify only); pressed state, button input, label pen, logo/fire file names; text control layout (F1.1 r8), credits wrap in all columns (old "column C never wraps" wrong); create popups and fonts; every 0x03 rebuilds the act; Configure Controls in game §O9 r9; cube close and 0x20 cites in panels | `ui/frontend-menus.md` F1.1 r4/r4a/r5/r8, F1.5, F3.6, F3.8; `ui/frontend-credits.md` C3, C4; `ui/frontend-options.md` §O5 r7, §O8, §O9; `ui/frontend-loading.md` L9, L10; `ui/panels.md` §12; `ui/panels-2.md` §20 | slider math, repeated 0x03, glyph offsets match | `q-fix-options-effects`, `q-fix-fe-buttons`, `q-fix-create-popups`, `q-fix-controls-ingame`; `q-fix-esc-menu`, `q-fe-text-widths` extended; `q-options-art`, `q-controls-ingame` superseded |
| REC-242 (all but the unused elixir text path) | new `ui/item-tips.md`: 18 blocks bottom-up with colours, red requirement lines, name rules per quality, merged property list by descpriority, all 28 descfuncs, set tips, store price line, tome/gamble tips | `ui/item-tips.md` new; pointers in `ui/inventory.md` §5 r1, `data/runtime-maps.md` §3 | none match | `q-fix-item-tip-layout`, `q-fix-item-tip-props` |
| REC-252, REC-253 | globe numbers Font16 always (set at `0x00499053`), stamina tip pop-up `0x00502280(W/2-76, H-52)`, `stambarblue` flag; requirement check out flags, dex/str asymmetry, socket contribution `0x0062B450`, 0x4000 effects and refusing callers, client body-click pre-check events 19/20 | `items/inventory.md` §4.2, §5.6, §5.7 r4, Q1-Q6; `ui/control-panel.md` §3 r6, §4 r2 | globe font, sweep cap, item_active_on match | `q-fix-equip-reqs`; `q-fix-hud-popup` extended |
| REC-249, REC-246 (a)-(c) | Act I dungeon kinds (maze vs preset), connection slots and lvlwarp Ids, exit cells and tile places for every stamp; old maze "unconditional stamps" never apply (DrlgType 2); quest item use sends 5D chain 8/18/33; Docks waypoint place; Arreat Summit exits and units. Live data spawns no warp tiles (warp_unit not implemented) | `drlg/levels.md` §12 intro, §12.3, §12.4, §12.5 new, OQ12; `drlg/maze.md` §6 | waypoint indexes match; tree, Docks, summit, warp_unit, 5D, summit gate do not | `q-fix-warp-tile-unit`, `q-fix-quest-item-msg`, `q-fix-summit-gate`; `q-fix-level-connections` extended |
| REC-232 (re-filed): Leap Attack (Barbarian 143, not a monster skill), kicks, Dragon Flight, Whirlwind, sequences | Leap Attack: Land / Launch / Strike order, no landing hit, restart at frame 16 on a target, strike index 16/17/18 by weapon class, re-pick, knockback + ED; Dragon Talon plays KK, refused in town; Dragon Flight seq 21 teleport at 9, kick at 14, no range test, failed teleport still kicks; player sequence table `0x007483B8` (23 × 14) dumped with lookup / loader | `skills/sequences.md` + `sequences.tsv` new; `skills/use.md` §5.2; `skills/bodies-2b.md` §6.12, §7.20; `skills/bodies-2.md` §3.11 | Leap Attack, kicks, Dragon Flight sim match; sequences missing in play | `q-skill-sequences`; `q-fix-moving-skill-do` extended |
| REC-250 (1)-(4), REC-255, REC-258 | lighting: quest byte = A[1] of the last 0x5E (fatal 0x60 if none), override counters once per client update `0x0046BEB0` (order darkness, Den, 107/108 gated on its flag), near list = room adjacency array; Cain caption from the client quest record `[0x007C0D43]` (the REC premise was wrong); front-end controls measure width A, clip/wrap width B | `render/lighting.md` §3 r3, §10 r1, r2, r5, §13; `world/npc.md` §6; `ui/text.md` §13 | none match | `q-fix-cain-caption`; `q-fix-preview-light`, `q-fe-text-widths` extended |
| unowned: client body-location clicks, hireling item checks | box dispatch `0x004912A0` / table `0x00721E58`, socket pre-dispatch, use cursors, equip-check table e 0-7 per location with messages, store click; hireling wearable test `0x0048B290` per class, panel on mouse up `0x0048B7C0` (event 86 branch dead), portrait drop `0x004934D0` | `ui/panels-3.md` §29, §30 new; `world/hirelings.md` §11 r9 | `inv_items.rs` equip_press d2rs-own, no hireling panel | `q-fix-body-clicks`, `q-fix-merc-panel-items` |

Follow-ups for a later worker (other owners: `ui/panels.md` §15 rows body location click / socket fill → `ui/panels-3.md` §29, mercenary → §30 (`0x0048B7C0` is mouse up); `items/inventory.md` §5.6 the rest of the handlers → panels-3 §29, hireling slots → §30; `ui/inventory.md` §10 r5 → panels-3 §29; `audio/triggers.md` §2 r2 events 84–86 also from `0x0048B7C0`/`0x004934D0`; `sim/units.md` §4.2 "sequence spec" → `skills/sequences.md`, §4.1 prepare → `sequences.md` §2; `items/inventory-moves.md` §7.11 step 4 add the 5D chains 8/18/33 (`0x0055E40C`/`0x0055E48D`/`0x0055E506`); `world/quests-act5-2.md` §7.6, §7.8 link `drlg/levels.md` §12.4; `ui/control-panel.md` §5 r8 can link `ui/item-tips.md` §6 for `0x004E6410` line breaks; unowned specs needed: hireling slot checks `0x0048B290`/`0x0048B3F0`/`0x004934D0`, the rest of the body-click handlers (`ui/panels.md` §15); the items/use.md links, 0x82 pointer and hirelings save pointer are done; `ui/panels.md` cite `0x00487740`/`0x004786D0` for 0x20; msg-stats-items labels, properties §13 callers and the OQ pointers are done): `client/stat-lists.md` §2 r8 should link the `skills/use.md` §2 item type test.

HANDOFF REC lines to update when HANDOFF is free (Lane B is editing it): REC-237, REC-204, REC-211, REC-212, REC-229 → "settled from the binary 2026-10-08 (pc1-s8), capture verifies only", pointing at the sections above; REC-177 (3), REC-188, REC-232, REC-233 likewise; REC-117 and REC-243 settled (objects-2 §27, items/use.md); REC-90 closed (path-placement §10 r1); REC-95 reduced to confirmation captures (intents-events §7.4 r3, pathing §10 r2); `docs/handoff/stitch-server-core.md:23` → pathing §10 r2; REC-82, REC-60, REC-91, REC-92 settled from the binary (drop from the PC 2 list; REC-91 also clears local-run item 99). `formats/dt1.md` OQ3 count (251 / 15,928) vs measured 245 v7 files / 15,637 tiles (+6 v4 leftovers): recount wording.

Overlaps with staging rows added the same day (coordinator: merge them when launching): `q-hud-globes` ⊂ `q-fix-hud-popup` (2) (use control-panel §3 r5–r6); `q-options-art` overlaps `q-fix-esc-menu` (frontend-options §O2 r4 art table); `q-a2-duriel-ai` portal spot overlaps `q-fix-portal-pair` (objects-2 §25); `q-identify-cain` belt tips overlap `q-fix-belt-ui` (2).

## Lane B — local run (Batches 1, 2, 3, 5)

Recorded in `docs/HANDOFF.md` §5 Done block (2026-10-08, PC 1). Install
hash identical (19 entries). GPU Intel HD Graphics 630 / Vulkan.
Passing: Batch 1 all (1.4 with re-measured counts), 2.1–2.3, 2.6, 2.7,
2.9, 2.10, 2.12, 2.14–2.16, 2.18, Batch 3 all (21 GPU cases, map 0 of
32,238,080 differ, verify 11/11), Batch 5 baselines as expected. 31 claims
unlocked; coverage game-file 416 (was 384), verified 496 of 10,583.
Failing (findings, no expectation changed): 2.4 one test, 2.5 one test,
2.8 one test, 2.11 two tests, 2.13 seven tests → rows `q-fix-sweep-counts`,
`q-fix-lvlprest-beyond-files`, `q-fix-item-create-sweep`,
`q-fix-cold-plains-rooms`, `q-fix-wired-host-walk`,
`q-fix-placement-reader`. Spec note: evilhut is monstats row 529 (0-based),
`population.md` says 528. Not run: Batch 4, 2.17, 3.15. Stale LOCAL-RUN
expectations updated (1.4, 2.4, 3.1, 5.2).

## Lane C — recordings PC 2 needs (moved into HANDOFF §7 "PC 1 s8 additions": R-SWAP=REC-300, R-SET=REC-301, R-WW=REC-302, R-CLAW=REC-303, R-SENTRY=REC-304, R-PAUSE=REC-305, R-ACT=REC-306, R-TYRPORT=REC-307, R-HCFLAG=REC-308, R-MSG=REC-309, R-EXIT=REC-310, R-MUPD=REC-311, R-DOOR=REC-312, R-BELT=REC-313, R-HUD=REC-314, R-GOLD=REC-315, R-LOAD=REC-316, R-LVL=REC-317, R-MODE=REC-318, R-ACTBYTE=REC-319, R-MIS=REC-320, R-SHAKE=REC-321, R-NOTICK=REC-322, R-SHRINE=REC-323, R-DRLG-DIR=REC-324, R-WARP-RESTORE=REC-325, R-TINT=REC-326, R-WALL=REC-327, R-LIGHTEDGE=REC-328, R-BUBBLE=REC-329, R-SYS4=REC-330, R-GARG=REC-331, R-TP=REC-332, R-CUBE=REC-333, R-SAVE=REC-334, R-LOS=REC-335)

Recordability (tools/original-hooks.md §7): R-MIS-1/2, R-PAUSE-1, R-NOTICK-1, R-LVL-1/2, R-EXIT-1, R-MSG-1 need `q-rec-hooks` first. R-HCFLAG-1 from the menu by hand (no `--auto`) until `q-rec-joinflags`. R-MODE-1 needs a second client. R-LVL-2, R-ACT-1, R-TYRPORT-1 need saves at those quest stages. The rest is covered by the existing recorders.

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
- **R-MSG-1** [AUTO] Settles: msg-stats-items OQ1. Recorder dumps the
  local player's stat list after frame 2 of a join.
- **R-MSG-2** [MANUAL] Settles: msg-units OQ7. Hire a hireling: 0x7A, 0x81, 0xAC.
- **R-MSG-3** [MANUAL] Checks msg-skills §1 r6 (REC-09). Equip / unequip a
  charged item, use the charges.
- **R-MSG-4** [AUTO] Settles: msg-skills OQ4. Scan existing recordings for a
  frame where a later message changes the unit, target or state 118 after
  a 0x99 / 0x9A event.
- **R-MSG-5** [MANUAL] Checks msg-units §8 r10.1. Necromancer skeleton
  render capture (expect palette map 1).
- **R-EXIT-1** [AUTO] Settles: intents-events OQ2 remainder. Single-player
  exit: S→C receive order on the client (system 0xB0 vs game 0x05 / 0x06 in
  the same drain).
- **R-MUPD-1** [AUTO] Settles: intents-events OQ10 remainder. One plain Act I
  fight: which monster-update senders fire (0x11, 0x57, 0xA7–0xA9).
- **R-DOOR-1** [MANUAL] Confirms intents-events §9 r4. Hover an open door
  with a monster in the doorway: C→S 0x3D and the door's next mode (2 → 5).
- Confirmation only: 0x4C level byte with a +skills item (REC-95); 0x7B
  count at a new-character join (REC-02).
- **R-BELT-1** [MANUAL] Settles: control-panel §5 r4, r13. Set `CfgBelt1`
  to F1, bind `CfgBelt3` in its secondary slot only; screenshot the belt
  (expect "F1" and "None").
- **R-HUD-1** [MANUAL] Settles: control-panel §5 r14, REC-238. Hovered run
  button and experience bar at 800×600, screenshots.
- **R-BELT-2** [MANUAL] Settles: control-panel §5 r8. Hovered belt potion
  with and without an NPC shop open, screenshot.
- **R-BELT-3** [MANUAL] Settles: control-panel §5 r4–r5. Highlight pixels:
  hovered usable potion; cursor item over empty, swappable, invalid box.
- **R-GOLD-1** [AUTO trace + MANUAL screenshot] Settles: panels-2 §21 r6,
  r8. Stash gold withdraw and deposit dialogs, C→S 0x4F trace.
- **R-LOAD-1** [MANUAL] Settles: REC-236 (and `frontend-loading.md` L5–L7
  checks). Draw calls of the loading screen across a waypoint change and a
  game start at 800×600 (repeated 0x03 for the same act, 0x61 videos).
- **R-LVL-1** [AUTO] Confirms `drlg/levels.md` §12.3 (REC-230). Enter Act III
  with `-seed 644409375` (TestSor), dump drlg +0x90 vis/warp of levels 75–83.
- **R-LVL-2** [MANUAL] Confirms §12.4 (REC-230). Act V save: dump vis/warp of
  109–112, walk into 112 and 117, list warp tile units (class 71/72).
- **R-MODE-1** [ASSISTED] Confirms `sim/pathing.md` §10 r2 (REC-95). A
  dodge/avoid trigger and a normal A1 attack with a second client: S→C
  0x4C/0x4D received by the own and the other client.
- **R-ACTBYTE-1** [AUTO] Confirms path-placement §13 r2. Load a character
  saved in Act III, log the S→C 0x03 act byte (expect 2, town 75).
- **R-MIS-1** [AUTO] Confirms missiles OQ9. Quill rat A2 on Normal and Hell,
  missile positions per tick (expect v = 2112 / 3456).
- **R-MIS-2** [AUTO] Confirms missiles OQ11. Hook `0x0059FA30` in a combat
  recording, log the tick step in progress (expect step 4 or between ticks).
- **R-SHAKE-1** [MANUAL] Optional confirmation of camera §8 rows 36, 38
  (REC-62). Next to the Hellforge or at Duriel's death: shake start time,
  `seed_start` / `seed_end`, (dx, dy) per frame.
- **R-NOTICK-1** [AUTO] Settles camera OQ8 / capture OQ8 against recordings.
  For runs 1–2 list frames after 0 ticks; each must be paused or inside the
  `[0x007A04BC]` hold (record it and GetTickCount per frame).
- **R-SHRINE-1** [AUTO] Settles objects-2 §26.1–§26.3 (REC-239). Stat-list
  trace (state, flags, expiry, source, stats) after each state shrine (codes
  6–15); same shrine twice, a second shrine, a shrine while cursed (Amplify
  Damage), expiry of the stamina and skill shrines.
- **R-SHRINE-2** [MANUAL] Settles objects-client §29 r2–r5, OQ4. 800×600
  hover screenshots: chest, locked chest, shrine before/after, a door per
  mode, a waypoint, a town portal.
- **R-SHRINE-3** [MANUAL] Settles objects-2 §26.4. Frame-stepped capture from
  shrine operation until the bubble disappears.
- **R-DRLG-DIR** [AUTO] Settles outdoor OQ1. After Act I creation read the
  u32 at `[level+0x14]+0x04` for levels 1 and 27, and 40 after Act II
  (expect 3 and 1 for the recorded seed).
- **R-WARP-RESTORE** [AUTO] Confirms rooms §8 r6. Blood Moor: log the room's
  type-5 units (GUID, class, x, y) before deactivation and after
  reactivation, plus the tick step they appear in (expect new GUIDs).
- **R-TINT-1** [MANUAL] Confirms shading §6 r1.1, lighting §8. Monster under
  Poison then Cold, local player under Poison, GDI `-w`: cel draws' palette
  argument and the player's light colour (record +0x25..+0x27).
- **R-WALL-1** [MANUAL] Confirms lighting §11 r2, r4 / blend-modes §6. A wall
  in a preset with Logicals (Jail or Cathedral) next to a light, plus a Lut
  Gholein roof: per-pixel tile output and the light-map words.
- **R-LIGHTEDGE-1** [AUTO] Confirms lighting §4 r2. Light-map flag words at
  `0x007B0E68` at the edge of the loaded rooms.
- **R-BUBBLE-1** [MANUAL] Confirms messages §5 r1. An NPC overhead bubble at
  two draw rates; when it disappears, in drawn frames.
- **R-SYS4-1** [AUTO, optional] Scan packet recordings for S→C 0x26 with
  u8@1 = 4 (none expected).
- **R-GARG-1** [MANUAL] Settles sprite-placement §3 PROVISIONAL, dcc §Frame
  size limit. Walk a room with a `gargoyletrap` preset
  (`ACT1\CATACOMB\catNEtheme1.ds1` has 3): record unit and cel draws, or
  note an abort with 0x58C.
- **R-SHRINE-4** [MANUAL] Confirms client/model §15 r6. Storm, Exploding and
  Poison shrines: client missile creates (class, position, target, level, order).
- **R-TP-1** [MANUAL] Confirms objects-2 §27.1, §27.4, §27.5. TP scroll in
  Blood Moor with an older pair elsewhere; enter O1, return through O2.
  Expect old pair 0x0A; O1's 0x51/0x60/0x82 + sound 7; 0x7C, 0x22, 0x3F,
  scroll removal; 0x0D on arrival; 0x0A for both after the return.
- **R-TP-2** [MANUAL] Confirms §27.1 r4, use §1 r5. Scroll in the Rogue
  Encampment: sound 24, one 0x3F, one 0x7C, scroll kept, no 0x22.
- **R-TP-3** [MANUAL] Confirms use edge 1. Tome in the field: 0x7C, 0x3E
  (stat 70), 0x7C, 0x22; tome keeps flag 0x4.
- **R-TP-4** [MANUAL, second client] Confirms §27.5 r2. Second player enters
  the room with the owner's portal: 0x82 fields.
- **R-CUBE-1** [MANUAL] Confirms cube §11 r2–r3 (REC-244). Items in the
  cube, panel open, lift the cube, place it elsewhere, Transmute a valid
  recipe; then drop the cube on the ground. Expect the panel stays open, the
  transmute works, page-3 items go to the inventory, no 0x4F 0x17 / 0x77.
- **R-CUBE-2** [MANUAL] Confirms panels-2 §20 r3. Hover close and Transmute
  at 800×600, screenshots.
- **R-SAVE-1** [AUTO] Confirms d2s §2.1. Waypoint to Act II, save: header
  +0xA8..+0xAA = `81 00 00`.
- **R-SAVE-2** [MANUAL] Confirms d2s §2.3. Andariel killed in Normal, saved:
  status bits 8–12 = 1 (5 on Nightmare after Andariel, expansion).
- **R-SAVE-3** [MANUAL] Confirms d2s §2.4 r6. Oskill / charged item skill as
  right skill, save and reload; variant with the item stashed first.
- **R-SAVE-4** [MANUAL] Confirms d2s §2.5 r3, §8.4, OQ4. Hire after the last
  load, give items, save; gain experience, save again.
- **R-SAVE-5** [MANUAL] Confirms d2s-load §3, d2s OQ15. Necromancer with an
  item Iron Golem, saved, rejoin with a packet log: recast at the join.
- IT-8 (existing entry) now also confirms d2s-load §6 r1.2.
- **R-LOS-1** [AUTO hook, MANUAL placement] Confirms pathing §13.4. Hook
  `0x00645950` entry/exit (x, y, GUID, mask, result, unit position) casting a
  lineofsight-4 skill across a dungeon wall and in the open; also client
  `0x004C6140`.

### Lane C round 2 (not yet in HANDOFF §7; Lane B round 2 is editing HANDOFF)

- **R-LAIR-1** [MANUAL] Settles quests-act2 §8.12 r3–4. Operate object 100:
  the player's 0x15 position, the room 0x07 messages, every unit of the
  Lair's first population (expect near sub-tile (138,193); presets only).
- **R-LAIR-2** [AUTO, same run] Settles §8.12 r5, ai.md §1.5. Duriel's AI
  think schedules from creation and from the first client entering his room.
- **R-TYRPORT-2** [MANUAL] Settles objects-2 §25 r11–13, §8.12 r6. Tyrael's
  portal pair 0x51/0x60 (no 0x82), arrival after walking in (record 0,
  tile (33,9)).
- **R-TP-5** [AUTO] Settles objects-2 §27.2. Town portal cast from the field
  with Lut Gholein = LutN: second portal position and the level-seed
  `roll(2)` between (35,10) and (32,13).
- **R-FE-1** [MANUAL] (extends REC-201) Main menu with a button held, dragged
  off, then Enter on Single Player: mode-4 glyphs, x-2/y+2 shift (§F1.1 r4-r5).
- **R-FE-2** [MANUAL] Credits at draws 0, 300, 12,130 (classic); create
  screen hardcore and name-taken pop-ups (§F1.1 r8, C4 r5, §F3.6).
- **R-FE-3** [AUTO] (REC-213 verify) `[0x0047CD00]` for Contrast p 99 under DirectDraw.
- **R-TIP-1** [MANUAL] text-0002 hover boxes: rare weapon reqs met / str failed;
  set item with 0/1/2/all pieces worn then equipped; store item unusable in
  buy/sell/repair/identify + ethereal in repair; a rune and a gem; items with
  descfunc 11, 13-18, 22-24, 27, 28; a suffix-only magic item (item-tips §2-§11, OQ2).
- **R-HUD-2** [AUTO] Hovered stamina bar and life globe (Show HP Text on) at
  800x600 and 640x480, normal and with state 136 (control-panel §3 r6, §4 r2).
- **R-EQUIP-1** [MANUAL] Body click with an item failing strength, then a
  class item of another class: no C->S, speech cantuseyet / impossible
  (inventory §5.6); a worn socketed item with a +dex jewel, unlinked, re-check (Q1).
- **R-A1DUNGEON-1** [MANUAL] Confirms REC-249 (levels §12.5, OQ12). Blood
  Moor->Den, Cold Plains->Cave 1->Cave 2, Black Marsh->Tower->Cellar 5,
  Monastery Gate->Catacombs 4: each type-5 unit (level, room origin, class,
  position) and the drlg +0x90 vis/warp records.
- **R-SUMMIT-1** [MANUAL] Confirms REC-246 (c) (levels §12.4). Save at the
  Arreat Summit: units created in level 120 and the two tile units.
- **R-LEAPATK-1** [MANUAL] Settles sequences OQ1, bodies-2b §6.12 s4-5. One
  Leap Attack with hth and one with a 2ht weapon, monster at range: per tick
  position, type-0 event ticks and args, the monster's life.
- R-WW-1 and bodies-2 OQ2 (Dragon Talon kicks at L 6 and 12) stay as listed.
- **R-LIGHT-2** [MANUAL] Confirms lighting §3 r3, §4, §7.3. Light map
  `0x007B0E68` (48x48x8) and q `[0x007B567C]` beside a dungeon wall, and at a
  border between rooms with different level ambients (Act IV town / outdoor).
- **R-LIGHT-3** [MANUAL] (extends RA-L1) Confirms lighting §10 r1, r5. After
  clearing the Den: `[0x007129CC]`, `[0x007A745C]`, `[0x007C0EA5]` per client
  update over 31 updates.
- **R-CAIN-1** [MANUAL] (extends R-NV-2) Confirms npc §6 caption. Cain's menu
  with Search for Cain done (4020, no cost) and not done (4021 + 100 x n).
- **R-FE-4** [AUTO] Confirms frontend-credits C3 r2, text §13. Credits pixel
  capture at a fixed draw count.
- **R-BODY-1** [MANUAL] Confirms panels-3 §29 r1-r5. C->S bytes and sounds:
  sword into an empty right hand; 2H axe on the right with a shield left;
  empty left hand opposite a bow (0x1C [4]); swap rings; stack arrows on worn
  arrows; remove a helm with inventory mode 0, then with a shop in a selling
  mode; item with too-high strength (no message, event 20); empty box with an
  identify scroll as use cursor.
- **R-MERC-1** [MANUAL] Confirms panels-3 §30 r2-r4 (Act 2 hireling): give a
  polearm (61 00 00); a sword (event 85); armor needing too much strength (85,
  not 86); take the helm (61 01 00); healing potion on the portrait (84 + 61
  00 00); unidentified item on the portrait (86, no message).
