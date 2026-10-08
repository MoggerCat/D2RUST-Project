# q-sorc: Sorceress skills end to end (`claude/q-sorc`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Synthetic fixtures only. Nothing is verified against 1.14d
> (rule 10); the one spec gap is PROVISIONAL (REC-154). Sound not wired.

## 1. Links connected

The cast path (`q-skills-cast`), client missiles (`q-missiles-draw`) and
states (`q-states-auras`) were reused unchanged. Traced per skill with one
headless test each (`crates/d2-client/tests/app_sorc.rs`: C→S 0x0C → the
server's skill use → the body's effect, on test-local rows):

| Skill | Body | Before | Now |
|---|---|---|---|
| Fire Bolt, Ice Bolt | `srvmissile` path | worked (Fire Bolt, q-skills-cast) | test: mana, missile, S→C 0x4D |
| Charged Bolt | do 17 | worked | test: `calc1` bolts |
| Frozen Armor | do 18 | worked | test: state on the caster, S→C 0xA8 |
| Enchant | do 25 | worked | test: state on the caster |
| Nova | do 22 | worked | test: the ring |
| **Teleport** | do 27 | **nothing**: level `Teleport` none, placement refused | **connected** |
| **Fire Wall** | do 24 | **nothing**: box collision "collides", target point (0, 0) | **connected** |
| **Meteor, Blizzard** | do 28 | **nothing**: as Fire Wall | **connected** |

The break was `UseView`'s room seams (`d2-sim` `wiring/interaction/skill_use.rs`):
they asked `Pending`, whose defaults are "collides", "no level row", "no
free point", "not placed". `skill_rooms.rs` (new) answers them from the DRLG
rooms when the path provider is on (the play host's case):
`level_teleport`, `rooms_box_collides`, `rooms_free_point`,
`rooms_place_unit`, plus `cast_target_point` for the path's target point.
The four tests Teleport / Fire Wall / Blizzard+Meteor fail before.

## 2. PROVISIONAL (REC-154)

The path's target point (`bodies-2b.md` §6.2 step 3) is the point kept at
the cast's mode start. Everything else is the spec's.

## 3. What is left

- Teleport with `Teleport` = 2 (`line_clear` is still blocked on the seam).
- Enchant's weapon check, the Ice Bolt chill, all damage / duration /
  count formulas (the test rows use constants).
- The client draws these through the generic missile / state overlay
  path; the `skills.txt` `cltmissile`, `castoverlay` and `states.txt`
  `overlay1` names of each skill are not checked (live data needed).
- Casting in the camp is refused (`intown`, town levels have `Teleport` 0).

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-sorc
git checkout claude/q-sorc
cargo run -p d2-client --release -- play --new sorceress Test
```

Walk out of the camp, put each skill on the right button and right-click
the ground: Fire Bolt / Ice Bolt / Charged Bolt fly; Frozen Armor and
Enchant show their state overlay; Nova bursts around you; Teleport moves
you to the point; Fire Wall, Meteor and Blizzard place their missiles at
the point. Copy any `effect art: ... in no archive` line into
`docs/HANDOFF.md` (REC-116).

Headless: `cargo nextest run -p d2-client --test app_sorc`.
