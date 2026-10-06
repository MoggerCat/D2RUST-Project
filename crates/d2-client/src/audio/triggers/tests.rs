// Spec: specs/audio/triggers.md (Test vectors)
//! Synthetic test vectors of `triggers.md` against a recording fake of the
//! sound layer (log of calls + scripted rolls).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use d2_data::tables::{Monsounds, Record};

use super::events::{player_event, server_event, EventExtra, Followup};
use super::modes::{convert_mode, impact, mode_sound};
use super::movement::{
    circular_distance, flee, footstep, footstep_called, footstep_material, init_voice, neutral,
    Floor,
};
use super::npc::{dialog_line, greet, set_npc_speech_option, DialogState, GreetMode, NpcGreeting};
use super::objects::object_mode;
use super::skills::{item_drop, skill_start, ItemRows, ItemSounds, SkillStart};
use super::tables::{NpcSpeech, ObjectSounds, TableError, NPC_SPEECH_TSV, OBJECT_SOUNDS_TSV};
use super::ui::{ui_action, UI_SOUNDS};
use super::*;
use crate::audio::calls::Handle;

const SPEC_MD: &str = include_str!("../../../../../specs/audio/triggers.md");

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Req(i32, Option<UnitKey>, u32, u32),
    Vol(Handle, i32),
    Fade(Handle, i32, u32, u32),
    Detach(Handle, UnitKey, bool),
    StopSpeech,
    Other(&'static str),
    Roll(i32, u32),
    Variant(i32),
}

#[derive(Default)]
struct Fake {
    log: Vec<Call>,
    rolls: VecDeque<u32>,
    variants: VecDeque<i32>,
    next: Handle,
    speaking: BTreeSet<UnitKey>,
    active: BTreeSet<Handle>,
    unit_reqs: BTreeMap<UnitKey, Vec<(Handle, i32)>>,
    bases: BTreeMap<i32, i32>,
    loops: BTreeSet<i32>,
    counts: BTreeMap<Handle, usize>,
    off: bool,
}

impl Fake {
    fn rolls(mut self, r: &[u32]) -> Self {
        self.rolls = r.iter().copied().collect();
        self
    }
    fn reqs(&self) -> Vec<(i32, Option<UnitKey>, u32)> {
        self.log
            .iter()
            .filter_map(|c| match c {
                Call::Req(id, u, d, _) => Some((*id, *u, *d)),
                _ => None,
            })
            .collect()
    }
    /// Requests with id ≥ 1 (the ones the sound layer makes).
    fn made(&self) -> Vec<(i32, Option<UnitKey>, u32)> {
        self.reqs().into_iter().filter(|r| r.0 >= 1).collect()
    }
    fn draws(&self) -> Vec<i32> {
        self.log
            .iter()
            .filter_map(|c| match c {
                Call::Roll(n, _) => Some(*n),
                _ => None,
            })
            .collect()
    }
}

impl SoundCalls for Fake {
    fn request(
        &mut self,
        id: i32,
        unit: Option<UnitKey>,
        delay: u32,
        flags: u32,
        _: u32,
    ) -> Handle {
        self.log.push(Call::Req(id, unit, delay, flags));
        if id < 1 || self.off {
            return 0;
        }
        self.next += 1;
        self.active.insert(self.next);
        self.next
    }
    fn set_volume(&mut self, h: Handle, v: i32) {
        self.log.push(Call::Vol(h, v));
    }
    fn fade(&mut self, h: Handle, t: i32, d: u32, l: u32) {
        self.log.push(Call::Fade(h, t, d, l));
    }
    fn detach(&mut self, h: Handle, u: UnitKey, f: bool) {
        self.log.push(Call::Detach(h, u, f));
    }
    fn stop_handle(&mut self, _: Handle) {
        self.log.push(Call::Other("stop_handle"));
    }
    fn stop_id(&mut self, _: i32) {
        self.log.push(Call::Other("stop_id"));
    }
    fn stop_songs(&mut self) {
        self.log.push(Call::Other("stop_songs"));
    }
    fn stop_52_71_except(&mut self, _: i32, _: i32) {
        self.log.push(Call::Other("stop_52_71"));
    }
    fn stop_72_201_except(&mut self, _: i32, _: i32) {
        self.log.push(Call::Other("stop_72_201"));
    }
    fn stop_speech(&mut self) {
        self.log.push(Call::StopSpeech);
    }
    fn speaking(&self, u: UnitKey) -> bool {
        self.speaking.contains(&u)
    }
    fn any_speech(&self) -> bool {
        !self.speaking.is_empty()
    }
    fn is_active(&self, h: Handle) -> bool {
        self.active.contains(&h)
    }
    fn sound_tick(&self) -> u32 {
        0
    }
    fn roll(&mut self, n: i32) -> u32 {
        let r = self.rolls.pop_front().expect("unscripted roll");
        assert!(
            (r as i32) < n.max(1),
            "scripted roll {r} out of range for {n}"
        );
        self.log.push(Call::Roll(n, r));
        r
    }
}

impl TriggerSound for Fake {
    fn sound_on(&self) -> bool {
        !self.off
    }
    fn group_base(&self, id: i32) -> i32 {
        self.bases.get(&id).copied().unwrap_or(id)
    }
    fn looping(&self, id: i32) -> bool {
        self.loops.contains(&id)
    }
    fn unit_requests(&self, u: UnitKey) -> Vec<(Handle, i32)> {
        self.unit_reqs.get(&u).cloned().unwrap_or_default()
    }
    fn unit_count(&self, h: Handle) -> usize {
        self.counts.get(&h).copied().unwrap_or(1)
    }
    fn variant(&mut self, id: i32) -> i32 {
        self.log.push(Call::Variant(id));
        self.variants.pop_front().unwrap_or(id)
    }
}

const P: UnitKey = UnitKey::new(PLAYER, 1);
const M: UnitKey = UnitKey::new(MONSTER, 7);
const O: UnitKey = UnitKey::new(OBJECT, 9);

fn player(class: i32) -> Unit<'static> {
    let mut u = Unit::new(P, class);
    u.is_local = true;
    u
}

fn zero_sounds() -> Monsounds {
    Monsounds::decode(&[0u8; Monsounds::SIZE])
}

fn monster(r: &Monsounds) -> Unit<'_> {
    let mut u = Unit::new(M, 100);
    u.monsounds = Some(r);
    u
}

fn run<T>(f: &mut Fake, g: &mut Globals, c: u32, body: impl FnOnce(&mut Ctx) -> T) -> T {
    let mut cx = Ctx::new(f, g, c);
    body(&mut cx)
}

// ---- spec-table checks (M05) ----

/// Rows of the markdown table whose first cell starts with a number.
fn md_rows(md: &str, header_start: &str) -> Vec<Vec<String>> {
    let mut lines = md.lines().skip_while(|l| !l.starts_with(header_start));
    lines.next();
    lines
        .skip(1)
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            l.trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_owned())
                .collect()
        })
        .collect()
}

fn num(s: &str) -> i32 {
    s.split_whitespace()
        .next()
        .unwrap()
        .replace(',', "")
        .parse()
        .unwrap()
}

/// Mismatches between the §3 r1 table in `md` and [`CLASS_RECORDS`].
fn class_table_diff(md: &str) -> Vec<(usize, usize)> {
    let rows = md_rows(md, "| Class | hit |");
    assert_eq!(rows.len(), 7);
    let mut diff = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let c = &CLASS_RECORDS[i];
        let code = [
            c.hit,
            c.death,
            c.impossible,
            c.needmana,
            c.needkey,
            c.cantcarry,
            c.cantuseyet,
            c.notintown,
            c.chat_base,
            c.quest_base,
            c.footstep_base,
        ];
        assert_eq!(num(&r[0]), i as i32);
        for (k, v) in code.iter().enumerate() {
            if num(&r[k + 1]) != *v {
                diff.push((i, k));
            }
        }
    }
    diff
}

// Covers: specs/audio/triggers.md §3 r1, §3 row1, §3 row2, §3 row3, §3 row4, §3 row5, §3 row6, §3 row7
#[test]
fn class_records_match_spec_table() {
    assert_eq!(class_table_diff(SPEC_MD), Vec::<(usize, usize)>::new());
    // M08: one perturbed cell is reported exactly.
    let bad = SPEC_MD.replacen(
        "| 2 necromancer | 2894 | 2899 | 3109",
        "| 2 necromancer | 2894 | 2899 | 3110",
        1,
    );
    assert_eq!(class_table_diff(&bad), vec![(2, 2)]);
    assert_eq!(class_record(7), Err(TriggerError::PlayerClass(7)));
}

/// Mismatches between the §3 r5 table and the code, by event.
fn fixed_events_diff(md: &str) -> Vec<u16> {
    let mut diff = Vec::new();
    for r in md_rows(md, "| e | Sound | Unit |") {
        if r[1] == "none" {
            continue;
        }
        let e: u16 = num(&r[0]) as u16;
        if e >= 19 {
            continue; // class speech lines: the class table above
        }
        let id = num(&r[1]);
        let on_u = r[2] == "U";
        let code = events::FIXED_EVENTS
            .iter()
            .find(|f| f.0 == e)
            .map(|f| (f.1, f.2))
            .or((e == events::EVENT_KEY_USED.0).then_some((events::EVENT_KEY_USED.1, true)));
        if code != Some((id, on_u)) {
            diff.push(e);
        }
    }
    diff
}

// Covers: specs/audio/triggers.md §3 r5, §3 t2 row1, §3 t2 row2, §3 t2 row3, §3 t2 row4, §3 t2 row5, §3 t2 row6, §3 t2 row7, §3 t2 row8, §3 t2 row9, §3 t2 row10
#[test]
fn fixed_events_match_spec_table() {
    assert!(fixed_events_diff(SPEC_MD).is_empty());
    let bad = SPEC_MD.replacen(
        "| 4 | 11 `cursor_convert_item` | none |",
        "| 4 | 11 `cursor_convert_item` | U |",
        1,
    );
    assert_eq!(fixed_events_diff(&bad), vec![4]);
}

fn ui_table_diff(md: &str) -> Vec<i32> {
    let rows = md_rows(md, "| Id | Sound | Sites |");
    let spec: Vec<(i32, String)> = rows
        .iter()
        .map(|r| (num(&r[0]), r[1].trim_matches('`').to_owned()))
        .collect();
    let mut diff = Vec::new();
    for (i, (id, name)) in spec.iter().enumerate() {
        if UI_SOUNDS.get(i).map(|&(a, b)| (a, b.to_owned())) != Some((*id, name.clone())) {
            diff.push(*id);
        }
    }
    if spec.len() != UI_SOUNDS.len() {
        diff.push(-1);
    }
    diff
}

// Covers: specs/audio/triggers.md §11 text, §11 row1, §11 row2, §11 row3, §11 row4, §11 row5, §11 row6, §11 row7, §11 row8
#[test]
fn ui_sounds_match_spec_table() {
    assert!(ui_table_diff(SPEC_MD).is_empty());
    let bad = SPEC_MD.replacen(
        "| 15 | `cursor_repair_item`",
        "| 14 | `cursor_repair_item`",
        1,
    );
    assert_eq!(ui_table_diff(&bad), vec![14]);
    let mut f = Fake::default();
    let mut g = Globals::default();
    run(&mut f, &mut g, 0, |cx| {
        ui::ui_sound(cx, ui::id::CURSOR_HOSTILE)
    });
    assert_eq!(f.reqs(), vec![(16, None, 0)]);
}

// ---- tables ----

// Covers: specs/audio/triggers.md §7 text, §10 r2
#[test]
fn spec_tsvs_parse_with_their_counts() {
    let o = ObjectSounds::spec();
    assert_eq!(o.rows().len(), 453);
    // The 115 distinct records of the Test vectors are record addresses
    // (real check); several hold equal contents, so the TSV shows fewer.
    let distinct: BTreeSet<_> = o.rows().iter().map(|r| r.record()).collect();
    assert!(distinct.len() <= 115);
    assert!(o.rows().iter().filter(|r| r.ordered).count() > 1);
    let n = NpcSpeech::spec();
    assert_eq!(n.rows().len(), 864);
    let keys: BTreeSet<i32> = n.rows().iter().map(|r| r.key).collect();
    assert_eq!(keys.len(), 863, "exactly one duplicate key");
    // First wins.
    let dup = n
        .rows()
        .iter()
        .find(|r| n.rows().iter().filter(|x| x.key == r.key).count() == 2)
        .unwrap();
    assert_eq!(n.sound(dup.key), dup.sound);
    assert_eq!(n.sound(14), 3488);
    assert_eq!(n.sound(-5), 0);
}

// Covers: specs/audio/triggers.md §7 text, §10 r2
#[test]
fn tsv_parsers_report_perturbations() {
    // M08: each perturbation is reported at its line and column.
    let bad = OBJECT_SOUNDS_TSV.replacen("27\t2569\t2568", "27\t2569\t25x8", 1);
    let line = OBJECT_SOUNDS_TSV
        .lines()
        .position(|l| l.starts_with("27\t"))
        .unwrap()
        + 1;
    assert_eq!(
        ObjectSounds::parse(&bad),
        Err(TableError::Value {
            table: "object-sounds.tsv",
            line,
            col: 2,
            value: "25x8".into()
        })
    );
    let bad = OBJECT_SOUNDS_TSV.replacen("\t2574\t1\t2574\t2\t0", "\t2574\t9\t2574\t2\t0", 1);
    assert!(matches!(
        ObjectSounds::parse(&bad),
        Err(TableError::Value { col: 10, .. })
    ));
    let bad = OBJECT_SOUNDS_TSV.replacen("\n27\t", "\n25\t", 1);
    assert!(matches!(
        ObjectSounds::parse(&bad),
        Err(TableError::Order { key: 25, .. })
    ));
    let bad = OBJECT_SOUNDS_TSV.replacen("ordered", "orderd", 1);
    assert!(matches!(
        ObjectSounds::parse(&bad),
        Err(TableError::Header { .. })
    ));
    let bad = NPC_SPEECH_TSV.replacen("\n3\t17\t3491", "\n3\t17\t4699", 1);
    assert_eq!(
        NpcSpeech::parse(&bad),
        Err(TableError::Value {
            table: "npc-speech.tsv",
            line: 5,
            col: 2,
            value: "4699".into()
        })
    );
    let bad = NPC_SPEECH_TSV.replacen("\n3\t17\t3491", "\n3\t17", 1);
    assert!(matches!(
        NpcSpeech::parse(&bad),
        Err(TableError::Cells { line: 5, .. })
    ));
    let bad = NPC_SPEECH_TSV.replacen("\n3\t17\t3491", "\n4\t17\t3491", 1);
    assert!(matches!(
        NpcSpeech::parse(&bad),
        Err(TableError::Order { line: 5, .. })
    ));
}

// ---- §2, §3 ----

// Covers: specs/audio/triggers.md §3 r5, §3 r6, §3 r7, §3 t2 row13, §edge-cases-original-bugs r5
#[test]
fn speech_guard_vectors() {
    let mut f = Fake::default();
    let mut g = Globals::default();
    let u = player(0);
    let out = run(&mut f, &mut g, 1000, |cx| player_event(cx, &u, 21)).unwrap();
    assert_eq!(f.reqs(), vec![(2957, Some(P), 0)]);
    assert_eq!(out, vec![Followup::OverheadText(2957)]);
    assert_eq!((g.speech_time, g.speech_id), (1000, 2957));
    run(&mut f, &mut g, 1074, |cx| player_event(cx, &u, 21)).unwrap();
    assert_eq!(f.reqs().len(), 1);
    // Another player's identical line is also held (global guard).
    let mut other = Unit::new(UnitKey::new(PLAYER, 2), 0);
    other.is_local = false;
    run(&mut f, &mut g, 1074, |cx| player_event(cx, &other, 21)).unwrap();
    assert_eq!(f.reqs().len(), 1);
    run(&mut f, &mut g, 1075, |cx| player_event(cx, &u, 21)).unwrap();
    assert_eq!(f.reqs().len(), 2);
    // Speaking: nothing, guard unchanged.
    f.speaking.insert(P);
    run(&mut f, &mut g, 5000, |cx| player_event(cx, &u, 19)).unwrap();
    assert_eq!(f.reqs().len(), 2);
    assert_eq!(g.speech_time, 1075);
}

// Covers: specs/audio/triggers.md §3 r3, §3 r4, §3 r2, §edge-cases-original-bugs r4
#[test]
fn chat_and_quest_lines() {
    let mut f = Fake::default();
    let mut g = Globals::default();
    let u = player(1);
    run(&mut f, &mut g, 0, |cx| player_event(cx, &u, 26)).unwrap();
    assert_eq!(f.reqs(), vec![(3251, Some(P), 0)]);
    let a = player(0);
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| player_event(cx, &a, 38)).unwrap();
    run(&mut f, &mut g, 0, |cx| player_event(cx, &a, 39)).unwrap();
    assert_eq!(f.reqs(), vec![(2966, Some(P), 0), (2967, Some(P), 12)]);
    // Stinger event: no request, even while speaking.
    f.speaking.insert(P);
    let out = run(&mut f, &mut g, 0, |cx| player_event(cx, &a, 33)).unwrap();
    assert_eq!(
        out,
        vec![Followup::QuestStinger {
            event: 33,
            id: 2961
        }]
    );
    // Chat while speaking: nothing.
    run(&mut f, &mut g, 0, |cx| player_event(cx, &a, 25)).unwrap();
    assert_eq!(f.reqs().len(), 2);
    // Local player dead: nothing.
    let mut dead = player(0);
    dead.mode = 17;
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| player_event(cx, &dead, 1)).unwrap();
    assert!(f.log.is_empty());
}

// Covers: specs/audio/triggers.md §3 t2 row1, §3 t2 row2, §3 t2 row11, §3 t2 row17, §edge-cases-original-bugs r6
#[test]
fn fixed_player_events() {
    let mut f = Fake::default();
    let mut g = Globals::default();
    let u = player(3);
    for e in [1, 2, 10, 11, 12, 18] {
        run(&mut f, &mut g, 0, |cx| player_event(cx, &u, e)).unwrap();
    }
    assert_eq!(
        f.reqs(),
        vec![(235, Some(P), 0), (7, None, 0), (228, Some(P), 0)]
    );
}

fn extra() -> EventExtra<'static> {
    EventExtra {
        local: Some(P),
        event12_stsound: 0,
        greeting: None,
        day_phase: 1,
    }
}

// Covers: specs/audio/triggers.md §2 r1, §2 r2, §2 r3, §2 row1, §2 row3, §2 row4, §2 row5, §2 row9, §2 row10, §2 row13, §2 row14, §2 row15, §2 row16
#[test]
fn server_events() {
    let mut f = Fake::default();
    let mut g = Globals::default();
    let mut us = UnitSound::default();
    let rogue = Unit::new(M, events::ROGUEHIRE);
    let other = Unit::new(M, 300);
    run(&mut f, &mut g, 0, |cx| {
        server_event(cx, &rogue, &mut us, 10, extra()).unwrap();
        server_event(cx, &other, &mut us, 13, extra()).unwrap();
        server_event(cx, &other, &mut us, 15, extra()).unwrap();
        server_event(cx, &rogue, &mut us, 15, extra()).unwrap();
        server_event(cx, &other, &mut us, 91, extra()).unwrap();
    });
    assert_eq!(
        f.log[..2],
        [Call::Req(2673, Some(M), 0, 0), Call::Vol(1, 120)]
    );
    assert_eq!(
        f.made(),
        vec![
            (2673, Some(M), 0),
            (2634, Some(M), 0),
            (4290, Some(M), 0),
            (8, Some(P), 0)
        ]
    );
    let mut f = Fake::default();
    let out = run(&mut f, &mut g, 0, |cx| {
        let a = server_event(cx, &rogue, &mut us, 84, extra()).unwrap();
        let b = server_event(cx, &other, &mut us, 87, extra()).unwrap();
        let c = server_event(cx, &other, &mut us, 92, extra()).unwrap();
        // Event 25 on a monster: not a player, nothing.
        let d = server_event(cx, &other, &mut us, 25, extra()).unwrap();
        [a, b, c, d]
    });
    assert_eq!(f.made(), vec![(4615, Some(M), 0), (4622, Some(M), 0)]);
    assert_eq!(out[0], vec![Followup::OverheadText(4615)]);
    assert_eq!(out[2], vec![Followup::StingerRearm]);
    assert!(out[3].is_empty());
    // Event 25 on a player: the chat line (§2 r3 → §3).
    let mut f = Fake::default();
    let p = player(0);
    run(&mut f, &mut g, 0, |cx| {
        server_event(cx, &p, &mut us, 25, extra())
    })
    .unwrap();
    assert_eq!(f.made(), vec![(2937, Some(P), 0)]);
}

// Covers: specs/audio/triggers.md §2 row6, §2 row7, §2 row8, §2 row2, §6 r3, §6 r4
#[test]
fn server_events_monster_voices() {
    let mut r = zero_sounds();
    r.taunt = 900;
    r.flee = 901;
    let u = monster(&r);
    let mut us = UnitSound::default();
    let mut g = Globals::default();
    let mut f = Fake::default().rolls(&[2]);
    run(&mut f, &mut g, 10, |cx| {
        server_event(cx, &u, &mut us, 16, extra()).unwrap();
        server_event(cx, &u, &mut us, 17, extra()).unwrap();
        // Flee again within 4 updates of the last voice: nothing, no draw.
        cx.c = 13;
        server_event(cx, &u, &mut us, 17, extra()).unwrap();
        let mut e = extra();
        e.event12_stsound = 55;
        server_event(cx, &u, &mut us, 12, e).unwrap();
    });
    assert_eq!(
        f.made(),
        vec![(900, Some(M), 0), (901, Some(M), 8), (55, Some(M), 0)]
    );
    assert_eq!((us.last_voice, g.last_voice_any), (10, 10));
    // Event 18: greeting on P with flags 1.
    let mut gr = NpcGreeting {
        ret: 77,
        greet: 70,
        ..Default::default()
    };
    let mut f = Fake::default();
    let mut e = extra();
    e.greeting = Some(&mut gr);
    run(&mut f, &mut g, 3, |cx| server_event(cx, &u, &mut us, 18, e)).unwrap();
    assert_eq!(f.log, vec![Call::Variant(70), Call::Req(70, Some(P), 0, 1)]);
}

// ---- §4 ----

// Covers: specs/audio/triggers.md §4.3 r1, §1 r9, §4.1 r2
#[test]
fn player_attack_swing() {
    let mut f = Fake::default();
    let mut g = Globals::default();
    let mut us = UnitSound::default();
    let mut u = player(0);
    u.weapon_hit_class = 5;
    u.speed = 256;
    run(&mut f, &mut g, 40, |cx| mode_sound(cx, &u, &mut us, 7)).unwrap();
    assert_eq!(f.reqs(), vec![(280, Some(P), 8)]);
    assert_eq!(us.last_voice, 40);
    assert_eq!(g.last_voice_any, 0);
    u.weapon_hit_class = 2;
    u.speed = 0;
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| mode_sound(cx, &u, &mut us, 8)).unwrap();
    assert_eq!(f.reqs(), vec![(262, Some(P), 1664)]);
    u.weapon_hit_class = 14;
    let mut f = Fake::default();
    assert_eq!(
        run(&mut f, &mut g, 0, |cx| mode_sound(cx, &u, &mut us, 7)),
        Err(TriggerError::SwingIndex(14))
    );
    // S3 attack only for barbarians; kick delay 4 for assassins.
    let mut f = Fake::default();
    let mut sorc = player(1);
    sorc.weapon_hit_class = 1;
    let mut sin = player(6);
    sin.weapon_hit_class = 1;
    run(&mut f, &mut g, 0, |cx| {
        mode_sound(cx, &sorc, &mut us, 15).unwrap();
        mode_sound(cx, &sin, &mut us, 16).unwrap();
        mode_sound(cx, &sin, &mut us, 12).unwrap();
        mode_sound(cx, &sorc, &mut us, 12).unwrap();
    });
    assert_eq!(
        f.reqs(),
        vec![(251, Some(P), 1664), (255, Some(P), 4), (255, Some(P), 0)]
    );
}

// Covers: specs/audio/triggers.md §4.2 r2
#[test]
fn impact_vectors() {
    assert_eq!(impact(0x22, MONSTER, false, 3), (341, 356));
    assert_eq!(impact(0x30, PLAYER, false, 0), (0, 359));
    assert_eq!(impact(0x35, MONSTER, false, 0), (341, 0));
    assert_eq!(impact(0x35, MONSTER, false, 3), (341, 362));
    assert_eq!(impact(0x35, MONSTER, true, 3), (0, 0));
    assert_eq!(impact(0x10, PLAYER, false, 0), (0, 0));
    assert_eq!(impact(0x51, PLAYER, false, 0), (321, 0));
    assert_eq!(impact(0x51, OBJECT, false, 0), (321, 376));
    assert_eq!(impact(0xBC, PLAYER, false, 0), (325, 391));
}

// Covers: specs/audio/triggers.md §4.2 r1, §4.2 r3, §4.2 r5, §4.2 r6, §4.2 r8, §4.1 r3, §edge-cases-original-bugs r3
#[test]
fn monster_hit_and_death() {
    let mut r = zero_sounds();
    r.hitsound = 600;
    r.hitdelay = 3;
    r.deathsound = 745;
    r.deadelay = 2;
    let mut u = monster(&r);
    let mut us = UnitSound {
        hit_class: 0x22,
        ..Default::default()
    };
    let mut g = Globals::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 50, |cx| mode_sound(cx, &u, &mut us, 3)).unwrap();
    assert_eq!(
        f.reqs(),
        vec![(341, Some(M), 0), (356, Some(M), 0), (600, Some(M), 3)]
    );
    assert_eq!((us.last_voice, g.last_voice_any), (50, 50));
    // Death: Diablo's extra lines, no speaking guard.
    let mut f = Fake::default();
    f.speaking.insert(M);
    us.hit_class = 0;
    run(&mut f, &mut g, 51, |cx| mode_sound(cx, &u, &mut us, 0)).unwrap();
    assert_eq!(
        f.made(),
        vec![(745, Some(M), 2), (746, Some(M), 106), (747, Some(M), 193)]
    );
    // Hit while speaking: impacts only, times still set.
    let mut f = Fake::default();
    f.speaking.insert(M);
    us.hit_class = 0x01;
    run(&mut f, &mut g, 52, |cx| mode_sound(cx, &u, &mut us, 13)).unwrap();
    assert_eq!(f.made(), vec![(321, Some(M), 0)]);
    assert_eq!(us.last_voice, 52);
    // Frozen: impacts only, no time update.
    u.frozen = true;
    let mut f = Fake::default();
    run(&mut f, &mut g, 60, |cx| mode_sound(cx, &u, &mut us, 3)).unwrap();
    assert_eq!(f.made(), vec![(321, Some(M), 0)]);
    assert_eq!(us.last_voice, 52);
    // State 146: nothing at all.
    u.state_146 = true;
    let mut f = Fake::default();
    run(&mut f, &mut g, 61, |cx| mode_sound(cx, &u, &mut us, 3)).unwrap();
    assert!(f.log.is_empty());
    // No record: nothing.
    let bare = Unit::new(M, 1);
    let mut f = Fake::default();
    run(&mut f, &mut g, 61, |cx| mode_sound(cx, &bare, &mut us, 3)).unwrap();
    assert!(f.log.is_empty());
}

// Covers: specs/audio/triggers.md §4.2 r7, §4.2 r3, §4.2 r4, §4.6 r1
#[test]
fn player_hit_death_and_monster_voice_stop() {
    let mut g = Globals::default();
    let mut us = UnitSound {
        hit_class: 0x01,
        ..Default::default()
    };
    let u = player(4);
    let mut f = Fake::default();
    run(&mut f, &mut g, 5, |cx| {
        mode_sound(cx, &u, &mut us, 4).unwrap();
        mode_sound(cx, &u, &mut us, 0).unwrap();
    });
    assert_eq!(
        f.log,
        vec![
            Call::Req(321, Some(P), 0, 0),
            Call::Req(0, Some(P), 0, 0),
            Call::Vol(1, 255),
            Call::Req(2902, Some(P), 2, 0),
            Call::Req(321, Some(P), 0, 0),
            Call::Req(0, Some(P), 0, 0),
            Call::Vol(3, 255),
            Call::Req(2907, Some(P), 1, 0),
        ]
    );
    // Monster death (m = 0) detaches its voices with force first.
    let mut r = zero_sounds();
    r.attack1 = 500;
    r.neutral = 510;
    let m = monster(&r);
    let mut f = Fake::default();
    f.bases.insert(502, 500);
    f.unit_reqs.insert(M, vec![(31, 502), (32, 999), (33, 510)]);
    us.hit_class = 0;
    run(&mut f, &mut g, 5, |cx| mode_sound(cx, &m, &mut us, 0)).unwrap();
    assert_eq!(
        f.log
            .iter()
            .filter(|c| matches!(c, Call::Detach(..)))
            .cloned()
            .collect::<Vec<_>>(),
        vec![Call::Detach(31, M, true), Call::Detach(33, M, true)]
    );
    // Mode 12 (DD) does the same.
    let mut f = Fake::default();
    f.unit_reqs.insert(M, vec![(33, 510)]);
    run(&mut f, &mut g, 5, |cx| mode_sound(cx, &m, &mut us, 12)).unwrap();
    assert_eq!(f.log, vec![Call::Detach(33, M, true)]);
}

// Covers: specs/audio/triggers.md §4.3 r2, §edge-cases-original-bugs r2
#[test]
fn monster_attack_voice() {
    let mut r = zero_sounds();
    r.attack1 = 500;
    r.att1del = 4;
    r.att1prb = 50;
    r.weapon1 = 520;
    r.wea1del = 1;
    r.wea1vol = 99;
    r.neutral = 510;
    r.attack2 = 501;
    r.att2prb = 100;
    let mut u = monster(&r);
    u.speed = 256;
    let mut us = UnitSound::default();
    // Within 15 of the last voice: no draw, weapon only.
    let mut g = Globals {
        last_voice_any: 100,
        ..Default::default()
    };
    let mut f = Fake::default();
    run(&mut f, &mut g, 114, |cx| mode_sound(cx, &u, &mut us, 4)).unwrap();
    assert_eq!(f.log, vec![Call::Req(520, Some(M), 1, 0), Call::Vol(1, 99)]);
    // Allowed: draw 49 < 50 → neutral detach / fade, attack voice, weapon.
    let mut f = Fake::default().rolls(&[49]);
    f.unit_reqs.insert(M, vec![(7, 510), (8, 511)]);
    f.bases.insert(511, 510);
    f.counts.insert(8, 2);
    run(&mut f, &mut g, 115, |cx| mode_sound(cx, &u, &mut us, 4)).unwrap();
    assert_eq!(
        f.log,
        vec![
            Call::Roll(100, 49),
            Call::Fade(7, 0, 0, 6),
            Call::Detach(8, M, true),
            Call::Req(500, Some(M), 4, 0),
            Call::Req(520, Some(M), 1, 0),
            Call::Vol(2, 99),
        ]
    );
    assert_eq!((us.last_voice, g.last_voice_any), (115, 115));
    // Draw 50: no voice.
    let mut f = Fake::default().rolls(&[50]);
    run(&mut f, &mut g, 200, |cx| mode_sound(cx, &u, &mut us, 4)).unwrap();
    assert_eq!(f.made(), vec![(520, Some(M), 1)]);
    // Speaking with Prb < 100: no draw.
    let mut f = Fake::default();
    f.speaking.insert(M);
    run(&mut f, &mut g, 300, |cx| mode_sound(cx, &u, &mut us, 4)).unwrap();
    assert!(f.draws().is_empty());
    // Prb ≥ 100: draws even while speaking and within the gap.
    let mut f = Fake::default().rolls(&[99]);
    f.speaking.insert(M);
    run(&mut f, &mut g, 301, |cx| mode_sound(cx, &u, &mut us, 5)).unwrap();
    assert_eq!(f.draws(), vec![100]);
    assert_eq!(f.made(), vec![(501, Some(M), 0)]);
}

// Covers: specs/audio/triggers.md §4.1 r3, §4.5, §4.4, §4.1 r4
#[test]
fn mode_conversion_skill_voice_block() {
    let mut r = zero_sounds();
    r.cvtmo1 = 4;
    r.cvtsk1 = u32::MAX;
    r.cvttgt1 = 8;
    r.cvtmo2 = 8;
    r.cvtsk2 = 5; // not < 0: no conversion
    r.cvttgt2 = 3;
    r.skill1 = 2692;
    assert_eq!(convert_mode(&r, 4), 8);
    assert_eq!(convert_mode(&r, 5), 5);
    let u = monster(&r);
    let mut us = UnitSound::default();
    let mut g = Globals::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 24, |cx| mode_sound(cx, &u, &mut us, 4)).unwrap();
    assert!(f.log.is_empty(), "chicken within 25 of U+0x7C = 0");
    run(&mut f, &mut g, 25, |cx| mode_sound(cx, &u, &mut us, 4)).unwrap();
    assert_eq!(f.reqs(), vec![(2692, Some(M), 0)]);
    assert_eq!((us.last_voice, g.last_voice_any), (25, 25));
    // Block by hit class.
    let p = player(3);
    let mut f = Fake::default();
    for h in [1u8, 10, 12] {
        us.hit_class = h;
        run(&mut f, &mut g, 30, |cx| mode_sound(cx, &p, &mut us, 9)).unwrap();
    }
    assert_eq!(
        f.reqs(),
        vec![(398, Some(P), 0), (401, Some(P), 0), (0, Some(P), 0)]
    );
    // Objects are not handled by the mode dispatch.
    let o = Unit::new(O, 27);
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| mode_sound(cx, &o, &mut us, 0)).unwrap();
    assert!(f.log.is_empty());
}

// ---- §5 ----

fn walker(r: &Monsounds) -> Unit<'_> {
    let mut u = monster(r);
    u.frame_count = 2048;
    u.speed = 256;
    u.frame = 1024;
    u.mode = 2;
    u
}

// Covers: specs/audio/triggers.md §5 r2, §5 r3, §5 r4, §5 r5, §5 r6, §5 r7
#[test]
fn footstep_vectors() {
    let mut r = zero_sounds();
    r.fscnt = 2;
    r.footstep = 700;
    let u = walker(&r);
    let mut g = Globals::default();
    let mut us = UnitSound {
        last_footstep: 100,
        ..Default::default()
    };
    // Elapsed 1 < ⌊8/3⌋ = 2: nothing.
    let mut f = Fake::default();
    run(&mut f, &mut g, 101, |cx| footstep(cx, &u, &mut us, 1)).unwrap();
    assert!(f.log.is_empty());
    // Elapsed 2: step, volume 200.
    run(&mut f, &mut g, 102, |cx| footstep(cx, &u, &mut us, 1)).unwrap();
    assert_eq!(
        f.log,
        vec![Call::Req(700, Some(M), 0, 0), Call::Vol(1, 200)]
    );
    assert_eq!(us.last_footstep, 102);
    // Elapsed 7 > 6: 200 → 125; material 3 → +8; running → +24.
    let mut f = Fake::default();
    let mut run_u = u;
    run_u.mode = 15;
    run(&mut f, &mut g, 109, |cx| footstep(cx, &run_u, &mut us, 3)).unwrap();
    assert_eq!(
        f.log,
        vec![Call::Req(732, Some(M), 0, 0), Call::Vol(1, 125)]
    );
    // Not at a step point (f = 1024 + 512): nothing.
    let mut off = u;
    off.frame = 1536;
    let mut f = Fake::default();
    run(&mut f, &mut g, 200, |cx| footstep(cx, &off, &mut us, 1)).unwrap();
    assert!(f.log.is_empty());
    // Player: class base, local volume 255 → 160.
    let mut p = player(4);
    p.frame_count = 2048;
    p.speed = 256;
    p.mode = 3;
    let mut us = UnitSound::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 1000, |cx| footstep(cx, &p, &mut us, 6)).unwrap();
    assert_eq!(
        f.log,
        vec![Call::Req(2816 + 20 + 24, Some(P), 0, 0), Call::Vol(1, 160)]
    );
    // Layer: roll(100) after the step, < FsPrb.
    r.footsteplayer = 750;
    r.fsprb = 30;
    let u = walker(&r);
    let mut us = UnitSound::default();
    let mut f = Fake::default().rolls(&[29]);
    run(&mut f, &mut g, 1000, |cx| footstep(cx, &u, &mut us, 0)).unwrap();
    assert_eq!(
        f.log,
        vec![
            Call::Req(700, Some(M), 0, 0),
            Call::Vol(1, 125),
            Call::Roll(100, 29),
            Call::Req(750, Some(M), 0, 0)
        ]
    );
    // Speed 0 or FsCnt 0: nothing.
    let mut z = u;
    z.speed = 0;
    let mut f = Fake::default();
    run(&mut f, &mut g, 1000, |cx| footstep(cx, &z, &mut us, 0)).unwrap();
    r.fscnt = 0;
    let z = walker(&r);
    run(&mut f, &mut g, 1000, |cx| footstep(cx, &z, &mut us, 0)).unwrap();
    assert!(f.log.is_empty());
}

// Covers: specs/audio/triggers.md §5 r4
#[test]
fn circular_distance_vectors() {
    assert_eq!(circular_distance(1024, 0, 1024), 0);
    assert_eq!(circular_distance(1280, 0, 1024), 256);
    assert_eq!(circular_distance(768, 0, 1024), 256);
    assert_eq!(circular_distance(10, 0, 12), 2);
    assert_eq!(circular_distance(1, 11, 12), 2);
}

// Covers: specs/audio/triggers.md §5 r1, §5 r8, §edge-cases-original-bugs r7
#[test]
fn footstep_callers_and_material() {
    assert!(footstep_called(PLAYER, 0, 2));
    assert!(footstep_called(PLAYER, 0, 19));
    assert!(!footstep_called(PLAYER, 0, 1));
    assert!(footstep_called(MONSTER, 3, 15));
    assert!(!footstep_called(MONSTER, 3, 1));
    assert!(footstep_called(MONSTER, 15, 1));
    assert!(footstep_called(MONSTER, 110, 8));
    assert!(!footstep_called(MONSTER, 110, 2));
    assert_eq!(footstep_material(3, Floor::NoRoom), 0);
    assert_eq!(footstep_material(3, Floor::NotFound), 3);
    assert_eq!(footstep_material(9, Floor::NotFound), 1);
    assert_eq!(footstep_material(3, Floor::Tile(0x28)), 1);
    assert_eq!(footstep_material(3, Floor::Tile(0x408)), 2);
    assert_eq!(footstep_material(3, Floor::Tile(0x400)), 5);
    assert_eq!(footstep_material(3, Floor::Tile(0x80)), 6);
    assert_eq!(footstep_material(4, Floor::Tile(0x01)), 4);
}

// ---- §6 ----

// Covers: specs/audio/triggers.md §6 r1
#[test]
fn neutral_voice() {
    let mut r = zero_sounds();
    r.neutral = 510;
    r.neutime = 10;
    let mut u = monster(&r);
    u.mode = 1;
    u.near_local = true;
    let mut us = UnitSound::default();
    let mut g = Globals::default();
    // All conditions hold: roll(15) = 0, request, then roll(61).
    let mut f = Fake::default().rolls(&[0, 20]);
    run(&mut f, &mut g, 100, |cx| neutral(cx, &u, &mut us));
    assert_eq!(
        f.log,
        vec![
            Call::Roll(15, 0),
            Call::Req(510, Some(M), 0, 0),
            Call::Roll(61, 20)
        ]
    );
    assert_eq!((g.last_idle_any, us.last_idle, g.idle_gap), (100, 100, 50));
    // Gap not reached: no draw.
    let mut f = Fake::default();
    run(&mut f, &mut g, 149, |cx| neutral(cx, &u, &mut us));
    assert!(f.log.is_empty());
    // Roll ≠ 0: no request.
    let mut f = Fake::default().rolls(&[3]);
    run(&mut f, &mut g, 150, |cx| neutral(cx, &u, &mut us));
    assert_eq!(f.log, vec![Call::Roll(15, 3)]);
    // Already a request in the group: no draw; but with the system off the
    // group test is false and the draw happens.
    let mut f = Fake::default();
    f.unit_reqs.insert(M, vec![(4, 510)]);
    run(&mut f, &mut g, 150, |cx| neutral(cx, &u, &mut us));
    assert!(f.log.is_empty());
    f.off = true;
    f.rolls = [1].into();
    run(&mut f, &mut g, 150, |cx| neutral(cx, &u, &mut us));
    assert_eq!(f.draws(), vec![15]);
    // Town, far away, wrong mode: no draw.
    for change in 0..3 {
        let mut v = u;
        match change {
            0 => v.in_town = true,
            1 => v.near_local = false,
            _ => v.mode = 8,
        }
        let mut f = Fake::default();
        run(&mut f, &mut g, 150, |cx| neutral(cx, &v, &mut us));
        assert!(f.log.is_empty());
    }
    // Mode 8 for base class 110.
    let mut v = u;
    v.mode = 8;
    v.base_class = 110;
    let mut f = Fake::default().rolls(&[1]);
    run(&mut f, &mut g, 150, |cx| neutral(cx, &v, &mut us));
    assert_eq!(f.draws(), vec![15]);
    assert!(within_700(699, 0));
    assert!(!within_700(0, 350));
}

// Covers: specs/audio/triggers.md §6 r2, §6 r4
#[test]
fn init_and_flee() {
    let mut r = zero_sounds();
    r.init = 530;
    r.skill1 = 540;
    r.flee = 550;
    let u = monster(&r);
    let mut us = UnitSound::default();
    let mut g = Globals::default();
    let mut f = Fake::default().rolls(&[0]);
    run(&mut f, &mut g, 7, |cx| init_voice(cx, &u, &mut us));
    assert_eq!(
        f.log,
        vec![Call::Req(530, Some(M), 0, 0), Call::Roll(61, 0)]
    );
    assert_eq!((us.last_idle, g.idle_gap), (7, 30));
    // Second time (U+0x80 ≠ 0), mode 8: skill voice.
    let mut v = u;
    v.mode = 8;
    let mut f = Fake::default();
    run(&mut f, &mut g, 8, |cx| init_voice(cx, &v, &mut us));
    assert_eq!(f.reqs(), vec![(540, Some(M), 0)]);
    // Flee: roll(3) before the request.
    let mut f = Fake::default().rolls(&[0]);
    run(&mut f, &mut g, 12, |cx| flee(cx, &u, &mut us));
    assert_eq!(f.log, vec![Call::Roll(3, 0), Call::Req(550, Some(M), 6, 0)]);
}

// ---- §7 ----

// Covers: specs/audio/triggers.md §7 r1, §7 r3, §7 r4, §7 r5, §7 r6
#[test]
fn object_vectors() {
    let t = ObjectSounds::spec();
    let mut g = Globals::default();
    // Door 27: first call m 0 only sets the fields; m 1 → transition 2568.
    let mut door = Unit::new(O, 27);
    let mut us = UnitSound::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| object_mode(cx, t, &door, &mut us)).unwrap();
    assert!(f.log.is_empty());
    door.mode = 1;
    run(&mut f, &mut g, 1, |cx| object_mode(cx, t, &door, &mut us)).unwrap();
    assert_eq!(f.reqs(), vec![(2568, Some(O), 0)]);
    // Same mode again: nothing.
    run(&mut f, &mut g, 2, |cx| object_mode(cx, t, &door, &mut us)).unwrap();
    assert_eq!(f.reqs().len(), 1);
    // Brazier 39, first call m 1: loop a, no transition.
    let mut br = Unit::new(O, 39);
    br.mode = 1;
    let mut us = UnitSound::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| object_mode(cx, t, &br, &mut us)).unwrap();
    assert_eq!(f.reqs(), vec![(2574, Some(O), 0)]);
    // m 2 with the loop already playing: no new loop request.
    br.mode = 2;
    f.unit_reqs.insert(O, vec![(1, 2574)]);
    run(&mut f, &mut g, 1, |cx| object_mode(cx, t, &br, &mut us)).unwrap();
    assert_eq!(f.reqs().len(), 1);
    // m 0: no loop → detach the looping requests (no force).
    br.mode = 0;
    f.loops.insert(2574);
    f.unit_reqs.insert(O, vec![(1, 2574), (2, 9)]);
    run(&mut f, &mut g, 2, |cx| object_mode(cx, t, &br, &mut us)).unwrap();
    assert_eq!(f.log.last(), Some(&Call::Detach(1, O, false)));
    // Well (ordered): 2 → 1 gives no transition; 1 → 2 does.
    let mut well = Unit::new(O, 111);
    well.mode = 2;
    let mut us = UnitSound {
        obj_seen: true,
        obj_prev_mode: 1,
        ..Default::default()
    };
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| object_mode(cx, t, &well, &mut us)).unwrap();
    well.mode = 1;
    run(&mut f, &mut g, 1, |cx| object_mode(cx, t, &well, &mut us)).unwrap();
    assert_eq!(f.reqs(), vec![(2644, Some(O), 0)]);
    // Town portal 59: transition on the first call.
    let mut tp = Unit::new(O, 59);
    tp.mode = 1;
    let mut us = UnitSound::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| object_mode(cx, t, &tp, &mut us)).unwrap();
    assert_eq!(f.reqs(), vec![(2633, Some(O), 0)]);
    // Class > 572: fatal.
    let bad = Unit::new(O, 573);
    assert_eq!(
        run(&mut f, &mut g, 0, |cx| object_mode(cx, t, &bad, &mut us)),
        Err(TriggerError::ObjectClass(573))
    );
}

// Covers: specs/audio/triggers.md §7 r2
#[test]
fn cain_gibbet_line() {
    let t = ObjectSounds::spec();
    let mut g = Globals::default();
    let mut cain = Unit::new(O, 26);
    cain.local_dist = 19;
    let mut us = UnitSound::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 33, |cx| object_mode(cx, t, &cain, &mut us)).unwrap();
    assert_eq!(f.reqs(), vec![(3671, Some(O), 0)]);
    assert_eq!(us.last_idle, 33);
    run(&mut f, &mut g, 34, |cx| object_mode(cx, t, &cain, &mut us)).unwrap();
    assert_eq!(f.reqs().len(), 1);
    let mut far = cain;
    far.local_dist = 20;
    let mut us = UnitSound::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 33, |cx| object_mode(cx, t, &far, &mut us)).unwrap();
    assert!(f.log.is_empty());
}

// ---- §8, §9 ----

// Covers: specs/audio/triggers.md §8 r1
#[test]
fn skill_start_sounds() {
    let mut g = Globals::default();
    let mut u = player(1);
    u.weapon_hit_class = 1;
    u.speed = 256;
    let sk = SkillStart {
        stsound: 600,
        stsoundclass: 610,
        stsounddelay: true,
        weaponsnd: true,
        stsuccessonly: false,
        charclass: 1,
        item_cast_sound: None,
    };
    // swing = (6·256+128)/256 = 6; delay 6 − 3 = 3.
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| skill_start(cx, &u, &sk, true)).unwrap();
    assert_eq!(
        f.reqs(),
        vec![(600, Some(P), 3), (251, Some(P), 6), (610, Some(P), 3)]
    );
    // Item cast sound replaces stsound and suppresses stsoundclass.
    let mut f = Fake::default();
    let item = SkillStart {
        item_cast_sound: Some(620),
        weaponsnd: false,
        ..sk
    };
    run(&mut f, &mut g, 0, |cx| skill_start(cx, &u, &item, true)).unwrap();
    assert_eq!(f.reqs(), vec![(620, Some(P), 3)]);
    // Negative delay passed as is (speed large: swing 0).
    u.speed = 100_000;
    let mut f = Fake::default();
    let only = SkillStart {
        weaponsnd: false,
        stsoundclass: 0,
        ..sk
    };
    run(&mut f, &mut g, 0, |cx| skill_start(cx, &u, &only, true)).unwrap();
    assert_eq!(f.reqs(), vec![(600, Some(P), (-3i32) as u32)]);
    // stsuccessonly and a failed start: nothing.
    let mut f = Fake::default();
    let so = SkillStart {
        stsuccessonly: true,
        ..sk
    };
    run(&mut f, &mut g, 0, |cx| skill_start(cx, &u, &so, false)).unwrap();
    assert!(f.log.is_empty());
}

// Covers: specs/audio/triggers.md §8 r2, §8 r3, §8 r4
#[test]
fn skill_missile_state_sounds() {
    let mut g = Globals::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| {
        skills::skill_do(cx, P, Some(M), 10, 11);
        skills::skill_do(cx, P, None, 0, 11);
        skills::missile_hit(cx, O, -1);
        skills::missile_hit(cx, O, 0);
        skills::state_on(cx, M, false, true, true, 20);
        skills::state_on(cx, M, true, false, false, 21);
        skills::state_on(cx, M, false, false, false, 22);
        skills::state_off(cx, M, true, 23);
        skills::state_off(cx, M, false, 24);
    });
    assert_eq!(
        f.reqs(),
        vec![
            (10, Some(P), 0),
            (11, Some(M), 0),
            (0, Some(O), 0),
            (22, Some(M), 0),
            (23, Some(M), 0)
        ]
    );
}

// Covers: specs/audio/triggers.md §9 r1, §9 r2, §9 r3, §9 r4, §9 r5, §edge-cases-original-bugs r1
#[test]
fn item_sounds() {
    let base = ItemSounds {
        dropsound: 300,
        dropsfxframe: 0,
        usesound: 301,
    };
    let uniq = ItemSounds {
        dropsound: 310,
        dropsfxframe: 0,
        usesound: 311,
    };
    let plain = ItemRows {
        base,
        quality: 2,
        special: None,
    };
    assert_eq!(plain.drop_sound(), (300, 12));
    let u = ItemRows {
        quality: 7,
        special: Some(uniq),
        ..plain
    };
    assert_eq!(u.drop_sound(), (310, 12));
    assert_eq!(u.use_sound(), 311);
    let s = ItemRows {
        quality: 5,
        special: Some(ItemSounds {
            dropsfxframe: 4,
            ..uniq
        }),
        ..plain
    };
    assert_eq!(s.drop_sound(), (310, 4));
    // Unique row without a drop sound: the base stays.
    let nu = ItemRows {
        quality: 7,
        special: Some(ItemSounds {
            dropsound: 0,
            ..uniq
        }),
        ..plain
    };
    assert_eq!(nu.use_sound(), 301);
    // Use sound only when the base dropsound is set.
    let nodrop = ItemRows {
        base: ItemSounds {
            dropsound: 0,
            ..base
        },
        ..plain
    };
    assert_eq!(nodrop.use_sound(), 0);
    let item = UnitKey::new(4, 3);
    let mut g = Globals::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| {
        item_drop(cx, item, &plain);
        skills::item_to_cursor(cx);
        skills::item_place(cx, &u);
        skills::item_use(cx, &nodrop);
        skills::item_gold(cx);
    });
    assert_eq!(
        f.log,
        vec![
            Call::Req(216, Some(item), 0, 0),
            Call::Req(300, Some(item), 12, 0),
            Call::Vol(2, 180),
            Call::Req(235, None, 0, 0),
            Call::Req(310, None, 0, 0),
            Call::Req(0, None, 0, 0),
            Call::Req(221, None, 0, 0),
        ]
    );
}

// ---- §10 ----

// Covers: specs/audio/triggers.md §10 r1, §edge-cases-original-bugs r8
#[test]
fn greeting_picks() {
    let mut g = Globals::default();
    // Akara-like record, mode 2: return line, no draw, no variant.
    let mut akara = NpcGreeting {
        greet: 100,
        inactive: 110,
        time: 120,
        ret: 130,
        ..Default::default()
    };
    let mut f = Fake::default();
    let pick = run(&mut f, &mut g, 5, |cx| {
        greet(cx, &mut akara, GreetMode::Return, 1)
    });
    assert_eq!(pick, 130);
    assert!(f.log.is_empty());
    // Mode 0: roll(2) = 0 → inactive; roll(3) = 1 keeps it; variant.
    let mut f = Fake::default().rolls(&[0, 1]);
    f.variants = [112].into();
    let pick = run(&mut f, &mut g, 6, |cx| {
        greet(cx, &mut akara, GreetMode::Idle, 2)
    });
    assert_eq!(pick, 112);
    assert_eq!(
        f.log,
        vec![Call::Roll(2, 0), Call::Roll(3, 1), Call::Variant(110)]
    );
    assert_eq!((akara.last, akara.tick), (112, 6));
    // Same pick as last → retry; time line for day phase 2–3 = +1.
    let mut f = Fake::default().rolls(&[1, 0, 1, 2]);
    f.variants = [112, 121].into();
    let pick = run(&mut f, &mut g, 7, |cx| {
        greet(cx, &mut akara, GreetMode::Idle, 3)
    });
    assert_eq!(pick, 121);
    assert_eq!(
        f.log,
        vec![
            Call::Roll(2, 1),
            Call::Roll(3, 0),
            Call::Variant(121),
            Call::Roll(2, 1),
            Call::Roll(3, 2),
            Call::Variant(100)
        ]
    );
    // Warriv-like (greet 0, time 0), mode 1: variant(0) every attempt until
    // it differs from last; here last = 0, so all 20 attempts, 20th kept.
    let mut w = NpcGreeting::default();
    let mut f = Fake::default();
    let pick = run(&mut f, &mut g, 8, |cx| {
        greet(cx, &mut w, GreetMode::Event, 1)
    });
    assert_eq!(pick, 0);
    assert_eq!(f.log, vec![Call::Variant(0); 20]);
    // time ≠ 0 and s = 0: no roll(3), time line for phase 4 = +2.
    let mut t = NpcGreeting {
        time: 50,
        ..Default::default()
    };
    let mut f = Fake::default();
    run(&mut f, &mut g, 8, |cx| {
        greet(cx, &mut t, GreetMode::Event, 4)
    });
    assert_eq!(f.log, vec![Call::Variant(52)]);
}

// Covers: specs/audio/triggers.md §10 r2, §10 r3
#[test]
fn dialog_lines() {
    let table = NpcSpeech::spec();
    let mut g = Globals::default();
    let mut st = DialogState::default();
    let npc = Unit::new(M, 148);
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| {
        dialog_line(cx, &mut st, table, &npc, Some(P), 14)
    });
    assert_eq!(
        f.log,
        vec![Call::StopSpeech, Call::Req(3488, Some(P), 5, 0)]
    );
    assert_eq!((st.handle, st.id), (1, 3488));
    // Next line: fade the previous one over 4, stop speech, request.
    run(&mut f, &mut g, 1, |cx| {
        dialog_line(cx, &mut st, table, &npc, Some(P), 15)
    });
    assert_eq!(
        f.log[2..],
        [
            Call::Fade(1, 0, 0, 4),
            Call::StopSpeech,
            Call::Req(3489, Some(P), 5, 0)
        ]
    );
    // Option 1 disables the lines (text on).
    assert!(set_npc_speech_option(&mut st, 1));
    let mut f = Fake::default();
    run(&mut f, &mut g, 2, |cx| {
        dialog_line(cx, &mut st, table, &npc, Some(P), 16)
    });
    assert_eq!(f.log, vec![Call::StopSpeech]);
    assert!(!set_npc_speech_option(&mut st, 0));
    assert!(st.enabled);
    assert!(set_npc_speech_option(&mut st, 2));
    assert!(st.enabled);
    // Unknown key: no request.
    let mut f = Fake::default();
    run(&mut f, &mut g, 2, |cx| {
        dialog_line(cx, &mut st, table, &npc, Some(P), 60000)
    });
    assert_eq!(f.log, vec![Call::StopSpeech]);
}

// ---- §11 ----

// Covers: specs/audio/triggers.md §11 text
#[test]
fn ui_action_sounds() {
    let mut g = Globals::default();
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| {
        ui_action(cx, 1, 33, 0);
        ui_action(cx, 2, 4, 0);
        ui_action(cx, 2, 18, 0);
        ui_action(cx, 2, 32, 0);
        ui_action(cx, 2, 33, 0);
        ui_action(cx, 2, 99, 0);
        ui_action(cx, 0x10, 10, 0);
        ui_action(cx, 0x10, 33, 1234);
        ui_action(cx, 0x11, 33, 1234);
    });
    let ids: Vec<i32> = f.reqs().iter().map(|r| r.0).collect();
    assert_eq!(ids, vec![237, 241, 7, 217, 243, 2456, 2474, 1234, 237]);
    assert!(f.reqs().iter().all(|r| r.1.is_none() && r.2 == 0));
}

// Covers: specs/audio/triggers.md §2 row11, §2 row12, §3 t2 row12, §3 t2 row14, §3 t2 row15, §3 t2 row16
#[test]
fn hireling_and_class_speech_events() {
    let mut g = Globals::default();
    let mut us = UnitSound::default();
    let rogue = Unit::new(M, events::ROGUEHIRE);
    let other = Unit::new(M, 300);
    let mut f = Fake::default();
    run(&mut f, &mut g, 0, |cx| {
        for e in [85, 86] {
            server_event(cx, &rogue, &mut us, e, extra()).unwrap();
            server_event(cx, &other, &mut us, e, extra()).unwrap();
        }
    });
    let ids: Vec<i32> = f.made().iter().map(|r| r.0).collect();
    assert_eq!(ids, vec![4612, 4616, 4613, 4619]);
    let u = player(0);
    let mut f = Fake::default();
    // Distinct ids: the global guard does not hold them.
    run(&mut f, &mut g, 0, |cx| {
        for e in [20, 22, 23, 24] {
            player_event(cx, &u, e).unwrap();
        }
    });
    let ids: Vec<i32> = f.made().iter().map(|r| r.0).collect();
    assert_eq!(ids, vec![2936, 2955, 2934, 2959]);
}
