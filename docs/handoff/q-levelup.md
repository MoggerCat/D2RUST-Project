# Play preview: XP and level up (`claude/q-levelup`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Nothing here is verified against 1.14d (rule 10). Preview fills
> are marked `// d2rs-own, unverified` or PROVISIONAL (M22, REC-94). Audio
> not wired.

## Links connected

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Kill → experience → level-up (sim) | `combat::vitals` (gain, distribute, `level_up`) was complete and tested; in the preview `hooks.vitals` was set but nothing reached the client | unchanged (the sim half is the spec's); the kill path itself is stitch-server-core's |
| 2 | Level, stat points, skill points, attributes, maxima, next-level experience → client | only the join sent stats (`session::stat_messages`); `StatHost` is empty, the changed-stat array is emptied by the room clean-up before the sync, so no S→C 0x1D–0x1F ever followed a change | `wiring/action/vitals_sync.rs` `stat_changes`: at each tick's sync the watched stats (0–5, 7, 9, 11, 12, 30) are diffed against a per-client cache (`SyncState::stats`, seeded at the join) and sent as 0x1D / 0x1E / 0x1F. PROVISIONAL, REC-94 |
| 3 | Vitals sync (0x95, experience 0x1A–0x1C) | never switched on in the preview (`stitch-combat` row 14) | `single_player.rs` calls `hooks.enable_vitals_sync()` |
| 4 | C→S 0x3A (stat point) | `NoSkills` slot: stub | new `app/levelup.rs` `LevelUpSkills` (the world's skill slot `World = WiredWorld<AppRest, LevelUpSkills>`) runs `WiredSkills` for 0x3A and 0x3B only; every other skill id stays a stub (the casting session's) |
| 5 | C→S 0x3B (skill point) | stub | same slot. The skill book is new (`SkillBook` in `LocalSeams`): `Pending::skill_list`, `UseRest` (narrowest answers), `LearnRest` (`is_class_skill` from `skills.txt` `charclass`; the entry gains a level and S→C 0x21 `UpdateItemOSkill` goes to the client). The cost comes off stat 5 in `handlers/skills/world.rs` `LearnUnits::add_skill_level` (levels.md §6.4 step 4; the rest has no stat access) |
| 6 | Level-up buttons (control panel §8) | not drawn | `ui/hud.rs`: `Panel\Level` frame 0 while stat 4 / 5 is above 0, frame 2 otherwise (800 × 600); press and release by `NewButtons`; a click opens the character panel (state 2) / skill tree (state 4) |
| 7 | Panels follow | the character panel and the skill tree already read the model and send 0x3A / 0x3B | unchanged; they update from the stat messages and 0x21 above |

## PROVISIONAL points (grep `PROVISIONAL`; settled by REC-94)

- Stat messages by a diff at the tick's sync, not from the changed-stat array at the unit's client update (`vitals_sync.rs`).
- 0x21 as the message after a spent skill point (type 0, remove 0, base level, bonus 0); what the validator's codes 2 / 3 send (levels.md OQ5) is not sent.
- States 6 and 7 are not driven; the model's unspent points stand for them (`hud.rs` module doc). The 640 × 480 variant of §8 is not drawn.

## Tests

- `crates/d2-client/tests/app_levelup.rs` (new, synthetic, over the real server thread): experience → level 2 with +5 stat points and +1 skill point arrive as 0x1D messages and are not repeated; C→S 0x3A spends points on the server and sends the new values (a request past the points spends what is left, then fails); C→S 0x3B spends the skill point, adds the skill and sends 0x21 + the new stat 5.
- `crates/d2-client/tests/app_levelup_ui.rs` (new): the level buttons are frame 2 with no points, frame 0 once 0x1D 4 / 5 arrive, a click opens states 2 / 4, and they close again at 0 points.
- `d2-sim` `vitals_sync::tests::changed_mod_stats_follow_as_stat_messages_once`.

## What is left

- **Kills in play.** The kill → `distribute` path is the combat session's (`stitch-combat`, `stitch-server-core`): a monster must have stat 13 (its experience) and level set at init. The tests grant experience through `add_experience` (what `distribute` ends in); a synthetic server-side monster kill needs a spawnable `monstats` row (not available in the synthetic game).
- **Skill book from the save.** The book starts empty; the join's saved skill levels (`add_skill_level` in `adapters/character.rs`) and the 0x94 list are play-fix2's. Until then the skill tree shows the points spent in this run.
- **Tool tips** for the buttons (3986 / 3987) need the string table (`q-strings`).
- **Level-up effects**: sound (deferred), the level-up animation and the level-change timer.

## The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-levelup
git checkout claude/q-levelup
cargo run -p d2-client --release -- play --new sorceress Test
```

What to see: the bottom bar shows the two empty level sockets (frame 2) at x 206 and 563. Once a kill grants experience (needs the combat branches) the buttons light up; click the left one to open the character panel, click a `+` button: strength / energy / dexterity / vitality go up and the points count down; click the right one to open the skill tree, click a skill icon: its level becomes 1 and the point is spent. Headless: `cargo test -p d2-client --test app_levelup --test app_levelup_ui`.
