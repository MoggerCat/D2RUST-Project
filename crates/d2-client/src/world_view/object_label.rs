// Spec: specs/world/objects-client.md (§25 hover target); preview fill: docs/PLAN.md decision D1
//! The mouse-over label of an object in the `play` preview: the object's
//! name drawn over the hovered object.
//!
//! PROVISIONAL (REC-239; d2rs-own, unverified): no spec states the label
//! (its font, colour, position or string source). The preview takes the
//! hover pick (`bridge::hover::pick`), keeps objects only (unit type 2),
//! and draws the name of the object's `objects.txt` row: the string table
//! entry whose key is the row's `Name`, else the `Name` itself. The text
//! is centred above the object's feet, in the chat font, colour 0.
//! Monsters and items keep their own paths.

use crate::bridge::hover::{feet, HIT_ABOVE};
use crate::bridge::world::{ClientWorld, UnitKey};
use crate::rules::camera::Camera;
use crate::ui::messages::FONT_CHAT;
use crate::ui::text::TextOpts;
use crate::ui::{Point, StringLookup, TextRequest, TextStyle, UiDraw, FRAME};

/// The unit type of an object (`client/model.md` §1).
pub const OBJECT: u8 = 2;

/// The `objects.txt` `Name` of each class, by class id. The default has
/// no names: no label.
#[derive(Clone, Debug, Default)]
pub struct ObjectLabels {
    pub names: Vec<String>,
}

impl ObjectLabels {
    pub fn new(names: Vec<String>) -> Self {
        ObjectLabels { names }
    }

    /// The label text of class `class`: the string for the row's `Name`
    /// key, else the name itself (UTF-16); none when the row is blank.
    pub fn text(&self, class: u32, strings: &dyn StringLookup) -> Option<Vec<u16>> {
        let name = self.names.get(class as usize)?;
        if name.is_empty() {
            return None;
        }
        Some(
            strings
                .get(name)
                .map_or_else(|| name.encode_utf16().collect(), <[u16]>::to_vec),
        )
    }

    /// The label draw of the hovered unit `hover`: `None` unless it is an
    /// object with a name. The frame clip is the whole UI frame.
    pub fn draw(
        &self,
        world: &ClientWorld,
        cam: &Camera,
        hover: Option<UnitKey>,
        strings: &dyn StringLookup,
    ) -> Option<UiDraw> {
        let key = hover.filter(|k| k.unit_type == OBJECT)?;
        let u = world.units.get(&key)?;
        let text = self.text(u.class, strings)?;
        let cell = u.position?;
        let (x, y) = feet(cam, cell);
        Some(UiDraw::Text(TextRequest {
            text,
            at: Point::new(x, y - HIT_ABOVE),
            style: TextStyle {
                font: FONT_CHAT,
                color: 0,
            },
            opts: TextOpts::centered(),
            clip: FRAME,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{ClientUnit, KindData, PlayerData};

    struct Strings;
    impl StringLookup for Strings {
        fn get(&self, key: &str) -> Option<&[u16]> {
            const CHEST: &[u16] = &[0x43, 0x68, 0x65, 0x73, 0x74];
            (key == "chest").then_some(CHEST)
        }
    }

    const ME: UnitKey = UnitKey {
        unit_type: 0,
        guid: 1,
    };
    const CHEST: UnitKey = UnitKey {
        unit_type: OBJECT,
        guid: 7,
    };
    const MON: UnitKey = UnitKey {
        unit_type: 1,
        guid: 8,
    };

    fn world() -> ClientWorld {
        let mut w = ClientWorld::default();
        let mut p = ClientUnit::new(ME);
        p.position = Some((100, 100));
        p.kind = KindData::Player(PlayerData::default());
        w.units.insert(ME, p);
        w.local_player = Some(ME);
        for (k, class, at) in [(CHEST, 1, (103, 100)), (MON, 1, (104, 100))] {
            let mut u = ClientUnit::new(k);
            u.class = class;
            u.position = Some(at);
            w.units.insert(k, u);
        }
        w
    }

    fn labels() -> ObjectLabels {
        ObjectLabels::new(vec!["".into(), "chest".into()])
    }

    // Covers: specs/world/objects-client.md §25
    #[test]
    fn hovering_an_object_yields_its_name_over_its_feet() {
        let w = world();
        let cam = crate::world_view::corpse_click::camera_for(&w, 0).unwrap();
        let at = feet(&cam, (103, 100));
        let hover = crate::bridge::hover::pick(&w, &cam, at);
        assert_eq!(hover, Some(CHEST));
        let Some(UiDraw::Text(t)) = labels().draw(&w, &cam, hover, &Strings) else {
            panic!("no label");
        };
        assert_eq!(String::from_utf16(&t.text).unwrap(), "Chest");
        assert_eq!(t.at, Point::new(at.0, at.1 - HIT_ABOVE));
    }

    #[test]
    fn the_name_is_the_fallback_and_only_objects_are_labelled() {
        let w = world();
        let cam = crate::world_view::corpse_click::camera_for(&w, 0).unwrap();
        let l = ObjectLabels::new(vec!["".into(), "barrel".into()]);
        let Some(UiDraw::Text(t)) = l.draw(&w, &cam, Some(CHEST), &Strings) else {
            panic!("no label");
        };
        assert_eq!(String::from_utf16(&t.text).unwrap(), "barrel");
        // A monster, no hover and a blank row draw nothing.
        assert!(labels().draw(&w, &cam, Some(MON), &Strings).is_none());
        assert!(labels().draw(&w, &cam, None, &Strings).is_none());
        assert!(ObjectLabels::new(vec!["".into(), "".into()])
            .draw(&w, &cam, Some(CHEST), &Strings)
            .is_none());
    }
}
