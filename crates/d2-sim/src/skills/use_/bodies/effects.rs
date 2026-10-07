// Spec: specs/skills/bodies.md §6–§8, specs/skills/bodies-2.md §2–§5, specs/skills/bodies-2b.md §6–§8
//! The calls of the batch 2 and 3 bodies into systems other specs own
//! that return nothing the bodies read: one [`BodyEffect`] per 1.14d
//! callee, sent through [`super::BodyWorld::effect`], and the path
//! operations ([`PathOp`], [`super::BodyWorld::path_op`]). Each variant
//! names its address; the provider performs the call (`Pending` on the
//! wired host where no provider exists yet).

/// A call with no result the bodies read (`U` unit, `I` item, `R` room).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyEffect<U, I, R> {
    // ---- monsters (`monsters/ai.md`, `monsters/init.md`)
    /// Owner data `0x0058F030(game, m, owner GUID, owner type, a, b)` (no
    /// owner: GUID −1, type 6).
    OwnerData {
        m: U,
        owner: Option<U>,
        a: i32,
        b: i32,
    },
    /// Umod `0x005A4850(game, m, umod, arg)`.
    Umod { m: U, umod: i32, arg: i32 },
    /// AI params `0x005B0D70(control, p0, p1, p2)` of m's AI control (−666
    /// = unchanged).
    AiParams { m: U, p0: i32, p1: i32, p2: i32 },
    /// `0x00573780(game, m)` (`monsters/ai.md` §1).
    AiRefresh(U),
    /// An AI command {type, x, y, tx, ty} made current (`0x0058EF40`,
    /// `monsters/ai.md` §8).
    AiCommand {
        m: U,
        kind: i32,
        x: i32,
        y: i32,
        tx: i32,
        ty: i32,
    },
    /// Leash owner `0x005DD330(m, owner)`.
    LeashOwner { m: U, owner: Option<U> },
    /// Target override `0x00573090(m, kind, guid)`.
    TargetOverride { m: U, kind: i32, guid: i32 },
    /// Target override cleared `0x00573120(m)`.
    ClearTargetOverride(U),
    /// Player mode request, unit form `0x00580A70` (`sim/pathing.md` §1.2)
    /// with the entry of `skill`, `mode`, the target's type / GUID,
    /// re-entry 1.
    UnitModeRequest {
        u: U,
        skill: i32,
        mode: i32,
        target: U,
    },
    /// Component byte k of m's monster data (+0x04 + k).
    Component { m: U, k: i32, v: i32 },
    /// Alignment `0x005543B0(u, a, v)`.
    Alignment { u: U, a: i32, v: i32 },
    /// The pack dissolved `0x0058F260(u)`.
    DissolvePack(U),
    /// `u` leaves its leader's minion list `0x0058F160(u)`.
    LeaveLeader(U),
    /// `m` prepended to the leader's minion list `0x0058F100(game, leader,
    /// m)`.
    AddMinion { leader: U, m: U },
    /// Summon equipment `0x005D6B60(game, owner, m, skill, L, ilvl, 0)`.
    Equipment {
        owner: U,
        m: U,
        skill: i32,
        lvl: i32,
        ilvl: i32,
    },
    /// Pet list add `0x00575D90(game, owner, pet, t, max)` (`sim/pets.md`
    /// §2).
    PetAdd { owner: U, pet: U, t: i32, max: i32 },
    /// Target-node insert after the head of game list `slot`
    /// (`0x005B1900`; the caller tested m +0xD0, slot and the type).
    NodeInsert { m: U, slot: i32 },
    /// Target-list prepend `0x005B1990(game, u, 0, slot)`.
    NodePrepend { u: U, slot: i32 },
    /// Target-list remove `0x005B1A90(game, u)`.
    NodeRemove(U),
    /// The owner's linked unit GUID := m's (−1 none) (`0x00554040`).
    SetLinked { owner: U, m: Option<U> },
    /// `0x0057CCB0(linked, 1)` on a replaced linked unit (§6.2 step 4).
    KillReplaced(U),
    /// Kill `0x0057CCB0(game, u, a, b)`.
    Kill { u: U, a: i32, b: i32 },
    /// Unit removal `0x00555600(game, u)`.
    RemoveUnit(U),
    /// Room delete record `0x0061A270(room of u, u type, u GUID)`.
    RoomDelete(U),
    /// An item leaves its room (`0x0061A270`, `0x00623830`, `0x0064C370`).
    ItemLeaveRoom(U),
    /// Item unit mode := v (`0x00624690`).
    ItemMode { item: U, mode: i32 },
    /// Equip `0x005606B0(game, m, item GUID, loc, skip 1, none)`.
    Equip { m: U, item: U, loc: i32 },
    /// Free the unit's combat records for `target` (`0x0057C9F0`).
    FreeCombat { u: U, target: U },
    /// Item stat lists toggled `0x00627910(unit, item, on)`.
    ItemLists { u: U, item: I, on: bool },
    /// Item break `0x0055F850(game, unit, item)`.
    BreakItem { u: U, item: I },
    /// Skill select `0x005701B0(u, side, skill, owner)`.
    SelectSkill {
        u: U,
        side: i32,
        skill: i32,
        owner: i32,
    },
    /// `0x0056DEB0(m, skill, lvl)` (`bodies.md` §6.5 step 6): the entry
    /// added when missing, its base level, `0x00646D60`, passive refresh,
    /// a player's resync.
    SetSkill { m: U, skill: i32, lvl: i32 },
    /// `0x00643C50(u, 0, −1)` (`bodies-2.md` Open question 11).
    Op643C50(U),
    /// Source fields +0x94 / +0x98 := (type, GUID), 0 / 0 for none
    /// (`0x00621C30`).
    SourceFields { m: U, owner: Option<U> },
    // ---- messages, sounds, world
    /// S→C 0xA3 record queued `0x00571AA0` (`bodies-2.md` §2.21).
    MsgA3 {
        u: U,
        target: Option<U>,
        skill: i32,
        lvl: i32,
        x: i32,
        y: i32,
        v: i32,
    },
    /// S→C 0xA5 {unit, skill} queued on the unit (`0x00571B70`).
    MsgA5 { u: U, skill: i32 },
    /// S→C 0x7F about m to the unit's client (`0x0053CDF0`).
    AllyInfo { u: U, m: U },
    /// Object operate `0x00584420(game, u, 2, guid, &out)`.
    OperateObject { u: U, object: U },
    /// Auto pickup `0x00563560(game, u, guid, &out)`.
    AutoPickup { u: U, item: U },
    /// Sound event `0x00553380(u, id)`.
    Sound { u: U, id: i32 },
    /// Item drop `0x0056DAB0(game, at, code, quality)` (`bodies-2.md`
    /// §2.4).
    DropItem { at: U, code: [u8; 4], quality: i32 },
    /// Treasure drop `0x005A8000(game, corpse, killer, q)`.
    TreasureDrop { corpse: U, killer: U, q: i32 },
    /// Missile hit handler `0x005ADF10(game, missile, unit, 1)`
    /// (`missiles.md` §R5).
    MissileHit { missile: U, unit: U },
    /// Missile data +0x28 := v (`0x0064A710`).
    MissileData28 { missile: U, v: i32 },
    /// Missile data +0x2C := v (`0x0064A760`).
    MissileData2C { missile: U, v: i32 },
    // ---- rooms and collision (`sim/path-placement.md`)
    /// Pattern stamp `0x0064EA90(room, x, y, u's pattern, mask)`.
    PatternStamp {
        room: R,
        x: i32,
        y: i32,
        u: U,
        mask: u32,
    },
    /// Pattern clear `0x0064EC10(room, x, y, u's pattern, mask)`.
    PatternClear {
        room: R,
        x: i32,
        y: i32,
        u: U,
        mask: u32,
    },
}

/// A path operation (`sim/pathing.md`) on the unit's path (+0x2C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOp<U> {
    /// Move-test mask `0x00648CE0`.
    MoveMask(u32),
    /// Footprint mask `0x00648C30`.
    FootprintMask(u32),
    /// Target point `0x00648AD0`.
    TargetPoint(i32, i32),
    /// Unit path target `0x00620C10` (some) / `0x00648B90(P, 0)` (none).
    TargetUnit(Option<U>),
    /// Path type `0x00648CF0`.
    Type(u8),
    /// Velocity `0x00648690`.
    Velocity(i32),
    /// Compute `0x00649970(P, unit, 0)`; [`super::BodyWorld::path_op`]
    /// returns its result.
    Compute,
    /// Snap to the sub-tile centre `0x00650590`.
    SnapCenter,
    /// Step counts +0x90 / +0x91 := n `0x00648E70`.
    Steps(i32),
    /// `0x00649070(P, v)`.
    Op649070(i32),
    /// `0x00648E40(P, v)`.
    Op648E40(i32),
    /// P +0x14 and +0x16 := 0 `0x00649050`.
    Clear14,
    /// Direction toward (x, y) set on the path (`0x0064FDC0`, `0x006488A0`).
    Face(i32, i32),
    /// Path reset `0x00649CA0` (`client/msg-units.md` §3).
    Reset,
}
