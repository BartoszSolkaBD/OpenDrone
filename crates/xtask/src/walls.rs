//! `cargo xtask walls`: checks the walls between OpenDrone's crates
//! ([ADR-0003]).
//!
//! It reads the workspace's dependencies from `cargo metadata` and the rules
//! from `walls.toml`, and prints one plain sentence for each broken wall:
//!
//! - every crate in the workspace is listed in `walls.toml`, and uses only the
//!   OpenDrone crates listed for it, so physics and the Flight Controller never
//!   use each other;
//! - no core crate reaches, even through other libraries, Bevy or another
//!   library on the never-in-core list, or any outside library missing from
//!   the core-libraries list;
//! - no library the core reaches has a feature on the never-in-core-features
//!   list turned on;
//! - no core crate has a `.clippy.toml`, which Clippy would read instead of
//!   the `clippy.toml` that holds the house rules ([ADR-0001]);
//! - every crate, and each of its targets, is on the Rust edition `walls.toml`
//!   names, the one the Review Report reads code in.
//!
//! [ADR-0001]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0001-bit-exact-determinism-with-ordinary-floats.md
//! [ADR-0003]: https://github.com/BartoszSolkaBD/OpenDrone/blob/main/docs/adr/0003-crate-split-and-flight-inputs.md

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

/// The rules, built into the program so the readable checks use the real ones.
const RULES: &str = include_str!("../walls.toml");
pub(crate) const RULES_FILE: &str = "crates/xtask/walls.toml";

pub fn run(args: &[String]) -> ExitCode {
    let manifest_path = match args {
        [] => None,
        [flag, path] if flag == "--manifest-path" => Some(path.as_str()),
        _ => {
            eprintln!("Usage: cargo xtask walls [--manifest-path <Cargo.toml>]");
            return ExitCode::from(2);
        }
    };
    let checked = Rules::parse(RULES).and_then(|rules| {
        let workspace = Workspace::load(manifest_path)?;
        Ok((check(&workspace, &rules), workspace.members.len()))
    });
    match checked {
        Err(error) => {
            eprintln!("Could not check the walls: {error}");
            ExitCode::from(2)
        }
        Ok((problems, crates)) if problems.is_empty() => {
            println!(
                "The walls hold: all {crates} crates use only what {RULES_FILE} allows, \
                 and the core reaches no Bevy and nothing that touches the operating system."
            );
            ExitCode::SUCCESS
        }
        Ok((problems, _)) => {
            println!("The walls between crates are broken (ADR-0003):");
            for problem in &problems {
                println!("- {problem}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Every broken wall, as a plain sentence, in a fixed order.
fn check(workspace: &Workspace, rules: &Rules) -> Vec<String> {
    let mut problems = Vec::new();

    for library in rules.core_libraries.keys() {
        if let Some(reason) = rules.never_reason(library) {
            problems.push(format!(
                "{RULES_FILE} lets the core use `{library}`, but also says the core must never \
                 reach it: {reason}."
            ));
        }
    }
    for name in &rules.core {
        if !rules.crates.contains_key(name) {
            problems.push(format!(
                "{RULES_FILE} names `{name}` as a core crate but gives no rule for it."
            ));
        }
        if let Some(dotted) = workspace
            .member_named(name)
            .map(|id| workspace.folder(id).join(".clippy.toml"))
            .filter(|path| path.exists())
        {
            problems.push(format!(
                "`{name}` has a `.clippy.toml` ({}). Clippy reads it instead of the crate's \
                 `clippy.toml`, which holds the house rules (ADR-0001), so it turns them off. \
                 Keep the house rules in `clippy.toml` only, and remove the `.clippy.toml`.",
                workspace.relative(&dotted)
            ));
        }
    }

    for member in &workspace.members {
        let name = workspace.name(member);
        for (target, edition) in workspace.editions(member) {
            if *edition != rules.edition {
                let what = match target {
                    None => format!("`{name}`"),
                    Some(target) => format!("`{name}`'s target {target}"),
                };
                problems.push(format!(
                    "{what} is on Rust edition {edition} ({}), but every crate is on edition {} \
                     ({RULES_FILE}): the Review Report reads code the way rustc reads it in that \
                     edition. Take the workspace's with `edition.workspace = true`.",
                    workspace.relative_manifest(member),
                    rules.edition
                ));
            }
        }
        let Some(rule) = rules.crates.get(name) else {
            problems.push(format!(
                "`{name}` ({}) isn't one of the crates in {RULES_FILE}. ADR-0003 decides which \
                 crates exist, so a new crate needs that decision first, then a rule there.",
                workspace.relative_manifest(member)
            ));
            continue;
        };
        for dependency in workspace.dependencies(member) {
            if !workspace.is_member(&dependency.id) {
                continue;
            }
            let used = workspace.name(&dependency.id);
            if !rule.may_use.iter().any(|allowed| allowed == used) {
                problems.push(format!(
                    "`{name}` uses `{used}`, but {}: {}.",
                    allowed(&rule.may_use),
                    rule.rule
                ));
            }
        }
    }

    for (id, chain) in reached_from_core(workspace, rules) {
        if workspace.is_member(&id) {
            continue;
        }
        let library = workspace.name(&id);
        let chain = chain.join(" → ");
        for feature in workspace.features(&id) {
            if let Some(reason) = rules
                .never_in_core_features
                .get(&format!("{library}/{feature}"))
            {
                problems.push(format!(
                    "The core reaches `{library}` with its `{feature}` feature turned on, which \
                     it must never have: {reason}. It gets there through {chain}."
                ));
            }
        }
        if let Some(reason) = rules.never_reason(library) {
            problems.push(format!(
                "The core must never reach `{library}`: {reason}. It gets there through {chain}."
            ));
        } else if !rules.core_libraries.contains_key(library) {
            problems.push(format!(
                "The core reaches `{library}`, which isn't on its list of allowed outside \
                 libraries. It gets there through {chain}. If `{library}` never touches the \
                 operating system (no clock, files, threads, devices, network or \
                 operating-system randomness), add it under [core-libraries] in {RULES_FILE} \
                 with a one-line reason; otherwise remove it."
            ));
        }
    }

    problems
}

/// "it may use only `a` and `b`", or "it may use no other OpenDrone crate".
fn allowed(may_use: &[String]) -> String {
    match may_use {
        [] => "it may use no other OpenDrone crate".to_owned(),
        [only] => format!("it may use only `{only}`"),
        [first @ .., last] => {
            let first: Vec<String> = first.iter().map(|name| format!("`{name}`")).collect();
            format!("it may use only {} and `{last}`", first.join(", "))
        }
    }
}

/// Every package the core crates reach through the dependencies they are built
/// with (not those only their tests use), each with the shortest chain from a
/// core crate to it. The walk doesn't go on through OpenDrone crates outside
/// the core: a core crate that uses one is already reported by the rules for
/// who may use whom.
fn reached_from_core(workspace: &Workspace, rules: &Rules) -> Vec<(String, Vec<String>)> {
    let mut came_from: BTreeMap<&str, Option<&str>> = BTreeMap::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    for core_crate in &rules.core {
        if let Some(id) = workspace.member_named(core_crate) {
            came_from.insert(id, None);
            queue.push_back(id);
        }
    }
    let is_core = |id: &str| rules.core.iter().any(|name| name == workspace.name(id));

    let mut reached = Vec::new();
    while let Some(id) = queue.pop_front() {
        if workspace.is_member(id) && !is_core(id) {
            continue;
        }
        for dependency in workspace.dependencies(id) {
            if !dependency.built_with || came_from.contains_key(dependency.id.as_str()) {
                continue;
            }
            came_from.insert(&dependency.id, Some(id));
            queue.push_back(&dependency.id);
            reached.push(dependency.id.as_str());
        }
    }

    reached
        .into_iter()
        .map(|id| {
            let mut chain = vec![workspace.name(id).to_owned()];
            let mut at = id;
            while let Some(&Some(previous)) = came_from.get(at) {
                chain.push(workspace.name(previous).to_owned());
                at = previous;
            }
            chain.reverse();
            (id.to_owned(), chain)
        })
        .collect()
}

/// The rules in `walls.toml`.
pub(crate) struct Rules {
    pub(crate) core: Vec<String>,
    /// The Rust edition every crate is on.
    edition: String,
    crates: BTreeMap<String, CrateRule>,
    core_libraries: BTreeMap<String, String>,
    never_in_core: BTreeMap<String, String>,
    /// By "library/feature".
    never_in_core_features: BTreeMap<String, String>,
    /// Calls to the operating system's maths the core may keep, by
    /// "library/function" (`cargo xtask core-maths`).
    core_platform_maths: BTreeMap<String, PlatformMathsAllowance>,
}

/// One call to the operating system's maths the core may keep: only from
/// these functions, for this reason.
struct PlatformMathsAllowance {
    from: Vec<String>,
    reason: String,
}

struct CrateRule {
    may_use: Vec<String>,
    rule: String,
}

impl Rules {
    /// The rules in `walls.toml`, built into the program.
    pub(crate) fn load() -> Result<Rules, String> {
        Rules::parse(RULES)
    }

    /// Why the core may keep this call to the operating system's maths, if
    /// `walls.toml` allows it: only when every function that makes it is one
    /// the allowance names, so a new caller fails the check. A reference from
    /// outside any function, such as a table of function pointers, is never
    /// allowed: nothing says what calls through it.
    pub(crate) fn platform_maths_reason(&self, call: &crate::core_maths::Call) -> Option<&str> {
        let allowance = self
            .core_platform_maths
            .get(&format!("{}/{}", call.library, call.function))?;
        let all_named = !call.callers.is_empty()
            && call.callers.iter().all(|caller| {
                caller
                    .function_name()
                    .is_some_and(|name| allowance.from.iter().any(|from| from == name))
            });
        all_named.then_some(allowance.reason.as_str())
    }

    fn parse(text: &str) -> Result<Rules, String> {
        let table: toml::Table = text
            .parse()
            .map_err(|error| format!("{RULES_FILE} isn't valid TOML: {error}"))?;
        let mut crates = BTreeMap::new();
        for (name, rule) in section(&table, "crates")? {
            crates.insert(
                name.clone(),
                CrateRule {
                    may_use: strings(rule.get("may-use"), &format!("crates.{name}.may-use"))?,
                    rule: rule
                        .get("rule")
                        .and_then(toml::Value::as_str)
                        .ok_or(format!("{RULES_FILE} has no `rule` for {name}"))?
                        .to_owned(),
                },
            );
        }
        Ok(Rules {
            core: strings(table.get("core"), "core")?,
            edition: table
                .get("edition")
                .and_then(toml::Value::as_str)
                .ok_or(format!("{RULES_FILE} names no `edition`"))?
                .to_owned(),
            crates,
            core_libraries: reasons(&table, "core-libraries")?,
            never_in_core: reasons(&table, "never-in-core")?,
            never_in_core_features: reasons(&table, "never-in-core-features")?,
            core_platform_maths: platform_maths_allowances(&table)?,
        })
    }

    /// Why the core must never reach this library, if it's on that list. A name
    /// ending in `*` covers every library whose name starts that way.
    fn never_reason(&self, library: &str) -> Option<&str> {
        self.never_in_core
            .iter()
            .find(|(pattern, _)| match pattern.strip_suffix('*') {
                Some(start) => library.starts_with(start),
                None => library == pattern.as_str(),
            })
            .map(|(_, reason)| reason.as_str())
    }
}

fn section<'a>(table: &'a toml::Table, key: &str) -> Result<&'a toml::Table, String> {
    table
        .get(key)
        .and_then(toml::Value::as_table)
        .ok_or(format!("{RULES_FILE} has no [{key}] section"))
}

fn strings(value: Option<&toml::Value>, key: &str) -> Result<Vec<String>, String> {
    let wrong = || format!("{RULES_FILE}: `{key}` must be a list of names");
    value
        .and_then(toml::Value::as_array)
        .ok_or_else(wrong)?
        .iter()
        .map(|name| name.as_str().map(str::to_owned).ok_or_else(wrong))
        .collect()
}

fn platform_maths_allowances(
    table: &toml::Table,
) -> Result<BTreeMap<String, PlatformMathsAllowance>, String> {
    let key = "core-platform-maths";
    section(table, key)?
        .iter()
        .map(|(name, allowance)| {
            let wrong = || {
                format!(
                    "{RULES_FILE}: [{key}] {name} needs `{{ from = [\"<function>\", …], reason = \"…\" }}`"
                )
            };
            let reason = allowance
                .get("reason")
                .and_then(toml::Value::as_str)
                .ok_or_else(wrong)?
                .to_owned();
            let from = strings(allowance.get("from"), &format!("{key}.{name}.from"))?;
            Ok((name.clone(), PlatformMathsAllowance { from, reason }))
        })
        .collect()
}

fn reasons(table: &toml::Table, key: &str) -> Result<BTreeMap<String, String>, String> {
    section(table, key)?
        .iter()
        .map(|(name, reason)| match reason.as_str() {
            Some(reason) => Ok((name.clone(), reason.to_owned())),
            None => Err(format!(
                "{RULES_FILE}: [{key}] {name} needs a reason in quotes"
            )),
        })
        .collect()
}

/// The workspace's packages and dependencies, from `cargo metadata`.
struct Workspace {
    root: String,
    /// The workspace's own crates, by name.
    members: Vec<String>,
    names: BTreeMap<String, String>,
    manifests: BTreeMap<String, String>,
    /// Each package's dependencies, by name.
    dependencies: BTreeMap<String, Vec<Dependency>>,
    /// The features cargo turns on in each package, merged across the whole
    /// workspace.
    features: BTreeMap<String, Vec<String>>,
    /// Each package's Rust edition (no target), and each of its targets' (a
    /// target such as `lib \`opendrone_sim\``).
    editions: BTreeMap<String, Vec<(Option<String>, String)>>,
}

struct Dependency {
    id: String,
    /// The crate is built with it, as a normal or build dependency, rather
    /// than only its tests and examples using it.
    built_with: bool,
}

impl Workspace {
    fn load(manifest_path: Option<&str>) -> Result<Workspace, String> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
        command.args(["metadata", "--format-version", "1", "--all-features"]);
        if let Some(path) = manifest_path {
            command.args(["--manifest-path", path]);
        }
        let output = command
            .output()
            .map_err(|error| format!("couldn't run cargo metadata: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "cargo metadata failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let metadata: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("cargo metadata gave unreadable output: {error}"))?;
        Workspace::from_metadata(&metadata)
    }

    fn from_metadata(metadata: &Value) -> Result<Workspace, String> {
        let mut names = BTreeMap::new();
        let mut manifests = BTreeMap::new();
        let mut editions = BTreeMap::new();
        for package in list(&metadata["packages"], "packages")? {
            let id = text(&package["id"], "a package id")?;
            names.insert(id.clone(), text(&package["name"], "a package name")?);
            manifests.insert(
                id.clone(),
                text(&package["manifest_path"], "a manifest path")?,
            );
            let mut found = vec![(None, text(&package["edition"], "a package's edition")?)];
            for target in list(&package["targets"], "a package's targets")? {
                let kind = list(&target["kind"], "a target's kinds")?
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                found.push((
                    Some(format!(
                        "{kind} `{}`",
                        text(&target["name"], "a target's name")?
                    )),
                    text(&target["edition"], "a target's edition")?,
                ));
            }
            editions.insert(id, found);
        }
        let name_of = |id: &str| names.get(id).cloned().unwrap_or_else(|| id.to_owned());

        let mut members = list(&metadata["workspace_members"], "workspace_members")?
            .iter()
            .map(|id| text(id, "a workspace member"))
            .collect::<Result<Vec<_>, _>>()?;
        members.sort_by_key(|id| (name_of(id), id.clone()));

        let mut dependencies = BTreeMap::new();
        let mut features = BTreeMap::new();
        for node in list(&metadata["resolve"]["nodes"], "resolve.nodes")? {
            let on = list(&node["features"], "a node's features")?
                .iter()
                .map(|feature| text(feature, "a feature name"))
                .collect::<Result<Vec<_>, _>>()?;
            features.insert(text(&node["id"], "a node id")?, on);
            let mut uses = Vec::new();
            for dependency in list(&node["deps"], "a node's deps")? {
                let built_with = match dependency["dep_kinds"].as_array() {
                    Some(kinds) => kinds
                        .iter()
                        .any(|kind| kind["kind"].as_str() != Some("dev")),
                    None => true,
                };
                uses.push(Dependency {
                    id: text(&dependency["pkg"], "a dependency id")?,
                    built_with,
                });
            }
            uses.sort_by_key(|dependency| (name_of(&dependency.id), dependency.id.clone()));
            dependencies.insert(text(&node["id"], "a node id")?, uses);
        }

        Ok(Workspace {
            root: text(&metadata["workspace_root"], "workspace_root")?,
            members,
            names,
            manifests,
            dependencies,
            features,
            editions,
        })
    }

    /// A package's Rust edition, and each of its targets'.
    fn editions(&self, id: &str) -> &[(Option<String>, String)] {
        self.editions.get(id).map_or(&[], Vec::as_slice)
    }

    fn name<'a>(&'a self, id: &'a str) -> &'a str {
        self.names.get(id).map_or(id, String::as_str)
    }

    fn is_member(&self, id: &str) -> bool {
        self.members.iter().any(|member| member == id)
    }

    fn member_named(&self, name: &str) -> Option<&str> {
        self.members
            .iter()
            .find(|id| self.name(id) == name)
            .map(String::as_str)
    }

    /// The features cargo turns on in a package.
    fn features(&self, id: &str) -> &[String] {
        self.features.get(id).map_or(&[], Vec::as_slice)
    }

    fn dependencies(&self, id: &str) -> &[Dependency] {
        self.dependencies.get(id).map_or(&[], Vec::as_slice)
    }

    fn relative_manifest(&self, id: &str) -> String {
        let manifest = self.manifests.get(id).map_or(id, String::as_str);
        self.relative(Path::new(manifest))
    }

    /// The folder that holds a package's `Cargo.toml`.
    fn folder(&self, id: &str) -> PathBuf {
        let manifest = Path::new(self.manifests.get(id).map_or(id, String::as_str));
        manifest.parent().unwrap_or(manifest).to_path_buf()
    }

    /// A path from the workspace's root, with `/` between folders.
    fn relative(&self, path: &Path) -> String {
        path.strip_prefix(&self.root).map_or_else(
            |_| path.display().to_string(),
            |path| {
                path.components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            },
        )
    }
}

fn list<'a>(value: &'a Value, what: &str) -> Result<&'a Vec<Value>, String> {
    value
        .as_array()
        .ok_or(format!("cargo metadata has no list of {what}"))
}

fn text(value: &Value, what: &str) -> Result<String, String> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or(format!("cargo metadata is missing {what}"))
}
