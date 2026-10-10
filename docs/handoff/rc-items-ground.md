# rc-items-ground hand-back (2026-10-10)

Branch claude/rc-items-ground = staging-7 + integ-r16 merge.

## Checks (items area, `suite.py --filter 'items-*' --no-playthrough`)
Before (r16): 87 checks, 959/1127 ticks equal (85.1%), MATCH 44 / DIVERGED 28 / PARTIAL 17.
After: items-vendor-akara-buy state 19/40 -> 40/40. Ground rows
(items-ground-many 30/30, items-ground-pokes 40/40, items-load-mixed,
items-pickup-ama) were already equal on r16.

## For the coordinator
Ground rows (item.gen.pipeline, drop.placement, inv.cursor, item.identify) are
EQUAL on r16 for items-ground-many/-pokes (state channel PARTIAL = equal where
compared); the status file still says DIVERGED@4, so please re-triage them.

## Changed
- `d2-client state-dump`: store items (vendor flag, unit +0xC8 bit 2)
  report `own` absent, as 1.14d does (recorded in akara-buy: 41 store
  items have no owner, the bought copy has the player GUID).
  Spec: world/vendors.md §4 note. Ledger part: ledger/rc-items-ground.tsv.

## Open (not mine, sizes)
- items-drops-{cha,hel,nig,rbo,uni}-* x diffs of 1-2 sub-tiles (6 checks):
  not placement. floor drop (0x00555DA0/0x0064E810/0x0064DEA0) re-read and
  matches the Rust. Cause: monster positions at death differ (hel-04 state
  channel: class 66 at x 5146 on 1.14d, 5147 in d2rs at frame 200; other
  monsters move earlier in 1.14d). Owner: monster AI/movement. M.
- items-drops-* `frame` diffs (e.g. rbo-02 35 vs 43, uni-05 53 vs 92): kill
  timing, same family as rc-drop-nor-timing. M.
- items-drops-* "missing" items (hel-05/06, nig-07/08, nor-10): drop gate / TC
  walk; first rows need a per-case look. M.
- items-vendor-akara-buy packets: frame 4 s2c 0x15 (player reassign) vs 0x07;
  orig startup burst is at frame 1, d2rs at 2. Startup framing, S.
- tmp state check of hel-04 diverges at frame 44 (missile class 26 vs 22).
