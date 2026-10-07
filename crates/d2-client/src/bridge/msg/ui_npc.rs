// Spec: specs/client/msg-ui.md (§6, §9 r1–r2, §10 r1, §15 r1, §16 r1–r5, §17, §18)
//! NPC messages: 0x28 NPC dialog start and quest flags, 0x62 NPC dialog
//! end, 0x2A NPC transaction, 0x4E / 0x4F hire list, 0x8A NPC wants to
//! interact, 0x91 NPC intros, 0x9B hireling revive state. Each does its
//! model part at receive and emits one output for the UI layer.

use super::super::dispatch::{HandlerError, Message};
use super::super::output::{NpcDialog, Output};
use super::super::world::{ClientWorld, KindData, UnitKey, MONSTER, OBJECT};
use super::Bytes;

fn len(msg: &Message<'_>, n: usize, what: &'static str) -> Result<(), HandlerError> {
    if msg.bytes.len() == n {
        Ok(())
    } else {
        Err(HandlerError::Invalid(what))
    }
}

fn le(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

/// 0x28 QuestInfo (§16): type T u8@1, GUID G u32@2, R u8@6, quest flags
/// Q (96 bytes) @7.
pub fn quest_info(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    len(msg, 103, "0x28 is 103 bytes")?;
    let b = Bytes(msg.bytes);
    let (t, g, r) = (b.u8(1)?, b.u32(2)?, b.u8(6)?);
    let mut q = [0u8; 96];
    q.copy_from_slice(b.slice(7, 96)?);
    // Rule 2.
    if t == 6 {
        msg.out.push(Output::QuestFlags { record: q });
        return Ok(());
    }
    let key = UnitKey::new(t, g);
    if !w.units.contains_key(&key) {
        // Rule 3: the local player's player data +0x150 … +0x15C := 0
        // (no model field: no rule writes them otherwise, as 0x04,
        // `client/model.md` §7 rule 5); C→S 0x30 with u32 R, u32 G.
        let mut m = vec![0x30];
        m.extend(le(u32::from(r)));
        m.extend(le(g));
        w.outgoing.push(m);
        msg.out.push(Output::NpcGone { guid: g });
        return Ok(());
    }
    // Rule 4, the model part: unit flag 0x2 := 1, C→S 0x2F (u32 T, u32 G).
    let u = w.units.get_mut(&key).expect("checked above");
    u.flag_2 = Some(true);
    u.quest_untargetable = false;
    let class = u.class;
    let mut m = vec![0x2F];
    m.extend(le(u32::from(t)));
    m.extend(le(g));
    w.outgoing.push(m);
    // The captured inputs of the UI branch.
    let monsters = &msg.inputs.tables.monsters;
    let row = |c: u32| monsters.get(c as usize).copied().flatten();
    let cursor_item = matches!(
        w.local().map(|u| &u.kind),
        Some(KindData::Player(p)) if p.cursor_item.is_some()
    );
    let npc_monsters = w
        .units
        .values()
        .filter(|u| u.key.unit_type == MONSTER && row(u.class).is_some_and(|r| r.npc))
        .map(|u| u.key)
        .collect();
    // TODO(spec: client/msg-ui.md open question 10): the dialog branch
    // (§16 r4.3) decides on UI state and four of its effects write the
    // model (unit flag 0x2 := 0 in B0, U's mode and facing, the local
    // player's facing, U's path stop) and B2 sends C→S 0x31. Until the
    // question is settled the bridge does none of these; the UI layer
    // gets the captured inputs only.
    msg.out.push(Output::NpcDialog(Box::new(NpcDialog {
        kind: t,
        guid: g,
        quest_flags: q,
        unit: key,
        class,
        interact: row(class).is_some_and(|r| r.interact),
        f4b1a10: None,
        cursor_item,
        npc_monsters,
    })));
    Ok(())
}

/// 0x62 MakeUnitTargetable (§17): type T u8@1, GUID u32@2.
pub fn dialog_end(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    len(msg, 7, "0x62 is 7 bytes")?;
    let b = Bytes(msg.bytes);
    let t = b.u8(1)?;
    match t {
        1 => {
            if let Some(u) = w.units.get_mut(&UnitKey::new(MONSTER, b.u32(2)?)) {
                u.flag_2 = Some(true);
                u.quest_untargetable = false;
            }
        }
        2 | 4 | 6 => {}
        _ => return Ok(()),
    }
    msg.out.push(Output::NpcDialogEnd { kind: t });
    Ok(())
}

/// 0x2A NpcTransaction (§18): the 15 bytes and the local player's gold
/// (stat 14 total; 0 without a local player, a null unit).
pub fn npc_transaction(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    len(msg, 15, "0x2A is 15 bytes")?;
    let mut bytes = [0u8; 15];
    bytes.copy_from_slice(msg.bytes);
    let gold = w.local_player.map_or(0, |k| w.total(k, 14, 0));
    msg.out.push(Output::NpcTransaction { bytes, gold });
    Ok(())
}

/// 0x4E MercForHire (§6 r1): name u16@1, seed u32@3.
pub fn hire_offer(_: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    len(msg, 7, "0x4E is 7 bytes")?;
    let b = Bytes(msg.bytes);
    msg.out.push(Output::HireOffer {
        name: b.u16(1)?,
        seed: b.u32(3)?,
    });
    Ok(())
}

/// 0x4F StartMercList (§6 r2).
pub fn hire_list_reset(_: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    len(msg, 1, "0x4F is 1 byte")?;
    msg.out.push(Output::HireListReset);
    Ok(())
}

/// `eunuch harem blocker` (`objects.txt` 318, §9 r2).
pub const HAREM_BLOCKER: u32 = 318;

/// 0x8A NpcWantsInteract (§9 r1–r2): type u8@1, GUID u32@2.
pub fn npc_interact(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    len(msg, 6, "0x8A is 6 bytes")?;
    let b = Bytes(msg.bytes);
    let key = UnitKey::new(b.u8(1)?, b.u32(2)?);
    let unit = w.units.get(&key);
    let blocker_open = w
        .units
        .values()
        .any(|u| u.key.unit_type == OBJECT && u.class == HAREM_BLOCKER && u.mode == 2);
    msg.out.push(Output::NpcInteract {
        unit: key,
        present: unit.is_some(),
        class: unit.map_or(0, |u| u.class),
        mdata_3c: None,
        blocker_open,
    });
    Ok(())
}

/// 0x91 NpcGossipAct (§10 r1): u8@1 (not read), 12 class slots u16@2+2k.
pub fn npc_intro(_: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    len(msg, 26, "0x91 is 26 bytes")?;
    let b = Bytes(msg.bytes);
    let mut slots = [0u16; 12];
    for (k, s) in slots.iter_mut().enumerate() {
        *s = b.u16(2 + 2 * k)?;
    }
    msg.out.push(Output::NpcIntro { slots });
    Ok(())
}

/// 0x9B (§15 r1): u16@1, u16@3.
pub fn merc_revive(_: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    len(msg, 7, "0x9B is 7 bytes")?;
    let b = Bytes(msg.bytes);
    msg.out.push(Output::MercRevive {
        state: b.u16(1)?,
        value: b.u16(3)?,
    });
    Ok(())
}
