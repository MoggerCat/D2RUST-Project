// Spec: specs/client/msg-skills.md (§2 r3, r8, §3), specs/sim/intents-events.md (§8.2 r3.1), specs/skills/use.md (§7)
//! The play host's server-side skills, headless, on the synthetic
//! single-player game behind the app's link (`single_player::start`):
//! the join gives the server player its native skills (skill 0 and its
//! class's `charstats` `Skill 1`…`Skill 10`, Attack in both hands), sends
//! S→C 0x94 with them after the add messages, and the world's skill slot
//! runs the skill handlers (C→S 0x3C selects a hand's skill on the
//! server's list).
//!
//! The synthetic game has no `skills` rows: the test gives the game a
//! test-local copy of its action tables with eight zero `skills` records
//! (level cap 99) and the player class's ten skill ids, before the join.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link, DEFAULT_SEED, PLAYER_CLASS};
use d2_client::app::skill_rest::SkillStore;
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_data::tables::{Record, Skills};
use d2_server::seams::Clock;
use d2_sim::skills::SkillEntry;
use d2_sim::wiring::action::Pending;
use d2_sim::wiring::interaction::UseRest;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Skill rows of the test copy.
const SKILLS: usize = 8;
/// The player class's `Skill 1`…`Skill 10`: three skills, one id outside
/// the table (skipped, §2 r8.2) and empty (0xFFFF = −1) slots.
const CLASS_SKILLS: [u16; 10] = [3, 2, 0xFFFF, 40, 5, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF];

struct Game {
    link: ThreadLink<Link<StepClock>>,
    ms: Arc<AtomicU32>,
}

impl Game {
    /// The started game with the test-local skill rows, before the
    /// session sequence.
    fn started() -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let (mut link, _) =
            single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
        link.with(|l| {
            let h = l.host_mut().game.events.action.hooks();
            let mut t = (*h.tables).clone();
            t.skills.skills = vec![Skills::decode(&vec![0u8; Skills::SIZE]); SKILLS];
            t.skills.level_cap = d2_sim::skills::LEVEL_CAP_114D;
            h.tables = Arc::new(t);
            let mut store = SkillStore::from_tables(&h.tables);
            store.class_skills = vec![[0xFFFF; 10]; 7];
            store.class_skills[PLAYER_CLASS as usize] = CLASS_SKILLS;
            h.x.skills = store;
        })
        .unwrap();
        Self { link, ms }
    }

    /// C→S 0x67 and 0x6B and three more ticks; every S→C message
    /// received.
    fn join(&mut self) -> Vec<Vec<u8>> {
        let req = single_player::create_request();
        self.link.send(SendQueue::System, &req.encode()).unwrap();
        let mut got = self.ticks(1);
        self.link.send(SendQueue::System, &[0x6B]).unwrap();
        got.extend(self.ticks(3));
        got
    }

    fn ticks(&mut self, n: usize) -> Vec<Vec<u8>> {
        let mut got = Vec::new();
        for _ in 0..n {
            self.link.pump().unwrap();
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.link.pump().unwrap();
            got.extend(self.link.receive());
        }
        got
    }

    /// The server player's skill list, left and right skill and GUID.
    fn player_skills(&mut self) -> (Vec<SkillEntry>, Option<SkillEntry>, Option<SkillEntry>, u32) {
        self.link
            .with(|l| {
                let s = &mut l.host_mut().game;
                let (p, guid) = single_player::local_player(s).expect("joined");
                let x = &s.events.action.hooks().x;
                (x.skill_list(p), x.left_skill(p), x.right_skill(p), guid)
            })
            .unwrap()
    }
}

fn native(skill: i32) -> SkillEntry {
    SkillEntry {
        skill,
        base: 1,
        owner_guid: -1,
        ..SkillEntry::default()
    }
}

// Covers: specs/client/msg-skills.md §2 r8, §3 r1; specs/sim/intents-events.md §8.2 r3
#[test]
fn the_join_gives_the_server_player_its_native_skills_and_sends_0x94() {
    let mut g = Game::started();
    let got = g.join();
    let (list, left, right, guid) = g.player_skills();
    let want: Vec<_> = [0, 3, 2, 5].map(native).to_vec();
    assert_eq!(
        list, want,
        "skill 0, then the class skills inside the table"
    );
    assert_eq!((left, right), (Some(native(0)), Some(native(0))));

    let at = |id: u8| got.iter().position(|m| m.first() == Some(&id));
    let i = at(0x94).expect("S→C 0x94 in the join");
    let mut m94 = vec![0x94, 4];
    m94.extend_from_slice(&guid.to_le_bytes());
    for s in [0u16, 3, 2, 5] {
        m94.extend_from_slice(&s.to_le_bytes());
        m94.push(1);
    }
    assert_eq!(got[i], m94);
    assert!(at(0x76).unwrap() < i, "after the add messages");
    assert!(i < at(0x0B).unwrap(), "before the handshake");
    assert_eq!(
        got.iter().filter(|m| m.first() == Some(&0x23)).count(),
        2,
        "the join's two 0x23 (no StartSkill on synthetic data)"
    );
}

// Covers: specs/skills/use.md §7; specs/client/msg-skills.md §2 r3
#[test]
fn select_skill_runs_on_the_server_skill_list() {
    let mut g = Game::started();
    g.join();
    // 0x3C: skill 3 in the right hand (bit 31 clear), owner −1.
    let mut m = vec![0x3C];
    m.extend_from_slice(&3u32.to_le_bytes());
    m.extend_from_slice(&(-1i32).to_le_bytes());
    g.link.send(SendQueue::Game, &m).unwrap();
    g.ticks(2);
    let (_, left, right, _) = g.player_skills();
    assert_eq!(right, Some(native(3)), "the slot ran use.md §7");
    assert_eq!(left, Some(native(0)));
}
