# Uncovered-rules sort (triage)

Base `claude/specs-staging-7`. Buckets: A code exists (test+`Covers:` missing), B plain translation missing, C needs new wiring, X not code / out of scope.

**Caveat:** buckets were assigned per section from file-level evidence (spec addresses/names grepped in crates/, `// Spec:` modules), not rule-by-rule reading. Treat A and C as soft; verify when writing each test. Edge-case narration is X by instruction, though some describes real behaviour.

## Totals

| spec | A | B | C | X |
|---|---|---|---|---|
| specs/combat/damage.md | 19 | 0 | 0 | 0 |
| specs/combat/events.md | 18 | 0 | 0 | 0 |
| specs/combat/hit.md | 16 | 0 | 0 | 1 |
| specs/combat/vitals.md | 6 | 0 | 0 | 0 |
| specs/data/calc-expressions.md | 3 | 0 | 0 | 0 |
| specs/data/fixups.md | 2 | 0 | 0 | 0 |
| specs/data/loading.md | 7 | 0 | 0 | 6 |
| specs/data/patch-layers.md | 3 | 0 | 0 | 0 |
| specs/data/schema.md | 0 | 0 | 0 | 3 |
| specs/data/txt-format.md | 0 | 0 | 0 | 1 |
| specs/formats/animdata.md | 3 | 0 | 0 | 1 |
| specs/formats/d2s-appearance.md | 0 | 4 | 17 | 1 |
| specs/formats/d2s-legacy.md | 0 | 0 | 0 | 68 |
| specs/formats/d2s-load.md | 16 | 0 | 0 | 1 |
| specs/formats/d2s.md | 38 | 0 | 2 | 4 |
| specs/formats/tbl.md | 0 | 0 | 0 | 1 |
| specs/formats/wav.md | 2 | 0 | 0 | 1 |
| specs/items/affixes.md | 3 | 0 | 0 | 16 |
| specs/items/bitstream-legacy.md | 0 | 0 | 0 | 88 |
| specs/items/bitstream.md | 1 | 0 | 0 | 1 |
| specs/items/generation.md | 22 | 0 | 0 | 12 |
| specs/items/inventory-moves.md | 10 | 0 | 24 | 0 |
| specs/items/inventory.md | 11 | 0 | 22 | 0 |
| specs/items/properties.md | 0 | 0 | 8 | 1 |
| specs/items/quality.md | 1 | 0 | 0 | 18 |
| specs/items/treasure.md | 3 | 0 | 10 | 0 |
| specs/missiles/bodies-2.md | 43 | 0 | 0 | 0 |
| specs/missiles/bodies.md | 34 | 0 | 0 | 0 |
| specs/missiles/missiles.md | 6 | 0 | 0 | 1 |
| specs/monsters/ai-bodies-2.md | 0 | 0 | 0 | 1 |
| specs/monsters/ai-bodies-3.md | 1 | 0 | 0 | 1 |
| specs/monsters/ai-bodies-4.md | 1 | 0 | 0 | 1 |
| specs/monsters/ai-bodies-5.md | 0 | 0 | 0 | 1 |
| specs/monsters/ai-bodies-6.md | 47 | 0 | 0 | 1 |
| specs/monsters/ai-bodies-7.md | 34 | 0 | 0 | 1 |
| specs/monsters/ai-bodies.md | 4 | 0 | 0 | 0 |
| specs/monsters/ai.md | 26 | 0 | 0 | 0 |
| specs/monsters/init.md | 16 | 0 | 0 | 2 |
| specs/monsters/population.md | 8 | 0 | 0 | 0 |
| specs/monsters/umod-callbacks.md | 23 | 0 | 2 | 1 |
| specs/monsters/umod-init-bodies.md | 26 | 0 | 0 | 0 |
| specs/sim/intents-events.md | 35 | 0 | 0 | 16 |
| specs/sim/path-placement.md | 6 | 0 | 0 | 1 |
| specs/sim/pathing.md | 17 | 5 | 0 | 3 |
| specs/sim/pets.md | 1 | 0 | 0 | 0 |
| specs/sim/rng.md | 25 | 0 | 1 | 11 |
| specs/sim/stat-lists.md | 6 | 0 | 0 | 0 |
| specs/sim/stats.md | 0 | 0 | 0 | 1 |
| specs/sim/tick.md | 7 | 0 | 0 | 6 |
| specs/sim/units.md | 9 | 11 | 15 | 2 |
| specs/skills/bodies-2.md | 186 | 0 | 0 | 1 |
| specs/skills/bodies-2b.md | 178 | 0 | 0 | 0 |
| specs/skills/bodies-3.md | 245 | 0 | 0 | 2 |
| specs/skills/bodies-4.md | 167 | 0 | 0 | 2 |
| specs/skills/bodies.md | 200 | 0 | 0 | 2 |
| specs/skills/descriptions.md | 0 | 0 | 78 | 0 |
| specs/skills/levels.md | 29 | 0 | 0 | 0 |
| specs/skills/use.md | 1 | 0 | 0 | 0 |
| specs/ui/automap.md | 0 | 34 | 22 | 14 |
| specs/ui/control-panel.md | 0 | 49 | 0 | 2 |
| specs/ui/controls.md | 0 | 66 | 15 | 9 |
| specs/ui/inventory.md | 1 | 43 | 0 | 6 |
| specs/ui/menus.md | 4 | 21 | 0 | 1 |
| specs/ui/messages.md | 0 | 49 | 0 | 3 |
| specs/ui/panels-2.md | 24 | 27 | 0 | 5 |
| specs/ui/panels-3.md | 4 | 31 | 0 | 8 |
| specs/ui/panels.md | 19 | 5 | 0 | 1 |
| specs/ui/text.md | 7 | 12 | 0 | 3 |
| specs/client/assets.md | 1 | 0 | 0 | 2 |
| specs/client/audio.md | 0 | 0 | 0 | 2 |
| specs/client/bridge.md | 43 | 0 | 0 | 2 |
| specs/client/model.md | 6 | 10 | 12 | 5 |
| specs/client/msg-skills.md | 8 | 0 | 0 | 3 |
| specs/client/msg-stats-items.md | 1 | 1 | 0 | 0 |
| specs/client/msg-ui.md | 12 | 7 | 0 | 2 |
| specs/client/msg-units.md | 1 | 4 | 0 | 1 |
| specs/client/render-pipeline.md | 1 | 0 | 0 | 1 |
| specs/client/stat-lists.md | 2 | 3 | 8 | 2 |
| specs/client/ui.md | 0 | 0 | 0 | 8 |
| specs/world/cube.md | 4 | 0 | 1 | 3 |
| specs/world/hirelings-2.md | 2 | 1 | 0 | 3 |
| specs/world/hirelings-ai.md | 1 | 0 | 0 | 1 |
| specs/world/hirelings.md | 9 | 0 | 1 | 13 |
| specs/world/npc.md | 0 | 0 | 0 | 3 |
| specs/world/object-population.md | 47 | 0 | 0 | 21 |
| specs/world/objects-2.md | 35 | 0 | 9 | 16 |
| specs/world/objects-client.md | 0 | 0 | 65 | 8 |
| specs/world/objects.md | 15 | 0 | 1 | 19 |
| specs/world/quests-act1-rest.md | 7 | 0 | 0 | 4 |
| specs/world/quests-act1.md | 2 | 0 | 0 | 0 |
| specs/world/quests-act2-2.md | 7 | 0 | 0 | 6 |
| specs/world/quests-act2.md | 8 | 0 | 0 | 1 |
| specs/world/quests-act3-2.md | 1 | 0 | 0 | 1 |
| specs/world/quests-act3.md | 0 | 0 | 0 | 2 |
| specs/world/quests-act4.md | 0 | 0 | 0 | 5 |
| specs/world/quests-act5-2.md | 0 | 0 | 0 | 5 |
| specs/world/quests-act5.md | 1 | 0 | 0 | 4 |
| specs/world/quests-helpers.md | 8 | 0 | 0 | 3 |
| specs/world/quests-status.md | 4 | 53 | 0 | 12 |
| specs/world/quests.md | 19 | 0 | 0 | 5 |
| specs/world/vendors-2.md | 1 | 6 | 12 | 6 |
| specs/world/vendors.md | 2 | 0 | 0 | 4 |
| specs/world/waypoints.md | 5 | 0 | 0 | 9 |
| specs/render/blend-modes.md | 0 | 0 | 0 | 2 |
| specs/render/capture.md | 0 | 0 | 0 | 2 |
| specs/render/composition.md | 0 | 0 | 0 | 4 |
| specs/render/draw-order-2.md | 4 | 0 | 6 | 2 |
| specs/render/draw-order.md | 3 | 0 | 0 | 3 |
| specs/render/lighting.md | 3 | 0 | 0 | 2 |
| specs/render/map-preview.md | 0 | 0 | 0 | 2 |
| specs/render/overlay.md | 0 | 23 | 4 | 9 |
| specs/render/sprite-placement.md | 1 | 0 | 0 | 1 |
| specs/render/unit-composite.md | 7 | 0 | 2 | 3 |
| specs/audio/environment.md | 4 | 5 | 0 | 5 |
| specs/audio/sound-table-2.md | 7 | 5 | 0 | 23 |
| specs/audio/sound-table.md | 22 | 0 | 0 | 4 |
| specs/audio/triggers-2.md | 15 | 22 | 21 | 6 |
| specs/audio/triggers.md | 10 | 6 | 0 | 5 |
| specs/drlg/levels.md | 12 | 0 | 0 | 9 |
| specs/drlg/maze.md | 0 | 0 | 0 | 1 |
| specs/drlg/outdoor-act3-act5.md | 2 | 0 | 0 | 3 |
| specs/drlg/outdoor-tilesub.md | 0 | 0 | 0 | 2 |
| specs/drlg/outdoor.md | 4 | 0 | 0 | 0 |
| specs/drlg/preset.md | 7 | 0 | 1 | 6 |
| specs/drlg/rooms.md | 6 | 2 | 1 | 8 |
| specs/drlg/wall-remap.md | 0 | 0 | 0 | 3 |
| specs/tools/original-hooks-spawn.md | 0 | 0 | 0 | 30 |
| specs/tools/original-hooks.md | 0 | 0 | 0 | 47 |
| specs/tools/scenario.md | 14 | 0 | 0 | 8 |
| **total (3568)** | **1998** | **505** | **360** | **705** |

## Suggested batching

A+B sessions (Sonnet, ~300-500 rules, shared crate module):
1. skills/bodies + bodies-2 (d2-sim skills, ~390 A)
2. skills/bodies-2b + bodies-4 (~345 A)
3. skills/bodies-3 + levels + use (~275 A)
4. missiles bodies/bodies-2/missiles + monsters ai*, ai-bodies*, init, population, umod-* (~280 A; d2-sim missiles, monsters)
5. combat/* + sim/* + data/* + items/generation, affixes, quality (A parts) (~250)
6. world/object-population + objects + objects-2 + cube (~110 A; d2-sim world mech/misc/chests/populate.rs)
7. world/quests*, hirelings*, vendors*, waypoints, npc + drlg/* + render/* (~150 A, few B)
8. audio/* + client/bridge, msg-*, assets, render-pipeline + tools/scenario (~190 A/B)
9. ui/* B rules (automap, control-panel, controls, inventory, menus, messages, panels*, text; ~340 B, d2-client ui) + world/quests-status (53 B) + render/overlay B (23)

X marking: tools/original-hooks*, formats/d2s-legacy, items/bitstream-legacy (optional, becomes C only if old saves must load: HANDOFF IT-1/REC-44), edge-cases-original-bugs sections, provenance prose: mark non-claimable in spec.

C list (Opus):

From out1:
- Inactive-unit store (compress on room deactivation 0x005433F0, serialised records 0x00542E10/30, restore 0x00542B40): TickHooks::compress_unit / restore_inactive_units are empty defaults; needs per-room inactive store + unit serialisation + room lifecycle + one room-seed step. d2-sim tick, units, wiring/worldgen. sim/units.md §3.3 (10 rules), §3.4 (5).
- Corpse take-back 0x00562F30 + corpse slot fit 0x0055F2D0: Pending::corpse_take_back is a no-op default; implement over InvDesk (wiring/inventory) and call from wiring/action/death.rs:corpse_pickup. items/inventory-moves.md §8.5, §12.2, §12.3 (24 rules).
- Production InvRest/MoveRest implementor (only test stand-ins exist; MovePending defaults are no-ops): item-skill link 0x0055C110, inventory pass 0x0055DBC0, weapon bookkeeping 0x0055C5C0, stat refresh, plus set-item state update 0x00663CC0/0x00663A20/0x00663B40 (needs stats/lists park/unpark, sets tables, owner lists; TODO in items/props.rs). d2-sim wiring/inventory + d2-server adapters/handlers/items/moves.rs. items/inventory.md §5.5, §5.7, §5.8 (22), items/properties.md §9 r3, §13 (8).
- Quest drop helper 0x00559A30 + class sub-pickers 0x00555E70/0x00555FB0/0x005560F0: only seams (QuestItemsHost::drop_item_at, WorldPending::umod_quest_drop default no-op). Implement in d2-sim treasure/ (or wiring/economy) and wire both seams. items/treasure.md §9, §9.1 (10 rules).
- Save appearance fill (d2s +0x88..+0xA7: 0x0063E510, helm/hand 0x0063DA70, body armour, colour byte 0x0062C100): needs equipped-item + state-colour provider into the d2-server character writer (d2-formats d2s.rs has only the 0xFF prefill / stub), item tables, and the colour rules shared with d2-client rules/shading.rs. formats/d2s-appearance.md §3-§6 + edge (17), formats/d2s.md §2.8 r3-r4 (2). (Token table/lookup §1-§2 are B: pure fns over weapons/armor/misc rows.)
- Client skill description lines: new d2-client::ui::skill_desc module (spec's crate) + a client-side stat/inventory/weapon provider (temporary weapon stat-list toggles §2.10). skills/descriptions.md all 78 rules.
- Client umod burst/ring missiles (0x004AD6E0, 0x004C70D0) and the client umod hook bodies: client missile-creation seam in d2-client (rules/umod_hooks.rs has only the 43x5 table). monsters/umod-callbacks.md §28.3 r1-r2 (2).
- Automap seed {0,666} + cell picker 0x0061FFF0: d2-client automap (ui/automap spec). sim/rng.md §5.5 row1 (1).
- Optional / out of scope (bucketed X): legacy save + pre-1.10 item reader (0x47-0x5F; d2-formats d2s Legacy error, d2-sim items/bitstream version gates) and format-0 item routines: formats/d2s-legacy.md (68), items/bitstream-legacy.md (88), items/affixes.md §12, items/generation.md §11, items/quality.md §10, items/properties.md §14. Becomes a C item (d2-formats + d2-sim bitstream reader take a save version) only if old saves must load (HANDOFF IT-1/REC-44).

From out2:
- Client automap module + feeds (new d2-client/src/automap, ClientWorld rooms/units, frame hook, DRLG automap callbacks drlg/preset §3.2 r4 and drlg/rooms §5 r8, .map/.ma* file I/O, party 0x90 handler, options/UI state 0x0A): ui/automap §4, §5, §7, §8 r1-r2, §10 r1/r4, §12, §14.
- Client world-click pipeline (action kinds, held repeat, skill codes, target re-pick, belt/gate reads) from ClientWorld to bridge::intent senders: ui/controls §6, §7 r1-r4.
- Client mode machines + ClientFn/client-object update (player/object/item/monster mode requests, ClientFn 1-18, C objects walk, tick clock seam [GetTickCount choice], client RNG, set_mode/sound/overlay seams): client/model §8 r2-r6, world/objects-client §25-§28.
- Client unit audio feed into audio::driver (ClientUnit sound fields, frame events, floor material, quest state, weather, NPC interaction state, per-unit input order): client/model §18, audio/triggers-2 §21.
- Server sound-event queue (0x00553380) per unit with flush in the per-client update, S->C 0x2C, plus the client 0x2C handler (blocked rows in bridge-dispatch.tsv): audio/triggers-2 §14, world/cube §8 l2 r3, world/objects §14 r2 (0x00571740).
- Visibility predicate provider (COF/cel box test from rules/unit_composite + composite into bridge ModelInputs): client/model §13 r1/r3-r5.
- Overlay list on ClientUnit with light seam (0x00474160), graphics-record check, wall-clock seam, per-update walk call site, create call sites: render/overlay §2 r9-r10, §3 r1, §5; render/unit-composite §5 r3-r4.
- Client stat-list extension (item equip/unequip stat lists on owner, level hook, passive skill state lists) needing d2-sim stat-list + item property + skill/state tables in the client model: client/stat-lists §2, §4.
- Collision line between two units (0x00622AA0) seam provider (d2-sim path core line test 0x0064E260), used by AI, missiles and audio::driver: render/draw-order-2 §15.1.
- Object stand-drop provider (ObjectWorld::stand_drop empty default at d2-sim world/objects/mech.rs:77): armor/weapon pick, class picks, random class, floor search, item request in wiring/economy: world/objects-2 §20.1, §20.2, §20.5, §20.6.
- C->S 0x4F button handlers for stash close/withdraw/deposit (+ S->C 0x77 results, clamped gold add 0x0053FF00): d2-server handlers/items.rs + d2-sim stash gold seam: world/vendors-2 §10.1, §10.2, §10.4.
- Hireling items in the d2s save writer (0x005699A0): d2-formats d2s writer + d2-server character storage: world/hirelings §10 r8.
- Client game start sends C->S 0x67 and the app calls set_session (SessionFlow): app wiring in d2-client app/play + d2-server: client/model §7 r9.

# Per-spec detail

## specs/combat/damage.md  A=19 B=0 C=0 X=0
- §1 → A, d2-sim/src/combat/damage.rs (DamageRecord, apply_melee, apply)
- §2 text, r1 → A, d2-sim/src/combat/damage.rs:start_combat / melee pipeline
- §3 text → A, d2-sim/src/combat/damage.rs:start_combat / melee pipeline
- §4.5 text → A, d2-sim/src/combat/damage.rs:resist_value
- §5.1 r2 → A, d2-sim/src/combat/damage.rs (DamageRecord, apply_melee, apply)
- §5.2 text → A, d2-sim/src/combat/damage.rs (DamageRecord, apply_melee, apply)
- §5.3 r7 → A, d2-sim/src/combat/damage.rs:leech
- §5.4 → A, d2-sim/src/combat/events.rs:call (event table)
- §7.1 text, r1-6 → A, d2-sim/src/combat/mod.rs:CombatWorld::reaction, wiring/action/reaction.rs
- §8 → A, d2-sim/src/combat/events.rs:call (event table)
- §edge-cases-original-bugs r7, r11 → A, d2-sim/src/combat/damage.rs (DamageRecord, apply_melee, apply)
## specs/combat/events.md  A=18 B=0 C=0 X=0
- §2 text → A, d2-sim/src/combat/events.rs (event function table)
- §2.1 → A, events.rs:chilling_armor
- §2.2 → A, events.rs:frozen_armor
- §2.3 → A, events.rs:shiver_armor
- §2.4 → A, events.rs:iron_maiden
- §2.5 → A, events.rs:life_tap
- §2.8 → A, events.rs:howl
- §2.14 → A, events.rs:skill_on
- §2.15 → A, events.rs:skill_on_get_hit
- §2.19 → A, events.rs:blood_golem_taken
- §2.20 → A, events.rs:after_kill / rest-in-peace state 172
- §3 text, r2, r5-6 → A, d2-sim/src/combat/events.rs:cast / cast_point / core (item cast)
- §edge-cases-original-bugs r1, r4, r7 → A, d2-sim/src/combat/events.rs (event function table)
## specs/combat/hit.md  A=16 B=0 C=0 X=1
- §4 text, r6 → A, d2-sim/src/combat/hit.rs:melee_result
- §6.2 text → A, d2-sim/src/combat/hit.rs:dodge
- §7.1 text, r1-5 → A, combat/mod.rs hostility seam; monsters/ai/common.rs
- §7.2 r1-3 → A, combat/hit.rs in_melee_range / melee_range
- §7.3 r1-4 → A, combat/hit.rs in_melee_range / melee_range
- §7.4 → X, prose list of callers of hit_test / block_or_dodge
## specs/combat/vitals.md  A=6 B=0 C=0 X=0
- §4.4 text → A, d2-sim/src/combat/vitals/experience.rs (kill distribution)
- §4.8 text, r1-2 → A, d2-sim/src/wiring/action/death.rs (player death/dead start), combat/vitals/mod.rs
- §5 text → A, d2-sim/src/combat/vitals/sync.rs, wiring/action/vitals_sync.rs
- §5.1 r4 → A, d2-sim/src/combat/vitals/sync.rs, wiring/action/vitals_sync.rs
## specs/data/calc-expressions.md  A=3 B=0 C=0 X=0
- §3.1 text → A, d2-data/src/calc.rs, calc/ (compiler); d2-sim/src/skills/calc.rs (evaluator)
- §d2rs-policy-proposed-not-yet-logged-in-docs-plan-md-1-2-4-6-implemented r3, r5 → A, d2-data/src/calc.rs, calc/ (compiler); d2-sim/src/skills/calc.rs (evaluator)
## specs/data/fixups.md  A=2 B=0 C=0 X=0
- §2 text → A, d2-data/src/fixup/records.rs
- §11 text → A, d2-data/src/fixup/records.rs
## specs/data/loading.md  A=7 B=0 C=0 X=6
- §rules text → X, section heading text
- §1 r3 → A, d2-data/src/{compile_set,links,fixup}.rs; d2-server/src/world_data/
- §3.2 text, r2-3 → X, -txt mode not reproduced by d2rs
- §7.3 → A, d2-data/src/{compile_set,links,fixup}.rs; d2-server/src/world_data/
- §7.4 → A, d2-data/src/{compile_set,links,fixup}.rs; d2-server/src/world_data/
- §9 → A, d2-data/src/{compile_set,links,fixup}.rs; d2-server/src/world_data/
- §10 r6-7 → A, d2-data/src/{compile_set,links,fixup}.rs; d2-server/src/world_data/
- §d2-data-policy text → A, d2-data/src/{compile_set,links,fixup}.rs; d2-server/src/world_data/
- §d2-data-policy r5-6 → X, statement of what is not reproduced / survey scope
## specs/data/patch-layers.md  A=3 B=0 C=0 X=0
- §1 → A, d2-data/src/patch.rs, patch/{syntax,apply,check}.rs
- §11 → A, d2-data/src/patch.rs, patch/{syntax,apply,check}.rs
- §edge-cases-original-bugs r1 → A, d2-data/src/patch.rs, patch/{syntax,apply,check}.rs
## specs/data/schema.md  A=0 B=0 C=0 X=3
- §3 r1-3 → X, Game.exe table extraction procedure (RE tooling; output is fields.tsv/tables.tsv checked by codegen)
## specs/data/txt-format.md  A=0 B=0 C=0 X=1
- §1 → X, prose: where the reader is used
## specs/formats/animdata.md  A=3 B=0 C=0 X=1
- §1 → A, d2-formats/src/animdata.rs:AnimData::info; d2-sim wiring/path/place.rs (expfield)
- §5 → A, d2-formats/src/animdata.rs:AnimData::info; d2-sim wiring/path/place.rs (expfield)
- §7 → X, debug speed setter: not implemented by design
- §expfield-d2 → A, d2-formats/src/animdata.rs:AnimData::info; d2-sim wiring/path/place.rs (expfield)
## specs/formats/d2s-appearance.md  A=0 B=4 C=17 X=1
- §rules text → X, section heading text
- §1 r1-3 → B, d2-server/src/adapters/character.rs: token table (0x0063D710) + lookup (0x0063D900) as pure fns over weapons/armor/misc rows; today only STUB_COMPONENTS / 0xFF prefill in d2-formats d2s.rs
- §2 r1 → B, d2-server/src/adapters/character.rs: token table (0x0063D710) + lookup (0x0063D900) as pure fns over weapons/armor/misc rows; today only STUB_COMPONENTS / 0xFF prefill in d2-formats d2s.rs
- §3 r1 → C, appearance fill (0x0063E510): needs equipped-item + state provider into the save writer (d2-server character adapter), item tables, colour-byte rules shared with d2-client rules/shading.rs
- §4 r1-5 → C, appearance fill (0x0063E510): needs equipped-item + state provider into the save writer (d2-server character adapter), item tables, colour-byte rules shared with d2-client rules/shading.rs
- §5 r1-2 → C, appearance fill (0x0063E510): needs equipped-item + state provider into the save writer (d2-server character adapter), item tables, colour-byte rules shared with d2-client rules/shading.rs
- §6 text, r1-3 → C, appearance fill (0x0063E510): needs equipped-item + state provider into the save writer (d2-server character adapter), item tables, colour-byte rules shared with d2-client rules/shading.rs
- §edge-cases-original-bugs r1-5 → C, appearance fill (0x0063E510): needs equipped-item + state provider into the save writer (d2-server character adapter), item tables, colour-byte rules shared with d2-client rules/shading.rs
## specs/formats/d2s-legacy.md  A=0 B=0 C=0 X=68
- §rules text → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §1 r1-2 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §2 row1-18, r1-9 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §3 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §4 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §5 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §6 r1-3 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §7 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §8 text, r1-3 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §9 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §10 r1-6, row1-13 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §11 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §12 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
- §edge-cases-original-bugs r1-5 → X, legacy loader 0x47-0x5B: saves < 0x5C not loaded (D2sError::Legacy); out of scope until old saves are needed
## specs/formats/d2s-load.md  A=16 B=0 C=0 X=1
- §rules text → X, section heading text
- §3 r1-2 → A, d2-server/src/adapters/character.rs, adapters/session.rs; d2-client bridge/msg/session.rs (result codes)
- §4 r1-2 → A, d2-server/src/adapters/character.rs, adapters/session.rs; d2-client bridge/msg/session.rs (result codes)
- §5 r1-3 → A, d2-server/src/adapters/character.rs, adapters/session.rs; d2-client bridge/msg/session.rs (result codes)
- §6 r1 → A, d2-server/src/adapters/character.rs, adapters/session.rs; d2-client bridge/msg/session.rs (result codes)
- §7 r1 → A, d2-server/src/adapters/character.rs, adapters/session.rs; d2-client bridge/msg/session.rs (result codes)
- §8 text, r1-4 → A, d2-server/src/adapters/character.rs, adapters/session.rs; d2-client bridge/msg/session.rs (result codes)
- §edge-cases-original-bugs r2-3 → A, d2-server/src/adapters/character.rs, adapters/session.rs; d2-client bridge/msg/session.rs (result codes)
## specs/formats/d2s.md  A=38 B=0 C=2 X=4
- §2.4 r3, r5, r7 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §2.5 r1, r3 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §2.7 r1-3 → X, client-side header users (no server rule)
- §2.8 r3-4 → C, appearance bytes rebuilt on every save: depends on the unimplemented appearance fill (formats/d2s-appearance.md)
- §4 r4 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §6 r5-6 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §7.1 r8 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §7.2 r5 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §8.1 r3-4, r6-10 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §8.2 r1, r3, r5 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §8.3 r1, r5-7 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §8.4 r3-4 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §8.5 r4-5 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §10 r3 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §edge-cases-original-bugs r3-4, r6, r9, r11-15 → A, d2-formats/src/d2s.rs; d2-server/src/adapters/character.rs; wiring/inventory/save_index.rs
- §edge-cases-original-bugs r16 → X, reads uninitialised stack byte past the file
## specs/formats/tbl.md  A=0 B=0 C=0 X=1
- §live-tables-1-14d → X, observation table of live string-table archives
## specs/formats/wav.md  A=2 B=0 C=0 X=1
- §rules text → X, section heading text
- §2 text → A, d2-formats/src/wav.rs
- §5 → A, d2-client/src/audio/pool.rs (decoder hook)
## specs/items/affixes.md  A=3 B=0 C=0 X=16
- §1 r4 → A, d2-sim/src/items/affixes.rs (roll, wrappers, crafted)
- §3.1 → A, d2-sim/src/items/affixes.rs (roll, wrappers, crafted)
- §12 text → X, format-0 (version-0x47 save) routines: legacy saves out of scope (TODO in items/affixes.rs)
- §12.1 text, r1-7 → X, format-0 (version-0x47 save) routines: legacy saves out of scope (TODO in items/affixes.rs)
- §12.2 → X, format-0 (version-0x47 save) routines: legacy saves out of scope (TODO in items/affixes.rs)
- §12.3 text, r1-4 → X, format-0 (version-0x47 save) routines: legacy saves out of scope (TODO in items/affixes.rs)
- §12.4 → X, format-0 (version-0x47 save) routines: legacy saves out of scope (TODO in items/affixes.rs)
- §edge-cases-original-bugs r7 → A, d2-sim/src/items/affixes.rs (roll, wrappers, crafted)
## specs/items/bitstream-legacy.md  A=0 B=0 C=0 X=88
- §rules text → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §1 r1-4 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §2 r1-2, row1-20, r3, t2 row1-5 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §3 text, r1-12 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §4 r1-5, row1-6, r6-8 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §5 r1-6 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §6 text, r1-5 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §7 r1-3 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §8 r1-3 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
- §edge-cases-original-bugs r1-10 → X, item reader by save version 0x47-0x60: pre-1.10 saves not loaded (D2sError::Legacy; HANDOFF IT-1/REC-44 defer until old saves are in scope)
## specs/items/bitstream.md  A=1 B=0 C=0 X=1
- §edge-cases-original-bugs r9 → A, d2-sim/src/wiring/economy/item_records.rs (alt-code gamble list record)
- §edge-cases-original-bugs r10 → X, peek nec->neg for save version < 0x5D: legacy-version reader
## specs/items/generation.md  A=22 B=0 C=0 X=12
- §3 r9 → A, d2-sim/src/items/create.rs:create_item pipeline
- §9 r2 → A, d2-sim/src/items/create.rs:create_item pipeline
- §10.1 → A, wiring/interaction/vendor_world.rs, wiring/economy/quest_items.rs (start items, create from index)
- §10.2 → A, wiring/interaction/vendor_world.rs, wiring/economy/quest_items.rs (start items, create from index)
- §10.3 text, r1-2 → A, wiring/interaction/vendor_world.rs, wiring/economy/quest_items.rs (start items, create from index)
- §10.4 → A, wiring/interaction/vendor_world.rs, wiring/economy/quest_items.rs (start items, create from index)
- §11 text → X, format-0 branches (version-0x47 saves only)
- §11.1 text, r1-6 → X, format-0 branches (version-0x47 saves only)
- §11.2 text, r1-3 → X, format-0 branches (version-0x47 saves only)
- §12 text → A, d2-sim/src/wiring/economy/cube_items.rs (repair, recharge, runeword removal)
- §12.1 text, r1-5 → A, d2-sim/src/wiring/economy/cube_items.rs (repair, recharge, runeword removal)
- §12.2 text, r1-5 → A, d2-sim/src/wiring/economy/cube_items.rs (repair, recharge, runeword removal)
- §12.3 → A, d2-sim/src/wiring/economy/cube_items.rs (repair, recharge, runeword removal)
## specs/items/inventory-moves.md  A=10 B=0 C=24 X=0
- §7.10 r4-6 → A, d2-sim/src/items/moves/handlers.rs (swap, socket), wiring/action/death.rs:corpse_pickup
- §7.19 r4 → A, d2-sim/src/items/moves/handlers.rs (swap, socket), wiring/action/death.rs:corpse_pickup
- §8.5 text, r1-5 → C, corpse take-back 0x00562F30 / corpse slot fit 0x0055F2D0: Pending::corpse_take_back is an empty default; implement in d2-sim wiring/inventory (InvDesk) and bridge from wiring/action/death.rs
- §12 text → A, d2-sim/src/items/moves/handlers.rs (swap, socket), wiring/action/death.rs:corpse_pickup
- §12.1 r1-5 → A, d2-sim/src/items/moves/handlers.rs (swap, socket), wiring/action/death.rs:corpse_pickup
- §12.2 text, r1-8, l2 r1-3 → C, corpse take-back 0x00562F30 / corpse slot fit 0x0055F2D0: Pending::corpse_take_back is an empty default; implement in d2-sim wiring/inventory (InvDesk) and bridge from wiring/action/death.rs
- §12.3 text, r1-5 → C, corpse take-back 0x00562F30 / corpse slot fit 0x0055F2D0: Pending::corpse_take_back is an empty default; implement in d2-sim wiring/inventory (InvDesk) and bridge from wiring/action/death.rs
## specs/items/inventory.md  A=11 B=0 C=22 X=0
- §2.4 text → A, d2-sim/src/wiring/inventory/{mod,ops}.rs, items/moves/ground.rs (placement, equip)
- §4.9 text, r1-3, l2 r1 → A, d2-sim/src/wiring/inventory/{mod,ops}.rs, items/moves/ground.rs (placement, equip)
- §5.5 text, r1-5 → C, item-skill link 0x0055C110, inventory pass 0x0055DBC0, weapon bookkeeping 0x0055C5C0: MovePending/InvRest defaults are no-ops and there is no production InvRest implementor (only test stand-ins); needs d2-sim wiring on real StatLists/skills
- §5.7 text, r1-8 → C, item-skill link 0x0055C110, inventory pass 0x0055DBC0, weapon bookkeeping 0x0055C5C0: MovePending/InvRest defaults are no-ops and there is no production InvRest implementor (only test stand-ins); needs d2-sim wiring on real StatLists/skills
- §5.8 text, r1-6 → C, item-skill link 0x0055C110, inventory pass 0x0055DBC0, weapon bookkeeping 0x0055C5C0: MovePending/InvRest defaults are no-ops and there is no production InvRest implementor (only test stand-ins); needs d2-sim wiring on real StatLists/skills
- §edge-cases-original-bugs r10-14 → A, d2-sim/src/wiring/inventory/{mod,ops}.rs, items/moves/ground.rs (placement, equip)
## specs/items/properties.md  A=0 B=0 C=8 X=1
- §9 r3 → C, set-item state update 0x00663CC0 (+0x00663A20/0x00663B40): TODO in items/props.rs:~595; needs stats/lists park/unpark (stat-lists §8.5), set tables and owner lists
- §13 text, r1-6 → C, set-item state update 0x00663CC0 (+0x00663A20/0x00663B40): TODO in items/props.rs:~595; needs stats/lists park/unpark (stat-lists §8.5), set tables and owner lists
- §14 → X, format-0 property functions (version-0x47 saves only)
## specs/items/quality.md  A=1 B=0 C=0 X=18
- §2 → X, format-0 branches / cross-reference to treasure.md / unreachable overflow (1.14d has 8 rows)
- §8 r1 → X, format-0 branches / cross-reference to treasure.md / unreachable overflow (1.14d has 8 rows)
- §10 text → X, format-0 branches / cross-reference to treasure.md / unreachable overflow (1.14d has 8 rows)
- §10.1 → X, format-0 branches / cross-reference to treasure.md / unreachable overflow (1.14d has 8 rows)
- §10.2 r1-5 → X, format-0 branches / cross-reference to treasure.md / unreachable overflow (1.14d has 8 rows)
- §10.3 r1-3 → X, format-0 branches / cross-reference to treasure.md / unreachable overflow (1.14d has 8 rows)
- §10.4 r1-5 → X, format-0 branches / cross-reference to treasure.md / unreachable overflow (1.14d has 8 rows)
- §edge-cases-original-bugs r5 → X, format-0 branches / cross-reference to treasure.md / unreachable overflow (1.14d has 8 rows)
- §edge-cases-original-bugs r7 → A, d2-sim/src/items/quality.rs (quest/magic >= 4 rule)
## specs/items/treasure.md  A=3 B=0 C=10 X=0
- §6 text, r4 → A, d2-sim/src/treasure/quality.rs (drop quality)
- §9 text, r1-5 → C, quest drop helper 0x00559A30 and class sub-pickers: only seams (QuestItemsHost::drop_item_at, WorldPending::umod_quest_drop default no-op); implement in d2-sim treasure/ or wiring/economy and wire both seams
- §9.1 text, r1-3 → C, quest drop helper 0x00559A30 and class sub-pickers: only seams (QuestItemsHost::drop_item_at, WorldPending::umod_quest_drop default no-op); implement in d2-sim treasure/ or wiring/economy and wire both seams
- §edge-cases-original-bugs r9 → A, d2-sim/src/treasure/quality.rs (drop quality)
## specs/missiles/bodies-2.md  A=43 B=0 C=0 X=0
- §31 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §32 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §33 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §34 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §35 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §36 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §37 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §38 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §39 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §40 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §41 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §42 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §43 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §44 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §44 l3 r1-4, l4 r1-5 → A, d2-sim/src/missiles/bodies_ext2.rs:unit_find / default_filter
- §45 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §46 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §47 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §48 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §49 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §50 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §51 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §52 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §53 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §54 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §55 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §56 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §57 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §58 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §59 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §61 text → A, d2-sim/src/missiles/bodies_ext2.rs:srv_do_N / srv_hit_N (one fn per section)
- §edge-cases-original-bugs r2, r5, r9, r11 → A, d2-sim/src/missiles/bodies_ext2.rs (quirk inside the bodies)
## specs/missiles/bodies.md  A=34 B=0 C=0 X=0
- §1 text, r1 → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §2 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §3 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §4 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §5 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §6 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §7 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §8 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §9 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §10 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §11 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §12 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §13 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §14 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §15 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §16 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §17 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §18 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §19 text, l2 r4 → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §20 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §21 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §22 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §23 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §24 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §25 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §26 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §27 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §28 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §29 text → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_N / srv_hit_N (one fn per section; table catalogue.rs)
- §edge-cases-original-bugs r1, r3, r6 → A, d2-sim/src/missiles/bodies_ext.rs:srv_do_6/28/34 (the quirks live in the bodies)
## specs/missiles/missiles.md  A=6 B=0 C=0 X=1
- §r2-2-entry-points → A, missiles/create.rs; wiring/action/switch.rs (0x31 missile add message)
- §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r9 → A, d2-sim/src/missiles/create.rs:create_missile
- §r2-4-client-message → A, missiles/create.rs; wiring/action/switch.rs (0x31 missile add message)
- §r9-3-seeded-sub-missile-helper-0x005a9820-d2moo-missmode-createmissilewithcollisioncheck-1-14d-confirmed text → A, d2-sim/src/missiles/catalogue.rs:srv_do_8/10/17/25 + seeded sub-missile helper
- §r9-5-server-do-bodies-1-14d-confirmed text → A, d2-sim/src/missiles/catalogue.rs:srv_do_8/10/17/25 + seeded sub-missile helper
- §r11-missiles-txt-columns-and-their-server-use → X, reference table of missiles.txt columns (table-match tests cover it)
- §edge-cases-original-bugs r10 → A, d2-sim/src/missiles/
## specs/monsters/ai-bodies-2.md  A=0 B=0 C=0 X=1
- §1 → X, scope/order index table of the AI bodies in this spec
## specs/monsters/ai-bodies-3.md  A=1 B=0 C=0 X=1
- §1 → X, scope/order index table of the AI bodies in this spec
- §edge-cases-original-bugs r6 → A, d2-sim/src/monsters/ai/bodies3.rs (quirk inside the body)
## specs/monsters/ai-bodies-4.md  A=1 B=0 C=0 X=1
- §1 → X, scope/order index table of the AI bodies in this spec
- §edge-cases-original-bugs r4 → A, d2-sim/src/monsters/ai/bodies4.rs (quirk inside the body)
## specs/monsters/ai-bodies-5.md  A=0 B=0 C=0 X=1
- §1 → X, scope/order index table of the AI bodies in this spec
## specs/monsters/ai-bodies-6.md  A=47 B=0 C=0 X=1
- §1 → X, scope/order index table of the AI bodies in this spec
- §3 r4-9, l2 r1, l2 r4 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §5 r2-3 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §7 r7, r9-11, l2 r1-2, l2 r5 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §10 r5 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §11 r3 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §14 l2 r2-3 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §17 r2 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §20 r2-3, r5-7 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §24 r4 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §24.1 r5-8 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §24.2 r5-10 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
- §25 r2-5, r7-11 → A, d2-sim/src/monsters/ai/bodies6.rs (+ seams.rs, common.rs)
## specs/monsters/ai-bodies-7.md  A=34 B=0 C=0 X=1
- §1 → X, scope/order index table of the AI bodies in this spec
- §7 r3 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
- §12 r6 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
- §17 r4 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
- §18 r3-4, l2 r2, l2 r4-8 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
- §19 r4-9 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
- §20 r2-5 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
- §21 r2, r5-8 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
- §23 r3 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
- §27 r3, r5, r7-11 → A, d2-sim/src/monsters/ai/bodies7.rs (+ seams.rs, common.rs)
## specs/monsters/ai-bodies.md  A=4 B=0 C=0 X=0
- §9 text → A, monsters/ai/functions.rs:quill_rat / andariel / dispatch table
- §9.7 text → A, monsters/ai/functions.rs:quill_rat / andariel / dispatch table
- §9.12 text → A, monsters/ai/functions.rs:quill_rat / andariel / dispatch table
- §9.31 r0 → A, monsters/ai/npc.rs:good_npc_ranged
## specs/monsters/ai.md  A=26 B=0 C=0 X=0
- §1.3 text → A, d2-sim/src/monsters/ai/{mod,target,tactics,seams,common}.rs
- §2.1 r4 → A, d2-sim/src/monsters/ai/{mod,target,tactics,seams,common}.rs
- §2.3 r0 → A, d2-sim/src/monsters/ai/{mod,target,tactics,seams,common}.rs
- §3.3 text, l2 r1-2 → A, d2-sim/src/monsters/ai/{mod,target,tactics,seams,common}.rs
- §5.1 text, r1-4 → A, d2-sim/src/monsters/ai/{mod,target,tactics,seams,common}.rs
- §5.3 → A, d2-sim/src/monsters/ai/{mod,target,tactics,seams,common}.rs
- §5.4 → A, d2-sim/src/monsters/ai/{mod,target,tactics,seams,common}.rs
- §7.4 text, r1-9 → A, d2-sim/src/monsters/ai/{mod,target,tactics,seams,common}.rs
- §edge-cases-original-bugs r13-14, r16 → A, d2-sim/src/monsters/ai/seams.rs (quirk inside the implementation)
## specs/monsters/init.md  A=16 B=0 C=0 X=2
- §1 → A, d2-sim/src/monsters/init/create.rs
- §4.1 text, r1-3 → A, d2-sim/src/monsters/init/create.rs
- §19.7 → X, index of bodies owned by other specs
- §25 text → X, tool-call intro prose
- §25.1 → A, d2-sim/src/monsters/population/spawn.rs, population/seams.rs
- §25.2 r1-4 → A, d2-sim/src/monsters/population/spawn.rs, population/seams.rs
- §25.3 r1-4 → A, d2-sim/src/monsters/population/spawn.rs, population/seams.rs
- §edge-cases-original-bugs r13-14 → A, monsters/init/seams.rs
## specs/monsters/population.md  A=8 B=0 C=0 X=0
- §1 r2-3 → A, d2-sim/src/monsters/population/{seams,preset,spawn}.rs; monsters/init/umods.rs
- §11.5 r6 → A, d2-sim/src/monsters/population/{seams,preset,spawn}.rs; monsters/init/umods.rs
- §14 r2-6 → A, d2-sim/src/monsters/population/{seams,preset,spawn}.rs; monsters/init/umods.rs
## specs/monsters/umod-callbacks.md  A=23 B=0 C=2 X=1
- §1 r1, r3, r5-6 → A, d2-sim/src/monsters/init/{find,callbacks,umods}.rs
- §2 text, r2, r5 → A, d2-sim/src/monsters/init/{find,callbacks,umods}.rs
- §3.1 text, r5, l3 r1-3 → A, d2-sim/src/monsters/init/{find,callbacks,umods}.rs
- §3.6 → X, table of monster data fields (data dependency list)
- §26 → A, d2-sim/src/monsters/init/{find,callbacks,umods}.rs
- §28 text → A, d2-client/src/rules/umod_hooks.rs (table, dispatcher)
- §28.1 r3-4 → A, d2-client/src/rules/umod_hooks.rs (table, dispatcher)
- §28.3 r1-2 → C, client-side burst/ring missile creation: no client umod hook bodies (d2-client rules/umod_hooks.rs has only the table)
- §28.4 → A, d2-client/src/rules/umod_hooks.rs (table, dispatcher)
- §edge-cases-original-bugs r1, r3, r6, r12-14 → A, monsters/init/callbacks.rs
## specs/monsters/umod-init-bodies.md  A=26 B=0 C=0 X=0
- §1 r1-4 → A, monsters/init/umods.rs
- §2 text, r1-11 → A, monsters/init/umods.rs:elemental
- §3 → A, monsters/init/umods.rs (per-umod value table in elemental())
- §4 r1-4 → A, monsters/init/umods.rs:teleport
- §edge-cases-original-bugs r1-5 → A, monsters/init/umods.rs
## specs/sim/intents-events.md  A=35 B=0 C=0 X=16
- §1 text → A, d2-sim/src/tick/mod.rs (loop order)
- §1 r5 → X, leave-game flush / worker path unused by single player
- §2.1 r8 → A, d2-proto c2s builders
- §2.2 text → A, d2-sim/src/wiring/action/dispatch.rs
- §2.3 r4 → X, host sync timer never read by the simulation
- §2.4 r6, r8 → A, d2-sim/src/wiring/action/dispatch.rs (handler results)
- §3.1 r2 → A, d2-proto generated size table
- §3.2 r3-4 → A, d2-server transport buffers (local mode flush)
- §3.2 r5-6 → X, network mode (game type 1/2, compression, length prefix) out of scope
- §3.4 r2-3 → A, d2-client/src/bridge/msg/mod.rs (receive walk)
- §6 r1-5 → X, definition of the comparison (frame unit, trace, masks): check method, not code
- §7 text → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §7.1 r2 → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §7.3 r1, r3-4 → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §7.4 r1 → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §7.5 text, r1-2, r5-6, r8 → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §7.6 r1-4 → X, worked example composing 7.1-7.5 (no rule of its own)
- §7.6 r5 → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §7.7 r1 → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §7.8 text, r1, r4 → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §7.9 r2, r4-5 → A, d2-sim/src/wiring/action/unit_update.rs, monsters/mode_message.rs, wiring/action/switch.rs
- §9 text, r15-16 → A, d2-server/src/adapters/handlers/player.rs (0x4F, 0x15 relay)
- §edge-cases-original-bugs r8, r10 → X, uninitialised memory / null deref / unreachable id: not reproducible
- §edge-cases-original-bugs r11 → A, d2-sim/src/drlg/active.rs (room switch sends 0x07 twice)
- §edge-cases-original-bugs r12 → X, uninitialised memory / null deref / unreachable id: not reproducible
- §edge-cases-original-bugs r13 → A, d2-proto s2c item message padding
## specs/sim/path-placement.md  A=6 B=0 C=0 X=1
- §10 text → A, d2-sim/src/path/place.rs, path/place_seams.rs, wiring/path/place.rs; d2-server adapters/session_flow.rs
- §12.2 text → A, d2-sim/src/path/place.rs, path/place_seams.rs, wiring/path/place.rs; d2-server adapters/session_flow.rs
- §13 text, r2, r4 → A, d2-sim/src/path/place.rs, path/place_seams.rs, wiring/path/place.rs; d2-server adapters/session_flow.rs
- §edge-cases-original-bugs text → X, heading text only
- §edge-cases-original-bugs r10 → A, d2-sim/src/path/place.rs, path/place_seams.rs, wiring/path/place.rs; d2-server adapters/session_flow.rs
## specs/sim/pathing.md  A=17 B=5 C=0 X=3
- §9.6 r2 → A, path/walk/step.rs
- §10 r4-5 → A, wiring/path/walk.rs
- §12 text → X, section intro prose
- §12.2 → A, d2-sim/src/path/walk/other.rs, find.rs
- §12.6 → A, d2-sim/src/path/walk/other.rs, find.rs
- §12.7 text, r2 → A, d2-sim/src/path/walk/other.rs, find.rs
- §12.8 text, r5 → A, d2-sim/src/path/walk/other.rs, find.rs
- §13 text → X, section intro prose
- §13.1 r1-3 → A, path record + PathOp (skills/use_/bodies/effects.rs), monsters/mode_message.rs
- §13.2 r1-3 → A, path record + PathOp (skills/use_/bodies/effects.rs), monsters/mode_message.rs
- §13.3 text, r1-4 → B, d2-sim/src/path/collision.rs: add `line_blocked(rooms, from, to, mask)` (0x0064E260) over find_room/point_value; only a seam (Pending::body_line_blocked, default blocked) exists
- §edge-cases-original-bugs text → X, heading text only
- §edge-cases-original-bugs r1-2 → A, path/walk/find.rs
## specs/sim/pets.md  A=1 B=0 C=0 X=0
- §10 → A, d2-sim/src/player/pets.rs, world/hirelings/pets.rs (creation, free, maximum resync)
## specs/sim/rng.md  A=25 B=0 C=1 X=11
- §3 r6 → X, calling-convention note for the roll helpers
- §5.4 text, row3-5 → A, d2-sim/src/drlg/level.rs, drlg/active.rs (seed derivation)
- §5.5 text → X, heading intro of client-only globals
- §5.5 row1 → C, automap seed {0,666} and cell picker: no automap in d2-client yet (ui/automap)
- §5.5 row2 → A, d2-client/src/rules/draw_order/background.rs (particle globals)
- §6 text, row1-8 → X, survey table of inlined-draw shapes (provenance)
- §7 text, row1-19 → A, per-system seed ownership: treasure, items, population, ai, combat, skills, objects, drlg modules draw from the listed seed
## specs/sim/stat-lists.md  A=6 B=0 C=0 X=0
- §4 r3-4 → A, d2-sim/src/stats/lists.rs (+ stats/states.rs)
- §6.4 text → A, d2-sim/src/stats/lists.rs (+ stats/states.rs)
- §8.9 → A, d2-sim/src/stats/lists.rs (+ stats/states.rs)
- §9.3 → A, d2-sim/src/stats/lists.rs (+ stats/states.rs)
- §edge-cases-original-bugs r5 → A, d2-sim/src/stats/lists.rs (+ stats/states.rs)
## specs/sim/stats.md  A=0 B=0 C=0 X=1
- §edge-cases-original-bugs r5 → X, original recursion bug on self-referencing op entries; 1.14d has no such data (not reproduced)
## specs/sim/tick.md  A=7 B=0 C=0 X=6
- §1 r6 → X, host driver load ratio, wall-clock
- §5.2 r4 → A, d2-sim/src/tick/{mod,timer,events}.rs
- §5.3 → A, d2-sim/src/tick/{mod,timer,events}.rs
- §5.5 text, r4 → A, d2-sim/src/tick/{mod,timer,events}.rs
- §5.7 → X, summary / wall-clock host-only prose
- §6 text → A, d2-sim/src/tick/{mod,timer,events}.rs
- §6 r1-2 → X, arena assert and wall-clock heartbeat (host-only)
- §6 r3 → A, d2-sim/src/tick/{mod,timer,events}.rs
- §7 → X, summary / wall-clock host-only prose
- §8 → X, summary / wall-clock host-only prose
- §edge-cases-original-bugs r3 → A, d2-sim/src/tick/{mod,timer,events}.rs
## specs/sim/units.md  A=9 B=11 C=15 X=2
- §3.1 r9 → A, d2-sim/src/units/{modes,anim,dispatch,hooks}.rs, tick/events.rs
- §3.3 text, r1-9 → C, inactive-record storage on room deactivation and restore (0x005433F0 / 0x00542E10-30 / 0x00542B40): TickHooks::compress_unit and restore_inactive_units are empty defaults; needs per-room inactive store + unit serialisation + room lifecycle + seed step
- §3.4 text, r1-4 → C, inactive-record storage on room deactivation and restore (0x005433F0 / 0x00542E10-30 / 0x00542B40): TickHooks::compress_unit and restore_inactive_units are empty defaults; needs per-room inactive store + unit serialisation + room lifecycle + seed step
- §4.5 → A, d2-sim/src/units/{modes,anim,dispatch,hooks}.rs, tick/events.rs
- §4.7 text, r1-10 → B, d2-sim/src/wiring/action/units.rs:anim_rate / frame_bonus (Pending defaults return 0): plain formula over mode row, stats 67-69/other_animrate, states
- §5 r4 → A, d2-sim/src/units/{modes,anim,dispatch,hooks}.rs, tick/events.rs
- §6.5 → A, d2-sim/src/units/{modes,anim,dispatch,hooks}.rs, tick/events.rs
- §6.6 text, r1-4 → A, d2-sim/src/units/{modes,anim,dispatch,hooks}.rs, tick/events.rs
- §8 → X, owned by render/draw-order-2.md §15.1/§16
- §edge-cases-original-bugs r4 → X, wall-clock read (GetTickCount) stored in the player movement step
## specs/skills/bodies-2.md  A=186 B=0 C=0 X=1
- §1 → X, conventions/notation
- §2.2 text, r1-2 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.5 text, r1-8 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.7 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.8 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.9 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.11 text, r0 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.12 text, r1-4 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.14 text, r1-3 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.17 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.18 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.20 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.21 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.22 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.24 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §2.26 text, r1-4 → A, d2-sim/src/skills/use_/bodies/helpers3.rs
- §3.1 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl01.rs
- §3.2 text, r1-6 → A, d2-sim/src/skills/use_/bodies/b3_lvl01.rs
- §3.6 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl01.rs
- §3.7 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl01.rs
- §3.8 → A, d2-sim/src/skills/use_/bodies/b3_lvl01.rs
- §3.9 text, r1-7 → A, d2-sim/src/skills/use_/bodies/b3_lvl01.rs
- §3.11 text, r1-6 → A, d2-sim/src/skills/use_/bodies/b3_lvl01.rs
- §4.1 text, r1-3, l2 r1-6 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.2 r1-3 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.3 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.4 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.6 r1-6 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.7 text, r1-3 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.8 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.9 text, r1-7 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.11 r1-8 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.12 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §4.13 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl06.rs
- §5.1 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl12.rs
- §5.4 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl12.rs
- §5.5 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl12.rs
- §5.6 → A, d2-sim/src/skills/use_/bodies/b3_lvl12.rs
- §5.8 text, r1-7, l2 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl12.rs
- §edge-cases-original-bugs r1-29 → A, d2-sim/src/skills/use_/bodies/*: quirk implemented inside the body
## specs/skills/bodies-2b.md  A=178 B=0 C=0 X=0
- §6.3 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.4 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.5 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.6 text, r1-5, l2 r1-7 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.9 r1-6 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.10 text, r1-2, l2 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.12 r1-2 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.13 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.14 r1-6 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.15 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.16 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.18 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §6.19 r1-3 → A, d2-sim/src/skills/use_/bodies/b3_lvl18.rs
- §7.3 r1-7 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.5 text, r0 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.6 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.7 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.8 text, r1-8 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.9 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.11 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.12 text, r1-6 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.13 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.14 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.15 r1-3 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.16 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.17 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.19 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §7.20 text, r1-3 → A, d2-sim/src/skills/use_/bodies/b3_lvl24.rs
- §8.1 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.2 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.3 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.4 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.6 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.7 r1-10 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.8 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.11 r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.12 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.13 r1-4 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
- §8.14 → A, d2-sim/src/skills/use_/bodies/b3_lvl30.rs
## specs/skills/bodies-3.md  A=245 B=0 C=0 X=2
- §1 → X, conventions/notation
- §3.1 text, r1-3 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §3.2 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §3.4 text, r1-6 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §3.5 text, r1-2 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §3.6 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §3.7 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §3.9 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §4.1 text, r1-3 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.2 text, r1-7 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.3 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.4 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.5 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.6 r1-6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.7 r1-8 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.8 text, r1-6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.9 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.10 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.11 text, r1-8 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.12 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §4.13 r1-6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5 text → X, section intro text
- §5.1 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.2 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.3 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.4 r1-5 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.5 r1-3 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.7 r1-6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.8 r1-3 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.9 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.10 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.11 r1-6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.12 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.13 r1-7 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.14 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.15 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.16 r1-3 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.17 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.18 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.19 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.20 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.21 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.22 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.23 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.24 text, r1-6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.25 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.26 r1-6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.27 r1-5 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.28 r1-6 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.29 r1-3 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.30 text, r1-7 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.31 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.32 r1-7 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §5.33 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs
- §edge-cases-original-bugs r1-14 → A, d2-sim/src/skills/use_/bodies/b4_mon.rs / b4_helpers.rs: quirk inside the body
## specs/skills/bodies-4.md  A=167 B=0 C=0 X=2
- §1 → X, conventions/notation
- §2.1 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §2.2 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §2.3 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §2.4 text, r1-3 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §2.6 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §2.7 → A, d2-sim/src/skills/use_/bodies/b4_helpers.rs
- §3 text → X, section intro text
- §3.1 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.2 r1-8 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.3 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.4 r1-3 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.5 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.6 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.7 r1-7 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.8 text, r1-2 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.9 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.10 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.11 r1-3 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.12 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.13 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.14 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.15 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.16 text, r1-3 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.17 text, r1-5 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.18 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.19 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.20 r1-5 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.21 text, r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.23 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.25 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §3.26 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.1 text, r1-3 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.2 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.3 r1-6 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.4 r1-6 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.5 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.6 r1-5 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.7 r1-4 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.8 text, r1-8 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.9 text, r4-9 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.10 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §4.11 r1-5 → A, d2-sim/src/skills/use_/bodies/b4_more.rs
- §edge-cases-original-bugs r1-10 → A, d2-sim/src/skills/use_/bodies/b4_more.rs: quirk inside the body
## specs/skills/bodies.md  A=200 B=0 C=0 X=2
- §1 → X, conventions/notation
- §2.1 text, r1-2 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.2 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.3 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.4 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.5 r4 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.6 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.7 text, r1-2 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.12 text, r1, r3 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.15 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.16 text, r1-3 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §2.17 text, r1-2 → A, d2-sim/src/skills/use_/bodies/helpers.rs / mod.rs
- §3.1 r1-4 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §3.3 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §3.4 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §3.5 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §3.6 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §3.7 r1-4 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §3.8 r1-8 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §3.9 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §3.10 → A, d2-sim/src/skills/use_/bodies/starts.rs
- §4.1 text, r1-8 → A, d2-sim/src/skills/use_/bodies/dos.rs
- §4.2 text, r1-5 → A, d2-sim/src/skills/use_/bodies/dos.rs
- §4.3 text, r6 → A, d2-sim/src/skills/use_/bodies/dos.rs
- §4.4 text, l2 r1-9 → A, d2-sim/src/skills/use_/bodies/dos.rs
- §5 text, r1-5 → A, d2-sim/src/skills/use_/mod.rs (srvmissile path)
- §6 text → X, section intro text
- §6.3 → A, d2-sim/src/skills/use_/bodies/helpers2.rs
- §6.5 text, r1-9 → A, d2-sim/src/skills/use_/bodies/helpers2.rs
- §6.6 → A, d2-sim/src/skills/use_/bodies/helpers2.rs
- §6.9 text, r1-9 → A, d2-sim/src/skills/use_/bodies/helpers2.rs
- §6.11 → A, d2-sim/src/skills/use_/bodies/helpers2.rs
- §6.19 text, r1-5 → A, d2-sim/src/skills/use_/bodies/helpers2.rs
- §7.2 text, r1-4 → A, d2-sim/src/skills/use_/bodies/starts2.rs
- §7.5 → A, d2-sim/src/skills/use_/bodies/starts2.rs
- §7.7 r1-5 → A, d2-sim/src/skills/use_/bodies/starts2.rs
- §8.1 text, r1-9 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §8.3 text, r1-4 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §8.4 text, r1-5 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §8.7 r1-9 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §8.9 r1-8 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §8.16 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §8.17 text, r1-4 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §8.18 r1-8 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §8.21 text, r1-12 → A, d2-sim/src/skills/use_/bodies/dos2.rs
- §edge-cases-original-bugs r1-6, r8-19 → A, d2-sim/src/skills/use_/bodies/*: quirk implemented inside the body
## specs/skills/descriptions.md  A=0 B=0 C=78 X=0
- §1 r1-5 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.1 text, r1-4 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.2 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.3 text, r1-6 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.4 text, r1-5 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.5 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.6 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.7 text, r1-3 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.8 text, r1-4 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.9 text, r1-2 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.10 text, r1-2 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §2.11 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §3 text, row1-24 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §4 row1-5 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
- §edge-cases-original-bugs r1-6 → C, new d2-client::ui::skill_desc module (spec crate/module): pure fns over client-side stats/items for the skill tooltip lines; needs a client stat/inventory provider; none exists (only skilldesc table decode in d2-sim skills/mod.rs)
## specs/skills/levels.md  A=29 B=0 C=0 X=0
- §3.1 text → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §3.3 text → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §6.4 text → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §6.5 text, r1-4 → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §7 text → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §7.1 text, r1-6 → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §7.2 → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §7.3 → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §7.4 → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §7.5 text, r1-3, l2 r1-2 → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §7.6 text, r1-2 → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
- §edge-cases-original-bugs r7 → A, d2-sim/src/skills/levels.rs, skills/special.rs, skills/mod.rs
## specs/skills/use.md  A=1 B=0 C=0 X=0
- §5.5 → A, d2-sim/src/skills/use_/mod.rs


## specs/ui/automap.md  A=0 B=34 C=22 X=14
- §1 (5) → B, new d2-client/src/automap/store.rs: cell pool, layers, AVL order, groups (self-contained data structure, no seam)
- §2:text (1) → X, heading
- §2 (3) → B, automap cell picker (store.rs; roll on automap seed via client RNG)
- §3 (4) → B, add-cell from floor records (store.rs, inputs passed in)
- §4:text (1) → X, heading
- §4 (4) → C, unit/object cells need client unit feed + monstats2/objects AutoMap + client position (ClientWorld -> automap)
- §5 (5) → C, frame hook, room add, whole-preset-level DRLG callback, countdown: client frame loop + DRLG automap callbacks (drlg/preset §3.2 r4, rooms §5 r8)
- §6 (1) → B, town art table (automap/town.rs)
- §7:text (1) → X, heading
- §7 (4) → C, .map/.ma0-.ma3 persistence: client save-dir file I/O + teardown hook
- §8:text (1) → X, heading
- §8:r1,r2 (2) → C, options read at UI init, UI state 0x0A open/close and keys: UI-state + options wiring
- §8:r3-r5 (3) → B, size/re-centre/cel file names (automap/view.rs)
- §9:text (1) → X, heading
- §9 (3) → B, view geometry (automap/view.rs)
- §10:r1,r4 (2) → C, UI pass step 3 draw call + CelDrawClipped via the d2-client renderer (needs ui draw seam)
- §10:r2,r3 (2) → B, clip-window tables and in-order walk with pruning (automap/draw.rs)
- §11:text (1) → X, heading
- §11 (7) → B, unit marker colour/shape/name rules (automap/markers.rs; palette nearest from ui draw)
- §12 (2) → C, S->C 0x90 PartyAutomapInfo handler + party roster draw (new bridge handler + automap)
- §13:text (1) → X, heading
- §13 (6) → B, header text lines (automap/header.rs using ui/text string table)
- §14 (3) → C, lifecycle hooks: UI init, act load 0x03 colours, teardown
- §edge-cases-original-bugs (7) → X, edge-case narration of §1-§7 rules
## specs/ui/control-panel.md  A=0 B=49 C=0 X=2
- §1:text (1) → X, heading
- §1 (5) → B, draw order of border/control panel in d2-client/src/ui/panels/border.rs (extend draw_border_and_ctrlpnl)
- §2 (1) → B, art file list (panels/border.rs)
- §3 (6) → B, life/mana globes, numbers, text toggles (panels/border.rs)
- §4 (2) → B, experience and stamina bars (panels/border.rs)
- §5 (12) → B, belt: type, popup, slots, hover, click (panels/border.rs; belt data from ClientWorld)
- §6 (4) → B, run/walk and menu buttons, tool tip (panels/border.rs)
- §7 (3) → B, skill buttons (panels/border.rs)
- §8 (5) → B, 800x600 / 640x480 button placement, press/release (panels/border.rs)
- §9 (8) → B, mini-panel/menu variants and buttons (panels/border.rs)
- §10 (3) → B, mouse down/up handlers (panels/border.rs, ui/original.rs before_event)
- §edge-cases-original-bugs (1) → X, edge narration
## specs/ui/controls.md  A=0 B=66 C=15 X=9
- §1 (4) → B, binding table 10-byte entries, lookups, command table (controls/original.rs new file in the controls module; names.rs reconcile)
- §2 (5) → B, .key / default.key file parse+write (controls/original.rs)
- §3:text (1) → X, heading
- §3:row1-row34 (34) → B, default key/command table rows as a constant checked row by row (controls/original.rs); handler effects need SetUIState wiring, see C list
- §3.1:r1,r2 (2) → B, skill hotkey open-bar/use (controls + ui/states)
- §3.1:r3 (1) → X, note: no default key
- §3.2 (1) → B, belt column use (controls + panels/border.rs belt)
- §3.3 (1) → B, key-config menu tables
- §3.4 (1) → B, key-commands.tsv
- §4.1 (6) → B, key down/up dispatch, key modes, Windows keys
- §4.2 (3) → B, key table scan, middle/wheel
- §4.3:r1,r2,r3 (3) → B, mouse button / stand-still / activate handling
- §4.3:r4 (1) → X, pointer to section 6
- §5 (3) → B, key assignment screen rules
- §6 (11) → C, world click -> action kind, filter, held repeat, skill codes, senders, re-pick: needs client world model reads + intent senders (bridge::intent) + target pick on ClientWorld
- §7:r1-r4 (4) → C, gates and belt use read UI/model/belt state and send intents
- §7:r5 (1) → B, pointer button meanings for d2rs input
- §b4-original-defaults-check-client-ui-md-b4:text (1) → X, heading
- §b4-original-defaults-check-client-ui-md-b4 (2) → B, unit test building the 1140-byte default.key and ignored game-file check (controls/tests.rs)
- §edge-cases-original-bugs (5) → X, edge narration
## specs/ui/inventory.md  A=1 B=43 C=0 X=6
- §1 (4) → B, grid layout records and mouse->cell (ui/panels/inventory.rs)
- §2 (3) → B, tint colours (panels/inventory.rs)
- §3 (4) → B, grid items draw (panels/inventory.rs:draw)
- §4 (3) → B, placement tint (panels/inventory.rs)
- §5 (3) → B, hover state (panels/inventory.rs)
- §6 (5) → B, equipment boxes (panels/inventory.rs)
- §7 (1) → X, not drawn here
- §8 (6) → B, item graphic draw (panels/inventory.rs)
- §9 (5) → B, item checks for tints (panels/inventory.rs)
- §10:text (1) → X, heading
- §10 (5) → B, grid click -> C->S message (panels/inventory.rs:press; intents via bridge::intent)
- §11:text (1) → X, heading
- §11 (5) → B, gold amount dialog (new widget in ui/widget.rs + panels)
- §b5-cellgrid-answers-client-ui-md-b5 (1) → A, crates/d2-client/src/ui/widget.rs:CellGrid::cell_at/cell_rect
- §edge-cases-original-bugs (3) → X, edge narration
## specs/ui/menus.md  A=4 B=21 C=0 X=1
- §1:r7,r8 (2) → A, crates/d2-client/src/ui/panels/waypoint.rs:self_close / close_hook
- §1 (6) → B, waypoint menu panel-local mouse, tabs, latched close (panels/waypoint.rs)
- §2:r2,r3 (2) → A, crates/d2-client/src/ui/panels/npc.rs:from_records/apply_builder/shown_options
- §2 (5) → B, menu object, layout, draw, anchor (panels/npc.rs)
- §3 (5) → B, hire list (panels/npc.rs)
- §4 (5) → B, shop transactions (panels/shop.rs)
- §edge-cases-original-bugs (1) → X, edge narration
## specs/ui/messages.md  A=0 B=49 C=0 X=3
- §1 (1) → B, entry points (new ui/messages.rs beside ui/msg_ui.rs)
- §2 (5) → B, screen message list (ui/messages.rs)
- §3:text (1) → X, heading
- §3 (3) → B, chat line formats (needs text.md section 14 formatter)
- §4 (2) → B, recipe scroll text
- §5 (6) → B, overhead text (needs unit screen position from world_view)
- §6 (5) → B, NPC text list (ui/msg_ui.rs:npc_text + talk box)
- §7 (8) → B, dialog panel
- §8 (2) → B, timed text box
- §9 (3) → B, hire offers and popup
- §10:text (1) → X, heading
- §10 (2) → B, other 0x50 UI effects
- §11 (6) → B, item-socket dialog (UI state 0x0E)
- §12 (1) → B, NPC alert
- §13 (4) → B, NPC intro table
- §14 (1) → B, interact NPC state
- §edge-cases-original-bugs (1) → X, edge narration
## specs/ui/panels-2.md  A=24 B=27 C=0 X=5
- §14:r1,r2,r3,r7,r8 (5) → A, crates/d2-client/src/ui/panels/npc.rs:load/from_records/reset_for_interaction/apply_builder/records
- §14:r4,r11 (2) → A, crates/d2-client/src/ui/panels/shop.rs:draw/button_cels
- §14:r5,r9 (2) → A, crates/d2-client/src/ui/panels/npc.rs:option_intent
- §14:r6 (1) → X, pointer to menu geometry elsewhere
- §14 (3) → B, shop tabs, mouse handlers, Cain count reset (panels/shop.rs, npc.rs)
- §17:r2 (1) → A, crates/d2-client/src/ui/panels/character.rs:add_stat_point/release
- §17 (9) → B, character panel details (panels/character.rs)
- §18:r1,r2 (2) → A, crates/d2-client/src/ui/panels/inventory.rs:release/close_rect
- §18:r4 (1) → X, statement, no behavior
- §18 (1) → B, belt test (panels/inventory.rs)
- §19:text (1) → X, heading
- §19:r1,r2,r6 (3) → A, crates/d2-client/src/ui/panels/skilltree.rs:mouse_down/mouse_up/draw
- §19 (3) → B, free points number, tab/close tool tips (panels/skilltree.rs)
- §20:r1,r2,r3,r4 (4) → A, crates/d2-client/src/ui/panels/stash_cube.rs:stash_close/cube_transmute/cube_close/horadric_pos
- §20 (2) → B, GoldMax font (panels/stash_cube.rs)
- §21 (9) → B, gold line, gold buttons, gold dialog (new widgets + panels/inventory.rs, stash_cube.rs)
- §22:r1,r2,r3 (3) → A, crates/d2-client/src/ui/widget.rs (Button frames, Label pen, ScrollList)
- §22:r4 (1) → A, crates/d2-client/src/world_view/ui_bind.rs (ui image request)
- §22:r5 (1) → A, crates/d2-client/src/world_view/feed.rs (open_mode)
- §22:r6 (1) → X, pointer to panels-3 section 23
- §edge-cases-original-bugs (1) → X, edge narration
## specs/ui/panels-3.md  A=4 B=31 C=0 X=8
- §23:r12,r13 (2) → X, dead copy / consequence narration
- §23 (12) → B, mouse cursor machine and draw (new ui/cursor.rs; ui/original.rs before_event feeds it)
- §24 (5) → B, character panel inputs (panels/character.rs)
- §25:r1,r4 (2) → A, crates/d2-client/src/ui/panels/skilltree.rs:icons/icon_remap
- §25 (2) → B, learnability, level number (panels/skilltree.rs)
- §26:r1,r2 (2) → A, crates/d2-client/src/ui/panels/waypoint.rs:open (row rebuild)
- §26:r3 (1) → X, pointer to panels.md
- §26 (1) → B, waypoint tab gates (panels/waypoint.rs; client quest records from bridge output)
- §27:r5,r6,r8 (3) → X, states never opened / anvil pointer / narration
- §27 (5) → B, scroll panel, recipe scroll, symbols (new ui/panels/scroll.rs)
- §28:text (1) → X, heading
- §28 (6) → B, gold dialog box, buttons, edit box, spinner (ui/widget.rs)
- §edge-cases-original-bugs (1) → X, edge narration
## specs/ui/panels.md  A=19 B=5 C=0 X=1
- §6:r3 (1) → A, crates/d2-client/src/ui/panels/border.rs:draw_border_and_ctrlpnl
- §7:r4 (1) → B, queued hover tool tips (ui/panels/mod.rs)
- §8:r7 (1) → A, crates/d2-client/src/ui/panels/character.rs:draw
- §8 (3) → B, damage/AR block, experience, popups (panels/character.rs)
- §9:r1,r6,r7,r8 (4) → A, crates/d2-client/src/ui/panels/inventory.rs:draw/press/release
- §10:r7 (1) → B, free-points box, tab tool tip (panels/skilltree.rs)
- §10:r9 (1) → A, crates/d2-client/src/ui/panels/skilltree.rs:mouse_down/mouse_up
- §11 (4) → A, crates/d2-client/src/ui/panels/stash_cube.rs:draw/stash_close
- §12 (6) → A, crates/d2-client/src/ui/panels/stash_cube.rs:draw/cube_transmute/cube_close/horadric_pos
- §13 (2) → A, crates/d2-client/src/ui/panels/waypoint.rs:draw/close_hover/tab_frame/choose_row
- §14 (1) → X, heading, content in panels-2
## specs/ui/text.md  A=7 B=12 C=0 X=3
- §1:r1 (1) → B, language byte of data\local\use (ui/text.rs)
- §1 (3) → A, crates/d2-client/src/ui/text.rs:font_info/font
- §2 (2) → A, crates/d2-formats/src/tbl.rs + crates/d2-data/src/strings.rs (UTF-8 string table, lookup by id)
- §4 (1) → A, crates/d2-client/src/ui/text.rs:units/width_a
- §11 (1) → A, crates/d2-client/src/ui/text.rs:centered/centered_span_x
- §14:text (1) → X, heading
- §14 (5) → B, wide formatter 0x005269D0 (new fn in ui/text.rs)
- §15:text (1) → X, heading
- §15 (6) → B, edit box caret/selection/key handler (ui/widget.rs text edit box; caret drawing not implemented)
- §edge-cases-original-bugs (1) → X, edge narration
## specs/client/assets.md  A=1 B=0 C=0 X=2
- §a4-residency (1) → A, crates/d2-client/src/assets (shared Pool residency)
- §a6-writes (1) → X, policy section (what the client may write)
- §b-original-behavior-to-reproduce-not-specified-here (1) → X, pointer heading
## specs/client/audio.md  A=0 B=0 C=0 X=2
- * (2) → X, headings/pointers only
## specs/client/bridge.md  A=43 B=0 C=0 X=2
- §10:text (1) → X, heading
- §10:r7 (1) → X, statement: outputs not persisted
- §10:row1-row40 (40) → A, crates/d2-client/src/bridge/output.rs:Output enum (one variant per row)
- §10:r1 (1) → A, crates/d2-client/src/bridge/output.rs (outputs list)
- §10:r9 (1) → A, crates/d2-client/src/bridge/msg/ui*.rs (UI-keyed lookups)
- §10:r10 (1) → A, crates/d2-client/src/bridge/local.rs / output.rs (UI-requested model writes)
## specs/client/model.md  A=6 B=10 C=12 X=5
- §2:r7 (1) → A, crates/d2-client/src/bridge/world.rs:room_of_point
- §5:r5 (1) → B, bridge/update.rs: unit flag 0x800000 set by active-room free (TODO(spec model.md OQ4) at update.rs:28)
- §7:r9 (1) → C, client game start sends C->S 0x67: app wiring (app/play) + server session (SessionFlow set_session not called by the app)
- §8:r2,r4,r5,r6 (4) → C, client mode machines (player/object/item/monster mode request, ClientFn dispatch): new d2-client bridge module + animation data + audio/overlay requests
- §8:r7 (1) → B, bridge/intent.rs: interact sender 0x00480930
- §11 (1) → X, heading
- §12:r4 (1) → B, bridge/world.rs: nearest-free-point fallback
- §13:text (1) → X, heading
- §13:r2,r6 (2) → A, crates/d2-client/src/bridge/world.rs (ModelInputs visibility predicate input)
- §13 (4) → C, visibility predicate provider: render composite COF/cel box test feeding ModelInputs (d2-client rules/unit_composite + composite)
- §15 (1) → X, heading
- §16:text (1) → X, heading
- §16 (4) → B, bridge/update.rs + world.rs:free_room (teleported hireling 0x4B, flag 0x800000 skip)
- §17:text (1) → X, heading
- §17:r1,r2 (2) → A, crates/d2-client/src/ui/panels/npc.rs:reset_for_interaction + ui/states.rs (E(G), M(U,a))
- §17:r5 (1) → A, crates/d2-client/src/bridge/output.rs:Output::TownExit
- §17:r3,r4,r6 (3) → B, bridge/update.rs / ui/panels/border.rs (stock discard, skill fallback, local room change)
- §18 (3) → C, audio per-unit inputs + call order + ClientFn owner: bridge model passes -> audio::driver (PENDING in audio/driver.rs)
## specs/client/msg-skills.md  A=8 B=0 C=0 X=3
- §1:r3,r4,r5 (3) → A, crates/d2-client/src/bridge/skills.rs (skill list entries)
- §2:text (1) → X, heading
- §2 (3) → A, crates/d2-client/src/bridge/skills.rs:remove/add (0x00646FD0)
- §3 (1) → A, crates/d2-client/src/bridge/skills.rs
- §7:text (1) → X, heading
- §7:r4 (1) → A, crates/d2-client/src/audio/triggers/skills.rs (0x004CA060 client skill event) + bridge Output::SkillEvent
- §edge-cases-original-bugs (1) → X, edge narration
## specs/client/msg-stats-items.md  A=1 B=1 C=0 X=0
- §2:r5 (1) → A, crates/d2-client/src/bridge/msg/stats_items.rs:clear_cursor/item_action
- §2:r6 (1) → B, bridge/msg/stats_items.rs: belt column-ready bytes ready[0..4]
## specs/client/msg-ui.md  A=12 B=7 C=0 X=2
- §1:r8 (1) → X, note, no rule change
- §3:r4 (1) → A, crates/d2-client/src/bridge/msg/ui.rs:trade_action
- §4:r3,r4 (2) → B, UI layer delivery of chat/overhead text (0x0049F490, 0x0049F410): ui/messages.rs (see ui/messages)
- §5:r3 (1) → A, crates/d2-client/src/ui/msg_ui.rs:npc_text
- §7:r4,r5 (2) → A, crates/d2-client/src/bridge/msg/ui_quest.rs:quest_special
- §8:r4 (1) → B, item-socket dialog (ui/messages section 11)
- §9:r3,r4 (2) → A, crates/d2-client/src/bridge/msg/ui_npc.rs:npc_interact
- §10:r2;11:r2;12:r2;13:r2;14:r2;15:r2 (6) → A, crates/d2-client/src/ui/msg_ui.rs:apply_output (UI layer at delivery)
- §16:r6,r7,r8,r9 (4) → B, ui/msg_ui.rs: UI-function model writes, quest record writers, NPC text list walk
- §edge-cases-original-bugs (1) → X, edge narration
## specs/client/msg-units.md  A=1 B=4 C=0 X=1
- §1.2:r6,r7 (2) → B, bridge/msg/units.rs: monster set-up 0x004AE8D0 + table inputs (TODO(spec msg-units.md OQ2) units.rs:191)
- §1.2:r8 (1) → A, crates/d2-client/src/bridge/world.rs:hireling_guid
- §1.3:r5 (1) → X, note: types never sent
- §3:r3 (1) → B, bridge/msg/units.rs: local player room check
- §5:r4 (1) → B, bridge/msg/units.rs: 0x18/0x95 life/mana mode 0x11 rule
## specs/client/render-pipeline.md  A=1 B=0 C=0 X=1
- §a7-composite-units-cof (1) → A, crates/d2-client/src/rules/unit_composite.rs / composite
- §b-original-behavior-to-reproduce-not-specified-here (1) → X, pointer heading
## specs/client/stat-lists.md  A=2 B=3 C=8 X=2
- §1:r1,r2 (2) → A, crates/d2-client/src/bridge/world.rs:stat/base/total (client stat list on ClientUnit)
- §1:r3 (1) → B, ui/panels/character.rs: panel totals from stat list
- §1:r4 (1) → B, bridge/msg/stats_items.rs:hook (empty stub) = client callback 0x004609F0
- §2 (4) → C, client item equip adds item stat lists to owner (0x004C0D20, 0x004C1910, level hook 0x0045D3E0): needs d2-sim stat-list extension in client + item property seams
- §3:r5 (1) → X, owner pointer into dispatch tsv
- §3:r6 (1) → B, bridge/msg/states.rs: state on/off in full
- §4 (4) → C, passive skill state lists (0x00643620): needs d2-sim skill/state tables in client model
- §edge-cases-original-bugs (1) → X, edge narration
## specs/client/ui.md  A=0 B=0 C=0 X=8
- §b8-1-ui-sound-request-sites-controls (5) → X, audit notes (site counts, owners elsewhere)
- * (3) → X, headings/pointers
## specs/world/cube.md  A=4 B=0 C=1 X=3
- §3:r2 (1) → A, crates/d2-sim/src/world/cube.rs (date input)
- §6.1:r3 (1) → A, crates/d2-sim/src/world/cube.rs
- §8 l2:r1,r2 (2) → A, crates/d2-sim/src/wiring/economy/cube_items.rs / wiring/inventory/host.rs (0x9D action 5 send)
- §8 l2:r3 (1) → C, server sound event queue 0x00553380 (S->C 0x2C): no provider yet
- §10 (1) → X, statement
- §edge-cases-original-bugs (2) → X, edge narration
## specs/world/hirelings-2.md  A=2 B=1 C=0 X=3
- §12 (1) → X, links
- §15:r4 (1) → A, crates/d2-sim/src/world/hirelings (resurrect at seller)
- §17:r3 (1) → B, wiring/inventory/copy.rs (TODO(spec vendors-2.md 7.3 step 5): recreate socketed children)
- §18:r4 (1) → A, crates/d2-sim/src/world/hirelings
- §edge-cases-original-bugs (2) → X, edge narration
## specs/world/hirelings-ai.md  A=1 B=0 C=0 X=1
- §1 (1) → A, crates/d2-sim/src/monsters/ai/bodies6.rs:hireling_attack (0x005E4D30)
- §edge-cases-original-bugs (1) → X, moved note
## specs/world/hirelings.md  A=9 B=0 C=1 X=13
- §1.1:r4,r5,r6 (3) → X, live-data / reader provenance
- §6:r3,r8 (2) → A, crates/d2-sim/src/world/hirelings (act change follow, range branch)
- §6:r6,r7 (2) → X, recording evidence / provenance
- §7.1:text (1) → X, heading
- §7.1:r3 (1) → A, crates/d2-sim/src/wiring/interaction/vitals.rs (share on kill 0x0057E990)
- §8:r5,r6 (2) → A, crates/d2-sim/src/world/hirelings/life.rs:kill_with_owner
- §10:r8 (1) → C, hireling items in the d2s save writer (0x005699A0): d2-formats d2s writer + d2-server character storage
- §10:r9 (1) → A, crates/d2-sim/src/... d2s hireling load (impl-d2s-load-hirelings)
- §11:r6,r8 (2) → A, crates/d2-sim/src/wiring/inventory (potions to hireling, failed duplicate)
- §12 (1) → X, links
- §13:r7,r8 (2) → X, links / string id fact
- §14 (1) → A, crates/d2-sim/src/monsters/ai/bodies6.rs:hireling_attack
- §edge-cases-original-bugs (4) → X, edge narration
## specs/world/npc.md  A=0 B=0 C=0 X=3
- §10 (1) → X, dead code in 1.14d
- §edge-cases-original-bugs (2) → X, edge narration
## specs/world/object-population.md  A=47 B=0 C=0 X=21
- §1:text (1) → X, heading
- §1 (3) → A, crates/d2-sim/src/world/objects/populate.rs:populate_room
- §2 (1) → A, crates/d2-sim/src/world/objects/populate.rs:region/want_health/shrine_cap/well_cap
- §3:text (1) → X, heading
- §3 (6) → A, crates/d2-sim/src/world/objects/populate.rs:precheck
- §4:text (1) → X, heading
- §4 (6) → A, crates/d2-sim/src/world/objects/populate.rs:theme_gate
- §5:text (1) → X, heading
- §5 (7) → A, crates/d2-sim/src/world/objects/populate.rs:run/members
- §6 (1) → A, crates/d2-sim/src/world/objects/populate.rs:fit_a/fit_b/fit_c/random_spot/oriented_spot/spread_spot
- §7:text (1) → X, heading
- §7.1:text (1) → X, heading
- §7.1 (4) → A, populate.rs:fn1
- §7.2 (4) → A, populate.rs:fn2
- §7.3 (1) → A, populate.rs:fn3
- §7.4:text (1) → X, heading
- §7.4 (2) → A, populate.rs:fn4
- §7.5:text (1) → X, heading
- §7.5 (2) → A, populate.rs:fn5
- §7.6 (1) → A, populate.rs:fn6
- §7.7:text (1) → X, heading
- §7.7 (4) → A, populate.rs:fn7
- §7.8 (4) → A, populate.rs:fn8
- §7.9 (1) → A, populate.rs:fn9
- §8 (1) → X, live data table
- §edge-cases-original-bugs (11) → X, edge narration
## specs/world/objects-2.md  A=35 B=0 C=9 X=16
- §16:text (1) → X, heading
- §16.2 (3) → A, crates/d2-sim/src/world/objects/mech.rs:trap_door
- §16.7:text (1) → X, heading
- §16.7 (5) → A, crates/d2-sim/src/world/objects/mech.rs:teleport_pad
- §18 (1) → A, crates/d2-sim/src/world/objects/mech.rs:event
- §19:text (1) → X, heading
- §19 (4) → A, crates/d2-sim/src/world/objects/mech.rs:obelisk
- §20:text (1) → X, heading
- §20.1:text (1) → X, heading
- §20.1 (5) → C, object stand-drop provider (ObjectWorld::stand_drop is an empty default at mech.rs:77): armor pick + floor search + item request in wiring/economy
- §20.2 (1) → C, same stand-drop provider (weapon)
- §20.3 (1) → A, crates/d2-sim/src/world/objects.rs (gold drop 0x00559300)
- §20.4 (6) → A, crates/d2-sim/src/world/objects/chests.rs:drop_item_code/code_drop
- §20.5:text (1) → X, heading
- §20.5 (2) → C, same stand-drop provider (class picks 0x00555E70/FB0/60F0)
- §20.6 (1) → C, same stand-drop provider (random class 0x00556240)
- §21 (1) → A, crates/d2-sim/src/world/objects/misc.rs:cure_states (0x00578C20)
- §22:text (1) → X, heading
- §22 (4) → A, crates/d2-sim/src/world/objects.rs allocate + wiring/action/objects.rs (allocation modes)
- §23 (3) → A, crates/d2-sim/src/wiring/action/objects.rs (0x0E/0x4D payload bytes)
- §24:text (1) → X, heading
- §24 (7) → A, crates/d2-sim/src/world/objects/{chests,mech,misc,shrines}.rs (guards)
- §edge-cases-original-bugs (8) → X, edge narration
## specs/world/objects-client.md  A=0 B=0 C=65 X=8
- §25 (7) → C, ClientFn table, call sites, C objects, tick clock: new client-object mode machine fed by ClientWorld (clock seam: GetTickCount choice, client RNG, sound/overlay/set_mode seams)
- §26.* (53) → C, ClientFn 1-18 bodies (same client-object machine; sound via audio triggers objects.rs)
- §27 (2) → C, client session bytes (ClientWorld/UI session state)
- §28 (3) → C, per-client-object state, update order, outputs to other layers
- §edge-cases-original-bugs (8) → X, edge narration
## specs/world/objects.md  A=15 B=0 C=1 X=19
- §7.3:text (1) → X, heading
- §7.3 (5) → A, crates/d2-sim/src/wiring/action/objects.rs (C->S 0x13 object case)
- §8.1 (2) → A, crates/d2-sim/src/world/objects/chests.rs:operate/is_magic
- §12 (8) → A, crates/d2-sim/src/world/objects/misc.rs:portal_travel/portal
- §14:r2 (1) → C, flag 0x400 sound (0x00571740) needs the server sound-event queue; flag 0x100 hover message is client
- §16-18-moved (1) → X, moved pointer
- §edge-cases-original-bugs (17) → X, edge narration
## specs/world/quests-act1-rest.md  A=7 B=0 C=0 X=4
- §4.1 (1) → A, crates/d2-sim/src/world/quests/act1 (missile helper 0x0056EDE0)
- §5 l2:r1 (1) → A, crates/d2-formats/src/d2s (difficulty unlock save loader status checks)
- §5 l2:r2 (1) → X, client title text, presentation of front end (out of scope)
- §8:text (1) → X, heading
- §8 (2) → A, crates/d2-sim/src/world/quests/act1 (tree operate drop result, Den of Evil region)
- §9:text (1) → X, heading
- §9:r5 (1) → X, no reader, note
- §9 (3) → A, crates/d2-sim/src/world/quests/act1 + wiring/economy/quest_host.rs (refresh room 0x0061AED0)
## specs/world/quests-act1.md  A=2 B=0 C=0 X=0
- * (2) → A, crates/d2-sim/src/world/quests/act1 (q2/q5 section headings with body prose)
## specs/world/quests-act2-2.md  A=7 B=0 C=0 X=6
- §1:r1 (1) → X, test vector note
- §1:r11,r14,r15,r16 (4) → A, crates/d2-sim/src/world/quests/act2 + world/objects/mech.rs (orifice, counts, intro record, palace spawn)
- §2:text (1) → A, crates/d2-sim/src/world/quests/act2 (Jerhyn's objects and spawns)
- §4 (1) → X, address list of quests.tsv
- §5.3:text (1) → X, heading
- §5.3 (2) → A, crates/d2-sim/src/wiring/economy/quest_items.rs (remove unit for everyone 0x0052E050)
- §edge-cases-original-bugs (3) → X, edge narration
## specs/world/quests-act2.md  A=8 B=0 C=0 X=1
- §5.9:text (1) → X, heading
- §5.9 (7) → A, crates/d2-sim/src/wiring/economy/quest_items.rs (server Tainted Sun 0x0061C450/C4D0)
- §6.10 (1) → A, crates/d2-sim/src/world/quests/act2 (Jerhyn / palace guard spawn)
## specs/world/quests-act3-2.md  A=1 B=0 C=0 X=1
- §11:text (1) → X, heading
- §11.7:text (1) → A, crates/d2-sim/src/world/quests/act3 (positions and rooms)
## specs/world/quests-act3.md  A=0 B=0 C=0 X=2
- §11 (1) → X, clarifications heading
- §edge-cases-original-bugs (1) → X, edge narration
## specs/world/quests-act4.md  A=0 B=0 C=0 X=5
- * (5) → X, edge narration
## specs/world/quests-act5-2.md  A=0 B=0 C=0 X=5
- * (5) → X, edge narration
## specs/world/quests-act5.md  A=1 B=0 C=0 X=4
- §1.4 (1) → A, crates/d2-sim/src/world/quests/act5.rs (Act V quest objects)
- §edge-cases-original-bugs (4) → X, edge narration
## specs/world/quests-helpers.md  A=8 B=0 C=0 X=3
- §6:text (1) → X, heading
- §6 (4) → A, crates/d2-sim/src/world/quests.rs (end-game 0x00530590)
- §8:text (1) → X, heading
- §8 (4) → A, crates/d2-sim/src/wiring/economy/quest_items.rs:of_type (0x00558110)
- §edge-cases-original-bugs (1) → X, edge narration
## specs/world/quests-status.md  A=4 B=53 C=0 X=12
- §1:r1,r2,r3,r4 (4) → A, crates/d2-client/src/bridge/msg/ui.rs:quest_status + ui_quest.rs:quest_log/quest_special
- §1:r5 (1) → B, bridge reset at game start (quest-log counters/latch) in bridge/msg/ui*.rs
- §1:r6 (1) → X, note: 0x4C is not a quest message
- §2 (1) → B, quest-log entry table (new ui/panels/quest_log.rs data)
- §3 (4) → B, status tables, tab draw/open (quest_log.rs)
- §4:text (1) → X, heading
- §4 (8) → B, row derivation (quest_log.rs)
- §5:text (1) → X, heading
- §5 (6) → B, icon states (quest_log.rs)
- §6 (1) → X, summary of per-quest special cases (restated in entries)
- entry-* (27) → B, per-quest special cases data (quest_log.rs)
- §12:text (1) → X, heading
- §12 (6) → B, quest_log.rs (section 12 rules)
- §edge-cases-original-bugs (7) → X, edge narration
## specs/world/quests.md  A=19 B=0 C=0 X=5
- §1.8:text (1) → X, heading
- §1.8:r1,r2,r3,r4 (4) → A, crates/d2-sim/src/world/quests.rs + crates/d2-formats/src/d2s (completion test, bit 1 without bit 0, record save)
- §1.8:r5 (1) → X, note
- §2.3:r4 (1) → X, field address fact
- §9.1 (1) → A, crates/d2-sim/src/wiring/economy/quest_items.rs (reward item 0x005466B0)
- §9.2 (1) → A, crates/d2-sim/src/wiring/economy/quest_items.rs (delete quest item)
- §9.6:text (1) → X, heading
- §9.6 (13) → A, crates/d2-sim/src/world/quests + objects/misc.rs:portal_travel (quest-owned object init/operate)
- §11 (1) → X, links to acts II-V
## specs/world/vendors-2.md  A=1 B=6 C=12 X=6
- §7.3:text (1) → X, heading
- §7.3:r7 (1) → A, crates/d2-sim/src/wiring/inventory/copy.rs (replenish stat 252/253)
- §7.3.1:text (1) → X, heading
- §7.3.1 (6) → B, crates/d2-sim/src/wiring/economy/item_records.rs / items/bitstream/read.rs (decoder rebuilt fields)
- §10:text (1) → X, heading
- §10.1 (5) → C, C->S 0x4F buttons: d2-server handlers/items.rs only handles cube; needs stash close/withdraw/deposit in sim + S->C 0x77 results
- §10.2:text (1) → X, heading
- §10.2 (4) → C, stash gold withdraw/deposit (clamped add 0x0053FF00): d2-sim stash gold seam + 0x4F server handler
- §10.3 (1) → X, player-trade buttons: out of scope (trade)
- §10.4:text (1) → X, heading
- §10.4 (3) → C, same 0x4F stash handler edge/test vectors
## specs/world/vendors.md  A=2 B=0 C=0 X=4
- §1:r6 (1) → A, crates/d2-sim/src/world/npc.rs (record lookup 0x00535F10)
- §7:text (1) → X, heading
- §7.3 (1) → X, moved
- §8.1:r7 (1) → A, crates/d2-sim/src/world/vendors/trade.rs (handler result)
- §9.2 l2:r5 (1) → X, empty rule
- §edge-cases-original-bugs (1) → X, edge narration
## specs/world/waypoints.md  A=5 B=0 C=0 X=9
- §3:r3 (1) → X, measured-on-saves note
- §4:text (1) → X, heading
- §4:r2 (1) → A, crates/d2-sim/src/world/waypoints.rs (no code sets bits)
- §4:r3 (1) → X, readers note
- §7:r6 (1) → A, crates/d2-sim/src/wiring/path/place.rs:game_entry (spawn search)
- §8:r1 (1) → A, crates/d2-server/src/dispatch.rs (0x13/0x49 drained before tick)
- §9 (1) → A, crates/d2-sim/src/world/objects/misc.rs:portal (town portals)
- §10 (1) → A, crates/d2-sim/src/world/objects/misc.rs / mech.rs (object mode change)
- §edge-cases-original-bugs (6) → X, edge narration
## specs/render/blend-modes.md  A=0 B=0 C=0 X=2
- * (2) → X, edge narration
## specs/render/capture.md  A=0 B=0 C=0 X=2
- * (2) → X, recorder configuration/hooks of the original game (tools/trace-recorder, local Windows)
## specs/render/composition.md  A=0 B=0 C=0 X=4
- §1:text (1) → X, heading
- §1:r1,r2 (2) → X, rationale for choosing the reference renderer
- §7 (1) → X, non-reference renderer (DirectDraw) differences, out of scope
## specs/render/draw-order-2.md  A=4 B=0 C=6 X=2
- §11.3:text (1) → A, crates/d2-client/src/rules/draw_order/weather.rs (rain cycle 0x00473E50)
- §11.9:text (1) → X, heading
- §11.9 (3) → A, crates/d2-client/src/rules/draw_order/weather.rs (particle move 0x004732C0)
- §15.1:text (1) → X, heading
- §15.1 (6) → C, collision line between two units 0x00622AA0: seam is Pending in d2-sim (monsters/ai/seams.rs, wiring/action/pending.rs), also read by audio::driver; provider on the path core line test
## specs/render/draw-order.md  A=3 B=0 C=0 X=3
- §3:text (1) → A, crates/d2-client/src/rules/draw_order/mod.rs:order_grid (grid fill per room)
- §5:text (1) → X, heading
- §5:r2 (1) → X, not in the reference renderer
- §5:r4 (1) → A, crates/d2-client/src/scene/order.rs / rules/draw_order (unit draw at client position)
- §7 (1) → A, crates/d2-client/src/rules/draw_order/source.rs (tile records that never draw)
- §edge-cases-original-bugs (1) → X, edge narration
## specs/render/lighting.md  A=3 B=0 C=0 X=2
- §1:r3 (1) → A, crates/d2-client/src/rules/lighting/map.rs
- §2:r4 (1) → A, crates/d2-client/src/rules/lighting/quality.rs
- §9.3:r5 (1) → A, crates/d2-sim/src/world/environment.rs (server advance 0x0061C040)
- §12:r4,r5 (2) → X, recorded-run evidence
## specs/render/map-preview.md  A=0 B=0 C=0 X=2
- * (2) → X, edge narration
## specs/render/overlay.md  A=0 B=23 C=4 X=9
- §1 (1) → B, overlay record layout (new d2-client/src/rules/overlay.rs, self-contained)
- §2:r9,r10 (2) → C, overlay create needs the light seam (0x00474160) and the unit graphics record check (render/lighting records + unit composite)
- §2 (12) → B, overlay create (rules/overlay.rs)
- §3:r1 (1) → C, per-client-unit overlay walk call site in the client update (bridge update pass -> overlay list on ClientUnit, wall clock seam)
- §3 (10) → B, overlay walk, advance, end, kind 8 cycle, removal (rules/overlay.rs)
- §4:text (1) → X, heading
- §4 (3) → X, restates the draw order of section 2/3 rules
- §5 (1) → C, overlay create call sites (mode machines, skills, UmodFx/UnitOverlay outputs): client model passes
- §edge-cases-original-bugs (5) → X, edge narration
## specs/render/sprite-placement.md  A=1 B=0 C=0 X=1
- §6 (1) → A, crates/d2-client/src/rules/placement.rs / shading.rs (transparency)
- §edge-cases-original-bugs (1) → X, edge narration
## specs/render/unit-composite.md  A=7 B=0 C=2 X=3
- §1:text (1) → A, crates/d2-client/src/rules/unit_composite.rs (which draw path)
- §2 (1) → A, crates/d2-client/src/rules/unit_composite.rs (missing COF -> not drawn)
- §3 (3) → A, crates/d2-client/src/rules/unit_composite.rs (direction and frame)
- §5:text (1) → X, heading
- §5:r3,r4 (2) → C, back-overlay draw in the slot loop 0x0046E300: needs overlay list (render/overlay) + composite slot loop
- §8:text (1) → A, crates/d2-client/src/rules/unit_composite.rs (extra offsets)
- §9 (1) → A, crates/d2-client/src/rules/unit_composite.rs (single-cel units)
- §10 (1) → X, d2rs mapping note
- §edge-cases-original-bugs (1) → X, edge narration
## specs/audio/environment.md  A=4 B=5 C=0 X=5
- §1:r5 (1) → X, inputs owned elsewhere
- §1:r6 (1) → A, crates/d2-client/src/audio/environment/mod.rs (play position by id)
- §2:r10;7:r6 (2) → X, live-table / recording evidence
- §2:r11 (1) → A, crates/d2-client/src/audio/environment/mod.rs:Environment::tick (unsigned time tests)
- §4:r4 (1) → A, crates/d2-client/src/audio/environment/mod.rs (reset)
- §6:r4 (1) → A, crates/d2-client/src/audio/environment/mod.rs (exact previous/stop)
- §9:text (1) → X, heading
- §9:r6 (1) → X, d2rs choice note (wall clock / rand)
- §9 (5) → B, front-end music playlists/pick/toggle (audio/environment/mod.rs new machine; wall clock + CRT rand seams)
## specs/audio/sound-table-2.md  A=7 B=5 C=0 X=23
- §14.1 (1) → X, analysis of audio tick closure
- §14.2 (2) → X, client loop pass order narration
- §14.3 (16) → X, table of non-audio RNG users, each owned by the cited spec
- §14.4 (1) → X, note on draw-phase steps
- §15:text (1) → X, heading
- §15:r6 (1) → X, d2rs setting choice
- §15 (5) → B, options-menu sliders (crates/d2-client/src/audio/sound_table/volume.rs settings)
- §16:r5 (1) → X, d2rs choice note
- §16 (4) → A, crates/d2-client/src/audio/sound_table/system.rs + pool.rs (cache use order, load, evict, unload)
- §17 (3) → A, crates/d2-client/src/audio/sound_table/system.rs (slot taken / failed start / duplicate test)
## specs/audio/sound-table.md  A=22 B=0 C=0 X=4
- §1 (1) → X, struct field table
- §4:r6 (1) → A, crates/d2-client/src/audio/sound_table/system.rs (no local player)
- §5 (3) → A, crates/d2-client/src/audio/sound_table/system.rs (list order, fade, set position)
- §6.2:r3 (1) → X, soundchaosdebug debug-only switch
- §6.3 (4) → A, crates/d2-client/src/audio/sound_table/system.rs (reading order, failed-start fade-in, resume offset)
- §6.4 (2) → A, crates/d2-client/src/audio/sound_table/system.rs + volume.rs (occlusion, step arithmetic)
- §6.6:r1,r2 (2) → A, crates/d2-client/src/audio/sound_table/system.rs (stop, natural end)
- §6.6:r3,r4 (2) → X, voice service thread / device side, no d2rs equivalent
- §7 (4) → A, crates/d2-client/src/audio/sound_table/system.rs (stream, variants, channel kinds, resume)
- §8.2:r12 (1) → A, crates/d2-client/src/audio/sound_table/volume.rs (f32 steps)
- §8.3 (2) → A, crates/d2-client/src/audio/sound_table/volume.rs (occlusion, global gain)
- §10:r1 (1) → A, crates/d2-client/src/audio/pool.rs (cache budget)
- §10 (2) → A, crates/d2-client/src/audio/sound_table/system.rs (preload pass, async completion)
## specs/audio/triggers-2.md  A=15 B=22 C=21 X=6
- §13.* (13) → B, umod / leap / path-flag hook sounds (audio/triggers/modes.rs, skills.rs; hook table already in rules/umod_hooks.rs; inputs from Output::UmodFx / ClientUnit)
- §14 (3) → C, 0x2C ServerSound queue + flush: server unit sound-event slot (0x00553380) in d2-sim per-client update -> S->C 0x2C; client handler is blocked in bridge-dispatch.tsv
- §15 (4) → B, frame-event sounds / generic skill do (audio/triggers/skills.rs)
- §16:text (1) → X, heading
- §16 (4) → A, crates/d2-client/src/audio/triggers/skills.rs (ProgSound conditions)
- §17 (1) → A, crates/d2-client/src/audio/triggers/ui.rs (options-menu UI sounds)
- §18 (4) → A, crates/d2-client/src/audio/triggers/mod.rs:class_record (identity record 0x004CA410)
- §19 (6) → A, crates/d2-client/src/audio/sound_table/system.rs:unit_requests + triggers/mod.rs:detach_skill_voices
- §20 (5) → B, object/ClientFn sound calls (audio/triggers/objects.rs; depends on world/objects-client C)
- §21:text (1) → X, heading
- §21 (18) → C, audio driver input feed per rule: ClientUnit audio fields, frame events, floor material, quest state, weather, NPC interaction (audio/driver.rs PENDING list)
- §edge-cases-original-bugs (4) → X, edge narration
## specs/audio/triggers.md  A=10 B=6 C=0 X=5
- §1:r10,r11,r12,r13 (4) → X, conventions (types, unguarded calls, unsigned time compares, identity lists)
- §4.1 (2) → A, crates/d2-client/src/audio/triggers/modes.rs (mode-set requests, order)
- §5:r9 (1) → A, crates/d2-client/src/audio/triggers/movement.rs (integer types 0x004E4180)
- §7:r7,r8 (2) → A, crates/d2-client/src/audio/triggers/mod.rs:class_record + triggers/objects.rs (classes without record, cairn loop table)
- §9:r6 (1) → A, crates/d2-client/src/audio/triggers/events.rs (server item events)
- §10:r4,r5,r6,r7 (4) → A, crates/d2-client/src/audio/triggers/npc.rs (fixed NPC lines, greeting records, dialog lookup)
- §12:text (1) → X, heading
- §12 (6) → B, other fixed requests (audio/triggers/ui.rs, events.rs)
## specs/drlg/levels.md  A=12 B=0 C=0 X=9
- §1 (1) → X, structure layout for recorders
- §3:text (1) → X, heading
- §3:r5,r7 (2) → A, crates/d2-sim/src/drlg/mod.rs (DRLG creation: tile library, level placement)
- §6:r2 (1) → A, crates/d2-sim/src/drlg/maze (maze after rooms)
- §10:text (1) → X, heading
- §10:r6,r7 (2) → A, crates/d2-sim/src/drlg (spawn room pick)
- §11:text (1) → X, heading
- §11.2:text (1) → X, heading
- §11.2 (2) → A, crates/d2-sim/src/drlg/room.rs (coordinate lists build)
- §11.3:text (1) → X, heading
- §11.3 (1) → A, crates/d2-sim/src/drlg (grid build, tree marks)
- §11.6 (4) → A, crates/d2-sim/src/drlg/active.rs (population build order)
- §edge-cases-original-bugs (3) → X, edge narration
## specs/drlg/maze.md  A=0 B=0 C=0 X=1
- * (1) → X, edge narration
## specs/drlg/outdoor-act3-act5.md  A=2 B=0 C=0 X=3
- §rules:text (1) → X, heading
- §4:r3 (1) → A, crates/d2-sim/src/drlg/outdoor (jungle head row)
- §5:text (1) → A, crates/d2-sim/src/drlg/outdoor (Act V outdoor levels)
- §edge-cases-original-bugs (2) → X, edge narration
## specs/drlg/outdoor-tilesub.md  A=0 B=0 C=0 X=2
- §2.1 (1) → X, context and callers
- §edge-cases-original-bugs (1) → X, edge narration
## specs/drlg/outdoor.md  A=4 B=0 C=0 X=0
- §3:text (1) → A, crates/d2-sim/src/drlg/outdoor (level generation 0x00675360)
- §7:text (1) → A, crates/d2-sim/src/drlg/outdoor (Act I wilderness)
- §7.5:text (1) → A, crates/d2-sim/src/drlg/outdoor (dirt paths 0x00681420)
- §12.2 (1) → A, crates/d2-sim/src/drlg/outdoor (outdoor room and grids)
## specs/drlg/preset.md  A=7 B=0 C=1 X=6
- §1 (1) → X, record layout for recorders
- §2:r3 (1) → A, crates/d2-sim/src/drlg/preset/data.rs (Expansion rows skipped)
- §3.1:text (1) → X, heading
- §3.1:r4 (1) → A, crates/d2-sim/src/drlg/preset (level position/size after allocation)
- §3.2:r4 (1) → C, automap callbacks of preset levels: part of the client automap (see ui/automap section 5)
- §5.2:text (1) → X, heading
- §5.2 (4) → A, crates/d2-formats/src/ds1.rs (DS1 parser keeps)
- §6:text (1) → A, crates/d2-sim/src/drlg/preset/map.rs (building a preset area)
- §12 (1) → X, presentation pops
- §13 (1) → X, lvlprest column use table
- §edge-cases-original-bugs (1) → X, edge narration
## specs/drlg/rooms.md  A=6 B=2 C=1 X=8
- §1 (1) → X, structure layout for recorders
- §2:r5 (1) → A, crates/d2-sim/src/drlg/room.rs (type data)
- §4.6:text (1) → X, heading
- §4.6:r3 (1) → X, statistics copied to globals, no consumer
- §4.6:r10,r11 (2) → B, crates/d2-client/src/bridge/drlg.rs (level-free counter, cursor on freed room)
- §5:r8 (1) → C, act callback +0x4C is set only by the client automap (see ui/automap section 5)
- §6:r4 (1) → X, consumers note
- §8 (2) → A, crates/d2-sim/src/drlg/active.rs (units in a freed room 0x0061A840)
- §9.2:text (1) → X, heading
- §9.2 (2) → A, crates/d2-sim/src/drlg/room.rs / active.rs (build sequence, 0x0066B4C0, 0x0061B560)
- §9.8 (1) → A, crates/d2-sim/src/drlg (RNG sites of the tile code)
- §9.9 (1) → X, recorded test vectors
- §edge-cases-original-bugs (2) → X, edge narration
## specs/drlg/wall-remap.md  A=0 B=0 C=0 X=3
- §2:r4 (1) → X, note: column never read
- §edge-cases-original-bugs (2) → X, edge narration
## specs/tools/original-hooks-spawn.md  A=0 B=0 C=0 X=30
- * (30) → X, trace-recorder tooling for the original game (tools/trace-recorder, local Windows), not crate code
## specs/tools/original-hooks.md  A=0 B=0 C=0 X=47
- * (47) → X, trace-recorder tooling for the original game (tools/trace-recorder, local Windows), not crate code
## specs/tools/scenario.md  A=14 B=0 C=0 X=8
- §1 (2) → A, crates/conformance/src/scenario/script.rs (trace naming, scripts hold inputs only)
- §2:text (1) → X, heading
- §3 (5) → A, crates/conformance/src/scenario/script.rs (target references @player/@x/@type/@wp)
- §3.1 (5) → A, crates/conformance/src/scenario/script.rs (spawn kinds, umods, unresolved refs)
- §4 (4) → X, run model of the original game (recorder side)
- §5:r7 (1) → A, crates/conformance/src/scenario/compare.rs (summary)
- §6:r2 (1) → A, crates/conformance/src/scenario/compare.rs (masks)
- §edge-cases-original-bugs (3) → X, edge narration

