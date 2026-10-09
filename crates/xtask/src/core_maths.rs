//! `cargo xtask core-maths`: checks that the code the core runs never calls
//! the operating system's maths library ([ADR-0001]).
//!
//! The operating systems' maths libraries (`cos`, `acos`, `log2` and the
//! rest) give different last bits on different computers, so the core takes
//! every maths function from `libm` instead. Clippy keeps std's float methods
//! out of the core crates' own code, but not out of the libraries they use,
//! and a library can switch to std's maths without a line of it changing:
//! once std is anywhere in a crate's dependencies, Rust resolves `x.acos()` on
//! an `f64` to std's own method, ahead of a maths trait such as simba's.
//!
//! So this check looks at the machine code itself:
//!
//! 1. It works out every library the core crates (the `core` list in
//!    `walls.toml`) are built with, leaving out code-writing macros, which
//!    only run while building.
//! 2. It builds the core crates, plus any packages named with `--with`, in one
//!    `cargo build`, so the features every package turns on are merged as they
//!    are in the real build. CI runs it `--with opendrone --with
//!    opendrone-scenario`, so the game's and the Scenario runner's features
//!    count.
//! 3. It reads every one of those libraries' compiled code (each `.rlib`) and
//!    lists every call to a maths function the code leaves for the operating
//!    system to supply, with the functions that make it. A call inside one of
//!    std's maths methods, which the compiler copies into each crate that uses
//!    it, counts as made by the function that calls the method.
//! 4. A generic or `#[inline]` function is compiled into the crate that uses
//!    it, so a core library's function can sit in another crate's compiled
//!    code: the game's, say, when it calls a generic function of the physics.
//!    And a crate built without optimisation shares its generic copies: a
//!    crate built later reuses one instead of building its own, so the
//!    game's copy of a physics function may call glamx's copy built in a
//!    crate that uses only glamx. So the check also reads the compiled code
//!    of every other crate in the build that uses any library the core is
//!    built with, including the `--with` packages' programs (kept from the
//!    same build, so with the same features), and counts the calls made
//!    there by a core library's functions. Symbol names say whose function
//!    each one is ([`owner`]). A crate's own use of a core library's generic
//!    maths counts too, which fails safe.
//!
//! Any call not allowed in `walls.toml`'s `[core-platform-maths]`, with its
//! reason, fails the check with a plain sentence naming the library, the
//! maths function, the Rust operation behind it and where it is called from.
//! Two more things fail it:
//!
//! - A reference to a maths function from outside any function, such as a
//!   table of function pointers. No allowance covers one, because nothing
//!   says what calls through it. That includes a table that points at a
//!   copy of std's maths method (`[f64::acos]`), however the code is built.
//! - Compiled code that is LLVM bitcode instead of machine code, which
//!   link-time optimisation makes. This check can't read it.
//!
//! What it can't see: a core function that the compiler writes straight into
//! another crate's function (inlines) has no name of its own there, so a call
//! it makes counts as that crate's. The game and the Scenario runner are
//! built without optimisation in the development builds this check makes,
//! and those inline only functions marked `#[inline(always)]`. Nor does it
//! follow a core function compiled into another crate when only a table of
//! function pointers, such as a trait object's, leads to it.
//!
//! [ADR-0001]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0001-bit-exact-determinism-with-ordinary-floats.md

mod build;
mod code;
mod owner;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::walls::{RULES_FILE, Rules};
pub(crate) use build::compile_keeping_programs;
use build::{Packages, build, cargo_metadata, host_triple};
use code::{Code, Depth, Place};

/// The C maths library's functions whose results can differ from one
/// operating system's library to another, as compiled code names them: in
/// double precision, in single precision, and the Rust operation behind them
/// (`{}` stands for `f64` or `f32`).
///
/// Operations whose every bit IEEE 754 fixes aren't listed, because every
/// library gives the same bits for them: `sqrt`, `fabs`, `floor`, `ceil`,
/// `trunc`, `round`, `copysign`, `fmod` (Rust's `%`), `fma` (Rust's
/// `mul_add`) and `remainder`. The core crates' `clippy.toml` allows `%` and
/// `mul_add` too. `fdim` is exact as well, but it is listed because
/// `clippy.toml` bans the Rust operation behind it, the old `abs_sub`. Nor is
/// `__powidf2`, which `powi` calls: it is Rust's own code, the same on every
/// operating system.
const PLATFORM_MATHS: &[(&str, &str, &str)] = &[
    ("acos", "acosf", "Rust's `{}::acos`"),
    ("acosh", "acoshf", "Rust's `{}::acosh`"),
    ("asin", "asinf", "Rust's `{}::asin`"),
    ("asinh", "asinhf", "Rust's `{}::asinh`"),
    ("atan", "atanf", "Rust's `{}::atan`"),
    ("atan2", "atan2f", "Rust's `{}::atan2`"),
    ("atanh", "atanhf", "Rust's `{}::atanh`"),
    ("cbrt", "cbrtf", "Rust's `{}::cbrt`"),
    ("cos", "cosf", "Rust's `{}::cos`"),
    ("cosh", "coshf", "Rust's `{}::cosh`"),
    ("erf", "erff", "Rust's `{}::erf`"),
    ("erfc", "erfcf", "Rust's `{}::erfc`"),
    ("exp", "expf", "Rust's `{}::exp`"),
    ("exp2", "exp2f", "Rust's `{}::exp2`"),
    ("exp10", "exp10f", "Rust's `{}::powf` with a base of 10"),
    ("expm1", "expm1f", "Rust's `{}::exp_m1`"),
    ("fdim", "fdimf", "Rust's `{}::abs_sub`"),
    ("hypot", "hypotf", "Rust's `{}::hypot`"),
    ("lgamma", "lgammaf", "Rust's `{}::ln_gamma`"),
    ("lgamma_r", "lgammaf_r", "Rust's `{}::ln_gamma`"),
    ("log", "logf", "Rust's `{}::ln` or `{}::log`"),
    ("log10", "log10f", "Rust's `{}::log10`"),
    ("log1p", "log1pf", "Rust's `{}::ln_1p`"),
    ("log2", "log2f", "Rust's `{}::log2`"),
    ("pow", "powf", "Rust's `{}::powf`"),
    ("sin", "sinf", "Rust's `{}::sin`"),
    (
        "sincos",
        "sincosf",
        "Rust's `{}::sin_cos`, or `{}::sin` and `{}::cos` of one number",
    ),
    ("sinh", "sinhf", "Rust's `{}::sinh`"),
    ("tan", "tanf", "Rust's `{}::tan`"),
    ("tanh", "tanhf", "Rust's `{}::tanh`"),
    ("tgamma", "tgammaf", "Rust's `{}::gamma`"),
    // Names only Apple's maths library has.
    (
        "__exp10",
        "__exp10f",
        "Rust's `{}::powf` with a base of 10, as the compiler writes it for Apple's systems",
    ),
    (
        "__sincos_stret",
        "__sincosf_stret",
        "Rust's `{}::sin_cos`, or `{}::sin` and `{}::cos` of one number, as the compiler \
         writes it for Apple's systems",
    ),
    (
        "__sinpi",
        "__sinpif",
        "Apple's sin(πx), which no Rust operation calls",
    ),
    (
        "__cospi",
        "__cospif",
        "Apple's cos(πx), which no Rust operation calls",
    ),
    (
        "__tanpi",
        "__tanpif",
        "Apple's tan(πx), which no Rust operation calls",
    ),
    (
        "__sincospi_stret",
        "__sincospif_stret",
        "Apple's sin(πx) and cos(πx) together, which no Rust operation calls",
    ),
];

/// Whether compiled code calling `name` calls one of the operating system's
/// maths functions.
fn is_platform_maths(name: &str) -> bool {
    PLATFORM_MATHS
        .iter()
        .any(|(double, single, _)| name == *double || name == *single)
}

/// The Rust operation that calls the operating system's maths function
/// `name`, in words.
fn rust_operation(name: &str) -> String {
    PLATFORM_MATHS
        .iter()
        .find_map(|(double, single, operation)| {
            if name == *double {
                Some(operation.replace("{}", "f64"))
            } else if name == *single {
                Some(operation.replace("{}", "f32"))
            } else {
                None
            }
        })
        .unwrap_or_default()
}

const USAGE: &str = "Usage: cargo xtask core-maths [--with <package>]... [--target <triple>] \
                     [--manifest-path <Cargo.toml>]";

pub fn run(args: &[String]) -> ExitCode {
    let mut with = Vec::new();
    let mut manifest_path = None;
    let mut target = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match (arg.as_str(), rest.next()) {
            ("--with", Some(package)) => with.push(package.clone()),
            ("--manifest-path", Some(path)) => manifest_path = Some(path.clone()),
            ("--target", Some(triple)) => target = Some(triple.clone()),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let checked = Rules::load().and_then(|rules| {
        let target = match target.clone() {
            Some(target) => target,
            None => host_triple()?,
        };
        let found = check(&rules, &with, manifest_path.as_deref(), &target)?;
        Ok((rules, found))
    });
    let (rules, found) = match checked {
        Ok(checked) => checked,
        Err(error) => {
            eprintln!("Could not check the core's maths: {error}");
            return ExitCode::from(2);
        }
    };
    let (allowed, refused): (Vec<&Call>, Vec<&Call>) = found
        .calls
        .iter()
        .partition(|call| rules.platform_maths_reason(call).is_some());
    if refused.is_empty() && found.bitcode.is_empty() {
        let others = match found.others_read {
            0 => String::new(),
            1 => format!(
                ", and 1 other crate's compiled code read for the core's functions (found in {})",
                found.others_holding
            ),
            read => format!(
                ", and {read} other crates' compiled code read for the core's functions (found in \
                 {})",
                found.others_holding
            ),
        };
        let with = if with.is_empty() {
            String::new()
        } else {
            format!(", built together with {}", with.join(", "))
        };
        println!(
            "The core calls none of the operating system's maths: {} libraries checked{others}{with}.",
            found.libraries
        );
        for call in allowed {
            println!(
                "- Allowed by {RULES_FILE}: `{}` calls `{}` (from {}): {}.",
                call.library,
                call.function,
                call.callers(),
                rules.platform_maths_reason(call).unwrap_or_default()
            );
        }
        return ExitCode::SUCCESS;
    }
    if !found.bitcode.is_empty() {
        println!(
            "This check can't read all of the core's compiled code, so it can't tell whether the \
             core calls the operating system's maths (ADR-0001):"
        );
        for crate_name in &found.bitcode {
            println!(
                "- `{crate_name}` was compiled to LLVM bitcode, not machine code. Link-time \
                 optimisation does that (`lto` in a Cargo profile, or `-C linker-plugin-lto` in \
                 RUSTFLAGS): run this check without it."
            );
        }
    }
    if !refused.is_empty() {
        println!(
            "The core calls the operating system's maths, whose results differ from one operating \
             system to another; the core must take every maths function from libm (ADR-0001):"
        );
    }
    for call in refused {
        let advice = if call.callers.iter().any(Caller::is_outside) {
            "No allowance can cover a reference from outside any function, such as a table of \
             function pointers, because nothing says what calls through it: stop it being made."
                .to_owned()
        } else {
            format!(
                "If it can never run in the Simulation, allow it under [core-platform-maths] in \
                 {RULES_FILE} with the reason; otherwise stop it being called."
            )
        };
        println!(
            "- `{}` calls the operating system's `{}` ({}) from {}. {advice}",
            call.library,
            call.function,
            rust_operation(&call.function),
            call.callers(),
        );
    }
    ExitCode::FAILURE
}

/// One library's calls to one of the operating system's maths functions.
#[derive(Debug)]
pub(crate) struct Call {
    pub library: String,
    pub function: String,
    pub callers: BTreeSet<Caller>,
}

/// Where a call to the operating system's maths is made from.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Caller {
    /// A reference from outside any function, such as a table of function
    /// pointers: the data's name when it has one, and the section of compiled
    /// code it sits in.
    Outside {
        data: Option<String>,
        section: String,
    },
    /// A function, demangled, with the crate or program whose compiled code
    /// holds it when that isn't the library's own (a generic or `#[inline]`
    /// function is compiled into the crate that uses it), in words.
    Function {
        name: String,
        compiled_into: Option<String>,
    },
}

impl Caller {
    /// The function's name, if the call is made from a function.
    pub(crate) fn function_name(&self) -> Option<&str> {
        match self {
            Caller::Function { name, .. } => Some(name),
            Caller::Outside { .. } => None,
        }
    }

    fn is_outside(&self) -> bool {
        matches!(self, Caller::Outside { .. })
    }
}

impl fmt::Display for Caller {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Caller::Function {
                name,
                compiled_into: None,
            } => write!(out, "{name}"),
            Caller::Function {
                name,
                compiled_into: Some(place),
            } => write!(out, "{name} (compiled into {place})"),
            Caller::Outside {
                data: Some(data),
                section,
            } => write!(
                out,
                "`{data}`, data outside any function (in section `{section}`)"
            ),
            Caller::Outside {
                data: None,
                section,
            } => write!(out, "outside any function (in section `{section}`)"),
        }
    }
}

impl Call {
    fn callers(&self) -> String {
        const SHOWN: usize = 3;
        let mut names: Vec<String> = self
            .callers
            .iter()
            .take(SHOWN)
            .map(Caller::to_string)
            .collect();
        if names.is_empty() {
            names.push("code this check couldn't name".to_owned());
        }
        let more = self.callers.len().saturating_sub(SHOWN);
        let mut text = names.join(", ");
        if more > 0 {
            text.push_str(&format!(" and {more} more"));
        }
        text
    }
}

/// What the check found.
struct Found {
    /// How many libraries the core is built with.
    libraries: usize,
    /// How many other crates' (and programs') compiled code was read for the
    /// core's functions.
    others_read: usize,
    /// How many of those hold any of the core's functions.
    others_holding: usize,
    /// Every call to the operating system's maths, in order of library and
    /// function.
    calls: Vec<Call>,
    /// The crates whose compiled code is LLVM bitcode, which this check
    /// can't read.
    bitcode: Vec<String>,
}

/// Builds the core with `with` and reads the compiled code.
fn check(
    rules: &Rules,
    with: &[String],
    manifest_path: Option<&str>,
    target: &str,
) -> Result<Found, String> {
    let packages = Packages::read(&cargo_metadata(manifest_path)?)?;
    let core = packages.core_closure(&rules.core);
    let built = build(&packages, &rules.core, with, manifest_path, target)?;
    let mut calls: BTreeMap<(String, String), BTreeSet<Caller>> = BTreeMap::new();
    let mut bitcode = Vec::new();

    // The core's own libraries first: their code also says which crates are
    // the core's, for finding the core's functions in other crates' code.
    let mut owners = Owners::default();
    let mut core_code = Vec::new();
    for (id, name) in &core {
        let code = read_code(
            built.rlibs.get(id).map(Vec::as_slice).unwrap_or_default(),
            Depth::Calls,
        )?;
        if code.bitcode {
            bitcode.push(name.clone());
        }
        owners.learn(packages.crate_name(id), name, &code);
        core_code.push((name, code));
    }
    for (name, code) in &core_code {
        core_calls(name, code, &owners, &mut calls);
    }

    // Then every other crate in the build that uses any library the core is
    // built with, and the `--with` packages' programs: the core's generic and
    // inline functions are compiled into them, and a crate built without
    // optimisation may hold a generic copy that the game's copy of a core
    // function calls.
    let mut others: Vec<(String, String, &[PathBuf])> = Vec::new();
    for (id, rlibs) in &built.rlibs {
        if !core.contains_key(id) && packages.uses_any(id, &core) {
            let name = packages.name(id);
            others.push((name.to_owned(), format!("`{name}`"), rlibs));
        }
    }
    for (id, program, object) in &built.programs {
        others.push((
            packages.name(id).to_owned(),
            format!("the program `{program}`"),
            std::slice::from_ref(object),
        ));
    }
    let mut others_read = 0;
    let mut others_holding = 0;
    if bitcode.is_empty() {
        for (name, place, files) in others {
            // Its names first, quickly: most crates name no maths function.
            let code = read_code(files, Depth::Names)?;
            others_read += 1;
            if code
                .functions
                .iter()
                .any(|function| owners.library(function).is_some())
            {
                others_holding += 1;
            }
            if code.bitcode {
                bitcode.push(name);
            } else if !code.maths.is_empty() {
                other_calls(
                    &place,
                    &read_code(files, Depth::Calls)?,
                    &owners,
                    &mut calls,
                );
            }
        }
    }

    Ok(Found {
        libraries: core.len(),
        others_read,
        others_holding,
        calls: calls
            .into_iter()
            .map(|((library, function), callers)| Call {
                library,
                function,
                callers,
            })
            .collect(),
        bitcode,
    })
}

/// Reads compiled code: `.rlib` archives, or a program's object file.
fn read_code(files: &[PathBuf], depth: Depth) -> Result<Code, String> {
    let mut code = Code::default();
    for file in files {
        let bytes = std::fs::read(file)
            .map_err(|error| format!("can't read {}: {error}", file.display()))?;
        if file.extension().is_some_and(|ext| ext == "rlib") {
            code.read_rlib(&bytes, depth)
        } else {
            code.read_object_file(&bytes, depth)
        }
        .map_err(|error| format!("can't read {}: {error}", file.display()))?;
    }
    Ok(code)
}

/// Adds the calls in one core library's compiled code. A call inside a
/// function of another crate's (such as std's `f64::acos`, copied in) counts
/// as made by the core's functions that lead to it, or by that function
/// itself when none does. Each core function's call counts as its own
/// library's, wherever the compiler put its code: an optimised library's
/// small functions are compiled into the crates that call them, too.
fn core_calls(
    library: &str,
    code: &Code,
    owners: &Owners,
    calls: &mut BTreeMap<(String, String), BTreeSet<Caller>>,
) {
    for (function, places) in &code.maths {
        // Even with no reference found, a maths function the code names
        // counts, from code this check couldn't name.
        if places.is_empty() {
            calls
                .entry((library.to_owned(), function.clone()))
                .or_default();
        }
        for place in places {
            match place {
                Place::Function(name) => {
                    let Leading {
                        core,
                        uncalled,
                        outside,
                    } = owners.functions_leading_to(code, name);
                    let mut theirs: BTreeSet<&str> = uncalled;
                    if core.is_empty() && theirs.is_empty() {
                        theirs.insert(name);
                    }
                    if !outside.is_empty() {
                        calls
                            .entry((library.to_owned(), function.clone()))
                            .or_default()
                            .extend(outside);
                    }
                    for name in core {
                        let owner = owners.library(name).unwrap_or(library);
                        calls
                            .entry((owner.to_owned(), function.clone()))
                            .or_default()
                            .insert(Caller::Function {
                                name: demangle(name),
                                compiled_into: (owner != library).then(|| format!("`{library}`")),
                            });
                    }
                    for name in theirs {
                        calls
                            .entry((library.to_owned(), function.clone()))
                            .or_default()
                            .insert(Caller::Function {
                                name: demangle(name),
                                compiled_into: None,
                            });
                    }
                }
                Place::Outside { data, section } => {
                    calls
                        .entry((library.to_owned(), function.clone()))
                        .or_default()
                        .insert(Caller::Outside {
                            data: data.as_deref().map(demangle),
                            section: section.clone(),
                        });
                }
            }
        }
    }
}

/// Adds the calls made by core libraries' functions compiled into another
/// crate's code. The crate's own calls are its business.
fn other_calls(
    compiled_into: &str,
    code: &Code,
    owners: &Owners,
    calls: &mut BTreeMap<(String, String), BTreeSet<Caller>>,
) {
    for (function, places) in &code.maths {
        for place in places {
            let Place::Function(name) = place else {
                continue;
            };
            let core = owners.functions_leading_to(code, name).core;
            for name in core {
                let Some(library) = owners.library(name) else {
                    continue;
                };
                calls
                    .entry((library.to_owned(), function.clone()))
                    .or_default()
                    .insert(Caller::Function {
                        name: demangle(name),
                        compiled_into: Some(compiled_into.to_owned()),
                    });
            }
        }
    }
}

fn demangle(symbol: &str) -> String {
    format!("{:#}", rustc_demangle::demangle(symbol))
}

/// Which core library each function in compiled code comes from, by the
/// crate its symbol name gives ([`owner`]).
#[derive(Default)]
struct Owners {
    /// Core libraries by their crate's name and number, as their own compiled
    /// code names them.
    numbered: BTreeMap<(String, u64), String>,
    /// Core libraries whose compiled code gave no number (it may hold no
    /// function of its own), by their crate's name.
    by_name: BTreeMap<String, String>,
}

impl Owners {
    /// Learns how one core library's crate is named from its compiled code.
    fn learn(&mut self, crate_name: &str, library: &str, code: &Code) {
        let mut learned = false;
        for function in &code.functions {
            if let Some(owner::Owner {
                name,
                disambiguator: Some(number),
            }) = owner::owner(function)
                && name == crate_name
            {
                self.numbered.insert((name, number), library.to_owned());
                learned = true;
            }
        }
        if !learned {
            self.by_name
                .insert(crate_name.to_owned(), library.to_owned());
        }
    }

    /// The core library a function comes from, if it's one of the core's.
    fn library(&self, symbol: &str) -> Option<&str> {
        let owner = owner::owner(symbol)?;
        if let Some(number) = owner.disambiguator
            && let Some(library) = self.numbered.get(&(owner.name.clone(), number))
        {
            return Some(library);
        }
        if let Some(library) = self.by_name.get(&owner.name) {
            return Some(library);
        }
        // An older-style name carries no number: any core crate of its name.
        if owner.disambiguator.is_none() {
            return self
                .numbered
                .iter()
                .find(|((name, _), _)| *name == owner.name)
                .map(|(_, library)| library.as_str());
        }
        None
    }

    /// The core's functions that lead to a call made inside `function`:
    /// `function` itself when it's one of them, or else the core's functions
    /// that call it, through other crates' functions such as std's. Then the
    /// other crates' functions on those ways that nothing in this code calls.
    fn functions_leading_to<'a>(&self, code: &'a Code, function: &'a str) -> Leading<'a> {
        let mut core = BTreeSet::new();
        let mut uncalled = BTreeSet::new();
        let mut outside = BTreeSet::new();
        let mut seen = BTreeSet::new();
        let mut waiting = vec![function];
        while let Some(name) = waiting.pop() {
            if !seen.insert(name) {
                continue;
            }
            if self.library(name).is_some() {
                core.insert(name);
                continue;
            }
            // Data pointing at another crate's function, such as a table of
            // function pointers, can call it from nowhere we can name.
            for (data, section) in code.data_refs.get(name).into_iter().flatten() {
                outside.insert(Caller::Outside {
                    data: data.as_deref().map(demangle),
                    section: section.clone(),
                });
            }
            match code.callers.get(name) {
                Some(callers) if !callers.is_empty() => {
                    waiting.extend(callers.iter().map(String::as_str));
                }
                _ => {
                    uncalled.insert(name);
                }
            }
        }
        Leading {
            core,
            uncalled,
            outside,
        }
    }
}

/// What [`Owners::functions_leading_to`] found.
struct Leading<'a> {
    /// The core's functions that lead to the call.
    core: BTreeSet<&'a str>,
    /// Other crates' functions on the way that nothing in this code calls.
    uncalled: BTreeSet<&'a str>,
    /// Data outside any function that points at another crate's function on
    /// the way.
    outside: BTreeSet<Caller>,
}
