# Diff-driven work: the recipe for area sessions

Every area session works the same loop against the real 1.14d (Wine in the
cloud, native on PC 1). Tools: `specs/tools/scenario-diff.md`; setup once per
container: `tools/cloud-game/README.md` (cloud-setup.sh, setup_winpy.sh,
fetch.sh, prepare_saves.sh).

1. Write `traces/checks/<area>-<what>.check` (copy the channel's example in
   `traces/checks/`): `save` (d2s-tool options: class, level, quests,
   waypoints, items), `seed`, `ticks` (long: 200–2,000), `input frame F; click
   X Y; ...` for what the player does, `at <frame> poke ...` to set up state,
   `channels state` (add `rng`, `packets`, `draws` when the area needs them).
2. Run: `python3 tools/scenario-diff/scenario_diff.py traces/checks/<name>.check`
   (PC 1: `py tools\scenario-diff\scenario_diff.py ...`).
3. Read the first divergence of the first channel that diverges, state first
   (`FIRST DIVERGENCE: frame N <unit> field K: 1.14d A vs d2rs B`); then
   `rng` (the draw and its d2rs site) tells which code took the wrong step.
4. Re-compare only d2rs while fixing: delete `traces/raw/check-<name>/d2rs.*`
   and run with `--reuse` (1.14d is recorded once).
5. Fix the first divergence only, in the spec or the code it traces to
   (M25: never change the expected value); a binary question goes to
   `docs/handoff/pc1-data.md` Step 4.
6. Re-run until the first divergence moves past it; note the new first
   divergence in the commit message.
7. A divergence that is a tool limit (the check file's known limits in
   `pc1-data.md` "How to check a behaviour in one command") is reported to
   q-tool-state-diff, not worked around in the check.
8. Commit the `.check` with the fix (never the `traces/raw/` outputs).
9. Gate (CLAUDE.md), merge `origin/claude/specs-staging-7` (never rebase),
   push.
10. Report one line: check name, the divergence fixed, the new first
    divergence (frame, unit, field), the command.
