# q-skill-moves: Leap, Whirlwind and Dragon Flight in the play preview (`claude/q-skill-moves`)

> Stitching session, 2026-10-08. Nothing here is verified against 1.14d
> (rule 10); fills are marked `d2rs-own, unverified`, spec gaps
> PROVISIONAL (REC-173). Sound not wired.

## 1. Links connected

The bodies were reached (probed), then stopped on host seams that answer
with a default. All on the existing path provider and skill list; no
second path system (`body_path.rs` unchanged).

| # | Seam | Was | Now |
|---|---|---|---|
| 1 | `UseView::pattern_collides` (`0x0064D910`) | "collides" always: `leap_clamp` failed, Leap returned 0 before mana | DRLG grid query with the unit's path pattern (`skill_rooms.rs`) |
| 2 | entry flags (+0x0C) and params 1 to 4 | dropped by the seam defaults: the Leap / Whirlwind phase flags and the landing point never persisted | `ListEntry::flags` / `params`, read and written by `UseView` |
| 3 | `used_skill_flags` | host seam (0) | the used entry's flags (`use.md` §5.2 step 2), so the moving bit reaches the frame event |
| 4 | `path_point_count`, `path_last_point` | 0 / (0,0): Whirlwind refused ("no path") | the path record |
| 5 | `step_path` (player frame event) | seam, never stepped | `PathCtx::step`, 2 when finished |

Tests: `leap_spends_mana_and_moves_the_barbarian`,
`whirlwind_spends_mana_and_moves_the_barbarian` (`tests/app_barbarian.rs`),
`dragon_flight_moves_the_assassin_to_the_monster` (`tests/app_assassin.rs`).
Each failed before its link.

## 2. PROVISIONAL points (REC-173)

- The collision pattern of the leap's check is the unit's own path pattern.
- Whirlwind's hits along the way: the do step runs only on arrival (the
  existing unit test `attack_frame_events` fixes this reading of `use.md`
  §5.2). The real Whirlwind hits while moving, so the clause needs a trace.
- Test rows: a Leap range formula, charstats speeds, levels with
  `Teleport` = 1 (Dragon Flight lands through the level's flag).

## 3. What is left

Whirlwind's damage while moving (needs the §5.2 answer), Dragon Talon's
kick damage (the body runs; no kick stats in the synthetic game), Leap
Attack, Leap's landing damage, the client's leap arc and whirl animation.

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
git fetch origin claude/q-skill-moves
git checkout claude/q-skill-moves
cargo run -p d2-client --release -- play --new barbarian Test
```

Walk out of the camp, put a point into Leap (and Whirlwind), bind it on
the right button, right-click a ground point: mana drops and the Barbarian
moves there. Dragon Flight (Assassin) in a field: right-click a monster, the
Assassin jumps to it (not in town). Whirlwind: moves, but does not damage
yet. Headless: `cargo nextest run -p d2-client --test app_barbarian --test app_assassin`.
