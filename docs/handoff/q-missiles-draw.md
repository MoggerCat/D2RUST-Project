# q-missiles-draw: client missiles and state overlays (`claude/q-missiles-draw`)

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Synthetic fixtures only. Nothing here is verified against
> 1.14d (rule 10); fills are marked `d2rs-own, unverified`, spec gaps
> PROVISIONAL (REC-116). Sound not wired.

## 1. The path and what is connected

| # | Link | State |
|---|---|---|
| 1 | Server cast → S→C 0x4C / 0x4D → `modes.rs` code 0x16 / 0x15 → unit `last_mode_request` | already connected (q-skills-cast); the server sends **no** missile (`intents-events.md` §7.6 r1) |
| 2 | Client missile created from the request | **new**: `world_view/missiles.rs` `Missiles::observe` (new request per unit; the first frame only learns) |
| 3 | Missile rows: `skills.cltmissile`, `castoverlay`, `missiles`, `overlay`, `states.overlay1` | **new**: `app/missile_art.rs` (`effect_rows`, `add_missiles`), one call in `app/play.rs` |
| 4 | Flight per server tick, impact, explosion | **new**: `Missiles::advance` / `step` |
| 5 | Art loader (DCC, else DC6, every direction) | **new**: `missiles::load` |
| 6 | Frame items in the draw list | **new**: one call in `world_view/present.rs` after the ground items |
| 7 | Cast overlay (`castoverlay`) and state overlays (`states.txt` `overlay1`) | **new**, same layer |

Tests (`world_view/missiles_tests.rs`, fail before): a synthetic Fire Bolt
cast shows a missile frame item that moves (+16, +8) px per tick; it
explodes at the end of its range and leaves; it stops at a monster; a
state overlay plays on its unit; a missing file is logged once.

## 2. PROVISIONAL points (REC-116)

- Missile start at the cast request (not the action frame); no skill level;
  flight at `Vel` · 4096 per tick; hit = first living monster within one
  subtile; the server's real hits are not sent to the client.
- Art file names and extensions; opaque blend (`Trans` not applied); no
  light; draw key pass 6 after all units.
- A repeated cast with the identical request and no request between is
  not seen as new.

## 3. What is left

Real client skill start / missile functions (spec), the blends, the get-hit
on the target (q-server-loose), `cltsubmissile`s, other overlay columns
(`predraw`, heights, radius light).

## 4. The user's local check (Windows, PowerShell, 1.14d files)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-missiles-draw
git checkout claude/q-missiles-draw
cargo run -p d2-client --release -- play --new sorceress Test
```

Select Fire Bolt on the right skill, walk out of the camp, right-click the
ground: a fire bolt flies from the Sorceress toward the point and bursts
at the end of its range (or at a monster in the way). Cast Frozen Armor:
its cast overlay plays once, then the state overlay loops. A line
`effect art: ... in no archive` means a file name guess is wrong; copy it
into `docs/HANDOFF.md` (REC-116).

Headless: `cargo test -p d2-client --lib missiles`.
