# Stitch: combat and monsters moving (`claude/stitch-combat`)

> Stitching session, 2026-10-07. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Base: `5eda08d9`. Synthetic fixtures only. Nothing here is
> verified against 1.14d (rule 10); every preview answer is marked
> `// d2rs-own, unverified` (decision D1). Sound not wired (deferred).

## 1. The path, end to end, and where it stops

| # | Link | State before | Now |
|---|---|---|---|
| 1 | Left / right click → hovered unit (`bridge/click.rs` `hover`) | `None` always: every click was a ground click (walk) | **connected (preview)**: `bridge::combat::hover_at`, the nearest non-NPC monster whose feet are within 1 sub-tile of the click point, or of the point moved up to 4 sub-tiles down-screen (a click on the body) |
| 2 | Skill row facts / `range` (`click.rs` `skill_row`, `range`) | `None` / `NONE`: §6 r8.2 read every click as a point click | **connected**: client `SkillRow` gained `range` and `flags` (the §6 r8 flag bits), filled from the user's `skills` table (`single_player::client_skill_rows`); `both` reads as h2h (no items, D1) |
| 3 | Hostility `0x00465C60` (`click.rs` `hostile`) | `false` | **connected (preview)**: in town 1 (§6 r9.7); out of town hostile unless the set-up `Align` is 1 (relations, `alSel` / `noSel` not in the model) |
| 4 | Melee range `0x00622C40` | `false` (→ approach walk with a client pending attack nobody fires) | **preview: always in range**, so the click sends C→S 0x06 / 0x0D and the server's `use_on_unit` (`skills/use.md` §3 r6) runs to the target and fires on arrival |
| 5 | C→S 0x06 / 0x0D → server dispatch | — | **stops**: `SimGame::unit_target` (`d2-server/src/adapters/sim.rs:371`) finds no staged `UnitFacts` → `Refused`; point forms 0x05 / 0x0C (and walks 0x01–0x04!) → `point_state` `None` → `Invalid` (`dispatch.rs:287`, `:308`). `set_unit` has no non-test caller. |
| 6 | Server skill handler | — | **stops**: the preview world is `WiredWorld<AppRest>` with `S = NoSkills` (`single_player.rs:140`, `handlers/world/action.rs:132`): skill ids end in the `unhandled` stub (`sim.rs:429`) |
| 7 | Server player skill list | — | **missing**: `LocalSeams` has no `UseRest` / `LearnRest`; the join's `has_skill` / `set_mouse_skills` / `add_skill_level` are `Unapplied` (`adapters/character.rs:478-530`) → `use_::handle_message` would return 3 / 2. play-fix2 is on the 0x94 skill list. |
| 8 | Hit → damage → reaction | — | `Pending::may_attack` defaults to false (`d2-sim/src/wiring/action/pending.rs:168`); `Pending::reaction` is a no-op (`:869`): no get-hit, no player damage |
| 9 | S→C attack / hit / death | — | 0x67–0x6D (monster modes, DT 0x69 code 8, DD code 9) **are sent** in the preview (tick client pass, `wiring/action/unit_update.rs`). **Not built**: 0x4C / 0x4D (logged `WiringError::ModeMessage(SkillMessage)`, `unit_update.rs:169`; the TSV layouts and `d2_proto` types exist now), 0x0C MonsterHit (`unit_update.rs:94`), player attack modes (`path/walk.rs:254-284` sends only walk 0x0F / 0x10 / 0x15) |
| 10 | Client model: monster modes | wired (`bridge/modes.rs` `monster_mode`, by the server mode table) | unchanged; the art follows `unit.mode` (`world_view/unit_assets.rs`), so attack / get-hit / death / corpse modes draw once the messages arrive |
| 11 | Client model: monster motion | a 0x67 set walk mode but the unit stayed on its cell until a 0x6D / position check | **connected (preview)**: `bridge::motion::MonsterMotion` steps every monster in walk / run toward its 0x67 point or 0x68 unit, in a straight line at the message's velocity × 16 (16.16 sub-tiles a tick, `pathing.md` §9.4 r2.1), on the model, after each bridge frame (`world_view::monster_walk`, added by `play::add_walk`); a placement or any other request ends the track |
| 12 | Monster AI engaging the player | — | the AI target seams (`good_target_search`, `nearest_player`, `walk_in_radius`, … `pending.rs:154-162`, `:637-666`) are `Pending` defaults that `LocalSeams` does not override: monsters never acquire the player |
| 13 | Missiles | — | no S→C missile message exists (1.14d's client creates missiles from 0x4C / 0x4D, row 9); the client draws none |
| 14 | Player life | 0x18 / 0x95 handlers wired (`msg/units.rs` `vitals`) | the server sends 0x95 only at the join: `enable_vitals_sync` is never called in the preview (`wiring/action/vitals_sync.rs:44`). Player death: client player codes 0x08 / 0x09 set DT / DD (`modes.rs`); **death screen not specified** (later spec) |
| 15 | Synthetic monsters | — | no synthetic `monstats` row has `isSpawn` (`play-server.md` §3.3): the app test injects its monster as S→C 0xAC |

## 2. What changed

| File | What |
|---|---|
| `bridge/combat.rs` (new) | `skill_flags`, `row_facts`, `skill_range` / `range_of`, `hover_at`, `hostile`, `melee_range` (preview answers, each marked) |
| `bridge/motion.rs` (new) | `MonsterMotion` (module doc: d2rs-own, unverified; PROVISIONAL client/model.md OQ1) |
| `world_view/monster_walk.rs` (new) | `MonsterWalk` resource, `monster_walk_frame` (`PreUpdate`, after `bridge_frame`, before `mirror_units`) |
| `bridge/click.rs` | `hover`, `skill_row`, `range`, `hostile`, `melee_range` delegate to `combat`; test `left_click_on_a_hostile_monster_sends_the_skill_on_the_unit` |
| `bridge/world.rs` | `SkillRow::{range, flags}` |
| `bridge/mod.rs` | module lines; `Bridge::preview_motion` |
| `app/single_player.rs` | `client_skill_rows` fills `range` / `flags` |
| `app/play.rs` | `add_walk` also adds the monster walk (one line) |
| `world_view/mod.rs` | module line |
| `tests/app_play_combat.rs` (new) | the play wiring over the synthetic game: an injected monster walks by its 0x67 and stops on its target; a left press on its feet sends C→S 0x06 [1][77] through the link |

## 3. What is left (in order of the path)

1. **Server unit facts** (row 5): stage `UnitFacts` from the sim's path positions before each drain, or answer `point_state` / `unit_target` from `ActionHooks::path_position` (`wire-path-server.md` §4 item 3). Without it the server refuses the player's walks too: the walk seen in the 2026-10-07 run is the client prediction (check: the server-side player position after a walk).
2. **Skill slot** (row 6): `WiredSkills` in the preview world, which needs a `UseRest` / `LearnRest` on `LocalSeams` (or a provider) and the server player skill list (row 7).
3. `may_attack` (hostility) and `reaction` (get-hit, player damage) providers (row 8).
4. S→C builders: 0x4C / 0x4D for monsters and players (layouts now in `server-messages.tsv`, rule in `sim/intents-events.md` §7.4), 0x0C (needs `0x00597CF0`'s fields specified).
5. `enable_vitals_sync` in the preview build (row 14).
6. AI target providers (row 12).
7. Client: the approach walk with the pending attack (§6 r9.3) once a melee-range rule is specified; predicted / server direction for monsters (play-fix2's `dir64`); the death screen.
8. `isSpawn` on a synthetic fixture copy for an app test with a real spawned monster.

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/stitch-combat
git checkout claude/stitch-combat
cargo run -p d2-client --release -- play --new barbarian Test
```

What to see (needs monsters in the model: once play-fix2 / the
server sessions stream the Blood Moor and make units visible):

- A monster that the server walks (0x67 / 0x68) **glides** cell by cell
  toward where it goes instead of jumping when it stops.
- In town, left-click an NPC: unchanged (NPCs are not hovered by this
  branch). Left-click the ground: still a walk.
- Outside town, left-click on a monster's feet or body: the client sends
  C→S 0x06 (with a message trace on: `06 01 00 00 00 <guid>`), not a
  walk `01`. **The server refuses it today** (row 5): no attack
  animation, no damage yet. That is expected until rows 5–9 land.
- `RUST_LOG=d2_server=debug` (if the dispatch logs result codes) shows
  the 0x06 result `Refused`; copy that line into `docs/HANDOFF.md`.

Headless check: `cargo test -p d2-client --test app_play_combat`.
