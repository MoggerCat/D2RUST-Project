# Decisions to get right early (to avoid redoing work later)

Each item here is cheap to decide now and expensive to change later.

## Architecture
1. **Bevy only in the client.** Logic in Bevy means rewriting it for the
   server and fighting the ECS for deterministic order.
2. **Client/server split from the first playable build.** Retrofitting
   networking onto a single-player game is one of the most common causes of
   total rewrites. Single player is a local server.
3. **Thin bridge.** Keeps Bevy upgrades (and any future renderer change)
   confined to one crate.

## Fidelity
4. **Lock the target version (1.14d).** Specs and traces depend on it. D2MOO documents 1.10f, so every behavior taken from it
   must be confirmed against 1.14d.
5. **Integer/fixed-point math in the sim.** Swapping floats out later
   changes every result and invalidates every conformance test.
6. **RNG and update order.** Implement and test the RNG first; never iterate
   unordered collections where order affects outcomes.
7. **Fixed 25 Hz tick.** Rendering interpolates; logic never runs at a
   variable rate.
8. **Trace format early.** Define a versioned trace format before recording
   lots of traces, or you'll re-record them.

## Gameplay decisions that affect online fairness
9. **Visible area / resolution.** In D2, what's on screen affects gameplay
   (monster awareness, item visibility). Decide the supported view size
   (e.g. 800×600 logical, scaled) and make it identical for all players.
10. **Original bugs.** Reproduce by default; decide mod changes per bug and
    record them in the spec under `Ruleset::Mod`.

## Data, mods and distribution
11. **Mods as patches.** The release must not contain modified copies of
    Blizzard tables (they still contain Blizzard data).
12. **No Blizzard files ever in git history.** Removing a committed MPQ
    later means rewriting history. Install the pre-commit hook on day one.
13. **Versioned formats** for saves, protocol, traces and caches from the
    first commit.

## Online service
14. **Never trust the client.** Item generation, damage, drops and
    inventory changes happen only on the server.
15. **Accounts done properly.** Use established libraries: argon2 for
    passwords, TLS for transport, no home-made crypto. Plan for data
    protection obligations for the countries your players are in
    (e.g. KVKK in Türkiye, GDPR in the EU).
16. **Ownership gate is two layers.** A "has the game" check on the client
    (required MPQs present, valid, containing the expected files), plus the
    account system. No exact-hash match against one Blizzard release
    (official installs changed over time), and nothing invasive: only the
    chosen game folder is read.

## Process
17. **Clean-room record.** Specs with provenance, code citing specs. Hard
    to reconstruct after the fact.
18. **Commit after every Claude Code session** with a clear message, so any
    bad session can be reverted cleanly.
19. **Keep `PLAN.md` current.** It's Claude Code's memory between sessions.
20. **Small tasks.** One spec or one module per session. Large vague tasks
    produce large vague code.
