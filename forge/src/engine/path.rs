//! Port of `workflow_app/forge/agents/shared/path.ts`.

pub fn file_of(path: &str) -> Option<String> {
    let t = path.trim().trim_start_matches("./");
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

pub fn within(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

pub fn overlap(a: &str, b: &str) -> bool {
    within(a, b)
}

pub fn shared_path(a: &[String], b: &[String]) -> Option<String> {
    for left in a {
        let Some(l) = file_of(left) else { continue };
        for right in b {
            let Some(r) = file_of(right) else { continue };
            if overlap(&l, &r) {
                return Some(if within(&l, &r) { l } else { r });
            }
        }
    }
    None
}
