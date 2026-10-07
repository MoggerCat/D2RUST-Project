// Spec: specs/monsters/ai.md §9 (per-AI behaviours), §10 (catalogue); specs/monsters/ai-bodies-2.md..ai-bodies-5.md (Act II–V bodies)
//! The AI functions, dispatched by their 1.14d address (the control
//! stores the address, as the original stores the pointer). Functions
//! with status `spec'd-here` in `ai-functions.tsv` are implemented here,
//! in [`super::bodies`], [`super::npc`] or the act modules
//! (`bodies2`..`bodies5`), with the init functions [`INIT_IMPLEMENTED`]
//! and the alternates of SandMaggot, BatDemon, FrogDemon, FetishShaman,
//! Diablo / BaalCrab and Nihlathak / Hireable. The special-state thinks
//! implemented are states 5 (GoodNpcRanged), 10 / 17, 11, 12 and 15
//! (SuicideMinion). Every other address is a stub that logs
//! [`Unhandled::Function`] and does nothing (TODO(ai.md open question
//! 10)).

use crate::game::Game;
use crate::units::{UnitId, UnitType};

use super::tactics::*;
use super::{bodies, bodies2, bodies3, bodies4, bodies5};
use super::{idle, mode, AiCommand, AiHost, Ctx, ModeTarget, TickParam, Unhandled};

/// Think functions implemented here, by address (AI table index in the
/// comment). Checked against the catalogue's `spec'd-here` rows by
/// `tests::implemented_matches_catalogue`.
pub const IMPLEMENTED: [(u32, u8); 93] = [
    (0x005B_0CC0, 0),   // None
    (0x005B_0CD0, 1),   // Idle
    (0x005E_FCF0, 2),   // Skeleton
    (0x005E_FE20, 3),   // Zombie
    (0x005E_FF50, 4),   // Bighead
    (0x005F_00E0, 5),   // BloodHawk
    (0x005F_02C0, 6),   // Fallen
    (0x005E_FB80, 7),   // Brute
    (0x005F_0700, 8),   // SandRaider
    (0x005F_0A20, 9),   // Wraith
    (0x005F_0B00, 10),  // CorruptRogue
    (0x005F_0CD0, 11),  // Baboon
    (0x005F_12A0, 12),  // Goatman
    (0x005F_1440, 13),  // FallenShaman
    (0x005F_1140, 14),  // QuillRat
    (0x005F_1800, 15),  // SandMaggot
    (0x005F_1B60, 16),  // ClawViper
    (0x005F_20A0, 17),  // SandLeaper
    (0x005F_22B0, 18),  // PantherWoman
    (0x005F_2460, 19),  // Swarm
    (0x005F_2540, 20),  // Scarab
    (0x005F_2850, 21),  // Mummy
    (0x005F_2B10, 22),  // GreaterMummy
    (0x005F_3170, 23),  // Vulture
    (0x005F_3730, 24),  // Mosquito
    (0x005F_39B0, 25),  // WillOWisp
    (0x005F_4510, 26),  // Arach
    (0x005F_4850, 27),  // ThornHulk
    (0x005F_4A70, 28),  // Vampire
    (0x005F_5040, 29),  // BatDemon
    (0x005F_53E0, 30),  // Fetish
    (0x005E_7880, 31),  // NpcOutOfTown
    (0x005E_7130, 32),  // Npc
    (0x005F_56D0, 33),  // HellMeteor
    (0x005F_5830, 34),  // Andariel
    (0x005F_5A20, 35),  // CorruptArcher
    (0x005F_5D50, 36),  // CorruptLancer
    (0x005F_6070, 37),  // SkeletonBow
    (0x005F_6220, 38),  // MaggotLarva
    (0x005F_6340, 39),  // PinHead
    (0x005F_6530, 40),  // MaggotEgg
    (0x005F_6650, 43),  // FoulCrowNest
    (0x005F_67B0, 44),  // Duriel
    (0x005F_6E60, 48),  // ZakarumZealot
    (0x005F_72D0, 49),  // ZakarumPriest
    (0x005F_78B0, 50),  // Mephisto
    (0x005E_9170, 51),  // Diablo
    (0x005F_8260, 52),  // FrogDemon
    (0x005F_85C0, 53),  // Summoner
    (0x005F_89B0, 55),  // Izual
    (0x005E_7E20, 58),  // Navi
    (0x005E_6320, 59),  // BloodRaven
    (0x005E_7AC0, 60),  // GoodNpcRanged
    (0x005E_7D60, 62),  // TownRogue
    (0x005F_96C0, 64),  // SkeletonMage
    (0x005F_9A80, 65),  // FetishShaman
    (0x005F_9CF0, 66),  // SandMaggotQueen
    (0x005F_A010, 68),  // VileMother
    (0x005F_A280, 69),  // VileDog
    (0x005F_A380, 70),  // FingerMage
    (0x005F_A710, 71),  // Regurgitator
    (0x005F_AA90, 72),  // DoomKnight
    (0x005F_AB80, 73),  // AbyssKnight
    (0x005F_AF00, 74),  // OblivionKnight
    (0x005E_0490, 85),  // HighPriest
    (0x005E_0C80, 89),  // Megademon
    (0x005E_5AC0, 90),  // Griswold
    (0x005E_1080, 95),  // PantherJavelin
    (0x005E_1250, 96),  // FetishBlowgun
    (0x005E_3890, 98),  // Smith
    (0x005E_7F50, 100), // Buffy
    (0x005E_1540, 114), // ReanimatedHorde
    (0x005E_1900, 115), // SiegeBeast
    (0x005E_1B60, 116), // Minion
    (0x005E_1D30, 117), // SuicideMinion
    (0x005E_1E00, 118), // Succubus
    (0x005E_2120, 119), // SuccubusWitch
    (0x005E_27A0, 120), // Overseer
    (0x005E_2FF0, 122), // Imp
    (0x005E_3530, 124), // FrozenHorror
    (0x005E_36F0, 125), // BloodLord
    (0x005E_E5D0, 128), // Nihlathak
    (0x005E_E260, 130), // DeathMauler
    (0x005E_EAA0, 132), // AncientStatue
    (0x005E_F1A0, 133), // Ancient
    (0x005E_F320, 134), // BaalThrone
    (0x005F_CFE0, 135), // BaalCrab
    (0x005E_F710, 136), // BaalTaunt
    (0x005E_FA90, 137), // PutridDefiler
    (0x005E_F620, 138), // BaalToStairs
    (0x005F_D210, 140), // BaalCrabClone
    (0x005E_F910, 141), // BaalMinion
    (0x005F_1DE0, 142), // ClawViperEx
];

/// Init functions implemented here (`ai.md` §9.17, §9.18;
/// `ai-bodies-2.md` §16 terror; `ai-bodies-5.md` §3, §20, §23).
pub const INIT_IMPLEMENTED: [u32; 6] = [
    0x005F_6630,
    0x005E_6300,
    0x005E_80E0,
    0x005E_2FD0,
    0x005E_F310,
    0x005E_E5C0,
];

/// Runs the init function at `addr` (§3.3 step 4); a stub logs
/// [`Unhandled::Function`].
pub fn run_init<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, addr: u32, u: UnitId) {
    match addr {
        0x005F_6630 => bodies::nest_init(game, cx, u),
        0x005E_6300 => bodies::blood_raven_init(cx, u),
        0x005E_80E0 => bodies2::terror_init(game, cx, u),
        0x005E_2FD0 => bodies5::imp_init(cx, u),
        0x005E_F310 => bodies5::baal_throne_init(),
        0x005E_E5C0 => bodies5::nihlathak_init(),
        _ => cx
            .store
            .unhandled
            .push(Unhandled::Function { addr, unit: u }),
    }
}

/// Whether `addr` has a body here.
pub fn implemented(addr: u32) -> bool {
    IMPLEMENTED.iter().any(|&(a, _)| a == addr)
}

/// Calls the AI function at `addr` (the dispatcher's step 4).
pub fn run_function<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    addr: u32,
    u: UnitId,
    p: &TickParam,
) {
    match addr {
        0x005B_0CC0 => {}
        0x005B_0CD0 | 0x005E_7F50 => ai_idle(game, cx, u),
        0x005E_FCF0 => skeleton_like(game, cx, u, p, Pattern::Skeleton),
        0x005E_FE20 => zombie(game, cx, u, p),
        0x005F_02C0 => fallen(game, cx, u, p),
        0x005E_FB80 => brute(game, cx, u, p),
        0x005F_0A20 => skeleton_like(game, cx, u, p, Pattern::Wraith),
        0x005F_12A0 | 0x005F_2460 => skeleton_like(game, cx, u, p, Pattern::Goatman),
        0x005F_1440 => fallen_shaman(game, cx, u, p),
        0x005F_1140 => quill_rat(game, cx, u, p),
        0x005F_5830 => andariel(game, cx, u, p),
        0x005F_5A20 => corrupt_archer(game, cx, u, p),
        0x005F_5D50 => corrupt_lancer(game, cx, u, p),
        0x005E_7E20 => navi(game, cx, u, p),
        0x005E_7D60 => town_rogue(game, cx, u),
        0x005E_7AC0 => super::npc::good_npc_ranged(game, cx, u, p),
        0x005E_7880 => super::npc::npc_out_of_town(game, cx, u, p),
        0x005E_7130 => super::npc::npc(game, cx, u, p),
        0x005E_FF50 => bodies::bighead(game, cx, u, p),
        0x005F_00E0 => bodies::blood_hawk(game, cx, u, p),
        0x005F_0700 => bodies::sand_raider(game, cx, u, p),
        0x005F_0B00 => bodies::corrupt_rogue(game, cx, u, p),
        0x005F_0CD0 => bodies::baboon(game, cx, u, p),
        0x005F_1800 => bodies::sand_maggot(game, cx, u, p),
        0x005F_1750 => bodies::sand_maggot_alt(game, cx, u, p),
        0x005F_2540 => bodies::scarab(game, cx, u, p),
        0x005F_4510 => bodies::arach(game, cx, u, p),
        0x005F_4A70 => bodies::vampire(game, cx, u, p),
        0x005F_53E0 => bodies::fetish(game, cx, u, p),
        0x005F_56D0 => bodies::hell_meteor(game, cx, u, p),
        0x005F_6070 => bodies::skeleton_bow(game, cx, u, p),
        0x005F_6650 => bodies::foul_crow_nest(game, cx, u, p),
        0x005E_6320 => bodies::blood_raven(game, cx, u, p),
        0x005F_96C0 => bodies::skeleton_mage(game, cx, u, p),
        0x005E_3890 => smith(game, cx, u, p),
        0x005E_5AC0 => griswold(game, cx, u, p),
        // Act II (`ai-bodies-2.md`).
        0x005E_1080 => bodies2::panther_javelin(game, cx, u, p),
        0x005F_2B10 => bodies2::greater_mummy(game, cx, u, p),
        0x005F_2850 => bodies2::mummy(game, cx, u, p),
        0x005F_22B0 => bodies2::panther_woman(game, cx, u, p),
        0x005F_6220 => bodies2::maggot_larva(game, cx, u, p),
        0x005F_20A0 => bodies2::sand_leaper(game, cx, u, p),
        0x005F_6530 => bodies2::maggot_egg(game, cx, u, p),
        0x005F_6340 => bodies2::pin_head(game, cx, u, p),
        0x005F_1B60 => bodies2::claw_viper(game, cx, u, p),
        0x005F_3170 => bodies2::vulture(game, cx, u, p),
        0x005F_5040 => bodies2::bat_demon(game, cx, u, p),
        0x005F_4FD0 => bodies2::bat_demon_alt(game, cx, u),
        0x005F_9CF0 => bodies2::sand_maggot_queen(game, cx, u, p),
        0x005F_67B0 => bodies2::duriel(game, cx, u, p),
        0x005F_85C0 => bodies2::summoner(game, cx, u, p),
        0x005E_8020 => bodies2::dim_vision(game, cx, u, p),
        0x005E_8140 => bodies2::terror(game, cx, u, p),
        0x005E_8340 => bodies2::taunted(game, cx, u, p),
        // Act III (`ai-bodies-3.md`).
        0x005F_3730 => bodies3::mosquito(game, cx, u, p),
        0x005F_4850 => bodies3::thorn_hulk(game, cx, u, p),
        0x005F_6E60 => bodies3::zakarum_zealot(game, cx, u, p),
        0x005F_72D0 => bodies3::zakarum_priest(game, cx, u, p),
        0x005F_8260 => bodies3::frog_demon(game, cx, u, p),
        0x005F_81D0 => bodies3::frog_demon_alt(game, cx, u, p),
        0x005F_9A80 => bodies3::fetish_shaman(game, cx, u, p),
        0x005F_9950 => bodies3::fetish_shaman_alt(game, cx, u),
        0x005E_0490 => bodies3::high_priest(game, cx, u, p),
        0x005E_1250 => bodies3::fetish_blowgun(game, cx, u, p),
        0x005F_39B0 => bodies3::will_o_wisp(game, cx, u, p),
        0x005F_78B0 => bodies3::mephisto(game, cx, u, p),
        // Act IV (`ai-bodies-4.md`).
        0x005F_A010 => bodies4::vile_mother(game, cx, u, p),
        0x005F_A280 => bodies4::vile_dog(game, cx, u, p),
        0x005F_A380 => bodies4::finger_mage(game, cx, u, p),
        0x005F_A710 => bodies4::regurgitator(game, cx, u, p),
        0x005E_0C80 => bodies4::megademon(game, cx, u, p),
        0x005E_9170 => bodies4::diablo(game, cx, u, p),
        // Diablo's alternate; BaalCrab's `0x005FCF30` is the same
        // (`ai-bodies-5.md` §21).
        0x005E_8480 | 0x005F_CF30 => bodies4::diablo_alt(game, cx, u),
        0x005F_89B0 => bodies4::izual(game, cx, u, p),
        0x005F_AA90 => bodies4::doom_knight(game, cx, u, p),
        0x005F_AB80 => bodies4::abyss_knight(game, cx, u, p),
        0x005F_AF00 => bodies4::oblivion_knight(game, cx, u, p),
        // Act V (`ai-bodies-5.md`).
        0x005E_1B60 => bodies5::minion(game, cx, u, p),
        0x005E_2FF0 => bodies5::imp(game, cx, u, p),
        0x005E_1E00 => bodies5::succubus(game, cx, u, p),
        0x005E_36F0 => bodies5::blood_lord(game, cx, u, p),
        0x005E_2120 => bodies5::succubus_witch(game, cx, u, p),
        0x005E_27A0 => bodies5::overseer(game, cx, u, p),
        0x005E_1540 => bodies5::reanimated_horde(game, cx, u, p),
        0x005F_1DE0 => bodies5::claw_viper_ex(game, cx, u, p),
        0x005E_E260 => bodies5::death_mauler(game, cx, u, p),
        0x005E_FA90 => bodies5::putrid_defiler(game, cx, u, p),
        0x005E_F1A0 => bodies5::ancient(game, cx, u, p),
        0x005E_EAA0 => bodies5::ancient_statue(game, cx, u),
        0x005E_3530 => bodies5::frozen_horror(game, cx, u, p),
        0x005E_1900 => bodies5::siege_beast(game, cx, u, p),
        0x005E_1D30 => bodies5::suicide_minion(game, cx, u, p),
        0x005E_F910 => bodies5::baal_minion(game, cx, u, p),
        0x005E_F710 => bodies5::baal_taunt(game, cx, u, p),
        0x005E_F620 => bodies5::baal_to_stairs(game, cx, u, p),
        0x005E_F320 => bodies5::baal_throne(game, cx, u, p),
        0x005F_CFE0 => bodies5::baal_crab(game, cx, u, p),
        0x005F_D210 => bodies5::baal_crab_clone(game, cx, u, p),
        0x005E_E5D0 => bodies5::nihlathak(game, cx, u, p),
        // Nihlathak's alternate, also the Hireable alternate.
        0x005E_5280 => bodies5::nihlathak_alt(game, cx, u),
        _ => cx
            .store
            .unhandled
            .push(Unhandled::Function { addr, unit: u }),
    }
}

fn a1<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, t: Option<UnitId>) {
    mode_at(game, cx, u, mode::ATTACK1, t);
}

fn a2<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, t: Option<UnitId>) {
    mode_at(game, cx, u, mode::ATTACK2, t);
}

fn at(t: Option<UnitId>) -> ModeTarget {
    match t {
        Some(t) => ModeTarget::Unit(t),
        None => ModeTarget::Point(0, 0),
    }
}

/// §9.2 Idle (1) / Buffy (100): with a room, idle 200.
fn ai_idle<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId) {
    if game.lists.unit(u).and_then(|e| e.room()).is_some() {
        idle(game, cx, u, 200);
    }
}

/// §9.3 Zombie.
fn zombie<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, p: &TickParam) {
    let t = p.target;
    if p.combat {
        if cx.chance(u, cx.aip(p, 4)) {
            a1(game, cx, u, t);
        } else {
            a2(game, cx, u, t);
        }
        return;
    }
    let run = cx.ai_state_set(u)
        || (p.distance < cx.aip(p, 2) && cx.chance(u, cx.aip(p, 1)))
        || cx.world.level_id(game, u) == 17;
    if !run {
        wander(game, cx, u, 3);
        return;
    }
    set_velocity(cx, u, 0, 100, 0);
    run_to(game, cx, u, t, 0);
}

/// §9.4 Fallen.
fn fallen<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, p: &TickParam) {
    let t = p.target;
    // 1.
    if matches!(cx.world.anim_mode(u), mode::DEATH | mode::DEAD) {
        return;
    }
    super::delete_thinks(game, u);
    // 2. Corpse check.
    let my_pos = cx.world.position(u);
    let rooms = game
        .lists
        .unit(u)
        .and_then(|e| e.room())
        .and_then(|r| game.lists.room(r))
        .map(|r| r.adjacent.clone())
        .unwrap_or_default();
    'rooms: for room in rooms {
        for dead in cx.world.last_dead(game, room).into_iter().flatten() {
            let is_corpse = game
                .lists
                .unit(dead)
                .is_some_and(|e| e.ty == UnitType::Monster)
                && cx.world.anim_mode(dead) == mode::DEATH
                && distance_no_size(cx.world.position(dead), my_pos) < 15;
            if !is_corpse {
                continue;
            }
            if let Some(c) = cx.store.control_mut(u) {
                c.params[0] = 1;
            }
            free_current_command(cx, u);
            set_velocity(cx, u, 0, 50, 0);
            if escape(game, cx, u, t, 12, true, false) {
                if cx.world.seed(u).step().is_multiple_of(20) {
                    cx.world.play_sound(game, u, 17, None);
                }
                return;
            }
            break 'rooms;
        }
    }
    // 3.
    if cx.world.anim_mode(u) != mode::NEUTRAL {
        idle(game, cx, u, 10);
        return;
    }
    // 4.
    if let Some(cmd) = current_command(cx, u) {
        if cmd.params[0] != 1 {
            free_current_command(cx, u);
            idle(game, cx, u, 10);
        } else if p.combat {
            if !cx.chance(u, cx.aip(p, 3)) {
                idle(game, cx, u, 5);
            } else if cx.chance(u, cx.aip(p, 4)) {
                a1(game, cx, u, t);
            } else {
                a2(game, cx, u, t);
            }
        } else if !walk_to(game, cx, u, t, 0) {
            free_current_command(cx, u);
        }
        return;
    }
    // 5.1.
    if !p.combat && cx.ai_state_set(u) {
        walk_to(game, cx, u, t, 0);
        return;
    }
    // 5.2.
    if p.distance < 15 && minion_owner(game, cx, u) == Some(u) && cx.chance(u, cx.aip(p, 1)) {
        let cmd = AiCommand {
            params: [1, 0, 0, 0, 0],
        };
        command_minions(game, cx, u, cmd);
        copy_command(cx, u, cmd);
        mode_at(game, cx, u, mode::SKILL2, t);
        return;
    }
    // 5.3.
    if !p.combat {
        if p.distance <= cx.aip(p, 2) {
            walk_to(game, cx, u, t, 7);
        } else if cx.chance(u, 30) {
            wander(game, cx, u, 3);
        } else {
            idle(game, cx, u, 10);
        }
        return;
    }
    // 5.4.
    let param0 = cx.store.control(u).map_or(0, |c| c.params[0]);
    if param0 == 0 && !cx.chance(u, cx.aip(p, 3)) {
        if cx.chance(u, 30) {
            mode_at(game, cx, u, mode::SKILL2, t);
        } else {
            idle(game, cx, u, 10);
        }
        return;
    }
    if let Some(c) = cx.store.control_mut(u) {
        c.params[0] = 0;
    }
    if cx.chance(u, cx.aip(p, 4)) {
        a1(game, cx, u, t);
    } else {
        a2(game, cx, u, t);
    }
}

/// §9.5 Brute (tests aip3 twice, edge case 6).
fn brute<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, p: &TickParam) {
    let t = p.target;
    if p.combat {
        if cx.chance(u, cx.aip(p, 3)) {
            if cx.chance(u, cx.aip(p, 4)) {
                a1(game, cx, u, t);
            } else {
                a2(game, cx, u, t);
            }
        } else if cx.chance(u, cx.aip(p, 3)) {
            circle(game, cx, u, t, 4, false);
        } else {
            idle(game, cx, u, 15);
        }
        return;
    }
    let speed = 100 - cx.world.life_percent(u).clamp(40, 100);
    set_velocity(cx, u, 0, speed, 0);
    walk_to_method13(game, cx, u, t, 7);
}

/// §9.6 FallenShaman.
fn fallen_shaman<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    u: UnitId,
    p: &TickParam,
) {
    let t = p.target;
    // 1.
    if p.combat && cx.chance(u, cx.aip(p, 3)) {
        a1(game, cx, u, t);
        return;
    }
    // 2.
    let aip4 = cx.aip(p, 4);
    let own = !cx.world.is_unique(u) || cx.world.is_champion(u);
    let (corpse, count) = cx
        .world
        .shaman_corpses(game, u, aip4.wrapping_mul(aip4), own);
    // 3.
    if cx.chance(u, cx.aip(p, 1)) {
        command_minions(
            game,
            cx,
            u,
            AiCommand {
                params: [1, 0, 0, 0, 0],
            },
        );
    }
    // 4.
    if count > 0 {
        let (skill1, _) = cx.skill(p, 1);
        if let Some(c) = corpse {
            if cx.chance(u, cx.aip(p, 1)) && cx.world.skill_usable(game, u, skill1, c) {
                use_sequence_skill(game, cx, u, skill1, ModeTarget::Unit(c));
                return;
            }
        }
    }
    let aip5 = cx.aip(p, 5);
    let (skill2, mode2) = cx.skill(p, 2);
    // 5.
    if let Some(tt) = t {
        if p.distance < aip5 && cx.chance(u, cx.aip(p, 2)) {
            use_skill(game, cx, u, mode2, skill2, ModeTarget::Unit(tt));
            return;
        }
    }
    // 6.
    let (s, e, _) = cx.world.secondary_target(game, u);
    if let Some(s) = s {
        if e < aip5 && cx.chance(u, cx.aip(p, 2)) {
            use_skill(game, cx, u, mode2, skill2, ModeTarget::Unit(s));
            return;
        }
    }
    // 7.
    if cx.chance(u, cx.aip(p, 3)) {
        circle(game, cx, u, t, 3, false);
    } else {
        idle(game, cx, u, 10);
    }
}

/// §9.7 QuillRat.
fn quill_rat<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, p: &TickParam) {
    let t = p.target;
    // 1.
    if let Some(cmd) = current_command(cx, u) {
        let ty = UnitType::ALL
            .get(usize::try_from(cmd.params[1]).unwrap_or(usize::MAX))
            .copied();
        let exists = ty.is_some_and(|ty| game.lists.find_unit(ty, cmd.params[2] as u32).is_some());
        if exists {
            a2(game, cx, u, t);
            free_current_command(cx, u);
            return;
        }
        free_current_command(cx, u);
    }
    // 2.
    if p.combat {
        a1(game, cx, u, t);
        return;
    }
    // 3.
    if cx.ai_state_set(u) {
        a2(game, cx, u, t);
        return;
    }
    let walk = i32::from(cx.aip(p, 4) as u8).max(3);
    // 4.
    if p.distance >= cx.aip(p, 1) {
        wander(game, cx, u, walk);
        return;
    }
    // 5.
    if cx.chance(u, cx.aip(p, 2)) {
        a2(game, cx, u, t);
        return;
    }
    // 6.
    let n = i32::from(cx.aip(p, 4) as u8);
    if escape(game, cx, u, t, n, true, false) {
        return;
    }
    // 7.
    if p.distance > 3 {
        wander(game, cx, u, walk);
    } else {
        a2(game, cx, u, t);
    }
}

/// §9.8 CorruptLancer.
fn corrupt_lancer<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    u: UnitId,
    p: &TickParam,
) {
    let t = p.target;
    let melee_rng = cx
        .tables
        .monstats2
        .get(p.class2)
        .map_or(0, |r| i32::from(r.meleerng));
    // 1.
    if p.distance > cx.aip(p, 5) {
        cx.world.set_path_steps(u, melee_rng);
        set_velocity(cx, u, 13, 100, 0);
        move_steps(game, cx, u, t, true, melee_rng);
        if let Some(c) = cx.store.control_mut(u) {
            c.params[0] = 1;
        }
        return;
    }
    // 2.
    if p.combat {
        let param0 = cx.store.control(u).map_or(0, |c| c.params[0]);
        if param0 == 0 && !cx.chance(u, cx.aip(p, 2)) {
            idle(game, cx, u, cx.aip(p, 3));
            return;
        }
        if let Some(c) = cx.store.control_mut(u) {
            c.params[0] = 0;
        }
        for (n, aip) in [(1, 6), (2, 7), (3, 8)] {
            let (skill, m) = cx.skill(p, n);
            if skill >= 0 && cx.chance(u, cx.aip(p, aip)) {
                use_skill(game, cx, u, m, skill, at(t));
                return;
            }
        }
        a1(game, cx, u, t);
        return;
    }
    // 3.
    if !cx.chance(u, cx.aip(p, 1)) {
        idle(game, cx, u, cx.aip(p, 3));
        return;
    }
    cx.world.set_path_steps(u, melee_rng);
    if cx.chance(u, cx.aip(p, 4)) {
        set_velocity(cx, u, 0, 100, 0);
        move_steps(game, cx, u, t, true, melee_rng);
    } else {
        move_steps(game, cx, u, t, false, 3);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pattern {
    Skeleton,
    Wraith,
    Goatman,
}

/// §9.11 Skeleton (2), Wraith (9), Goatman (12), Swarm (19).
fn skeleton_like<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    u: UnitId,
    p: &TickParam,
    pat: Pattern,
) {
    let t = p.target;
    if p.combat {
        if cx.chance(u, cx.aip(p, 3)) {
            if pat == Pattern::Skeleton && !cx.chance(u, cx.aip(p, 4)) {
                a2(game, cx, u, t);
            } else {
                a1(game, cx, u, t);
            }
            return;
        }
    } else if cx.chance(u, cx.aip(p, 1)) {
        match (pat, t) {
            (Pattern::Wraith, Some(tt)) => {
                cx.world.walk_in_radius(game, u, tt, 12, 0);
            }
            (Pattern::Wraith, None) => {}
            _ => {
                walk_to(game, cx, u, t, 7);
            }
        }
        return;
    }
    idle(game, cx, u, cx.aip(p, 2));
}

/// §9.12 Andariel.
fn andariel<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, p: &TickParam) {
    let t = p.target;
    let (s1, m1) = cx.skill(p, 1);
    let (s2, m2) = cx.skill(p, 2);
    if p.combat {
        if s1 >= 0 && cx.chance(u, cx.aip(p, 1)) {
            use_skill(game, cx, u, m1, s1, at(t));
        } else {
            a1(game, cx, u, t);
        }
        return;
    }
    if cx.chance(u, cx.aip(p, 2)) {
        idle(game, cx, u, 5);
        return;
    }
    if cx.chance(u, cx.aip(p, 3)) {
        if s1 >= 0 && cx.chance(u, cx.aip(p, 4)) {
            use_skill(game, cx, u, m1, s1, at(t));
            return;
        } else if s2 >= 0 {
            use_skill(game, cx, u, m2, s2, at(t));
            return;
        }
    }
    set_velocity(cx, u, 1, 0, 0);
    walk_to(game, cx, u, t, 7);
}

/// §9.13 CorruptArcher.
fn corrupt_archer<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    u: UnitId,
    p: &TickParam,
) {
    let t = p.target;
    let (s, e, _) = cx.world.secondary_target(game, u);
    // 1.
    let Some(s) = s else {
        if (cx.world.seed(u).step() % 100) > 49 {
            idle(game, cx, u, cx.aip(p, 3));
        } else {
            circle(game, cx, u, t, 3, false);
        }
        return;
    };
    let st = Some(s);
    // 2.
    if !p.combat && cx.ai_state_set(u) {
        a1(game, cx, u, st);
        return;
    }
    // 3.
    if e < 6 && cx.chance(u, cx.aip(p, 4)) {
        set_velocity(cx, u, 0, 100, 0);
        if escape(game, cx, u, st, 12, true, true) {
            return;
        }
    }
    // 4.
    let aip8 = cx.aip(p, 8);
    if 0 < aip8 && aip8 < e && cx.chance(u, cx.aip(p, 1)) {
        set_velocity(cx, u, 0, 10, 0);
        move_steps(game, cx, u, st, false, aip8);
        return;
    }
    // 5.
    let aip5 = cx.aip(p, 5);
    if e > aip5 {
        set_velocity(cx, u, 0, 100, 0);
        move_steps(game, cx, u, st, true, aip5);
        return;
    }
    // 6.
    if !cx.chance(u, cx.aip(p, 2)) {
        idle(game, cx, u, cx.aip(p, 3));
        return;
    }
    // 7.
    let (s1, m1) = cx.skill(p, 1);
    let (s2, m2) = cx.skill(p, 2);
    let (s3, m3) = cx.skill(p, 3);
    if s2 >= 0 && cx.chance(u, cx.aip(p, 6)) {
        use_skill(game, cx, u, m2, s2, ModeTarget::Unit(s));
    } else if s3 >= 0 && cx.chance(u, cx.aip(p, 7)) {
        use_skill(game, cx, u, m3, s3, ModeTarget::Unit(s));
    } else if s1 < 0 {
        a1(game, cx, u, st);
    } else {
        use_skill(game, cx, u, m1, s1, ModeTarget::Unit(s));
    }
}

/// §9.10 Navi.
///
/// TODO(spec gap): "clamp param 1 at 0 and count it down": read as
/// `max(param1, 0)`, then − 1 while > 0.
fn navi<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, _p: &TickParam) {
    // 1.
    if cx.world.interacting(u) {
        idle(game, cx, u, 10);
        return;
    }
    // 2.
    let (pl, close) = cx.world.nearest_player(game, u);
    let is_player = game
        .lists
        .unit(pl)
        .is_some_and(|e| e.ty == UnitType::Player);
    if is_player && !cx.world.busy(pl) && cx.world.seed(u).roll(3) != 0 && close {
        let p1 = cx.store.control(u).map_or(0, |c| c.params[1]);
        if p1 == 0 {
            if let Some(c) = cx.store.control_mut(u) {
                c.params[1] = 60;
            }
            cx.world.play_sound(game, u, 18, Some(pl));
        } else if let Some(c) = cx.store.control_mut(u) {
            let v = c.params[1].max(0);
            c.params[1] = if v > 0 { v - 1 } else { 0 };
        }
        idle(game, cx, u, 20);
        return;
    }
    // 3.
    town_rogue(game, cx, u);
}

/// §9.10 TownRogue (Navi step 3).
fn town_rogue<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId) {
    let (s, e, _) = cx.world.secondary_target(game, u);
    match s {
        Some(s) if e < 25 => a1(game, cx, u, Some(s)),
        _ => idle(game, cx, u, 50),
    }
}

/// §9.30 Smith (the Smith, hephasto): no draws.
fn smith<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, p: &TickParam) {
    let t = p.target;
    if p.combat {
        a1(game, cx, u, t);
        return;
    }
    let l = cx.world.life_percent(u).clamp(0, 100);
    set_velocity(cx, u, 0, (100 - l) >> 1, 0);
    walk_to(game, cx, u, t, 7);
}

/// §9.30 Griswold.
fn griswold<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, u: UnitId, p: &TickParam) {
    let t = p.target;
    if p.combat {
        if cx.chance(u, 80) {
            a1(game, cx, u, t);
        } else {
            idle(game, cx, u, 10);
        }
    } else if cx.chance(u, 50) {
        walk_to(game, cx, u, t, 7);
    } else {
        idle(game, cx, u, 10);
    }
}
