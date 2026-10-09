//! Legacy JavaScript and TypeScript heuristics. These never run over Rust source.

use super::RuleObservation;
use std::collections::BTreeSet;

pub(super) fn scan(files: &[(String, String)], revision: &str) -> Vec<RuleObservation> {
    let captures = [
        "captureServerError",
        "captureServerLog",
        "captureError",
        "recordError",
        "withApiHandler",
        "withServerErrorCapture",
    ];
    let mut findings = Vec::new();
    let mut seen = BTreeSet::new();

    for (path, content) in files.iter().filter(|(path, _)| is_js_or_ts(path)) {
        let compact = content.replace('\r', "");
        for needle in ["catch {}", "catch{}", "catch (_) {}", "catch(_){ }"] {
            let mut from = 0;
            while let Some(offset) = compact[from..].find(needle) {
                let at = from + offset;
                push(&mut findings, &mut seen, "JS-EMPTY-CATCH", path, &compact, at, revision,
                    "An empty catch block visibly discards the exception.",
                    "The text rule can still match comments or string literals; review the source context.");
                from = at + needle.len();
            }
        }

        for fallback in [
            "=> []",
            "=> null",
            "=> undefined",
            "=> 0",
            "=> ''",
            "=> \"\"",
        ] {
            let mut from = 0;
            while let Some(offset) = compact[from..].find(".catch(") {
                let at = from + offset;
                let tail = &compact[at..compact.len().min(at + 180)];
                if tail.contains(fallback) {
                    push(&mut findings, &mut seen, "JS-SWALLOWED-CATCH", path, &compact, at, revision,
                        "A promise catch appears to turn an error into a default value.",
                        "The text rule does not establish whether the default is contractually safe.");
                }
                from = at + 7;
            }
        }

        let server = path.starts_with("app/")
            || path.starts_with("services/")
            || path.contains("/app/")
            || path.contains("/services/");
        let captured = captures.iter().any(|marker| compact.contains(marker));
        if server && !captured {
            let mut from = 0;
            while let Some(offset) = compact[from..].find("console.error(") {
                let at = from + offset;
                push(&mut findings, &mut seen, "JS-CONSOLE-ERROR-WITHOUT-CAPTURE", path, &compact, at, revision,
                    "A server-side console.error call has no recognized durable-capture marker in this file.",
                    "Capture may be provided by a wrapper or helper this file-level heuristic cannot see.");
                from = at + 14;
            }
        }

        if compact.contains("catch") && compact.contains("status: 500") && !captured {
            if let Some(at) = compact.find("status: 500") {
                push(&mut findings, &mut seen, "JS-BARE-500-IN-CATCH", path, &compact, at, revision,
                    "A catch-associated 500 response has no recognized durable-capture marker in this file.",
                    "The rule does not prove the route is authoritative or uncaptured at a shared boundary.");
            }
        }
    }
    findings
}

fn is_js_or_ts(path: &str) -> bool {
    [".ts", ".tsx", ".js", ".mjs"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
}

fn push(
    findings: &mut Vec<RuleObservation>,
    seen: &mut BTreeSet<String>,
    rule_id: &str,
    path: &str,
    source: &str,
    byte: usize,
    revision: &str,
    rationale: &str,
    limitations: &str,
) {
    let start_line = source[..byte.min(source.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let key = format!("{rule_id}:v1:{path}:{start_line}");
    if !seen.insert(key.clone()) {
        return;
    }
    let line = source.lines().nth(start_line - 1).unwrap_or("").trim();
    findings.push(RuleObservation {
        rule_id: rule_id.to_string(),
        rule_version: 1,
        key,
        source_revision: revision.to_string(),
        path: path.to_string(),
        start_line,
        end_line: start_line,
        normalized_context: line
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(200)
            .collect(),
        rationale: rationale.to_string(),
        severity: "review".to_string(),
        confidence: "low".to_string(),
        limitations: limitations.to_string(),
    });
}
