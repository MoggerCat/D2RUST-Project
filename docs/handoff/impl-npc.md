# Handoff: NPC interaction (`d2_sim::world::npc`) — `claude/impl-npc`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec, medium (METHODS M14). Branched from
`claude/bold-ptolemy-jvyvxy` at `a5b323a`. Repo only. Spec:
`specs/world/npc.md` (+ `world/vendors.tsv`). For the coordinator to fold
into `docs/HANDOFF.md` / `docs/PLAN.md` (not edited here).

## 1. State

**Implemented, unverified** (draft spec; no recording of hire, resurrect,
heal, identify or services exists yet, spec Open question 6).

- New files: `crates/d2-sim/src/world/npc.rs`, `world/npc/hire.rs`,
  `world/npc/services.rs`, `world/npc/tests.rs`. Outside them: one line
  `pub mod npc;` in `crates/d2-sim/src/world/mod.rs`, and this file. No
  other module, spec, dependency or public signature changed.
- Tests: 33 unit tests + 1 ignored game-file test (`live_monstats_records`).
  `cargo test -p d2-sim`: 544 pass, 4 ignored.
- Coverage (`py tools/coverage.py`): `npc.md` 69 / 71 units claimed
  (unit tier). Not claimed: §1.1 r3 (item ids of `cqv` / `aqv`, owned by
  `vendors.md`) and §10 (dead code, not implemented on purpose).
- Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
  --all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run -p
  depcheck`, `python3 tools/spec_index.py --check`, `python3
  tools/methods.py check`, `python3 tools/coverage.py --check`.

## 2. Code map rows

| Path | What | Spec |
|---|---|---|
| `world/npc.rs` | class constants and role lists (`TRADERS`, `GAMBLERS`, `HEALERS`, `IDENTIFIERS`, `SELLERS`, `RESURRECTORS`); `vendors.tsv` embedded + strict parser (`parse_npc_table`); `NpcRecord`, `NpcControl` (`new` = `0x00536070`, record lookup, monstats flags); `InteractionList`; seams `NpcWorld`, `NpcVendors`; handlers `interact` (0x13), `chat_open` (0x2F), `chat_close` (0x30), `menu_action` (0x38), `identify` (0x34); heal (§5); message builders `transaction` (0x2A), `service_result` (0x58), `resurrect_message` (0x9B); `HireRow::from_table` | §1–§6, §9 |
| `world/npc/hire.rs` | `HireRow`, `HireSlot`, `HireList`, `HireOffer`, `MercInit`; `make_hire_list` (§7.1, public for the vendors trade open), `send_hire_list` (0x4F / 0x4E), `hire` (0x36), `resurrect` (0x62), `quest_mercenary` (§7.5), `hire_init` (`0x006637F0`, also the client's offer derivation), `price`, `resurrect_cost`, `capped_level` | §7 |
| `world/npc/services.rs` | `Place`, `InvEntry`, `ItemFacts`, `ImbueMods`; predicates `can_imbue` / `can_socket` / `can_personalize`; imbue, socket, personalize; `imbue_level`, `socket_count`; respec (§8.2), act travel (§8.3) | §6, §8 |
| `world/npc/tests.rs` | fake `NpcWorld + NpcVendors`; every test vector and edge case | Test vectors |

## 3. Integration notes

- `NpcControl::new(monstats, HireRow::from_table(hireling), expansion,
  difficulty, &mut game_seed)` steps the game seed once: call it third
  among the game-creation seeds (after the object-control seed, before
  `QuestControl::new`; `rng.md` §5.2).
- The NPC-control seed (`NpcControl::seed`) is shared with the vendors:
  store generation draws from it (`vendors.md` §3). `NpcVendors::open_trade`
  receives `&mut NpcControl` for that, and calls
  `NpcControl::make_hire_list` for asheara (trade open makes the hire list
  of the four sellers, §7.1).
- **Vendors seam** (`NpcVendors`, provider `world::vendors`, parallel
  session): `open_trade` (`0x00579430`), `drop_gamble_list`
  (`0x00537190`), `pay` (§9.1), `repair` (`0x005761C0`). The vendors
  module can reuse `transaction` for its own 0x2A messages and
  `capped_level` for `vendors.md` §2.
- **Quests through `NpcWorld`**: `quest_text_list` →
  `QuestControl::npc_activate`; `send_game_quests` →
  `QuestControl::send_game_flags`; `send_player_quests` →
  `quests::send_player_flags`; `quest_chat_end` →
  `QuestControl::npc_deactivate`; `respec_offer` / `respec_done` /
  `imbue_granted` → `quests::act1`; `act_completion` →
  `QuestControl::act_completion` (its `level` / `from` arguments are not
  read by the quests side). `QuestWorld::mercenary_reward(player, npc)`
  should call `NpcControl::quest_mercenary`.
- Embed `InteractionList` in the monster data of `interact` monsters
  (monster init); the player's interact unit is the units group's.
- Remaining `NpcWorld` methods: units / stats / states (units, stats
  specs), items (items group, inventory owner), mercenary spawn / init /
  revive (mercenary spec, not written), `encode_text_list` (`0x00661480`,
  not specified: `server-messages.tsv` 0x27 is `partial`), unit / act /
  distance checks (`intents-events.md` §2.4), `start_allowed` and
  `npc_ai_param` (spec Open questions 1, 2).
- Handlers return the dispatcher result code; `interact` returns `None`
  for a unit type other than 1 (another spec's 0x13 branch).

## 4. Narrowest readings (each has a `TODO(npc …)` at the site)

- N1 §2 start rule 1: failures other than `0x00457490` and "already in
  the list" (interact unit set, dead NPC, busy player, Tristram Cain)
  return 0.
- N2 §2: an NPC without an interaction list cannot start (0).
- N3 §5 step 5: a pet's "life to max" is applied (and counts as a change)
  only when below max, like step 1. Removed state lists count as changes.
- N4 §7.3 steps 1, 3: a missing record answers code 9 before the level
  cap (step 1 needs the record's act); a seller without a Normal row or
  hire list answers 9.
- N5 §7.3 step 5: "player level" is stat 12 uncapped (not step 1's lvl).
- N6 §7.5: no row for the game's difficulty, or no slot offered and not
  hired: nothing.
- N7 §8.1 Socket: a failed duplicate refuses before the removal; a failed
  removal after a good duplicate refuses and leaves the duplicate.
- N8 §8.3: call order act completion → act change → waypoint, from
  `quests.md` §8.1 "before the act change"; meshif1 / tyrael2 act change
  argument 0 like warriv1's.
- Edge case 6 (personalize, failed duplicate): stops after the refusal,
  as the spec says for d2rs (Open question 4).
- `0x00576770`'s `first` argument is accepted and unused (the spec gives
  it no effect).
- 0x2A bytes 3–6 and 0x58 byte 6 are written as 0 (edge cases 1, 10).
- Unit lookups: 0x2F / 0x30 use `unit_by_guid` (any type; a non-monster
  → 3); 0x13, 0x34, 0x36, 0x38, 0x62 use `monster_by_guid`.

## 5. Checks to queue (local run queue, `docs/HANDOFF.md` §5)

1. **Game files**: `D2_GAME_DIR=… cargo test -p d2-sim -- --ignored
   live_monstats_records`: 47 records from live `monstats`, every one of
   the 43 `vendors.tsv` entries attached with its act / trader / byte 6,
   and a Normal expansion `hireling` row for each seller. Also check that
   the `hireling` name ids at +0x114 / +0x116 are present in the loaded
   table (fix-up) before `make_hire_list` runs on live data.
2. **Recordings** (spec Open question 6, `packets-0002`): replay the
   talk / trade sequence of `20261006-015956-packets.jsonl` (frames 746,
   747, 898, 1958) and the hire / resurrect / heal / Cain / service
   recordings through these handlers once the seams have providers;
   compare message bytes (0x2A bytes 3–6 masked) and order. These settle
   N1–N8 and spec Open questions 1, 4, 5.
