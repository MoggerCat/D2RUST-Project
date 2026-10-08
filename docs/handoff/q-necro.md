# q-necro: Necromancer skills in play (`claude/q-necro`)

Nothing here is verified against 1.14d (rule 10). Synthetic fixtures only; sound not wired. PROVISIONAL: REC-151.

## Result

The cast path (q-skills-cast), missiles (q-missiles-draw), states (q-states-auras) and summons (q-summons) already carry the Necromancer bodies. `crates/d2-client/tests/app_necro.rs` casts, through C→S 0x0C on the play host:

| Skill | Body | Checked |
|---|---|---|
| Teeth | srvdo 8 | mana spent, server missiles, 0x4D to the client |
| Poison Nova | srvdo 22 | mana spent, a ring of missiles (> 1) |
| Bone Armor | srvdo 18 | state on the player, 0xA8 sent |
| Clay Golem | srvdo 56 | a monster exists, 0x7A sent |

No production code changed; the only missing pieces were test fixtures (a synthetic states table, `BodyTables`, formula code).

## Left

- Curses (Amplify Damage ... srvdo 30): need a hostile monster in range in the client harness (the sim-level body tests exist).
- Corpse Explosion, Raise Skeleton / Skeletal Mage, Revive: need a corpse unit as the target (0x0D on a corpse).
- Bone Spear / Wall / Prison, Golem equipment, Skeleton mastery stats.
- Live skill rows: calc formulas and states come from real `skills.txt` / `states.txt` only on the local run.

## Local check (Windows, 1.14d files)

```powershell
git fetch origin claude/q-necro; git checkout claude/q-necro
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
cargo run -p d2-client --release -- play --new necromancer Test
```
Leave town. Right-click with Teeth (missiles fly), Bone Armor (state overlay on you, mana drops), Poison Nova (a ring), Clay Golem (a pet appears and follows). Report any `NoRecord` / `Pet(` error in the log.

Headless: `cargo test -p d2-client --test app_necro`.

Local check done 2026-10-08 (PC 1 round 2, branch claude/local-pc1-s8): headless part passes: `app_necro` 4 pass; the Teeth / Bone Armor / Poison Nova / Clay Golem play check needs a player.
