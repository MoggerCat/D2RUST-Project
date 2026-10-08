// Spec: specs/ui/panels.md (§3 r1), specs/sim/intents-events.md (§9 r6), specs/combat/vitals.md (§4.8)
//! The local player's death in the play preview: the death screen text
//! while the player is dying or dead, and Esc as the respawn path
//! (`ui/panels.md` §3 r1: the escape menu does not open for a dead
//! player, `0x004647D0` sends C→S 0x41 and returns 0).
//!
//! The server owns the outcome (it starts DT when the life reaches 0,
//! makes the corpse, and answers 0x41 with the respawn in town,
//! `d2_server::adapters::handlers::player::resurrect`); this module only
//! shows the state and sends the intent.
//!
//! PROVISIONAL (M22; REC-98): no spec gives the text or the screen
//! shown while dead. The preview draws one line of text, "You have
//! died", with the Esc hint; the original's wording and layout are
//! unverified. `// d2rs-own, unverified`.

use bevy::prelude::*;

use crate::bridge::modes::player_mode;
use crate::bridge::world::ClientWorld;
use crate::bridge::BridgeResource;

/// The text of the death screen. d2rs-own, unverified.
pub const DEATH_TEXT: &str = "You have died\nPress Esc to return to town";

/// The C→S 0x41 Resurrect (1 byte, `sim/client-messages.tsv`).
pub const RESURRECT: [u8; 1] = [0x41];

/// Whether the death screen is up.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeathScreen {
    pub active: bool,
}

/// The death screen's text entity.
#[derive(Component)]
pub struct DeathText;

/// The local player is dying (mode 0) or dead (mode 0x11).
pub fn is_down(w: &ClientWorld) -> bool {
    w.local()
        .is_some_and(|u| u.mode == player_mode::DEATH || u.mode == player_mode::DEAD)
}

/// The local player lies dead (mode 0x11): the only mode the server
/// accepts 0x41 in (`intents-events.md` §2.3 r3).
pub fn is_dead(w: &ClientWorld) -> bool {
    w.local().is_some_and(|u| u.mode == player_mode::DEAD)
}

fn spawn_text(mut commands: Commands) {
    commands.spawn((
        DeathText,
        Text::new(DEATH_TEXT),
        TextFont {
            font_size: bevy::text::FontSize::Px(28.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.1, 0.1)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(35.0),
            left: Val::Percent(35.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

/// Shows the screen while the player is down; Esc while dead sends 0x41.
fn death_screen(
    mut bridge: ResMut<BridgeResource>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut screen: ResMut<DeathScreen>,
    mut texts: Query<&mut Visibility, With<DeathText>>,
) {
    let w = bridge.0.world();
    let down = is_down(w);
    let respawn = is_dead(w) && keys.is_some_and(|k| k.just_pressed(KeyCode::Escape));
    if screen.active != down {
        screen.active = down;
        for mut v in &mut texts {
            *v = if down {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    if respawn {
        if let Err(e) = bridge.0.send_bytes(&RESURRECT) {
            warn!("respawn request refused: {e}");
        }
    }
}

/// Installs the death screen and the respawn key.
pub fn add_death(app: &mut App) {
    app.init_resource::<DeathScreen>()
        .add_systems(Startup, spawn_text)
        .add_systems(
            Update,
            death_screen.run_if(resource_exists::<BridgeResource>),
        );
}
