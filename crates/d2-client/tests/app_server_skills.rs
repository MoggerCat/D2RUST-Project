// Spec: specs/client/msg-skills.md (§2 r3, r8, §3), specs/sim/intents-events.md (§8.2 r3.1), specs/skills/use.md (§7)
//! The play host's server-side skills, headless, on the synthetic
//! single-player game behind the app's link (`single_player::start`):
//! the server player's skill list is d2-sim's (`ActionHooks::skill_lists`)
//! and the world's skill slot (`WiredSkills`) runs the skill handlers on
//! it (C→S 0x3C selects a hand's skill on the server's list). A new
//! character's join sends no S→C 0x94 (its stub load reads no skills
//! section, `intents-events.md` §8.2 rule 3.1).
//!
//! The synthetic game has no `skills` rows and its `charstats` name no
//! class skill: the test gives the game a test-local copy of its action
//! tables with eight zero `skills` records (level cap 99) and of its
//! vitals tables with the class's ten skill ids, so the join's native
//! skills (§2 rule 8) run on them.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, Link, DEFAULT_SEED, PLAYER_CLASS};
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_data::tables::{Record, Skills};
use d2_server::seams::Clock;
use d2_sim::skills::SkillEntry;

mod app_support;

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
        let (mut link, _) = single_player::start(
            app_support::game_data(),
            DEFAULT_SEED,
            StepClock(ms.clone()),
        )
        .unwrap();
        link.with(|l| {
            let h = l.host_mut().game.events.action.hooks();
            let mut t = (*h.tables).clone();
            t.skills.skills = vec![Skills::decode(&vec![0u8; Skills::SIZE]); SKILLS];
            t.skills.level_cap = d2_sim::skills::LEVEL_CAP_114D;
            h.tables = Arc::new(t);
            let mut v = (**h.vitals.as_ref().expect("synthetic vitals")).clone();
            let c = &mut v.charstats[PLAYER_CLASS as usize];
            for (slot, id) in [
                &mut c.skill_1,
                &mut c.skill_2,
                &mut c.skill_3,
                &mut c.skill_4,
                &mut c.skill_5,
                &mut c.skill_6,
                &mut c.skill_7,
                &mut c.skill_8,
                &mut c.skill_9,
                &mut c.skill_10,
            ]
            .into_iter()
            .zip(CLASS_SKILLS)
            {
                *slot = id;
            }
            h.vitals = Some(Arc::new(v));
        })
        .unwrap();
        Self { link, ms }
    }

    /// C→S 0x67 and 0x6B and three more ticks (the join runs rule 8 on
    /// the player's list, module docs); every S→C message received.
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
                let l = &s.events.action.hooks().skill_lists[&p];
                let v = l.view();
                let at = |i: Option<usize>| i.and_then(|i| v.get(i).copied());
                (v.clone(), at(l.left), at(l.right), guid)
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

// Covers: specs/client/msg-skills.md §2 r8; specs/sim/intents-events.md §8.2 r3
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_server_player_list_holds_its_native_skills_and_a_new_join_sends_no_0x94() {
    let mut g = Game::started();
    let got = g.join();
    let (list, left, right, _) = g.player_skills();
    let want: Vec<_> = [0, 3, 2, 5].map(native).to_vec();
    assert_eq!(
        list, want,
        "skill 0, then the class skills inside the table"
    );
    assert_eq!((left, right), (Some(native(0)), Some(native(0))));
    assert!(
        got.iter().all(|m| m.first() != Some(&0x94)),
        "a new character's stub load sends no 0x94"
    );
    assert_eq!(
        got.iter().filter(|m| m.first() == Some(&0x23)).count(),
        2,
        "the join's two 0x23 (no StartSkill on synthetic data)"
    );
}

// Covers: specs/skills/use.md §7; specs/client/msg-skills.md §2 r3
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
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
