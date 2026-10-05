// Spec: specs/formats/cof.md
//! COF: which layers make up a composite animation, frame events, and the
//! per-frame layer draw order.

use crate::cursor::{invalid, Cursor, FormatError};

const FORMAT: &str = "cof";
const HEADER_LEN: usize = 28;
const LAYER_LEN: usize = 9;
pub const COMPONENTS: usize = 16;

/// Component names, indexed by component ID.
pub const COMPONENT_NAMES: [&str; COMPONENTS] = [
    "HD", "TR", "LG", "RA", "LA", "RH", "LH", "SH", "S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CofLayer {
    pub component: u8,
    pub shadow: u8,
    pub selectable: u8,
    pub override_translucency: u8,
    pub new_translucency: u8,
    /// Weapon class, e.g. `*b"hth\0"`.
    pub weapon_class: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cof {
    pub layers_count: u8,
    pub frames: u8,
    pub directions: u8,
    pub version: u8,
    pub unknown: [u8; 4],
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
    pub animation_rate: u32,
    pub layers: Vec<CofLayer>,
    /// One event per frame: 0 none, 1 attack, 2 missile, 3 sound, 4 skill.
    pub events: Vec<u8>,
    /// Extra event bytes beyond one per frame.
    pub event_padding: Vec<u8>,
    /// `[(d * frames + f) * layers + slot]` = component ID, back to front.
    pub draw_order: Vec<u8>,
}

impl Cof {
    pub fn parse(data: &[u8]) -> Result<Cof, FormatError> {
        let mut c = Cursor::new(data, FORMAT);
        let layers_count = c.u8()?;
        let frames = c.u8()?;
        let directions = c.u8()?;
        let version = c.u8()?;
        let mut unknown = [0u8; 4];
        unknown.copy_from_slice(c.bytes(4)?);
        let x_min = c.i32()?;
        let x_max = c.i32()?;
        let y_min = c.i32()?;
        let y_max = c.i32()?;
        let animation_rate = c.u32()?;

        let (l, f, d) = (
            usize::from(layers_count),
            usize::from(frames),
            usize::from(directions),
        );
        let mut layers = Vec::with_capacity(l);
        for i in 0..l {
            let component = c.u8()?;
            if usize::from(component) >= COMPONENTS {
                return Err(invalid(FORMAT, format!("layer {i}: component {component}")));
            }
            let shadow = c.u8()?;
            let selectable = c.u8()?;
            let override_translucency = c.u8()?;
            let new_translucency = c.u8()?;
            let mut weapon_class = [0u8; 4];
            weapon_class.copy_from_slice(c.bytes(4)?);
            layers.push(CofLayer {
                component,
                shadow,
                selectable,
                override_translucency,
                new_translucency,
                weapon_class,
            });
        }

        let order_len = d * f * l;
        let fixed = HEADER_LEN + LAYER_LEN * l + order_len;
        let k = data
            .len()
            .checked_sub(fixed)
            .filter(|&k| k >= f)
            .ok_or_else(|| {
                invalid(
                    FORMAT,
                    format!("{} bytes is too short for {f} frame events", data.len()),
                )
            })?;
        let event_bytes = c.bytes(k)?;
        let events = event_bytes[..f].to_vec();
        let event_padding = event_bytes[f..].to_vec();
        let draw_order = c.bytes(order_len)?.to_vec();
        if let Some(&bad) = draw_order.iter().find(|&&b| usize::from(b) >= COMPONENTS) {
            return Err(invalid(FORMAT, format!("draw order component {bad}")));
        }
        Ok(Cof {
            layers_count,
            frames,
            directions,
            version,
            unknown,
            x_min,
            x_max,
            y_min,
            y_max,
            animation_rate,
            layers,
            events,
            event_padding,
            draw_order,
        })
    }

    /// Component drawn in `slot` (back to front) for direction `d`, frame `f`.
    pub fn component_at(&self, d: usize, f: usize, slot: usize) -> Option<u8> {
        let (l, fr) = (usize::from(self.layers_count), usize::from(self.frames));
        if slot >= l || f >= fr {
            return None;
        }
        self.draw_order.get((d * fr + f) * l + slot).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(l: u8, f: u8, d: u8, components: &[u8], events: &[u8], order: &[u8]) -> Vec<u8> {
        let mut v = vec![l, f, d, 20, 0, 0, 0, 0];
        for x in [-10i32, 10, -20, 0] {
            v.extend_from_slice(&x.to_le_bytes());
        }
        v.extend_from_slice(&256u32.to_le_bytes());
        for &comp in components {
            v.extend_from_slice(&[comp, 1, 1, 0, 0]);
            v.extend_from_slice(b"hth\0");
        }
        v.extend_from_slice(events);
        v.extend_from_slice(order);
        v
    }

    #[test]
    fn parses() {
        let cof = Cof::parse(&file(1, 2, 1, &[1], &[1, 0], &[1, 1])).unwrap();
        assert_eq!(cof.events, [1, 0]);
        assert!(cof.event_padding.is_empty());
        assert_eq!(cof.layers[0].weapon_class, *b"hth\0");
        assert_eq!(cof.component_at(0, 1, 0), Some(1));
        assert_eq!(cof.component_at(0, 2, 0), None);
        assert_eq!((cof.x_min, cof.y_min), (-10, -20));
    }

    #[test]
    fn padded_events() {
        let data = file(1, 1, 1, &[0], &[3, 0, 0, 0], &[0]);
        assert_eq!(data.len(), 42);
        let cof = Cof::parse(&data).unwrap();
        assert_eq!(cof.events, [3]);
        assert_eq!(cof.event_padding, [0, 0, 0]);
    }

    #[test]
    fn errors() {
        assert!(
            Cof::parse(&file(1, 1, 1, &[16], &[0], &[0])).is_err(),
            "component"
        );
        assert!(
            Cof::parse(&file(1, 1, 1, &[0], &[0], &[17])).is_err(),
            "order"
        );
        assert!(
            Cof::parse(&file(1, 2, 1, &[0], &[0], &[0, 0])).is_err(),
            "too short"
        );
    }
}
