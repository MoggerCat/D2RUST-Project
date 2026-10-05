# Survey of existing open-source Diablo II projects

Status as researched in October 2026. Re-check before relying on it.

| Project | Language | License | Status | Use for this project |
|---|---|---|---|---|
| D2MOO (ThePhrozenKeep) | C++ | MIT | Active (updates into mid-2026) | **Primary reference** for game logic |
| Riiablo (collinsmith) | Java / libGDX | Apache-2.0 | Not playable; sporadic, last commit 1+ yr old | File formats, client/network ideas; code reusable with notices |
| OpenDiablo2 | Go | GPL-3.0 | Archived Dec 2021; reached "walk around Act 1 town" | Read-only reference; **do not copy code** |
| Abyss Engine (OpenDiablo2 successor) | Lua / C | see repos | Quiet since 2023–2024 | Read-only reference |
| OpenD2 | C++ / SDL2 | GPL-3.0 | Halted, unplayable | Read-only reference |
| Diablerie | C# / Unity | see repo | Very early | Low value |

## D2MOO in detail

- "Diablo II Method and Ordinal Overhaul": reimplements the game-logic DLLs
  (D2Common, D2Game, etc.) and loads them into a real D2 process via
  D2.Detours.
- Targets **1.10f**. Replacement functions are only switched on once proven
  bit-exact against the original.
- **Our target is 1.14d.** Patches 1.11–1.13 added content and changed rules,
  and 1.14 merged the DLLs into `Game.exe`. So D2MOO shows how a system
  probably works and what things are called; each behavior is still
  confirmed against the 1.14d binary and traces.
- Intentionally does not fix original bugs, so it is a fidelity reference.
- Does **not** cover a standalone executable, renderer, client UI or
  networking; those are on us.
- Repo: https://github.com/ThePhrozenKeep/D2MOO

## Why none of them are "99.9% D2"

No project is a standalone, near-complete reimplementation. D2MOO is closest
on simulation logic but is a drop-in DLL replacement, not an engine. Our plan
combines D2MOO-informed specs for logic with a new Rust client and server.

## Value for this project

- High: D2MOO (logic, names, structs), Phrozen Keep docs (formats, .txt
  columns), community 1.14d address/struct lists (bridge D2MOO names to
  1.14d addresses).
- Medium: Riiablo (format parsers, client/network structure; adaptable
  with notices).
- Low: OpenDiablo2 (overlaps Riiablo; GPL, read only).
- Skip: OpenD2, Abyss, Diablerie.

Local copies live in `../refs/<project>/`, outside the repo. Spec sessions
read only the files relevant to the behavior being specced.

## Other references

- The Phrozen Keep (https://www.d2mods.info): file format docs, .txt table
  guides, modding knowledge base.
- Riiablo: https://github.com/collinsmith/riiablo
- OpenDiablo2 (archived): https://github.com/OpenDiablo2/OpenDiablo2
