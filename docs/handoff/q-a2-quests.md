# q-a2-quests: Act II quest hooks in the play host

Branch `claude/q-a2-quests`. Nothing is verified against 1.14d (rule 10). Open point: REC-141 (`docs/HANDOFF.md` §7).

## Finding

The Act II quest rules (`world/quests/act2/q1..q6`), the object routes (altar, tome, orifice, portal, blocker, chests; `quest_objects.rs`), kills (`Pending::kill_step` → `QuestEvent::Kill`, q-a1-tower) and level changes already ran on the wired host. Three hooks had no caller:

| Hook | Caller in the original | Link now |
|---|---|---|
| `q1::radament_ai` (`0x00599420`) | Radament's AI (`QuestCall::RadamentActivated`) | `LocalSeams::ai_quest_call` → `QuestEvent::RadamentActivated` → `run_quest_events` |
| `q5::summoner_seen` (`0x0059C330`) | Summoner AI (`QuestCall::SummonerActivated`) | same, `QuestEvent::SummonerActivated` |
| `q2::staff_assembled` (`0x0059E5C0`) | cube transmute placing `hst ` | `PreviewCubePending::quest_item_hook` records it; `ItemPending::take_quest_items`; `run_quest_events` drains `WiredWorld::cube` → `QuestEvent::StaffAssembled` |

Tests: `d2-server` `world::tests::quests_act2` (Radament outside / inside Sewers 3, Summoner seen, staff assembled, the cube pending's record).

## PROVISIONAL (REC-141)

The hooks run once per tick after the steps, not inside the AI / cube call. `// d2rs-own, unverified`.

## What's left

- No end-to-end client test: the synthetic play world has no Act II (q-act-worlds). Kills of Radament / Summoner / Duriel take the existing `Kill` path; unverified in Act II.
- `use_book_of_skill` (Book of Skill use) has no item-use caller.
- Jerhyn's and Meshif's travel east (`act_change`) is the q-act-travel path; not exercised for these NPCs here.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Reach Act II (Warriv), accept Atma's Radament quest, enter Sewers Level 3: the quest log (Q) moves to "Radament found"; kill him: Book of Skill drops. In the Arcane Sanctuary, approach the Summoner: Tainted/Arcane log updates. Assemble Staff + Amulet in the cube (Horadric Staff): the log shows the staff done. Send log lines with `quest`.
