//! Port of migration-applied-guard.ts. Ledger is injected.

pub fn migration_ledger_filename(path: &str) -> Option<String> {
    let n = path.trim().replace('\\', "/");
    let n = n.strip_prefix("./").unwrap_or(&n);
    let ok = n.to_ascii_lowercase().starts_with("db/migrations/")
        && n.to_ascii_lowercase().ends_with(".sql")
        && !n[14..].contains('/');
    if ok {
        Some(n.to_string())
    } else {
        None
    }
}

pub fn assess_migration_applied(changed_paths: &[String], ledger: &[String]) -> Vec<String> {
    let files: Vec<String> = changed_paths
        .iter()
        .filter_map(|p| migration_ledger_filename(p))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let ledger: std::collections::BTreeSet<_> = ledger
        .iter()
        .filter_map(|f| migration_ledger_filename(f))
        .collect();
    let mut unapplied = Vec::new();
    for f in files {
        if !seen.insert(f.clone()) {
            continue;
        }
        if !ledger.contains(&f) {
            unapplied.push(f);
        }
    }
    unapplied
}

pub fn migration_applied_refusal(unapplied: &[String]) -> String {
    format!(
        "story change set adds migration(s) not recorded in the PROD schema_migration ledger: {}",
        unapplied.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_ledger_row_is_unapplied() {
        let u = assess_migration_applied(
            &["db/migrations/200.sql".into(), "src/x.rs".into()],
            &["db/migrations/199.sql".into()],
        );
        assert_eq!(u, vec!["db/migrations/200.sql"]);
    }
}
