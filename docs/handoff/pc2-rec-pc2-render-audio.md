# PC 2 recording list — render / audio lane (`claude/pc2-render-audio`)

Open questions of `render/shading.md`, `render/blend-modes.md`,
`render/lighting.md`, `audio/*`, `client/audio.md` and `formats/wav.md`
that need a recording, a capture or a debugger read of the running 1.14d
game. The coordinator merges this file into `docs/HANDOFF.md` §7.

Common set-up: Windows, `game/Game.exe` with the reference hash
(`docs/LOCAL-RUN.md` §0.1), recorders of `tools/trace-recorder`
(`README.md`). Frame captures: `record_frames.py` (`render/capture.md`
§8); packets: `record_packets.py`. **Audio entries (RA-A*) need a recorder
extension first (M10):** a `record_sound.py` subclass of
`record_tick.py`'s `TickRecorder` with the breakpoints each entry names
(read registers and the listed memory at the breakpoint, log one JSON line
with the client frame and the last server tick). Until it exists, the same
reads can be taken by hand with x32dbg (conditional breakpoint "log
without break", the log text given per entry).

| Id | Spec, question | Recording / capture (exact steps, command) | What to look for |
|---|---|---|---|
| RA-L1 | `render/lighting.md` OQ5 (0x89 id 0) | `py tools/trace-recorder/record_packets.py --seconds 300` plus `record_frames.py --seconds 300` in a second sitting with the same save. New Amazon, Normal, enter the Den of Evil, kill every monster, stand still 10 s in the Den, walk out. | One S→C `89 00` when the last monster dies (`world/quests.md` §6.5); from that tick the Den's ambient follows §10 r1 (frames: `env` and the light-map digest change at that tick). |
| RA-L2 | `render/lighting.md` OQ6 | `record_packets.py --seconds 60` and `record_frames.py --seconds 60` (two runs). Start Single Player, load any character, wait 10 s in town, quit. Repeat for a character saved outdoors (Cold Plains waypoint). | The flushed S→C buffer holding 0x53 precedes the first in-game `EndScene` frame (record_frames `frame` 0 tick ≥ the 0x53 tick). Any frame drawn before 0x53: its `env` must be `I` 128, white (§9.1). |
| RA-L3 | `render/lighting.md` OQ7 (`sin`/`cos` rounding) | First extend `record_frames.py` to log env +0x08 (ticks) beside +0x0C. Then `record_frames.py --seconds 2100` (one day: speed 128 × 360 = 46,080 ticks ≈ 1,843 s, §9.1, §9.3 r2). Blood Moor, stand still at a dead end, no panels, no eclipse. | For every frame, recorded `env[0]` (env +0x0C) equals `I` computed by §9.3 r4 from the recorded ticks; list every θ where it differs by 1. |
| RA-L4 | `render/lighting.md` OQ9 | Rerun `capture.md` run 1b's still segment (Rogue Encampment, 7 standing positions, 30 s each) with `record_frames.py --seconds 300 --draws-every 1`, and the light-map digest of §12 r3 in the key. | The light records (§6.1 fields) listed in the draw log at frames whose map differs from the previous frame of the same key; name the record (unit type and class) whose position changed. |
| RA-B1 | `render/blend-modes.md` OQ1, OQ2 (shadow position, blend-table orientation) | `record_frames.py --seconds 60 --draws-every 1` after RA-B3's recorder change. Rogue Encampment, a flat open floor patch away from walls and torches, daytime, character standing still 20 s, no panels; then the same character with an ethereal weapon in the right hand and an ethereal shield (a `d2s-tool` save, `docs/LOCAL-RUN.md` §6.7) standing 20 s on the same spot. | CPU reference renders of those frames (`d2-client verify`, `composition` scene) equal the capture: the shadow shape and position of §5 r2–r4, its pixels `A0[256·d + 0]`; weapon pixels with the mode-1 table and shield pixels with the mode-2 table read in the §2 orientation. A frame that matches only with row and column swapped decides the orientation the other way. |
| RA-B2 | `render/blend-modes.md` OQ2 (§6 translucent walls) | `record_frames.py --seconds 30 --draws-every 1`. Lut Gholein, walk slowly behind a building wall towards the palace (wall between camera and player), stop 5 s behind it, walk out. | For every frame the wall record's alpha byte in the draw log and its pixels equal §6 (`A0` / `A1` / `A2` by the alpha band, blocks under 0x40 not drawn) with the §2 orientation. |
| RA-B3 | `render/blend-modes.md` OQ3 | Extend `record_frames.py` `read_light` to log the settings word `[0x0072DA5C]` (Blended Shadows) once per frame; any recording afterwards (e.g. the stability-0001 capture of `docs/LOCAL-RUN.md` §6.1). | The value (0 or 1) on the recording machine; RA-B1's shadow pixels use `A0` only when it is 1. |
| RA-B4 | `render/blend-modes.md` OQ6 (GDI lines) | `record_frames.py --seconds 60 --draws-every 1`. Act 1 Cold Plains during rain (weather on: frames' `weather.rain` = 1), stand still 30 s; or a Sorceress casting Arcane-star overlays. | Rain line pixels equal §8 r1 (x-major at 45°, endpoints as given) for the logged line draws. |
