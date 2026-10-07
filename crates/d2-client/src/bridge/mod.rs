// Spec: specs/client/bridge.md, specs/client/model.md (§4, §5, §6 rule 8, §7 rule 3)
//! The only link between the Bevy app and the game. Outbound: requests
//! become 1.14d C→S bytes built with `d2-proto` ([`intent`]). Inbound: the
//! S→C bytes the server delivers are split with `d2-proto` and dispatched
//! by id to handlers that update the [`world::ClientWorld`] ([`receive`],
//! [`dispatch`]); ids no spec owns yet are recorded, never interpreted.
//! Unit-handler messages are queued on their unit and applied by the
//! update pass ([`update`]) in frames where the server ticked; the C→S
//! messages the model answers with on its own (0x6B, 0x5F) go through the
//! send path at the end of the frame.
//! Bevy entities only mirror the world model ([`mirror`]). Contains no
//! game rules: the server decides every outcome.
//!
//! Everything except [`mirror`] is plain Rust without Bevy types.

pub mod bits;
pub mod check;
pub mod click;
pub mod dispatch;
pub mod drlg;
pub mod intent;
pub mod link;
pub mod local;
pub mod mirror;
pub mod modes;
#[cfg(test)]
mod modes_tests;
pub mod msg;
pub mod object_hover;
pub mod objects;
pub mod output;
pub mod passive;
#[cfg(test)]
mod passive_tests;
pub mod predict;
pub mod receive;
pub mod skills;
pub mod update;
pub mod world;

#[cfg(test)]
mod gaps_numbered_tests;
#[cfg(test)]
mod local_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_c2cli;

use d2_proto::transport::SplitError;
use d2_proto::{FixedMessage, PROTOCOL_VERSION};

use dispatch::{Dispatch, TableError};
use intent::IntentError;
use link::{LinkError, Sent, ServerLink};
use output::Output;
use receive::{receive_chunk, ReceiveLog};
use world::{ClientTables, ClientWorld, ModelInputs, VisibleFn};

pub use link::{Pumped, SendQueue, LOCAL_CLIENT};
pub use local::{LocalLink, SinglePlayer};
pub use mirror::{BridgePlugin, BridgeResource, FrameOutputs, UnitView};
pub use world::{ClientUnit, UnitKey};

/// A bridge failure. All of them stop the frame (spec §8 rule 4).
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("server protocol version {server}, client {client}")]
    Version { client: u32, server: u32 },
    #[error(transparent)]
    Table(#[from] TableError),
    #[error(transparent)]
    Intent(#[from] IntentError),
    #[error(transparent)]
    Link(#[from] LinkError),
    /// A chunk 1.14d asserts on (spec §2 rule 4).
    #[error("S→C chunk refused: {0}")]
    Split(#[from] SplitError),
    /// A world click the original asserts on (`ui/controls.md` §6).
    #[error("world click: {0}")]
    Click(dispatch::HandlerError),
}

/// One bridge frame (spec §8 rule 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameReport {
    pub ticked: bool,
    pub chunks: usize,
    pub messages: usize,
    pub handled: usize,
    /// Unit-handler messages queued on their unit (`model.md` §4).
    pub queued: usize,
    /// Unit-handler messages whose unit was not in the model.
    pub dropped: usize,
    /// Queued messages the update pass applied (`model.md` §5).
    pub drained: usize,
    pub unowned: usize,
    pub rejected: usize,
    pub discarded_bytes: usize,
    /// C→S messages the model sent on its own (0x6B, 0x5F).
    pub answered: usize,
    /// UI and sound outputs the frame's handlers made (spec §10).
    pub outputs: usize,
}

/// The bridge: a server link, the client world model and the dispatch
/// table.
pub struct Bridge<L> {
    link: L,
    dispatch: Dispatch,
    world: ClientWorld,
    inputs: ModelInputs,
    log: ReceiveLog,
    /// The frame's UI and sound outputs, in order (spec §10 rule 1).
    outputs: Vec<Output>,
}

impl<L: ServerLink> Bridge<L> {
    /// A bridge over `link` with the spec's dispatch table.
    pub fn new(link: L) -> Result<Self, BridgeError> {
        Self::with_dispatch(link, Dispatch::from_spec()?)
    }

    /// A bridge with a given dispatch table (tests and tools). Refuses a
    /// link of another protocol version (spec §9 rule 1).
    pub fn with_dispatch(link: L, dispatch: Dispatch) -> Result<Self, BridgeError> {
        let server = link.protocol_version();
        if server != PROTOCOL_VERSION {
            return Err(BridgeError::Version {
                client: PROTOCOL_VERSION,
                server,
            });
        }
        Ok(Self {
            link,
            dispatch,
            world: ClientWorld::default(),
            inputs: ModelInputs::default(),
            log: ReceiveLog::default(),
            outputs: Vec::new(),
        })
    }

    /// Sends a typed C→S message (spec §4 rule 1).
    pub fn send<M: FixedMessage>(&mut self, msg: &M) -> Result<Sent, BridgeError> {
        self.send_bytes(&intent::encode(msg))
    }

    /// Sends C→S bytes after the classifier check (spec §4 rules 2–3).
    pub fn send_bytes(&mut self, msg: &[u8]) -> Result<Sent, BridgeError> {
        let queue = intent::route(msg)?;
        Ok(self.link.send(queue, msg)?)
    }

    /// One bridge frame: pump the server, receive and dispatch every
    /// delivered chunk (spec §8 rule 1), then, if the server ticked and
    /// the model is in game, the update pass (`model.md` §5 rule 1), and
    /// the C→S messages the model answered with. A refused chunk ends the
    /// frame with an error; chunks after it in the same receive are not
    /// processed (a fatal assert in 1.14d). The frame's outputs wait in
    /// the bridge for [`Self::take_outputs`] (spec §10 rule 4).
    pub fn frame(&mut self) -> Result<FrameReport, BridgeError> {
        // A dialog-reply slot the UI layer did not answer after the last
        // frame's outputs carries no message (`msg-ui.md` §16 r4.3: no
        // case was handed back, so no C→S 0x31).
        self.world.outgoing.retain(|m| !m.is_empty());
        let pumped = self.link.pump()?;
        let mut report = FrameReport {
            ticked: pumped.ticked,
            ..FrameReport::default()
        };
        self.world.frames += 1;
        if pumped.ticked {
            self.world.server_ticks += 1;
        }
        let before = self.outputs.len();
        for chunk in self.link.receive() {
            let c = receive_chunk(
                &mut self.world,
                &self.inputs,
                &self.dispatch,
                &mut self.log,
                &mut self.outputs,
                &chunk,
            )?;
            report.chunks += 1;
            report.messages += c.messages;
            report.handled += c.handled;
            report.queued += c.queued;
            report.dropped += c.dropped;
            report.unowned += c.unowned;
            report.rejected += c.rejected;
            report.discarded_bytes += c.discarded_bytes;
        }
        if pumped.ticked && self.world.in_game {
            let before = self.log.rejected.len();
            report.drained = self.update_pass();
            report.rejected += self.log.rejected.len() - before;
        }
        report.answered = self.send_outgoing()?;
        report.outputs = self.outputs.len() - before;
        Ok(report)
    }

    /// Applies one S→C chunk without pumping (spec §2).
    pub fn receive_chunk(&mut self, chunk: &[u8]) -> Result<receive::ChunkReport, BridgeError> {
        Ok(receive_chunk(
            &mut self.world,
            &self.inputs,
            &self.dispatch,
            &mut self.log,
            &mut self.outputs,
            chunk,
        )?)
    }

    /// The update pass alone (`model.md` §5): drains every unit's queue.
    pub fn update_pass(&mut self) -> usize {
        update::update_pass(
            &mut self.world,
            &self.inputs,
            &self.dispatch,
            &mut self.log,
            &mut self.outputs,
        )
    }

    /// Hands the outputs made since the last call over, in order, and
    /// clears the list (spec §10 rule 4: once per frame, after the
    /// frame).
    pub fn take_outputs(&mut self) -> Vec<Output> {
        std::mem::take(&mut self.outputs)
    }

    /// The outputs not yet handed over.
    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    /// Sends the model's own C→S messages (`model.md` §6 rule 8, §7 rule
    /// 3) through the send path, in order, and clears them, up to the
    /// first reserved dialog-reply slot (`world::DIALOG_REPLY_SLOT`):
    /// that slot and what follows wait for [`Self::npc_dialog_branch`].
    pub fn send_outgoing(&mut self) -> Result<usize, BridgeError> {
        let n = self
            .world
            .outgoing
            .iter()
            .position(|m| m.is_empty())
            .unwrap_or(self.world.outgoing.len());
        let out: Vec<Vec<u8>> = self.world.outgoing.drain(..n).collect();
        for m in &out {
            self.send_bytes(m)?;
        }
        Ok(out.len())
    }

    /// One world click (`ui/controls.md` §6 r1–r2, [`click`]): the
    /// dispatcher against the model, its C→S messages sent at once
    /// (`client/bridge.md` §4). Returns the outputs the UI layer applies
    /// (sounds, hover calls, the pending record) and the interact
    /// sender's outputs.
    pub fn world_click(
        &mut self,
        st: &mut crate::controls::click::ClickState,
        view: click::ClickView,
        kind: crate::controls::click::Kind,
        at: Option<(i32, i32)>,
        mods: u32,
    ) -> Result<(Vec<crate::controls::click::ClickOut>, Vec<output::Output>), BridgeError> {
        let r = click::world_click(&mut self.world, &self.inputs, st, view, kind, at, mods)
            .map_err(BridgeError::Click)?;
        self.send_outgoing()?;
        Ok(r)
    }

    /// The held repeat of a loop pass (`ui/controls.md` §6 r6), its
    /// messages sent at once.
    pub fn click_repeat(
        &mut self,
        st: &mut crate::controls::click::ClickState,
        view: click::ClickView,
        mods: u32,
    ) -> Result<(Vec<crate::controls::click::ClickOut>, Vec<output::Output>), BridgeError> {
        let r = click::held_repeat(&mut self.world, &self.inputs, st, view, mods)
            .map_err(BridgeError::Click)?;
        self.send_outgoing()?;
        Ok(r)
    }

    /// The UI layer's answer to an `NpcDialog` output (`msg-ui.md` §16
    /// r4.3; open question 10 decided as A, `bridge.md` §10 r6): the
    /// bridge applies the branch's model writes and C→S 0x31 in 1.14d
    /// order ([`msg::ui_npc::apply_dialog_branch`]), then sends the
    /// messages that waited behind the slot. Returns how many were sent.
    /// [`Self::world_click`] with the local player read at `local_at`
    /// (`click::world_click_at`; the `play` preview's predicted position,
    /// decision D2, d2rs-own, unverified).
    #[allow(clippy::too_many_arguments)]
    pub fn world_click_at(
        &mut self,
        st: &mut crate::controls::click::ClickState,
        view: click::ClickView,
        kind: crate::controls::click::Kind,
        at: Option<(i32, i32)>,
        mods: u32,
        local_at: Option<(u32, u32)>,
    ) -> Result<(Vec<crate::controls::click::ClickOut>, Vec<output::Output>), BridgeError> {
        let r = click::world_click_at(
            &mut self.world,
            &self.inputs,
            st,
            view,
            kind,
            at,
            mods,
            local_at,
        )
        .map_err(BridgeError::Click)?;
        self.send_outgoing()?;
        Ok(r)
    }

    /// [`Self::click_repeat`] with the local player read at `local_at`
    /// (`click::held_repeat_at`).
    pub fn click_repeat_at(
        &mut self,
        st: &mut crate::controls::click::ClickState,
        view: click::ClickView,
        mods: u32,
        local_at: Option<(u32, u32)>,
    ) -> Result<(Vec<crate::controls::click::ClickOut>, Vec<output::Output>), BridgeError> {
        let r = click::held_repeat_at(&mut self.world, &self.inputs, st, view, mods, local_at)
            .map_err(BridgeError::Click)?;
        self.send_outgoing()?;
        Ok(r)
    }

    pub fn npc_dialog_branch(
        &mut self,
        dialog: &output::NpcDialog,
        case: msg::ui_npc::DialogCase,
    ) -> Result<usize, BridgeError> {
        msg::ui_npc::apply_dialog_branch(&mut self.world, dialog, case);
        self.send_outgoing()
    }

    /// The draw's Y sort of a room's unit list written back to the client
    /// list (`sim/unit-order.md` §5 rule 7: the sorted order persists).
    /// `false`: `order` is not a permutation of the list (nothing changed).
    pub fn set_room_order(&mut self, room: drlg::DrlgRoomId, order: &[UnitKey]) -> bool {
        self.world.room_units.set_order(room, order)
    }

    /// The tables the message rules read (`msg-units.md` Inputs).
    pub fn set_tables(&mut self, tables: ClientTables) {
        self.inputs.tables = tables;
    }

    /// The unit-message rows (`msg-units.md` §1.2 r7, §1.3 r3,
    /// `model.md` §15 r1): `monstats` / `monstats2`, `itemstatcost` send
    /// columns, `objects.txt` and `shrines.txt`; the other tables stay.
    pub fn set_unit_rows(&mut self, rows: world::UnitRows) {
        let t = &mut self.inputs.tables;
        t.monsters = rows.monsters;
        t.monster_skill_bonus = rows.monster_skill_bonus;
        t.stats = rows.stats;
        t.objects = rows.objects;
        t.shrines = rows.shrines;
    }

    /// The host's wall-clock seconds `0x00410A80` (`render/lighting.md`
    /// §10 r4).
    pub fn set_wall_seconds(&mut self, f: fn() -> i32) {
        self.inputs.wall_seconds = Some(f);
    }

    /// The skills tables of the passive refresh (`msg-skills.md` §2 r4).
    pub fn set_skill_tables(&mut self, tables: std::sync::Arc<d2_sim::skills::SkillTables>) {
        self.inputs.skill_tables = Some(tables);
    }

    /// The `skills` rows of the client skill list (`msg-skills.md`
    /// Inputs); the other tables stay.
    pub fn set_skill_rows(&mut self, rows: Vec<world::SkillRow>) {
        self.inputs.tables.skills = rows;
    }

    /// Each class's `charstats` Skill 1–10 (`msg-skills.md` §2 rule 8);
    /// the other tables stay.
    pub fn set_class_skills(&mut self, class_skills: Vec<[u16; 10]>) {
        self.inputs.tables.class_skills = class_skills;
    }

    /// What the client DRLG of 0x03 is built from (`model.md` §12 rule
    /// 1); `None`: no client DRLG.
    pub fn set_drlg_source(&mut self, source: Option<drlg::DrlgSource>) {
        self.inputs.drlg = source;
    }

    /// The wall clock of the next update, `GetTickCount()` in wrapping
    /// milliseconds (`model.md` §5 rule 2; `world/objects-client.md` §25
    /// r6): the live client passes the host clock, tests a scripted value.
    pub fn set_now(&mut self, now: u32) {
        self.inputs.now = now;
    }

    /// The `objects.txt` rows the client object update reads
    /// (`world/objects-client.md` §28 r1); empty: no object update.
    pub fn set_object_rows(&mut self, rows: Vec<objects::ObjClientRow>) {
        self.inputs.objclient.rows = rows;
    }

    /// The UI layer's client quest record `[0x007C0D43]` (`ClientFn` 13,
    /// `world/objects-client.md` §26.13 r3).
    pub fn set_client_quest_flags(&mut self, flags: Option<[u8; objects::QUEST_RECORD]>) {
        self.inputs.objclient.quest_flags = flags;
    }

    /// The visibility predicate of the position check (`model.md` §6
    /// rule 6, open question 7).
    pub fn set_visibility(&mut self, visible: Option<VisibleFn>) {
        self.inputs.visible = visible;
    }

    pub fn inputs(&self) -> &ModelInputs {
        &self.inputs
    }

    /// The model, writable (tests only).
    #[cfg(test)]
    pub(crate) fn world_mut(&mut self) -> &mut ClientWorld {
        &mut self.world
    }

    pub fn world(&self) -> &ClientWorld {
        &self.world
    }

    pub fn log(&self) -> &ReceiveLog {
        &self.log
    }

    pub fn link(&self) -> &L {
        &self.link
    }

    pub fn link_mut(&mut self) -> &mut L {
        &mut self.link
    }
}

impl<L: ServerLink + ?Sized> ServerLink for Box<L> {
    fn protocol_version(&self) -> u32 {
        (**self).protocol_version()
    }

    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        (**self).send(queue, msg)
    }

    fn pump(&mut self) -> Result<Pumped, LinkError> {
        (**self).pump()
    }

    fn receive(&mut self) -> Vec<Vec<u8>> {
        (**self).receive()
    }
}
