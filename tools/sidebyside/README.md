# sidebyside: 1.14d and d2rs frames on one page

Judge "same experience as D2" by eye in minutes: every recorded scene
rendered on both 1.14d (under Wine) and d2rs at the same server ticks, in
one self-contained HTML page. Our own code. Handoff:
`docs/handoff/q-tool-side-by-side.md`.

```sh
# once per container: tools/cloud-game/README.md "Setup", then
tools/cloud-game/prepare_scene_chars.sh && tools/cloud-game/prepare_saves.sh
python3 tools/sidebyside/build.py --out ~/sbs/run [--groups walk,panels] [--scenes a1-walk-n] [--reuse] [--jobs 2]
```

| Step | What |
|---|---|
| input | the group script in the shared frame-anchored form (`frame F; click X Y`, `specs/tools/scenario-diff.md` §2 r4): each step acts in the same server frame on both sides (autostart's plain steps drift one to three ticks per click under Wine) |
| 1.14d | per group of `tools/cloud-game/scene_defs.py` (plus `arrival`: the scene of `traces/checks/draws-town-arrival-ama.check`), one `record_frames.py` run with that input, every drawn frame's PNG in `OUT/orig/<group>/img` (`--img-dir`) |
| d2rs | per scene one `d2-client play --dump-draws --at-tick <every 1.14d frame tick up to the scene frame> --dump-image` (`specs/tools/facts-render.md` §5 r19), with the steps acting up to the scene tick T (a paused scene: the steps before its mark) |
| compare | per tick: RGB pixels after each side's palette, equal or not; the scene frame's `facts-compare --ignore tick,index_sha256 --skip-weather` for the first difference line |
| page | `OUT/side-by-side.html`: index (badge, match % of the scene frame, first differing tick, first difference line), per scene the first frame, the first differing frame, fixed ticks 12 / 40 / 100 / 200 and the scene frame, with a slider, a toggle and a diff overlay; `OUT/summary.json` |

**Rule 1:** the frames are rendered game art. `--out` must lie outside the
repository (the tool refuses a folder inside it). The page goes to the
private data repo, `reports/side-by-side/<date>/`, with the command and the
tool version; only this code is public.

Badge: **pass** only when every compared tick is pixel-identical and
`facts-compare` reports a match (rule 10: exact, no tolerance). The scene
frame and the first differing frame are lossless PNG; the other frames are
WebP previews (the diff overlay is computed on the lossless frames and is
exact everywhere).
