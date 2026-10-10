# rc-pc1late-npc hand-back

Rows: q-fix-pc1late-walk-radius, q-fix-pc1late-gamble-place (removed from build-queue.tsv).

Checks (items-vendor-gheed*): 74/88 equal (2 MATCH, stock-goto DIVERGED) -> 88/88 (3 MATCH).
town-ama-10k replay-diff (monsters, frames 270-300): no divergence; Warriv ty 4228 at 287.
items-vendor*: 21 checks, 19 MATCH, 466/469 ticks; akara-buy packets DIVERGED at frame 20
s2c 0x9C byte 37 (161 vs 160); earlier it diverged at frame 16 (q-fix-pc1-proto-items), not this change.
cargo nextest -p d2-sim -p d2-server: 5134 pass.

Changed
- d2-sim monsters/ai/tactics.rs: radius_point(u, size, t, a, b) per ai.md §7.2 (0x005DE4E0); wiring/action/ai.rs passes the size.
- Gamble list items: InvState.gamble holds one Inventory per (NPC, player GUID) node; InvDesk::gamble_place/gamble_unlink swap it in for the NPC's own inventory around place/store_unlink; wired through NpcInventory, vendor_world.rs, d2-server vendor_inv.rs. The grid is separate from the store's (stock collisions had put the ring at y 2).

Open
- d2-client rest.rs / test fakes still return true for place_in_gamble (preview has no NPC grid model), S.
- coverage, spec_index, ledger --check pass.
