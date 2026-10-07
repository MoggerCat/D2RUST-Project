# c2-monsters coverage handoff

Scope: specs/monsters/*.md, specs/missiles/*.md. Time-boxed session
(deadline 21:16 UTC); the build of d2-sim from cold took most of it.

## Counts

Uncovered before: 12 spec files with gaps, ~50 rules. Claimed in this
session: `umod-init-bodies.md` §4 r2, §4 r3 (new test
`teleport_assigns_skill_184_level_1_and_mode_4` in
`crates/d2-sim/src/monsters/init/tests_c2mon.rs`, registered with one
`#[path]` line in `init/tests.rs`). No code fixes.

## Exempt candidates

specs/monsters/umod-init-bodies.md	§2 r2	game-null guard: the Rust host always has a game; unreachable
specs/monsters/umod-callbacks.md	§1 r3	conventions of another spec (stat/unit helper addresses)
specs/monsters/umod-callbacks.md	§1 r5	conventions: dead/owner/minion-owner helper definitions
specs/monsters/umod-callbacks.md	§2 text	call-site list of the dispatcher in engine order (pointers to other specs)
specs/monsters/umod-callbacks.md	§2 r5	call-site in combat reaction; owned by combat/damage.md
specs/monsters/umod-callbacks.md	§28 text	intro: client hook table lives in d2-client (Phase 6)
specs/monsters/umod-callbacks.md	§28.1 r3	client wrapper/call-site table (client-only)
specs/monsters/umod-callbacks.md	§28.1 r4	client conventions
specs/monsters/umod-callbacks.md	§28.3 r1	client burst hook (client-only, no sim code)
specs/monsters/umod-callbacks.md	§28.3 r2	client ring hook (client-only, no sim code)
specs/monsters/umod-callbacks.md	§28.4	reachability note (umod 40 on clients), not behaviour
specs/monsters/umod-callbacks.md	§edge-cases-original-bugs r12	client hook differences (client-only)
specs/monsters/umod-callbacks.md	§edge-cases-original-bugs r13	client frame repeat (client-only)
specs/monsters/umod-callbacks.md	§edge-cases-original-bugs r14	client dispatcher list walk (client-only)
specs/monsters/init.md	§1	entry-point table (callers of the creation function)
specs/monsters/init.md	§4.1 text	allocator intro prose
specs/monsters/init.md	§4.1 r1	steps owned by sim/path-placement.md and sim/units.md
specs/monsters/init.md	§4.1 r2	pointer to sim/unit-order.md
specs/monsters/init.md	§4.1 r3	pointer to sim/units.md
specs/monsters/init.md	§25.1	calling-convention table of tool calls
specs/monsters/init.md	§25.2 r1	narration of which seeds draw (checked by the draw-order tests)
specs/monsters/init.md	§25.2 r3	state NOT touched by a tool call (negative note)
specs/monsters/init.md	§edge-cases-original-bugs r13	original use-after-free; d2rs deliberately clears it (Open question 11)
specs/monsters/population.md	§1 r2	caller list of 0x0052D0F0 (other specs)
specs/monsters/population.md	§1 r3	recording-confirmed ordering: trace check only
specs/monsters/population.md	§14 r2	"no column triggers a spawn on death" (negative note)
specs/monsters/population.md	§14 r3	pointer to AI/skills callers
specs/monsters/population.md	§14 r4	"no minion respawn in population code" (negative note)
specs/monsters/population.md	§14 r5	pointer: MonSpcWalk read by AI only
specs/monsters/population.md	§14 r6	pointer to missiles spec
specs/monsters/ai.md	§5.3	table of helper addresses (pointers)
specs/monsters/ai.md	§2.1 r4	fatal assert on bad code pointer; no d2rs counterpart (see comment in tests/rules.rs)
specs/missiles/missiles.md	§r2-4-client-message	0x73 message builder: server-messages / client spec
specs/missiles/missiles.md	§edge-cases-original-bugs r10	crash-if-used rows; no behaviour

## Rules left (need real tests, not done for lack of time)

- specs/monsters/ai-bodies-7.md §27 r3, r5, r7–r11 (ShadowMaster think:
  code exists in `ai/bodies7.rs`; needs a scan-callback harness like
  `tests/act7.rs`).
- specs/monsters/ai.md §2.1 r4 / §3.3 text / §5.1 text / edge r13
  (BloodRaven halved distance: code exists, needs a test).
- specs/monsters/ai-bodies.md §9.12 text (Andariel), ai-bodies-3 edge r6,
  ai-bodies-4 edge r4 (each: code exists, claim + short test).
- specs/missiles/bodies-2.md edge r2 (server-hit 21 state request with
  stat 0 value 0: not modelled by the `apply_state` seam),
  bodies.md edge r6.
