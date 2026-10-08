# q-amazon: Amazon skills in `play` (`claude/q-amazon`)

Nothing here is verified against 1.14d (rule 10); fills are `d2rs-own, unverified`, spec gaps PROVISIONAL (REC-150, reserved by the task). Synthetic fixtures only. Sound not wired. Reuses the cast path (`q-skills-cast`: `skill_events`, `anim_names`), missiles (`q-missiles-draw`), states (`q-states-auras`) and summons (`q-summons`).

## Links traced and connected

| # | Link | Before | Now |
|---|---|---|---|
| 1 | Bow / crossbow / javelin skills: ammo start check (`srvst 4`), bow missile pick, ammo take (`dec_quantity`) | read the weapon in use, the hands, the hand class and the item's `shoots` / stack facts from `Pending`'s item seams, all defaults (no weapon): every arrow skill refused at its start | `app/weapons.rs` `Weapons` (on `LocalSeams`): a copy of the inventory model's hands and item facts, refreshed by `weapons::sync` before each intent and each tick; `Pending::hand_class` / `composit_weapon_class` / `item_is` / `item_shoots` / `item_stackable` / `item_max_stack` and `UseRest::bow_equipped` / `skill_weapon` / `skill_item_at` answer from it |
| 2 | The sync's read of the inventory | `SimGame` only synced from `&Game` and the sim | new `SimGame::set_world_sync` (d2-server, additive; same call points as `set_host_sync`) so the sync can read the world's `InvParts` |
| 3 | Skills see the weapon, melee does not | — | `UseRest::skill_weapon` / `skill_item_at` (default `None`) are asked first by `UseView` only; `CombatView` keeps `Pending::current_weapon` = none, because the preview's equipped items do not feed the player's damage stats (`equip_rules` is off): a weapon in combat would zero the melee damage |
| 4 | Passives (Critical Strike, Dodge, Avoid, Evade, Penetrate, Pierce): stats from the skill level | `Pending::passive_state_apply` / `passive_refresh` were empty defaults: a learned passive did nothing | `bodies/passive.rs` `refresh` (`client/msg-skills.md` §2 r4, `0x00646D60`): the passive state's list holds `passivestat1…5` = `eval(passivecalc_i)` and the markers 350 / 351; `UseView::passive_state_apply` and `passive_refresh` run it; the skill-point spend (`skills/world.rs` `add_skill_level`) turns the state on and refreshes |
| 5 | Valkyrie (`srvdo 16`) | q-summons wired `create_monster` / `PetAdd` | works unchanged: test only (monster made, listed under its pet type, state 93) |
| 6 | Jab (`srvst 5` / `srvdo 7`) on a monster | cast path | works unchanged: test only |

## Tests

- `d2-sim` `wiring::interaction::tests::amazon` (3): Valkyrie is a listed pet with state 93; a learned passive gives its stat through the state list (fails before); level 0 gives nothing.
- `d2-client/tests/app_amazon.rs` (4, over the real server thread): a bow skill takes an arrow and shoots the missile (fails without the seams); no arrows / bare hands: no start, no mana; Jab on a targetable monster runs the attack mode and ends it.
- `d2-client` `app::weapons::tests` (5): hand class and `shoots` from the item type (bow 1, javelin 3, other weapon 2), stacks, the hands lookup.

## PROVISIONAL / d2rs-own (REC-150)

- Hand class from the item's *type* code (`wclass` is not in the inventory's table projection): `bow` 1, `xbow` 7, `jave` / `ajav` 3, other `weap` 2.
- Weapon in use = the right-hand item with a hand class; arrows are read in the left hand.
- Passive layered stats (`passiveitype` > 0: the masteries) are set on layer 0.
- The player's attack animation is the bare-hand `hth` COF (the client art holds no equipped items), so a bow shot plays the swing; the action frame is the bare-hand one.
- The synthetic Jab test cannot observe damage: the camp's town rule cuts the hit before damage (`combat/damage.rs`).

## Left

1. Client art for the bow / javelin attack (weapon class from the local player's equipped item in `unit_assets::weapon_class` and `anim_names`).
2. `use_state` does not check the weapon a skill requires (`itypea1`): Magic Arrow with a javelin is allowed.
3. `line_clear` is still "blocked" on the seam: skills with `lineofsight` > 0 (e.g. Lightning Fury's ... none for the Amazon bow, but Teleport-like ones) do not start.
4. Javelin throws (Power Strike / Charged Strike / Lightning Strike / Lightning Fury / Poison Javelin: `srvdo 2`, `14`, `17`, ...) are the same cast path; no test of their missiles (`pmissile`, `lob`), and no ammo for thrown javelins is exercised (`item_flag_throw` is still the default).
5. Decoy (`srvdo 15` Dopplezon), Strafe (`srvst 8` / `srvdo 12`), Guided Arrow (`srvdo 10`): bodies exist, not exercised in play.
6. Saved characters: the native passives are not turned on at the load (`adapters/character` rule 8 does not call the refresh).
7. A server-level test of the 0x3B → passive path (needs a states table, which only the sim's tests can build).

## The user's local check (Windows, PowerShell, 1.14d files)

```powershell
git fetch origin claude/q-amazon; git checkout claude/q-amazon
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info,d2_server=info"
cargo run -p d2-client --release -- play --new amazon Test
```

1. Buy a bow and arrows from Charsi (or Akara's stock), put the bow in the right-hand box and the arrows in the left-hand box (inventory, `I`). A new Amazon needs experience for a skill point: kill a few monsters, open the skill tree and put the point into Magic Arrow (or Jab for the javelin the Amazon starts with).
2. Select Magic Arrow on the right button, walk out of the camp, right-click the ground: the arrow count in the left hand drops by 1 per shot, a missile flies (q-missiles-draw), mana is spent. With no arrows or no bow nothing happens.
3. Select Jab, right-click a monster: the attack mode plays and ends; the monster loses life.
4. Put a point into Critical Strike / Dodge: the character panel's stats do not show them; the effect is in `RUST_LOG=d2_server=debug` damage rolls (crit rolls draw a number).
5. Valkyrie needs skill level 30 (cheat the level with a save); the pet appears at the aimed point and follows.

Copy any `Anim(NoRecord)`, `ammo` or `Pet(` error line from the log into `docs/HANDOFF.md` under REC-150. Headless: `cargo nextest run -p d2-client --test app_amazon` and `cargo nextest run -p d2-sim amazon`.
