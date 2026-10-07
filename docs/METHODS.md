# Methods

The working methods of this project, written to carry over to other
projects. Each entry states the method in general terms; its **Here** line
binds it to this project. `CLAUDE.md` holds the hard rules and project
facts; it points here for how work is done. Every session follows every
method below.

Format (checked by `py tools/methods.py check`, run in CI): `## Mnn Title`,
then exactly the lines Rule, Why, Check, Here, Status, in that order.
Status is `proven — <evidence>` (shown to work in this project) or
`trial — <what would prove it>` (adopted, not yet shown). Add a method
with `py tools/methods.py new "Title"`; export the general form (no Here
lines) with `py tools/methods.py export <file>`. Change an entry only with
evidence; a trial that fails is removed or rewritten, not kept.

## M01 Ground truth decides
- Rule: Every behavior taken from a reference is held to exact match under a comparison the spec defines for its area (bytes, pixels, decoded samples, ticks). There is no "close enough" tier.
- Why: no reliable boundary between "must be exact" and "may be approximate" exists, and one slipped area costs more debugging than loosening saves.
- Check: each feature names its comparison and the reference it runs against; a passing comparison is the only proof of done.
- Here: the reference is the original 1.14d (`CLAUDE.md` hard rule 10): live `.bin` data, traces from `tools/trace-recorder`, CPU-reference renders.
- Status: proven — 73 tables byte-identical from text (`data-tool tables`), 32,543 recorded RNG draws matched, GPU render byte-identical to the CPU reference.

## M02 Unverified is not done
- Rule: A feature whose check cannot run yet is marked unverified and its check is queued with the exact command and the expected result; nobody judges it done by eye.
- Why: an unchecked "done" is where errors enter silently and spread.
- Check: every unverified item has a queue entry; queue entries are removed only with a recorded result.
- Here: the local run queue in `docs/HANDOFF.md` §5; spec statuses in `specs/`.
- Status: proven — table callbacks and fix-ups written in the cloud, confirmed through the queue on game files (2026-10-05).

## M03 Specs are the memory
- Rule: Behavior lives in specs: confirmed facts with provenance, a status, open questions, one owner per fact. Code implements a spec and names it; transcripts are never the record.
- Why: sessions forget; a fact stated in two places drifts.
- Check: every module names its spec; every rule has one owner spec; unknowns are open questions, not guesses.
- Here: `specs/` (bar and template in `specs/README.md`, `specs/_TEMPLATE.md`); module header `// Spec: specs/...`.
- Status: proven — every Phase 0–2 module built from specs; callbacks reproduced 17,355 calls from their spec.

## M04 Clean-room separation
- Rule: Sessions that read reference implementations or decompiled code write prose, tables and test vectors only; implementation sessions read specs only.
- Why: keeps licenses clean and forces every fact through a checkable statement instead of copied code.
- Check: implementation inputs are limited to specs, docs and the codebase; specs contain no pasted code.
- Here: `CLAUDE.md` hard rules 2–4 (`re/`, `../refs/` for spec sessions only).
- Status: proven — all crates written from specs.

## M05 Turn facts into checks
- Rule: A list or table that code consumes is kept as machine-readable data with a mechanical check, not as prose.
- Why: a failing check is cheaper and surer than rereading prose.
- Check: each such list has a data file and a command or test that fails when the data and the code disagree.
- Here: `specs/data/fields.tsv` + `tables.tsv` with `data-tool tables`; generated code with its staleness test.
- Status: proven — 3,591 field entries drive the compiler, the cross-check and the generated structs.

## M06 Evidence over reviewers
- Rule: One focused writer per spec or module, then an executable check. No stacked review, revise and critic layers. Agent prompts are tight, with exact file and section pointers.
- Why: more reviewers add cost and correlated opinions, not proof.
- Check: quality claims point to a check result, not to a review.
- Here: `data-tool tables`, `mpq-tool formats`, `d2-client verify`, traces.
- Status: proven — a 24-agent review workflow cost about 6M tokens for little gain; single writers plus checks did better.

## M07 Stop errors where they are made
- Rule: Check every artifact at the boundary where it is produced or extended, before anything consumes it. Inputs are strict; errors are loud; nothing falls back silently.
- Why: if each error causes fewer than one new error before it is caught, errors die out at a cost that grows with the work, not with its square; global checking after the fact does not scale.
- Check: each producer (spec edit, generator, compiler, loader) has its own check; unknown input is an error, not a default.
- Here: strict parsers (`txt-format.md` §9), spec index check and generated-file check in CI, load checks in `d2-data::bin`.
- Status: proven — strict format and table parsers surfaced every 1.14d quirk as a spec rule instead of a hidden fallback.

## M08 Prove the check can fail
- Rule: For each check, change an input on purpose and confirm the check reports exactly that change.
- Why: a check that cannot fail proves nothing.
- Check: each acceptance check has a perturbation test.
- Here: `crosscheck_catches_perturbations`, `d2-client verify --perturb N`.
- Status: proven — both report exactly the perturbed bytes or pixels.

## M09 Say where a claim holds
- Rule: A fact passed between sessions, agents or documents names its scope: branch or commit, version, the data it was checked on. "Exists" means "exists on the main branch" unless stated.
- Why: a fact true in one place and assumed everywhere is an error crossing a boundary unnoticed.
- Check: handoffs and prompts state scope; a receiver treats an unscoped claim as unverified.
- Here: branch names in prompts and HANDOFF; 1.14d and archive (P/X/D) in specs.
- Status: trial — adopted 2026-10-05 after a prompt claimed typed structs existed that were only on an unmerged branch; proven when handoffs stop producing such leaks.

## M10 Proven methods before new ones
- Rule: Solve a problem with a project convention, an existing tool or an established method first. Invent only when none fits; then say so and give the new piece its own check before anything builds on it.
- Why: an improvised solution is an untested component added where checks are thinnest.
- Check: commits and specs that introduce a new method say so and name its check.
- Here: tools in `tools/`, the methods in this file, debugger traces, Ghidra exports.
- Status: trial — adopted 2026-10-05.

## M11 Read by section
- Rule: Large documents start with a section index with line ranges; readers read the index, then only the sections they need, and prompts name sections.
- Why: context is a finite channel; whole-file reads waste it.
- Check: a tool regenerates indexes and CI fails on a stale one.
- Here: specs over 12 KB; `py tools/spec_index.py` after every spec edit (`--check` in CI).
- Status: proven — CI caught a stale index on `main` (2026-10-05).

## M12 Start from the map
- Rule: One handoff file holds state, next steps in order, a code map and the command that proves each thing; sessions start there instead of exploring.
- Why: exploration costs tokens and rediscovers what is known.
- Check: every session ends by updating the handoff file.
- Here: `docs/HANDOFF.md`, plus the checklist and decisions log in `docs/PLAN.md`.
- Status: proven — fresh cloud sessions completed tasks from it alone.

## M13 One step per session
- Rule: A session does one step: start from the handoff and the latest commit, end by updating the handoff, committing and pushing. Start fresh rather than continue a long thread.
- Why: long threads cost more every turn and carry stale context.
- Check: each step ends in a commit that updates the handoff.
- Here: `CLAUDE.md` "How to work a task".
- Status: proven — RNG and cross-reference steps each finished in one short session.

## M14 Match the agent to the task
- Rule: Pick model and effort by task class; run parallel agents only on independent work with separate files.
- Why: parallel agents buy speed, not savings; overpowered agents waste budget, underpowered ones make errors.
- Check: launches state the task class and why they are independent.
- Here: RE, spec writing, exactness debugging, architecture: Opus, high. Implementation from a clear spec, extraction scripts, tools: Opus or Sonnet, medium. Docs cleanup, formatting, boilerplate, renames, simple fixes: Sonnet or Haiku, low.
- Status: proven — medium-effort implementation sessions finished specced work first time.

## M15 Budget the spend
- Rule: Plan token spend per usage window against a cap, a target and a floor, state it before launching a batch, spend on the critical path first, and raise effort only where the task class needs it. Reach the floor with more independent useful work running in parallel, never with padding (review layers, repeated runs, higher effort than the task class needs).
- Why: spend grows quietly, and unused budget is lost speed; a stated plan makes both overruns and idle capacity visible.
- Check: each batch launch states its planned spend.
- Here: user's plan (2026-10-06, raised twice the same day): cap 500M tokens per 5-hour window (was 120M, before that 24M), shared by the local and the cloud coordinator; aim for a median of 80% (about 400M) across windows, never under a floor of 60% (about 300M) when enough independent work exists; if it does not, say so instead of padding. Cloud work is bounded by written specs (spec writing is local), so the cap is reached with many parallel cloud implementation, test and tooling sessions beside the local spec sessions. A focused run costs about 250–500k; a long main thread costs more every turn. High effort only for RE and spec writing, an exactness mismatch the first fix did not solve, architecture, and a spec a whole phase builds on.
- Status: proven — measured costs match the plan.

## M16 Split work by access to ground truth
- Rule: Work that needs the reference runs where the reference is; elsewhere, work proceeds on a branch and queues its checks (M02).
- Why: environments without the reference cannot prove fidelity, but can do everything else.
- Check: work that needs the reference is never marked done where the reference is absent.
- Here: cloud has the repo only; local has `game/`, `re/`, `../refs/` (`CLAUDE.md` "Where work runs").
- Status: proven — cloud implementation plus local confirmation (2026-10-05).

## M17 Generated code is committed and checked
- Rule: Code derived from data is generated by a tool, committed, never edited by hand, and guarded by a test that regenerating gives the same output.
- Why: the data stays the single source; drift between data and code fails a test.
- Check: the staleness test runs in CI.
- Here: `data-tool gen-tables` → `crates/d2-data/src/tables/generated.rs`; test `generated_file_is_current`.
- Status: trial — adopted 2026-10-05; proven once schema changes flow through it.

## M18 Frameworks at the edge
- Rule: Core logic is pure and deterministic and never lives inside a framework; frameworks are confined to the outer layer; dependency rules are checked mechanically.
- Why: the core stays testable and portable; framework upgrades touch one layer.
- Check: a dependency checker runs in CI.
- Here: Bevy only in `d2-client` (`CLAUDE.md` hard rules 5–7); `cargo run -p depcheck`.
- Status: proven — depcheck green; data and formats crates are framework-free.

## M19 Variants beside the reference
- Rule: Changes to reference behavior are data layers or explicit switches beside the reference code path, never edits of it.
- Why: the reference stays checkable while variants grow; each change is visible as a layer.
- Check: reference conformance tests run with no variant enabled.
- Here: `Ruleset::Original` vs `Ruleset::Mod`, patch layers (`CLAUDE.md` hard rules 8–9, `specs/data/patch-layers.md`).
- Status: trial — proven once the first patch layer applies with all conformance checks still passing.

## M20 Version every persisted format
- Rule: Every format written to disk or the wire carries a version number from its first commit.
- Why: unversioned data cannot evolve without breaking old files silently.
- Check: format specs state the version field.
- Here: saves, protocol, traces (`traces/FORMAT.md`), caches.
- Status: proven — traces carry their format version.

## M21 Log escapes
- Rule: When an error escapes a check, record where it was made, where it was caught, and the check that would have caught it at its source; then add that check.
- Why: the distance from creation to catch shows which boundaries lack a local check.
- Check: each escape has a log line and a new or changed check.
- Here: the "Lessons" table in `docs/HANDOFF.md` §8.
- Status: trial — adopted 2026-10-05.

## M22 Provisional over blocked
- Rule: When a behavior can only be settled by a capture or recording, do not leave the code blocked: implement the most plausible reading of the evidence already in the spec, mark it provisional in spec and code, and name the capture that settles it. Captures of choices that spread silently (RNG draw order, persisted or wire byte layouts) go to the top of the recording list.
- Why: a blocked seam leaves holes that stop the build from being played and tested; the capture comparison runs the same whether the code guessed or not, and in this project the misses found by testing have been small, local edits.
- Check: every provisional point is greppable and names its settling capture; it counts as unverified (M02), never done.
- Here: spec line `PROVISIONAL: <chosen behavior> (because …); settled by <capture / recording id>`; code comment `// PROVISIONAL (<spec ref>)` on a plain implementation (no blocking seam); the cloud implementation session makes the choice while coding, no PC research; `grep -rn PROVISIONAL specs crates` is the testing-phase checklist; the settling captures are in the HANDOFF §7 recording list.
- Status: trial — adopted 2026-10-07 by the user's decision; proven when the testing phase settles the provisional points with only local edits.
