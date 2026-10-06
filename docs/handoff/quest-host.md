# Handoff: quest world host on the server — `claude/quest-host`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Base: `main` at `edad871`. Repo only,
synthetic tables, fixed seed (1234), no game files (M09). For the
coordinator to fold into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is
edited here). Closes HANDOFF §2 step 3's quest part and
`e2e-vendor-host.md` §6 item 2.

## 1. State

**Wired, unverified** (M02): `quests.md` is a draft; this layer adds
no rule, it routes the existing `d2-sim` quest module onto the unit
world `TradeWorld` already uses.

- `WorldHost::quests` on `handlers::world::TradeWorld<R>`: the real
  `QuestControl` runs on `wiring::economy::EconomyQuests` built from
  the interaction `Desk`'s economy and rest (same per-call economy as
  the NPC / vendor calls: the action sim's units, stats, data, hooks;
  game seed lent and written back). C→S 0x31, 0x40, 0x58 leave their
  stubs on this host. `handlers::world` (`world.rs`) is unchanged; on
  `ActionWorld` the three ids stay stubs.
- **Mercenary reward routed (HANDOFF §2 step 7b).** The desk allows it:
  `EconomyQuests::mercenaries` collects the rewards `0x00579180` during
  the quest call; right after it they run as
  `NpcControl::quest_mercenary(desk, player, class)` with the NPC
  control block (`npc.md` §7.5), before the result goes back. This is
  `Desk::quest_message`'s order, applied to every quest call (the host
  takes a generic `QuestCall`, so it cannot call `Desk::quest_message`
  itself). A reward's `NpcError` goes to `state.errors` as there.
  `QuestRest::mercenary_reward` is no longer reached on this host.
- Act II–V callbacks: none is raised by these Act I paths. The
  `unhandled` calls the tests reach are Act I gaps, left as they are
  and asserted exactly (§4).

## 2. Code map rows

| Path | What |
|---|---|
| `crates/d2-server/src/adapters/handlers/world/trade.rs` | `TradeWorld::quests` (`WorldHost`), the mercenary routing |
| `crates/d2-server/src/adapters/handlers/world/tests/trade_quests.rs` | 7 tests on `SimGame<ActionSim, TradeWorld>` through the host frame; staged `Rest` |

## 3. Tests (`cargo test -p d2-server --lib trade_quests`, 7 pass)

| Test | C→S | Asserted |
|---|---|---|
| `akara_message_64_starts_den_of_evil_then_chat_end` | `31 <Akara GUID> 4000 0000`, then `30 01000000 <GUID>` | Done; S→C `27 01 <GUID>` + 34 staged zero bytes, then `29` + game record (§7.2); record diff = slot 1 bit 2 only; chain 1 state 2 (§10.1); chat end → `5d 01 00 01 0000`, slot 1 = `04 00` (Test vectors, `015956` 1729–1751); exact `unhandled` list |
| `kashya_message_92_grants_the_mercenary_on_the_npc_control` | `31 <Kashya GUID> 5c00 0000`, slot 2 bit 1 preset | Done; S→C `50 0200 <first offered name> 00×10` (`npc.md` §7.5); slot 2 bit 0 set, bit 1 cleared, nothing else; chain 2 state 5; the slot is hired; spawn seam tried with modes 4, 6, 12; `QuestRest::mercenary_reward` never called |
| `kashya_message_92_without_reward_pending_does_nothing` | same, no preset | Done; nothing sent; record unchanged |
| `request_quest_data` | `40` | Done; `28 06 00000000 00` + record, `52` + 41 zero bytes (fresh game: every status 0, no 0x50) |
| `one_flag_bit_changes_one_byte_of_the_0x28` | `40` ×2 runs | **M08**: slot 3 bit 2 set → exactly one byte differs, 0x28 byte 13, xor 0x04 |
| `quest_completed_sets_the_log_bit` | `58 0500`, `58 2900`, `58 2a00` | Done / Done / Invalid; record diff = slot 5 and slot 41 bit 12 only; nothing sent |
| `same_seed_same_run` | 0x31 ×2, 0x40 | identical codes, frames, records, log twice |

M08 on the routing too: with the `mercenaries` collection removed from
`TradeWorld::quests` (local edit, reverted), the Kashya test fails
(no 0x50; `rest merc reward` logged instead).

`Covers:` claims: `quests.md §7.3`, `§6.2 r1, r2, r4`, `§1.7`. The
Kashya tests carry none (`npc.md` §7.5's expansion and refill branches
are not asserted; §10.2 / §10.5 are whole-table units).

Fixture answers (staged, not behaviour): 0x27 bytes 6–39 zero
(`0x00661480`, `server-messages.tsv` 0x27 `partial`), no hireling pet,
mercenary spawn fails, unit act 0, one `hireling` row for Kashya
(version 0, Normal, names 100–104), hire list made at creation with
`make_hire_list` on the NPC seed. The player enters through
`QuestControl::player_enters` (mode 0) before joining.

## 4. Seams reached

| Seam / callback | Where | Owner |
|---|---|---|
| `unhandled 37 0x58f870` | chain 37 (Act I intro) event 11, every 0x31 | `quests.tsv` chain 37; `wire-open-seams.md` open question (order vs. the mercenary reward) |
| `unhandled 37 0x58f8f0`, `6 0x595e20`, `5 0x594c50`, `4 0x592580`, `3 0x5916a0`, `2 0x590b10`, `1 0x58ff90` | event 0 (NPC activate) of the text refresh after Akara's 64 (§7.2): text list comes out empty | Act I event-0 callbacks (`quests.md` §10, not written) |
| `QuestRest::send_text_list` | 0x27 of the refresh | `0x00661480` builder |
| `NpcRest::spawn_mercenary`, `init_mercenary` | quest mercenary after 0x50 | monster spec |
| `QuestRest::*` player data (records, act, level) | every call | player spec |

## 5. Signature changes

None. No public item added or changed; `world.rs`, `action.rs` and
`d2-sim` untouched. Stale doc: `world/action.rs` lines 5–6 still say
"The quests' ids stay stubs on both" (file not owned here); should read
"on this host; `TradeWorld` adds them".

## 6. Questions

1. Should `Desk::quest_message` (d2-sim) become a generic
   `Desk::quests(|ctl, w| …)` so the host's copy of its reward loop
   goes away? Left as is (d2-sim not touched).
2. The 0x40 handler returns 0 by reading (`world.rs` `QuestRun`: "the
   spec names no result"); unchanged.

## 7. Local checks to queue

When a packet recording replay through the server host exists
(`e2e-vendor-host.md` §8), run `015956` frames 1729–1751 through
`Host` + `TradeWorld`: expect S→C 0x27, 0x29 in frame 1729 and
`5d 01 00 01 0000` at 1744; compare bytes with 0x27 bytes 6–39 masked
until `0x00661480` is specified.

## 8. Gate

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets
-- -D warnings`; `cargo test --workspace` (all ok, 0 failed);
`cargo run -p depcheck` (OK, 8 crates); `python3 tools/spec_index.py
--check`; `python3 tools/methods.py check` (21 OK); `python3
tools/coverage.py --check` (0 errors) and `--selftest` (ok).
