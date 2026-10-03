//! Moved from `lint.rs` (move only): load_harness_files, load_baseline, run, exit_code.

#[allow(unused_imports)]
use super::*;

/// The harness files, and only the harness files.
pub fn load_harness_files(root: &Path) -> Vec<HarnessFile> {
    fn add(root: &Path, path: &Path, out: &mut Vec<HarnessFile>) {
        let Ok(content) = fs::read_to_string(path) else {
            return; // a missing optional file is not a finding
        };
        let Ok(relative) = path.strip_prefix(root) else {
            return;
        };
        out.push(HarnessFile {
            path: relative.to_string_lossy().to_string(),
            content,
        });
    }
    fn add_dir(
        root: &Path,
        dir: &Path,
        filter: &dyn Fn(&str) -> bool,
        out: &mut Vec<HarnessFile>,
    ) {
        let Ok(entries) = fs::read_dir(dir) else {
            return; // absent directory
        };
        let mut names: Vec<String> = entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| filter(name))
            .collect();
        names.sort();
        for name in names {
            add(root, &dir.join(name), out);
        }
    }

    let mut out: Vec<HarnessFile> = Vec::new();
    add(root, &root.join("AGENTS.md"), &mut out);
    add(root, &root.join("docs/agent/MEMORY.md"), &mut out);
    add_dir(
        root,
        &root.join("docs/agent/packets"),
        &|name: &str| name.ends_with(".md"),
        &mut out,
    );
    add_dir(
        root,
        &root.join("docs/agent/skills"),
        &|name: &str| name.ends_with(".md"),
        &mut out,
    );
    // The MAP pages: they claim to point at real files, so they are scanned (rule 7 checks the claim).
    add_dir(
        root,
        &root.join("docs/agent"),
        &|name: &str| name.starts_with("ORIENTATION") || name.starts_with("MAP-"),
        &mut out,
    );
    add_dir(
        root,
        &root.join("agent-runtime"),
        &|name: &str| name.ends_with(".ts") && !name.ends_with(".test.ts"),
        &mut out,
    );
    // Generated scope manifests: each row claims a path on disk (rule 8 checks the claim).
    add_dir(
        root,
        &root.join("docs/agent/manifest"),
        &|name: &str| name.ends_with(".md"),
        &mut out,
    );
    // Decision mirrors (migration 180): scanned for STRUCTURE only — whether each file still matches
    // its row needs the database, and that check lives in `pnpm forge:decision check`.
    add_dir(
        root,
        &root.join("docs/agent/decisions"),
        &|name: &str| name.ends_with(".md"),
        &mut out,
    );
    // Vendor pointer files that carry a generated block (rule 9 checks it has not drifted).
    for name in MANAGED_VENDOR_FILES {
        add(root, &root.join(name), &mut out);
    }

    out
}

/// The recorded debt, if any. An ABSENT file means "no baseline", which fails on everything — that is a
/// stated fact, not a silent one. An UNREADABLE file is an error the caller turns into a finding: the
/// TypeScript caught the parse error and returned "no debt", so a corrupt file quietly promoted
/// recorded debt back into violations.
pub fn load_baseline(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join(BASELINE_PATH);
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{BASELINE_PATH} could not be read: {error}")),
    };
    let parsed: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "{BASELINE_PATH} is not valid JSON ({error}) — the recorded debt cannot be read, so every \
             baselined finding would fail. Fix the file: it must be one JSON object with a `findings` array."
        )
    })?;
    Ok(parsed
        .get("findings")
        .and_then(serde_json::Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter_map(|key| key.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

/// The CLI. Exit is 0 unless `--strict` is passed and something failed: a doc gate must not be able to
/// stop a release, which is why the default is "reported, not blocking".
pub fn run(args: &[String]) -> Result<u8, Failure> {
    let mut strict = false;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--strict" => strict = true,
            "--format" => {
                json = args.get(index + 1).map(String::as_str) == Some("json");
                index += 1;
            }
            other => {
                return Err(Failure::usage(format!(
                    "unknown argument `{other}`; usage: forge harness-lint [--strict] [--format json]"
                )))
            }
        }
        index += 1;
    }

    let root = crate::forge::repo_root();
    let files = load_harness_files(&root);
    let resolver = Resolver::new(root.clone());
    let mut findings: Vec<Finding> = Vec::new();
    let baseline: HashSet<String> = match load_baseline(&root) {
        Ok(keys) => keys.into_iter().collect(),
        Err(message) => {
            findings.push(finding(
                Level::Fail,
                "baseline-unreadable",
                BASELINE_PATH,
                None,
                message,
            ));
            HashSet::new()
        }
    };
    findings.extend(lint_harness(&files, &KNOWN_SKILLS, &baseline, &resolver));

    let failures = findings
        .iter()
        .filter(|finding| finding.level == Level::Fail)
        .count();
    let warnings = findings.len() - failures;
    let baselined = findings.iter().filter(|finding| finding.baselined).count();

    if json {
        let payload = serde_json::json!({
            "filesScanned": files.len(),
            "failures": failures,
            "warnings": warnings,
            "baselined": baselined,
            "findings": findings
                .iter()
                .map(|finding| serde_json::json!({
                    "level": finding.level.as_str(),
                    "rule": finding.rule,
                    "file": finding.file,
                    "line": finding.line,
                    "message": finding.message,
                    "baselined": finding.baselined,
                }))
                .collect::<Vec<_>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(exit_code(strict, failures));
    }

    for finding in &findings {
        let location = match finding.line {
            Some(line) => format!("{}:{}", finding.file, line),
            None => finding.file.clone(),
        };
        println!(
            "{}  {:<32} {}\n      {}",
            if finding.level == Level::Fail {
                "FAIL"
            } else {
                "warn"
            },
            finding.rule,
            location,
            finding.message
        );
    }
    println!(
        "\nforge:packet-lint — {failures} failure(s), {warnings} warning(s) ({baselined} baselined), {} harness file(s) scanned{}",
        files.len(),
        if strict && failures > 0 {
            ""
        } else {
            " — reported, not blocking (use --strict to block)"
        }
    );
    Ok(exit_code(strict, failures))
}

pub(super) fn exit_code(strict: bool, failures: usize) -> u8 {
    if strict && failures > 0 {
        1
    } else {
        0
    }
}
