# cov-client-audio (coverage session)

Branch `claude/cov-client-audio`, base `claude/specs-staging-7` @ 0c70873. Batch 8 of `uncovered-rules-sort.md`. All claims are `unit` tier (unverified against 1.14d).

## Counts (`python3 tools/coverage.py`, covered / units; before → after)

| spec | before | after |
|---|---|---|
| audio/environment.md | 73 / 82 | 82 / 82 |
| audio/sound-table.md | 112 / 128 | 125 / 128 |
| audio/sound-table-2.md | 6 / 14 | 13 / 14 |
| audio/triggers.md | 129 / 144 | 140 / 144 |
| audio/triggers-2.md | 10 / 63 | 41 / 63 |
| client/bridge.md | 50 / 95 | 94 / 95 |
| client/model.md | 75 / 103 | 81 / 103 |
| client/msg-skills.md | 29 / 38 | 37 / 38 |
| client/msg-ui.md | 53 / 73 | 70 / 73 |
| client/msg-stats-items.md, msg-units.md, assets.md, render-pipeline.md | unchanged |
| tools/scenario.md | 14 / 63 | 15 / 63 (+48 whole-table/`game`-tier column as reported) |

## New code (bucket B)

- Audio (subagent, `crates/d2-client/src/audio/`): front-end jukebox (`environment/frontend.rs`), options sliders (`sound_table/sliders.rs`), cache limit, triggers `ui.rs` (§12), `hooks.rs` (triggers-2 §13), frame-event/skill-do (§15), object ClientFn sounds (§20), identity substitution (§18), mode_set order. Assumptions: triggers-2 §20 r1 vs r2 (ClientFn 3) followed r1; Inifuss panel step time and the quake loop stale handle are unspecified (mirrored).
- `ui/msg_ui_more.rs`: UI dispatch of 0x50 (hire popup), 0x8A (interact sounds/overlay), 0x91, 0x78, 0x29, 0x52, 0x5E, 0x9B, 0x28 client quest record, overhead text store, chat line by type (§4 r3; text filter PROVISIONAL: nothing filtered).
- NPC text list m/m2 per msg-ui §16 r9; `Output::TradeAction.dead_or_absent` captured; close trade(1) toggles inventory.
- `bridge/skills.rs`: `set_bonus`, `add_bonus`, `split_level` (§2 r7).
- `scenario-run`: `default_trace_path`.

## Code fixes found by tests / spec reading

- Audio: mode dispatch used raw unit type instead of the sound identity (triggers-2 §18 r2); group walk used group bases (§19 r3); NPC dialog key compared as 16 bits (triggers §10 r6).
- 0x8A `mdata_3c` was a constant -1: now the 0xAC `value` (msg-ui §9 r2).
- NPC text list m was "first entry" (provisional): now smallest kind-0 id (§16 r9); m2 added. Two older tests encoded the provisional and were updated to the spec.
- Skill remove `0x00646FD0(d)`: d≠0 decrements and frees below 1 (was always unlinked); a refused dangling hand leaves the list unchanged (msg-skills §2 r5–r6).
- 0x93 add clamps a negative bonus at 0 (§2 r7.2); an older test expecting removal from base 1 / bonus -127 was corrected (entry stays).
- A new 0x03 freed the old act's rooms without flagging linked units 0x800000 / 0x20 (model §16 r1–r2).
- Trade code 0x0A now plays on the 0x78 partner.

## Left

- Bucket C: triggers-2 §14 (server sound queue), §21 (driver feed), edge r1; model §8 r2/r4–r6, §18 (client mode machines, impl-c-client); model §13 r1–r5 (render visibility predicate); msg-skills §7 r4 (client skill start); msg-stats-items §2 r6 (needs the item stream header parse, OQ3); model §12 r4 / msg-units §3 r4.5 (needs client collision map); msg-units §1.2 r6 (monster set-up needs many monstats columns, path/light/skill add); msg-units §5 r4 (mode 0x11, Phase 6 player modes); msg-units §3 r3 (callee `0x0061AB30` unspecified).
- model §17 r1–r6, bridge §10 r10 (UI-requested model writes ordered before the next message) and msg-ui §16 r6: not implemented; need a request path from the UI layer and the town-exit delivery inside the update pass.
- X / not claimable: triggers §1 r10–r13, sound-table §1 t2 row4, §6.2 r3, sound-table-2 §16 r5, scenario §4 (original-side run model), edge-case narration, msg-units §8 r11 (multiplayer), client/audio.md pointers, assets §a4, render-pipeline §a7 (not reached).

## Gate

See the final message to the parent session.
