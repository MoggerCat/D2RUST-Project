// Spec: specs/client/model.md (§14)
//! The pet list messages 0x7A PetAction and 0x81 AssignMerc. Fields are
//! read at the offsets `model.md` §14 states (0x7A: pet GUID u32@9, owner
//! GUID u32@5).

use super::super::dispatch::{HandlerError, Message};
use super::super::world::{ClientWorld, PetRecord, PET_HIRELING};
use super::Bytes;

/// The set `0x00478B10(pet, owner, type, class)` (§14 rule 2).
fn set(w: &mut ClientWorld, pet: u32, owner: u32, pet_type: u8, class: u16) {
    if pet_type == PET_HIRELING {
        // `0x00478AB0`: the type-7 record with that pet GUID is freed.
        if let Some(i) = w
            .pets
            .iter()
            .position(|r| r.pet_type == PET_HIRELING && r.pet == pet)
        {
            w.pets.remove(i);
        }
    }
    if let Some(r) = w.pets.iter_mut().find(|r| r.pet == pet) {
        r.pet_type = pet_type;
        r.owner = owner;
        r.class = class;
        r.gone = false;
    } else {
        w.pets.insert(
            0,
            PetRecord {
                class,
                pet_type,
                pet,
                owner,
                f1c: 100,
                gone: false,
                extra: None,
            },
        );
    }
}

/// The remove `0x00478C90(pet)` (§14 rule 2): the first record with that
/// pet GUID; the local player's hireling stays with gone := 1.
fn remove(w: &mut ClientWorld, pet: u32) {
    let local = w.local_player.map(|k| k.guid);
    let Some(i) = w.pets.iter().position(|r| r.pet == pet) else {
        return;
    };
    let r = &mut w.pets[i];
    if r.pet_type == PET_HIRELING && Some(r.owner) == local {
        r.gone = true;
    } else {
        w.pets.remove(i);
    }
}

/// 0x7A PetAction (`0x0045E860`, §14 rule 2).
pub fn pet_action(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    if msg.bytes.len() != 13 {
        return Err(HandlerError::Invalid("0x7A is 13 bytes"));
    }
    let b = Bytes(msg.bytes);
    let pet = b.u32(9)?;
    if b.u8(1)? != 0 {
        set(w, pet, b.u32(5)?, b.u8(2)?, b.u16(3)?);
    } else {
        remove(w, pet);
    }
    Ok(())
}

/// 0x81 AssignMerc (`0x0045E890`, §14 rule 3).
pub fn assign_merc(w: &mut ClientWorld, msg: &Message<'_>) -> Result<(), HandlerError> {
    if msg.bytes.len() != 20 {
        return Err(HandlerError::Invalid("0x81 is 20 bytes"));
    }
    let b = Bytes(msg.bytes);
    let pet = b.u32(8)?;
    set(w, pet, b.u32(4)?, b.u8(1)?, b.u16(2)?);
    let extra = [b.u32(0xC)?, b.u32(0x10)?, 0];
    match w.pets.iter_mut().find(|r| r.pet == pet) {
        Some(r) => r.extra = Some(extra),
        None => return Err(HandlerError::Fatal(0x95)),
    }
    Ok(())
}
