# Measured 1.14d call forms

Call forms of 1.14d functions measured on calls the game makes itself
(`tools/cloud-game/probe_call.py`, format `probe-call-1`), for the poke
directives whose form no spec states (`specs/tools/poke.md` open questions).
The warp entry agrees with `poke.py` `CALL_FORMS` `warp` (read from the
asm, REC-655) on a call the game made itself.

| File | What |
|---|---|
| `warp-0x53aec0.jsonl` | the level warp `0x0053AEC0` during a waypoint warp to Cold Plains (level 3): ECX = game (the tick hook's game pointer), EDX = player (= ESI), stack (level 3, tile 0), ESP moved 12 at the return (`ret 8`), EAX 1 |
