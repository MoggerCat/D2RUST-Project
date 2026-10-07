# Stitch: hirelings (`claude/stitch-hireling`)

Read only `specs/`, `docs/`, `crates/`, `tools/`. Nothing here is verified
against 1.14d (rule 10); every fill is `// d2rs-own, unverified`.

## Links traced (talk -> Hire -> list -> hire -> merc)

| Link | State |
|---|---|
| C->S 0x13 talk -> server `NpcControl::interact` -> S->C 0x4F + 0x4E | existed (`world/npc/hire.rs` `send_hire_list`) |
| Client bridge 0x4F / 0x4E -> `Output::HireListReset` / `HireOffer` | existed, **no consumer** (fell to `skip::NOT_APPLIED`) |
| Hire list UI (box, rows, back, choose -> C->S 0x36) | **added**: `ui/hire_list.rs`, fed in `ui/msg_ui_more.rs`, installed in `ui/original.rs` |
| NPC menu "Hire" option -> open the list | **seam**: `OriginalUi::open_hire_list(npc_guid)` (stitch-npc calls it). Until then `hire_auto_open` opens it for the nearest seller on 0x4F |
| C->S 0x36 -> `NpcControl::hire` -> pay -> `spawn_mercenary` | `spawn_mercenary` had **no provider** (`AppRest` returned `None`, hire ended in code 15). **Added** `LifecycleHooks::spawn_near`, implemented on `ActionHooks`; `Desk::spawn_mercenary` tries it first |
| Spawned merc -> S->C 0xAC + 0x2A code 5 (+ 0x6B) to the client | works: `e2e_night_world::hiring_at_greiz_spawns_the_mercenary` |

## Tests

- `ui::hire_list::tests` (3): list opens for the nearest seller, row click
  sends exactly `0x36 [npc][name]`, Back sends nothing, no reopen after a hire.
- `d2-server/tests/e2e_night_world.rs` hire test now asserts the spawn
  (was: stops at NOT_PLACED).

## Preview fills (d2rs-own, unverified)

- Spawn spot: the NPC's path position + (2, 2) (`units.rs` `spawn_near`).
- List auto-open on 0x4F; choose always takes the "no hireling" direct
  branch (no confirm dialog, `menus.md` §3.4).
- Row text: Life / Def are 0 (no client stat source), no skill text.

## Not done (left)

1. `app/rest.rs` `HirelingRest` is a logging stub: `hireling_ai`, `join_team`,
   `set_owner`, `owner`, `set_mode`, `warp_to`, `dismiss`... So the merc is
   created but does **not follow or fight** yet (`hirelings-ai.md`).
2. Nothing calls `OriginalUi::set_hire_stats`, so list rows draw no text
   until the play app passes a stats closure (`hire_init` over
   `GameData` hire rows). `app/play.rs` call needed.
3. Hireling tables on the live host: the night-world fixture has none
   (`NoHirelingTables` is reported); live `GameData.hirelings` loads them.
4. Merc art (unit art path for the mercenary tokens) not checked.
5. No end-to-end app test through `app_play_e2e.rs` (needs the stats closure).

## Your local check (Windows, 1.14d files)

```powershell
git fetch origin claude/stitch-hireling; git checkout claude/stitch-hireling
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
cargo run -p d2-client --release -- play --new sorceress Test
```
Walk to Kashya, click her. Expect the hire box (gold line, Back); rows only
after item 2. Click Back: box closes. Server log shows `0x4F` then `0x4E`s.
