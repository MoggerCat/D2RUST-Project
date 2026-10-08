// Spec: specs/ui/panels-2.md (§17 r3, r5), specs/ui/panels-3.md (§24), specs/skills/descriptions.md (§3, §4)
//! The character panel's remaining inputs from the client model: the
//! class line (`charstats` class name), the resist / defense effect
//! colours (`states.txt` flags), next-level experience and the damage /
//! attack-rating block of the left and right skill (`char_details`).
//! Nothing here decides an outcome; it reads the model and draws.
//!
//! PROVISIONAL (REC-269, d2rs-own, unverified): the damage block only
//! knows the plain weapon-physical `descdam` entries and the
//! attack-rating `descatt` entries 1 and 2, from the unit's stats
//! (min / max damage with the percent bonuses; attack rating with the
//! to-hit percent); other entries draw the skill name only.

use std::collections::{BTreeMap, BTreeSet};

use super::draw::UiDrawSink;
use super::layout::Screen;
use super::panel::StringLookup;
use super::panels::char_details::{self, BlockItem, HandBlock};
use super::panels::char_inputs::{self, ValueState};
use super::panels::character::ResistEffect;
use super::panels::{centered_in, text, utf16, TextMeasure};
use super::skill_desc::{ar_value, range_text};
use super::skill_desc_more::{pct, range_font};
use crate::bridge::world::{ClientWorld, UnitKey};

/// The skilldesc fields the damage block reads (`panels-2.md` §17 r5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DescRow {
    /// `str name` (+0x0E): a string id.
    pub name_id: u16,
    /// `descdam` (+0x12).
    pub descdam: u16,
    /// `descatt` (+0x14).
    pub descatt: u16,
}

/// The tables of the character panel's extra inputs (empty without game
/// files: nothing extra is drawn).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CharTables {
    /// `charstats` `class` key per class (the class line's string key).
    pub class_keys: Vec<String>,
    /// Per `states` row: bit `i` = the flag of [`ValueState::ALL`]`[i]`.
    pub state_flags: Vec<u16>,
    /// Skill id → its skilldesc fields.
    pub skill_desc: BTreeMap<u16, DescRow>,
}

/// `strchrskm`: "Damage".
const STR_DAMAGE: u16 = 4061;
/// `strchratr`: "%s\nAttack Rating".
const STR_ATTACK: u16 = 4063;
/// The skill name without a skilldesc row (`panels-2.md` §17 r5).
const STR_NO_NAME: u16 = 5382;
/// `holyshield` (`panels-3.md` §24 r3).
const STATE_HOLYSHIELD: u8 = 101;

/// Weapon-physical `descdam` entries this build computes (PROVISIONAL).
fn descdam_known(n: u16) -> bool {
    matches!(n, 1 | 7 | 18 | 19 | 20)
}

impl CharTables {
    fn has(&self, states: &BTreeSet<u8>, v: ValueState) -> bool {
        let i = ValueState::ALL.iter().position(|&x| x == v).unwrap_or(0);
        states.iter().any(|&s| {
            self.state_flags
                .get(usize::from(s))
                .is_some_and(|f| f >> i & 1 != 0)
        })
    }

    /// The resist effect of resist stat `stat` (39, 43, 41, 45): a red
    /// state wins over a blue one (`character.rs` `ResistEffect`).
    pub fn resist_effect(&self, states: &BTreeSet<u8>, stat: u16) -> ResistEffect {
        let (blue, red) = match stat {
            39 => (ValueState::RfBlue, ValueState::RfRed),
            43 => (ValueState::RcBlue, ValueState::RcRed),
            41 => (ValueState::RlBlue, ValueState::RlRed),
            45 => (ValueState::RpBlue, ValueState::RpRed),
            _ => return ResistEffect::None,
        };
        if self.has(states, red) {
            ResistEffect::Lowered
        } else if self.has(states, blue) {
            ResistEffect::Raised
        } else {
            ResistEffect::None
        }
    }

    /// The defense colour (`panels-3.md` §24 r3); `None` without the
    /// `states` flags (the caller keeps the §8.7 compare).
    pub fn defense_color(&self, states: &BTreeSet<u8>) -> Option<u16> {
        if self.state_flags.is_empty() {
            return None;
        }
        // d2rs-own, unverified: the shield test (item type 51) is not
        // read; `holyshield` alone stands for it.
        Some(u16::from(char_inputs::defense_color(
            self.has(states, ValueState::ArmBlue),
            states.contains(&STATE_HOLYSHIELD),
            self.has(states, ValueState::ArmRed),
        )))
    }
}

/// The class line (`panels-2.md` §17 r3): the class name in Font16,
/// colour 0, centred in [`sx + 193`, `sx + 310`].
pub fn class_line(
    class: u32,
    tables: &CharTables,
    strings: &dyn StringLookup,
    s: &Screen,
    measure: &dyn TextMeasure,
    out: &mut dyn UiDrawSink,
) {
    let Some(name) = usize::try_from(class)
        .ok()
        .and_then(|c| tables.class_keys.get(c))
        .and_then(|k| strings.get(k))
    else {
        return;
    };
    let Some(w) = measure.width(1, name) else {
        return;
    };
    let (a, b) = char_details::class_span(s);
    out.push(text(
        name.to_vec(),
        centered_in(a, b, w),
        char_details::line_y(s),
        1,
        0,
    ));
}

/// The next-level experience (`panels.md` §8.11): `experience` rows
/// (row 0 `MaxLvl`, row L + 1 level L) for the player's base level.
pub fn next_level(experience: &[[u32; 7]], class: u32, level: u32, stat30: u32) -> Option<u32> {
    let max = |c: u32| experience.first().map_or(0, |r| r[c as usize]);
    experience.first()?;
    Some(char_details::next_level_value(
        level,
        class,
        &max,
        &|c, l| {
            experience
                .get(l as usize + 1)
                .map_or(stat30, |r| r[c as usize])
        },
        stat30,
    ))
}

fn lossy(s: &[u16]) -> String {
    String::from_utf16_lossy(s)
}

/// The block of one hand (`0x004ED570`) from the unit's stats.
fn hand(
    skill: u16,
    row: &DescRow,
    strings: &dyn StringLookup,
    stat: &dyn Fn(u16) -> i32,
) -> HandBlock {
    let name = strings
        .get_id(row.name_id)
        .or_else(|| strings.get_id(STR_NO_NAME))
        .map(lossy)
        .unwrap_or_default();
    let _ = skill;
    let mut h = HandBlock {
        name: name.clone(),
        damage_label: strings.get_id(STR_DAMAGE).map(lossy).unwrap_or_default(),
        ..HandBlock::default()
    };
    h.damage = descdam_known(row.descdam);
    if matches!(row.descatt, 1 | 2) {
        h.attack = true;
        h.v1 = ar_value(stat(19), stat(119));
        h.attack_label = strings
            .get_id(STR_ATTACK)
            .map(|s| lossy(s).replace("%s", &name))
            .unwrap_or_default();
    }
    h
}

/// Draws the damage block (`panels-2.md` §17 r5) for the left then the
/// right skill of the local player.
#[allow(clippy::too_many_arguments)]
pub fn damage_block(
    world: &ClientWorld,
    key: UnitKey,
    tables: &CharTables,
    strings: &dyn StringLookup,
    s: &Screen,
    measure: &dyn TextMeasure,
    out: &mut dyn UiDrawSink,
) {
    let Some(list) = world.units.get(&key).and_then(|u| u.skills.as_ref()) else {
        return;
    };
    let stat = |id: u16| world.total(key, id, 0);
    let mut once = false;
    for (p, entry) in [(0usize, list.left_entry()), (6, list.right_entry())] {
        let Some(e) = entry else { continue };
        let Some(row) = tables.skill_desc.get(&e.skill) else {
            continue;
        };
        let h = hand(e.skill, row, strings, &stat);
        for item in char_details::hand_block(&h, p, s, false, &mut once) {
            match item {
                BlockItem::Text(t) => {
                    // The label lines: Font6 as `hand_block` names them.
                    let s16 = utf16(&t.text);
                    if let Some(w) = measure.width(t.font, &s16) {
                        out.push(text(
                            s16,
                            centered_in(t.x1, t.x2, w),
                            t.y,
                            t.font,
                            u16::from(t.color),
                        ));
                    }
                }
                BlockItem::DamageValue { x1, x2, y } => {
                    // d2rs-own, unverified (REC-269): min / max damage
                    // with the percent bonuses, no skill modifiers.
                    let mn = stat(21) + pct(stat(21), stat(18), 100);
                    let mx = stat(22) + pct(stat(22), stat(17), 100);
                    let s16 = utf16(&range_text(mn, mx));
                    if let Some(w) = measure.width(1, &s16) {
                        let (small, y) = range_font(w, x1, x2, y);
                        let font = if small { 6 } else { 1 };
                        if let Some(w) = measure.width(font, &s16) {
                            out.push(text(s16, centered_in(x1, x2, w), y, font, 0));
                        }
                    }
                }
            }
        }
    }
}
