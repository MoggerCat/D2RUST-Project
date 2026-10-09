// Spec: specs/ui/controls.md (§6 r1–r11, §7 r5)
//! World clicks: the left / right button handlers, the click dispatcher
//! `0x00462D00`, its press / held / release filter, the per-kind steps,
//! the action `0x004625B0` with its senders, the held repeat and the
//! target re-pick. Plain Rust: the client model and the client-side
//! tests the dispatcher reads come in through [`ClickWorld`]; what the
//! click does comes out as [`ClickOut`]s, in 1.14d order. The
//! dispatcher never decides an outcome: every result is a C→S message
//! (or a client sound), the server decides (`CLAUDE.md` hard rule 7).

use crate::bridge::world::UnitKey;

/// Unit types (`client/model.md` §1).
const PLAYER: u8 = 0;
const MONSTER: u8 = 1;
const OBJECT: u8 = 2;
const ITEM: u8 = 4;
const TILE: u8 = 5;

/// The click kinds of §6 r1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    LeftDown = 0,
    LeftHeld = 1,
    LeftUp = 2,
    RightDown = 3,
    RightHeld = 4,
    RightUp = 5,
}

/// The click record flags (`C+0`, §6 r2, r8).
pub mod flag {
    pub const LEFT: u32 = 1;
    pub const RIGHT: u32 = 2;
    pub const PRESS: u32 = 4;
    pub const HELD: u32 = 8;
    pub const RELEASE: u32 = 0x10;
    pub const STAND_STILL: u32 = 0x20;
    pub const RUN: u32 = 0x40;
    pub const PATH: u32 = 0x80;
    pub const TILE: u32 = 0x100;
}

/// The `mods` word of §4.3 r1.
pub mod mods {
    pub const STAND_STILL: u32 = 4;
    pub const RUN: u32 = 8;
}

/// `skills.txt` flag bits the action reads (§6 r8: mask table
/// `0x006CE268`, entry n = 1 << n).
pub mod skill_flag {
    pub const PASSIVE: u32 = 1 << 4;
    pub const IN_TOWN: u32 = 1 << 8;
    pub const TARGETABLE_ONLY: u32 = 1 << 20;
    pub const SEARCH_ENEMY_XY: u32 = 1 << 21;
    pub const SEARCH_ENEMY_NEAR: u32 = 1 << 22;
    pub const SEARCH_OPEN_XY: u32 = 1 << 23;
    pub const TARGET_CORPSE: u32 = 1 << 24;
    pub const TARGET_PET: u32 = 1 << 25;
    pub const TARGET_ALLY: u32 = 1 << 26;
    pub const TARGET_ITEM: u32 = 1 << 27;
    pub const ATTACK_NO_MANA: u32 = 1 << 28;
}

/// `range(P, skill)` (`0x00645460`, `skills/use.md` §3 r6).
pub mod range {
    pub const NONE: u8 = 0;
    pub const H2H: u8 = 1;
    pub const RNG: u8 = 2;
    pub const LOC: u8 = 4;
}

/// The `srvdofunc` of the approach sender (§6 r9.3: Inferno, Arctic
/// Blast).
pub const SRVDOFUNC_APPROACH: i16 = 0x13;
/// The interact code (§6 r9.2; `client/model.md` §8).
pub const CODE_INTERACT: u8 = 0x13;
/// Town portal class (§6 r9.2) and the `just_portaled` state.
pub const TOWN_PORTAL: u32 = 59;
/// Skill id 0 (Attack, §6 r8.4).
pub const ATTACK: u16 = 0;
/// The walk re-send gap of a held walk (§6 r9.6), in client updates.
pub const HELD_WALK_GAP: u32 = 7;
/// The warp re-click gap (§6 r9.2 tile), in ms.
pub const WARP_GAP: u32 = 500;
/// The panel bottom margin of a right click in open mode 2 (§6 r1).
pub const PANEL_BOTTOM: i32 = 47;

/// A skill entry of P's skill list: id and its mode (entry +8,
/// `0x00643860`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillRef {
    pub id: u16,
    pub mode: u32,
}

/// The `skills.txt` columns the action reads (§6 r4, r8, r9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillRowFacts {
    /// Flag bits by [`skill_flag`].
    pub flags: u32,
    /// `range` (+0x14) as the right-down step reads it (§6 r4).
    pub range: u8,
    /// `srvdofunc` (+0x2E).
    pub srvdofunc: i16,
}

impl SkillRowFacts {
    fn has(&self, f: u32) -> bool {
        self.flags & f != 0
    }
}

/// A pending interaction (player data +0x150..+0x15C, §6 r9.2 "pend").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub code: u8,
    pub target: UnitKey,
}

/// What a click does, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClickOut {
    /// `0x00481030(code, P, a, b)` (§6 r7): the client mode request and,
    /// for codes 1–0x11, the C→S message whose id is the code; code 0x13
    /// is the interact (sent by `client/model.md` §8 rule 7).
    Code { code: u8, a: u32, b: u32 },
    /// A C→S message built here (0x17, 0x27, 0x4C, 0x59).
    Send(Vec<u8>),
    /// The player event sound `0x004CB9C0(P, event)`.
    Sound(u16),
    /// The pending interaction was set (`0x00460780`) or cleared.
    Pend(Option<Pending>),
    /// `0x00480E70(U, 1)` and monster data +0x28 |= 1 on an NPC (§6 r9.2
    /// tail), after its path stop.
    NpcHold(UnitKey),
    /// P's path re-targeted (`0x00648B90`, §6 r4 kind 1).
    Retarget,
    /// The hover was dropped (`0x00466DE0`, §6 r10).
    DropHover,
    /// A hover / cursor-unit call by address: `0x00467A70` and
    /// `0x00466FE0` (§6 r2, r8.4), `0x00467410(0)` (kinds 2, 5).
    HoverCall(u32),
}

/// The click globals (§6 r1–r3, r6, r9.2, r9.6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickState {
    /// `[0x007A0650]` left held.
    pub left_held: bool,
    /// `[0x007A0654]` right held.
    pub right_held: bool,
    /// `[0x007A0658]`: an interaction stopped the left repeat.
    pub stop_repeat: bool,
    /// `[0x007A5264]`: one positioned dispatch per loop pass.
    pub latch: bool,
    /// `[0x007A526C]`: 0, or 1 + (a unit was hovered) at the press.
    pub press: u8,
    /// `[0x007A5268]`: the update counter of the last walk (§6 r9.6).
    pub last_walk: u32,
    /// `[0x007A04C8]`: the warp re-click time (§6 r9.2 tile).
    pub warp_after: u32,
}

impl ClickState {
    /// The per-pass latch clear (`0x00462920`, at `0x0044F24C`).
    pub fn end_pass(&mut self) {
        self.latch = false;
    }

    /// The input reset `0x0044DA40` (`client/msg-ui.md` §2 r2.2): left
    /// and right held := 0, so no held repeat follows (the button states
    /// `[0x0070F234]` / `[0x0070F2BC]` := up are the edge's; the hover
    /// clear `0x00466FE0` and `[0x007A066C]`, `[0x007A0670]` are not
    /// modelled).
    pub fn input_reset(&mut self) {
        self.left_held = false;
        self.right_held = false;
    }
}

/// The click record `C` (§6 r2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub flags: u32,
    pub player: UnitKey,
    pub unit: Option<UnitKey>,
    pub x: i32,
    pub y: i32,
    /// +0x14 / +0x18: the walk codes to a point / to a unit (§6 r5).
    pub walk_point: u8,
    pub walk_unit: u8,
    pub skill: Option<SkillRef>,
}

/// What the dispatcher reads (the client model and the client-side
/// tests 1.14d runs; the adapter answers them from the bridge).
pub trait ClickWorld {
    /// `[0x007A061C]` ≠ 0.
    fn in_game(&self) -> bool;
    /// The local player (type 0).
    fn player(&self) -> Option<UnitKey>;
    /// `0x004538D0(9)`: the game menu is open.
    fn game_menu_open(&self) -> bool;
    /// The UI open mode (`render/camera.md` §3).
    fn open_mode(&self) -> u8;
    /// Frame width and height.
    fn frame_size(&self) -> (i32, i32);
    /// `[0x007A521C]`: the bottom of a right panel (open mode 1).
    fn right_panel_bottom(&self) -> i32;
    /// `0x00454970()`: the y limit of the skill codes (§6 r7).
    fn skill_y_limit(&self) -> i32;
    /// The current mouse position.
    fn mouse(&self) -> (i32, i32);
    /// `0x00467A10`: the hovered unit.
    fn hover(&self) -> Option<UnitKey>;
    /// `0x0045AFF0`: screen → world subtile.
    fn to_world(&self, x: i32, y: i32) -> (i32, i32);
    /// A unit's position as r9.6 reads it.
    fn position(&self, u: UnitKey) -> Option<(i32, i32)>;
    fn mode(&self, u: UnitKey) -> u32;
    fn class(&self, u: UnitKey) -> u32;
    /// P's stat 10 (stamina).
    fn stamina(&self) -> i32;
    /// `0x00464600(P, skill)`.
    fn can_act(&self, skill: Option<SkillRef>) -> bool;
    /// `0x00620190` / `0x006201D0`.
    fn left_skill(&self) -> Option<SkillRef>;
    fn right_skill(&self) -> Option<SkillRef>;
    /// The Attack entry (`0x006439F0`).
    fn attack_skill(&self) -> Option<SkillRef>;
    /// The skill's `skills.txt` row (`0x00644140`).
    fn skill_row(&self, id: u16) -> Option<SkillRowFacts>;
    /// `range(P, skill)` (`0x00645460`).
    fn range(&self, skill: SkillRef) -> u8;
    /// The use state of §6 r9.1 (`0x004D9FC0`).
    fn use_state(&self, skill: SkillRef) -> u32;
    /// The refusal sound of a use state (table `0x00711DDC`); `None`: out
    /// of the table (fatal 0x2CB).
    fn refusal_sound(&self, state: u32) -> Option<u16>;
    /// The cursor state, the cursor unit's GUID and the cursor item.
    fn cursor_state(&self) -> u32;
    fn cursor_unit(&self) -> Option<u32>;
    fn cursor_item(&self) -> Option<u32>;
    /// `0x00478F20(0)` ≠ −1, and the cursor unit's mode record +8.
    fn has_hireling(&self) -> bool;
    fn cursor_unit_mode_record(&self) -> Option<i32>;
    /// `0x0061AB00` on U's room.
    fn in_town(&self, u: UnitKey) -> bool;
    /// `0x00464820`.
    fn is_dead(&self, u: UnitKey) -> bool;
    /// U's flag +0xC4 bit 2 (selectable, §6 r8.3.4).
    fn selectable(&self, u: UnitKey) -> bool;
    /// The hostility test r9.7 (`0x00465C60(P, U)`).
    fn hostile(&self, u: UnitKey) -> bool;
    /// The monster's `monstats` bits (`npc` bit 0, `interact` bit 1 of
    /// byte +0xD).
    fn monster_npc_interact(&self, u: UnitKey) -> (bool, bool);
    /// Whether the object has an `objects.txt` row (§6 r9.4).
    fn object_has_row(&self, u: UnitKey) -> bool;
    /// `0x00641530(P, U)` unit distance.
    fn distance(&self, u: UnitKey) -> i32;
    /// `0x006416D0(P, U)` path distance.
    fn path_distance(&self, u: UnitKey) -> i32;
    /// `0x00622B50(P, U, 0x804)` = 0.
    fn clear(&self, u: UnitKey) -> bool;
    /// `0x00623660(P, U)`: P stands at the object.
    fn at_object(&self, u: UnitKey) -> bool;
    /// P has state 102 `just_portaled`.
    fn just_portaled(&self) -> bool;
    /// `0x00622C40(P, U, moving)`: P in melee range of U.
    fn melee_range(&self, u: UnitKey) -> bool;
    /// `0x00622D00(P)`: P moves.
    fn moving(&self) -> bool;
    /// `0x004648F0`: P's current target (busy when some).
    fn current_target(&self) -> Option<UnitKey>;
    /// The kind-1 "speed changed" test (§6 r4).
    fn speed_changed(&self, running: bool) -> bool;
    /// P's player data +0x154 (an interaction in progress) and the
    /// pending record.
    fn pending(&self) -> Option<Pending>;
    /// The skill range of the approach (`0x00646CA0(P, row +0x64, …)`).
    fn approach_range(&self, skill: SkillRef) -> i32;
    /// The path compute of r9.8: (x, y) target → the path's end point,
    /// `None`: no path.
    fn walk_path(&self, x: i32, y: i32) -> Option<(i32, i32)>;
    /// P's current path end point (`0x00648A40`, `0x00648A60`).
    fn path_end(&self) -> Option<(i32, i32)>;
    /// `[0x007A0498]`.
    fn update_counter(&self) -> u32;
    /// `[0x007A048C]` (ms).
    fn now_ms(&self) -> u32;
    /// P's direction (`0x00620100`) & 0xFF.
    fn direction(&self) -> u8;
    /// The re-pick searches of r11.2–r11.4: `None` = U unchanged (r11.2,
    /// r11.4) / U := none (r11.3 returns its own answer).
    fn search_enemy_near(
        &self,
        row: SkillRowFacts,
        skill: SkillRef,
        u: Option<UnitKey>,
        x: &mut i32,
        y: &mut i32,
    ) -> Option<UnitKey>;
    fn search_enemy_xy(&self, row: SkillRowFacts) -> Option<UnitKey>;
    fn search_open_xy(
        &self,
        u: Option<UnitKey>,
        x: &mut i32,
        y: &mut i32,
    ) -> Option<Option<UnitKey>>;
}

/// The point codes (C→S `[x u16][y u16]`, `sim/client-messages.tsv`).
pub fn is_point_code(code: u8) -> bool {
    matches!(code, 1 | 3 | 5 | 8 | 0x0C | 0x0F)
}

/// The C→S bytes of `0x00481030(code, P, a, b)` (§6 r7): point codes
/// `[code][a u16][b u16]` (`0x004785D0`), unit codes `[code][a u32][b
/// u32]` (`0x004786A0`); codes outside 1–0x11 send nothing here.
pub fn code_bytes(code: u8, a: u32, b: u32) -> Option<Vec<u8>> {
    if !(1..=0x11).contains(&code) || code == 0x0B {
        return None;
    }
    let mut m = vec![code];
    if is_point_code(code) {
        m.extend_from_slice(&(a as u16).to_le_bytes());
        m.extend_from_slice(&(b as u16).to_le_bytes());
    } else {
        m.extend_from_slice(&a.to_le_bytes());
        m.extend_from_slice(&b.to_le_bytes());
    }
    Some(m)
}

fn send_u32(id: u8, a: u32, b: Option<u32>) -> Vec<u8> {
    let mut m = vec![id];
    m.extend_from_slice(&a.to_le_bytes());
    if let Some(b) = b {
        m.extend_from_slice(&b.to_le_bytes());
    }
    m
}

/// One world click (§6 r1 handlers + r2 dispatcher). `at` is the event
/// position (kinds 0, 3, 5) or `None` for the current mouse (1, 2, 4).
/// Returns the dispatcher's result.
pub fn click(
    st: &mut ClickState,
    w: &dyn ClickWorld,
    kind: Kind,
    at: Option<(i32, i32)>,
    mods_word: u32,
    out: &mut Vec<ClickOut>,
) -> bool {
    // §6 r1: in game, with a local player.
    if !w.in_game() {
        return false;
    }
    let Some(p) = w.player() else {
        return false;
    };
    let (x, y) = at.unwrap_or_else(|| w.mouse());
    match kind {
        Kind::LeftUp => st.left_held = false,
        Kind::RightUp => st.right_held = false,
        Kind::RightDown => {
            let (fw, fh) = w.frame_size();
            let mode = w.open_mode();
            if mode == 3 {
                return false;
            }
            let in_left = mode == 2 && 0 < x && x < fw / 2 && 0 < y && y < fh - PANEL_BOTTOM;
            let in_right = mode == 1 && fw / 2 < x && x < fw && 0 < y && y < w.right_panel_bottom();
            if in_left || in_right {
                return false;
            }
        }
        _ => {}
    }
    let r = dispatch(st, w, p, kind, x, y, mods_word, out);
    match kind {
        Kind::LeftDown => st.left_held = r,
        Kind::RightDown => st.right_held = r,
        Kind::LeftUp => st.stop_repeat = false,
        _ => {}
    }
    r
}

/// The held repeat (§6 r6), once per client loop pass before the
/// receive: kind 1 while left is held and the repeat is not stopped,
/// then kind 4 while right is held.
pub fn held_repeat(
    st: &mut ClickState,
    w: &dyn ClickWorld,
    mods_word: u32,
    out: &mut Vec<ClickOut>,
) {
    if st.left_held && !st.stop_repeat {
        click(st, w, Kind::LeftHeld, None, mods_word, out);
    }
    if st.right_held {
        click(st, w, Kind::RightHeld, None, mods_word, out);
    }
}

/// The dispatcher `0x00462D00` (§6 r2) and the per-kind steps (r3, r4).
#[allow(clippy::too_many_arguments)]
fn dispatch(
    st: &mut ClickState,
    w: &dyn ClickWorld,
    p: UnitKey,
    kind: Kind,
    sx: i32,
    sy: i32,
    mods_word: u32,
    out: &mut Vec<ClickOut>,
) -> bool {
    if w.game_menu_open() {
        return false;
    }
    if st.latch && !(sx == 0 && sy == 0) {
        return false;
    }
    st.latch = true;
    let hover = w.hover();
    let (mut x, mut y) = w.to_world(sx, sy);
    if let Some(u) = hover.filter(|u| matches!(u.unit_type, OBJECT | ITEM)) {
        if let Some(pos) = w.position(u) {
            (x, y) = pos;
        }
    }
    let mut flags = match kind {
        Kind::LeftDown | Kind::LeftHeld | Kind::LeftUp => flag::LEFT,
        _ => flag::RIGHT,
    };
    flags |= match kind {
        Kind::LeftDown | Kind::RightDown => flag::PRESS,
        Kind::LeftHeld | Kind::RightHeld => flag::HELD,
        _ => flag::RELEASE,
    };
    if mods_word & mods::STAND_STILL != 0 {
        flags |= flag::STAND_STILL;
    }
    if mods_word & mods::RUN != 0 && w.stamina() != 0 {
        flags |= flag::RUN;
    }
    match kind {
        Kind::LeftDown | Kind::RightDown => {
            out.push(ClickOut::HoverCall(if hover.is_some() {
                0x0046_7A70
            } else {
                0x0046_6FE0
            }));
        }
        Kind::LeftUp | Kind::RightUp => out.push(ClickOut::HoverCall(0x0046_6FE0)),
        _ => {}
    }
    let mut c = Click {
        flags,
        player: p,
        unit: hover,
        x,
        y,
        walk_point: 0,
        walk_unit: 0,
        skill: None,
    };
    // r3. A release only clears the press latch; the release kinds' own
    // step (the hireling / cursor-unit call and, for kind 2, the path end
    // `0x00462370`) still runs and returns 0 (Reading of r3 with r4).
    if !filter(st, w, &c) && c.flags & flag::RELEASE == 0 {
        return false;
    }
    match kind {
        Kind::LeftDown => {
            c.skill = w.left_skill();
            if !w.can_act(c.skill) {
                cursor_sends(w, out);
                if let Some(item) = w.cursor_item() {
                    out.push(ClickOut::Send(send_u32(0x17, item, None)));
                }
                return false;
            }
            c.flags |= flag::PATH;
            action(st, w, &mut c, out);
            true
        }
        Kind::LeftHeld => {
            c.skill = w.left_skill();
            if !w.can_act(c.skill) {
                return false;
            }
            let running = c.flags & flag::RUN != 0;
            let changed = w.speed_changed(running);
            if let Some(u) = c.unit {
                if w.current_target() != Some(u) || changed {
                    out.push(ClickOut::Retarget);
                    c.flags |= flag::PATH;
                }
            } else if c.flags & flag::STAND_STILL == 0 {
                if w.pending().is_some() {
                    return true;
                }
                c.flags |= flag::PATH;
            }
            action(st, w, &mut c, out);
            true
        }
        Kind::LeftUp => {
            release_step(w, out);
            // `0x00462370`: P not busy in mode 2 / 6 → walk code 1, mode 3
            // → run code 3, with P's path end point.
            let mode = w.mode(p);
            if w.current_target().is_none() {
                let c = match mode {
                    2 | 6 => Some(1),
                    3 => Some(3),
                    _ => None,
                };
                if let (Some(c), Some((ex, ey))) = (c, w.path_end()) {
                    code(out, c, ex as u32, ey as u32);
                }
            }
            false
        }
        Kind::RightDown | Kind::RightHeld => {
            c.skill = w.right_skill();
            if !w.can_act(c.skill) {
                return false;
            }
            let h2h = c
                .skill
                .and_then(|s| w.skill_row(s.id))
                .is_some_and(|r| r.range == range::H2H);
            if h2h && c.flags & flag::RUN == 0 {
                c.flags |= flag::PATH;
            }
            action(st, w, &mut c, out);
            true
        }
        Kind::RightUp => {
            release_step(w, out);
            false
        }
    }
}

/// The hireling / cursor-unit step of kinds 2 and 5 (`0x00467410(0)`).
fn release_step(w: &dyn ClickWorld, out: &mut Vec<ClickOut>) {
    if w.has_hireling() && w.cursor_unit().is_some() && w.cursor_unit_mode_record() == Some(2) {
        out.push(ClickOut::HoverCall(0x0046_7410));
    }
}

/// Cursor states 6 and 8 (§6 r4 kind 0, r8.1).
fn cursor_sends(w: &dyn ClickWorld, out: &mut Vec<ClickOut>) {
    match w.cursor_state() {
        6 => {
            let g = w.cursor_unit().unwrap_or(u32::MAX);
            out.push(ClickOut::Send(send_u32(0x27, g, Some(g))));
        }
        8 => out.push(ClickOut::Send(send_u32(0x4C, u32::MAX, None))),
        _ => {}
    }
}

/// The filter `0x00462930` (§6 r3).
fn filter(st: &mut ClickState, w: &dyn ClickWorld, c: &Click) -> bool {
    if c.flags & flag::RELEASE != 0 {
        st.press = 0;
        return false;
    }
    if c.flags & flag::PRESS != 0 {
        st.press = 1 + u8::from(c.unit.is_some());
        return true;
    }
    if st.press == 0 {
        return false;
    }
    match c.unit {
        None => true,
        Some(u) => {
            matches!(u.unit_type, PLAYER | MONSTER | TILE) && !matches!(w.mode(c.player), 2 | 3 | 6)
        }
    }
}

/// `0x00481030` through the click record.
fn code(out: &mut Vec<ClickOut>, code: u8, a: u32, b: u32) {
    out.push(ClickOut::Code { code, a, b });
}

fn pend(out: &mut Vec<ClickOut>, code: u8, target: UnitKey) {
    out.push(ClickOut::Pend(Some(Pending { code, target })));
}

/// The walk to a point `0x00461840(x, y, code)` (r9.8): the path compute;
/// no path → nothing; else the code with the path's end point.
fn walk_to(w: &dyn ClickWorld, at: (i32, i32), c: u8, out: &mut Vec<ClickOut>) -> bool {
    match w.walk_path(at.0, at.1) {
        Some((ex, ey)) => {
            code(out, c, ex as u32, ey as u32);
            true
        }
        None => false,
    }
}

/// The skill codes `0x00461700(onUnit)` (§6 r7).
fn skill_code(w: &dyn ClickWorld, c: &Click, on_unit: bool, out: &mut Vec<ClickOut>) {
    if w.mouse().1 > w.skill_y_limit() {
        return;
    }
    let held = c.flags & flag::HELD != 0;
    let ss = c.flags & flag::STAND_STILL != 0;
    let right = c.flags & flag::RIGHT != 0;
    if on_unit {
        let (t, g) = c
            .unit
            .map_or((6, u32::MAX), |u| (u32::from(u.unit_type), u.guid));
        let id = match (right, ss, held) {
            (false, false, false) => 0x06,
            (false, false, true) => 0x09,
            (false, true, false) => 0x07,
            (false, true, true) => 0x0A,
            (true, false, false) => 0x0D,
            (true, false, true) => 0x10,
            (true, true, false) => 0x0E,
            (true, true, true) => 0x11,
        };
        code(out, id, t, g);
    } else {
        let id = match (right, held) {
            (false, false) => 0x05,
            (false, true) => 0x08,
            (true, false) => 0x0C,
            (true, true) => 0x0F,
        };
        code(out, id, c.x as u32, c.y as u32);
    }
}

/// The use check `0x004610C0(&s)` (§6 r9.1). Returns (passes, s).
fn use_check(w: &dyn ClickWorld, c: &mut Click, out: &mut Vec<ClickOut>) -> (bool, u32) {
    if Some(c.player) != w.player() {
        return (true, 0);
    }
    let Some(skill) = c.skill else {
        return (false, 0);
    };
    let Some(row) = w.skill_row(skill.id) else {
        return (false, 0);
    };
    let mut s = w.use_state(skill);
    if matches!(s, 1 | 2 | 4) && row.has(skill_flag::ATTACK_NO_MANA) {
        c.skill = w.attack_skill();
        s = c.skill.map_or(s, |k| w.use_state(k));
    }
    if matches!(s, 0 | 5) {
        return (true, s);
    }
    if let Some(sound) = w.refusal_sound(s) {
        if sound != 0 {
            out.push(ClickOut::Sound(sound));
        }
    }
    (false, s)
}

/// The action `0x004625B0(C)` (§6 r5, r8).
fn action(st: &mut ClickState, w: &dyn ClickWorld, c: &mut Click, out: &mut Vec<ClickOut>) {
    let right = c.flags & flag::RIGHT != 0;
    let ss = c.flags & flag::STAND_STILL != 0;
    let run = c.flags & flag::RUN != 0;
    // r8.1: the set-up `0x004621D0`.
    let rng = c.skill.map_or(range::NONE, |s| w.range(s));
    if let Some(skill) = c.skill {
        if right && rng != range::H2H {
            let Some(row) = w.skill_row(skill.id) else {
                return;
            };
            if !row.has(skill_flag::IN_TOWN) && w.in_town(c.player) {
                out.push(ClickOut::Sound(0x13));
                return;
            }
            if row.has(skill_flag::PASSIVE) {
                return;
            }
        }
    }
    (c.walk_point, c.walk_unit) = if run { (3, 4) } else { (1, 2) };
    if w.pending().is_some() {
        out.push(ClickOut::Pend(None));
    }
    let Some(skill) = c.skill else {
        return;
    };
    let row = w.skill_row(skill.id);
    repick(w, c, row, ss || (right && c.unit.is_none()), out);
    cursor_sends(w, out);
    // r8.2: unit or point.
    let mut item_skill = false;
    let on_unit = match (row, c.unit) {
        (Some(row), Some(u)) => {
            let mut to_point = false;
            if right && rng != range::H2H {
                match u.unit_type {
                    OBJECT | ITEM => {
                        if row.has(skill_flag::TARGET_ITEM) {
                            item_skill = true;
                        } else {
                            to_point = true;
                        }
                    }
                    TILE => {
                        c.flags |= flag::TILE;
                        to_point = true;
                    }
                    _ => {}
                }
            }
            if !to_point
                && u.unit_type == MONSTER
                && w.is_dead(u)
                && !row.has(skill_flag::TARGET_CORPSE)
            {
                to_point = true;
            }
            !to_point
        }
        _ => false,
    };
    if on_unit {
        unit_action(st, w, c, row.expect("checked"), rng, item_skill, out);
    } else {
        point_action(st, w, c, rng, out);
    }
}

/// r8.3: on a unit.
#[allow(clippy::too_many_arguments)]
fn unit_action(
    st: &mut ClickState,
    w: &dyn ClickWorld,
    c: &mut Click,
    row: SkillRowFacts,
    rng: u8,
    item_skill: bool,
    out: &mut Vec<ClickOut>,
) {
    let u = c.unit.expect("unit action");
    let (t, g) = (u.unit_type, u.guid);
    let right = c.flags & flag::RIGHT != 0;
    let ss = c.flags & flag::STAND_STILL != 0;
    // 3.1.
    if !item_skill && matches!(t, OBJECT | ITEM | TILE) && right && rng != range::H2H {
        return;
    }
    let skill = c.skill.expect("action has a skill");
    // 3.2.
    let mode_zero = skill.mode == 0;
    if mode_zero && ss {
        return;
    }
    if !mode_zero {
        // 3.3.
        if row.has(skill_flag::TARGET_ITEM) {
            if use_check(w, c, out).0 {
                skill_code(w, c, true, out);
            }
            return;
        }
        // 3.4.
        let act = w.selectable(u)
            && (row.has(skill_flag::TARGET_PET)
                || row.has(skill_flag::TARGET_ALLY)
                || w.hostile(u));
        if act {
            match t {
                OBJECT => {
                    if !ss {
                        object_sender(st, w, c, out);
                    }
                }
                PLAYER => {
                    if w.in_town(u) && w.in_town(c.player) {
                        town_player(w, c, out);
                    } else {
                        attack(w, c, row, rng, out);
                    }
                }
                _ => {
                    if w.in_town(u) && !row.has(skill_flag::IN_TOWN) {
                        return;
                    }
                    attack(w, c, row, rng, out);
                }
            }
            return;
        }
        // 3.5.
        if ss {
            if use_check(w, c, out).0 {
                skill_code(w, c, false, out);
            }
            return;
        }
    }
    // 3.6.
    if t != MONSTER {
        interact(st, w, c, out);
    } else if c.flags & flag::PRESS != 0 {
        if w.monster_npc_interact(u).1 {
            interact(st, w, c, out);
        } else {
            code(out, c.walk_unit, u32::from(t), g);
        }
    }
}

/// r8.4: on a point.
fn point_action(
    st: &mut ClickState,
    w: &dyn ClickWorld,
    c: &mut Click,
    rng: u8,
    out: &mut Vec<ClickOut>,
) {
    let right = c.flags & flag::RIGHT != 0;
    let ss = c.flags & flag::STAND_STILL != 0;
    if ss || (right && rng != range::H2H) {
        let (ok, s) = use_check(w, c, out);
        if !ok {
            return;
        }
        let attack_no_mana = c.skill.is_some_and(|k| k.id == ATTACK) && s == 1 && !ss;
        if !attack_no_mana {
            skill_code(w, c, false, out);
            out.push(ClickOut::HoverCall(0x0046_7A70));
            return;
        }
    }
    if walk_clamp(st, w, c) && walk_to(w, (c.x, c.y), c.walk_point, out) {
        out.push(ClickOut::HoverCall(0x0046_7A70));
    }
}

/// The walk clamp `0x004623C0(P)` (§6 r9.6).
pub fn walk_clamp(st: &mut ClickState, w: &dyn ClickWorld, c: &mut Click) -> bool {
    let Some((px, py)) = w.position(c.player) else {
        return false;
    };
    let (mut dx, mut dy) = (c.x - px, c.y - py);
    if dx.abs() >= 0x100 || dy.abs() >= 0x100 || (dx == 0 && dy == 0) {
        return false;
    }
    let (ax, ay) = (dx.abs(), dy.abs());
    let len = ((ax.max(ay) * 0x3D7 + ax.min(ay) * 0x197) >> 10).max(1);
    let run = c.flags & flag::RUN != 0;
    let t = if c.flags & flag::PRESS != 0 {
        if run {
            5
        } else {
            3
        }
    } else if c.flags & flag::HELD != 0 {
        if run {
            5
        } else {
            4
        }
    } else {
        0
    };
    if len < t {
        // (float32)(t / len), x87 multiply, `__ftol2` truncation.
        let f = t as f32 / len as f32;
        dx = (dx as f32 * f) as i32;
        dy = (dy as f32 * f) as i32;
    }
    c.x = px + dx;
    c.y = py + dy;
    let n = w.update_counter();
    if c.player.unit_type == PLAYER
        && c.flags & flag::HELD != 0
        && matches!(w.mode(c.player), 2 | 3 | 6)
        && n.wrapping_sub(st.last_walk) < HELD_WALK_GAP
    {
        return false;
    }
    st.last_walk = n;
    true
}

/// The interact sender `0x00461DC0(T, g)` (§6 r9.2).
fn interact(st: &mut ClickState, w: &dyn ClickWorld, c: &Click, out: &mut Vec<ClickOut>) {
    let u = c.unit.expect("interact target");
    let (t, g) = (u32::from(u.unit_type), u.guid);
    let held = c.flags & flag::HELD != 0;
    let d = w.distance(u);
    let clear = w.clear(u);
    let pending_same = w.pending().is_some_and(|p| p.target == u);
    match u.unit_type {
        PLAYER => {
            if d <= 4 && clear {
                code(out, CODE_INTERACT, t, g);
            } else {
                code(out, c.walk_unit, t, g);
                pend(out, CODE_INTERACT, u);
            }
        }
        OBJECT => {
            if held {
                return;
            }
            let notify = !(w.class(u) == TOWN_PORTAL && w.just_portaled());
            if w.at_object(u) && clear {
                if notify {
                    code(out, CODE_INTERACT, t, g);
                    st.stop_repeat = true;
                }
                return;
            }
            if pending_same {
                return;
            }
            let at = w.position(u).unwrap_or((c.x, c.y));
            walk_to(w, at, c.walk_point, out);
            if notify {
                pend(out, CODE_INTERACT, u);
                st.stop_repeat = true;
            }
        }
        ITEM => {
            if held {
                return;
            }
            if d <= 4 && clear {
                code(out, CODE_INTERACT, t, g);
                st.stop_repeat = true;
                return;
            }
            if pending_same {
                return;
            }
            code(out, c.walk_unit, t, g);
            pend(out, CODE_INTERACT, u);
            st.stop_repeat = true;
        }
        TILE => {
            let now = w.now_ms();
            if now < st.warp_after {
                return;
            }
            if d > 4 {
                code(out, c.walk_unit, t, g);
                pend(out, CODE_INTERACT, u);
            } else {
                code(out, CODE_INTERACT, t, g);
            }
            st.warp_after = now.wrapping_add(WARP_GAP);
        }
        _ => {
            // Monster (reach 2 with `interact`, else 5), missile / other
            // (reach 5), then the tail.
            let (npc, inter) = if u.unit_type == MONSTER {
                w.monster_npc_interact(u)
            } else {
                (false, false)
            };
            let reach = if u.unit_type == MONSTER && inter {
                2
            } else {
                5
            };
            if u.unit_type == MONSTER && npc && inter {
                if let Some((x, y)) = w.position(u) {
                    let mut m = vec![0x59];
                    m.extend_from_slice(&t.to_le_bytes());
                    m.extend_from_slice(&g.to_le_bytes());
                    m.extend_from_slice(&(x as u32).to_le_bytes());
                    m.extend_from_slice(&(y as u32).to_le_bytes());
                    out.push(ClickOut::Send(m));
                }
                out.push(ClickOut::NpcHold(u));
            }
            if d > reach {
                code(out, c.walk_unit, t, g);
                pend(out, CODE_INTERACT, u);
            } else {
                code(out, CODE_INTERACT, t, g);
            }
        }
    }
}

/// The attack sender `0x00461C70(T, g)` (§6 r9.3).
fn attack(w: &dyn ClickWorld, c: &mut Click, row: SkillRowFacts, rng: u8, out: &mut Vec<ClickOut>) {
    if !use_check(w, c, out).0 {
        return;
    }
    let Some(u) = c.unit else {
        return;
    };
    if w.position(u).is_none() {
        return;
    }
    if w.melee_range(u) {
        skill_code(w, c, true, out);
        return;
    }
    if row.srvdofunc == SRVDOFUNC_APPROACH {
        approach(w, c, out);
        return;
    }
    if matches!(rng, range::H2H | range::LOC) {
        if c.flags & flag::STAND_STILL != 0 {
            skill_code(w, c, false, out);
        } else {
            code(out, c.walk_unit, u32::from(u.unit_type), u.guid);
            if c.flags & flag::LEFT != 0 {
                pend(out, 6, u);
            }
            if c.flags & flag::RIGHT != 0 {
                pend(out, 0xD, u);
            }
        }
        return;
    }
    skill_code(w, c, true, out);
}

/// The approach `0x00461B40` (§6 r9.9).
fn approach(w: &dyn ClickWorld, c: &Click, out: &mut Vec<ClickOut>) {
    if c.flags & flag::STAND_STILL != 0 {
        skill_code(w, c, true, out);
        return;
    }
    let u = c.unit.expect("approach target");
    let skill = c.skill.expect("approach skill");
    let range = w.approach_range(skill);
    let dist = w.path_distance(u);
    if dist <= range {
        skill_code(w, c, true, out);
        return;
    }
    if c.flags & flag::LEFT != 0 {
        pend(out, 6, u);
    }
    if c.flags & flag::RIGHT != 0 {
        pend(out, 0xD, u);
    }
    let (Some((px, py)), Some((ux, uy))) = (w.position(c.player), w.position(u)) else {
        return;
    };
    let x = px + (ux - px) * (dist - range) / dist;
    let y = py + (uy - py) * (dist - range) / dist;
    walk_to(w, (x, y), c.walk_point, out);
}

/// The object sender `0x00461890(g)` (§6 r9.4). An object without an
/// `objects.txt` row is fatal 0x4C4 in 1.14d: reported as nothing sent.
fn object_sender(st: &mut ClickState, w: &dyn ClickWorld, c: &Click, out: &mut Vec<ClickOut>) {
    let u = c.unit.expect("object target");
    if w.position(u).is_none() || !w.object_has_row(u) {
        return;
    }
    let _ = st;
    let notify = !(w.class(u) == TOWN_PORTAL && w.just_portaled());
    if w.at_object(u) && w.clear(u) {
        if notify {
            code(out, CODE_INTERACT, u32::from(OBJECT), u.guid);
        }
        return;
    }
    let at = w.position(u).expect("checked");
    walk_to(w, at, c.walk_point, out);
    if notify {
        pend(out, CODE_INTERACT, u);
    }
}

/// The town player sender `0x004619E0` (§6 r9.5).
fn town_player(w: &dyn ClickWorld, c: &Click, out: &mut Vec<ClickOut>) {
    let u = c.unit.expect("player target");
    if w.distance(u) < 3 && w.clear(u) {
        code(out, CODE_INTERACT, 0, u.guid);
    } else {
        code(out, c.walk_unit, 0, u.guid);
        pend(out, CODE_INTERACT, u);
    }
}

/// The nudge `0x004C51E0` (§6 r11.1).
pub fn nudge(p: (i32, i32), direction: u8) -> (i32, i32) {
    const A: [i32; 8] = [0, -1, -2, -1, 0, 1, 2, 1];
    const B: [i32; 8] = [2, 1, 0, -1, -2, -1, 0, 1];
    let d = usize::from(direction >> 3);
    (p.0 + A[d], p.1 + B[d])
}

/// The target re-pick `0x00467880(&x, &y, F, force)` (§6 r10).
fn repick(
    w: &dyn ClickWorld,
    c: &mut Click,
    row: Option<SkillRowFacts>,
    force: bool,
    out: &mut Vec<ClickOut>,
) {
    let row = row.unwrap_or_default();
    if let Some(u) = c.unit {
        if u.unit_type == PLAYER && w.is_dead(u) {
            return;
        }
        if w.is_dead(u) && !row.has(skill_flag::TARGET_CORPSE) {
            out.push(ClickOut::DropHover);
            if let Some(pos) = w.position(u) {
                (c.x, c.y) = pos;
            }
            c.unit = None;
        }
    } else if Some((c.x, c.y)) == w.position(c.player) {
        (c.x, c.y) = nudge((c.x, c.y), w.direction());
    }
    let held_left = c.flags & flag::HELD != 0 && c.flags & flag::LEFT != 0;
    if held_left && c.flags & flag::STAND_STILL == 0 && w.moving() {
        return;
    }
    let Some(skill) = c.skill else {
        return;
    };
    if w.use_state(skill) != 0 {
        return;
    }
    if row.has(skill_flag::SEARCH_ENEMY_NEAR) {
        if let Some(n) = w.search_enemy_near(row, skill, c.unit, &mut c.x, &mut c.y) {
            c.unit = Some(n);
        }
    } else if c.unit.is_none() && row.has(skill_flag::SEARCH_ENEMY_XY) && force {
        c.unit = w.search_enemy_xy(row);
    } else if row.has(skill_flag::SEARCH_OPEN_XY) {
        if let Some(u) = w.search_open_xy(c.unit, &mut c.x, &mut c.y) {
            c.unit = u;
        }
    }
}
