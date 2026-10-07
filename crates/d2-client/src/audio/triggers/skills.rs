// Spec: specs/audio/triggers.md §8 (skills, missiles, states), §9 (items)
//! Sound columns of `skills`, `missiles`, `states` and the item tables.
//! Inputs are the columns as signed plain values (the rules test `> 0` /
//! `≥ 0`); the callers read them from the typed tables.

use super::{swing, swing_entry, Ctx, TriggerError, Unit, PLAYER};
use crate::bridge::world::UnitKey;

/// The `skills` columns of §8 r1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillStart {
    /// `stsound`.
    pub stsound: i32,
    /// `stsoundclass`.
    pub stsoundclass: i32,
    /// `stsounddelay` flag.
    pub stsounddelay: bool,
    /// `weaponsnd` flag.
    pub weaponsnd: bool,
    /// `stsuccessonly` flag.
    pub stsuccessonly: bool,
    /// `charclass` (+0x0C).
    pub charclass: i32,
    /// `ItemCastSound`, when cast from an item (item ≠ −1); `None` otherwise.
    pub item_cast_sound: Option<i32>,
}

/// Skill start sounds (`0x004C6140`, §8 r1), after the start function
/// (`start_ok` = it returned non-zero) and the mode set.
pub fn skill_start(
    cx: &mut Ctx,
    u: &Unit,
    sk: &SkillStart,
    start_ok: bool,
) -> Result<(), TriggerError> {
    if !start_ok && sk.stsuccessonly {
        return Ok(());
    }
    let h = u.weapon_hit_class;
    // Delay of r1.1 / r1.3: swing − 3, signed, passed as is.
    let delay = || -> Result<u32, TriggerError> {
        Ok(if sk.stsounddelay {
            (swing(u.speed, h)? as i32).wrapping_sub(3) as u32
        } else {
            0
        })
    };
    // r1.1.
    let mut s = sk.stsound;
    let replaced = matches!(sk.item_cast_sound, Some(x) if x > 0);
    if let (true, Some(x)) = (replaced, sk.item_cast_sound) {
        s = x;
    }
    if s > 0 {
        let d = delay()?;
        cx.req(s, Some(u.key), d);
    }
    // r1.2.
    if sk.weaponsnd {
        let (id, _) = swing_entry(h)?;
        if id != 0 {
            let d = swing(u.speed, h)?;
            cx.req(id, Some(u.key), d);
        }
    }
    // r1.3.
    if sk.stsoundclass > 0 && !replaced && u.unit_type() == PLAYER && u.class == sk.charclass {
        let d = delay()?;
        cx.req(sk.stsoundclass, Some(u.key), d);
    }
    Ok(())
}

/// Skill do / target sounds (§8 r2), after the client do function
/// returned non-zero: `dosound` > 0 on the caster, `tgtsound` > 0 on the
/// target if any.
///
/// TODO(spec: audio/triggers.md open question 5): `dosound a` / `dosound b`.
pub fn skill_do(
    cx: &mut Ctx,
    caster: UnitKey,
    target: Option<UnitKey>,
    dosound: i32,
    tgtsound: i32,
) {
    if dosound > 0 {
        cx.req(dosound, Some(caster), 0);
    }
    if let (true, Some(t)) = (tgtsound > 0, target) {
        cx.req(tgtsound, Some(t), 0);
    }
}

/// Item cast sound `ItemCastSound` ≥ 0 on the caster (`0x004CA060`, §8 r2).
pub fn item_cast(cx: &mut Ctx, caster: UnitKey, item_cast_sound: i32) {
    if item_cast_sound >= 0 {
        cx.req(item_cast_sound, Some(caster), 0);
    }
}

/// `prgsound` on the unit from the progressive function `0x004D9090`
/// (§8 r2). The caller decides when.
pub fn skill_progressive(cx: &mut Ctx, unit: UnitKey, prgsound: i32) {
    cx.req(prgsound, Some(unit), 0);
}

/// Missile `TravelSound` after its client init (`0x004CD540`, §8 r3).
pub fn missile_travel(cx: &mut Ctx, missile: UnitKey, travel_sound: i32) {
    cx.req(travel_sound, Some(missile), 0);
}

/// Missile `HitSound` ≥ 0 when its client hit function returned non-zero
/// (`0x004D2D70`, §8 r3).
pub fn missile_hit(cx: &mut Ctx, missile: UnitKey, hit_sound: i32) {
    if hit_sound >= 0 {
        cx.req(hit_sound, Some(missile), 0);
    }
}

/// Missile `ProgSound` (§8 r3). TODO(spec: audio/triggers.md open
/// question 6): the conditions per client progressive function are the
/// caller's.
pub fn missile_prog(cx: &mut Ctx, missile: UnitKey, prog_sound: i32) {
    cx.req(prog_sound, Some(missile), 0);
}

/// State on (S→C 0xA7/0xA8/0xAA, `0x004D9B20`, §8 r4). `had` = U had the
/// state; `dead` = U is a monster in mode 12 or a player in mode 17.
pub fn state_on(cx: &mut Ctx, unit: UnitKey, had: bool, notondead: bool, dead: bool, onsound: i32) {
    if notondead && dead {
        return;
    }
    if !had && onsound >= 0 {
        cx.req(onsound, Some(unit), 0);
    }
}

/// State off (S→C 0xA9, `0x004D9C30`, §8 r4).
pub fn state_off(cx: &mut Ctx, unit: UnitKey, had: bool, offsound: i32) {
    if had && offsound >= 0 {
        cx.req(offsound, Some(unit), 0);
    }
}

/// An item row's sound columns (`dropsound`, `dropsfxframe`, `usesound`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemSounds {
    pub dropsound: i32,
    pub dropsfxframe: u32,
    pub usesound: i32,
}

/// The rows an item's sounds come from (§9 r2, r4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemRows {
    /// Base row (`weapons` / `armor` / `misc`).
    pub base: ItemSounds,
    /// Item quality (5 set, 7 unique).
    pub quality: u8,
    /// The unique row (quality 7) or set row (quality 5), if any.
    pub special: Option<ItemSounds>,
}

/// §9 r2: default drop frame and the drop volume.
pub const DROP_FRAME_DEFAULT: u32 = 12;
pub const DROP_VOLUME: i32 = 180;
/// §9: fixed item sounds.
pub const ITEM_PICKUP: i32 = 235;
pub const ITEM_FLIPPY: i32 = 216;
pub const ITEM_GOLD: i32 = 221;

impl ItemRows {
    fn special_row(&self) -> Option<&ItemSounds> {
        match self.quality {
            5 | 7 => self.special.as_ref().filter(|r| r.dropsound > 0),
            _ => None,
        }
    }

    /// Drop sound and delay (`0x004C16D0`, §9 r2).
    pub fn drop_sound(&self) -> (i32, u32) {
        let mut id = self.base.dropsound;
        let mut d = match self.base.dropsfxframe {
            0 => DROP_FRAME_DEFAULT,
            f => f,
        };
        if let Some(r) = self.special_row() {
            id = r.dropsound;
            if r.dropsfxframe != 0 {
                d = r.dropsfxframe;
            }
        }
        (id, d)
    }

    /// Place sound (`0x004C1D60`, §9 r3): the drop sound without frame.
    pub fn place_sound(&self) -> i32 {
        self.drop_sound().0
    }

    /// Use sound (`0x004C1E20`, §9 r4; the `dropsound` test is kept,
    /// Edge cases r1).
    pub fn use_sound(&self) -> i32 {
        match self.special_row() {
            Some(r) => r.usesound,
            None if self.base.dropsound > 0 => self.base.usesound,
            None => 0,
        }
    }
}

/// Item mode 4, on cursor (`0x004C1910`, §9 r1).
pub fn item_to_cursor(cx: &mut Ctx) {
    cx.req(ITEM_PICKUP, None, 0);
}

/// Item mode 5, dropping to the ground (`0x004C18B0`, §9 r2).
pub fn item_drop(cx: &mut Ctx, item: UnitKey, rows: &ItemRows) {
    cx.req(ITEM_FLIPPY, Some(item), 0);
    let (id, d) = rows.drop_sound();
    if id > 0 {
        let h = cx.req(id, Some(item), d);
        if h != 0 {
            cx.s.set_volume(h, DROP_VOLUME);
        }
    }
}

/// Item placed in a grid (§9 r3): request(place sound, none).
pub fn item_place(cx: &mut Ctx, rows: &ItemRows) {
    cx.req(rows.place_sound(), None, 0);
}

/// Item used (§9 r4): request(use sound, none).
pub fn item_use(cx: &mut Ctx, rows: &ItemRows) {
    cx.req(rows.use_sound(), None, 0);
}

/// Gold (§9 r5): 221 `item_gold`, none.
pub fn item_gold(cx: &mut Ctx) {
    cx.req(ITEM_GOLD, None, 0);
}
