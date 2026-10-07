// Spec: specs/formats/d2s.md (tests)
//! NPC section claims worked from the spec.

use super::*;

// Covers: specs/formats/d2s.md §6 r5
#[test]
fn fresh_npc_section_is_all_zero() {
    let n = Npcs::default();
    assert_eq!(n.a, [[0u8; 8]; 3]);
    assert_eq!(n.b, [[0u8; 8]; 3]);
}

// Covers: specs/formats/d2s.md §6 r6
#[test]
fn every_listed_talk_class_sets_one_nonzero_a_bit() {
    let classes = [
        147, 148, 150, 154, 175, 177, 178, 198, 199, 200, 202, 210, 245, 252, 254, 255, 264, 297,
        512, 513, 514, 515, 520,
    ];
    for c in classes {
        let mut n = Npcs::default();
        n.set_intro_a(0, c).unwrap();
        let bits: u32 = n.a[0].iter().map(|b| b.count_ones()).sum();
        assert_eq!(bits, 1, "class {c}");
        assert_ne!(n.a[0][0] & 1, 1, "class {c} must not fall back to bit 0");
        assert_eq!(n.b, [[0; 8]; 3], "class {c}");
    }
}

struct NoItems;

impl SaveTables for NoItems {
    fn stat_save(&self, _id: u16) -> Option<StatSave> {
        None
    }
    fn item_entry_len(&self, _buf: &[u8]) -> Result<usize, String> {
        Err("no items".into())
    }
}

/// A level-1 style body: 30 zero skill bytes, no items, no corpse, an
/// expansion-less game (no `jf`), and an empty golem section.
fn empty_body() -> D2s {
    D2s {
        header: Header::default(),
        body: Some(Body {
            skills: vec![0; 30],
            golem: Some(Golem::default()),
            ..Body::default()
        }),
    }
}

// Covers: specs/formats/d2s.md §8.3 r5, §8.5 r4, §7.2 r5
#[test]
fn empty_corpse_header_and_golem_tail_and_zero_skills() {
    let f = write(&empty_body(), &NoItems).unwrap();
    // The stats section holds no entries here (every id unsaved): `gf`
    // marker, then the 9-bit 0x1FF terminator in two bytes (§7.1 rule 3).
    let at = f.windows(2).position(|w| w == [0x69, 0x66]).unwrap();
    assert_eq!(&f[at - 2..at], &[0xFF, 0x01]);
    // §7.2 r5: `if` directly follows the stats byte end; 30 zero bytes.
    assert!(f[at + 2..at + 32].iter().all(|&b| b == 0));
    // §8.3 r5: no corpse → `4A 4D 00 00` right after the item list (an
    // empty list is `4A 4D 00 00` too, so the two sit back to back).
    let after = &f[at + 32..];
    assert_eq!(&after[..8], &[0x4A, 0x4D, 0, 0, 0x4A, 0x4D, 0, 0]);
    // §8.5 r4: g = 0 and the file ends `6B 66 00`.
    assert_eq!(&f[f.len() - 3..], &[0x6B, 0x66, 0x00]);
}

// Covers: specs/formats/d2s.md §edge-cases-original-bugs r16
#[test]
fn file_ending_after_kf_marker_is_rejected_with_23() {
    let mut s = empty_body();
    s.body.as_mut().unwrap().hireling_items = Some(None);
    let mut f = write(&s, &NoItems).unwrap();
    // Drop the g byte, then re-store size and checksum.
    f.pop();
    assert_eq!(&f[f.len() - 2..], &[0x6B, 0x66]);
    finish(&mut f);
    let o = ReadOptions {
        expansion: true,
        game: None,
    };
    let e = read(&f, &o, &NoItems).unwrap_err();
    assert_eq!(e.internal(), Some(23));
}

// Covers: specs/formats/d2s.md §edge-cases-original-bugs r4
#[test]
fn short_skills_section_is_rejected_with_19() {
    let mut s = empty_body();
    s.header.skill_count = 30;
    s.body.as_mut().unwrap().skills = vec![0; 5];
    let f = write(&s, &NoItems).unwrap();
    let o = ReadOptions {
        expansion: false,
        game: None,
    };
    assert_eq!(read(&f, &o, &NoItems).unwrap_err().internal(), Some(19));
}
