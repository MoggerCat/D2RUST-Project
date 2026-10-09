// Spec: specs/sim/path-placement.md §2, §3, §5; specs/sim/units.md §2, §3; specs/missiles/missiles.md §R2.3 step 15–18, §R4
//! The unit path record (unit +0x2C) and the path seams of the action
//! adapters that need no game: position (§2.1), room, size and shape
//! (§3), footprints (§5.2), the path setters of the missile set-up
//! (`missiles.md` §R2.3), the cached collision word and the crossed
//! sub-tiles (§R4), the path part of `SUNIT_Add` (§2.5) at allocation
//! and the path free at removal.
//!
//! Each method answers from [`super::PathState`] when the provider is on
//! and from [`Pending`] otherwise (the module doc of [`super`]).

use crate::game::Game;
use crate::path::footprint::{
    add_footprint, make_corpse_footprint, remove_footprint, set_foot_mask,
};
use crate::path::record::{alloc_dynamic_path, TargetUnit};
use crate::path::{
    DynamicKind, FootShape, Footprint, MonsterShape, RemoveRule, StaticPath, UnitPath, UnitShape,
};
use crate::units::{UnitId, UnitType};
use crate::wiring::action::{ActionHooks, Pending, View, WiringError};

/// monstats `Velocity` and the `npc` bit, the walk velocity source of a
/// monster (`pathing.md` §8.1).
pub type MonsterVelocity = (i32, bool);

impl<X: Pending> ActionHooks<X> {
    /// Position in sub-tiles (§2.1): the path's; a unit without a path
    /// reads (0, 0).
    pub fn path_position(&self, unit: UnitId) -> (i32, i32) {
        match &self.paths {
            Some(p) => crate::path::record::unit_position(p.record(unit)),
            None => self.x.position(unit),
        }
    }

    /// The unit has a path (unit +0x2C ≠ null, §2.1).
    pub fn path_has(&self, unit: UnitId) -> bool {
        match &self.paths {
            Some(p) => p.record(unit).is_some(),
            None => self.x.has_path(unit),
        }
    }

    /// Path velocity (+0x7C).
    pub fn path_velocity(&self, unit: UnitId) -> i32 {
        match &self.paths {
            Some(p) => p.dynamic(unit).map_or(0, |d| d.velocity),
            None => self.x.velocity(unit),
        }
    }

    /// `0x00648690` (`pathing.md` §8.1 rule 3).
    pub fn path_set_velocity(&mut self, unit: UnitId, v: i32) {
        match &mut self.paths {
            Some(p) => {
                if let Some(d) = p.dynamic_mut(unit) {
                    crate::path::walk::velocity::set_velocity(d, v);
                }
            }
            None => self.x.set_velocity(unit, v),
        }
    }

    /// `0x00648AD0`: target point, target unit cleared.
    pub fn path_set_target_point(&mut self, unit: UnitId, x: i32, y: i32) {
        match &mut self.paths {
            Some(p) => {
                if let Some(d) = p.dynamic_mut(unit) {
                    d.set_target_point(x as u16, y as u16);
                }
            }
            None => self.x.set_target_point(unit, x, y),
        }
    }

    /// `0x00648CE0`: move-test mask (+0x50).
    pub fn path_set_move_mask(&mut self, unit: UnitId, mask: u16) {
        match &mut self.paths {
            Some(p) => {
                if let Some(d) = p.dynamic_mut(unit) {
                    d.move_mask = mask;
                }
            }
            None => self.x.set_move_mask(unit, mask),
        }
    }

    /// Path acceleration (+0x88) and maximum velocity (+0x84)
    /// (`missiles.md` §R2.3 step 18).
    pub fn path_set_acceleration(&mut self, unit: UnitId, accel: i32, max_velocity: i32) {
        match &mut self.paths {
            Some(p) => {
                if let Some(d) = p.dynamic_mut(unit) {
                    d.acceleration = accel;
                    d.max_velocity = max_velocity;
                }
            }
            None => self.x.set_acceleration(unit, accel, max_velocity),
        }
    }

    /// The collision word the last step cached in the path (+0x54, the
    /// collided-with mask, `missiles.md` §R4 step 6).
    pub fn path_cached_word(&self, unit: UnitId) -> Option<u16> {
        match &self.paths {
            Some(p) => p.dynamic(unit).map(|d| d.collided_mask),
            None => self.x.cached_collision_word(unit),
        }
    }

    /// `0x00648F40`: the sub-tiles the last step crossed, in path order
    /// (the saved steps, +0x1D4 / +0x1D8).
    pub fn path_crossed(&self, unit: UnitId) -> Vec<(i32, i32)> {
        match &self.paths {
            Some(p) => p.dynamic(unit).map_or_else(Vec::new, |d| {
                d.saved_steps
                    .iter()
                    .take(d.saved_count as usize)
                    .map(|s| (i32::from(s.x), i32::from(s.y)))
                    .collect()
            }),
            None => self.x.crossed_subtiles(unit),
        }
    }

    /// The path free at removal (`units.md` §2 +0x2C "freed at removal",
    /// §3.2 footprint removal `0x00649F50` for types 0–3). Removal clears
    /// the footprint with force (`path-placement.md` §5.3 rule 4), so the
    /// §5.2 mode conditions do not apply.
    pub(crate) fn path_free(&mut self, unit: UnitId, ty: Option<UnitType>, mode: u32) {
        let Some(p) = self.paths.as_mut() else {
            return;
        };
        if matches!(
            ty,
            Some(UnitType::Player | UnitType::Monster | UnitType::Object | UnitType::Missile)
        ) {
            if let Some((fp, _)) = footprint_of(p.record(unit), ty, mode, None) {
                remove_footprint(&mut self.drlg, &fp, RemoveRule::Other, true);
            }
        }
        p.records.remove(&unit);
        p.history.remove(&unit);
        p.setup.remove(&unit);
    }
}

/// The footprint of a unit with path `rec` (§5.2: players and monsters
/// by pattern, missiles, items and tiles by size, objects by box) and
/// its removal rule. Objects need their objects.txt shape; `None` when
/// the shape is unknown or the unit has no path.
pub(crate) fn footprint_of(
    rec: Option<&UnitPath>,
    ty: Option<UnitType>,
    mode: u32,
    object: Option<crate::path::ObjectShape>,
) -> Option<(Footprint, RemoveRule)> {
    let rec = rec?;
    let ty = ty?;
    let (room, (x, y)) = (rec.room(), rec.position());
    let fp = |shape, mask| Footprint {
        room,
        x,
        y,
        shape,
        mask,
    };
    Some(match (ty, rec) {
        (UnitType::Player, UnitPath::Dynamic(d)) => (
            fp(FootShape::Pattern(d.pattern), d.foot_mask),
            RemoveRule::Player { mode },
        ),
        (UnitType::Monster, UnitPath::Dynamic(d)) => (
            fp(FootShape::Pattern(d.pattern), d.foot_mask),
            RemoveRule::Monster { mode },
        ),
        (UnitType::Missile, UnitPath::Dynamic(d)) => (
            fp(FootShape::Size(d.unit_size), d.foot_mask),
            RemoveRule::Other,
        ),
        (UnitType::Object, UnitPath::Static(_)) => {
            let o = object?;
            (
                fp(
                    FootShape::Box {
                        size_x: o.size_x,
                        size_y: o.size_y,
                    },
                    o.foot_mask(),
                ),
                RemoveRule::Object { mode, shape: o },
            )
        }
        (UnitType::Item, UnitPath::Static(_)) => (
            fp(
                FootShape::Size(UnitShape::Item.size()),
                UnitShape::Item.foot_mask(None),
            ),
            RemoveRule::Other,
        ),
        (UnitType::Tile, UnitPath::Static(_)) => (
            fp(
                FootShape::Size(UnitShape::Tile.size()),
                UnitShape::Tile.foot_mask(None),
            ),
            RemoveRule::Other,
        ),
        _ => return None,
    })
}

impl<X: Pending> View<'_, X> {
    /// The §3 inputs of a unit: monstats / monstats2 of a monster
    /// (`SizeX` signed, `BaseId`, `flying`, `opendoors`, `npc`, `inTown`,
    /// `interact`, unit flag bit 31), missiles `Size`, the objects.txt
    /// row of an object ([`View::object_path_shape`]).
    pub fn path_shape(&self, unit: UnitId) -> Option<UnitShape> {
        let r = self.units.get(unit)?;
        Some(match r.ty {
            UnitType::Player => UnitShape::Player,
            UnitType::Monster => UnitShape::Monster(self.monster_shape(unit)?),
            UnitType::Missile => UnitShape::Missile {
                size: i32::from(self.h.tables.missiles.get(r.class as usize)?.size),
            },
            UnitType::Item => UnitShape::Item,
            UnitType::Tile => UnitShape::Tile,
            UnitType::Object => UnitShape::Object(self.object_path_shape(unit)?),
        })
    }

    /// An object's §3 / §5.2 inputs from its objects.txt row (`SizeX`,
    /// `SizeY`, `IsDoor`, `BlocksVis`, `BlockMissile`, `SubClass`,
    /// `HasCollision0..7`); `None` without the object state or row.
    pub fn object_path_shape(&self, unit: UnitId) -> Option<crate::path::ObjectShape> {
        let r = self.units.get(unit).filter(|r| r.ty == UnitType::Object)?;
        let st = self.h.objects.as_ref()?;
        let o = st.tables.object(r.class as u16).ok()?;
        Some(crate::wiring::action::objects::object_shape(o))
    }

    /// A monster's §2.4 / §3 inputs.
    pub fn monster_shape(&self, unit: UnitId) -> Option<MonsterShape> {
        let r = self.units.get(unit)?;
        let t = &self.h.tables.combat;
        let m = t.monstats.get(r.class as usize)?;
        let size_x = t
            .monstats2
            .get(usize::from(m.monstatsex))
            .map_or(0, |m2| i32::from(m2.sizex as i8));
        Some(MonsterShape {
            size_x,
            base_id: u32::from(m.baseid),
            flying: m.flying,
            open_doors: m.opendoors,
            npc: m.npc,
            in_town: m.intown,
            unit_flag_31: r.flags & 0x8000_0000 != 0,
            interact: m.interact,
        })
    }

    /// Unit size `0x00620510` (§3); without the provider [`Pending::size`].
    pub fn path_size(&self, unit: UnitId) -> i32 {
        if self.h.paths.is_none() {
            return self.h.x.size(unit);
        }
        self.path_shape(unit).map_or(0, |s| s.size())
    }

    /// `0x00648B90`: target unit, its type and GUID (+0x58..+0x60).
    pub fn path_set_target_unit(&mut self, unit: UnitId, target: UnitId) {
        let Some(p) = self.h.paths.as_mut() else {
            self.h.x.set_target_unit(unit, target);
            return;
        };
        let Some(t) = self.units.get(target) else {
            return;
        };
        let tu = TargetUnit {
            unit: target,
            ty: t.ty,
            guid: t.guid,
        };
        if let Some(d) = p.dynamic_mut(unit) {
            d.target_unit = Some(tu);
        }
    }

    /// `0x00648C30` (§5.3 rule 1): footprint mask change with restamp
    /// (missiles by size).
    pub fn path_set_foot_mask(&mut self, unit: UnitId, mask: u16) {
        let missile = self
            .units
            .get(unit)
            .is_some_and(|r| r.ty == UnitType::Missile);
        let h = &mut *self.h;
        let Some(p) = h.paths.as_mut() else {
            h.x.set_footprint_mask(unit, mask);
            return;
        };
        if let Some(d) = p.dynamic_mut(unit) {
            set_foot_mask(&mut h.drlg, d, missile, mask);
        }
    }

    /// The path part of `SUNIT_Add` `0x00554850` (§2.5) for a unit just
    /// allocated at (x, y) in its list room (`units.md` §3.1 step 8;
    /// the list part — room list, hash, update queue — is
    /// `UnitLists::add_unit`'s, already done). Without the provider:
    /// [`Pending::place`].
    ///
    /// Per type: player, monster, missile: dynamic path allocation
    /// (§2.4, stamps the footprint when a room is given); item in mode 3
    /// and tile: static set and footprint; object: static set and its
    /// box footprint when `HasCollision[mode]` ≠ 0; item in another mode:
    /// no path.
    ///
    /// The three dynamic allocations pass `set0x10` = 0 (§2.5).
    // The monster calls after the allocation (`0x005735A0`, then
    // `0x00573780`; `init.md` §4.1) run in [`View::add_allocated`]
    // (`monster_added`). The corpse path settings of `units.md` §3.1
    // step 8 run at the end (player mode 0 / 17, monster mode 0 / 12
    // without monstats2 `deadCol`).
    pub fn path_place(&mut self, game: &Game, unit: UnitId, x: i32, y: i32) {
        if self.h.paths.is_none() {
            self.h.x.place(unit, x, y);
            return;
        }
        let Some(r) = self.units.get(unit) else {
            return;
        };
        let (ty, mode) = (r.ty, r.mode);
        let room = game.lists.unit(unit).and_then(|e| e.room());
        let kind = match ty {
            UnitType::Player => Some(DynamicKind::Player),
            UnitType::Monster => self.monster_shape(unit).map(DynamicKind::Monster),
            UnitType::Missile => match self.path_shape(unit) {
                Some(UnitShape::Missile { size }) => Some(DynamicKind::Missile { size }),
                _ => None,
            },
            _ => None,
        };
        // `units.md` §3.1 step 8: `0x0063EA40` and not monstats2 flag 19
        // (`deadCol`, `0x004638A0(class, 0x13)`).
        let dead_body = match ty {
            UnitType::Player => matches!(mode, 0 | 17),
            UnitType::Monster => {
                matches!(mode, 0 | 12)
                    && !self
                        .units
                        .get(unit)
                        .and_then(|r| self.h.tables.combat.monstats.get(r.class as usize))
                        .and_then(|m| {
                            self.h
                                .tables
                                .combat
                                .monstats2
                                .get(usize::from(m.monstatsex))
                        })
                        .is_some_and(|m2| m2.deadcol)
            }
            _ => false,
        };
        let object = self.object_path_shape(unit);
        let h = &mut *self.h;
        let p = h.paths.as_mut().expect("checked above");
        let rec = match (ty, kind) {
            (_, Some(kind)) => {
                match alloc_dynamic_path(&p.tables, &mut h.drlg, kind, unit, room, x, y, false) {
                    Ok(d) => UnitPath::Dynamic(Box::new(d)),
                    Err(e) => {
                        h.errors.push(WiringError::Path(e));
                        return;
                    }
                }
            }
            (UnitType::Item, _) if mode != 3 => return,
            (UnitType::Object | UnitType::Item | UnitType::Tile, _) => {
                let mut s = StaticPath::default();
                s.set(room, x, y);
                let rec = UnitPath::Static(s);
                // An object is stamped only when `HasCollision[mode]` ≠ 0
                // (§2.5), in the mode its init left (`objects.md` §3).
                let stamp = object.is_none_or(|o| o.collides_in(mode));
                if let Some((fp, _)) = footprint_of(Some(&rec), Some(ty), mode, object) {
                    if stamp {
                        add_footprint(&mut h.drlg, &fp);
                    }
                }
                rec
            }
            // A monster class without a monstats row or a missile class
            // without a missiles row: no path.
            _ => return,
        };
        p.records.insert(unit, rec);
        if dead_body {
            if let Some(d) = p.dynamic_mut(unit) {
                make_corpse_footprint(&mut h.drlg, d);
            }
        }
    }

    /// The unit's footprint and removal rule (`0x00649400` /
    /// `0x00649560` inputs, §5.2).
    pub(crate) fn path_footprint(&self, unit: UnitId) -> Option<(Footprint, RemoveRule)> {
        let r = self.units.get(unit)?;
        let p = self.h.paths.as_ref()?;
        footprint_of(
            p.record(unit),
            Some(r.ty),
            r.mode,
            self.object_path_shape(unit),
        )
    }

    /// Footprint add `0x00649400` at the unit's stored position.
    pub fn path_add_footprint(&mut self, unit: UnitId) {
        if let Some((fp, _)) = self.path_footprint(unit) {
            add_footprint(&mut self.h.drlg, &fp);
        }
    }

    /// Footprint remove `0x00649560(unit, force)` at the stored position;
    /// whether it cleared.
    pub fn path_remove_footprint(&mut self, unit: UnitId, force: bool) -> bool {
        match self.path_footprint(unit) {
            Some((fp, rule)) => remove_footprint(&mut self.h.drlg, &fp, rule, force),
            None => false,
        }
    }

    /// monstats `Velocity` and `npc` of a monster (`pathing.md` §8.1).
    pub fn monster_velocity(&self, unit: UnitId) -> MonsterVelocity {
        self.units
            .get(unit)
            .and_then(|r| self.h.tables.combat.monstats.get(r.class as usize))
            .map_or((0, false), |m| (i32::from(m.velocity), m.npc))
    }

    /// charstats `WalkVelocity`, `RunVelocity`, `RunDrain` of a player.
    pub fn charstats_velocity(&self, unit: UnitId) -> (i32, i32, i32) {
        self.units
            .get(unit)
            .and_then(|r| self.h.tables.combat.charstats.get(r.class as usize))
            .map_or((0, 0, 0), |c| {
                (
                    i32::from(c.walkvelocity),
                    i32::from(c.runvelocity),
                    i32::from(c.rundrain),
                )
            })
    }
}

impl<X: Pending> View<'_, X> {
    /// `0x00622AA0(a, b, mask)` (`render/draw-order-2.md` §15.1,
    /// [`crate::path::line::units_line_blocked`]) on the path records and
    /// the DRLG rooms: `a`'s room (`0x00620BB0`), the path positions
    /// (§2.1) and sizes (§3). `None` without the path provider (the
    /// caller keeps its [`Pending`] answer).
    pub fn units_line_blocked(&self, game: &Game, a: UnitId, b: UnitId, mask: u16) -> Option<bool> {
        self.h.paths.as_ref()?;
        let end = |u: UnitId| {
            let (x, y) = self.h.path_position(u);
            crate::path::line::LineUnit {
                room: game.lists.unit(u).and_then(|e| e.room()),
                x,
                y,
                size: self.path_size(u),
            }
        };
        Some(crate::path::line::units_line_blocked(
            &self.h.drlg,
            &end(a),
            &end(b),
            mask,
        ))
    }
}
