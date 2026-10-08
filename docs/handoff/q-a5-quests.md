# q-a5-quests: Act V quest hooks in the play host

Branch `claude/q-a5-quests`. Nothing is verified against 1.14d (rule 10). Open point: REC-148 (`docs/HANDOFF.md` §7).

## Finding

The Act V rules (`world/quests/act5/q1..q6`, `intro`) and the shared event path (kills, links, level changes: q-a1-tower, q-a2-quests) existed; the Act V hooks and object functions had no caller on the wired host.

| Hook | Caller in the original | Link now |
|---|---|---|
| `q1::shenk_activated` (`0x00587900`) | Shenk's AI (`QuestCall::Shenk`) | `LocalSeams::ai_quest_call` → `QuestEvent::ShenkActivated` → `run_quest_events` |
| `q4::nihlathak_ai_status` (`0x0058BC40`) | Nihlathak's AI | `QuestEvent::NihlathakActivated` |
| `q5::disarm` (`0x0058CF90`) | Ancients' AI (`AncientsNotActivatable`) | `QuestEvent::AncientsDisarm`; the call answers false |
| `q6::chamber_open` (`0x0058E600`) | BaalToStairs AI | `QuestEvent::BaalToStairs` |
| `q4::anya_ai_portal` (`0x0058BC80`) | Anya's AI (`AiActs::anya_open_portal`) | `QuestEvent::AnyaOpenPortal` |
| Object init 62–77, 79 and operate 62–67, 69–72 | object dispatch (`quests-act5.md` §1.4) | `wiring/economy/quest_objects.rs` `init_fn`/`operate_fn` + `init`/`operate` arms |
| Shenk's superunique | `UnitKind::Monster.superunique` was always `None` in the app | `AppRest::unit_kind`: the unit linked to chain 31 reads as superunique 42 |

Tests: `d2-server` `world::tests::quests_act5` (each hook through the host tick; keep door / altar / last portal inits through real object allocation), `d2-sim` `quest_objects::tests::act_v_functions_are_stated` and the existing table check (addresses against `object-functions.tsv`).

## PROVISIONAL (REC-148)

Hooks run once per tick after the steps, not inside the AI call. Shenk identified via chain 31. Operate results ignored. `// d2rs-own, unverified`.

## What's left

- `AncientsActivatable` / `AncientsPortal` (statue AI, `bodies5.rs` `ancient_statue`) still answer false: the portal count needs the quest control.
- Warp gates (`warp_check` 118/128 from 120, 132) have no caller in the warp path; unused for any act.
- No Harrogath / Arreat / Worldstone levels in the synthetic world (q-a5-fields / q-a5-town run separately); kills of Act V bosses use the existing `Kill` path, unverified there.
- Larzuk socket reward, Anya's personalize and the Malah intro have NPC-side callers (q-a5-town).

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new barbarian Test
```
Reach Harrogath (Act V): the quest log (Q) shows Siege on Harrogath; walk to Shenk's ridge: the log moves when he activates (status 2). In Nihlathak's temple the Betrayal log moves on his activation. Note console lines if an Act V quest does not react.
