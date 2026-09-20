//! Port of smith-candidate-parse.ts. Git is the cabinet; this only parses a marker.

pub const SMITH_CANDIDATE_PREFIX: &str = "SMITH_CANDIDATE:";
pub const SMITH_CANDIDATE_MISSING: &str =
    "SMITH: no candidate. The runner diffs worktree HEAD against merge base; a SMITH_CANDIDATE chat line is not a candidate.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmithCandidate {
    pub assignment_id: String,
    pub candidate_sha: String,
    pub merge_base: String,
    pub changed_paths: Vec<String>,
}

fn is_sha(s: &str) -> bool {
    let v = s.trim();
    (7..=40).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn parse_smith_candidate_line(notes: &str) -> Option<SmithCandidate> {
    let lines: Vec<&str> = notes
        .lines()
        .map(str::trim)
        .filter(|s| s.starts_with(SMITH_CANDIDATE_PREFIX))
        .collect();
    if lines.len() != 1 {
        return None;
    }
    let raw = lines[0][SMITH_CANDIDATE_PREFIX.len()..].trim();
    // Minimal JSON field scrape without serde.
    let assignment = extract(raw, "assignmentId")?;
    let sha = extract(raw, "candidateSha")?;
    let base = extract(raw, "mergeBase")?;
    if !is_sha(&sha) || !is_sha(&base) {
        return None;
    }
    Some(SmithCandidate {
        assignment_id: assignment,
        candidate_sha: sha.to_ascii_lowercase(),
        merge_base: base.to_ascii_lowercase(),
        changed_paths: vec![],
    })
}

fn extract(raw: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = raw.find(&pat)?;
    let rest = &raw[i + pat.len()..];
    let rest = rest.trim_start().trim_start_matches(':').trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_marker() {
        let notes = r#"SMITH_CANDIDATE: {"version":1,"assignmentId":"a1","candidateSha":"aaaaaaaa","mergeBase":"bbbbbbbb","changedPaths":["x"]}"#;
        let c = parse_smith_candidate_line(notes).unwrap();
        assert_eq!(c.assignment_id, "a1");
    }
}
