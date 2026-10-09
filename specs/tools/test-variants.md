# Spec: Tools — Test variants (a patched copy of the install for one check)

- **Status:** draft: format and build are ours; the 1.14d load of a
  variant is not yet run (REC-591).
- **Target version:** 1.14d (the install the variant is built from).
- **Crate/module:** `data-tool variant` (`tools/data-tool/src/variant.rs`)
- **Related specs:** `data/patch-layers.md` (the `d2stack` / `d2patch`
  format, apply, compile), `data/loading.md` §2 (archive search order:
  `patch_d2.mpq` first for excel files), `formats/mpq.md` (archive
  layout, hash and block tables, encryption keys), `tools/poke.md`
  (state a variant cannot set), `CLAUDE.md` rules 1 and 9.

## Summary

Some checks need data the shipped tables never give: a level whose only
monster is the one under test, a skill with a fixed damage range, a
treasure class with one entry. A **test variant** is a patch stack
(`data/patch-layers.md`) committed in the public repo; `data-tool
variant build` applies it to the user's own tables, compiles the patched
`.bin` files with the verified compiler and writes a **variant install**:
a copy of the install whose `patch_d2.mpq` also holds those `.bin`
files. 1.14d and d2rs both load that install unchanged, so a comparison
on it stays exact. Variant installs are game files: they live only in
the private data area or a local folder, never in the public repo.

## Inputs

| Name | Type | Source |
|---|---|---|
| variant | `traces/variants/<name>/<name>.d2stack` and its layers | public repo (our patches only, rule 9) |
| base install | the 1.14d install | `D2_GAME_DIR` / `--game` |

## Outputs / state changes

`<out>/` (default `$D2_GAME_DIR/../variants/<name>/`; refused inside the
repository work tree): every file of the base install's top folder (hard
link, else copy), except `patch_d2.mpq`, which is rewritten (§2), and
`variant.json` (§3).

## Rules

### 1. Variant files

1. `traces/variants/<name>/`: `<name>.d2stack` and the `.d2patch`
   layers it names (`patch-layers.md` §3). `<name>` matches
   `[a-z0-9-]+`. Layers hold `set` / `add` / `check` statements only:
   never a full table (rule 9).
2. A layer may patch any table that `patch-layers.md` §2 lists.

### 2. Build

1. Apply the stack to the base tables and compile (`patch-layers.md`
   §5, §7). Any error finding stops the build (exit 1).
2. The patched set is the tables whose compiled `.bin` differs from the
   live `.bin` the base install loads (`loading.md` §2: P → X → D).
3. `patch_d2.mpq` of the variant is the base `patch_d2.mpq` with one
   change per patched table `data\global\excel\<table>.bin`:
   - the file's data is appended after the last byte the base archive
     uses, stored (no compression, no encryption, single unit off,
     sectored as the archive's sector size requires; `mpq.md` §8);
   - its block entry is replaced when the name has a hash entry, else a
     new block entry is appended and the name takes the first free
     hash slot of its probe (`mpq.md` §5);
   - the hash and block tables are written again after the data,
     encrypted with their fixed keys (`mpq.md` §4), and the header's
     table offsets, block count and archive size are updated.
   Every other byte of the base archive is kept, so no file name list
   is needed (`patch_d2.mpq` has no `(listfile)`, `mpq.md`
   Observations).
4. **Check** (the build fails on any difference): every patched name
   read back from the variant archive equals its compiled `.bin`; every
   other file of the base archive (by block index) reads back
   byte-identical; reading the excel tables of the variant install
   with `d2-data` gives the patched tables.

### 3. `variant.json`

`{"format": "test-variant", "version": 1, "name", "stack_sha256":
[{"file", "sha256"}] (the stack file first, then each layer in order),
"base_patch_d2_sha256", "variant_patch_d2_sha256", "tables": [{"table"
(the `.bin` name without `.bin`), "bin_sha256", "source":
"patch_d2"|"d2exp"|"d2data" (where the live `.bin` came from)}],
"tool"}`. `data-tool variant check <out> [--game DIR]` repeats the §2
rule 4 check from it (the base archive is needed for the other blocks). It is a description of game files and stays beside
them (never committed).

### 4. Use

1. d2rs: `D2_GAME_DIR=<out>` (scenario-run, d2-client).
2. 1.14d: `tools/cloud-game/run.sh --exe <out>/Game.exe`, or a recorder's
   `--game <out>/Game.exe` (the reference hash check is on `Game.exe`,
   which is unchanged).
3. A scenario names its variant with the header line `variant <name>`
   (`tools/scenario.md`); the runners refuse an install whose
   `variant.json` names another variant or none.

## Constants & data dependencies

Excel path `data\global\excel\<table>.bin` (`loading.md` §3.1).

## Randomness

None.

## Edge cases & original bugs

1. A patch that leaves every compiled `.bin` equal to the live one
   builds a variant identical to the base (no table patched); the build
   says so and still writes it.
2. A table whose `.bin` the 1.14d loader does not read (compile-only
   tables, `loading.md` §7.2) cannot be varied this way.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| synthetic install, one `set` on `monstats` | variant `patch_d2.mpq` reads the new `monstats.bin`; every other block byte-identical | `data-tool` tests |
| same, the hash slot taken by a new name | found by the §5 probe | synthetic |
| empty stack | variant equal to base, note printed | synthetic |
| real install, `traces/variants/only-fallen` | 1.14d and d2rs load it; `dump_tables.py` + `data-tool dump-compare` on the variant match | REC-591 (not run yet) |

## Provenance

d2rs-own tool. Archive facts from `formats/mpq.md`; load order from
`data/loading.md`.

## Open questions

1. Does 1.14d's archive layer accept a block appended after the base
   archive's end with the header's archive size updated (`mpq.md` §1)?
   Settled by REC-591.
