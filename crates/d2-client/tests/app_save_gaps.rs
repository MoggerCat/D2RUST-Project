// Spec: specs/formats/d2s.md §2.4, §8.4, §8.5; specs/formats/d2s-load.md §4
//! The save gaps (q-save-gaps): mouse skills of both weapon sets, the town
//! byte's act, the hireling's and the golem's items round-trip through a
//! written file field by field. d2rs-own, unverified (REC-241).

use d2_client::app::save_gaps::{apply_gaps, mouse_slots, select_mouse, Gaps};
use d2_formats::d2s::{self, Body, D2s, Golem, Header, ItemEntry, ReadOptions, Slot, StatSave};
use d2_sim::skills::list::{ListEntry, SkillList};

/// 32-bit stats; every item entry is 3 bytes.
struct Tables;

impl d2s::SaveTables for Tables {
    fn stat_save(&self, id: u16) -> Option<StatSave> {
        (id < 400).then_some(StatSave {
            bits: 32,
            param: 0,
            signed: true,
        })
    }
    fn item_entry_len(&self, _: &[u8]) -> Result<usize, String> {
        Ok(3)
    }
}

fn item(b: u8) -> ItemEntry {
    ItemEntry { bytes: vec![b; 3] }
}

fn base() -> D2s {
    let mut header = Header::default();
    header.set_name(b"Rolf").unwrap();
    header.class = 1;
    header.status = d2s::status::EXPANSION;
    header.towns = [0x80, 0, 0];
    header.hireling.seed = 77;
    // The weapon-swap pair as loaded (no sim state: it passes through).
    header.mouse[2] = Slot::encode(40, false, 0).unwrap();
    header.mouse[3] = Slot::encode(41, true, 0).unwrap();
    D2s {
        header,
        body: Some(Body {
            skills: vec![0; 30],
            hireling_items: Some(Some(vec![])),
            golem: Some(Golem {
                flag: 0,
                item: None,
            }),
            ..Body::default()
        }),
    }
}

fn entry(skill: u16, owner: i32) -> ListEntry {
    ListEntry {
        skill,
        base: 1,
        owner,
        ..ListEntry::default()
    }
}

/// Mouse skills, the act, the hireling's items and the golem's item are
/// written and read back field by field.
// Covers: specs/formats/d2s.md §2.4 r1, §2.4 r3, §2.4 r5, §8.4 r1, §8.5 r1
#[test]
fn gaps_round_trip_through_a_written_file() {
    let gaps = Gaps {
        mouse: Some([
            Slot::encode(36, true, 0).unwrap(),
            Slot::encode(59, false, 2).unwrap(),
        ]),
        town: Some((1, 3)),
        hireling_items: Some(vec![item(1), item(2)]),
        golem: Some(Some(item(7))),
    };
    let mut save = base();
    apply_gaps(&mut save, &gaps);
    let bytes = d2s::write(&save, &Tables).unwrap();
    let opts = ReadOptions {
        expansion: true,
        game: None,
    };
    let back = d2s::read(&bytes, &opts, &Tables).unwrap();
    let h = &back.header;
    assert_eq!(h.mouse[0], gaps.mouse.unwrap()[0]);
    assert_eq!(h.mouse[1], gaps.mouse.unwrap()[1]);
    assert_eq!(
        h.mouse[2],
        base().header.mouse[2],
        "swap left passes through"
    );
    assert_eq!(
        h.mouse[3],
        base().header.mouse[3],
        "swap right passes through"
    );
    // Nightmare's byte: act 3 | 0x80; the others are zero (§2.1).
    assert_eq!(h.towns, [0, 0x83, 0]);
    let b = back.body.unwrap();
    assert_eq!(b.hireling_items, Some(Some(vec![item(1), item(2)])));
    assert_eq!(
        b.golem,
        Some(Golem {
            flag: 1,
            item: Some(item(7))
        })
    );
}

/// Nothing read from the game leaves the loaded values as they were.
#[test]
fn no_gaps_keep_the_loaded_values() {
    let mut save = base();
    save.header.mouse[0] = Slot::encode(36, true, 0).unwrap();
    let before = save.clone();
    apply_gaps(&mut save, &Gaps::default());
    assert_eq!(save, before);
}

/// A golem without an item clears the section; a classic save has none.
#[test]
fn a_golem_without_an_item_writes_flag_zero() {
    let mut save = base();
    save.body.as_mut().unwrap().golem = Some(Golem {
        flag: 1,
        item: Some(item(9)),
    });
    apply_gaps(
        &mut save,
        &Gaps {
            golem: Some(None),
            ..Gaps::default()
        },
    );
    assert_eq!(
        save.body.unwrap().golem,
        Some(Golem {
            flag: 0,
            item: None
        })
    );
}

/// The mouse skills of a list encode with the 1-based position of the
/// owner item and select again on the loaded list (§2.4 rules 2, 4, 6).
// Covers: specs/formats/d2s.md §2.4 r2, §2.4 r4, §2.4 r6
#[test]
fn mouse_skills_select_with_their_item() {
    let guids = [500u32, 501, 502];
    let mut list = SkillList {
        entries: vec![entry(0, -1), entry(36, -1), entry(36, 501), entry(59, -1)],
        left: Some(2),
        right: Some(3),
        ..SkillList::default()
    };
    let slots = mouse_slots(&list, &guids);
    assert_eq!(slots[0], Slot::encode(36, true, 2).unwrap());
    assert_eq!(slots[1], Slot::encode(59, false, 0).unwrap());
    // The loaded list starts with nothing selected.
    let (l, r) = (list.left.take(), list.right.take());
    assert_eq!((l, r), (Some(2), Some(3)));
    select_mouse(&mut list, &slots, &guids);
    assert_eq!((list.left, list.right), (Some(2), Some(3)));
}

/// No left skill is the all-zero pair; an item past the list ends on the
/// native entry (item GUID −1); the right skill is always written.
#[test]
fn absent_and_unknown_mouse_entries() {
    let list = SkillList {
        entries: vec![entry(0, -1), entry(36, -1)],
        left: None,
        right: Some(1),
        ..SkillList::default()
    };
    let slots = mouse_slots(&list, &[]);
    assert_eq!(slots[0], Slot::default());
    assert_eq!(slots[1], Slot::encode(36, false, 0).unwrap());
    let mut loaded = SkillList {
        entries: vec![entry(0, -1), entry(36, -1)],
        ..SkillList::default()
    };
    // Left skill 36 on item index 5, which the list does not have.
    select_mouse(
        &mut loaded,
        &[Slot::encode(36, true, 5).unwrap(), slots[1]],
        &[],
    );
    assert_eq!((loaded.left, loaded.right), (Some(1), Some(1)));
    // The all-zero left pair selects nothing.
    let mut none = SkillList {
        entries: vec![entry(0, -1)],
        ..SkillList::default()
    };
    select_mouse(&mut none, &[Slot::default(), Slot::default()], &[]);
    assert_eq!((none.left, none.right), (None, Some(0)));
}

/// A hireling hired since the load has no header block yet, so its list
/// is not written (it would not read back).
#[test]
fn hireling_items_need_a_hireling_block() {
    let mut save = base();
    save.header.hireling = d2s::Hireling::default();
    save.body.as_mut().unwrap().hireling_items = Some(None);
    apply_gaps(
        &mut save,
        &Gaps {
            hireling_items: Some(vec![item(1)]),
            ..Gaps::default()
        },
    );
    assert_eq!(save.body.unwrap().hireling_items, Some(None));
}
