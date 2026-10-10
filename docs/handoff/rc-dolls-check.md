# rc-dolls-check hand-back (REC-2295..2299; used 2295)

Check `traces/checks/ui-charselect-dolls.check` (new channel `frontend`, scenario-diff.md §3 r16): 1.14d under
Wine (12 timed charselect shots) vs d2rs (70 dense shots), figure pixels per slot at the best-matching frame.
Ledger EQUAL 1035 -> 1036 (row `ui.frontend.charselect.dolls` NO-CHECK -> EQUAL; the parent row
`ui.frontend.charselect` stays DIVERGED: snow, selection box, REC-1550 brightness offset).

## Result
- slot 0 and 1: 0 differing doll pixels in all 12 shots; slot 2 (Amazon): at most 0.56 % (dark-noise pixels).
- dy probe: offset 0 differs 0..1 px, +-1 row 430+ px: REC-2183 (one-row shift) is exact; settled in the spec.
- Shadow probe: the feet band is not darker in 1.14d than d2rs (+0.4..+1.3 vs -0.1..+0.2 elsewhere on a channel
  sum): REC-2184 settled, no shadow is visible in 1.14d on these backgrounds.
- REC-2182: loop and rate settled (all 12 shots match a frame); the build instant stays PROVISIONAL (input is wall-clock).
- Ran through the suite: `suite.py --filter 'ui-charselect*'` MATCH (about 75 s). Tolerances in dolls_check.py are
  documented in the spec (16 per channel, 1 % of the mask); the REC-1550 brightness offset is why not 0.

## Changed
- `tools/frontend-sbs/dolls_check.py` (comparator), `frontend_sbs.py` (`--script dolls`, display/prefix/bin dir from
  the suite's env), `tools/scenario-diff/frontend_channel.py` + `scenario_diff.py` (channel), `suite.py` (orig output).
- specs: scenario-diff.md (r16, channel table), frontend-menus.md §F2.10; ledger part `rc-dolls-check.tsv`.
- Comment in `front_host.rs` (REC-2183). No logic change; clippy/nextest not run (comment only).

## Open
- REC-2181 equipped saves / slot fix-ups `0x00504D60` (S, needs PC1, already queued).
- REC-2182 build instant (S, needs tick-anchored front-end input).
- Check depends on the three saves of `prepare_saves.sh`; the zero-tolerance version needs the REC-1550 offset fixed (S).
