# rc-pc1late-world hand-back

Checks (state channel, this checkout, EQUAL before -> after):
- a1-warp-tower-cellar-ama: 160/160 frames equal -> 160/160 (rng MATCH)
- a3-warp-durance-ama: MATCH -> MATCH
- milestone-act5-entry: 40 frames, no difference -> same

The three queue rows were already EQUAL on staging-7 for these checks
(measured with the baseline code for the vision one; orb and Larzuk run
on the current code only). Orb and Larzuk code paths the rows call
missing exist: `HostQuests::spawn_monster_flags` (quest_host.rs) takes
`spawn_monster_at_unit`; `spawn_preset_units` stores map-AI paths for
543/459/461 (`MAP_AI_STORE_CLASSES`), `apply_map_ai` is implemented in
quest_host.rs. No code change for them.

Changed (vision token, `ai.md` §5.2 steps 2/7, 0x005DD9B6, 0x005DDBE6):
- `monsters/ai/target.rs`: step 2 records whether the vision record was
  loaded and S; step 7 writes +0x24 := (S == 0) only when loaded.
- `mark_seen(unit, value)` in `AiWorld`, `wiring/action/ai.rs`, pending.
- Unit test `vision_token_toggles_only_when_loaded`; `ai.md` text updated.
  Record identity (act, rect, index) stays PROVISIONAL REC-1698.

Open: `build-queue.tsv` is sync-owned; rows q-fix-pc1late-vision-token,
-orb-spawn, -larzuk-map-ai can be dropped by the integrator. Narrower
checks (frame 45 token, frame 26/34 Larzuk steps) not isolated.
