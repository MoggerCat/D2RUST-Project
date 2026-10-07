# PC 2 session 3 (2026-10-07 evening): report for the cloud coordinator

Base: `origin/claude/specs-staging-4` `a8a7458`. Budget: halved; at most 2 subagents.

## 0. Merge first: claude/local-pc2-integration `ae4ce27`

`specs-staging-4` holds PC 2's integration only up to `e76afc8`. Everything PC 2 did after that is on `claude/local-pc2-integration` `ae4ce27`: objects (`objects-2.md`, `object-population.md`, `object-functions.tsv`), UI (`ui/menus.md`, `panels-2.md`, `messages.md`, `control-panel.md`), audio (`sound-table-2.md`, `triggers-2.md`, `npc-greetings.tsv`), items / economy (four passes), `quests-status.md`, d2s passes 3–4, `docs/handoff/pc2-for-cloud.md`, and the strikes of every `xpc-to-pc2.md` line this session's prompt queued (cube.md §1 0x77 `2a72110`; quests.md §6.7, hirelings-ai §1, npc §2 r2, inventory 0x00623990 `4e6adc4`; ui/* consumers `a82a04c`; npc.md §8.2 and the corpse / toa links `15b9899`). A trial merge of `ae4ce27` into `specs-staging-4` conflicts in 26 files (about 45 hunks); 17 of them are PC 1's (`sim/*`, `render/*`, `combat/*`, `crates/d2-proto/src/generated.rs`), so the user decided the coordinator merges it with PC 1's batch. Nothing this session redoes `ae4ce27` work.

## 1. This session

- **Wave B tests** (`claude/pc2-waveb-tests` `2c1acad`, note `docs/handoff/pc2-waveb-tests.md`): C10, C14, C18, C19 PASS (11 `// Covers:` added); C11 FAIL `real_levels_rows` (Act 0 WarpDist ≠ 2025: [(15, 3800), (20, 100), (21, 100), (23, 100), (25, 100)]); C12: new test `maze_defs_exist_in_live_lvlprest` FAIL (15 defs with Files 0: type 19 defs 512, 514–516, 518–524; type 33 Rooms = 1 defs 1–3; fixed def 167; all `maze-specials.tsv` replacement defs have Files ≥ 1). Same binaries also fail `lvlprest_measurements` (80 vs 82) and `every_skill_function_in_table` (catalogue Mapped set empty). No expected value or production code changed.
- **Message rows 0x2A, 0x50, 0x58, 0x63** (this branch): four `xpc-to-pc1.md` lines with builder evidence (`0x0053D740`, `0x0053D7E0`, `0x0053D8D0`, `0x0053D960`); TSVs not edited. 0x2A: layout agrees, bytes 3–6 unwritten (mask). 0x50: verbatim 15-byte copy, no change. 0x58: add `arg:u8@6` (result 5 writes 1). 0x63: @5 is a 16-byte binary record (magic 0x0102 + bits), not `cstr16`.
- **Wave D** (`claude/spec-ui-controls-inventory` `c5d5159`, note `docs/handoff/pc2-spec-ui-controls-inventory.md`): new `ui/controls.md` (command table `0x00712698`, compiled-in defaults `0x00712220`, `.key` / `default.key` (archive copies are version 0x22 / 0x24, rejected by 1.14d), input dispatch, §B4 original-defaults check; skills 9–16 have no default key; hold-to-run / stand-still = flag bits 8 / 4 on every world click) and `ui/inventory.md` (grid, tints, equipment boxes, hover; §B5). 13 open questions (2 need a recording).

## 2. Skipped (owner or waiting)

- PC 1-owned: Wave C monster init / population OQs; Wave D `render/shading.md`, `blend-modes.md`, `lighting.md`; most of the "spec edits, no Ghidra" wording list.
- Waiting for the `ae4ce27` merge (files it rewrote): NPC / vendor OQs, `audio/triggers.md`, `ui/panels.md`, and the PC 2 wording items (`treasure.md` §1.2 GT1, `affixes.md` §5 rewrap, `quests.md` / `npc.md` respec addresses WO1). Also `ui/panels.md` OQ2 → `ui/controls.md` §3 and §9 r6 / OQ5 (`0x004845A0` is the equipment draw, not gold buttons) from the Wave D note.
- Not run (per prompt): the buddy-run failures (room.rs:144, C23/C34–C37/C45/C46, game_wired_host).

## 3. Code for a cloud session

- `crates/d2-client/src/controls/{names,mod}.rs`: build the `original` preset from `ui/controls.md` §3 and check it by §B4.
- Everything in `docs/handoff/pc2-for-cloud.md` (on `ae4ce27`).
