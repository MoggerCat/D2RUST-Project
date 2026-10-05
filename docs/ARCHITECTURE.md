# Architecture

## The one diagram that matters

```
                 ┌──────────────────────────────────────┐
 user's D2 files │ game/  (MPQs, never in git)          │
                 └───────────────┬──────────────────────┘
                                 │ read at runtime
                ┌────────────────▼───────────────┐
                │ d2-formats  →  d2-data          │  plain Rust
                │ (+ mod patch layers)            │
                └───────┬──────────────────┬──────┘
                        │                  │
          ┌─────────────▼──────┐   ┌───────▼────────────────────┐
          │ d2-sim             │   │ d2-client (Bevy)           │
          │ deterministic      │   │  bridge ◄── snapshots/events│
          │ 25 ticks/sec       │   │  render, UI, audio, input   │
          └─────────┬──────────┘   └───────┬────────────────────┘
                    │                      │ intents
          ┌─────────▼──────────┐   ┌───────▼───────┐
          │ d2-server          │◄──┤ d2-net        │
          │ owns d2-sim        │   │ d2-proto msgs │
          │ accounts, chars    │   └───────────────┘
          └────────────────────┘
```

Single player = `d2-server` running in-process, connected through the same
`d2-proto` messages over an in-memory channel. Online = the same server
remote. There is no separate "offline mode" code path to maintain.

## Boundaries

**d2-sim**
- Input: the previous state + a list of player intents for this tick.
- Output: new state + a list of events (sound, visual effect, chat, etc.).
- Fixed 25 Hz tick, matching the original game. Never variable timestep.
- Unit update order, RNG draw order and integer math follow the original.
- Can be run headless by tests, the server and conformance tools.

**d2-client::bridge**
- Receives snapshots/events from the server.
- Spawns/updates/despawns Bevy entities that *mirror* sim units for drawing.
- Interpolates between ticks for smooth rendering at any frame rate.
- Converts input into intents (move, attack, cast, pick up, etc.).
- Contains no game rules. If you need to know whether an action is legal,
  the server tells you.

**d2-client (Bevy)**
- Palette shader: sprites uploaded as 8-bit index textures; palette and
  color-map lookup happens in the shader.
- D2 blend modes and lighting implemented as custom 2D materials.
- Isometric draw ordering implemented to D2's rules, not a generic depth sort.
- Assets loaded through a Bevy asset loader backed by `d2-formats`, reading
  from the user's MPQs. Any decoded cache stays on the user's machine.

## Data and mods

- Base tables load from the user's MPQs.
- Mod layers are patch files (add/change/remove rows and columns) applied in
  order at load time. The mod release contains only these patches plus
  original mod assets.
- `Ruleset` selects original vs mod behavior where code differs.

## Persistence

- Characters live on the server in our own versioned format.
- `.d2s` import/export is a tool, not the storage format.
- Every persisted format carries a version and has migrations.
