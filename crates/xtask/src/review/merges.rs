//! Merges that skipped a check (ADR-0010): after a PR merges into main, which
//! required checks its last commit hadn't passed. Agents never merge past a
//! failing check, but they hold the maintainer's rights in Phase 1, so the
//! next Review Reports name any merge that did.

use std::collections::BTreeMap;

use serde_json::Value;

/// One check that hadn't passed when the PR merged, with its state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub check: String,
    /// Such as "failure", "in progress" or "never reported".
    pub state: String,
}

/// The checks main requires, from branch protection (`GET
/// /repos/{owner}/{repo}/branches/main`) and rulesets (`GET
/// /repos/{owner}/{repo}/rules/branches/main`).
pub fn required_checks(branch: &Value, rules: &Value) -> Vec<String> {
    let mut required = Vec::new();
    let protection = branch.pointer("/protection/required_status_checks");
    let contexts = protection
        .and_then(|p| p.get("contexts"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    let checks = protection
        .and_then(|p| p.get("checks"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("context").and_then(Value::as_str));
    required.extend(contexts.chain(checks).map(str::to_string));
    for rule in rules.as_array().into_iter().flatten() {
        if rule.get("type").and_then(Value::as_str) != Some("required_status_checks") {
            continue;
        }
        let checks = rule
            .pointer("/parameters/required_status_checks")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|c| c.get("context").and_then(Value::as_str));
        required.extend(checks.map(str::to_string));
    }
    required.sort();
    required.dedup();
    required
}

/// The required checks the merged commit hadn't passed. `check_runs` is GitHub's
/// list of check runs for the commit (one or more pages of `GET
/// /repos/{owner}/{repo}/commits/{sha}/check-runs`), and `status` its combined
/// status (`GET /repos/{owner}/{repo}/commits/{sha}/status`). If main requires
/// no checks yet, every check that reported counts.
pub fn skipped_checks(required: &[String], check_runs: &str, status: &Value) -> Vec<Skipped> {
    // Each check run's name, with its newest run's state: GitHub numbers runs
    // in order, so a re-run has a higher id.
    let mut runs: BTreeMap<String, (u64, String)> = BTreeMap::new();
    for page in serde_json::Deserializer::from_str(check_runs).into_iter::<Value>() {
        let Ok(page) = page else { continue };
        let pages = match page {
            Value::Array(pages) => pages,
            page => vec![page],
        };
        for page in pages {
            for run in page
                .get("check_runs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(name) = run.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let id = run.get("id").and_then(Value::as_u64).unwrap_or_default();
                let state = match (
                    run.get("status").and_then(Value::as_str),
                    run.get("conclusion").and_then(Value::as_str),
                ) {
                    (Some("completed"), Some(conclusion)) => conclusion.to_string(),
                    _ => "in progress".to_string(),
                };
                if runs.get(name).is_none_or(|(newest, _)| id > *newest) {
                    runs.insert(name.to_string(), (id, state));
                }
            }
        }
    }
    let statuses: BTreeMap<String, String> = status
        .get("statuses")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some((
                s.get("context")?.as_str()?.to_string(),
                s.get("state")?.as_str()?.to_string(),
            ))
        })
        .collect();

    let names: Vec<String> = if required.is_empty() {
        runs.keys().chain(statuses.keys()).cloned().collect()
    } else {
        required.to_vec()
    };
    let mut skipped = Vec::new();
    for name in names {
        // As GitHub does: when a check run and a status share a name, both
        // must have passed.
        let run = runs.get(&name).map(|(_, state)| state.as_str());
        let status = statuses.get(&name).map(String::as_str);
        let run_passed = run.is_none_or(|s| matches!(s, "success" | "neutral" | "skipped"));
        let status_passed = status.is_none_or(|s| s == "success");
        if (run.is_some() || status.is_some()) && run_passed && status_passed {
            continue;
        }
        let state = match (run, status) {
            (Some(run), _) if !run_passed => run,
            (_, Some(status)) if !status_passed => status,
            _ => "never reported",
        }
        .to_string();
        if !skipped.iter().any(|s: &Skipped| s.check == name) {
            skipped.push(Skipped { check: name, state });
        }
    }
    skipped
}

/// The comment for a PR that merged past a check.
pub fn comment(skipped: &[Skipped]) -> String {
    let checks: Vec<String> = skipped
        .iter()
        .map(|s| format!("- **{}**: {}", s.check, s.state))
        .collect();
    format!(
        "**This PR merged before every required check had passed:**\n\n{}\n\n\
         Agents never merge past a failing check (ADR-0010), so every Review Report names this \
         merge until the maintainer takes the `{}` label off.\n",
        checks.join("\n"),
        super::update::SKIPPED_A_CHECK
    )
}
