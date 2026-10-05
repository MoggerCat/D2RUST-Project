# Clean-room process

Goal: the Rust engine is written from descriptions of **behavior**, not
translated from Blizzard's code. This is what makes the public mod's code
defensible as our own work. (This is a process document, not legal advice.)

## Two roles

**Spec writer (RE role)**
- Works in `re/` with Ghidra and a debugger, plus the reference sources in
  `../refs/` (D2MOO, Riiablo, ...).
- Produces `specs/*.md`: formulas, tables, state machines, edge cases,
  observed numbers, test vectors.
- Never pastes decompiler output, assembly, or near-verbatim C into a spec.
  Write it the way you'd explain it to another programmer.

**Implementer (Rust role)**
- Reads `specs/` only. Does not open `re/` or `../refs/`.
- If a spec is ambiguous, file a question in the spec's "Open questions"
  section and hand it back to the RE role. Don't go peek.

With one person (plus Claude), enforce the split through **separate sessions**:
one Claude Code session for spec writing, a fresh one for implementation that
is told to read only `specs/`. A one-person clean room is weaker than a
two-team one; the written record of specs is what demonstrates the process.

## What a good spec contains

- Inputs, outputs, state touched.
- The rule in plain language and math.
- Tables of constants (these are game data/facts).
- Edge cases and known original bugs (we reproduce them by default; mod
  changes go in the mod layer).
- Test vectors: given seed/inputs X, original game produced Y.
- Provenance: how it was determined (observation, D2MOO reference, community
  docs) — without copying code.

## Asset rules

- Engine loads assets from the user's installation at runtime.
- No Blizzard asset, string, or table is stored in our repo or release.
- The public mod ships only original mod content plus our code.
