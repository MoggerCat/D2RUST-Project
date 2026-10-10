// Spec: specs/ui/controls.md (§6 r1–r11), specs/client/model.md (§8 rule 7)
//! The world-click dispatcher ([`crate::controls::click`]) against the
//! client model: [`ModelClick`] answers its questions from the
//! [`ClientWorld`] and the frame's view facts, [`apply`] turns its
//! outputs into C→S messages on the model's `outgoing` list (the bridge
//! sends them, `client/bridge.md` §4) and the interact sender of
//! `client/model.md` §8 rule 7.
//!
//! What the model does not hold yet is answered by a stated reading,
//! each marked:
//! - no hover model (`0x00467A10`, `client/model.md` hover): the play
//!   preview hovers monsters near the click ([`super::combat`], d2rs-own,
//!   unverified); hostility and melee range are its answers too;
//! - no client path record: see [`ModelClick::walk_path`];
//! - [`ModelClick::skill_row`] reads the client `skills` rows' flag
//!   columns and `range`; an id outside the table answers `None`, which
//!   §6 r8.2 reads as a point click.
//!
//! The `mods` word of §4.3 r1 comes from [`RunMods`] (commands 34–36).
//! In the `play` preview the local player's position the dispatcher reads
//! may be the predicted one ([`ModelClick::local_at`],
//! [`super::predict`]).

use crate::controls::click::{
    self, ClickOut, ClickState, ClickWorld, Kind, Pending, SkillRef, SkillRowFacts,
};
use crate::rules::camera::{Camera, ClientPos, FrameSize, OpenMode};

use super::dispatch::HandlerError;
use super::objects::interact;
use super::output::{Output, Outputs};
use super::world::{ClientWorld, ModelInputs, UnitKey, PLAYER};

/// The frame's view facts the dispatcher reads (§6 r1, r2, r7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClickView {
    pub size: FrameSize,
    pub open_mode: u8,
    /// `[0x007A521C]`.
    pub right_panel_bottom: i32,
    /// `0x00454970()`.
    pub skill_y_limit: i32,
    /// The current mouse position.
    pub mouse: (i32, i32),
    /// UI 9 (game menu) open.
    pub game_menu_open: bool,
    /// The `play` preview picks the hover target ([`super::hover::pick`],
    /// d2rs-own, unverified); `false`: no hover model (strict).
    pub pick: bool,
    /// The frame's shake `(dx, dy)` (`render/camera.md` §8; the frame
    /// anchor's): the pick inverts the shaken camera
    /// (`seams/world-screen.md` §2.6). `(0, 0)` when no shake runs.
    pub shake: (i32, i32),
    /// The hover target the previous pass's draw left (`Some(None)`: no
    /// unit): 1.14d reads the hovered unit while it draws (`0x00467A10`), so
    /// a press sees the cursor of the pass before it, and a press posted in
    /// the same pass as its move is a point click (`specs/tools/
    /// scenario-diff.md` §2 r5). `None`: the pick reads the click's own
    /// position (strict tests, d2rs-own).
    pub prev_hover: Option<Option<UnitKey>>,
}

/// The client model as the dispatcher reads it.
pub struct ModelClick<'a> {
    pub world: &'a ClientWorld,
    pub inputs: &'a ModelInputs,
    pub view: ClickView,
    /// The local player's predicted precise position (16.16 sub-tiles,
    /// [`super::predict::Predict::position`]); `None`: the model's cell.
    /// d2rs-own, unverified. PROVISIONAL (client/model.md OQ2; REC-51):
    /// in the preview the server sends the walker nothing, so the model
    /// cell stays at the walk's start while the view follows the
    /// prediction; the camera and the walk clamp read the prediction so a
    /// click lands where the player sees it.
    pub local_at: Option<(u32, u32)>,
}

/// The modifier keys of the world click (`ui/controls.md` §3 commands
/// 34 CfgRun, 35 CfgRunLock, 36 CfgStandStill; §4.3 r1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunMods {
    /// Run held (command 34 down / up).
    pub run_held: bool,
    /// The run lock (command 35 toggles it).
    pub run_lock: bool,
    /// Stand Still held (command 36 down / up).
    pub stand_still: bool,
}

impl RunMods {
    /// Command 35 (`0x00469060`): run lock := not run lock.
    pub fn toggle_run(&mut self) {
        self.run_lock = !self.run_lock;
    }

    /// The `mods` word passed to `0x00462D00` (§4.3 r1): 8 when Run held
    /// or the run lock is set (no inversion), plus 4 when Stand Still is
    /// held.
    pub fn word(self) -> u32 {
        let mut m = 0;
        if self.run_held || self.run_lock {
            m |= click::mods::RUN;
        }
        if self.stand_still {
            m |= click::mods::STAND_STILL;
        }
        m
    }
}

/// `0x00464600(P, skill)` (§6 r4 kind 0): 0 when P holds a cursor item
/// or is not a player, or in mode 0, 4, 7–12, 17 or 19; mode 13 with
/// class 3, mode 14 with class 6, modes 15–16 with classes 4–6; mode 18
/// (sequence) as `0x004645B0` decides ([`sequence_can_act`]).
/// `used`: P's used skill's `(anim, seqinput)` (skill list +0x10), `None`
/// when there is none; `event_index`: unit +0x38 >> 8.
pub fn can_act(
    is_player: bool,
    cursor_item: bool,
    mode: u32,
    class: u32,
    used: Option<(u8, u8)>,
    event_index: u32,
) -> bool {
    if !is_player || cursor_item {
        return false;
    }
    if mode == 18 {
        return sequence_can_act(used, event_index);
    }
    !matches!(
        (mode, class),
        (0 | 4 | 7..=12 | 17 | 19, _) | (13, 3) | (14, 6) | (15 | 16, 4..=6)
    )
}

/// `0x004645B0` (`ui/controls.md` §6 r4): no used skill → act; its `anim`
/// not 18 → act; `seqinput` 0 or 255 → no; else act iff the frame event
/// index has reached `seqinput`.
pub fn sequence_can_act(used: Option<(u8, u8)>, event_index: u32) -> bool {
    match used {
        None => true,
        Some((anim, _)) if anim != 18 => true,
        Some((_, 0 | 255)) => false,
        Some((_, seq)) => event_index >= u32::from(seq),
    }
}

impl ModelClick<'_> {
    /// The local player's own position (`seams/movement-prediction.md`
    /// §2.9 r1): the one the caller read for the frame ([`Self::local_at`]),
    /// else the model's ([`ClientWorld::local_position`]).
    fn own_position(&self) -> Option<(u32, u32)> {
        self.world.local()?;
        self.local_at.or_else(|| self.world.local_position())
    }

    pub(super) fn camera(&self) -> Option<Camera> {
        let (px, py) = self.own_position()?;
        let at = crate::rules::camera::moving_to_client(px, py);
        let mode = OpenMode::new(self.view.open_mode).unwrap_or(OpenMode::NONE);
        Some(Camera::new(self.view.size, mode, at, self.view.shake))
    }

    fn skill(&self, left: bool) -> Option<SkillRef> {
        let list = self.world.local()?.skills.as_ref()?;
        let i = if left { list.left } else { list.right }?;
        let e = list.entries.get(i)?;
        Some(SkillRef {
            id: e.skill,
            mode: e.mode,
        })
    }
}

/// Screen → world subtile (`0x0045AFF0`): the inverse of the unit draw
/// (`render/camera.md` §4) and of the static projection (§2).
/// PROVISIONAL (ui/controls.md §6 r2, REC-514: measured on the
/// `a1-walk-*` scenes): screen (sx, sy) → client (sx + cx_u − shift_x,
/// sy + cy_u − 4) → subtile ((px + 2·py) / 32, (2·py − px) / 32)
/// floored: the unit draw's inverse (`sy + cy_u − 8`) four rows down.
pub fn screen_to_world(cam: &Camera, sx: i32, sy: i32) -> (i32, i32) {
    let p = ClientPos {
        x: sx + cam.unit.x - cam.view.shift_x,
        y: sy + cam.unit.y - 4,
    };
    (
        (p.x + 2 * p.y).div_euclid(32),
        (2 * p.y - p.x).div_euclid(32),
    )
}

impl ClickWorld for ModelClick<'_> {
    fn in_game(&self) -> bool {
        self.world.in_game
    }
    fn player(&self) -> Option<UnitKey> {
        self.world
            .local_player
            .filter(|k| k.unit_type == PLAYER && self.world.units.contains_key(k))
    }
    fn game_menu_open(&self) -> bool {
        self.view.game_menu_open
    }
    fn open_mode(&self) -> u8 {
        self.view.open_mode
    }
    fn frame_size(&self) -> (i32, i32) {
        (self.view.size.width, self.view.size.height)
    }
    fn right_panel_bottom(&self) -> i32 {
        self.view.right_panel_bottom
    }
    fn skill_y_limit(&self) -> i32 {
        self.view.skill_y_limit
    }
    fn mouse(&self) -> (i32, i32) {
        self.view.mouse
    }
    fn hover(&self) -> Option<UnitKey> {
        // TODO(spec: client/model.md hover `0x00467A10`): no hover model;
        // The preview's pick (any unit under the cursor, screen space) and,
        // without it, the monster hover; both d2rs-own, unverified.
        if let Some(h) = self.view.prev_hover {
            return h;
        }
        if self.view.pick {
            if let Some(k) = super::hover::pick(self.world, &self.camera()?, self.view.mouse) {
                return Some(k);
            }
        }
        let (x, y) = self.view.mouse;
        super::combat::hover_at(self.world, self.inputs, self.to_world(x, y))
    }
    fn to_world(&self, x: i32, y: i32) -> (i32, i32) {
        self.camera().map_or((0, 0), |c| screen_to_world(&c, x, y))
    }
    fn position(&self, u: UnitKey) -> Option<(i32, i32)> {
        if self.world.local_player == Some(u) {
            let (x, y) = self.own_position()?;
            return Some(((x >> 16) as i32, (y >> 16) as i32));
        }
        let u = self.world.units.get(&u)?;
        let (x, y) = u.position?;
        Some((i32::from(x), i32::from(y)))
    }
    fn mode(&self, u: UnitKey) -> u32 {
        self.world.units.get(&u).map_or(0, |u| u.mode)
    }
    fn class(&self, u: UnitKey) -> u32 {
        self.world.units.get(&u).map_or(0, |u| u.class)
    }
    fn stamina(&self) -> i32 {
        self.world.local().map_or(0, |p| p.stat(10))
    }
    fn can_act(&self, _skill: Option<SkillRef>) -> bool {
        let Some(p) = self.world.local() else {
            return false;
        };
        let used = p
            .skills
            .as_ref()
            .and_then(|l| l.entries.get(l.current?))
            .map(|e| {
                let r = self.inputs.tables.skills.get(usize::from(e.skill));
                r.map_or((0, 0), |r| (r.anim, r.seqinput))
            });
        can_act(
            p.key.unit_type == PLAYER,
            self.world.use_cursor.is_some(),
            p.mode,
            p.class,
            used,
            p.event_index,
        )
    }
    fn left_skill(&self) -> Option<SkillRef> {
        self.skill(true)
    }
    fn right_skill(&self) -> Option<SkillRef> {
        self.skill(false)
    }
    fn attack_skill(&self) -> Option<SkillRef> {
        let list = self.world.local()?.skills.as_ref()?;
        list.entries
            .iter()
            .find(|e| e.skill == click::ATTACK)
            .map(|e| SkillRef {
                id: e.skill,
                mode: e.mode,
            })
    }
    fn skill_row(&self, id: u16) -> Option<SkillRowFacts> {
        // The client `skills` row when the table has it; otherwise the
        // preview's flag-less row (ui/controls.md §6 r8: a click on a picked
        // unit takes the unit path). d2rs-own, unverified (D1).
        super::combat::row_facts(self.inputs, id)
            .or_else(|| self.view.pick.then(SkillRowFacts::default))
    }
    fn range(&self, skill: SkillRef) -> u8 {
        super::combat::range_of(self.inputs, self.world.local(), skill.id)
    }
    /// The client use state `0x004D9FC0` of P's entry of `skill`
    /// ([`super::use_state::use_state`]): the hand entry of that id, else
    /// the native one; no entry → 3.
    fn use_state(&self, skill: SkillRef) -> u32 {
        let Some(p) = self.world.local() else {
            return super::use_state::code::DISABLED;
        };
        let Some(list) = p.skills.as_ref() else {
            return super::use_state::code::DISABLED;
        };
        let hand = [list.left, list.right]
            .into_iter()
            .flatten()
            .filter_map(|i| list.entries.get(i))
            .find(|e| e.skill == skill.id);
        let e = hand.or_else(|| list.native(skill.id).map(|i| &list.entries[i]));
        e.map_or(super::use_state::code::DISABLED, |e| {
            super::use_state::use_state(self.world, self.inputs, p.key, e)
        })
    }
    fn refusal_sound(&self, _state: u32) -> Option<u16> {
        None
    }
    fn cursor_state(&self) -> u32 {
        0
    }
    fn cursor_unit(&self) -> Option<u32> {
        None
    }
    fn cursor_item(&self) -> Option<u32> {
        self.world.use_cursor.map(|c| c.item.guid)
    }
    fn has_hireling(&self) -> bool {
        false
    }
    fn cursor_unit_mode_record(&self) -> Option<i32> {
        None
    }
    fn in_town(&self, u: UnitKey) -> bool {
        super::modes::in_town(self.world, u)
    }
    fn is_dead(&self, u: UnitKey) -> bool {
        self.world.units.get(&u).is_some_and(|u| u.is_dead())
    }
    fn selectable(&self, u: UnitKey) -> bool {
        self.world.units.get(&u).is_some_and(|u| u.flag_4)
    }
    fn hostile(&self, u: UnitKey) -> bool {
        super::combat::hostile(self.world, self.inputs, u)
    }
    /// The `monstats` `npc` / `interact` bits of U's class
    /// ([`super::world::MonsterClass`]); no row: neither.
    fn monster_npc_interact(&self, u: UnitKey) -> (bool, bool) {
        self.world
            .units
            .get(&u)
            .and_then(|u| self.inputs.tables.monsters.get(u.class as usize))
            .and_then(|c| c.as_ref())
            .map_or((false, false), |c| (c.npc, c.interact))
    }
    fn object_has_row(&self, u: UnitKey) -> bool {
        self.world
            .units
            .get(&u)
            .is_some_and(|u| (u.class as usize) < self.inputs.tables.objects.len())
    }
    /// `0x00641530(P, U)` (`sim/pathing.md` §9.5, the server's own
    /// function: the item, NPC and player decisions of §6 r9.2 and the
    /// server's walk-or-act test of the same message agree on it), from
    /// the local player's own cell; sizes `sim/path-placement.md` §3 (a
    /// monster's `monstats2` `SizeX`).
    fn distance(&self, u: UnitKey) -> i32 {
        let size = |c: &super::world::ClientUnit| match c.key.unit_type {
            super::world::MONSTER => self
                .inputs
                .tables
                .monsters
                .get(c.class as usize)
                .and_then(|r| r.as_ref())
                .map_or(0, |r| i32::from(r.size_x)),
            _ => super::objects::unit_size(c, &self.inputs.objclient.rows),
        };
        match (
            self.world.units.get(&u),
            self.world.local(),
            self.own_position(),
        ) {
            (Some(u), Some(p), Some((x, y))) => super::objects::unit_distance_at(
                u.cell(),
                size(u),
                ((x >> 16) as u16, (y >> 16) as u16),
                size(p),
            ),
            _ => i32::MAX,
        }
    }
    fn path_distance(&self, u: UnitKey) -> i32 {
        let rows = &self.inputs.objclient.rows;
        // From the local player's own cell, as its position
        // (`seams/movement-prediction.md` §2.9 r2).
        match (
            self.world.units.get(&u),
            self.world.local(),
            self.own_position(),
        ) {
            (Some(u), Some(p), Some((x, y))) => super::objects::distance_at(
                u.cell(),
                super::objects::unit_size(u, rows),
                ((x >> 16) as u16, (y >> 16) as u16),
                super::objects::unit_size(p, rows),
            ),
            _ => i32::MAX,
        }
    }
    fn clear(&self, _u: UnitKey) -> bool {
        true
    }
    fn at_object(&self, _u: UnitKey) -> bool {
        false
    }
    fn just_portaled(&self) -> bool {
        self.world.local().is_some_and(|p| p.states.contains(&102))
    }
    fn melee_range(&self, u: UnitKey) -> bool {
        super::combat::melee_range(self.world, u)
    }
    fn moving(&self) -> bool {
        false
    }
    fn current_target(&self) -> Option<UnitKey> {
        None
    }
    fn speed_changed(&self, _running: bool) -> bool {
        false
    }
    fn pending(&self) -> Option<Pending> {
        None
    }
    fn approach_range(&self, _skill: SkillRef) -> i32 {
        0
    }
    /// PROVISIONAL (ui/controls.md §6 r9.8; controls-0001): the client
    /// holds no path record or collision for the path compute
    /// `0x00649970`, so the path's end point is the clamped target itself
    /// (the server computes the real path from the C→S point).
    fn walk_path(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        Some((x, y))
    }
    fn path_end(&self) -> Option<(i32, i32)> {
        None
    }
    fn update_counter(&self) -> u32 {
        self.world.drlg_updates
    }
    fn now_ms(&self) -> u32 {
        self.inputs.now
    }
    fn direction(&self) -> u8 {
        0
    }
    fn search_enemy_near(
        &self,
        _row: SkillRowFacts,
        _skill: SkillRef,
        _u: Option<UnitKey>,
        _x: &mut i32,
        _y: &mut i32,
    ) -> Option<UnitKey> {
        None
    }
    fn search_enemy_xy(&self, _row: SkillRowFacts) -> Option<UnitKey> {
        None
    }
    fn search_open_xy(
        &self,
        _u: Option<UnitKey>,
        _x: &mut i32,
        _y: &mut i32,
    ) -> Option<Option<UnitKey>> {
        None
    }
}

/// Applies the dispatcher's outputs in order: codes 1–0x11 become their
/// C→S message on `outgoing` (§6 r7), code 0x13 the interact sender
/// (`client/model.md` §8 rule 7), [`ClickOut::Send`] its bytes. A skill
/// code first runs the local player's mode request (`0x00481030` →
/// `0x00480C10(0x15 | 0x16, P, record, 0)`, [`skill_request`]): the
/// attack or cast mode starts at the click, and the client player update
/// ends it ([`super::player_anim`]). The walk codes' mode requests are
/// not applied (the walk prediction moves the player).
/// PROVISIONAL (skills/sequences.md local player rule 1; REC-1002): the
/// gate `0x00480BA0` (current skill without `interrupt` and mode not 1 /
/// 5 → no request, no send) is not run; the dispatcher's
/// [`can_act`] refuses the attack and cast modes before it; settled by a
/// 1.14d click log while walking with a non-interrupt skill selected.
/// Sounds, hover calls and the pending record are returned to the
/// caller (the UI layer).
pub fn apply(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    outs: Vec<ClickOut>,
) -> Result<(Vec<ClickOut>, Vec<Output>), HandlerError> {
    let mut rest = Vec::new();
    let mut outputs = Vec::new();
    for o in outs {
        match o {
            ClickOut::Code {
                code: click::CODE_INTERACT,
                a,
                b,
            } => outputs.extend(interact::send(world, inputs, a as u16, b)?),
            ClickOut::Code { code, a, b } => {
                // The C→S leaves only when the mode request returned
                // non-zero (`skills/sequences.md` local player rule 2).
                let mut send = true;
                if let Some((p, req, record)) = skill_request(world, code, a, b) {
                    let sink = Outputs::default();
                    send = super::modes::mode_request(world, inputs, p, req, record, &sink)?;
                    outputs.extend(sink.take());
                }
                if let Some(m) = click::code_bytes(code, a, b).filter(|_| send) {
                    world.outgoing.push(m);
                }
            }
            ClickOut::Send(m) => world.outgoing.push(m),
            other => rest.push(other),
        }
    }
    Ok((rest, outputs))
}

/// The local player's mode request of a skill code
/// (`skills/sequences.md` local player rule 1, `0x00481030`): point codes
/// 5, 8, 0xC, 0xF → 0x15; unit codes 6, 7, 9, 0xA, 0xD, 0xE, 0x10, 0x11
/// → 0x16; record r0 := the side's skill (left for codes below 0xC),
/// r1 := its owner, r2 / r3 := a / b. `None`: another code, or no local
/// player or skill on that side.
pub fn skill_request(
    world: &ClientWorld,
    code: u8,
    a: u32,
    b: u32,
) -> Option<(UnitKey, u8, [i32; 7])> {
    let req = match code {
        5 | 8 | 0xC | 0xF => 0x15,
        6 | 7 | 9 | 0xA | 0xD | 0xE | 0x10 | 0x11 => 0x16,
        _ => return None,
    };
    let p = world.local()?;
    let list = p.skills.as_ref()?;
    let side = if code < 0xC { list.left } else { list.right };
    let e = list.entries.get(side?)?;
    let record = [
        i32::from(e.skill),
        e.owner as i32,
        a as i32,
        b as i32,
        0,
        0,
        0,
    ];
    Some((p.key, req, record))
}

/// One world click against the model (§6 r1–r2 and [`apply`]).
pub fn world_click(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    st: &mut ClickState,
    view: ClickView,
    kind: Kind,
    at: Option<(i32, i32)>,
    mods: u32,
) -> Result<(Vec<ClickOut>, Vec<Output>), HandlerError> {
    world_click_at(world, inputs, st, view, kind, at, mods, None)
}

/// [`world_click`] with the local player read at `local_at`
/// ([`ModelClick::local_at`]; the `play` preview's predicted position).
#[allow(clippy::too_many_arguments)]
pub fn world_click_at(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    st: &mut ClickState,
    view: ClickView,
    kind: Kind,
    at: Option<(i32, i32)>,
    mods: u32,
    local_at: Option<(u32, u32)>,
) -> Result<(Vec<ClickOut>, Vec<Output>), HandlerError> {
    let mut outs = Vec::new();
    {
        let m = ModelClick {
            world,
            inputs,
            view,
            local_at,
        };
        click::click(st, &m, kind, at, mods, &mut outs);
    }
    apply(world, inputs, outs)
}

/// The held repeat of a loop pass (§6 r6) against the model.
pub fn held_repeat(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    st: &mut ClickState,
    view: ClickView,
    mods: u32,
) -> Result<(Vec<ClickOut>, Vec<Output>), HandlerError> {
    held_repeat_at(world, inputs, st, view, mods, None)
}

/// [`held_repeat`] with the local player read at `local_at`
/// ([`ModelClick::local_at`]).
pub fn held_repeat_at(
    world: &mut ClientWorld,
    inputs: &ModelInputs,
    st: &mut ClickState,
    view: ClickView,
    mods: u32,
    local_at: Option<(u32, u32)>,
) -> Result<(Vec<ClickOut>, Vec<Output>), HandlerError> {
    let mut outs = Vec::new();
    {
        let m = ModelClick {
            world,
            inputs,
            view,
            local_at,
        };
        click::held_repeat(st, &m, mods, &mut outs);
    }
    apply(world, inputs, outs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/ui/controls.md §6 r2
    /// `a1-walk-n` (1.14d): the player at client (10320, 72816), a click
    /// at (400, 184) walks to subtile (4868, 4222), the preparation's
    /// probe of the blocked (4867, 4222): the click maps to (4867, 4222).
    #[test]
    fn a_click_maps_to_the_rounded_subtile() {
        let cam = Camera::new(
            FrameSize::D2RS,
            OpenMode::NONE,
            ClientPos { x: 10320, y: 72816 },
            (0, 0),
        );
        assert_eq!(screen_to_world(&cam, 400, 184), (4867, 4222));
        // The player's draw point (400, 292) picks its own subtile.
        assert_eq!(screen_to_world(&cam, 400, 292), (4873, 4228));
        // M08: the unit draw's plain inverse picked the blocked (4866, 4221).
        assert_ne!(screen_to_world(&cam, 400, 184 - 4), (4867, 4222));
    }
    use crate::bridge::predict::{walk_of, Walk, WalkTo};
    use crate::bridge::skills::{SkillEntry, SkillList};
    use crate::bridge::update::update_pass;
    use crate::bridge::world::ClientUnit;

    // Synthetic fixture: in game, the local player in town mode 1 at
    // (100, 100) with stamina, its left skill Attack (mode 0).
    fn world() -> ClientWorld {
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.position = Some((100, 100));
        u.server_point = (100, 100);
        u.mode = 1;
        u.stats.insert(10, 100 << 8);
        u.skills = Some(SkillList {
            entries: vec![SkillEntry {
                skill: click::ATTACK,
                base: 1,
                owner: crate::bridge::skills::NATIVE,
                ..SkillEntry::default()
            }],
            left: Some(0),
            ..SkillList::default()
        });
        let mut w = ClientWorld::default();
        w.units.insert(key, u);
        w.local_player = Some(key);
        w.in_game = true;
        w
    }

    fn view(mouse: (i32, i32)) -> ClickView {
        ClickView {
            size: FrameSize::D2RS,
            open_mode: 0,
            right_panel_bottom: FrameSize::D2RS.play_height(),
            skill_y_limit: FrameSize::D2RS.play_height(),
            mouse,
            game_menu_open: false,
            pick: false,
            shake: (0, 0),
            prev_hover: None,
        }
    }

    /// One left press at `at` with `mods`; the walk the client sent.
    fn press(mods: u32, at: (i32, i32), local_at: Option<(u32, u32)>) -> Option<Walk> {
        let mut w = world();
        let inputs = ModelInputs::default();
        let mut st = ClickState::default();
        world_click_at(
            &mut w,
            &inputs,
            &mut st,
            view(at),
            Kind::LeftDown,
            Some(at),
            mods,
            local_at,
        )
        .unwrap();
        let walks: Vec<Walk> = w.outgoing.iter().filter_map(|m| walk_of(m)).collect();
        assert!(walks.len() <= 1, "{walks:?}");
        walks.first().copied()
    }

    // Covers: specs/ui/controls.md §4.3 r1
    #[test]
    fn run_mods_word() {
        let mut m = RunMods::default();
        assert_eq!(m.word(), 0);
        m.toggle_run();
        assert_eq!(m.word(), click::mods::RUN);
        // No inversion: Run held with the lock on still runs.
        m.run_held = true;
        assert_eq!(m.word(), click::mods::RUN);
        m.toggle_run();
        assert_eq!(m.word(), click::mods::RUN);
        m.run_held = false;
        m.stand_still = true;
        assert_eq!(m.word(), click::mods::STAND_STILL);
        m.toggle_run();
        assert_eq!(m.word(), click::mods::STAND_STILL | click::mods::RUN);
    }

    // Covers: specs/ui/controls.md §6 r7, §4.3 r1
    #[test]
    fn ground_click_walks_or_runs_with_the_toggle() {
        let at = (500, 200);
        let walk = press(0, at, None).expect("a walk");
        assert!(!walk.run);
        let mut run = RunMods::default();
        run.toggle_run();
        let r = press(run.word(), at, None).expect("a run");
        assert!(r.run);
        assert_eq!(r.to, walk.to);
        // Stand Still: no walk.
        let ss = RunMods {
            stand_still: true,
            ..RunMods::default()
        };
        assert_eq!(press(ss.word(), at, None), None);
    }

    // Covers: specs/seams/world-screen.md §2.6
    #[test]
    fn the_pick_inverts_the_shaken_camera() {
        let (w, inputs) = (world(), ModelInputs::default());
        let cam = |shake| {
            ModelClick {
                world: &w,
                inputs: &inputs,
                view: ClickView {
                    shake,
                    ..view((0, 0))
                },
                local_at: None,
            }
            .camera()
            .unwrap()
        };
        let shaken = cam((3, -5));
        let (px, py) = w.local_position().unwrap();
        let player = crate::rules::camera::moving_to_client(px, py);
        assert_eq!(
            shaken,
            Camera::new(FrameSize::D2RS, OpenMode::NONE, player, (3, -5))
        );
        assert_ne!(shaken, cam((0, 0)));
        // A pixel whose world point the shake moves: the click's world
        // point is the shaken camera's.
        let at = (0..800)
            .flat_map(|x| (0..550).map(move |y| (x, y)))
            .find(|&(x, y)| screen_to_world(&shaken, x, y) != screen_to_world(&cam((0, 0)), x, y))
            .expect("the shake moves the world under some pixel");
        assert_ne!(
            screen_to_world(&shaken, at.0, at.1),
            screen_to_world(&cam((0, 0)), at.0, at.1)
        );
    }

    // Covers: specs/render/camera.md §4
    #[test]
    fn screen_to_world_is_the_unit_draw_inverse_four_rows_down() {
        // REC-514 (measured on 1.14d, supersedes camera.md §4's "no -8"
        // reading): client (sx + cx_u - shift_x, sy + cy_u - 4).
        let cam = |x, y| {
            Camera::new(
                FrameSize::D2RS,
                OpenMode::NONE,
                crate::rules::camera::ClientPos { x, y },
                (0, 0),
            )
        };
        let (w, h) = (FrameSize::D2RS.width, FrameSize::D2RS.height);
        let shift = cam(0, 0).view.shift_x;
        let want = |x: i32, y: i32| ((x + 2 * y) >> 5, (2 * y - x) >> 5);
        for (px, py) in [(0, 0), (16, 0), (-40, -24), (7, -3)] {
            let c = cam(px, py);
            assert_eq!(
                screen_to_world(&c, w / 2 + shift, h / 2 - 8),
                want(px, py + 4)
            );
        }
    }

    // Covers: specs/ui/controls.md §6 r4
    #[test]
    fn sequence_mode_gates_on_the_frame_event_index() {
        let act = |used, ev| can_act(true, false, 18, 0, used, ev);
        assert!(!act(Some((18, 4)), 3));
        assert!(act(Some((18, 4)), 4));
        assert!(!act(Some((18, 0)), 9));
        assert!(!act(Some((18, 255)), 9));
        assert!(act(Some((7, 4)), 0), "a non-SQ used skill acts");
        assert!(act(None, 0), "no used skill acts");
        assert!(!can_act(true, true, 18, 0, None, 0), "cursor item wins");
    }

    // Covers: specs/ui/controls.md §6 r8
    #[test]
    fn left_click_on_a_hostile_monster_sends_the_skill_on_the_unit() {
        use crate::bridge::world::{SkillRow, MONSTER};
        let mut w = world();
        let m = UnitKey::new(MONSTER, 9);
        let mut u = ClientUnit::new(m);
        u.position = Some((104, 104));
        u.flag_4 = true;
        u.mode = 1;
        w.units.insert(m, u);
        // Attack: anim A1 (mode 7), range h2h (synthetic rows).
        if let Some(e) = w.units.get_mut(&UnitKey::new(PLAYER, 1)) {
            e.skills.as_mut().unwrap().entries[0].mode = 7;
        }
        let mut inputs = ModelInputs::default();
        inputs.tables.skills = vec![SkillRow {
            anim: 7,
            range: 1,
            ingame: true,
            ..SkillRow::default()
        }];
        // The mouse on the monster's feet.
        let cam = ModelClick {
            world: &w,
            inputs: &inputs,
            view: view((0, 0)),
            local_at: None,
        }
        .camera()
        .unwrap();
        let at = (0..800)
            .flat_map(|x| (0..550).map(move |y| (x, y)))
            .find(|&(x, y)| screen_to_world(&cam, x, y) == (104, 104))
            .expect("the monster is on screen");
        let mut st = ClickState::default();
        world_click(
            &mut w,
            &inputs,
            &mut st,
            view(at),
            Kind::LeftDown,
            Some(at),
            0,
        )
        .unwrap();
        let mut want = vec![0x06];
        want.extend_from_slice(&1u32.to_le_bytes());
        want.extend_from_slice(&9u32.to_le_bytes());
        assert_eq!(w.outgoing, vec![want], "C→S 0x06 [type 1][guid 9]");
    }

    // Covers: specs/ui/controls.md §6 r8
    #[test]
    fn right_click_on_the_ground_casts_the_right_skill_at_the_point() {
        use crate::bridge::world::SkillRow;
        let mut w = world();
        // The right skill: id 3, cast mode SC (10), range rng (synthetic rows).
        if let Some(list) = w
            .units
            .get_mut(&UnitKey::new(PLAYER, 1))
            .and_then(|e| e.skills.as_mut())
        {
            list.entries.push(SkillEntry {
                skill: 3,
                mode: 10,
                base: 1,
                owner: crate::bridge::skills::NATIVE,
                ..SkillEntry::default()
            });
            list.right = Some(1);
        }
        let mut inputs = ModelInputs::default();
        inputs.tables.skills = vec![
            SkillRow::default(),
            SkillRow::default(),
            SkillRow::default(),
            SkillRow {
                anim: 10,
                range: 2,
                ingame: true,
                ..SkillRow::default()
            },
        ];
        let at = (500, 200);
        let mut st = ClickState::default();
        world_click(
            &mut w,
            &inputs,
            &mut st,
            view(at),
            Kind::RightDown,
            Some(at),
            0,
        )
        .unwrap();
        assert_eq!(w.outgoing.len(), 1, "{:?}", w.outgoing);
        assert_eq!(w.outgoing[0][0], 0x0C, "C→S 0x0C RightSkill at a point");
        assert_eq!(w.outgoing[0].len(), 5);
    }

    /// A synthetic animation lookup: every player mode 4 frames at
    /// speed 256.
    #[derive(Debug)]
    struct Four;
    impl crate::bridge::player_anim::PlayerAnims for Four {
        fn anim(
            &self,
            _: &ClientWorld,
            _: UnitKey,
            _: u32,
        ) -> Option<crate::bridge::player_anim::PlayerAnim> {
            Some(crate::bridge::player_anim::PlayerAnim {
                frames: 4,
                speed: 256,
                weapon: 0,
                events: None,
            })
        }
    }

    // The local player rules and the client mode machine (REC-1000,
    // q-fix-pt-cast-input-lock).
    // Covers: specs/skills/sequences.md §3
    #[test]
    fn a_cast_ends_in_neutral_and_the_next_right_click_casts_again() {
        use crate::bridge::dispatch::Dispatch;
        use crate::bridge::receive::ReceiveLog;
        use crate::bridge::world::SkillRow;
        let key = UnitKey::new(PLAYER, 1);
        let mut w = world();
        if let Some(list) = w.units.get_mut(&key).and_then(|e| e.skills.as_mut()) {
            list.entries.push(SkillEntry {
                skill: 3,
                mode: 10,
                base: 1,
                owner: crate::bridge::skills::NATIVE,
                ..SkillEntry::default()
            });
            list.right = Some(1);
        }
        let mut inputs = ModelInputs::default();
        inputs.tables.skills = vec![
            SkillRow::default(),
            SkillRow::default(),
            SkillRow::default(),
            SkillRow {
                anim: 10,
                range: 2,
                ingame: true,
                ..SkillRow::default()
            },
        ];
        inputs.player_anims = Some(std::sync::Arc::new(Four));
        let dispatch = Dispatch::from_spec().unwrap();
        let mut st = ClickState::default();
        let at = (500, 200);
        let click = |w: &mut ClientWorld, st: &mut ClickState| {
            w.outgoing.clear();
            world_click(w, &inputs, st, view(at), Kind::RightDown, Some(at), 0).unwrap();
            world_click(w, &inputs, st, view(at), Kind::RightUp, Some(at), 0).unwrap();
            // One loop pass per click (the per-pass latch, §6 r2).
            st.end_pass();
            w.outgoing.iter().filter(|m| m[0] == 0x0C).count()
        };
        let update = |w: &mut ClientWorld| {
            let (mut log, mut out) = (ReceiveLog::default(), Vec::new());
            update_pass(w, &inputs, &dispatch, &mut log, &mut out);
            assert!(log.rejected.is_empty(), "{:?}", log.rejected);
            w.units[&key].mode
        };
        assert_eq!(click(&mut w, &mut st), 1, "the first cast");
        // The click starts the cast mode SC locally (frame 0 of 4).
        assert_eq!(w.units[&key].mode, 10);
        // While it runs, a right click sends nothing (`can_act`).
        assert_eq!(click(&mut w, &mut st), 0);
        // Updates 1–2 advance to frame 2; after update 3's advance frame
        // 3 + speed 1 reaches 4: neutral on update 3 (mode 1: the player
        // is in no town room).
        assert_eq!(update(&mut w), 10);
        assert_eq!(update(&mut w), 10);
        assert_eq!(update(&mut w), 1);
        assert_eq!(
            click(&mut w, &mut st),
            1,
            "the second cast after the first ended"
        );
        assert_eq!(w.units[&key].mode, 10);
    }

    #[test]
    fn ground_click_reads_the_predicted_position() {
        let at = (500, 200);
        let WalkTo::Point(x, y) = press(0, at, None).unwrap().to else {
            panic!("a point walk");
        };
        // The player predicted 3 sub-tiles further in x: the camera and
        // the clamp follow it, so the target moves by the same 3.
        let local = ((103 << 16) | 0x8000, (100 << 16) | 0x8000);
        let WalkTo::Point(px, py) = press(0, at, Some(local)).unwrap().to else {
            panic!("a point walk");
        };
        assert_eq!((px, py), (x + 3, y));
        // The model's cell read as the prediction: the same as none.
        let same = ((100 << 16) | 0x8000, (100 << 16) | 0x8000);
        assert_eq!(press(0, at, Some(same)).unwrap().to, WalkTo::Point(x, y));
    }
    // Covers: specs/seams/movement-prediction.md §2.9 r2
    #[test]
    fn an_npc_click_measures_from_the_predicted_position() {
        use crate::bridge::world::{MonsterClass, MONSTER};
        // Model (100, 100), the walk prediction at (121, 100), a town NPC
        // at (122, 100): next to the player's own position, so the click
        // interacts at once instead of walking there.
        let mut w = world();
        w.set_local_walk(Some(((121 << 16) | 0x8000, (100 << 16) | 0x8000)), None);
        let npc = UnitKey::new(MONSTER, 9);
        let mut u = ClientUnit::new(npc);
        u.position = Some((122, 100));
        u.flag_4 = true;
        u.mode = 1;
        w.units.insert(npc, u);
        let mut inputs = ModelInputs::default();
        inputs.tables.monsters = vec![Some(MonsterClass {
            npc: true,
            interact: true,
            ..MonsterClass::default()
        })];
        let c = ModelClick {
            world: &w,
            inputs: &inputs,
            view: view((0, 0)),
            local_at: None,
        };
        assert!(c.path_distance(npc) <= 2, "{}", c.path_distance(npc));
        let cam = c.camera().unwrap();
        let at = crate::bridge::hover::unit_feet(&cam, MONSTER, (122, 100));
        // The play preview's pick (`bridge::hover`) finds the NPC under
        // the press.
        let mut v = view(at);
        v.pick = true;
        let mut st = ClickState::default();
        world_click(&mut w, &inputs, &mut st, v, Kind::LeftDown, Some(at), 0).unwrap();
        // The NPC hold (C→S 0x59 with the NPC's cell) and the interact
        // (§6 r9.2: reach 2, the interact sender), no walk to the unit:
        // from the model cell (distance 21) a C→S 0x02 would follow.
        let mut hold = vec![0x59];
        hold.extend_from_slice(&1u32.to_le_bytes());
        hold.extend_from_slice(&9u32.to_le_bytes());
        hold.extend_from_slice(&122u32.to_le_bytes());
        hold.extend_from_slice(&100u32.to_le_bytes());
        assert_eq!(w.outgoing, vec![hold], "no walk to the NPC");
    }

    // Covers: specs/ui/controls.md §6 r9; specs/sim/pathing.md §9.5
    #[test]
    fn the_interact_distance_is_the_unit_distance() {
        use crate::bridge::world::ITEM;
        // The player at (100, 100), a ground item at (101, 105): the
        // size-reduced 0x006416D0 reads 4 (an at-once pick-up), the unit
        // distance 0x00641530 of the decision reads more than 4, so the
        // client walks to the item first, as the server's 0x16 test
        // (`items/inventory-moves.md` §7.1 r2) would walk the player.
        let mut w = world();
        let item = UnitKey::new(ITEM, 6);
        let mut u = ClientUnit::new(item);
        u.position = Some((101, 105));
        u.mode = 3;
        w.units.insert(item, u);
        let inputs = ModelInputs::default();
        let c = ModelClick {
            world: &w,
            inputs: &inputs,
            view: view((0, 0)),
            local_at: None,
        };
        assert_eq!(c.path_distance(item), 4);
        let t = super::super::predict::path_tables().unwrap();
        let d = d2_sim::path::walk::geom::unit_distance(
            t,
            d2_sim::path::coords::Point::new(101, 105),
            1,
            d2_sim::path::coords::Point::new(100, 100),
            2,
        );
        assert_eq!(c.distance(item), d);
        assert!(d > 4, "{d}");
    }
}
