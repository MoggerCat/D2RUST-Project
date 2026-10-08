// Spec: specs/formats/mpq.md, specs/formats/tbl.md, specs/formats/dc6.md, specs/formats/dcc.md, specs/formats/animdata.md, specs/formats/ds1.md, specs/formats/dt1.md, specs/data/schema.md, specs/data/loading.md (test support only)
//! Test-only synthetic game data. CI has no game files, so this crate
//! builds a small, coherent install from scratch at test time: string
//! tables, `AnimData.d2` and a made-up excel table set whose column
//! layouts come from the schema (`specs/data/fields.tsv`) only, packed
//! into MPQ archives by `d2_formats::mpq::writer`. No value comes from a
//! Blizzard file, and nothing here is committed as a binary blob (hard
//! rules 1 and 9).
//!
//! - [`tbl`], [`animdata`], [`ds1`], [`dt1`]: format writers.
//! - [`drlg`]: the DS1 / DT1 files of the synthetic levels (a one-room
//!   preset town).
//! - [`act1`]: an Act I-shaped variant of the set (the act placer's
//!   levels and the lvlprest ids the Act I generator stamps).
//! - [`synth`]: the `.txt` table model (schema headers, rows).
//! - [`content`]: the made-up rows, strings and animation records.
//! - [`sprites`]: live-shaped DC6 / DCC files for the format benches.
//! - [`install`]: archives on disk, the text compile, the `.bin` pack and
//!   the load through `d2-data`.
//! - [`host`]: a joined single-player host over a [`game::GameData`]
//!   (creation, join, frames, walk legs).
//! - [`game`]: a game from a loaded set: the table views, the DRLG
//!   providers and a `WorldSim` (what the server builds a game from).
//!
//! Never a dependency of a game crate's normal build: use it from
//! `[dev-dependencies]`.

pub mod act1;
pub mod act2;
pub mod act3;
pub mod act4;
pub mod act5;
pub mod animdata;
pub mod content;
pub mod drlg;
pub mod ds1;
pub mod dt1;
pub mod game;
pub mod host;
pub mod install;
pub mod sprites;
pub mod synth;
pub mod tbl;
