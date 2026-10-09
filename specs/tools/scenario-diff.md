# Spec: Tools — One-command checks against 1.14d (`scenario-diff`)

- **Status:** draft: the check-file format and the run; the `state`
  channel runs on both sides, `draws` reuses the rendering-facts tools,
  `rng` and `packets` record 1.14d only (no d2rs recorder yet).
- **Target version:** 1.14d (the original side); the format is d2rs-own.
- **Crate/module:** `tools/scenario-diff/scenario_diff.py`.
- **Related specs:** `tools/state-snapshot.md` (state channel),
  `tools/facts-render.md` (draws channel), `tools/scenario.md`
  (message-injection scenarios, a different run model), `tools/original-hooks.md`
  §5 (starting 1.14d unattended).

## Summary

A **check file** names a character save, a game seed, a tick count and
optional scripted input. `scenario_diff.py <check>` builds the save
with `d2s-tool`, runs 1.14d (under Wine in the cloud, natively on
Windows) with one recorder per channel, runs d2rs headless with the same
save and seed, and prints the first difference of each channel, game
state first. It is the default way to compare a behaviour with 1.14d.

## Inputs

| Name | Type | Source |
|---|---|---|
| check file | text, §2 | `traces/checks/<name>.check` (committed) |
| the install | 1.14d `Game.exe` and data | `D2_GAME_DIR` (private data repo, `tools/cloud-game/fetch.sh`) |

## Outputs / state changes

- Work dir `traces/raw/check-<name>/` (gitignored): the save, each
  side's files (`orig.state.jsonl`, `d2rs.state.jsonl`, frames, draws),
  the Wine run folders.
- The 1.14d save folder gets `<Char>.d2s` (the run uses `-nosave`, so the
  game never writes it back).
- stdout: per channel the comparator's report; a summary line per
  channel. Exit code: the worst channel's (0 match, 1 diverged,
  2 partial, 3 error).

## Rules

### 1. Files

1. `traces/checks/<name>.check`, UTF-8, `<name>` matches `[a-z0-9-]+`
   and equals the `name` line. Inputs only (numbers, names, options);
   never game data.

### 2. Syntax

1. Line based; `#` starts a comment (not on `input` lines, whose script
   is taken verbatim). Blank lines skipped.
2. First line `check 1` (format version; others rejected).
3. Keywords (each once unless noted; unknown keyword, repeat, missing
   required line or bad value: an error naming the line):

<!-- rows -->
| Line | Required | Meaning |
|---|---|---|
| `name <name>` | yes | §1 |
| `save <Char> [options]` | yes | `d2s-tool new --name <Char> [options]`; the same file is the 1.14d save and d2rs' `--save` |
| `seed <1..2^31−1>` | yes | 1.14d `-seed N` (`original-hooks.md` §2 rule 3); d2rs `--seed N` |
| `ticks <n>` | yes | snapshots / ticks recorded on both sides |
| `seconds <n>` | no (300) | 1.14d wall-clock limit per recorder run |
| `difficulty normal\|nightmare\|hell` | no (normal) | d2rs `--difficulty` |
| `channels <ch>...` | no (`state`) | from `state`, `draws`, `rng`, `packets` |
| `draws-at <tick>` | with `draws` | the server tick whose frame is compared (≤ `ticks`) |
| `input orig <script>` | no | `autostart.py` input script (seconds, client pixels) |
| `input d2rs <script>` | no | `d2-client play --input` script (server ticks) |
| `ignore <field>...` | no, repeatable | state fields not compared (`state_diff.py --ignore`) |
| `at <frame> poke <directive> <args...>` | no, repeatable | state injection (`tools/poke.md` §1, §2 rule 6; `spawn` lines as §1 rule 3): `<frame>` ≥ 1 is the absolute game frame (the `f` of `tools/state-snapshot.md`); applied after frame `<frame>` − 1 and its snapshot, before frame `<frame>`'s drain; passed in file order to every 1.14d recorder and to d2rs `state-dump` / `play` as `--poke "<frame> <directive> <args...>"` |

### 3. Run

1. Build the save once (`cargo run --release -p d2s-tool -- new ...`),
   copy it to the 1.14d save folder: `D2_SAVE_DIR`, else on Windows
   `%USERPROFILE%\Saved Games\Diablo II`, else
   `$WINEPREFIX/drive_c/users/$USER/Saved Games/Diablo II`
   (`original-hooks.md` §5.3 rule 2a).
2. Per channel, one 1.14d run: the channel's recorder with `--auto
   <Char> --seed N --ticks <ticks> --seconds <seconds>` and `input
   orig`; under Linux through `tools/cloud-game/run.sh --python`.
3. **state**: `record_state.py --snap-every 1` and `d2-client
   state-dump --save --seed --difficulty --ticks` (`tools/state-snapshot.md`),
   compared by `state_diff.py`. **draws**: `record_frames.py --every 1
   --draws-every 1`, `facts_render.py` on the frame tied to `draws-at`,
   d2rs `play --dump-draws --at-tick`, `d2-client facts-compare
   --ignore tick` (`tools/facts-render.md`).
4. A channel with no d2rs recorder (`rng`, `packets`) is reported as not
   compared (partial), never as a match.
5. Pokes (§2 `at` lines) go to both sides of every channel as
   `--poke "<frame> <directive> <args...>"`. Each side writes one
   `poke` record per result (`{"k":"poke","f","frame","i","d","r",
   "guid"?,"note"?,"src"}`, `tools/poke.md` §3 rule 3 with `f` for `t`)
   into its state file, between the snapshots of frames f − 1 and f;
   `state_diff.py` does not compare them (the snapshots after them
   show the effect).
6. `--orig-only` / `--d2rs-only` run one side; `--reuse` keeps outputs
   already in the work dir; `--dry-run` prints the commands.

## Constants & data dependencies

None beyond the tools named.

## Randomness

None in the tool. Both games run on `seed`.

## Edge cases & original bugs

1. A d2rs side that cannot follow an input line (units differ between
   the two scripts) is the check author's job: the two `input` lines are
   separate on purpose.

## Test vectors

| Input | Expected |
|---|---|
| `--selftest` | the parser accepts the sample and every committed check; each malformed line kind is rejected; a dry run issues the recorder, state-dump and comparator commands with the check's save and seed |

## Provenance

d2rs-own tool; no 1.14d fact.

## Open questions

1. Resolved (2026-10-09, q-tool-poke with q-tool-state-diff): the
   `poke` directives run on both sides as a repeatable `--poke "<frame>
   <directive> <args>"` (`record_state.py`, `record_frames.py` through
   `poke.py`; d2rs `state-dump`, `play`), §2 and §3 rule 5.
2. d2rs `state-dump` takes no input script (`play --input` drives the
   Bevy pointer, which the headless run does not have): a check whose
   behaviour needs C→S input beyond the bridge's own answers (a skill
   cast, a click on a unit) has no d2rs side yet
   (`traces/checks/poke-firebolt.check`).
   Until it does, the state channel runs d2rs without the `input d2rs`
   line and reports at best partial (never a match).
