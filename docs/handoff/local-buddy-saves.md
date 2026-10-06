# local-buddy: real 1.14d test saves

Task `saves`, branch `claude/local-buddy-saves-2026-10-07`. Saves are never
committed (gitignored `*.d2s`; this note has sizes only, no content).

## Where the game writes saves

`%USERPROFILE%\Saved Games\Diablo II\` (confirmed by the run), 4 files per
character: `<name>.d2s`, `.key`, `.ma0`, `.map`. `game\save\` stays empty.
`.key` and `.map` are written when the character is created, `.d2s` and
`.ma0` on every Save And Exit.

## Saves created (all Expansion, Normal, level 1, single player)

| name | class | .d2s bytes | merc | corpse | golem | notes |
|---|---|---|---|---|---|---|
| bdAma | Amazon (0) | 981 | no | no | no | created, Rogue Encampment, Save And Exit |
| bdSor | Sorceress (1) | 958 | no | no | no | same |
| bdNec | Necromancer (2) | 958 | no | no | no | same |
| bdPal | Paladin (3) | 980 | no | no | no | same |
| bdBar | Barbarian (4) | 980 | no | no | no | same |
| bdDru | Druid (5) | 980 | no | no | no | same |
| bdAss | Assassin (6) | 980 | no | no | no | same |
| bdDead | Sorceress (1) | 974 | no | **yes** | no | killed by a monster in Blood Moor; ESC respawned in town; saved without fetching the corpse. 16 bytes larger than bdSor, one extra `JM` item-list header. |
| bdMerc | Barbarian (4) | 985 | **no** | no | no | see "Failed" |

Each `.d2s` in the table was written by the game's own Save And Exit and
checked only for class byte, level byte and size. The class bytes match
the list above.

## Failed / skipped

- **bdMerc has NO mercenary.** Kashya's menu on a fresh character offers
  only TALK and CANCEL (screenshot checked). Hire requires, per
  `specs/world/npc.md` 7.3 step 2, the Sisters' Burial Grounds quest (Blood
  Raven) while the player is below level 8, and then gold (price at least
  the hireling row's gold). Neither is reachable for a level-1 character
  in reasonable time with click automation. bdMerc is therefore just a
  second level-1 Barbarian (985 bytes vs 980 for bdBar; the 5 byte
  difference is unexplained, it only walked a short way around town and
  Kashya's menu). To get a real merc save: kill Blood Raven (any char),
  collect gold, hire, Save And Exit.
- **bdGolem skipped:** level 6 Necromancer is not practical here.
- bdDead took ~25 minutes of walking Blood Moor; monsters are sparse near
  the encampment and several steering clicks were done by hand.

## How it was driven (findings for re-use)

- `Game.exe -w -ns` with cwd = `game/`; window class `Diablo II`; the
  client area is 1200x900 at 150% display scaling, and the game takes the
  window-message mouse coordinates in those physical pixels.
- Menus: `SendInput` mouse clicks work; typed text works (`SendInput`).
  In game, `SendInput` scancodes and mouse are ignored in practice, use
  `PostMessage` (`WM_KEYDOWN/UP`, `WM_MOUSEMOVE` + `WM_LBUTTONDOWN/UP`).
  `tools/trace-recorder/d2ui.py` has both (`pclick`, `pkey`, `click`).
- Wait ~12 s after launch before the first click (earlier clicks are lost).
- `taskkill`/`Stop-Process` on `Game.exe` was refused (access denied) from
  this session; `WM_CLOSE` to the window closes it.
- Screenshots: screen-DC BitBlt of the client rect to PNG (`d2ui.screenshot`).
- Click map (screenshot pixels): Single Player (600,462); Create New
  Character (175,750); class portraits x = Amazon 150, Assassin 300,
  Necromancer 440, Barbarian 600, Paladin 790, Sorceress 920, Druid 1080 at
  y=480; name field (605,760); OK (1037,832); in game ESC then Save And Exit
  (600,390). Kashya is about (1083,725) from the start point.

## Re-run

```
py tools/trace-recorder/make_saves.py classes   # 7 class chars (skips existing)
py tools/trace-recorder/make_saves.py merc      # bdMerc + Kashya menu probe
py tools/trace-recorder/make_saves.py dead      # bdDead (long, may stall)
```

Takes `C:\d2slots\game.lock`, kills the game at the end, screenshots to
`C:\Users\zffit\Desktop\D2test\out-saves\`. Delete a character's four
files in the save folder to regenerate it. `classes` was re-run once
because the first run clicked too early for one character; it is
idempotent.
