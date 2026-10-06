# Handoff: spec-complete AI / server-do / server-hit bodies — `claude/spec-bodies-ai-missiles`

Cloud implementation session, 2026-10-06, task class: implementation
from clear specs, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `9b49081`. Repo only, synthetic data, no game files (M09). For the
coordinator to fold into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is
edited here).

## 1. Result of the audit

Every row of `specs/monsters/ai-functions.tsv` (148),
`specs/missiles/srvdo.tsv` (53) and `specs/missiles/srvhit.tsv` (71) was
checked against `specs/monsters/ai.md` §3, §9, §10 and
`specs/missiles/missiles.md` §R4, §R9, plus a search of every other spec
for the function addresses (only `rng.md` §5.3 / §6 and `units.md` §6
name some of them, as draw sites or REMOVESTATE sites, with no
behaviour).

**No new body is implementable exactly.** The spec text gives full
rules (all branches, draws in order, constants, 1.14d-read) only for
the rows already implemented on the base:

- AI (17, `spec'd-here`): None 0, Idle 1, Skeleton 2, Zombie 3, Fallen 6,
  Brute 7, Wraith 9, Goatman 12, FallenShaman 13, QuillRat 14, Swarm 19,
  Andariel 34, CorruptArcher 35, CorruptLancer 36, Navi 58, TownRogue 62,
  Buffy 100 (`monsters/ai/functions.rs`). Re-read against §9.2–§9.13:
  no divergence found.
- Server-do: 1 (default flight, §R4) and the helper `0x005A9820`
  (§R9.3) that 8, 10, 17, 25 call. Null entries 0, 4, 38–52 log.
- Server-hit: none (every non-null row is `D2MOO-only` or `summarized`).

Every other row is `summarized`, `D2MOO-only` or `unread`: its behaviour
is D2MOO 1.10f's or absent, and `CLAUDE.md` (pinned versions) requires
it confirmed against 1.14d before it goes in a spec, so implementing it
would invent behaviour. Stubs unchanged (log `Unhandled`, §4 of
`impl-monsters.md`).

## 2. Code change

- New check `missiles::tests_bodies` (`crates/d2-sim/src/missiles/tests_bodies.rs`,
  `#[cfg(test)] mod tests_bodies;` in `missiles/mod.rs`): the server-do /
  server-hit bodies (`catalogue::SRV_DO_IMPLEMENTED`,
  `SRV_HIT_IMPLEMENTED`) equal the non-null `spec'd-here` rows of the
  TSVs, the missile counterpart of the AI's `implemented_matches_catalogue`
  (which existed). M08: promoting server-do 2 in the TSV text, or claiming
  a server-hit body, is reported exactly
  (`bodies_check_catches_perturbations`). Claim: `missiles.md §R9.2`.
- No other source change.

## 3. Missing spec, per entry (for the local spec coordinator)

What every entry below needs to become implementable is the same bar as
`ai.md` §9.3–§9.13 / `missiles.md` §R4: the 1.14d body read as a numbered
step list with every branch, every draw in order and on which seed,
every constant and column/skill-calc read, the seam calls with their
arguments, and (missiles) the return value. Then set the row's `status`
to `spec'd-here`; `implemented_matches_catalogue` /
`bodies_match_catalogue_status` fail until the body lands.

### 3.1 AI, `summarized` (1)

- 32 Npc (`0x005E7130`): only the top-level order is 1.14d; missing the
  bodies of `0x005E6800` (home command), the class cases 0xC9, 0xFE,
  0xFF, 0x109, 0x200 (quest movers), `0x005E68F0` (interaction / greet /
  walk-around), `0x005E6AE0` (command 4/5/7 path actions, idles 8–50),
  `0x005E7080` (map-AI node actions: the node record and each action),
  and the source of idle 10 (`ai.md` open question 8).

### 3.2 AI, `D2MOO-only` (11; summary in the TSV, 1.14d not compared)

- 4 Bighead (`0x005EFF50`): 1.14d body not compared; only the D2MOO one-line summary.
- 5 BloodHawk (`0x005F00E0`): 1.14d body not compared; only the D2MOO one-line summary.
- 8 SandRaider (`0x005F0700`): 1.14d body not compared; only the D2MOO one-line summary.
- 10 CorruptRogue (`0x005F0B00`): 1.14d body not compared; only the D2MOO one-line summary.
- 11 Baboon (`0x005F0CD0`): 1.14d body not compared; only the D2MOO one-line summary.
- 15 SandMaggot (`0x005F1800`, alt `0x005F1750`): 1.14d body not compared; only the D2MOO one-line summary.
- 20 Scarab (`0x005F2540`): 1.14d body not compared; only the D2MOO one-line summary.
- 33 HellMeteor (`0x005F56D0`): 1.14d body not compared; only the D2MOO one-line summary.
- 37 SkeletonBow (`0x005F6070`): 1.14d body not compared; only the D2MOO one-line summary.
- 43 FoulCrowNest (`0x005F6650`, init `0x005F6630`): 1.14d body not compared; only the D2MOO one-line summary.
- 59 BloodRaven (`0x005E6320`, init `0x005E6300`): 1.14d body not compared; only the D2MOO one-line summary.

### 3.3 AI, `unread` (119)

- 16 ClawViper (`0x005F1B60`): unread; no behaviour stated.
- 17 SandLeaper (`0x005F20A0`): unread; no behaviour stated.
- 18 PantherWoman (`0x005F22B0`): unread; no behaviour stated.
- 21 Mummy (`0x005F2850`): unread; no behaviour stated.
- 22 GreaterMummy (`0x005F2B10`): unread; no behaviour stated.
- 23 Vulture (`0x005F3170`): unread; no behaviour stated.
- 24 Mosquito (`0x005F3730`): unread; no behaviour stated.
- 25 WillOWisp (`0x005F39B0`): unread; no behaviour stated.
- 26 Arach (`0x005F4510`): unread; no behaviour stated.
- 27 ThornHulk (`0x005F4850`): unread; no behaviour stated.
- 28 Vampire (`0x005F4A70`): unread; no behaviour stated.
- 29 BatDemon (`0x005F5040`, alt `0x005F4FD0`): unread; no behaviour stated.
- 30 Fetish (`0x005F53E0`): unread; no behaviour stated.
- 31 NpcOutOfTown (`0x005E7880`): unread; no behaviour stated.
- 38 MaggotLarva (`0x005F6220`): unread; no behaviour stated.
- 39 PinHead (`0x005F6340`): unread; no behaviour stated.
- 40 MaggotEgg (`0x005F6530`): unread; no behaviour stated.
- 41 Towner (`0x005E7540`): unread; no behaviour stated.
- 42 Vendor (`0x005E9E00`): unread; no behaviour stated.
- 44 Duriel (`0x005F67B0`): unread; no behaviour stated.
- 45 Sarcophagus (`0x005F6A10`, init `0x005F6630`): unread; no behaviour stated.
- 46 ElementalBeast (`0x005F6B70`): unread; no behaviour stated.
- 47 FlyingScimitar (`0x005F6CA0`): unread; no behaviour stated.
- 48 ZakarumZealot (`0x005F6E60`): unread; no behaviour stated.
- 49 ZakarumPriest (`0x005F72D0`): unread; no behaviour stated.
- 50 Mephisto (`0x005F78B0`): unread; no behaviour stated.
- 51 Diablo (`0x005E9170`, alt `0x005E8480`): unread; no behaviour stated.
- 52 FrogDemon (`0x005F8260`, alt `0x005F81D0`): unread; no behaviour stated.
- 53 Summoner (`0x005F85C0`): unread; no behaviour stated.
- 54 NpcStationary (`0x005E73A0`): unread; no behaviour stated.
- 55 Izual (`0x005F89B0`): unread; no behaviour stated.
- 56 Tentacle (`0x005F8F80`): unread; no behaviour stated.
- 57 TentacleHead (`0x005F9270`): unread; no behaviour stated.
- 60 GoodNpcRanged (`0x005E7AC0`): unread; no behaviour stated.
- 61 Hireable (`0x005E52D0`, alt `0x005E5280`): unread; no behaviour stated.
- 63 GargoyleTrap (`0x005F9490`): unread; no behaviour stated.
- 64 SkeletonMage (`0x005F96C0`): unread; no behaviour stated.
- 65 FetishShaman (`0x005F9A80`, alt `0x005F9950`): unread; no behaviour stated.
- 66 SandMaggotQueen (`0x005F9CF0`): unread; no behaviour stated.
- 67 NecroPet (`0x005E4CF0`): unread; no behaviour stated.
- 68 VileMother (`0x005FA010`): unread; no behaviour stated.
- 69 VileDog (`0x005FA280`): unread; no behaviour stated.
- 70 FingerMage (`0x005FA380`): unread; no behaviour stated.
- 71 Regurgitator (`0x005FA710`): unread; no behaviour stated.
- 72 DoomKnight (`0x005FAA90`): unread; no behaviour stated.
- 73 AbyssKnight (`0x005FAB80`): unread; no behaviour stated.
- 74 OblivionKnight (`0x005FAF00`): unread; no behaviour stated.
- 75 QuillMother (`0x005FB2A0`): unread; no behaviour stated.
- 76 EvilHole (`0x005FB410`): unread; no behaviour stated.
- 77 Trap-Missile (`0x005FB5B0`): unread; no behaviour stated.
- 78 Trap-RightArrow (`0x005FB6C0`): unread; no behaviour stated.
- 79 Trap-LeftArrow (`0x005FB7E0`): unread; no behaviour stated.
- 80 Trap-Poison (`0x005FB900`): unread; no behaviour stated.
- 81 JarJar (`0x005E7590`): unread; no behaviour stated.
- 82 InvisoSpawner (`0x005E0160`): unread; no behaviour stated.
- 83 MosquitoNest (`0x005E0260`): unread; no behaviour stated.
- 84 BoneWall (`0x005E0400`, init `0x005E0390`): unread; no behaviour stated.
- 85 HighPriest (`0x005E0490`): unread; no behaviour stated.
- 86 Hydra (`0x005E9E60`): unread; no behaviour stated.
- 87 Trap-Melee (`0x005FBA60`): unread; no behaviour stated.
- 88 7TIllusion (`0x005EA080`): unread; no behaviour stated.
- 89 Megademon (`0x005E0C80`): unread; no behaviour stated.
- 90 Griswold (`0x005E5AC0`): unread; no behaviour stated.
- 91 DarkWanderer (`0x005EA130`): unread; no behaviour stated.
- 92 Trap-Nova (`0x005FB9B0`): unread; no behaviour stated.
- 93 ArcaneTower (`0x005E0F60`): unread; no behaviour stated.
- 94 DesertTurret (`0x005E0980`): unread; no behaviour stated.
- 95 PantherJavelin (`0x005E1080`): unread; no behaviour stated.
- 96 FetishBlowgun (`0x005E1250`): unread; no behaviour stated.
- 97 Spirit (`0x005E3840`): unread; no behaviour stated.
- 98 Smith (`0x005E3890`): unread; no behaviour stated.
- 99 TrappedSoul (`0x005E9F10`): unread; no behaviour stated.
- 101 AssassinSentry (`0x005EA3D0`, init `0x005EA290`): unread; no behaviour stated.
- 102 BladeCreeper (`0x005EA540`, init `0x005EA510`): unread; no behaviour stated.
- 103 InvisoPet (`0x005EA7A0`): unread; no behaviour stated.
- 104 DeathSentry (`0x005EA980`, init `0x005EA290`): unread; no behaviour stated.
- 105 ShadowWarrior (`0x005EAFA0`, init `0x005EAF50`): unread; no behaviour stated.
- 106 ShadowMaster (`0x005EB970`, init `0x005EB490`): unread; no behaviour stated.
- 107 Raven (`0x005ECC10`, init `0x005ECB70`): unread; no behaviour stated.
- 108 DruidWolf (`0x005ED710`): unread; no behaviour stated.
- 109 Totem (`0x005ED9E0`): unread; no behaviour stated.
- 110 Vines (`0x005EC6C0`, init `0x005EC6A0`): unread; no behaviour stated.
- 111 CycleOfLife (`0x005EC8C0`, init `0x005EC6A0`): unread; no behaviour stated.
- 112 DruidBear (`0x005ED730`): unread; no behaviour stated.
- 113 SiegeTower (`0x005E1860`): unread; no behaviour stated.
- 114 ReanimatedHorde (`0x005E1540`): unread; no behaviour stated.
- 115 SiegeBeast (`0x005E1900`): unread; no behaviour stated.
- 116 Minion (`0x005E1B60`): unread; no behaviour stated.
- 117 SuicideMinion (`0x005E1D30`): unread; no behaviour stated.
- 118 Succubus (`0x005E1E00`): unread; no behaviour stated.
- 119 SuccubusWitch (`0x005E2120`): unread; no behaviour stated.
- 120 Overseer (`0x005E27A0`): unread; no behaviour stated.
- 121 MinionSpawner (`0x005E2BD0`, init `0x005F6630`): unread; no behaviour stated.
- 122 Imp (`0x005E2FF0`, init `0x005E2FD0`): unread; no behaviour stated.
- 123 Catapult (`0x005E34C0`): unread; no behaviour stated.
- 124 FrozenHorror (`0x005E3530`): unread; no behaviour stated.
- 125 BloodLord (`0x005E36F0`): unread; no behaviour stated.
- 126 CatapultSpotter (`0x005EE040`): unread; no behaviour stated.
- 127 NpcBarb (`0x005EDC50`, init `0x005EDC40`): unread; no behaviour stated.
- 128 Nihlathak (`0x005EE5D0`, init `0x005EE5C0`, alt `0x005E5280`): unread; no behaviour stated.
- 129 GenericSpawner (`0x005E61B0`, init `0x005E6190`): unread; no behaviour stated.
- 130 DeathMauler (`0x005EE260`): unread; no behaviour stated.
- 131 Wussie (`0x005EE3C0`): unread; no behaviour stated.
- 132 AncientStatue (`0x005EEAA0`): unread; no behaviour stated.
- 133 Ancient (`0x005EF1A0`): unread; no behaviour stated.
- 134 BaalThrone (`0x005EF320`, init `0x005EF310`): unread; no behaviour stated.
- 135 BaalCrab (`0x005FCFE0`, alt `0x005FCF30`): unread; no behaviour stated.
- 136 BaalTaunt (`0x005EF710`): unread; no behaviour stated.
- 137 PutridDefiler (`0x005EFA90`): unread; no behaviour stated.
- 138 BaalToStairs (`0x005EF620`): unread; no behaviour stated.
- 139 BaalTentacle (`0x005EF820`): unread; no behaviour stated.
- 140 BaalCrabClone (`0x005FD210`, alt `0x005FCF30`): unread; no behaviour stated.
- 141 BaalMinion (`0x005EF910`): unread; no behaviour stated.
- 142 ClawViperEx (`0x005F1DE0`): unread; no behaviour stated.
- 143 ShadowMasterNoInit (`0x005EB970`, init `0x005EB5C0`): unread; no behaviour stated.
- 144 UberIzual (`0x005F8C80`): unread; no behaviour stated.
- 145 UberBaal (`0x005FD200`, alt `0x005FCF30`): unread; no behaviour stated.
- 146 UberMephisto (`0x005F81C0`): unread; no behaviour stated.
- 147 UberDiablo (`0x005E9DF0`, alt `0x005E8480`): unread; no behaviour stated.

Also unspecified: the special-state thinks of `ai.md` §3.2 other than
Idle (states 2–17: `0x005B14E0`, `0x005E5870`, `0x005E52D0`, `0x005E7AC0`,
`0x005E7C10`, `0x005E4CF0`, `0x005E7DC0`, `0x005E7F80`, `0x005E8020`,
`0x005E8140`, `0x005E8340`, `0x005E5C50`, `0x005E2610`, `0x005E1D30`,
`0x005E2D80`) and their init functions (`0x005E5730`, `0x005E80E0`,
`0x005E2CD0`); every AI init / alternate function in the TSV.

### 3.4 Server-do, `summarized` (7; 1.14d entry or helper read, behaviour D2MOO's)

- 8 MonBlizzCenter (`0x005AE8A0`): which level "lvl" is (missile +level
  or skill level), Param3 = 0 (division), whether interval/range are
  `max(Param2 − lvl/Param3, 3)` / `Param1 + max(lvl/Param3, 2)` in
  1.14d, order against the default flight, return value.
- 10 BlizzardCenter (`0x005AEA60`): how skill `Calc1` / `Calc2` are
  evaluated (skill record and level source, calc seam), order against
  the default flight, return value.
- 17 CairnStones (`0x005AF240`): Param1–5 meanings, the "mid-life" frame
  window of the sparks, the portal condition ("once near the end") and
  the object-creation call, return value.
- 25 EruptionCenter (`0x005AF880`): as 10 (mask 0x45).
- 28 Volcano (`0x005AFB80`): what missile data +0x28 holds and who sets
  it first, the frame window (Param3…Param4 inclusive?), the offset
  formula with Param2 (`roll(2r + 1) − r`?), the sub-missile creation
  arguments, Param5, return value.
- 34 BaalTauntControl (`0x005B04A0`): which of SubMissile1–3 count
  (non-negative columns?), the interval column, the creation arguments,
  Param1–4 meanings, return value.
- 35 RoyalStrikeChaosIce (`0x005B0640`): the turn (angle / path re-aim
  call), which bit value is left, +0x28's first value, Param1 test
  (elapsed multiple?), return value.

### 3.5 Server-do, `D2MOO-only` (28)

- 2 PlagueJavelin_PoisonJavelin_PoisonTrap (`0x005AE400`): D2MOO-only; reads SrvCalc1;SubMissile1.
- 3 PoisonCloud_Blizzard_ThunderStorm_HandOfGod (`0x005AE480`): D2MOO-only; reads -.
- 5 FireWall_ImmolationFire_MeteorFire_MoltenBoulderFirePath (`0x005AE520`): D2MOO-only; reads SubStart;SubStop.
- 6 MoltenBoulder_FireWallMaker (`0x005AE680`): D2MOO-only; reads SubMissile1.
- 7 GuidedArrow_BoneSpirit (`0x005AE780`): D2MOO-only; reads Param1.
- 9 BatLightningBolt (`0x005AE940`): D2MOO-only; reads SubMissile1.
- 11 FingerMageSpider (`0x005AEB60`): D2MOO-only; reads Param1;Param2;Param3.
- 12 DiabWallMaker (`0x005AECA0`): D2MOO-only; reads SubMissile1.
- 13 BoneWallMaker (`0x005AEDA0`): D2MOO-only; reads - (skills PetType).
- 14 GrimWard (`0x005AEF70`): D2MOO-only; reads Param1;Param2.
- 15 FrozenOrb (`0x005AF030`): D2MOO-only; reads Param1;Param2;SubMissile1.
- 16 FrozenOrbNova (`0x005AF170`): D2MOO-only; reads Param1;Param2.
- 18 TowerChestSpawner (`0x005AF300`): D2MOO-only; reads Param1;Param2;Param3.
- 19 RadamentDeath (`0x005B0940`): D2MOO-only; reads -.
- 20 BladeCreeper (`0x005AF540`): D2MOO-only; reads -.
- 21 Distraction (`0x005AF590`): D2MOO-only; reads SubMissile1.
- 22 LightningTrailingJavelin (`0x005AF620`): D2MOO-only; reads Param1;SubMissile1.
- 23 24_SuccFireBall_FirestormMaker (`0x005AF790`): D2MOO-only; reads Param1;SubMissile1.
- 24 24_SuccFireBall_FirestormMaker (`0x005AF790`): D2MOO-only; reads Param1;SubMissile1.
- 26 Vines_PlagueVines (`0x005AF980`): D2MOO-only; reads Param1;SubMissile1.
- 27 Tornado (`0x005AFA30`): D2MOO-only; reads Param1;Param2.
- 29 RecyclerDelay (`0x005AFD70`): D2MOO-only; reads Param1.
- 30 RabiesPlague (`0x005B0010`): D2MOO-only; reads Param1;Param2;SubMissile1.
- 31 WakeOfDestructionMaker_BaalColdMaker (`0x005B01F0`): D2MOO-only; reads SubMissile1.
- 32 TigerFury (`0x005B03E0`): D2MOO-only; reads SubMissile1.
- 33 VineRecyclerDelay (`0x005AFEC0`): D2MOO-only; reads Param1.
- 36 BaalFxControl (`0x005B0A40`): D2MOO-only; reads -.
- 37 Unused (`0x005B0AA0`): D2MOO-only; reads Param1-3;SubMissile1-3.

### 3.6 Server-hit, `summarized` (1)

- 58 BaalTauntLightningControl (`0x005ACDF0`): seed re-init is read
  (`init_low(missile x)`, §R9.4); missing the draws after it (count,
  offset formula with sHitPar1), the HitSubMissile1 creation arguments,
  return value.

### 3.7 Server-hit, `D2MOO-only` (52)

- 1 Fireball_ExplodingArrow_FreezingArrowExplosion (`0x005A9A70`): D2MOO-only; reads sHitPar1 (or skill Calc1).
- 2 PlagueJavelin_PoisonPotion (`0x005A9D80`): D2MOO-only; reads sHitPar1-3;HitSubMissile1.
- 3 ExplosivePotion_BombOnGround (`0x005A9F90`): D2MOO-only; reads (via 44).
- 4 ExplodingArrow_FreezingArrow_RoyalStrikeMeteorCenter (`0x005B07A0`): D2MOO-only; reads sHitPar1;HitSubMissile1-4.
- 5 Unused (`0x005ABC40`): D2MOO-only; reads sHitPar1;HitSubMissile1.
- 6 Unused (`0x005AA1C0`): D2MOO-only; reads sHitPar1;sHitPar2.
- 7 HolyBolt_FistOfTheHeavenBolt (`0x005A9FB0`): D2MOO-only; reads sHitPar1;sHitPar2.
- 8 Blaze (`0x005AA180`): D2MOO-only; reads -.
- 9 ImmolationArrow (`0x005AA250`): D2MOO-only; reads sHitPar1;sHitPar2;SHitCalc1;HitSubMissile1.
- 10 GuidedArrow_BoneSpirit (`0x005AA650`): D2MOO-only; reads Param2.
- 11 Unused (`0x005B0870`): D2MOO-only; reads sHitPar1;HitSubMissile1-4.
- 12 ChainLightning_LightningStrike (`0x005AA730`): D2MOO-only; reads sHitPar1.
- 13 GlacialSpike_HellMeteorDown (`0x005AA8B0`): D2MOO-only; reads sHitPar1;sHitPar2.
- 14 MeteorCenter_CatapultMeteor_RoyalStrikeMeteor (`0x005AABB0`): D2MOO-only; reads sHitPar1;sHitPar2;HitSubMissile1;Param3;Param4.
- 15 SpiderGooLay (`0x005AAD40`): D2MOO-only; reads HitSubMissile1.
- 16 SpiderGoo_VinesTrail_VinesWither (`0x005AAE10`): D2MOO-only; reads - (skill calcs).
- 17 Howl (`0x005AAFB0`): D2MOO-only; reads - (skill params).
- 18 Shout_BattleCommand_BattleOrders (`0x005AB0B0`): D2MOO-only; reads -.
- 19 FingerMageSpider (`0x005AB110`): D2MOO-only; reads - (skill calcs).
- 20 LightningFury (`0x005AB370`): D2MOO-only; reads sHitPar1;sHitPar2;HitSubMissile1.
- 21 BattleCry (`0x005AB500`): D2MOO-only; reads - (skill calcs).
- 22 FistOfTheHeavensDelay (`0x005ADD20`): D2MOO-only; reads sHitPar1;sHitPar2;HitSubMissile1.
- 23 Unused (`0x005ACFC0`): D2MOO-only; reads sHitPar1.
- 24 PantherPotOrange (`0x005A9BF0`): D2MOO-only; reads sHitPar1.
- 25 PantherPotGreen (`0x005AB820`): D2MOO-only; reads sHitPar1;HitSubMissile1.
- 26 GrimWardStart (`0x005AB8D0`): D2MOO-only; reads sHitPar1;HitSubMissile1.
- 27 GrimWard (`0x005ABA00`): D2MOO-only; reads -.
- 28 GrimWardScare (`0x005ABA10`): D2MOO-only; reads - (skill params).
- 29 FrozenOrb (`0x005ABB00`): D2MOO-only; reads sHitPar1;HitSubMissile1.
- 31 FireHead (`0x005ABD70`): D2MOO-only; reads -.
- 32 CairnStones (`0x005ABE50`): D2MOO-only; reads Param4.
- 33 TowerChestSpawner (`0x005ABEB0`): D2MOO-only; reads -.
- 35 OrbMist (`0x005ABEE0`): D2MOO-only; reads -.
- 36 MissileInAir (`0x005ABF70`): D2MOO-only; reads HitSubMissile1.
- 37 BladeCreeper (`0x005AC020`): D2MOO-only; reads -.
- 38 CatapultChargedBall (`0x005AC0A0`): D2MOO-only; reads sHitPar1;sHitPar2;HitSubMissile1.
- 39 ImpSpawnMonsters (`0x005AC1D0`): D2MOO-only; reads -.
- 40 CatapultSpikeBall (`0x005AC250`): D2MOO-only; reads sHitPar1;sHitPar2;HitSubMissile1.
- 43 HealingVortex (`0x005AC350`): D2MOO-only; reads - (skill damage).
- 44 ExplodingJavelin (`0x005A9E10`): D2MOO-only; reads sHitPar1.
- 45 LightningTrailingJavelin (`0x005AC480`): D2MOO-only; reads sHitPar1;sHitPar2;HitSubMissile1.
- 47 MoltenBoulder (`0x005AC550`): D2MOO-only; reads sHitPar1;sHitPar2;HitSubMissile1.
- 48 MoltenBoulderEmerge (`0x005AC6D0`): D2MOO-only; reads HitSubMissile1.
- 50 PlagueVinesTrail (`0x005AC800`): D2MOO-only; reads sHitPar1.
- 51 VolcanoDebris (`0x005AC870`): D2MOO-only; reads HitSubMissile1-3.
- 52 BladeFury (`0x005AC940`): D2MOO-only; reads sHitPar1;HitSubMissile1.
- 53 RabiesContagion (`0x005ACA50`): D2MOO-only; reads -.
- 54 BaalSpawnMonsters (`0x005ACAF0`): D2MOO-only; reads -.
- 55 Baalnferno (`0x005ACB60`): D2MOO-only; reads sHitPar1.
- 56 ArmageddonControl (`0x005ACC50`): D2MOO-only; reads sHitPar1;HitSubMissile1.
- 57 BaalFxControl (`0x005AD970`): D2MOO-only; reads -.
- 59 BaalTauntPoisonControl (`0x005ACF20`): D2MOO-only; reads sHitPar1;HitSubMissile1.

Priority from `missiles.md` open question 8: server-do 2, 3, 5, 7
(Act 1–2 monsters); server-hit 1, 4, 12, 13 (common skills). For AI,
the D2MOO-only Act I rows (3.2) come first (`ai.md` §9.14).

## 4. Seams

None added or changed. The stubs keep their contracts
(`missiles::Unhandled::{SrvDo, NullSrvDo, SrvHit, NullSrvHit}`,
`monsters::ai::Unhandled::Function`); stubbed server-do functions still
keep the missile forever (`impl-monsters.md` open question 11).

## 5. Gate

All clean on this branch: `cargo fmt --all -- --check`;
`cargo clippy --workspace --all-targets -- -D warnings`;
`cargo test -p d2-sim -p conformance` (d2-sim 1,234 pass, 5 ignored;
conformance 7 + 2 + 2 pass); `cargo run -p depcheck` (OK, 8 crates);
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`
(21 methods); `python3 tools/coverage.py --check` (3,260 claims, 0
errors) and `--selftest` (ok). Counts are this branch's, not a merged
count.
