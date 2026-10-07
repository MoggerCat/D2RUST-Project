// Spec: specs/audio/triggers.md §11 (UI sounds), §12 (other fixed requests)
//! UI clicks (no unit, delay 0), the S→C 0x5D sound actions
//! (`0x004A2CB0`) and the fixed ids of §12, whose conditions are the
//! owning features' (open question 10).

use super::Ctx;

/// §11 UI sound ids (`client/ui.md` §B8 owns which control is which).
pub mod id {
    pub const CURSOR_PASS: i32 = 1;
    pub const CURSOR_SELECT: i32 = 2;
    pub const CURSOR_ERROR: i32 = 3;
    pub const CURSOR_BUTTON_CLICK: i32 = 4;
    pub const CURSOR_POINT_DROP: i32 = 5;
    pub const CURSOR_SWITCH: i32 = 6;
    pub const CURSOR_REPAIR_ITEM: i32 = 15;
    pub const CURSOR_HOSTILE: i32 = 16;
}

/// The §11 table: (id, name) (checked against the spec, M05).
pub const UI_SOUNDS: [(i32, &str); 8] = [
    (1, "cursor_pass"),
    (2, "cursor_select"),
    (3, "cursor_error"),
    (4, "cursor_button_click"),
    (5, "cursor_point_drop"),
    (6, "cursor_switch"),
    (15, "cursor_repair_item"),
    (16, "cursor_hostile"),
];

/// A UI sound: request(id, none), delay 0 (§11).
pub fn ui_sound(cx: &mut Ctx, id: i32) {
    cx.req(id, None, 0);
}

/// S→C 0x5D sound actions (`0x004A2CB0`, §11): flags f, code c, value v.
/// First match wins (`client/msg-ui.md` §1 r2): with f bit 0 set only c =
/// 33 plays (237) and the bit-1 rows are not reached (f = 3, c = 33 plays
/// 237 only); the bit-1 rows need bit 0 clear; the f = 0x10 rows need
/// bits 0 and 1 clear.
pub fn ui_action(cx: &mut Ctx, f: u8, c: u8, v: i16) {
    let id = if f & 1 != 0 {
        if c == 33 {
            237
        } else {
            0
        }
    } else if f & 2 != 0 {
        match c {
            4 => 241,
            8 | 15 | 18 | 22 | 35 => 7,
            32 => 217,
            33 => 243,
            _ => 0,
        }
    } else if f == 0x10 {
        match c {
            10 => {
                cx.req(2456, None, 0);
                2474
            }
            33 => i32::from(v),
            _ => 0,
        }
    } else {
        0
    };
    if id != 0 {
        cx.req(id, None, 0);
    }
}

/// §12 fixed ids (on a unit or none as the table says; conditions are
/// the owning features').
pub mod fixed {
    pub const IMPACT_STEAL_LIFE: i32 = 396;
    pub const IMPACT_STEAL_MANA: i32 = 397;
    pub const EVENT_THUNDER_1: i32 = 202;
    pub const ANDARIEL_QUAKE_LOOP: i32 = 452;
    pub const PLAYER_TOWNPORTAL_ENTER: i32 = 2231;
    pub const SHRINE_PORTAL: i32 = 2671;
    pub const MONSTER_DIABLO_TAUNT_EX: i32 = 4640;
    pub const MONSTER_DIABLO_TAUNT_1: i32 = 4638;
    pub const NECROMANCER_CORPSEEXP_1: i32 = 2458;
    pub const MINION_DEATHS: [i32; 4] = [1308, 1311, 1314, 1317];
    pub const FIREBALL_IMPACT: i32 = 2419;
    pub const DRUID_POD_DEATH: i32 = 790;
    pub const BARBARIAN_LEAP_LAND: i32 = 2517;
    pub const SPIDER_WEB_1: i32 = 1830;
}
