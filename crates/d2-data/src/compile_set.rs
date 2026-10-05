// Spec: specs/data/field-types.md §6.4–§6.5 (load order, hand-built linkers); order from specs/data/loading.md §6–§7
//! Compiles every table of the 1.14d load sequence from text, in
//! `tables.tsv` execution order, with linkers kept across tables and the
//! hand-built `@range`, `@treasureclass`, `@uniques` and `@sets` linkers
//! built where their loaders build them.

use std::collections::BTreeMap;

use crate::calc::CalcDiag;
use crate::compile::{
    check_field_list, compile_table, range_linker, special_linker, tc_linker, Compiled, Linker,
    Linkers, StdCallbacks, RANGE_LINKER, SETS_LINKER, TC_LINKER, UNIQUES_LINKER,
};
use crate::schema::{schema, CalcBuffer};
use crate::strings::StringTables;
use crate::txt::{ErrorCode, TxtError, TxtTable};

/// One compiled table.
#[derive(Debug, Clone)]
pub struct CompiledTable {
    /// `tables.tsv` name.
    pub name: String,
    /// Archive the `.txt` came from.
    pub txt_source: String,
    pub compiled: Compiled,
}

/// The whole text compile.
#[derive(Debug, Clone)]
pub struct CompiledSet {
    /// Every called table, in execution order.
    pub tables: Vec<CompiledTable>,
    /// The four code buffers.
    pub buffers: BTreeMap<CalcBuffer, Vec<u8>>,
    /// Calls of unknown table-specific callbacks (`field-types.md` §8.3),
    /// by callback; empty for the 1.14d field lists.
    pub unspecified_callbacks: BTreeMap<String, usize>,
    pub calc_diagnostics: BTreeMap<(CalcBuffer, CalcDiag), usize>,
    /// Linkers after the last table.
    pub linkers: Linkers,
}

impl CompiledSet {
    pub fn table(&self, name: &str) -> Option<&CompiledTable> {
        self.tables.iter().find(|t| t.name == name)
    }
}

/// A source of `.txt` files: `(archive, bytes)` for an excel file name,
/// `None` when absent.
pub type TxtReader<'a> = dyn FnMut(&str) -> Result<Option<(String, Vec<u8>)>, String> + 'a;

/// A text compile failure.
#[derive(Debug, thiserror::Error)]
pub enum CompileSetError {
    #[error("{file}: not found in any archive")]
    Missing { file: String },
    #[error("{file}: {detail}")]
    Read { file: String, detail: String },
    #[error(transparent)]
    Txt(#[from] TxtError),
}

/// Compiles every called table (`tables.tsv` order) from its `.txt`.
pub fn compile_all(
    read: &mut TxtReader<'_>,
    strings: &StringTables,
) -> Result<CompiledSet, CompileSetError> {
    let mut linkers = Linkers::default();
    let mut callbacks = StdCallbacks::new(strings);
    let mut tables: Vec<CompiledTable> = Vec::new();
    for def in schema().called() {
        if def.name == "skills" {
            // Built inside step 10, before the skills compile (`loading.md` §10.3).
            linkers.insert(RANGE_LINKER, range_linker());
        }
        // E13 comes before the file is read (`txt-format.md` §9).
        check_field_list(&def.txt_name, &def.fields, def.record_size)?;
        let (source, bytes) = read(&def.txt_name)
            .map_err(|detail| CompileSetError::Read {
                file: def.txt_name.clone(),
                detail,
            })?
            .ok_or_else(|| CompileSetError::Missing {
                file: def.txt_name.clone(),
            })?;
        let file = format!("{source}:{}", def.txt_name);
        let txt = TxtTable::parse(&file, &bytes)?;
        let compiled = compile_table(
            &file,
            &txt,
            &def.fields,
            def.record_size,
            &mut linkers,
            &mut callbacks,
        )?;
        tables.push(CompiledTable {
            name: def.name.clone(),
            txt_source: source,
            compiled,
        });
        let special = match def.name.as_str() {
            "uniqueitems" => Some((UNIQUES_LINKER, 52)),
            "setitems" => Some((SETS_LINKER, 48)),
            _ => None,
        };
        if let Some((name, lvl_offset)) = special {
            // The loader's name registration (`callbacks.md` §7).
            let c = &tables.last().expect("just pushed").compiled;
            let (linker, items) =
                special_linker(c.records.chunks_exact(c.record_size), 40, lvl_offset).ok_or_else(
                    || TxtError::new(&file, ErrorCode::E11).with_detail("unique or set name"),
                )?;
            linkers.insert(name, Linker::Name(linker));
            if name == UNIQUES_LINKER {
                callbacks.special.uniques = items;
            } else {
                callbacks.special.sets = items;
            }
        }
        if def.name == "treasureclassex" {
            // The TC routine of step 46 (`loading.md` §10.6).
            let records = |n: &str| {
                let c = &tables
                    .iter()
                    .find(|t| t.name == n)
                    .expect("compiled earlier")
                    .compiled;
                c.records.chunks_exact(c.record_size).collect::<Vec<_>>()
            };
            let tc =
                tc_linker(records("itemtypes"), records("treasureclassex")).ok_or_else(|| {
                    TxtError::new(&file, ErrorCode::E11).with_detail("treasure class name")
                })?;
            linkers.insert(TC_LINKER, Linker::Name(tc));
        }
    }
    Ok(CompiledSet {
        tables,
        buffers: callbacks.buffers,
        unspecified_callbacks: callbacks.unspecified,
        calc_diagnostics: callbacks.calc_diagnostics,
        linkers,
    })
}
