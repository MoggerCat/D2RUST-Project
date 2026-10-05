# Third-party notices

Record here any code adapted from permissively licensed projects, with its
original copyright and license text.

- D2MOO — MIT License — https://github.com/ThePhrozenKeep/D2MOO
- Riiablo — Apache License 2.0 — https://github.com/collinsmith/riiablo

## Adapted material

- `crates/d2-formats/src/mpq/tables.rs` (spec: `specs/formats/mpq-tables.md`):
  PKWARE DCL code tables, Storm Huffman weight tables and IMA ADPCM tables,
  taken as format constants from Riiablo (`com.riiablo.mpq_bytebuf`),
  Copyright the Riiablo authors (Collin Smith; see Riiablo's AUTHORS file),
  Apache License 2.0
  (https://www.apache.org/licenses/LICENSE-2.0). The decoders themselves
  were written from the spec, not translated from Riiablo's code.

GPL-licensed projects (OpenDiablo2, OpenD2) are reference reading only;
no code from them is included.

Diablo, Diablo II and Lord of Destruction are trademarks of Blizzard
Entertainment, Inc. This project is not affiliated with or endorsed by
Blizzard. Original game files are required and are not included.
