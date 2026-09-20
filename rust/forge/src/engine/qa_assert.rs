//! Port of workflow_app/forge/agents/qa/run.ts assertion provenance rules.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerVerdict {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssertionEntry {
    pub name: String,
    pub file: Option<String>,
    pub verdict: MarkerVerdict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbsentReason {
    NotRun,
    WrongFile,
    NoProvenance,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssertionResolution {
    Passed,
    Failed,
    Skipped,
    Absent {
        reason: AbsentReason,
        detail: String,
    },
}

pub fn parse_assertion_ref(r#ref: &str) -> (Option<String>, String) {
    let trimmed = r#ref.trim();
    match trimmed.rfind('#') {
        Some(at) => {
            let file = trimmed[..at].trim();
            let name = trimmed[at + 1..].trim();
            (
                if file.is_empty() {
                    None
                } else {
                    Some(file.to_string())
                },
                name.to_string(),
            )
        }
        None => (None, trimmed.to_string()),
    }
}

fn skip_directive(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("# skip") || lower.contains("# todo")
}

fn marker_line(line: &str) -> Option<MarkerVerdict> {
    let text = line.trim_start();
    if skip_directive(text) {
        return Some(MarkerVerdict::Skipped);
    }
    if text.starts_with("not ok") {
        return Some(MarkerVerdict::Failed);
    }
    if text.starts_with('✖') || text.starts_with('✗') || text.starts_with('✘') {
        return Some(MarkerVerdict::Failed);
    }
    if text.starts_with('✔') || text.starts_with('✓') {
        return Some(MarkerVerdict::Passed);
    }
    if text.starts_with("ok") && (text.len() == 2 || !text.as_bytes()[2].is_ascii_alphanumeric()) {
        return Some(MarkerVerdict::Passed);
    }
    None
}

fn marker_name(line: &str) -> Option<String> {
    marker_line(line)?;
    let text = line.trim_start();
    let raw = if let Some(rest) = text.strip_prefix("not ok").or_else(|| {
        text.strip_prefix("ok")
            .filter(|s| s.starts_with(|c: char| c.is_whitespace() || c == '-'))
    }) {
        let rest = rest.trim_start();
        let rest = rest
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start();
        rest.strip_prefix('-').unwrap_or(rest).trim()
    } else if text.starts_with(['✔', '✓', '✖', '✗', '✘']) {
        text.chars()
            .next()
            .and_then(|_| Some(text.chars().skip(1).collect::<String>()))
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
    } else {
        return None;
    };
    let cleaned = raw
        .split('#')
        .next()
        .unwrap_or(raw)
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_whitespace());
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn decode_xml(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn attr<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let start = attrs.find(&key)? + key.len();
    let end = attrs[start..].find('"')? + start;
    Some(&attrs[start..end])
}

pub fn assertion_entries(output: &str) -> Vec<AssertionEntry> {
    if output.trim().is_empty() {
        return vec![];
    }
    let junit = junit_entries(output);
    if !junit.is_empty() {
        return junit;
    }
    tap_entries(output)
}

fn junit_entries(text: &str) -> Vec<AssertionEntry> {
    let mut entries = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("<testcase") {
        rest = &rest[at + 9..];
        let Some(end_attrs) = rest.find('>') else {
            break;
        };
        let attrs = &rest[..end_attrs];
        let self_close = attrs.ends_with('/');
        let body = if self_close {
            ""
        } else {
            let close = rest.find("</testcase>").unwrap_or(rest.len());
            &rest[end_attrs + 1..close]
        };
        if let Some(name) = attr(attrs.trim_end_matches('/'), "name") {
            let file = attr(attrs.trim_end_matches('/'), "file").map(decode_xml);
            let verdict = if body.contains("<failure") || body.contains("<error") {
                MarkerVerdict::Failed
            } else if body.contains("<skipped") || body.contains("<disabled") {
                MarkerVerdict::Skipped
            } else {
                MarkerVerdict::Passed
            };
            entries.push(AssertionEntry {
                name: decode_xml(name),
                file,
                verdict,
            });
        }
        rest = &rest[end_attrs.min(rest.len())..];
    }
    entries
}

fn tap_entries(text: &str) -> Vec<AssertionEntry> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut entries = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(verdict) = marker_line(line) else {
            continue;
        };
        let Some(name) = marker_name(line) else {
            continue;
        };
        let mut file = None;
        if lines.get(i + 1).map(|s| s.trim()) == Some("---") {
            for follow in lines.iter().skip(i + 2) {
                if follow.trim() == "..." {
                    break;
                }
                if let Some(loc) = follow.trim().strip_prefix("location:") {
                    let raw = loc.trim().trim_matches('\'');
                    let without = raw.rsplit_once(':').map(|(a, _)| a).unwrap_or(raw);
                    let without = without.rsplit_once(':').map(|(a, _)| a).unwrap_or(without);
                    if !without.is_empty() {
                        file = Some(without.to_string());
                    }
                    break;
                }
            }
        }
        entries.push(AssertionEntry {
            name,
            file,
            verdict,
        });
    }
    entries
}

fn same_file_path(candidate: &str, wanted: &str) -> bool {
    let clean = |v: &str| {
        v.replace('\\', "/")
            .trim_start_matches("./")
            .trim_end_matches('/')
            .to_string()
    };
    let a = clean(candidate);
    let b = clean(wanted);
    a == b || a.ends_with(&format!("/{b}")) || b.ends_with(&format!("/{a}"))
}

pub fn assertion_resolution(output: &str, r#ref: &str) -> AssertionResolution {
    let (file, name) = parse_assertion_ref(r#ref);
    if name.is_empty() {
        return AssertionResolution::Absent {
            reason: AbsentReason::NotRun,
            detail: "the ref names no assertion".into(),
        };
    }
    let entries = assertion_entries(output);
    let by_name: Vec<_> = entries.into_iter().filter(|e| e.name == name).collect();
    if by_name.is_empty() {
        return AssertionResolution::Absent {
            reason: AbsentReason::NotRun,
            detail: format!("no marker line reports {name}"),
        };
    }
    if let Some(wanted) = file {
        let matched: Vec<_> = by_name
            .iter()
            .filter(|e| {
                e.file
                    .as_deref()
                    .map(|f| same_file_path(f, &wanted))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        if matched.is_empty() {
            if by_name.iter().all(|e| e.file.is_none()) {
                return AssertionResolution::Absent {
                    reason: AbsentReason::NoProvenance,
                    detail: format!("{name} ran with no file provenance"),
                };
            }
            return AssertionResolution::Absent {
                reason: AbsentReason::WrongFile,
                detail: format!("{name} ran in a different file than {wanted}"),
            };
        }
        return fold(&matched);
    }
    let origins: Vec<_> = by_name.iter().filter_map(|e| e.file.clone()).collect();
    let mut uniq = origins.clone();
    uniq.sort();
    uniq.dedup();
    if uniq.len() > 1 {
        return AssertionResolution::Absent {
            reason: AbsentReason::Ambiguous,
            detail: format!("{name} reported by multiple files"),
        };
    }
    fold(&by_name)
}

fn fold(entries: &[AssertionEntry]) -> AssertionResolution {
    if entries.iter().any(|e| e.verdict == MarkerVerdict::Failed) {
        return AssertionResolution::Failed;
    }
    if entries.iter().any(|e| e.verdict == MarkerVerdict::Passed) {
        return AssertionResolution::Passed;
    }
    AssertionResolution::Skipped
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skip_is_not_pass() {
        let out = "ok 1 - required assertion # SKIP missing tool\n";
        assert_eq!(
            assertion_resolution(out, "required assertion"),
            AssertionResolution::Skipped
        );
    }
    #[test]
    fn file_qualified_ref_rejects_other_file() {
        let out = r#"<testcase name="same assertion" file="/b.test.ts"></testcase>"#;
        match assertion_resolution(out, "a.test.ts#same assertion") {
            AssertionResolution::Absent {
                reason: AbsentReason::WrongFile,
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }
}
