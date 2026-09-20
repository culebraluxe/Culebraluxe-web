//! Port of workflow_app/forge/story-scope-base.ts. Pure; git reads injected.

fn normalize(value: Option<&str>) -> Option<String> {
    let sha = value.unwrap_or("").trim().to_ascii_lowercase();
    if sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(sha)
    } else {
        None
    }
}

pub fn story_scope_base(story_commits: &[Option<String>], read_parent: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut earliest = None;
    for c in story_commits {
        if let Some(sha) = normalize(c.as_deref()) {
            earliest = Some(sha);
        }
    }
    earliest.and_then(|sha| normalize(read_parent(&sha).as_deref()))
}

pub fn recorded_scope_base(recorded: &[Option<String>]) -> Option<String> {
    let mut earliest = None;
    for c in recorded {
        if let Some(sha) = normalize(c.as_deref()) {
            earliest = Some(sha);
        }
    }
    earliest
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateOwnChanges {
    Ok { changed_files: Vec<String> },
    Fail { reason: String },
}

pub fn candidate_own_changed_files(
    candidate_sha: Option<&str>,
    recorded_base: Option<&str>,
    lane_commits: &[String],
    read_changed_files: impl Fn(&str) -> Vec<String>,
    is_ancestor: impl Fn(&str, &str) -> bool,
) -> CandidateOwnChanges {
    let Some(candidate) = normalize(candidate_sha) else {
        return CandidateOwnChanges::Fail {
            reason: format!("candidate {:?} is not a commit", candidate_sha.unwrap_or("(none)")),
        };
    };
    let base = recorded_base.unwrap_or("").trim();
    if base.is_empty() {
        return CandidateOwnChanges::Fail {
            reason: format!("candidate {candidate} has no recorded base to measure against"),
        };
    }
    if !is_ancestor(base, &candidate) {
        return CandidateOwnChanges::Fail {
            reason: format!("candidate {candidate} is not a descendant of its recorded base {base}"),
        };
    }
    let mut commits = vec![candidate.clone()];
    for raw in lane_commits {
        if let Some(sha) = normalize(Some(raw)) {
            if !commits.contains(&sha) {
                commits.push(sha);
            }
        }
    }
    let mut changed = Vec::new();
    for sha in commits {
        for path in read_changed_files(&sha) {
            let clean = path.trim();
            if !clean.is_empty() && !changed.contains(&clean.to_string()) {
                changed.push(clean.to_string());
            }
        }
    }
    changed.sort();
    CandidateOwnChanges::Ok { changed_files: changed }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_base_is_earliest_newest_first() {
        let newer = Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
        let older = Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into());
        assert_eq!(recorded_scope_base(&[newer, older.clone()]), older);
    }
    #[test]
    fn foreign_history_fails_closed() {
        let r = candidate_own_changed_files(
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            &[],
            |_| vec!["src/x.rs".into()],
            |_, _| false,
        );
        match r {
            CandidateOwnChanges::Fail { reason } => assert!(reason.contains("not a descendant")),
            other => panic!("{other:?}"),
        }
    }
}
