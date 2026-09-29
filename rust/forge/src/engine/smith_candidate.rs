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
        changed_paths: extract_array(raw, "changedPaths")?,
    })
}

/// The candidate's changed-path set, preserved from the marker.
///
/// `legacy/workflow_app/forge/smith-candidate.parse.test.ts` asserts this survives the parse, and
/// `migration_guard::assess_migration_applied` takes exactly this list — hard-coding `vec![]` here
/// handed the migration guard an empty change set, which is a guard that can never fire.
fn extract_array(raw: &str, key: &str) -> Option<Vec<String>> {
    let pat = format!("\"{key}\"");
    let i = raw.find(&pat)?;
    let rest = &raw[i + pat.len()..];
    let rest = rest.trim_start().trim_start_matches(':').trim_start();
    if !rest.starts_with('[') {
        return None;
    }
    let end = rest.find(']')?;
    let inner = rest[1..end].trim();
    if inner.is_empty() {
        return Some(vec![]);
    }
    let mut out = Vec::new();
    for part in inner.split(',') {
        let item = part.trim();
        let body = item
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .or_else(|| item.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))?;
        out.push(body.to_string());
    }
    Some(out)
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
    //! Ported from `legacy/workflow_app/forge/smith-candidate.parse.test.ts`.
    //! The `changedPaths` assertion is why the parser no longer hard-codes an empty list.
    use super::*;

    #[test]
    fn parses_one_smith_candidate_line() {
        let notes = format!(
            r#"{} {{"version":1,"assignmentId":"a1","candidateSha":"aaaaaaaa","mergeBase":"bbbbbbbb","changedPaths":["legacy/workflow_app/forge/a.ts"]}}"#,
            SMITH_CANDIDATE_PREFIX
        );
        let c = parse_smith_candidate_line(&notes).expect("one well-formed marker parses");
        assert_eq!(c.assignment_id, "a1");
        assert_eq!(c.candidate_sha, "aaaaaaaa");
        assert_eq!(
            c.changed_paths,
            vec!["legacy/workflow_app/forge/a.ts".to_string()]
        );
    }

    #[test]
    fn duplicate_or_broken_lines_fail_closed() {
        let line = format!(
            r#"{} {{"version":1,"assignmentId":"a1","candidateSha":"aaaaaaaa","mergeBase":"bbbbbbbb","changedPaths":["a.ts"]}}"#,
            SMITH_CANDIDATE_PREFIX
        );
        assert!(parse_smith_candidate_line(&format!("{line}\n{line}")).is_none());
        assert!(parse_smith_candidate_line("SMITH_CANDIDATE: {broken").is_none());
        assert!(parse_smith_candidate_line("no marker").is_none());
        assert!(SMITH_CANDIDATE_MISSING.contains("SMITH_CANDIDATE"));
    }

    #[test]
    fn parses_marker() {
        let notes = r#"SMITH_CANDIDATE: {"version":1,"assignmentId":"a1","candidateSha":"aaaaaaaa","mergeBase":"bbbbbbbb","changedPaths":["x"]}"#;
        let c = parse_smith_candidate_line(notes).unwrap();
        assert_eq!(c.assignment_id, "a1");
    }
}
