# q-fix-replay-hooks

Branch `claude/q-fix-replay-hooks`. REC-1610..1614 unused (tool fixes only).

## Done
- `d2-client state-dump --no-own-c2s <id>[,<id>]` (ids decimal or `0x` hex):
  the bridge's own C->S messages with those ids are dropped (`Bridge::set_drop_own`,
  `take_dropped`), `--send` untouched. One footer note per drop:
  `own c2s dropped: frame F: <hex>`. Spec: `specs/tools/scenario-diff.md` §3 rule 13.
  replay_diff.py should pass `--no-own-c2s 0x5F`.
- `tools/trace-recorder/autostart.py` `clickunit`/`rclickunit`: a unit whose click
  point is outside 800x600 is refused (log line `ERROR ... refused`, nothing clicked,
  per scenario-diff.md §2 r4.5); selftest covers it. ESC-opens-SP-menu note added to
  the autostart docstring and scenario-diff.md §2 r4.4.

## Not verified
- Not run on 1.14d / the ScnAma replay (no game files used); the 0x5F divergence
  at frame 86 should vanish with the flag.
