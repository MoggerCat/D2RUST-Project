// Spec: specs/ui/control-panel.md (§4 r1, §4 r2, §7 r2)
//! The tables the play HUD reads ([`HudTables`]): each skill's
//! `charclass` and skilldesc `IconCel` (§7 r2), the `experience` rows
//! (§4 r1) and the `stambarblue` states (§4 r2), from the user's `.bin`
//! set.

use bevy::prelude::*;
use d2_data::bin::TableFiles;
use d2_data::tables::{decode_all, Charstats, Experience, Skilldesc, Skills, States};

use crate::ui::char_feed::{CharTables, DescRow};
use crate::ui::original::hud::HudTables;
use crate::world_view::WorldViewUi;

/// The HUD tables of a live install.
pub fn hud_tables(archives: &dyn TableFiles) -> Result<HudTables, String> {
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    let table = |name: &str| set.table(name).ok_or(format!("{name} not loaded"));
    let skills: Vec<Skills> = decode_all(table("skills")?).map_err(|e| e.to_string())?;
    let descs: Vec<Skilldesc> = decode_all(table("skilldesc")?).map_err(|e| e.to_string())?;
    let exp: Vec<Experience> = decode_all(table("experience")?).map_err(|e| e.to_string())?;
    let states: Vec<States> = decode_all(table("states")?).map_err(|e| e.to_string())?;
    let mut tables = HudTables {
        // §4 r2: the states with flag bit 24 (`stambarblue`).
        stambarblue: states
            .iter()
            .enumerate()
            .filter(|(_, s)| s.stambarblue)
            .filter_map(|(id, _)| u8::try_from(id).ok())
            .collect(),
        ..HudTables::default()
    };
    for (id, s) in skills.iter().enumerate() {
        // d2rs-own, unverified: the `skilldesc` link is read as the
        // skilldesc row index.
        let (Ok(id), Some(d)) = (u16::try_from(id), descs.get(usize::from(s.skilldesc))) else {
            continue;
        };
        tables.icons.insert(id, (s.charclass, d.iconcel));
    }
    tables.experience = exp
        .iter()
        .map(|e| {
            [
                e.amazon,
                e.sorceress,
                e.necromancer,
                e.paladin,
                e.barbarian,
                e.druid,
                e.assassin,
            ]
        })
        .collect();
    Ok(tables)
}

/// Gives the original UI of `app` the tables of `archives`.
pub fn install_hud_tables(app: &mut App, archives: &dyn TableFiles) -> Result<(), String> {
    let tables = hud_tables(archives)?;
    set_hud_tables(app, tables);
    Ok(())
}

/// Gives the original UI of `app` (when installed) `tables`.
pub fn set_hud_tables(app: &mut App, tables: HudTables) {
    if let Some(mut ui) = app.world_mut().get_non_send_mut::<WorldViewUi>() {
        if let Some(o) = ui.original.as_mut() {
            o.set_hud_tables(tables);
        }
    }
}

/// The character panel's extra tables of a live install: the `charstats`
/// class keys, the `states` colour flags and each skill's skilldesc
/// `str name`, `descdam` and `descatt` (`ui::char_feed`).
pub fn char_tables(archives: &dyn TableFiles) -> Result<CharTables, String> {
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    let table = |name: &str| set.table(name).ok_or(format!("{name} not loaded"));
    let chars: Vec<Charstats> = decode_all(table("charstats")?).map_err(|e| e.to_string())?;
    let states: Vec<States> = decode_all(table("states")?).map_err(|e| e.to_string())?;
    let skills: Vec<Skills> = decode_all(table("skills")?).map_err(|e| e.to_string())?;
    let descs: Vec<Skilldesc> = decode_all(table("skilldesc")?).map_err(|e| e.to_string())?;
    let class_keys = chars
        .iter()
        .map(|c| {
            let n = c
                .class
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(c.class.len());
            String::from_utf8_lossy(&c.class[..n]).into_owned()
        })
        .collect();
    let state_flags = states
        .iter()
        .map(|s| {
            [
                s.armblue, s.rfblue, s.rcblue, s.rlblue, s.rpblue, s.armred, s.rfred, s.rcred,
                s.rlred, s.rpred,
            ]
            .iter()
            .enumerate()
            .fold(0u16, |a, (i, &f)| a | (u16::from(f) << i))
        })
        .collect();
    let mut skill_desc = std::collections::BTreeMap::new();
    for (id, s) in skills.iter().enumerate() {
        // d2rs-own, unverified: the `skilldesc` link is the row index
        // (as `hud_tables` reads it).
        if let (Ok(id), Some(d)) = (u16::try_from(id), descs.get(usize::from(s.skilldesc))) {
            skill_desc.insert(
                id,
                DescRow {
                    name_id: d.str_name,
                    descdam: d.descdam,
                    descatt: d.descatt,
                    src_dam: s.srcdam,
                },
            );
        }
    }
    let weapons: Vec<d2_data::tables::Weapons> =
        decode_all(table("weapons")?).map_err(|e| e.to_string())?;
    let weapons = weapons
        .iter()
        .map(|w| {
            (
                w.code,
                crate::ui::char_feed::WeaponRow {
                    str_bonus: i32::from(w.strbonus as i16),
                    dex_bonus: i32::from(w.dexbonus as i16),
                },
            )
        })
        .collect();
    Ok(CharTables {
        class_keys,
        state_flags,
        skill_desc,
        tohit_factor: chars.iter().map(|c| c.tohitfactor as i32).collect(),
        weapons,
    })
}

/// Gives the original UI of `app` the character panel's tables.
pub fn install_char_tables(app: &mut App, archives: &dyn TableFiles) -> Result<(), String> {
    let tables = char_tables(archives)?;
    if let Some(mut ui) = app.world_mut().get_non_send_mut::<WorldViewUi>() {
        if let Some(o) = ui.original.as_mut() {
            o.set_char_tables(tables);
        }
    }
    Ok(())
}

/// The skill tree's class skill rows of a live install
/// ([`crate::ui::skill_tree_ui::SkillTreeTables`]): `skills` joined with
/// `skilldesc` (page, row, column, `IconCel`) by the `skilldesc` link.
pub fn skill_tree_tables(
    archives: &dyn TableFiles,
) -> Result<crate::ui::skill_tree_ui::SkillTreeTables, String> {
    use crate::ui::skill_tree_ui::{SkillTreeRow, SkillTreeTables};
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    let table = |name: &str| set.table(name).ok_or(format!("{name} not loaded"));
    let skills: Vec<Skills> = decode_all(table("skills")?).map_err(|e| e.to_string())?;
    let descs: Vec<Skilldesc> = decode_all(table("skilldesc")?).map_err(|e| e.to_string())?;
    let mut t = SkillTreeTables::default();
    for (id, s) in skills.iter().enumerate() {
        let (Ok(id), Some(d)) = (u16::try_from(id), descs.get(usize::from(s.skilldesc))) else {
            continue;
        };
        t.rows.push(SkillTreeRow {
            skill: id,
            class: s.charclass,
            page: d.skillpage,
            row: d.skillrow,
            column: d.skillcolumn,
            icon_cel: d.iconcel,
            maxlvl: s.maxlvl,
            ingame: s.ingame,
            passive: s.passive,
            reqlevel: s.reqlevel,
            reqskill: [s.reqskill1, s.reqskill2, s.reqskill3],
            reqstr: s.reqstr,
            reqdex: s.reqdex,
            reqint: s.reqint,
            reqvit: s.reqvit,
        });
    }
    Ok(t)
}

/// Gives the original UI of `app` (when installed) the skill tree rows
/// of `archives`.
pub fn install_skill_tree_tables(app: &mut App, archives: &dyn TableFiles) -> Result<(), String> {
    let tables = skill_tree_tables(archives)?;
    if let Some(mut ui) = app.world_mut().get_non_send_mut::<WorldViewUi>() {
        if let Some(o) = ui.original.as_mut() {
            o.set_skill_tree_tables(tables);
        }
    }
    Ok(())
}
