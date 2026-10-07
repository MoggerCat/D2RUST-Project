# PC 2 session 4 (2026-10-07 evening): spec-complete pass, report for the cloud coordinator

Goal: every behaviour Phases 0–6 need in PC 2's areas specified (objects, hirelings, all quests, UI except `ui/text.md`, audio, `wav`, `d2s`, items, vendors / cube / npc / waypoints). Base `origin/claude/specs-staging-5` `666e2f2`.

## Merge

One branch holds everything, merged in order and checked (`spec_index --check`, `coverage --check`, no conflict markers): **`claude/pc2-s4-integration`** (this file). Its parts, in merge order:

1. `claude/pc2-sync-staging-5` `3430b0a`: staging-5 + `claude/local-pc2-integration` `ae4ce27` (sessions 1–2, which staging-5 lacked; 8 PC 2 files conflicted, resolved to the newer PC 2 side) + session 3 (`pc2-waveb-tests` `2c1acad`, `spec-ui-controls-inventory` `c5d5159`, `pc2-xpc-msg-rows` `23cf641`).
2. `claude/pc2-spec-gaps` `dd480ad`: the gap audit (another PC 2 session's audit on staging-4 + the update on the synced base).
3. `claude/spec-objects-hirelings-s4` `cda2df9`
4. `claude/spec-items-s4` `e18a010`
5. `claude/spec-world-econ-s4` `83193d1`
6. `claude/spec-quests-s4` `6e9592c` (contains 3)
7. `claude/spec-ui-s4` `b92d91b`
8. `claude/spec-audio-s4` `80867fe` (contains 3 and 4)
9. `claude/pc2-s4-relay` `6adca9e`: `xpc-to-pc1.md` lines and `docs/handoff/pc2-for-cloud.md` (implementation impacts).

CODE-TABLE CHANGE commits this session: **none** (the ones inside `ae4ce27` are listed in `docs/handoff/pc2-session3.md` / the session-2 report: `cfd93db`, `b242ee7`, `39dabf1`, `375bb6b` quests TSVs, `d44907e` object-functions.tsv, `0d617d9` audio/npc-greetings.tsv).

## Per lane (handoff notes `docs/handoff/pc2-s4-*.md` map every code `TODO(spec)` to its answer)

- **objects / hirelings** (`pc2-s4-objects-hirelings.md`, `pc2-s4-objects-client.md`): all 36 TODOs mapped; `objects-2.md` §22 r4 allocation modes, §24 guards; new `hirelings-2.md` §15–§19 (player death kills the hireling in every game type, restore, timer cancels, range pets, entry points / tables); new `objects-client.md` §25–§28 (ClientFn 1–18, wall clock as a `now` input).
- **items** (`pc2-s4-items.md`): `inventory-moves.md` §12 corpse take-back; `generation.md` §12 repair / recharge / runeword removal; `inventory.md` §5.8 weapon bookkeeping `0x0055C5C0`; new `bitstream-legacy.md` (item records for save versions 0x47–0x60); d2rs choices for 2 Pending points.
- **NPC / vendors / cube / waypoints** (`pc2-s4-world-econ.md`): ~45 TODOs; new `vendors-2.md` (item copy moved; §10 C→S 0x4F server side: stash gold, stash max 2,500,000).
- **quests** (`pc2-s4-quests.md`): every `QuestWorld` `unhandled(addr)` default specified; new `quests-helpers.md`; `quests.md` §8.1 order corrected.
- **UI** (`pc2-s4-ui.md`): every `TODO(spec: ui/…)` and the `ui::original::PENDING` gaps; new `panels-3.md` §23–§28; `controls.md` §6–§7 incl. the world-click decision `0x004625B0`; `inventory.md` §8–§9.
- **audio / wav / d2s** (`pc2-s4-audio.md`): `triggers-2.md` §18–§21, `sound-table-2.md` §16–§17, `wav.md` §5, `d2s-load.md` §8, new `d2s-legacy.md`.

## Pending (cannot be settled from the binary; none blocks code — each has a d2rs choice or is a recording)

Hireling death recording; classic game end once per game vs per client; Baal portal ClientFn 16; `0x0046F870` flag 1; legacy 1.00–1.08 save loads; new-character join 0x23; held-button send ticks; scroll-slot pointer writes; player trade (multiplayer, outside Phases 0–6).

## Sizes

`formats/d2s.md` 66 KB and `world/hirelings.md` ~62 KB stay unsplit: every section is cited by code `Covers:` lines (moving Provenance / Open questions to a part file would fix it).
