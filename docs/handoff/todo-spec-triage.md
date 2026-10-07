# TODO(spec) triage (base claude/specs-staging-6 6a41f91f)

**Totals (markers): ANSWERED 237 / NOT SPECIFIED 59 / PENDING-BY-DESIGN 5** (d2-client 118/17(12 groups)/3; d2-sim, d2-server, d2-data, d2-proto 119/42/2). Excluded: 11 template placeholders and 14 convention-doc mentions (not markers).

Re-check: NOT SPECIFIED addresses were grepped against origin/claude/pc2-{quests,render-audio,objects-hirelings,items,ui,recordings,npc-vendors,wave-b,xpc}; none gained coverage. (Vendors price/gamble/store/trade rows rest on pc2-s4-world-econ.md and were not re-read in the specs.) Lines marked "(code differs)" are ANSWERED but the code contradicts the spec.

# 1. ANSWERED — cloud coding list

## d2-client

### client: UI widgets and panels (pc2-s4-ui.md)
- src/ui/widget.rs:33 → specs/ui/panels-2.md §22 r1 (button image changes with pressed flag only)
- src/ui/widget.rs:85 → specs/ui/panels-2.md §22 r2 (label pen from the text rule)
- src/ui/widget.rs:219 → specs/ui/panels-2.md §22 r3 (no wheel scroll; step 0)
- src/ui/widget.rs:120, 211 → specs/ui/inventory.md §B5, §1, §3, §8 (cell sizes, item graphic, hover)
- src/ui/widget.rs:300 → specs/ui/text.md §15 (edit box caret and selection)
- src/ui/panel.rs:27, src/world_view/present.rs:342, src/world_view/ui_bind.rs:322 → specs/ui/controls.md §6, §7 r5 (pointer button meanings, unhandled events to world intents)
- src/world_view/ui_bind.rs:45 → specs/ui/panels-2.md §22 r4 (UI image: cel draw point, shading, blend)
- src/world_view/ui_bind.rs:55 → specs/render/draw-order.md §10 (UI pass = 11)
- src/world_view/feed.rs:64 → specs/ui/panels-2.md §22 r5 (open mode is UI state only)

### client: Single-player app / join (pc2-s4-audio.md d2s rows; pc2-s4-world-econ.md)
- src/app/single_player.rs:211 → specs/formats/d2s-load.md §1, §8 r3; specs/items/generation.md §10.3 (new-character record 0x00532590 / stub 0x00569F80, join 0x23s)
- src/app/single_player.rs:1049 → specs/world/vendors.md Edge case 10 (store time = GetTickCount, host input in ms)
- src/app/single_player.rs:1147 → specs/formats/d2s-load.md §2 (quests row, 0x0056A370 → 0x0065C4D0), specs/world/quests.md §1.6 (load from save)
- src/app/play.rs:60 → specs/render/composition.md §4 (act pl2 palette), specs/client/model.md §11 (player's level)

### client: Audio: sound table / wav (pc2-s4-audio.md)
- src/app/sound.rs:13 → specs/audio/triggers.md, triggers-2.md §21 (driver input contract)
- src/app/sound.rs:14, 41, src/audio/mod.rs:217, 240 → specs/audio/sound-table.md §1–§7, §13; sound-table-2.md §16–§17 (sounds.txt mapping, variants, cache)
- src/app/sound.rs:15, 63, src/audio/mod.rs:218 → specs/formats/wav.md §5 (WavDecoder hook contract)
- src/audio/mixer.rs:108 → specs/audio/sound-table.md §8.3 r1–r4 (volume/pan curves)
- src/audio/mod.rs:411 → specs/audio/sound-table.md §6.6 r1, §5 r5/r7; triggers.md §1 r3–r4
- src/audio/mod.rs:207 → specs/audio/triggers.md, triggers-2.md §21; environment.md (which events make requests)
- src/audio/sound_table/volume.rs:54 → specs/audio/sound-table.md §6.4 r1 (game-loaded flag on in game)
- src/audio/sound_table/system.rs:52 → specs/audio/sound-table.md §8.1 r1 (pixel points)
- src/audio/sound_table/system.rs:146 → specs/audio/sound-table.md §7 r7
- src/audio/sound_table/system.rs:298 → specs/audio/sound-table.md §7 r4, r8 (loop start = Block 1 x 2 bytes; offset x 4)
- src/audio/sound_table/system.rs:331 → specs/audio/sound-table.md §8.1 r1 (River)
- src/audio/sound_table/system.rs:387 → specs/audio/sound-table.md §5 r2 (merged call attaches its unit)
- src/audio/sound_table/system.rs:585 → specs/audio/sound-table.md §4 r6 (no local player)
- src/audio/sound_table/system.rs:631, 634 → specs/audio/sound-table-2.md §16 r1–r3; sound-table.md §10 r5 (eviction, T = 0 / 25 phase)
- src/audio/sound_table/system.rs:953 → specs/audio/sound-table.md §7 r6
- src/audio/sound_table/system.rs:995 → specs/audio/sound-table-2.md §17 r2 (stream start failure)
- src/audio/sound_table/system.rs:1314 → specs/audio/sound-table.md §7 r8; environment.md §1 r6 (current id)
- src/audio/sound_table/system.rs:1326 → specs/audio/sound-table.md §5 r8 (distance squared recomputed, z + 640)
- src/audio/sound_table/system.rs:1337 → specs/audio/triggers-2.md §19 r1–r2, r6 (unit request list, ended requests)

### client: Audio: environment (pc2-s4-audio.md)
- src/audio/environment/mod.rs:17, 148 → specs/audio/environment.md §1 r3, r5; OQ 3; render/lighting.md §9 (day phase = period index)
- src/audio/environment/mod.rs:18, 150, 154 → specs/audio/environment.md §1 r5, OQ 4; render/draw-order-2.md §11.1–§11.3 (weather flag, intensity)
- src/audio/environment/mod.rs:56 → specs/world/quests-status.md §12 (client quest check 0x004A4180)
- src/audio/environment/mod.rs:395 → specs/audio/environment.md §4 r4
- src/audio/environment/mod.rs:486 → specs/audio/environment.md §2 r9 (-ns switch)
- src/audio/environment/mod.rs:716 → specs/audio/environment.md §5 r2 (exception 0)
- src/audio/environment/mod.rs:754 → specs/audio/environment.md §6 r4

### client: Audio: triggers (pc2-s4-audio.md)
- src/audio/triggers/events.rs:33 → specs/audio/triggers.md OQ 4 (answered); triggers-2.md §18 r3, §19
- src/audio/triggers/events.rs:151 → specs/audio/triggers.md §3 r2
- src/audio/triggers/mod.rs:333 → specs/audio/triggers-2.md §19 r4 (identity type 1 with record; raw Skill cells)
- src/audio/triggers/movement.rs:87 → specs/audio/triggers.md §5 r9
- src/audio/triggers/movement.rs:201 → specs/audio/triggers.md OQ 11 (client creation of Init voice)
- src/audio/triggers/npc.rs:14 → specs/audio/npc-greetings.tsv; triggers.md §10 r5
- src/audio/triggers/npc.rs:46 → specs/audio/triggers.md §10 r7 (mode 2 writes nothing)
- src/audio/triggers/objects.rs:23 → specs/audio/triggers.md §7 r7 (all-zero record still updates fields)
- src/audio/triggers/objects.rs:26 → specs/audio/triggers.md §7 r8 (cairn ids 413–417)
- src/audio/triggers/skills.rs:78 → specs/audio/triggers.md OQ 5 (dosound a / b, answered)
- src/audio/triggers/skills.rs:120 → specs/audio/triggers-2.md §16 (missile ProgSound)
- src/audio/triggers/ui.rs:39 → specs/audio/triggers.md §11 (bit 0 set: bit-1 rows not reached)

### client: Render: hook declarations and rules (pc1-specs-s5.md; render specs)
- src/composite/mod.rs:210, src/world_view/mod.rs:218, 237 → specs/render/unit-composite.md §3, §5–§7, §10 (component file/frame/direction)
- src/composite/mod.rs:229, src/world_view/mod.rs:260 → specs/render/sprite-placement.md §2, §8 (place hook); camera.md §4, §10
- src/composite/mod.rs:237, src/world_view/mod.rs:271 → specs/render/shading.md §10, §6; unit-composite.md §7; lighting.md §11, §13 (shade chain)
- src/composite/mod.rs:241, src/world_view/mod.rs:279 → specs/render/blend-modes.md §3, §7 (blend op, layer override)
- src/composite/mod.rs:282, 286, src/world_view/mod.rs:228 → specs/render/draw-order.md §10; camera.md §7, §10 (pass/major/minor, clip)
- src/world_view/mod.rs:212 → specs/render/draw-order.md §9, §10; camera.md §6–§7; drlg/rooms.md §9 (tiles to draw)
- src/rules/view.rs:76 → specs/render/shading.md §4, lighting.md §11 (tile shade)
- src/rules/view.rs:78 → specs/render/blend-modes.md §6 (roof fade, translucent walls)
- src/rules/view.rs:113 → specs/render/lighting.md §11 r2–r4; shading.md §4; blend-modes.md §6 (per-block shade)
- src/rules/view.rs:88, src/rules/lighting/view.rs:45, src/world_view/feed.rs:59, 184 → specs/render/camera.md §2–§4; client/model.md §2, §3, §5 (unit and local-player positions)
- src/rules/view.rs:92, src/world_view/feed.rs:188 → specs/render/unit-composite.md §8 (extra offsets; camera OQ 3 answered)
- src/rules/lighting/view.rs:50 → specs/render/unit-composite.md §7; shading.md §6 r4; blend-modes.md §3 (component look inputs)
- src/rules/lighting/draws.rs:47 → specs/render/lighting.md §11 r2 (wall block column c, points c and c + 1)
- src/rules/shading.rs:30 → specs/render/shading.md §6 r1 (unit palette index p, map p − 1)
- src/scene/tests.rs:143 → specs/render/shading.md §7 (mapped index 0 is written as opaque index 0)
- src/scene/item.rs:404 → specs/render/sprite-placement.md §8 (flip_x stays false)
- src/verify/map.rs:27 → specs/render/composition.md OQ 1, §4 r (index 0 black in all acts); shading.md §7
- src/world_view/feed.rs:77 → specs/render/camera.md OQ 6; sim/rng.md §5.3; client/model.md Randomness r2 (client player seed)
- src/world_view/feed.rs:82 → specs/render/draw-order.md §9; drlg/rooms.md §9.3, §9.6; client/model.md §12 (near-room array)
- src/world_view/feed.rs:127 → specs/render/draw-order.md §8 (fade clock: one GetTickCount per frame, host input)
- src/world_view/feed.rs:136 → specs/render/draw-order.md OQ 12 (answered: drlg/rooms.md §9.3 Entry identity); shading.md; lighting.md; blend-modes.md §6
- src/world_view/feed.rs:146 → specs/render/composition.md §3 step 2; client/model.md §11 (BlankScreen of player's level)
- src/world_view/feed.rs:152 → specs/render/lighting.md §1 r3, §6, §8; client/model.md (light records)

### client: Bridge: messages and model (pc1-specs-s5.md, impl-pc1-s5.md)
- src/bridge/output.rs:253 → specs/ui/panels-2.md §14 r8 (0x004B1A10 = class list 146, 251, 266, 331, 377, 378, 406, 408, 521, 527, 537–539)
- src/bridge/skills.rs:59, src/bridge/msg/tests_outputs.rs:153, src/bridge/world.rs:676 → specs/client/stat-lists.md §1 r1–r2, r4, §3 r6; msg-skills.md §2 r4 (client lists, state on/off)
- src/bridge/world.rs:57 → specs/client/msg-stats-items.md §2 r5, r5.3 (which actions set cursor_item)
- src/bridge/world.rs:139 → specs/client/msg-units.md §1.3 r3 (0x004BD6B0 shrine record), OQ 5 answered; model.md §15 r1
- src/bridge/world.rs:847 → specs/client/msg-units.md §1.2 r1, r7 (monstats2 choice counts at row +0x15 + i)
- src/bridge/msg/skills.rs:185 → specs/skills/levels.md §1 r3 (cap = experience MaxLvl)
- src/bridge/msg/lighting.rs:149 → specs/render/lighting.md §9.2 r4.4, OQ 11 (0x004BC5E0: EnvEffect objects only)
- src/bridge/msg/unit_misc.rs:151 → specs/client/msg-units.md §7 r7.2 (corpse direction := P's, 0x006487F0)
- src/bridge/msg/units.rs:135 → specs/client/msg-units.md §1.2 r2.1, r3 (hireling re-init 0x0046EC10; classes 271, 338, 359, 560, 561)
- src/bridge/msg/units.rs:191 → specs/client/msg-units.md §1.2 r6 (0x004AE8D0 monster set-up, in order)
- src/bridge/msg/units.rs:198 → specs/client/msg-units.md §1.2 r6.8 (monstats Skill columns)
- src/bridge/msg/units.rs:204 → specs/client/msg-units.md §1.2 r4 (0x00621CC0 source-unit link)
- src/bridge/msg/units.rs:267 → specs/client/msg-units.md §1.3 r5 (types 0, 3, 4, 5 never sent; handler error)
- src/bridge/msg/units.rs:282 → specs/client/msg-units.md §7 r1 (0x00470B70 for type 1, no 1.14d sender)
- src/bridge/msg/units.rs:364 → specs/client/msg-units.md §3 r4.5; model.md §12 (nearest free point 0x0064E7B0)
- src/bridge/msg/units.rs:570 → specs/client/model.md §15 r1 (objects.txt shrine facts; wiring only)
- src/bridge/update.rs:28 → specs/client/model.md §5 r5 (flag 0x800000 set by room free 0x0061A840)

### client: Tests (pc2-s4-items.md)
- tests/e2e_full_loop.rs:2156 → specs/items/inventory-moves.md §8.1 r4; sim/pathing.md §9.5 unit distance (wiring)
- tests/e2e_full_loop.rs:2257 → specs/sim/pathing.md §9.5 r3 (player walk/run keeps stop distance 0)

## d2-sim / server / data / proto

### sim: skills / bodies (specs/skills)
- crates/d2-sim/src/skills/levels.rs:1088 → specs/skills/levels.md §3.6 : hit class is a plain store (+0x60 := h)
- crates/d2-sim/src/skills/use_/bodies/b4_helpers.rs:284 → specs/skills/bodies-3.md Open question 8 : non-throw item throw mastery = 0 (code differs: reads stats 342/343)
- crates/d2-sim/src/skills/use_/bodies/b4_helpers.rs:699 → specs/skills/bodies-4.md Edge case 1 : step <= 0 hangs in 1.14d; d2rs creates nothing (deliberate)
- crates/d2-sim/src/skills/use_/bodies/helpers2.rs:281 → specs/skills/bodies-3.md §2 answer 3 : R invalid writes no mode
- crates/d2-sim/src/skills/use_/bodies/helpers2.rs:449, :603 → specs/skills/bodies-3.md §2 answer 4 : itemevent2 same branch; clamps only when ilvl = 0
- crates/d2-sim/src/skills/use_/bodies/helpers2.rs:1000 → specs/skills/bodies-3.md §2 answer 5 : negative mastery = fatal assert (code returns silently)
- crates/d2-sim/src/skills/use_/bodies/helpers.rs:834 → specs/skills/bodies.md §2.13 : func 32-49 table slots null, return 0
- crates/d2-sim/src/skills/use_/bodies/helpers.rs:1001, :1010 → specs/skills/bodies.md §2.14 : result |= 0x4000 unconditional
- crates/d2-sim/src/skills/use_/bodies/b4_more.rs:779 → specs/skills/bodies-4.md Open question 4 (monsters/ai-bodies-2.md §13.1) : spawn info for non-Baal
- crates/d2-sim/src/skills/use_/bodies/b4_more.rs:871 → specs/skills/bodies-4.md Open question 6 : point is what 0x0056D2C0 wrote (code differs: not (0,0))
- crates/d2-sim/src/skills/use_/bodies/helpers3.rs:143 → specs/skills/bodies-3.md §2 answer 6 : TH from monlvl TH, pct
- crates/d2-sim/src/skills/use_/bodies/helpers3.rs:966, b3_lvl18.rs:394, b3_lvl24.rs:140, :450 → specs/skills/bodies-3.md §2 answer 8 : negative step fatal; k outside 0..2 no class; rewind only if K2 exists (code differs); pt := 0
- crates/d2-sim/src/skills/use_/bodies/helpers3.rs:1211 → specs/skills/bodies-3.md §2 answer 7 : Leap Attack strike hit clause order
- crates/d2-sim/src/skills/use_/bodies/b4_mon.rs:1225 → specs/skills/bodies-3.md Open question 5 (monsters/umod-callbacks.md §3.1) : unit finder
- crates/d2-sim/src/skills/use_/bodies/b4_mon.rs:1535 → specs/skills/bodies-3.md Open question 6 : P +0x10/+0x12 are target x,y; literal rule
- crates/d2-sim/src/skills/use_/bodies/starts.rs:214 → specs/sim/stat-lists.md §8.9 : TEMPONLY list freed at next mode change
- crates/d2-sim/src/combat/events.rs:1495 → specs/skills/bodies.md §2.18 : next record read after call (code differs: snapshot)
- crates/d2-sim/src/stats/lists.rs:53 → specs/sim/stat-lists.md §1 rule 4 : only null list has original behaviour

### sim: monsters / AI (specs/monsters)
- crates/d2-sim/src/monsters/ai/bodies3.rs:641, bodies2.rs:832 → specs/monsters/ai.md §7.1 + Open question 13 : request holds both T and point, T wins
- crates/d2-sim/src/monsters/ai/bodies5.rs:715 → specs/monsters/ai.md §7.1 (ai-bodies-5.md OQ7) : request byte +0x15 = path type 100/101
- crates/d2-sim/src/monsters/ai/mod.rs:176 → specs/monsters/ai.md Open question 12 : G process-wide (code keeps per store)
- crates/d2-sim/src/monsters/ai/functions.rs:896 → specs/monsters/ai-bodies.md §9.10 : exact clamp/count down
- crates/d2-sim/src/monsters/ai/bodies6.rs:1051 → specs/monsters/ai-bodies-6.md §8 step 1 : params 3/4 unwritten, unread
- T = 0 unreachable: crates/d2-sim/src/monsters/ai/bodies6.rs:1324, common.rs:230, :244, bodies.rs:76, :170, :281, :332, bodies7.rs:129, :193, tactics.rs:522, bodies4.rs:60 → specs/monsters/ai.md "Target 0 in mode-1 and mode-4 bodies" (§2.3): unreachable, assert
- crates/d2-sim/src/monsters/ai/tactics.rs:79, :179 → specs/monsters/ai.md §7.1 mode request record : builder zeroes record, point (0,0)
- crates/d2-sim/src/monsters/ai/target.rs:207 → specs/monsters/ai.md §5.2 step 5.3 : alternative's own no-size distance
- crates/d2-sim/src/monsters/ai/bodies.rs:1125 → specs/monsters/ai-bodies.md §9.28 (ai.md OQ17) : state > 3 above-ground steps
- crates/d2-sim/src/monsters/ai/npc.rs:71, :76 → specs/monsters/ai-bodies.md §9.31 : no room = out of town; Else = first 30% roll
- crates/d2-sim/src/monsters/ai/npc.rs:235, :255 → specs/monsters/ai-bodies.md §9.32 : setup follows leave; other classes use cain1 Act 1 functions (code does nothing: differs)
- crates/d2-sim/src/wiring/action/ai.rs:158 → specs/monsters/ai-bodies.md §9.26 : state toggle 0x00639DB0
- crates/d2-sim/src/monsters/ai/tactics.rs:389, :418 → specs/monsters/ai.md §8 : 0x0058EEF0 / allocator 0x0058EC90 semantics
- crates/d2-sim/src/monsters/ai/tactics.rs:458, :500 → specs/monsters/ai.md §7.2 table : coordinate walk/run step count 1
- crates/d2-sim/src/wiring/path/monsters.rs:94 → specs/monsters/ai.md §7.1, §7.5 : request built per AI change, path target, budget 20
- crates/d2-sim/src/monsters/ai/bodies7.rs:648 → specs/monsters/ai-bodies-7.md §16 : null owner unreachable
- crates/d2-sim/src/monsters/population/seams.rs:140, :151, :154 → specs/monsters/population.md Open question 4 : 0x0058F030(game,unit,GUID,1,0,0); 0x005B24E0 args
- crates/d2-sim/src/monsters/population/seams.rs:159 → specs/monsters/population.md Open question 3 : barricade objects 571/572
- crates/d2-sim/src/monsters/population/region.rs:212 → specs/monsters/population.md Open question 6 : 4 is not an alignment
- crates/d2-sim/src/monsters/population/placement.rs:174 → specs/monsters/population.md Open question 5 : n = 1 tests index 1 (past end; original bug)
- crates/d2-sim/src/monsters/init/message.rs:34 → specs/monsters/init.md §24 full layout rule 3 : 1 presence bit
- crates/d2-sim/src/monsters/init/callbacks.rs:249 → specs/monsters/umod-callbacks.md §15.2 : fewer than 349 rows = null-record read

### sim: drlg / worldgen
- crates/d2-sim/src/drlg/outdoor/jungle.rs:320 → specs/drlg/outdoor-act3-act5.md §2.7 table row 15 + OQ7 : zeros, fatal 0x78C
- crates/d2-sim/src/drlg/room.rs:688 → specs/drlg/rooms.md §4.6 rule 9 : head status := s by 0x0061B7E0
- crates/d2-sim/src/wiring/worldgen/init_units.rs:210 → specs/monsters/init.md §17.3 rule 2 (data/runtime-maps.md §2) : 0x005A0070 reads the matrix

### sim: path / units / action wiring
- crates/d2-sim/src/wiring/path/units.rs:128 → specs/sim/path-placement.md §5.3 rule 4 (+ §5.2 table) : removal clears with force
- crates/d2-sim/src/wiring/path/units.rs:311 → specs/sim/path-placement.md §2.5 : set0x10 = 0; monster calls named
- crates/d2-sim/src/wiring/path/walk.rs:426 → specs/sim/pathing.md §8.1 : +0x0C is the E-flags word
- crates/d2-sim/src/wiring/action/waypoints.rs:132 → specs/world/waypoints.md §6.3 rule 1 : 0x00554D00 = interact unit
- crates/d2-sim/src/wiring/action/switch.rs:218 → specs/sim/intents-events.md §7.9 rule 3 : 0x26 carries +0xA4 text
- crates/d2-sim/src/wiring/action/switch.rs:281 → specs/sim/intents-events.md §7.2 (0x82 layout line ~672) : class 59 owner name
- crates/d2-sim/src/wiring/action/pending.rs:712 → specs/data/runtime-maps.md §3 : shift 6, mask 0x3F (inferred from "stuff")
- crates/d2-sim/src/wiring/action/unit_update.rs:85 → specs/sim/intents-events.md §7.1 rule 2.1, §7.2 : add messages
- crates/d2-sim/src/wiring/action/unit_update.rs:233 → specs/sim/intents-events.md §7.5 : clean-up steps listed
- crates/d2-sim/src/wiring/action/unit_update.rs:328 → specs/sim/units.md §4.6 "Death and dead functions" (OQ9) : setter 0x00553570
- crates/d2-sim/src/wiring/action/vitals_sync.rs:38 → specs/combat/vitals.md §5.2 : cache starts all 0
- crates/d2-sim/src/wiring/action/vitals_sync.rs:116 → specs/combat/vitals.md §5.1 rule 3 : assert
- crates/d2-sim/src/wiring/action/vitals_sync.rs:118 → specs/combat/vitals.md §5.1 rule 4 : 0x0052DA00 not run in Original

### sim: missiles
- crates/d2-sim/src/missiles/create.rs:88 → specs/missiles/missiles.md §R8.1 step 3 : signed compare
- crates/d2-sim/src/missiles/mod.rs:221 → specs/missiles/missiles.md §R4.2 : collide table (modes 0-8)
- crates/d2-sim/src/missiles/hit.rs:349 → specs/missiles/missiles.md §R5 step 1 : null record unreachable, assert
- crates/d2-sim/src/missiles/bodies.rs:395, :414 → specs/missiles/missiles.md §R9.6 area_damage : attacker = owner; evade sets no bit
- crates/d2-sim/src/missiles/bodies.rs:620 → specs/missiles/missiles.md §R9.6 body 4 : len from sHitPar2 > 0 else auralencalc

### sim: items / inventory / bitstream
- crates/d2-sim/src/items/moves/handlers.rs:552 → specs/items/inventory-moves.md §7.8 (line ~250) : E unlink failure fatal (code ignores)
- crates/d2-sim/src/items/moves/handlers.rs:1132 → specs/items/inventory-moves.md §7.17 (line ~422) : no hireling = used on player (code returns)
- crates/d2-sim/src/items/moves/handlers.rs:1464 → specs/items/inventory-moves.md §7.23 (line ~528) : failed copy fatal
- crates/d2-sim/src/items/moves/ground.rs:53, :97 → specs/items/inventory-moves.md §8.1 rules 5, 7 : failures fatal / ignored
- crates/d2-sim/src/items/moves/ground.rs:456 → specs/items/inventory-moves.md §9.3 : unlink failure fatal
- crates/d2-sim/src/items/moves/ground.rs:520 → specs/items/inventory-moves.md §10.2 : failed pile skipped, continue (code stops)
- crates/d2-sim/src/wiring/inventory/pending.rs:61 → specs/items/inventory-moves.md §7.12 + world/cube.md §8 : unlink + 0x00555600 only
- crates/d2-sim/src/wiring/inventory/queries.rs:175 → specs/items/properties.md §10.2 (ladder) + specs/items/generation.md §12.2 : recharge body
- crates/d2-sim/src/wiring/economy/item_records.rs:98 → specs/world/vendors-2.md §7.3.1 rule 5 + sim/rng.md §5.1 : init_low {x,666}
- crates/d2-sim/src/items/bitstream/read.rs:119, crates/d2-sim/src/items/bitstream.rs:344, crates/d2-proto/src/item_bits.rs:268 → specs/items/bitstream.md §3 rule 3 : name 16 chars then 0

### sim: vendors / hirelings / session (PC 2)
- crates/d2-sim/src/world/vendors/trade.rs:223 → specs/world/vendors.md §7.1 rule 2 : GUID = requested (code differs: -1)
- crates/d2-sim/src/world/vendors/trade.rs:480, :559 → specs/world/vendors.md §7.2 rule 8, §8.1 : code agrees
- crates/d2-sim/src/world/vendors/price.rs:174, :188, :217, :443 → specs/world/vendors.md §9.2 : code agrees
- crates/d2-sim/src/world/vendors/price.rs:546 → specs/world/vendors.md §9.4 : missing normal code fatal (code: price 0)
- crates/d2-sim/src/world/vendors/gamble.rs:77, :103 → specs/world/vendors.md §5.1 : code agrees
- crates/d2-sim/src/world/vendors/gamble.rs:117 → specs/world/vendors.md §5.1 : missing rin/amu gives item 0 (code differs)
- crates/d2-sim/src/world/vendors/store.rs:87, :167 → specs/world/vendors.md §3.1 rules 1, 2 : class 0 -> null; null creation fatal (code differs)
- crates/d2-sim/src/world/vendors/store.rs:232, :280 → specs/world/vendors.md §3 : code agrees
- crates/d2-sim/src/world/vendors.rs:474 → specs/world/vendors.md §7.2 rule 7 : mask 4 (code uses 0)
- crates/d2-sim/src/world/hirelings/items.rs:64 → specs/world/hirelings-2.md §17 rule 2 : failed duplicate
- crates/d2-server/src/adapters/handlers/world/wired.rs:696 → specs/world/vendors.md §7 preamble : call order, 0x2A last (code: follows)
- crates/d2-server/src/adapters/handlers/items/vendor_inv.rs:319 → specs/world/vendors.md §7.2 rule 9 : 0x9D action 5 flags 0x20
- crates/d2-server/src/adapters/session.rs:142 → specs/formats/d2s-load.md §8 rules 1-4 : new character record (join recording still pending, OQ4)
- crates/d2-data/src/sounds.rs:62 → specs/audio/sound-table.md §2 (line ~157-168) : 22 columns, 13 EAX columns

# 2. NOT SPECIFIED — for PC 1 (owning spec | address/topic | code site | provisional | settled by; HIGH-PRIORITY CAPTURE = RNG order or wire/saved layout)

## d2-client
(owning spec | address/topic | code site)
- ui/menus.md (front-end create-game / character-select; server side already in sim/intents-events.md §3 row 0x67) | client sender of C→S 0x67: game name, template, arena, bytes 43–44 | src/app/single_player.rs:173 | provisional: zeros/defaults as now (no server rule reads them) | settled by: packet capture of a real 0x67 from the menus (HANDOFF §7 PC 2 recording list); HIGH-PRIORITY CAPTURE (wire byte layout)
- client/msg-stats-items.md OQ 3 (with items/bitstream.md) | item stream header `0x0062E410` and per-action placement; item stat lists (stat 204, flag 0x40); inventory nodes of P / owners | src/world_view/model_feed.rs:48, src/bridge/msg/stats_items.rs:105, src/bridge/msg/items.rs:82, 163, src/bridge/msg/unit_misc.rs:166 | provisional: none obvious (D2MOO-style item header; mode byte +8, flags +0x0C, page +0x10, body loc +0x11 per msg-stats-items.md §2 r5.3) | settled by: Ghidra read of 0x0062E410 plus a join/trade packet recording with items; HIGH-PRIORITY CAPTURE (wire byte layout)
- ui/messages.md §14 / client/msg-ui.md §16 r4.3 | `0x00661400` / `0x00661440` (first entry m, m2 of NPC text list) and effect of list start `0x006616E0`; only the one-entry kind-0 case is pinned | src/ui/msg_ui.rs:111 | provisional: m = string id of the single kind-0 entry; other shapes none obvious | settled by: capture of NPC dialogs with 2+ list entries (HANDOFF §7 PC 2 recording list)
- ui/messages.md §14 / client/msg-ui.md §16 | writer of UI global `[0x007C0C68]` (read by 0x28 dialog branch B0) | src/ui/msg_ui.rs:190 | provisional: stays 0 (matches recorded 0x28 of seq 37353) | settled by: Ghidra xref scan of [0x007C0C68] writers (no capture needed)
- render/lighting.md §10 r4 (+ client/msg-ui.md §1 r7) | `0x00410A80`: what the client "draw"/timer returns (sync timer per sim/intents-events.md §3 step 4); needed for 0x89 id 13 and 0x5D counter | src/bridge/msg/lighting.rs:74 | provisional: client draw of the local player's seed + 90 (lighting.md §10 wording); intents-events.md calls it a host sync timer, so possibly a host input | settled by: Ghidra read of 0x00410A80 plus a Terror's End trace; HIGH-PRIORITY CAPTURE (RNG draw order)
- client/msg-units.md §8 r3 | name comparison of `0x00479360` (case, length) in the roster lookup | src/bridge/msg/roster.rs:60 | provisional: exact byte compare up to the NUL (as coded) | settled by: Ghidra read of 0x00479360; no capture needed
- client/msg-ui.md OQ 2 → client/msg-units.md (unit flag word +0xC4) | client writers of flag 0x10000 (dead) and 0x200 (NpcHeal test) | src/bridge/world.rs:246, src/bridge/msg/unit_misc.rs:208 | provisional: 0x10000 set only by the dead-state flow already in the server sync (0x15 flag); 0x200 treated clear | settled by: Ghidra xref of all +0xC4 writers (msg-ui OQ 2)
- client/model.md §14 (OQ 10) | pet record fields +0x24… written by 0x81 and the readers of +0x1C (UI) | src/bridge/world.rs:322 | provisional: store the three 0x81 values only (as coded) | settled by: recording with a hireling (0x7A/0x81; model.md OQ 10, HANDOFF §7 PC 2 recording list)
- client/model.md OQ 1, OQ 2 (Phase 6 unit-modes spec) | per-type client unit updates (player, monster, object, missile, item mode machines `0x004AFF60` etc.; local walk prediction `0x00463390`) | src/bridge/update.rs:6 | provisional: none obvious (no client modes beyond what messages state; local player follows server position) | settled by: Phase 6 unit-modes spec plus frames-raw client_update counter vs server tick (camera.md OQ 5/8); HIGH-PRIORITY CAPTURE (client seed draws in animation)
- client/msg-ui.md §9 r2 (or client/msg-units.md) | writer of client monster data +0x3C (`0x004AE130` reader) | src/bridge/output.rs:78 | provisional: field stays -1 (act5pow sound 4607 path) | settled by: Ghidra xref of +0x3C writes in monster data
- client/stat-lists.md §3 r2 | `0x00463C00(value)` (stat 172) and which model fields it writes (`0x00623F50` is sim/units.md §4.7) | src/bridge/msg/states.rs:74 | provisional: no model field changes (as coded) | settled by: Ghidra read of 0x00463C00
- render/camera.md §8 (callers in effect specs) | callers of `0x00476A80` and their (A, t1, t2, t3) other than msg-ui.md §9 (6, 4000, 10000, 4000) | src/world_view/feed.rs:73 | provisional: only the msg-ui.md (6, 4000, 10000, 4000) shake; others none | settled by: Ghidra callers of 0x00476A80 (effect specs); HIGH-PRIORITY CAPTURE (shake consumes 2 client-seed draws per frame, RNG draw order)

Marker count for this section: 17 markers in 12 groups.

## d2-sim / server
(format: owning spec | address/topic | code site | provisional | settled by; HP = HIGH-PRIORITY CAPTURE)
- vendors-2.md §7.3 step 5 / inventory-moves.md §7.19 | flag args (0,1,0,0) of 0x00562660 | crates/d2-sim/src/wiring/inventory/copy.rs:105 | provisional: same as 1,1,1,1 path minus hand-offs, none obvious | settled by: static read of 0x00562660 (bin) or copy-item recording
- vendors-2.md §7.3.1 (bitstream.md §4.6) | flags of a decoded set list (0x2040 vs 0x40) | crates/d2-sim/src/wiring/economy/item_records.rs:35 | provisional: 0x2040 | settled by: save/copy round trip capture. HP (saved bytes)
- items/bitstream.md §4.2, §4.1 rule 11 | reader inverse of prefix/auto affix ids <= P/A | crates/d2-sim/src/items/bitstream/read.rs:150, :301 | provisional: prefix p'+P when p' != 0 (prefix ids always > P) | settled by: item-record capture with prefix near P. HP (wire layout)
- sim/units.md §2 | alloc values of +0x64/+0x68/+0x6C | crates/d2-sim/src/units/record.rs:165 | provisional: inactive (-1, 6, 0) as npc.md §? reset | settled by: bin read 0x00555230
- sim/units.md §4.5 | rest of 0x00580EC0, character save in 0x0057FCA0 | crates/d2-sim/src/wiring/action/death.rs:17 | provisional: none obvious | settled by: bin read; save capture on death. HP (saved bytes)
- combat/vitals.md §4.7 | no-corpse path of 0x0057F700 | crates/d2-sim/src/wiring/action/death.rs:135 | provisional: field unchanged | settled by: bin read
- skills/bodies.md §2.4 | stale target GUID in skill_missile target lookup | crates/d2-sim/src/wiring/path/missiles.rs:34 | provisional: fall through to path target point | settled by: bin read 0x0056D2C0
- skills/bodies-2.md §2.3 | negative n in step count | crates/d2-sim/src/wiring/path/missiles.rs:71 | provisional: unreachable (frame counts) | settled by: none needed
- skills/bodies-3.md §3.8 | coordinates fed by 0x0064FDC0 (sub-tile vs 16.16) | crates/d2-sim/src/wiring/path/missiles.rs:87 | provisional: sub-tile | settled by: bin read; direction trace. HP? only if direction diverges (RNG-free)
- missiles/missiles.md §R2.3 step 16 | town-access arg of missile build | crates/d2-sim/src/wiring/path/walk.rs:226 | provisional: 0 | settled by: bin read (missile compute likely ignores)
- monsters/ai.md §7.1 | 0x005A63F0 type write via set type vs direct; town-access arg | crates/d2-sim/src/wiring/path/monsters.rs:99 | provisional: set type, 0 | settled by: bin read; path trace
- sim/pathing.md §9.1 | do monster state 13/22 results gate the step | crates/d2-sim/src/wiring/path/monsters.rs:231 | provisional: no gate (player rule 9.2 step 2) | settled by: tick recording of a monster walk
- sim/units.md §4.6 | monster start bodies | crates/d2-sim/src/wiring/path/monsters.rs:213 | provisional: mode set 0x00553570 started | settled by: bin read (spec work)
- monsters/ai.md §1.4 | anim mode neutral before SplEndGeneric inline think | crates/d2-sim/src/monsters/ai/mod.rs:628 | provisional: not set (spec lists neutral only for table-1 modes) | settled by: bin read 0x005A8030 / think schedule recording
- monsters/ai.md §2.3 | can-walk test in 0x005DE9D0 | crates/d2-sim/src/monsters/ai/target.rs:287 | provisional: collision test only | settled by: bin read; wander-draw recording (RNG). HP
- monsters/ai-bodies-4.md §8 | Diablo k=1 walk with X = 0 | crates/d2-sim/src/monsters/ai/bodies4.rs:928 | provisional: (0,0) | settled by: bin read (likely unreachable)
- monsters/ai-bodies-6.md §2 pet move k 0 | "Else the same to the midpoint" scope | crates/d2-sim/src/monsters/ai/bodies6.rs:246 | provisional: else of coordinate-index test | settled by: pet-move recording (draws). HP
- ai-bodies-6.md §13 step 5 | args of the own-position skill check | crates/d2-sim/src/monsters/ai/bodies6.rs:1337 | provisional: (Skill1, T, own x, y) | settled by: bin read
- ai-bodies-6.md §24.1 step 7 | pet follow scope after d > aip3 | crates/d2-sim/src/monsters/ai/bodies6.rs:1955 | provisional: always evaluated | settled by: DruidWolf recording. HP
- ai-bodies-6.md §25 step 4 | "has its entry" = highest entry? | crates/d2-sim/src/monsters/ai/bodies6.rs:2122 | provisional: highest entry 0x006439F0 with bonus (skills/levels.md highest_entry) | settled by: bin read
- ai-bodies-7.md §18 init, §19 step 2, §27 init (x2, init 143) | "O's entry/level of K" lookup | crates/d2-sim/src/monsters/ai/bodies7.rs:749, :1082, :1528, :1544 | provisional: highest entry with bonus | settled by: bin read 0x006442A0 callers
- ai-bodies-7.md §18 Allowed | "pettype valid" | crates/d2-sim/src/monsters/ai/bodies7.rs:792 | provisional: != 0xFF | settled by: bin read 0x005EAB20
- ai-bodies-7.md §27 step 8 | independence of n/near/best tests | crates/d2-sim/src/monsters/ai/bodies7.rs:1672 | provisional: independent | settled by: bin read 0x005EB6D0
- ai-bodies-7.md §27 step 12 | aitype 1 "lacks it -> s := 0" ends case; aitype 12 non-progressive rule | crates/d2-sim/src/monsters/ai/bodies7.rs:1929, :2009 | provisional: ends case; 12 takes rule | settled by: bin read; Shadow Master recording (draws). HP
- monsters/population.md §6.3 | creation mode; room of nearest free point | crates/d2-sim/src/monsters/population/spawn.rs:86, :117 | provisional: mode 1; point's room | settled by: bin read 0x005A09E0. HP? (spawn state)
- monsters/population.md §10.3 | children owner data "as in 10.2.3" conditional on SetBoss? | crates/d2-sim/src/monsters/population/spawn.rs:307 | provisional: unconditional | settled by: bin read 0x005B2570
- monsters/population.md §11.5 | special preset ids not in TSV | crates/d2-sim/src/monsters/population/preset.rs:217 | provisional: nothing | settled by: bin read switch
- monsters/population.md §11.4 row hcIdx 10 | seed of roll(5) and mode | crates/d2-sim/src/monsters/population/preset.rs:441 | provisional: boss unit seed, mode 1 | settled by: Radament spawn RNG recording. HP (RNG)
- monsters/population.md §4, §3.1 | null region | crates/d2-sim/src/monsters/population/room.rs:50, :158 | provisional: no population | settled by: none needed (unreachable)
- drlg/maze.md §6 | maze type not in the table | crates/d2-sim/src/drlg/maze/layout.rs:331 | provisional: draws r, stamps nothing | settled by: none (no live level)
- sim/intents-events.md §8.2 rule 3.6 | hot-key slot contents of a new client record | crates/d2-server/src/adapters/handlers/player.rs:219 | provisional: zero-filled stub = skill 0 slots sent (code: unbound) | settled by: new-character join capture (d2s-load.md OQ4). HP (wire)
- sim/intents-events.md §9 rule 6 | side effects of 0x0053FDF0 | .../handlers/player.rs:511 | provisional: none | settled by: bin read
- sim/intents-events.md §9 rule 8 | params 3,4 of the merc command (0x0058EF40) | .../handlers/player.rs:566 | provisional: 0,0 | settled by: bin read
- sim/pathing.md §1.2 | behaviour without path provider | crates/d2-server/src/adapters/handlers/player/action.rs:226 | provisional: d2rs wiring choice, no spec needed | settled by: none
- sim/tick.md §3 step 1 / render/lighting.md §9.3 | server A and L args of 0x0061C040 | crates/d2-sim/src/world/environment.rs:10 | provisional: A = act index, L creation value | settled by: tick recording of cycle index change (0x53 message). HP (wire)

# 3. PENDING-BY-DESIGN

- src/audio/sound_table/system.rs:601 → specs/audio/sound-table.md OQ 10 (async-load latency; recording ST-4), OQ 13 (Async Only tick); d2rs has no load latency | provisional: loads complete next tick (as coded) | settled by: HANDOFF §7 sound recording ST-4/ST-7 (record_sound.py hooks)
- src/audio/sound_table/system.rs:1100 → specs/audio/sound-table.md §6.6 r3, OQ 12 (recording: tick at which one-shots finish; d2rs derives from tick) | provisional: derive finish from tick x 40 ms (as coded) | settled by: sound-table.md OQ 12 recording ST-4
- src/world_view/mod.rs:538 → no spec by design: "spec: none, design C2"; atlas page eviction is the residency cache's (client/assets.md §A5), d2rs design choice | provisional: error when the atlas is full | settled by: none needed (d2rs design)

- crates/d2-sim/src/combat/vitals/experience.rs:15 → specs/sim/stat-lists.md Open question 1 (PC 2 recording list) : x87 precision control; provisional: full precision; settled by: max-life change recording (check_stats.py). HP (RNG-free but byte-exact stats)
- crates/d2-sim/src/wiring/path/walk.rs:79 → specs/sim/path-placement.md §10 rule 1 (pending + d2rs design choice) : teleport of static-path unit; provisional: static set with footprint restamp; settled by: hook on 0x00554EA0 callers

# Other notes

## client: Template text, not a marker (placeholders in format strings; excluded from totals)
src/ui/text.rs:205, src/rules/view.rs:401, src/rules/draw_order/edges.rs:33, 37, src/rules/draw_order/background.rs:56, 60, 63, src/rules/draw_order/mod.rs:87, src/rules/unit_composite.rs:61, src/world_view/mod.rs:82, src/bridge/dispatch.rs:73

## client: Convention / meta mentions, not specific markers (module docs describing the TODO(spec: …) convention; excluded from totals)
src/gpu_compositor/mod.rs:23, src/ui/mod.rs:14, src/scene/mod.rs:14, src/audio/mod.rs:9, src/world_view/mod.rs:23, 210, src/world_view/feed.rs:10, src/composite/mod.rs:7, 206, src/rules/unit_composite.rs:13, src/app/play.rs:13, src/verify/capture_case/scene_source.rs:84

## client: Non-d2-client "no spec yet" notes (nospec.txt)
- crates/d2-client/src/controls/mod.rs:47 (not in todos.txt, so not counted above) → ANSWERED: specs/ui/controls.md §3, §B4 (original preset = 114 bindings of table 0x00712220).
- specs/render/unit-composite.md:703 ("overlay owner, no spec yet; 0x00470390") → ANSWERED/stale: specs/render/overlay.md §2 (create 0x00470390), §3, §4 (the three draws on the local player's seed).
- specs/render/draw-order.md:422 ("drawing has no spec yet makes the frame an error") → ANSWERED/stale wording: all passes specified (draw-order-2.md §11–§14); the sentence is the d2rs rule that an unspecified item is an error, not a gap.
## specs/ "no spec yet" notes (not code markers)
- specs/skills/bodies.md:1429 : "srvst 9, not yet specified" is stale; srvst 9 Fend is specified in skills/bodies-2b.md §7.4. Wording fix only.
- specs/monsters/ai-bodies-2.md:59, ai-bodies-3.md:55, ai-bodies-4.md:54 : prose ("AIs not yet specified in ai-bodies.md are written here"), not gaps.
- specs/world/objects.md:1080 : Open question 13 says "not yet specified" but continues "Answered: world/objects-2.md §20"; stale wording (0x00559A30 also in items/generation.md, quality.md).
- specs/sim/unit-events.tsv rows 55-63 : "trade: no spec yet" for player event 13 handler 0x005689D0 (f+1/125/5/25/4 reschedules); NOT SPECIFIED, owner sim/units.md §6.1 (or a player-trade spec, multiplayer only, outside Phases 0-6). provisional: reschedule only; settled by: none (out of scope).
