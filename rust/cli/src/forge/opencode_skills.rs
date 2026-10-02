//! forge:opencode-skills — generate the vendor-loadable skill tree from the canonical skill library.
//!
//!   cargo run -p cli -- forge opencode-skills           # write the tree (untouched files are left alone)
//!   cargo run -p cli -- forge opencode-skills --check   # exit 1 when a skill is missing, drifted or stale
//!
//! The tree under `.opencode/skills/<id>/SKILL.md` is GENERATED, not authored. Its one source is the canonical
//! library in `docs/agent/skills/<id>.md` — the copy AGENTS.md points at and a packet's `## Skills` list names —
//! and the renderer is `forge::engine::opencode_agents::render_skill_tree`, the same module that decides which
//! skills a role may load. So a `skill` grant in the V2 config and a file the vendor can actually read cannot
//! disagree without this command failing.
//!
//! It matters because the measured failure is silent: the live 2.x build answered `Unable to load skill planner`
//! for a granted skill that existed only under `docs/`, and answered a `skill rust-testing` request with the
//! skill's own body once `.opencode/skills/rust-testing/SKILL.md` existed. Nothing errored at the config level in
//! either case; the grant simply did nothing. Drift, a missing file and a directory the catalogue no longer names
//! are all refused for that reason — a stale directory is still loadable by the vendor, so it is a skill the
//! config does not govern.
//!
//! Unlike the vendor pointer files in `forge:sync-agents`, the generated files here ARE created when absent:
//! there is no vendor-owned document to preserve, and an absent one is a grant that loads nothing.

use crate::forge::{repo_root, Failure};
use forge::engine::opencode_agents;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// The vendor skill root, repo-relative. The renderer owns the path; this is the same constant it publishes.
pub const SKILL_ROOT: &str = opencode_agents::VENDOR_SKILL_DIR;

/// The library's own index, which is prose about the skills rather than a skill.
const LIBRARY_INDEX: &str = "README";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Catalogued and granted, but absent from the vendor tree: the grant loads nothing.
    Missing,
    /// In the tree, but not what the renderer produces: a hand edit, or a canonical source that moved on.
    Drifted,
    /// In the tree and absent from the catalogue: the vendor would still load it, and no config mentions it.
    Stale,
}

impl Change {
    fn as_str(&self) -> &'static str {
        match self {
            Change::Missing => "missing",
            Change::Drifted => "drifted",
            Change::Stale => "stale",
        }
    }
}

/// Pure: the tree on disk and the rendered tree go in, what to do about them comes out.
///
/// Sorted by path so the report is stable and a diff of two runs stays readable.
pub fn plan_tree(
    on_disk: &BTreeMap<String, String>,
    rendered: &BTreeMap<String, String>,
) -> Vec<(Change, String)> {
    let mut changes: Vec<(Change, String)> = Vec::new();
    for (path, expected) in rendered {
        match on_disk.get(path) {
            None => changes.push((Change::Missing, path.clone())),
            Some(actual) if actual != expected => changes.push((Change::Drifted, path.clone())),
            Some(_) => {}
        }
    }
    for path in on_disk.keys() {
        if !rendered.contains_key(path) {
            changes.push((Change::Stale, path.clone()));
        }
    }
    changes.sort_by(|left, right| left.1.cmp(&right.1));
    changes
}

/// The rendered tree, or the reason it cannot be rendered. Reading the prose is the only impure part: the bytes
/// come from the shared renderer, which is pure given the bodies.
fn render(root: &Path) -> Result<BTreeMap<String, String>, Failure> {
    let mut bodies: BTreeMap<String, String> = BTreeMap::new();
    for id in opencode_agents::all_skill_ids() {
        let relative = opencode_agents::canonical_skill_path(id);
        let body = fs::read_to_string(root.join(&relative)).map_err(|error| {
            Failure::failed(format!(
                "the canonical prose for '{id}' is unreadable at {relative}: {error}"
            ))
        })?;
        bodies.insert(id.to_string(), body);
    }
    let rendered = opencode_agents::render_skill_tree(&bodies)
        .map_err(|error| Failure::failed(format!("the skill tree does not render: {error}")))?;
    Ok(rendered.into_iter().collect())
}

/// A canonical file the catalogue does not name. The library is the renderer's INPUT, so a file with no id is a
/// skill no role can ever be granted — named here rather than left sitting in the library unnoticed.
fn orphans(root: &Path) -> Vec<String> {
    let known = opencode_agents::all_skill_ids();
    let mut orphans: Vec<String> = Vec::new();
    let Ok(entries) = fs::read_dir(root.join(opencode_agents::CANONICAL_SKILL_DIR)) else {
        return orphans;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Some(id) = name.strip_suffix(".md") else {
            continue;
        };
        if id == LIBRARY_INDEX || known.contains(&id) {
            continue;
        }
        orphans.push(name);
    }
    orphans.sort();
    orphans
}

/// The tree as it is, keyed by the same repo-relative paths the renderer produces.
///
/// Only directories are read: a loose file at the skills root is not a skill the vendor can address by name, and
/// the renderer never writes one.
fn read_tree(root: &Path) -> BTreeMap<String, String> {
    let mut on_disk: BTreeMap<String, String> = BTreeMap::new();
    let Ok(entries) = fs::read_dir(root.join(SKILL_ROOT)) else {
        return on_disk;
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    ids.sort();
    for id in ids {
        let relative = opencode_agents::vendor_skill_path(&id);
        if let Ok(content) = fs::read_to_string(root.join(&relative)) {
            on_disk.insert(relative, content);
        }
    }
    on_disk
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let check = args.iter().any(|arg| arg == "--check");
    let json = args.iter().any(|arg| arg == "--format")
        && args
            .iter()
            .position(|arg| arg == "--format")
            .and_then(|at| args.get(at + 1))
            .map(String::as_str)
            == Some("json");

    let root = repo_root();
    let orphans = orphans(&root);
    if !orphans.is_empty() {
        return Err(Failure::failed(format!(
            "{} holds skill files the catalogue does not name: {orphans:?} — add the id to \
             `opencode_agents` or remove the file",
            opencode_agents::CANONICAL_SKILL_DIR
        )));
    }

    let rendered = render(&root)?;
    let on_disk = read_tree(&root);
    let changes = plan_tree(&on_disk, &rendered);
    let count = rendered.len();

    if !check {
        for (change, relative) in changes.iter() {
            let target = root.join(relative);
            match change {
                Change::Stale => {
                    // Both the file and its now-empty directory: the vendor addresses a skill BY directory, so
                    // leaving the directory behind would leave the skill loadable.
                    let _ = fs::remove_file(&target);
                    if let Some(parent) = target.parent() {
                        let _ = fs::remove_dir(parent);
                    }
                }
                Change::Missing | Change::Drifted => {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent).map_err(|error| {
                            Failure::failed(format!(
                                "{} could not be created: {error}",
                                parent.display()
                            ))
                        })?;
                    }
                    let content = rendered
                        .get(relative)
                        .expect("every non-stale change names a rendered path");
                    fs::write(&target, content).map_err(|error| {
                        Failure::failed(format!("{relative} could not be written: {error}"))
                    })?;
                }
            }
        }
    }

    let refused = check && !changes.is_empty();
    // Two vocabularies on purpose: `--check` reports the FAULT it found, a write reports the ACTION it took.
    let lines: Vec<String> = changes
        .iter()
        .map(|(change, relative)| match (check, change) {
            (true, Change::Missing) => format!("MISSING {relative}"),
            (true, Change::Drifted) => format!("DRIFTED {relative}"),
            (true, Change::Stale) => format!("STALE   {relative}"),
            (false, Change::Stale) => format!("removed {relative}"),
            (false, _) => format!("wrote   {relative}"),
        })
        .collect();

    if json {
        let records: Vec<serde_json::Value> = changes
            .iter()
            .map(|(change, relative)| {
                serde_json::json!({ "kind": change.as_str(), "path": relative })
            })
            .collect();
        let payload = serde_json::json!({
            "mode": if check { "check" } else { "write" },
            "root": SKILL_ROOT,
            "status": if refused {
                "drifted"
            } else if changes.is_empty() {
                "unchanged"
            } else {
                "wrote"
            },
            "skills": count,
            "changes": records,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
        );
        return Ok(u8::from(refused));
    }

    if changes.is_empty() {
        println!("ok      {SKILL_ROOT} matches the canonical library ({count} skills)");
        return Ok(0);
    }
    for line in &lines {
        println!("{line}");
    }
    if refused {
        println!(
            "FAIL    {} skill file(s) out of step with `{}`: a granted skill the vendor cannot load does \
             nothing, and nothing else in the gate says so",
            changes.len(),
            opencode_agents::CANONICAL_SKILL_DIR
        );
        println!("  run `pnpm forge:opencode-skills` to regenerate.");
    } else {
        println!("wrote   {SKILL_ROOT} ({count} skills)");
    }
    Ok(u8::from(refused))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(path, body)| (path.to_string(), body.to_string()))
            .collect()
    }

    #[test]
    fn a_tree_that_matches_the_renderer_is_left_alone() {
        let rendered = tree(&[(".opencode/skills/ui/SKILL.md", "rendered\n")]);
        assert!(plan_tree(&rendered, &rendered).is_empty());
    }

    #[test]
    fn a_grant_with_no_file_is_missing_rather_than_tolerated() {
        let rendered = tree(&[(".opencode/skills/ui/SKILL.md", "rendered\n")]);
        assert_eq!(
            plan_tree(&BTreeMap::new(), &rendered),
            vec![(Change::Missing, ".opencode/skills/ui/SKILL.md".into())]
        );
    }

    #[test]
    fn a_hand_edited_skill_is_drift_and_the_renderer_wins() {
        let rendered = tree(&[(".opencode/skills/ui/SKILL.md", "rendered\n")]);
        let edited = tree(&[(".opencode/skills/ui/SKILL.md", "edited\n")]);
        assert_eq!(
            plan_tree(&edited, &rendered),
            vec![(Change::Drifted, ".opencode/skills/ui/SKILL.md".into())]
        );
    }

    #[test]
    fn a_directory_the_catalogue_does_not_name_is_stale_because_the_vendor_would_still_load_it() {
        let rendered = tree(&[(".opencode/skills/ui/SKILL.md", "rendered\n")]);
        let mut on_disk = rendered.clone();
        on_disk.insert(".opencode/skills/ghost/SKILL.md".into(), "body\n".into());
        assert_eq!(
            plan_tree(&on_disk, &rendered),
            vec![(Change::Stale, ".opencode/skills/ghost/SKILL.md".into())]
        );
    }

    /// The tree this command publishes: every catalogued skill renders, the path shape is the vendor's, and the
    /// frontmatter is first so the vendor reads a name and a description rather than prose.
    #[test]
    fn the_rendered_tree_covers_the_catalogue_and_opens_with_frontmatter() {
        let mut bodies: BTreeMap<String, String> = BTreeMap::new();
        for id in opencode_agents::all_skill_ids() {
            bodies.insert(id.to_string(), format!("# Skill: {id}\n\nbody of {id}\n"));
        }
        let rendered = opencode_agents::render_skill_tree(&bodies).expect("render");
        assert_eq!(rendered.len(), opencode_agents::all_skill_ids().len());
        for (path, content) in &rendered {
            let id = path
                .strip_prefix(&format!("{SKILL_ROOT}/"))
                .and_then(|rest| rest.strip_suffix("/SKILL.md"))
                .unwrap_or_else(|| panic!("the vendor tree shape: {path}"));
            assert_eq!(path, &opencode_agents::vendor_skill_path(id), "name == dir");
            assert!(
                content.starts_with(&format!("---\nname: {id}\n")),
                "{path} must open with frontmatter naming its directory"
            );
            assert!(
                content.contains("\ndescription: ") && !content.is_empty(),
                "{} has no description for the model to select on",
                opencode_agents::canonical_skill_path(id)
            );
        }
    }
}
