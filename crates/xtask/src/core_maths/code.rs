//! Reading compiled code: every reference to one of the operating system's
//! maths functions, where it is, and which function calls which.

use std::collections::{BTreeMap, BTreeSet};

use object::read::archive::ArchiveFile;
use object::{Object, ObjectSection, ObjectSymbol, RelocationTarget, SectionKind, SymbolKind};

/// Where compiled code refers to one of the operating system's maths
/// functions.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Place {
    /// Outside any function: in data, such as a table of function pointers,
    /// with the data's symbol name when it has one, in a section of the
    /// compiled code.
    Outside {
        data: Option<String>,
        section: String,
    },
    /// Inside a function, by its symbol name.
    Function(String),
}

/// What one crate's compiled code refers to.
#[derive(Default)]
pub(crate) struct Code {
    /// Every reference to one of the operating system's maths functions, by
    /// that function's name.
    pub maths: BTreeMap<String, BTreeSet<Place>>,
    /// The functions that call (or otherwise refer to) each symbol, by name.
    pub callers: BTreeMap<String, BTreeSet<String>>,
    /// Where data (not a function), such as a table of function pointers,
    /// points at each function, by the function's symbol name: the data's
    /// symbol name when it has one, and its section.
    pub data_refs: BTreeMap<String, BTreeSet<(Option<String>, String)>>,
    /// Every function the code holds, by symbol name.
    pub functions: BTreeSet<String>,
    /// Whether some of it is LLVM bitcode instead of machine code. This check
    /// can't read bitcode: link-time optimisation makes it.
    pub bitcode: bool,
}

/// A symbol a section holds: its address, name, and whether it's a function
/// (or else data).
type Defined = (u64, String, bool);

/// How much of the compiled code to read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Depth {
    /// Its functions, and which maths functions it names: enough to know
    /// whether a crate's code needs reading in full, quickly.
    Names,
    /// Everything, calls included.
    Calls,
}

impl Code {
    /// Adds one `.rlib`, an archive of object files.
    pub fn read_rlib(&mut self, bytes: &[u8], depth: Depth) -> Result<(), String> {
        let archive = ArchiveFile::parse(bytes).map_err(|error| error.to_string())?;
        for member in archive.members() {
            let member = member.map_err(|error| error.to_string())?;
            let data = member.data(bytes).map_err(|error| error.to_string())?;
            match object::File::parse(data) {
                Ok(file) => self.read_object(&file, depth),
                // The archive also holds Rust's own metadata, which isn't
                // code. LLVM bitcode is code, which this check can't read.
                Err(_) => self.bitcode |= is_bitcode(data),
            }
        }
        Ok(())
    }

    /// Adds one object file.
    pub fn read_object_file(&mut self, bytes: &[u8], depth: Depth) -> Result<(), String> {
        if is_bitcode(bytes) {
            self.bitcode = true;
            return Ok(());
        }
        let file = object::File::parse(bytes).map_err(|error| error.to_string())?;
        self.read_object(&file, depth);
        Ok(())
    }

    fn read_object(&mut self, file: &object::File<'_>, depth: Depth) {
        let mut maths: BTreeMap<usize, String> = BTreeMap::new();
        let mut defined: BTreeMap<usize, Vec<Defined>> = BTreeMap::new();
        for symbol in file.symbols() {
            let Ok(name) = symbol.name() else {
                continue;
            };
            let name = plain_name(file, name);
            if symbol.is_undefined() {
                if super::is_platform_maths(name) {
                    maths.insert(symbol.index().0, name.to_owned());
                }
                continue;
            }
            let is_function = match symbol.kind() {
                SymbolKind::Text => true,
                SymbolKind::Data => false,
                _ => continue,
            };
            let Some(section) = symbol.section_index() else {
                continue;
            };
            if is_function {
                self.functions.insert(name.to_owned());
            }
            defined.entry(section.0).or_default().push((
                symbol.address(),
                name.to_owned(),
                is_function,
            ));
        }
        for list in defined.values_mut() {
            list.sort();
        }
        // A maths function the code names but never refers to still counts,
        // from code this check couldn't name.
        for name in maths.values() {
            self.maths.entry(name.clone()).or_default();
        }
        if depth == Depth::Names {
            return;
        }
        for section in file.sections() {
            // Debug information describes the code; it never runs.
            if matches!(
                section.kind(),
                SectionKind::Debug | SectionKind::DebugString
            ) {
                continue;
            }
            let symbols = defined
                .get(&section.index().0)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for (offset, relocation) in section.relocations() {
                let at = section.address() + offset;
                // The symbols holding the reference: the last start at or
                // before it. Identical functions may be merged into one,
                // which then answers to each of their names.
                let holding: Vec<&Defined> =
                    match symbols.iter().rev().find(|(start, ..)| *start <= at) {
                        Some((start, ..)) => symbols
                            .iter()
                            .filter(|(address, ..)| address == start)
                            .collect(),
                        None => Vec::new(),
                    };
                let functions: Vec<&String> = holding
                    .iter()
                    .filter(|(.., is_function)| *is_function)
                    .map(|(_, name, _)| name)
                    .collect();
                let target = relocation.target();
                if let RelocationTarget::Symbol(index) = target
                    && let Some(name) = maths.get(&index.0)
                {
                    let places = self.maths.entry(name.clone()).or_default();
                    if functions.is_empty() {
                        places.insert(Place::Outside {
                            data: holding.first().map(|(_, name, _)| name.clone()),
                            section: section_name(&section),
                        });
                    }
                    for function in &functions {
                        places.insert(Place::Function((*function).clone()));
                    }
                    continue;
                }
                if functions.is_empty() {
                    if is_unwind_information(&section) {
                        continue;
                    }
                    let data = holding.first().map(|(_, name, _)| name.clone());
                    for callee in targets(file, target, &defined) {
                        self.data_refs
                            .entry(callee)
                            .or_default()
                            .insert((data.clone(), section_name(&section)));
                    }
                    continue;
                }
                for callee in targets(file, target, &defined) {
                    self.callers
                        .entry(callee)
                        .or_default()
                        .extend(functions.iter().map(|name| (*name).clone()));
                }
            }
        }
    }
}

/// Whether a section holds the information that unwinds the stack when a
/// function panics. It points at every function, but calls nothing.
fn is_unwind_information(section: &object::Section<'_, '_>) -> bool {
    let name = section.name().unwrap_or_default();
    ["unwind", "eh_frame", "pdata", "xdata", "gcc_except_table"]
        .iter()
        .any(|part| name.contains(part))
}

/// The symbols a reference points at, by name. ELF points at a function only
/// one file uses through its section, so a section counts as its function
/// when that's all it holds.
fn targets(
    file: &object::File<'_>,
    target: RelocationTarget,
    defined: &BTreeMap<usize, Vec<Defined>>,
) -> Vec<String> {
    let section = match target {
        RelocationTarget::Symbol(index) => {
            let Ok(symbol) = file.symbol_by_index(index) else {
                return Vec::new();
            };
            if symbol.kind() != SymbolKind::Section {
                return match symbol.name() {
                    Ok(name) if !name.is_empty() => vec![plain_name(file, name).to_owned()],
                    _ => Vec::new(),
                };
            }
            match symbol.section_index() {
                Some(section) => section,
                None => return Vec::new(),
            }
        }
        RelocationTarget::Section(section) => section,
        _ => return Vec::new(),
    };
    let functions: Vec<&Defined> = defined
        .get(&section.0)
        .into_iter()
        .flatten()
        .filter(|(.., is_function)| *is_function)
        .collect();
    match functions.first() {
        Some((start, ..)) if functions.iter().all(|(address, ..)| address == start) => {
            functions.iter().map(|(_, name, _)| name.clone()).collect()
        }
        _ => Vec::new(),
    }
}

/// A section's name, with its segment where the file has one
/// (`__DATA,__const`).
fn section_name(section: &object::Section<'_, '_>) -> String {
    let name = section.name().unwrap_or("an unnamed section");
    match section.segment_name() {
        Ok(Some(segment)) => format!("{segment},{name}"),
        _ => name.to_owned(),
    }
}

/// A symbol's plain name: Mach-O puts an underscore before every C name, and
/// Windows reaches a library's function through `__imp_` and its name.
fn plain_name<'a>(file: &object::File<'_>, name: &'a str) -> &'a str {
    match file.format() {
        object::BinaryFormat::MachO => name.strip_prefix('_').unwrap_or(name),
        object::BinaryFormat::Coff => name.strip_prefix("__imp_").unwrap_or(name),
        _ => name,
    }
}

/// LLVM bitcode, bare or in its wrapper.
fn is_bitcode(data: &[u8]) -> bool {
    data.starts_with(b"BC\xC0\xDE") || data.starts_with(&[0xDE, 0xC0, 0x17, 0x0B])
}
