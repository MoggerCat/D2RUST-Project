// Spec: specs/ui/frontend-menus.md (§F2.6, §F2.8, §F3.6), specs/ui/frontend-loading.md (L1–L10)
//! The glue between the front end and the game start: what the select,
//! create and difficulty screens decided becomes the game's character, its
//! C→S 0x67 and its loading screen. No game logic (CLAUDE.md rule 5): the
//! rules are the pure functions of `ui::front_end::screens::difficulty`.
//!
//! - [`StartHandles`] / [`registry`]: the shared cells the screens write
//!   their choice into, and a registry with them wired.
//! - [`StartChoice`]: the choice resolved from a [`GameLoad`]: the 0x67
//!   ([`StartChoice::create_request`]: name, class, difficulty,
//!   `start_flags`) and the character of the game.
//! - [`LoadFeed`] / [`LoadingState`]: the bridge's act / in-game state as
//!   [`LoadEvent`]s for the [`LoadingScreen`], and what to present.
//!
//! `// d2rs-own, unverified` and PROVISIONAL (REC-236): the events are
//! derived from the model's state (0x03 act change, 0x04 in-game, 0x05
//! leaving it), not from the message stream; 0x61 videos are not seen;
//! the start act (`start_act`) is reported, not yet used to place the
//! player (the save load does that).

use std::path::{Path, PathBuf};

use crate::app::single_player::{self, Character};
use crate::bridge::world::ClientWorld;
use crate::ui::front_end::screens::char_select::{self, Selection, SelectionHandle};
use crate::ui::front_end::screens::create::{self, NewCharacter, NewCharacterSink};
use crate::ui::front_end::screens::difficulty::{self, start_flags};
use crate::ui::front_end::screens::loading::{LoadEvent, LoadingScreen, Presented};
use crate::ui::front_end::{GameLoad, Registry};
use d2_proto::client::CreateGame;

/// The cells the select and create screens leave their choice in.
#[derive(Clone, Default)]
pub struct StartHandles {
    pub selection: SelectionHandle,
    pub created: NewCharacterSink,
}

/// A registry with every screen, select and create wired to `save_dir`
/// and `handles`.
pub fn registry(save_dir: &Path, handles: &StartHandles) -> Registry {
    let mut reg = Registry::default();
    crate::ui::front_end::screens::register_all(&mut reg);
    char_select::register_with(
        &mut reg,
        Some(save_dir.to_path_buf()),
        handles.selection.clone(),
    );
    let dir = save_dir.to_path_buf();
    create::register_with(
        &mut reg,
        handles.created.clone(),
        Box::new(move |name| create::name_taken_in(&dir, name)),
    );
    reg
}

/// The character the front end chose, and how to start it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartChoice {
    pub name: String,
    pub class: u8,
    /// The save's status word (new character: built from its choices).
    pub status: u16,
    /// Game difficulty 0–2.
    pub difficulty: u8,
    /// The save to load (`None`: a new character).
    pub save: Option<PathBuf>,
}

impl StartChoice {
    /// Resolves the flow's end. `None`: the screens left no choice (the
    /// default character starts).
    pub fn resolve(load: GameLoad, handles: &StartHandles, save_dir: &Path) -> Option<StartChoice> {
        if load.new_character {
            let n: NewCharacter = handles.created.borrow_mut().take()?;
            let mut status = 0;
            if n.hardcore {
                status |= difficulty::STATUS_HARDCORE;
            }
            if n.expansion {
                status |= difficulty::STATUS_EXPANSION;
            }
            return Some(StartChoice {
                name: n.name,
                class: n.class.id(),
                status,
                difficulty: 0,
                save: None,
            });
        }
        let Selection { entry, .. } = handles.selection.lock().ok()?.take()?;
        Some(StartChoice {
            save: Some(save_dir.join(format!("{}.d2s", entry.name))),
            name: entry.name,
            class: entry.class,
            status: entry.status,
            // Without the box the game starts at Normal (L2 r1).
            difficulty: load.difficulty.unwrap_or(0),
        })
    }

    pub fn hardcore(&self) -> bool {
        self.status & difficulty::STATUS_HARDCORE != 0
    }

    /// The C→S 0x67 (L2 r5): the character's class and name, the game's
    /// difficulty and `start_flags` of its status.
    pub fn create_request(&self) -> CreateGame {
        let mut r = single_player::create_request_for(&self.named());
        r.flags = start_flags(self.status);
        r
    }

    /// The character by name and class alone (synthetic data, or a new
    /// character).
    fn named(&self) -> Character {
        let mut name = [0u8; 16];
        let n = self.name.len().min(15);
        name[..n].copy_from_slice(&self.name.as_bytes()[..n]);
        Character::Named(single_player::NewCharacter {
            class: self.class,
            name,
            difficulty: self.difficulty,
        })
    }

    /// The character the game loads: the save with live data, else the
    /// named stand-in.
    pub fn character(
        &self,
        data: &single_player::GameData,
    ) -> Result<Character, single_player::BuildError> {
        match (&self.save, data) {
            (Some(p), single_player::GameData::Live(_)) => {
                single_player::load_character(data, p, self.difficulty)
            }
            _ => Ok(self.named()),
        }
    }
}

/// Turns the model's session messages into [`LoadEvent`]s, in arrival
/// order (L9, L10): 0x05, 0x03, 0x61, 0x04, including a repeated 0x03 of
/// the same act. A model without session marks (state set directly) falls
/// back to edge detection.
#[derive(Debug, Default)]
pub struct LoadFeed {
    act: Option<crate::bridge::world::ActLoad>,
    in_game: bool,
    seen: u64,
}

impl LoadFeed {
    pub fn observe(&mut self, w: &ClientWorld) -> Vec<LoadEvent> {
        use crate::bridge::world::SessionMark;
        let mut out = Vec::new();
        let fresh = (w.session_total - self.seen) as usize;
        if fresh > 0 {
            let log = &w.session_log;
            // Marks older than the log's window are gone; take what is left.
            for m in &log[log.len() - fresh.min(log.len())..] {
                out.push(match *m {
                    SessionMark::LoadAct(act) => LoadEvent::S03 { act },
                    SessionMark::LoadComplete => LoadEvent::S04,
                    SessionMark::Unload => LoadEvent::S05,
                    SessionMark::Video(id) => LoadEvent::S61 { id },
                });
            }
            self.seen = w.session_total;
            self.act = w.act;
            self.in_game = w.in_game;
            return out;
        }
        if w.act != self.act {
            if let Some(a) = w.act {
                out.push(LoadEvent::S03 { act: a.act });
            }
            self.act = w.act;
        }
        if w.in_game != self.in_game {
            out.push(if w.in_game {
                LoadEvent::S04
            } else {
                LoadEvent::S05
            });
            self.in_game = w.in_game;
        }
        out
    }
}

/// The loading screen of the running game.
#[derive(bevy::prelude::Resource)]
pub struct LoadingState {
    pub screen: LoadingScreen,
    feed: LoadFeed,
    /// What the last frame presented.
    pub last: Option<Presented>,
}

impl Default for LoadingState {
    /// Game start is the first loading draw (L5 row 1).
    fn default() -> Self {
        let mut screen = LoadingScreen::new();
        screen.event(LoadEvent::GameStart);
        Self {
            screen,
            feed: LoadFeed::default(),
            last: None,
        }
    }
}

impl LoadingState {
    /// One client loop pass over the model.
    pub fn frame(&mut self, w: &ClientWorld) -> Option<Presented> {
        let placed = w.local_room().is_some();
        self.advance(w, placed)
    }

    /// [`LoadingState::frame`] with `placed` (the local player has a room,
    /// L6 r1) given.
    pub fn advance(&mut self, w: &ClientWorld, placed: bool) -> Option<Presented> {
        for e in self.feed.observe(w) {
            self.screen.event(e);
        }
        self.last = self.screen.frame(placed).or(self.last);
        self.screen.presented.last().copied()
    }

    /// Whether the loading art covers the window.
    pub fn covering(&self) -> bool {
        self.screen.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::ActLoad;

    fn act(n: u8) -> ActLoad {
        ActLoad {
            act: n,
            init_seed: 1,
            town_level: 1,
            f8: 0,
        }
    }

    #[test]
    fn feed_emits_edges_once() {
        let mut f = LoadFeed::default();
        let mut w = ClientWorld::default();
        assert!(f.observe(&w).is_empty());
        w.act = Some(act(1));
        assert_eq!(f.observe(&w), vec![LoadEvent::S03 { act: 1 }]);
        assert!(f.observe(&w).is_empty());
        w.in_game = true;
        assert_eq!(f.observe(&w), vec![LoadEvent::S04]);
        w.in_game = false;
        assert_eq!(f.observe(&w), vec![LoadEvent::S05]);
    }

    #[test]
    fn loading_covers_until_the_world() {
        let mut s = LoadingState::default();
        assert!(s.covering());
        let mut w = ClientWorld {
            act: Some(act(0)),
            ..ClientWorld::default()
        };
        assert_eq!(s.frame(&w), Some(Presented::Loading { frame: 1 }));
        w.in_game = true;
        // In game but no player placed yet: still loading.
        assert!(matches!(s.frame(&w), Some(Presented::Loading { .. })));
        assert!(s.covering());
    }
}
