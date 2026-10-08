use std::collections::HashMap;

use super::character_bind_tests::tbl;
use super::*;
use crate::assets::path::MemorySource;
use crate::bridge::skills::{SkillEntry, SkillList};
use crate::bridge::world::{ClientUnit, PlayerData};
use crate::ui::char_feed::{CharTables, DescRow};
use crate::ui::{NoPanelRules, StringLookup, UiDraw};

struct Strs(HashMap<u16, Vec<u16>>, HashMap<String, Vec<u16>>);

fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

impl Strs {
    /// Every label id answers "S<id>" unless overridden.
    fn new(over: &[(u16, &str)]) -> Self {
        let mut ids: HashMap<u16, Vec<u16>> =
            (0..6000u16).map(|i| (i, u16s(&format!("S{i}")))).collect();
        for (i, t) in over {
            ids.insert(*i, u16s(t));
        }
        let keys = [("Barbarian".to_string(), u16s("Barbarian"))].into();
        Strs(ids, keys)
    }
}

impl StringLookup for Strs {
    fn get(&self, key: &str) -> Option<&[u16]> {
        self.1.get(key).map(Vec::as_slice)
    }
    fn get_id(&self, id: u16) -> Option<&[u16]> {
        self.0.get(&id).map(Vec::as_slice)
    }
}

fn fonts() -> FontMeasure {
    let mut src = MemorySource::default();
    for id in CHARACTER_FONTS {
        src.insert(crate::ui::font_info(id).unwrap().tbl_path, tbl(6));
    }
    FontMeasure::load(&src, &CHARACTER_FONTS).unwrap()
}

fn world(stats: &[(u16, i32)]) -> ClientWorld {
    let mut w = ClientWorld::default();
    let key = UnitKey::new(PLAYER, 1);
    let mut u = ClientUnit::new(key);
    u.class = 4;
    u.mode = 1;
    u.kind = KindData::Player(PlayerData::default());
    for &(s, v) in stats {
        u.stats.insert(s, v);
    }
    w.units.insert(key, u);
    w.local_player = Some(key);
    w
}

fn tables() -> CharTables {
    let mut t = CharTables {
        class_keys: ["Amazon", "Sorceress", "Necromancer", "Paladin", "Barbarian"]
            .map(String::from)
            .to_vec(),
        // State 5: rfblue (bit 1); state 6: armred (bit 5).
        state_flags: vec![0, 0, 0, 0, 0, 1 << 1, 1 << 5],
        ..Default::default()
    };
    t.skill_desc.insert(
        3,
        DescRow {
            name_id: 5000,
            descdam: 1,
            descatt: 2,
        },
    );
    t
}

type Text = (String, i32, i32, u16, u16);

fn draw(w: &ClientWorld, strings: &Strs, exp: Vec<[u32; 7]>) -> Vec<Text> {
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: true,
    };
    let mut ui = OriginalUi::new(config, None).unwrap();
    ui.set_fonts(fonts());
    ui.set_char_tables(tables());
    ui.set_hud_tables(hud::HudTables {
        experience: exp,
        ..Default::default()
    });
    let mut root = UiRoot::new(Box::new(NoPanelRules));
    ui.install(&mut root).unwrap();
    ui.set_ui(u32::from(UI_CHARACTER), 2, false).unwrap();
    root.sync_states(&ui.shared.borrow().states);
    let ctx = UiCtx {
        tick: 0,
        world: w,
        strings,
    };
    let mut out: Vec<UiDraw> = Vec::new();
    root.draw(&ctx, &mut out);
    out.iter()
        .filter_map(|d| match d {
            UiDraw::Text(t) => Some((
                String::from_utf16_lossy(&t.text),
                t.at.x,
                t.at.y,
                t.style.font,
                t.style.color,
            )),
            _ => None,
        })
        .collect()
}

fn find<'a>(v: &'a [Text], s: &str) -> Option<&'a Text> {
    v.iter().find(|t| t.0 == s)
}

// Covers: specs/ui/panels.md §8 r6
#[test]
fn labels_come_from_the_string_table_by_id() {
    let t = draw(&world(&[(12, 7)]), &Strs::new(&[]), vec![]);
    // The 15 label rows resolve their ids (`S<id>`), not nothing.
    let labels = t.iter().filter(|x| x.0.starts_with('S')).count();
    assert!(labels >= 15, "{labels} labels in {t:?}");
}

// Covers: specs/ui/panels-2.md §17 r3
#[test]
fn class_line_is_the_charstats_name_centered_in_its_span() {
    let t = draw(&world(&[]), &Strs::new(&[]), vec![]);
    let s = Screen::R800;
    // "Barbarian": 9 units of 6 in [sx + 193, sx + 310]: 118 wide.
    assert_eq!(
        find(&t, "Barbarian"),
        Some(&(
            "Barbarian".into(),
            s.sx() + 193 + ((118 - 54) >> 1),
            s.h + s.sy() - 455,
            1,
            0
        ))
    );
}

// Covers: specs/ui/panels.md §8 r11
#[test]
fn next_level_is_the_experience_row_of_the_base_level() {
    let mut rows = vec![[99u32; 7]; 10];
    rows[8][4] = 1_234_567; // level 7, class 4
    let t = draw(&world(&[(12, 7), (30, 5)]), &Strs::new(&[]), rows);
    assert!(find(&t, "1,234,567").is_some(), "{t:?}");
    // Without the table: stat 30.
    let t = draw(&world(&[(12, 7), (30, 5000)]), &Strs::new(&[]), vec![]);
    assert!(find(&t, "5,000").is_some());
}

// Covers: specs/ui/panels-3.md §24 r1, §24 r3
#[test]
fn resist_and_defense_take_their_colours_from_the_state_flags() {
    let mut w = world(&[(39, 10), (31, 77)]);
    let key = UnitKey::new(PLAYER, 1);
    let t = draw(&w, &Strs::new(&[]), vec![]);
    assert_eq!(find(&t, "10").map(|x| x.4), Some(0));
    assert_eq!(find(&t, "77").map(|x| x.4), Some(0));
    let u = w.units.get_mut(&key).unwrap();
    u.states.insert(5); // rfblue
    u.states.insert(6); // armred
    let t = draw(&w, &Strs::new(&[]), vec![]);
    assert_eq!(find(&t, "10").map(|x| x.4), Some(3));
    assert_eq!(find(&t, "77").map(|x| x.4), Some(1));
}

// Covers: specs/ui/panels-2.md §17 r5
#[test]
fn the_damage_block_prints_name_damage_and_attack_rating() {
    let mut w = world(&[(21, 10), (22, 20), (18, 50), (19, 100), (119, 10)]);
    let key = UnitKey::new(PLAYER, 1);
    w.units.get_mut(&key).unwrap().skills = Some(SkillList {
        entries: vec![SkillEntry {
            skill: 3,
            ..Default::default()
        }],
        left: Some(0),
        ..Default::default()
    });
    let strings = Strs::new(&[(5000, "Bash"), (4063, "%s\nAttack Rating")]);
    let t = draw(&w, &strings, vec![]);
    let s = Screen::R800;
    let (sx, base) = (s.sx(), s.h + s.sy() - 480);
    // e0: the upper-cased skill name, Font6, colour 0.
    let name = find(&t, "BASH").expect("name");
    assert_eq!((name.2, name.3, name.4), (base + 93, 6, 0));
    // e1: "Damage".
    assert_eq!(find(&t, "S4061").map(|x| x.2), Some(base + 101));
    // e2: min 10 + 50 % = 15, max 20: "15-20" is too wide for its span at
    // Font16 (30 × 11 / 7 > 44): Font6, one up.
    let dmg = find(&t, "15-20").expect("damage");
    assert_eq!((dmg.1, dmg.2, dmg.3), (sx + 263 + 7, base + 98 - 1, 6));
    // e3: the label split at its LF; e5: AR 100 + 10 % = 110, Font16.
    assert_eq!(find(&t, "Bash").map(|x| x.2), Some(base + 160 - 4));
    assert_eq!(find(&t, "Attack Rating").map(|x| x.2), Some(base + 160 + 4));
    let ar = find(&t, "110").expect("attack rating");
    assert_eq!((ar.2, ar.3, ar.4), (base + 160, 1, 0));
}
