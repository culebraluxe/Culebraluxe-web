//! Port of architect-handoff.ts + architect/assess.ts.

use crate::engine::path::file_of;

pub const ARCHITECT_HANDOFF_PREFIX: &str = "FORGE_ARCHITECT_HANDOFF:";
pub const MAX_SEAMS_PER_FINDING: usize = 3;
pub const ARCHITECT_HANDOFF_MISSING: &str = "ARCHITECT: no readable handoff. End the reply with exactly ONE un-fenced JSON line beginning FORGE_ARCHITECT_HANDOFF:";

#[derive(Debug, Clone)]
pub struct ArchitectHandoffFinding {
    pub id: String,
    pub required: bool,
    pub summary: String,
    pub scope: Vec<String>,
    pub proofs: Vec<String>,
    pub risks: Vec<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ArchitectHandoff {
    pub version: u32,
    pub base_ref: String,
    pub findings: Vec<ArchitectHandoffFinding>,
}

pub fn last_machine_line(raw: &str, prefix: &str) -> Option<String> {
    raw.lines()
        .map(|s| s.trim())
        .filter(|s| s.starts_with(prefix))
        .last()
        .map(|s| s.to_string())
}

fn extract_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, ch) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..start + i + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_findings_block(slice: &str) -> Option<ArchitectHandoff> {
    // Minimal: require version/baseRef/findings tokens present; extract findings via id+summary pairs.
    if !slice.contains("\"version\"")
        || !slice.contains("\"baseRef\"")
        || !slice.contains("\"findings\"")
    {
        return None;
    }
    let version_ok = slice.contains("\"version\":1") || slice.contains("\"version\": 1");
    if !version_ok {
        return None;
    }
    let base = capture_string(slice, "baseRef").unwrap_or_default();
    let mut findings = Vec::new();
    let mut rest = slice;
    while let Some(at) = rest.find("\"id\"") {
        rest = &rest[at + 4..];
        let id = capture_next_string(rest)?;
        let summary = capture_string(rest, "summary").unwrap_or_default();
        if id.is_empty() || summary.is_empty() {
            return None;
        }
        let required = rest.contains("\"required\":true") || rest.contains("\"required\": true");
        let hint = capture_string(rest, "hint");
        let scope = capture_string_array(rest, "scope");
        let seams = capture_string_array(rest, "seams");
        findings.push(ArchitectHandoffFinding {
            id,
            required,
            summary,
            scope: if scope.is_empty() { seams } else { scope },
            proofs: capture_string_array(rest, "proofs"),
            risks: capture_string_array(rest, "risks"),
            hint,
        });
        if let Some(end) = rest.find('}') {
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    Some(ArchitectHandoff {
        version: 1,
        base_ref: base,
        findings,
    })
}

fn capture_string(src: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let at = src.find(&pat)?;
    capture_next_string(&src[at + pat.len()..])
}

fn capture_next_string(src: &str) -> Option<String> {
    let start = src.find('"')?;
    let rest = &src[start + 1..];
    let mut out = String::new();
    let mut esc = false;
    for ch in rest.chars() {
        if esc {
            out.push(ch);
            esc = false;
            continue;
        }
        if ch == '\\' {
            esc = true;
            continue;
        }
        if ch == '"' {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn capture_string_array(src: &str, key: &str) -> Vec<String> {
    let pat = format!("\"{key}\"");
    let Some(at) = src.find(&pat) else {
        return vec![];
    };
    let after = &src[at + pat.len()..];
    let Some(lb) = after.find('[') else {
        return vec![];
    };
    let after = &after[lb + 1..];
    let Some(rb) = after.find(']') else {
        return vec![];
    };
    after[..rb]
        .split(',')
        .filter_map(|p| {
            let s = p.trim().trim_matches('"');
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        })
        .collect()
}

pub fn parse_architect_handoff(text: &str) -> Option<ArchitectHandoff> {
    let mut positions = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(ARCHITECT_HANDOFF_PREFIX) {
        positions.push(from + at);
        from += at + 1;
    }
    for &pos in positions.iter().rev() {
        let after = &text[pos + ARCHITECT_HANDOFF_PREFIX.len()..];
        if let Some(slice) = extract_object(after) {
            if let Some(h) = parse_findings_block(&slice) {
                return Some(h);
            }
        }
    }
    None
}

pub fn seam_for_new_file(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, _)) if !dir.is_empty() => dir.to_string(),
        _ => ".".into(),
    }
}

#[derive(Debug)]
pub enum ArchitectAssessment {
    Ok { advisories: Vec<String> },
    Fail { reasons: Vec<String> },
}

pub fn assess_architect_handoff(
    handoff: Option<&ArchitectHandoff>,
    exists_on_base_ref: Option<&dyn Fn(&str, &str) -> bool>,
) -> ArchitectAssessment {
    let Some(h) = handoff else {
        return ArchitectAssessment::Fail {
            reasons: vec![ARCHITECT_HANDOFF_MISSING.into()],
        };
    };
    let mut reasons = Vec::new();
    let mut advisories = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for f in &h.findings {
        if !ids.insert(&f.id) {
            reasons.push("Duplicate finding ids".into());
        }
    }
    if !h.findings.iter().any(|f| f.required) {
        reasons.push("No required findings. Adjacent-only discovery is a HOLD or a new story, not a silent empty plan.".into());
    }
    for f in &h.findings {
        let prefix = format!("{}: ", f.id);
        if f.scope.is_empty() {
            reasons.push(format!("{prefix}scope is empty"));
        }
        if f.scope.len() > MAX_SEAMS_PER_FINDING {
            reasons.push(format!(
                "{prefix}scope has {} seams; recut (max {MAX_SEAMS_PER_FINDING})",
                f.scope.len()
            ));
        }
        for seam in &f.scope {
            if file_of(seam).is_none() {
                reasons.push(format!("{prefix}illegal scope path {seam}"));
            }
        }
        if f.required && f.hint.as_deref() == Some("HOLD") && f.risks.is_empty() {
            reasons.push(format!("{prefix}required HOLD must name a concrete risk"));
        }
        if let Some(exists) = exists_on_base_ref {
            if !h.base_ref.is_empty() {
                for seam in &f.scope {
                    if let Some(path) = file_of(seam) {
                        if !exists(&h.base_ref, &path) {
                            reasons.push(format!(
                                "{prefix}scope {path} does not exist on {} — if this file is NEW, declare its directory instead: {}",
                                &h.base_ref[..h.base_ref.len().min(12)],
                                seam_for_new_file(&path)
                            ));
                        }
                    }
                }
            }
        }
    }
    if !h.base_ref.is_empty() {
        // ok
    } else if exists_on_base_ref.is_some() && h.findings.iter().any(|f| f.required) {
        reasons.push("baseRef is required when existence is enforced".into());
    } else {
        advisories.push("baseRef empty — existence check skipped".into());
    }
    if reasons.is_empty() {
        ArchitectAssessment::Ok { advisories }
    } else {
        ArchitectAssessment::Fail { reasons }
    }
}
