# rc-c011-gameseed: C011 "state game seed" (causes-99.tsv)

Branch `claude/rc-c011-gameseed` (from `claude/integ-r23`). REC ids
REC-3330..3349 (none used). Ledger part `docs/handoff/ledger/rc-c011-gameseed.tsv`.
EQUAL 3116 → 3210 (+94; 0 rows went EQUAL → DIVERGED). Sweeps (all checked
against the ledger / checks-status, 0 worse): gen-mon-* 337, gen-boss/su/ai 237,
the 194 class-skill checks, a5-su-*, dru/ass/milestone C011 checks.

## Fixes (draw site in 1.14d → d2rs gap)

| 1.14d | d2rs gap → fix | Moved |
|---|---|---|
| `0x005A03A0` superunique TC (`treasure.md` §3.2) | seam always none → monster data flag 0x02 / +0x26 | a5-su-* 16 → 36 |
| `0x005A4390` mode 3 from a umod area hit | world out → re-lent (`umod_missile_hit`) | anodized-elite |
| `0x006439F0` monster entry, Armageddon state fn | no list → init/summon entries | gen-boss-333 71 → 95, dru-armageddon |
| `0x005A43B0` mode 5 (multishot §19) in creation | store and world out → both lent back | gen-boss-707 EQUAL |
| `0x00553540`, `0x0056D2C0`, path target point | umod host seams always none → path provider | multishot / curse targets |
| `0x00463740`, `0x005B2F20` in the AI | seams none → DRLG room + `spawn_at` | maggot queen 43 → 74 |
| A1Q4 `0x005944B0` / `0x005943B0` / `0x00594450` / `0x005944F0` | not wired → quest loan; specced `quests-act1-rest.md` §3 | gen-ai-npcoutoftown EQUAL |
| `0x00571CD0` in the player update (`intents-events.md` §7.3 r1 step 4) + `MsgA3` | player records never sent; MsgA3 dropped | dru-volcano, dru-armageddon packets MATCH |

## Open (with sizes)

- a5-su-* (16 checks, M): frame 36 barbarian 1:15 hits mauler 1:18, 1.14d 1:19:
  the `ai.scan5.good-target` row (owner rc-ancient-tx).
- gen-su-26/27/28/30/31 (M): hydra bolts (missile 247) deal fire 6377 per hit and
  two hit in d2rs (player dies frame 68); 1.14d one hit of 2176 — missile damage.
- gen-ai-sandmaggotqueen, gen-su-15 (S?): frame 74 the spawned sandmaggot 1:9 seed.
- gen-boss-333 frame 95, gen-ai-mephisto/navi/nihlathak/genericspawner, gen-boss-242,
  gen-su-6/10/60: monster unit seed (AI draw order), not traced.
- gen-lvl-55/69/104/106 (level population at the warp, frame 21): level-pop owners.
- ass-blade-sentinel, milestone-izual: frame 28 game seed, not traced.
- Recorder: `record_rng.py --skip-inline` range 0x66B000–0x682000 ("DRLG only")
  also covers the path code (charged-bolt path `0x0067A240`), so 1.14d's path
  draws are never recorded and the rng channel reports d2rs's as extra.
- Packets seen DIVERGED with no status line (pre-existing): ass sentries miss the
  monster-add 0x21, sor-hydra 0xA7/0xAC order, dru-fissure missing 0xA7.
