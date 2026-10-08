// Spec: specs/ui/control-panel.md (§4 r1, §7 r2)
//! The tables the play HUD reads ([`HudTables`]): each skill's
//! `charclass` and skilldesc `IconCel` (§7 r2) and the `experience` rows
//! (§4 r1), from the user's `.bin` set.

use bevy::prelude::*;
use d2_data::bin::TableFiles;
use d2_data::tables::{decode_all, Experience, Skilldesc, Skills};

use crate::ui::original::hud::HudTables;
use crate::world_view::WorldViewUi;

/// The HUD tables of a live install.
pub fn hud_tables(archives: &dyn TableFiles) -> Result<HudTables, String> {
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    let table = |name: &str| set.table(name).ok_or(format!("{name} not loaded"));
    let skills: Vec<Skills> = decode_all(table("skills")?).map_err(|e| e.to_string())?;
    let descs: Vec<Skilldesc> = decode_all(table("skilldesc")?).map_err(|e| e.to_string())?;
    let exp: Vec<Experience> = decode_all(table("experience")?).map_err(|e| e.to_string())?;
    let mut tables = HudTables::default();
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
