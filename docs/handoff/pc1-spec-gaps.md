# PC 1 spec gap audit

Pointer: PC 1 gap audit for the "spec-complete" goal (2026-10-08). Read the totals line, then one area table.

Generated 2026-10-08 by grep over `claude/local-pc1-s3` @ 55b75a4 (staging-3 plus PC 1 work through the s3 batch; staging-5 was not pushed). Each `## Open questions` item not marked Answered or struck is classed by keywords: **bin** = answerable from the 1.14d binary, **rec** = needs a recording, hook or capture (goes to the PC 2 recording list), **file** = needs a local game-file count or measurement, **later** = a Phase 6+ or design item. **unspec** counts lines outside the open questions that say not specified / unspecified / no spec yet / Pending. The classes are keyword heuristics: the worker checks each item before acting.

Cost (one focused worker reading the export): bin about 0.3-1M per question (a pointer about 0.1M); an unspec line about 0.5M; rec and file cost nothing here.

**All areas:** bin 131, rec 181, file 16, later 15, unspec lines 73. Being answered on `claude/spec-s3-client` and still counted open here: bridge OQ7, msg-skills OQ2, msg-units OQ8-9, msg-stats-items OQ6, stat-lists OQ3-5, intents-events OQ7-8, ai-bodies-4 OQ3, model OQ16.

## messages / client

Totals: bin 32, rec 25, file 2, later 13, unspec lines 18.

| Spec | bin | rec | file | later | unspec | est. cost |
|---|---|---|---|---|---|---|
| `client/assets.md` | 2 | 0 | 1 | 0 | 1 | ~1.7M |
| `client/audio.md` | 2 | 1 | 0 | 1 | 2 | ~2.2M |
| `client/bridge.md` | 2 | 1 | 0 | 1 | 0 | ~1.2M |
| `client/model.md` | 3 | 5 | 0 | 1 | 1 | ~2.3M |
| `client/msg-skills.md` | 1 | 2 | 0 | 2 | 2 | ~1.6M |
| `client/msg-stats-items.md` | 4 | 1 | 0 | 1 | 0 | ~2.4M |
| `client/msg-ui.md` | 3 | 5 | 0 | 2 | 1 | ~2.3M |
| `client/msg-units.md` | 2 | 2 | 0 | 2 | 1 | ~1.7M |
| `client/render-pipeline.md` | 0 | 0 | 1 | 0 | 3 | ~1.5M |
| `client/stat-lists.md` | 4 | 1 | 0 | 0 | 0 | ~2.4M |
| `client/ui.md` | 1 | 0 | 0 | 2 | 2 | ~1.6M |
| `sim/intents-events.md` | 8 | 7 | 0 | 1 | 5 | ~7.3M |

## combat / units / spawn / pets

Totals: bin 10, rec 23, file 0, later 0, unspec lines 7.

| Spec | bin | rec | file | later | unspec | est. cost |
|---|---|---|---|---|---|---|
| `combat/damage.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `combat/events.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `combat/hit.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `combat/vitals.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `sim/units.md` | 0 | 3 | 0 | 0 | 3 | ~1.5M |
| `sim/pets.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `sim/stat-lists.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `sim/stats.md` | 2 | 1 | 0 | 0 | 0 | ~1.2M |
| `monsters/init.md` | 3 | 3 | 0 | 0 | 1 | ~2.3M |
| `monsters/population.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `monsters/umod-callbacks.md` | 2 | 3 | 0 | 0 | 1 | ~1.7M |
| `monsters/umod-init-bodies.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `sim/unit-order.md` | 3 | 1 | 0 | 0 | 2 | ~2.8M |

## AI

Totals: bin 1, rec 12, file 0, later 0, unspec lines 16.

| Spec | bin | rec | file | later | unspec | est. cost |
|---|---|---|---|---|---|---|
| `monsters/ai-bodies-2.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `monsters/ai-bodies-3.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `monsters/ai-bodies-4.md` | 1 | 2 | 0 | 0 | 3 | ~2.1M |
| `monsters/ai-bodies-5.md` | 0 | 1 | 0 | 0 | 1 | ~0.5M |
| `monsters/ai-bodies-6.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `monsters/ai-bodies-7.md` | 0 | 1 | 0 | 0 | 2 | ~1.0M |
| `monsters/ai.md` | 0 | 3 | 0 | 0 | 10 | ~5.0M |

## skills / missiles

Totals: bin 3, rec 38, file 0, later 0, unspec lines 3.

| Spec | bin | rec | file | later | unspec | est. cost |
|---|---|---|---|---|---|---|
| `skills/bodies-2.md` | 0 | 8 | 0 | 0 | 0 | ~0.0M |
| `skills/bodies-3.md` | 0 | 4 | 0 | 0 | 1 | ~0.5M |
| `skills/bodies-4.md` | 0 | 3 | 0 | 0 | 0 | ~0.0M |
| `skills/bodies.md` | 0 | 4 | 0 | 0 | 0 | ~0.0M |
| `skills/descriptions.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `skills/levels.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `skills/use.md` | 0 | 5 | 0 | 0 | 2 | ~1.0M |
| `missiles/bodies-2.md` | 1 | 2 | 0 | 0 | 0 | ~0.6M |
| `missiles/bodies.md` | 0 | 4 | 0 | 0 | 0 | ~0.0M |
| `missiles/missiles.md` | 2 | 5 | 0 | 0 | 0 | ~1.2M |

## DRLG / render

Totals: bin 21, rec 43, file 7, later 2, unspec lines 9.

| Spec | bin | rec | file | later | unspec | est. cost |
|---|---|---|---|---|---|---|
| `drlg/levels.md` | 0 | 3 | 1 | 0 | 1 | ~0.5M |
| `drlg/maze.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `drlg/outdoor-act3-act5.md` | 0 | 3 | 0 | 0 | 0 | ~0.0M |
| `drlg/outdoor-tilesub.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `drlg/outdoor.md` | 2 | 3 | 0 | 0 | 0 | ~1.2M |
| `drlg/preset.md` | 3 | 1 | 1 | 0 | 3 | ~3.3M |
| `drlg/rooms.md` | 4 | 3 | 1 | 1 | 0 | ~2.4M |
| `drlg/wall-remap.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `render/blend-modes.md` | 1 | 2 | 0 | 0 | 0 | ~0.6M |
| `render/camera.md` | 0 | 4 | 1 | 0 | 0 | ~0.0M |
| `render/capture.md` | 2 | 3 | 0 | 0 | 1 | ~1.7M |
| `render/composition.md` | 0 | 2 | 0 | 0 | 0 | ~0.0M |
| `render/draw-order-2.md` | 2 | 2 | 0 | 0 | 0 | ~1.2M |
| `render/draw-order.md` | 2 | 4 | 0 | 0 | 1 | ~1.7M |
| `render/lighting.md` | 1 | 3 | 0 | 0 | 1 | ~1.1M |
| `render/map-preview.md` | 1 | 0 | 0 | 1 | 0 | ~0.6M |
| `render/shading.md` | 1 | 3 | 0 | 0 | 0 | ~0.6M |
| `render/sprite-placement.md` | 2 | 0 | 3 | 0 | 0 | ~1.2M |
| `render/unit-composite.md` | 0 | 3 | 0 | 0 | 2 | ~1.0M |

## sim core / data / formats / tools / ui text

Totals: bin 64, rec 40, file 7, later 0, unspec lines 20.

| Spec | bin | rec | file | later | unspec | est. cost |
|---|---|---|---|---|---|---|
| `sim/rng.md` | 1 | 2 | 0 | 0 | 0 | ~0.6M |
| `sim/tick.md` | 1 | 3 | 0 | 0 | 4 | ~2.6M |
| `sim/path-placement.md` | 1 | 2 | 0 | 0 | 0 | ~0.6M |
| `sim/pathing.md` | 1 | 3 | 0 | 0 | 0 | ~0.6M |
| `data/calc-expressions.md` | 7 | 3 | 0 | 0 | 13 | ~10.7M |
| `data/callbacks.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `data/field-types.md` | 6 | 2 | 1 | 0 | 0 | ~3.6M |
| `data/fixups.md` | 4 | 1 | 1 | 0 | 1 | ~2.9M |
| `data/loading.md` | 4 | 4 | 0 | 0 | 2 | ~3.4M |
| `data/patch-layers.md` | 5 | 0 | 0 | 0 | 0 | ~3.0M |
| `data/runtime-maps.md` | 0 | 4 | 0 | 0 | 0 | ~0.0M |
| `data/schema.md` | 2 | 0 | 0 | 0 | 0 | ~1.2M |
| `data/txt-format.md` | 7 | 1 | 0 | 0 | 0 | ~4.2M |
| `formats/animdata.md` | 4 | 0 | 0 | 0 | 0 | ~2.4M |
| `formats/cof.md` | 1 | 0 | 0 | 0 | 0 | ~0.6M |
| `formats/dc6.md` | 0 | 0 | 1 | 0 | 0 | ~0.0M |
| `formats/dcc.md` | 0 | 0 | 2 | 0 | 0 | ~0.0M |
| `formats/ds1.md` | 1 | 1 | 0 | 0 | 0 | ~0.6M |
| `formats/dt1.md` | 2 | 0 | 1 | 0 | 0 | ~1.2M |
| `formats/font-tbl.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `formats/mpq.md` | 1 | 1 | 0 | 0 | 0 | ~0.6M |
| `formats/tbl.md` | 2 | 0 | 1 | 0 | 0 | ~1.2M |
| `tools/original-hooks-spawn.md` | 0 | 1 | 0 | 0 | 0 | ~0.0M |
| `tools/original-hooks.md` | 6 | 6 | 0 | 0 | 0 | ~3.6M |
| `tools/scenario.md` | 3 | 1 | 0 | 0 | 0 | ~1.8M |
| `ui/text.md` | 5 | 3 | 0 | 0 | 0 | ~3.0M |

