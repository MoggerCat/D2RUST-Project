// Spec: specs/formats/d2s.md (§2.2 r5), specs/sim/intents-events.md (§9 r6), specs/combat/vitals.md (§4.8 r2)
//! Hardcore and the saves of a death in the play preview.
//!
//! A hardcore character is marked by the save's status bit 0x4
//! (`d2s::status::HARDCORE`, or `play --hardcore` for `--new`). The game
//! then runs as a hardcore game: the server's client flag 4 is on
//! ([`super::single_player::LocalSeams::hardcore`]), so C→S 0x41 does not
//! respawn the dead player but drops the client with reason 3
//! (`handlers::player::resurrect`). Death is permanent: the character
//! save is written with the dead bit (0x8) as soon as the player dies, and
//! the next load refuses it (`d2s::read`, §2.2 r5: "dead hardcore
//! character"). The softcore character is saved at the same moment, so the
//! gold and experience the death took stay lost whichever way the window
//! is then closed.
//!
//! PROVISIONAL (M22; REC-126): the original saves at the DD start
//! (`vitals.md` §4.8 r2, `0x00532400`); this preview saves when the death
//! screen comes up (the DT start), from the app, because the server has no
//! save path of its own. Leaving a dead hardcore character closes the
//! game (the original returns to the character screen). d2rs-own,
//! unverified.

use bevy::app::AppExit;
use bevy::prelude::*;
use d2_formats::d2s;

use super::death::{is_dead, DeathScreen};
use super::save::SaveHandle;
use crate::bridge::BridgeResource;

/// Whether the save `bytes` belong to a hardcore character (status word
/// at 0x24, bit 0x4; `d2s.md` §2.1). A file shorter than the header is
/// not.
pub fn save_is_hardcore(bytes: &[u8]) -> bool {
    bytes
        .get(0x24..0x26)
        .is_some_and(|b| u16::from_le_bytes([b[0], b[1]]) & d2s::status::HARDCORE != 0)
}

/// The running character is hardcore.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HardcoreRun(pub bool);

/// `save`'s header marked as a dead hardcore character: a hardcore header
/// that has died gets the dead bit; anything else is left alone.
pub fn mark_dead(save: &mut d2s::D2s, hardcore: bool, died: bool) {
    if hardcore && died {
        save.header.status |= d2s::status::HARDCORE | d2s::status::DEAD;
    }
}

/// Saves the character once when the death screen comes up.
fn save_on_death(screen: Res<DeathScreen>, saver: Option<Res<SaveHandle>>, mut was: Local<bool>) {
    if screen.active && !*was {
        if let Some(h) = &saver {
            match h.save() {
                Ok(()) => info!("play: saved the character at its death"),
                Err(e) => warn!("play: the death was NOT saved: {e}"),
            }
        }
    }
    *was = screen.active;
}

/// Hardcore: Esc on the dead player (the server drops the client, 0x41)
/// closes the game; the dead save is already written.
fn leave_dead(
    run: Res<HardcoreRun>,
    bridge: ResMut<BridgeResource>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut exit: MessageWriter<AppExit>,
) {
    if run.0 && is_dead(bridge.0.world()) && keys.is_some_and(|k| k.just_pressed(KeyCode::Escape)) {
        println!("play: the hardcore character died; its save is marked dead");
        exit.write(AppExit::Success);
    }
}

/// Installs the save at death and, for a hardcore character, the leave.
pub fn add_hardcore(app: &mut App, hardcore: bool) {
    app.insert_resource(HardcoreRun(hardcore)).add_systems(
        Update,
        (
            save_on_death,
            leave_dead.run_if(resource_exists::<BridgeResource>),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/formats/d2s.md §2.2 r5
    #[test]
    fn the_status_bit_is_read_from_the_header() {
        let mut b = vec![0u8; 0x30];
        assert!(!save_is_hardcore(&b));
        b[0x24] = d2s::status::HARDCORE as u8;
        assert!(save_is_hardcore(&b));
        assert!(!save_is_hardcore(&b[..0x20]));
    }

    // Covers: specs/combat/vitals.md §4.8
    #[test]
    fn only_a_hardcore_death_marks_the_save_dead() {
        let mut s = super::super::save::base_save(&super::super::single_player::Character::New);
        s.header.status = 0;
        mark_dead(&mut s, false, true);
        assert_eq!(s.header.status & d2s::status::DEAD, 0);
        mark_dead(&mut s, true, false);
        assert_eq!(s.header.status & d2s::status::DEAD, 0);
        mark_dead(&mut s, true, true);
        assert_eq!(s.header.status, d2s::status::HARDCORE | d2s::status::DEAD);
    }
}
