# q-skills-cast: right-click casts the selected right skill (`claude/q-skills-cast`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Synthetic fixtures only. Nothing here is verified against
> 1.14d (rule 10); fills are marked `// d2rs-own, unverified`, spec gaps
> PROVISIONAL (REC-109). Sound not wired.

## 1. The path and where it stopped

| # | Link | State before | Now |
|---|---|---|---|
| 1 | Right press → `click.rs` → C→S 0x0C / 0x0D with the selected right skill | connected (stitch-combat, stitch-hud's skill menu sends 0x3C) | unchanged; test `right_click_on_the_ground_casts_the_right_skill_at_the_point` |
| 2 | Server skill use (`use_at_point`, mana state, `set_mode_with_skill`) | ran: mode 10 started | unchanged |
| 3 | Animation lookup for the mode | `Pending::anim_name` default `None` → `Anim(NoRecord)` for **every** attack / cast / get-hit / death mode: no action frame, no end of mode | **connected**: `app/anim_names.rs` (client art's name rules, live data only) and `anim_rate` = AnimData speed |
| 4 | Action frame → do step (`attack_frame_event`) | `Pending::action_frame` / `skill_event` / `monster_skill_start` / `monster_sequence_frame` were defaults: the do step never ran (no mana spent, no missile) | **connected**: `LocalSeams` routes the four to `skill_events` |
| 5 | The cast target | `World::start_mode` dropped it; `target_position` was `None`, so `skill_missile` returned no missile | **connected**: `UseRest::keep_target` (new, default no-op), kept in `SkillStore`, read by `target` / `target_position` |
| 6 | Mana, missile in the server store, S→C 0x4D | — | test `tests/app_cast.rs`: mana spent, the missile exists, 0x4D reaches the client |
| 7 | Client: cast mode from 0x4C / 0x4D (codes 0x15 / 0x16) | wired (stitch-server-core) | unchanged |
| 8 | Client: missile flight and impact | **not built** | see §3 |
| 9 | Client: state overlay (Frozen Armor) | **not built** | see §3 |

The Fire Bolt row is a test-local copy (the synthetic game has no `skills`,
`missiles`, `itemstatcost` or animdata); the test also installs a synthetic
stat table, because the synthetic game has none (`StatData::default()`: no
stat could be set, so mana and life did not exist).

## 2. PROVISIONAL points (REC-109)

- The kept cast target (`use.md` §4 does not say where it lives).
- The animation key and rate (`animdata.md` OQ2; animation-rate spec not
  written). Bare-hand weapon class (D1: no items).
- `line_clear` still "blocked" on the seam: skills with `lineofsight` > 0
  (Teleport, Static Field ...) do not start in the preview.
- Casting in the camp is refused by the rules (`intown` 0 on Fire Bolt:
  `use.md` §5.3): the player snaps back to town neutral. Walk out first.

## 3. What is left

1. **Client missiles.** The real client creates missiles from 0x4C / 0x4D
   (`intents-events.md` §7.6 r1), but the client missile body is not
   specified (`msg-units.md` §7 r10, `overlay.md` 0x004CF3C0). Needed:
   a missile object in the model (class from the skill's `cltmissile`,
   straight flight at `vel` to the target, impact `explosion`), its art
   (`missiles.txt` anim names → DC6/DCC) in the draw list, and removal on
   hit. Not started: the art loader for missiles does not exist.
2. **State overlays** (Frozen Armor `castoverlay` / state overlay,
   `render/overlay.md`) for the cast and for states on units.
3. Hits: the server missile flies and hits in the sim, but nothing tells
   the client (no get-hit on the target until `Pending::reaction`, q-server-loose).
4. Class skills other than the Sorceress's were not exercised; Charged Bolt
   (`b3_lvl01`) and Frozen Armor (a state) go through the same path but
   have no test.
5. Skill rows with a `srvstfunc` / `srvdofunc` outside the catalogued
   bodies are logged and return 0 (`skill_rest.rs`).

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-skills-cast
git checkout claude/q-skills-cast
cargo run -p d2-client --release -- play --new sorceress Test
```

What to see:

1. Open the right-skill menu (the skill button right of the belt), pick
   Fire Bolt. Walk out of the Rogue Encampment (east gate).
2. Right-click the ground: the Sorceress plays her cast animation (not the
   attack swing), the mana globe drops by 2 per cast (level 1), and
   the cast ends back at neutral. **No missile is drawn yet** (§3 item 1).
3. Right-click inside the camp: the player stays in neutral (casting is
   refused in town).
4. `RUST_LOG=d2_server=debug`: copy any `NoRecord` / `Anim` error into
   `docs/HANDOFF.md` (it means an AnimData name did not resolve; the
   printed key is the COF name in upper case).

Headless: `cargo test -p d2-client --test app_cast --lib right_click`.
