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
//! - no hover model (`0x00467A10`, `client/model.md` hover): no unit is
//!   hovered, so every click is a click on the ground;
//! - no client path record: see [`ModelClick::walk_path`];
//! - the `skills.txt` flag columns and `range` are not in the client
//!   tables: [`ModelClick::skill_row`] answers `None`, which §6 r8.2 reads
//!   as a point click.

use crate::controls::click::{
    self, ClickOut, ClickState, ClickWorld, Kind, Pending, SkillRef, SkillRowFacts,
};
use crate::rules::camera::{Camera, ClientPos, FrameSize, OpenMode};

use super::dispatch::HandlerError;
use super::objects::interact;
use super::output::Output;
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
}

/// The client model as the dispatcher reads it.
pub struct ModelClick<'a> {
    pub world: &'a ClientWorld,
    pub inputs: &'a ModelInputs,
    pub view: ClickView,
}

/// `0x00464600(P, skill)` (§6 r4 kind 0): 0 when P holds a cursor item
/// or is not a player, or in mode 0, 4, 7–12, 17 or 19; mode 13 with
/// class 3, mode 14 with class 6, modes 15–16 with classes 4–6.
/// PROVISIONAL (ui/controls.md §6 r4, `0x004645B0`; controls-0001): mode
/// 18 (sequence), whose answer `0x004645B0(skill)` decides, reads as 0.
pub fn can_act(is_player: bool, cursor_item: bool, mode: u32, class: u32) -> bool {
    if !is_player || cursor_item {
        return false;
    }
    !matches!(
        (mode, class),
        (0 | 4 | 7..=12 | 17 | 18 | 19, _) | (13, 3) | (14, 6) | (15 | 16, 4..=6)
    )
}

impl ModelClick<'_> {
    fn camera(&self) -> Option<Camera> {
        let p = self.world.local()?;
        let (x, y) = p.cell();
        let at = crate::rules::camera::moving_to_client(
            (u32::from(x) << 16) | 0x8000,
            (u32::from(y) << 16) | 0x8000,
        );
        let mode = OpenMode::new(self.view.open_mode).unwrap_or(OpenMode::NONE);
        Some(Camera::new(self.view.size, mode, at, (0, 0)))
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
/// PROVISIONAL (ui/controls.md §6 r2: `0x0045AFF0` is named, not
/// specified; controls-0001): screen (sx, sy) → client (sx + cx_u −
/// shift_x, sy + cy_u − 8) → subtile ((px + 2·py) / 32, (2·py − px) / 32)
/// floored.
pub fn screen_to_world(cam: &Camera, sx: i32, sy: i32) -> (i32, i32) {
    let p = ClientPos {
        x: sx + cam.unit.x - cam.view.shift_x,
        y: sy + cam.unit.y - 8,
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
        // TODO(spec: client/model.md hover `0x00467A10`): no hover model.
        None
    }
    fn to_world(&self, x: i32, y: i32) -> (i32, i32) {
        self.camera().map_or((0, 0), |c| screen_to_world(&c, x, y))
    }
    fn position(&self, u: UnitKey) -> Option<(i32, i32)> {
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
        can_act(
            p.key.unit_type == PLAYER,
            self.world.use_cursor.is_some(),
            p.mode,
            p.class,
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
    fn skill_row(&self, _id: u16) -> Option<SkillRowFacts> {
        // TODO(spec: ui/controls.md §6 r8): the client `skills` rows hold
        // no flag columns or `range` yet.
        None
    }
    fn range(&self, _skill: SkillRef) -> u8 {
        click::range::NONE
    }
    fn use_state(&self, _skill: SkillRef) -> u32 {
        // TODO(spec: skills/use.md §2 `0x00647960`): the client use state
        // is not computed; 0 = usable.
        0
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
    fn hostile(&self, _u: UnitKey) -> bool {
        false
    }
    fn monster_npc_interact(&self, _u: UnitKey) -> (bool, bool) {
        (false, false)
    }
    fn object_has_row(&self, u: UnitKey) -> bool {
        self.world
            .units
            .get(&u)
            .is_some_and(|u| (u.class as usize) < self.inputs.tables.objects.len())
    }
    fn distance(&self, u: UnitKey) -> i32 {
        self.path_distance(u)
    }
    fn path_distance(&self, u: UnitKey) -> i32 {
        let rows = &self.inputs.objclient.rows;
        match (self.world.units.get(&u), self.world.local()) {
            (Some(u), Some(p)) => super::objects::distance(
                u,
                super::objects::unit_size(u, rows),
                p,
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
    fn melee_range(&self, _u: UnitKey) -> bool {
        false
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
/// (`client/model.md` §8 rule 7), [`ClickOut::Send`] its bytes. The
/// client mode request of a code (`0x00480C10` inside `0x00481030`) is
/// not applied: PROVISIONAL (ui/controls.md §6 r7; REC-51): which mode
/// request code `0x00481030` passes for a click code is not stated, so
/// the local player's mode follows the server's echo of the message.
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
                if let Some(m) = click::code_bytes(code, a, b) {
                    world.outgoing.push(m);
                }
            }
            ClickOut::Send(m) => world.outgoing.push(m),
            other => rest.push(other),
        }
    }
    Ok((rest, outputs))
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
    let mut outs = Vec::new();
    {
        let m = ModelClick {
            world,
            inputs,
            view,
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
    let mut outs = Vec::new();
    {
        let m = ModelClick {
            world,
            inputs,
            view,
        };
        click::held_repeat(st, &m, mods, &mut outs);
    }
    apply(world, inputs, outs)
}
