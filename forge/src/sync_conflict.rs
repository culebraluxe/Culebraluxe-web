//! CONFLICT COPIES — the debris a cloud-sync client leaves when it reconciles a file that changed while
//! it was uploading: `routes.d 2.ts`, `package 3.json`, `cache-life.d 2.ts`.
//!
//! Ported from the retired `lib/git/sync-conflict.ts`, deleted with the TypeScript application in
//! `4cf98110`. It lives in `forge` rather than in the CLI because two callers need ONE definition: the
//! scope-manifest generator (which must never index debris into a generated page) and, when it is
//! ported, the `health --fix` cleanup that removes them. A second copy of this rule is how a file a
//! person wrote gets deleted.
//!
//! MEASURED 2026-09-18 on this machine: 554 of them inside `.next` after one build, and one in the
//! repository itself (`contact-export 2.json`). The user believed iCloud was syncing this folder; it is
//! not iCloud. `~/Library/CloudStorage/OneDrive-Personal/` contains `Documents` AND `Desktop`, which is
//! the signature of **OneDrive Known Folder Move** — OneDrive took over `~/Documents`, so every file
//! `pnpm build` and git itself write gets uploaded, and anything that changes mid-upload comes back as
//! a numbered sibling.
//!
//! THE NAME ALONE IS NOT ENOUGH TO DELETE SOMETHING, which is why the two functions are separate. A name
//! pattern of "space + number + extension" also matches `CHANGELOG 2026.md` and `Budget 2026.xlsx`, and
//! the first version of this rule would have let `pnpm health --fix` delete a person's file — a fence
//! caught exactly that case (2026-09-18). Two things make a numbered sibling debris rather than a
//! document: the counter is SMALL (a sync client counts from 2, a year is four digits), and **the
//! original sits beside it in the same directory**. Both are required.
//!
//! Residual risk, stated rather than hidden: `Report.md` next to `Report 2.md`, both written by a person,
//! would still be read as debris. It is rare, `--fix` is deliberate, and the tool prints every path it
//! removes — but it is why `--fix` is not the default.

/// The name this would be a copy OF, or `None` when it is not a numbered sibling.
///
/// `^(.+) [0-9]{1,2}(\.[A-Za-z0-9]+)$` without a regex crate: the extension is the last `.`, the counter
/// is one or two digits before it, and there is a space before the counter.
pub fn conflict_copy_base_name(name: &str) -> Option<String> {
    let dot = name.rfind('.')?;
    let (stem, extension) = name.split_at(dot);
    let extension = &extension[1..];
    if extension.is_empty() || !extension.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let (base, counter) = stem.rsplit_once(' ')?;
    if base.is_empty() || counter.is_empty() || counter.len() > 2 {
        return None;
    }
    if !counter.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{base}.{extension}"))
}

/// Candidate by name only: a SMALL counter, so a four-digit year is never one. Presence of the original
/// is the second test and lives in [`conflict_copies_in_directory`] — never decided by the name alone.
pub fn is_conflict_copy_name(name: &str) -> bool {
    conflict_copy_base_name(name).is_some()
}

/// Names in ONE directory that are debris: a small counter AND the original present beside them.
pub fn conflict_copies_in_directory(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = names
        .iter()
        .filter(|name| {
            conflict_copy_base_name(name)
                .map(|base| names.iter().any(|other| other == &base))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    out.sort();
    out
}

/// Paths from a scan that are debris, judged per directory (never across directories).
pub fn conflict_copies(paths: &[String]) -> Vec<String> {
    let mut directories: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for path in paths {
        let (directory, name) = match path.rsplit_once('/') {
            Some((directory, name)) => (directory.to_string(), name.to_string()),
            None => (String::new(), path.clone()),
        };
        directories.entry(directory).or_default().push(name);
    }
    let mut out: Vec<String> = Vec::new();
    for (directory, names) in directories {
        for name in conflict_copies_in_directory(&names) {
            out.push(if directory.is_empty() {
                name
            } else {
                format!("{directory}/{name}")
            });
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_counter_before_the_extension_is_a_sibling() {
        assert_eq!(
            conflict_copy_base_name("cache-life.d 2.ts").as_deref(),
            Some("cache-life.d.ts")
        );
        assert_eq!(
            conflict_copy_base_name("RUNLOG 2.md").as_deref(),
            Some("RUNLOG.md")
        );
        assert_eq!(
            conflict_copy_base_name("package 3.json").as_deref(),
            Some("package.json")
        );
    }

    #[test]
    fn a_year_is_a_document_not_debris() {
        // The measurement that produced the rule: `CHANGELOG 2026.md` and `Budget 2026.xlsx` are files a
        // person wrote, and the first version of this rule would have deleted them.
        assert_eq!(conflict_copy_base_name("CHANGELOG 2026.md"), None);
        assert_eq!(conflict_copy_base_name("Budget 2026.xlsx"), None);
        assert!(!is_conflict_copy_name("ORIENTATION.md"));
    }

    #[test]
    fn the_original_must_sit_beside_it() {
        let orphan = vec!["RUNLOG 2.md".to_string(), "other.md".to_string()];
        assert!(conflict_copies_in_directory(&orphan).is_empty());
        let both = vec!["RUNLOG 2.md".to_string(), "RUNLOG.md".to_string()];
        assert_eq!(conflict_copies_in_directory(&both), vec!["RUNLOG 2.md"]);
    }

    #[test]
    fn debris_is_judged_per_directory_and_reported_as_paths() {
        let paths = vec![
            "docs/a.md".to_string(),
            "docs/notes 2.md".to_string(),
            "docs/notes.md".to_string(),
        ];
        assert_eq!(conflict_copies(&paths), vec!["docs/notes 2.md"]);
    }
}
