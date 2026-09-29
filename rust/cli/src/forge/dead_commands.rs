//! THE DEAD-COMMAND SWEEP, in Rust — the `pnpm` menu measured against the retirement banner.
//!
//! Rust replacement for the retired `scripts/dead-command-sweep.mjs` (behind
//! `pnpm broken:ts:commands`). `forge ts-sweep` answers "which FILES cannot load"; this answers "which
//! COMMANDS cannot run", because the command menu is what an operator types, and a command naming a
//! bannered file fails with ERR_MODULE_NOT_FOUND before it does anything. No database, no network.
//!
//!   cargo run -p cli -- forge dead-commands              # the three blocks, and the count
//!   cargo run -p cli -- forge dead-commands --check      # fail when the count RISES
//!   cargo run -p cli -- forge dead-commands --format json
//!
//! The ledger those blocks feed is `docs/agent/DEAD-COMMANDS.md`. The count may only fall: porting a
//! command removes its name from here, and a new dead command is a finding, not a budget.
//!
//! TWO NOTES ON FAITHFULNESS.
//!
//! 1. The menu is read through `serde_json` into an `IndexMap`, so the order is `package.json`'s own.
//!    `serde_json`'s default map is sorted, and the order is part of the report the ledger mirrors —
//!    which is why the indexmap dependency is here rather than a sorted list.
//! 2. A refusal is PRINTED and returns exit code 1, rather than raised as a `Failure`: the sentence is
//!    quoted in `docs/agent/DEAD-COMMANDS.md`, and `Failure` reaches stderr through `main` with a
//!    `forge: ` prefix. Same exit contract, same words.

use crate::forge::{repo_root, Failure};
use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

/// The retirement banner, as the file head carries it.
const BANNER: &str = "BROKEN ON PURPOSE";

/// The total at the last sweep (`docs/agent/DEAD-COMMANDS.md`). Lower it when it falls.
///
/// **0 since 2026-09-29, and that is the end state this gate was built to reach**: the menu names no
/// bannered file at all. The road there was 53 → 25 → 19 (both lanes deleting), then 19 → 10 in the
/// TypeScript's own `--check` refusal while this port was being written, and 10 → 0 by the commits this
/// one landed on top of — so the number is measured here, on this tree, and not carried across.
///
/// It stays a gate rather than becoming a comment: `0` is the number a NEW dead command moves off, and
/// `--check` refuses either direction, so the next one is a finding and cannot be paid down silently.
const BASELINE: usize = 0;

/// Only the head of a bannered file is read: the banner is at the top by construction, and a file
/// that merely mentions it in prose further down is not bannered.
const HEAD_CHARS: usize = 600;

/// The three blocks: who owns the command, in the order they are worked.
pub const BLOCKS: [(&str, &str, &str); 3] = [
    (
        "forge",
        "Forge / story operator surface",
        r"^(?:forge:|story:|story-|sprint$|health$|test:story|test:sprint)",
    ),
    ("agent-runtime", "agent runtime", r"^agent:"),
    ("retired-stack", "retired server stack", r".*"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Dead,
    Live,
    Missing,
}

/// A `scripts/…` or `agent-runtime/…` file a command names, with its banner state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    pub file: String,
    pub state: State,
}

/// A command that names at least one file, and the ones that matter for its block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadCommand {
    pub name: String,
    pub files: Vec<String>,
}

fn referenced_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"((?:scripts|agent-runtime)/[A-Za-z0-9_./-]+\.(?:ts|mts|mjs|js))")
            .expect("the reference pattern is valid")
    })
}

fn block_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        BLOCKS
            .iter()
            .map(|(_, _, pattern)| Regex::new(pattern).expect("the block pattern is valid"))
            .collect()
    })
}

/// The block a command belongs to, by index into `BLOCKS`. The last block matches everything, so this
/// always lands somewhere.
pub fn block_of(name: &str) -> usize {
    block_res()
        .iter()
        .position(|pattern| pattern.is_match(name))
        .unwrap_or(BLOCKS.len() - 1)
}

/// The banner state of one command's file, read from the head only.
pub fn state_of(root: &Path, file: &str) -> State {
    let path = root.join(file);
    if !path.exists() {
        return State::Missing;
    }
    match fs::read_to_string(&path) {
        Ok(text) => {
            let head: String = text.chars().take(HEAD_CHARS).collect();
            if head.contains(BANNER) {
                State::Dead
            } else {
                State::Live
            }
        }
        Err(_) => State::Missing,
    }
}

/// Every `scripts/…` or `agent-runtime/…` file a command names, with its banner state.
pub fn referenced_files(root: &Path, command: &str) -> Vec<Reference> {
    referenced_re()
        .captures_iter(command)
        .map(|captures| {
            let file = captures
                .get(1)
                .map(|group| group.as_str())
                .unwrap_or_default()
                .to_string();
            let state = state_of(root, &file);
            Reference { file, state }
        })
        .collect()
}

#[derive(Deserialize)]
struct Package {
    /// `IndexMap`, not a map of any kind: the document's own order is part of the report.
    #[serde(default)]
    scripts: IndexMap<String, serde_json::Value>,
}

/// The menu, measured against the banner.
pub struct Menu {
    /// Every script in `package.json`, whether or not it names a file (the report's first line).
    pub scripts: usize,
    pub dead: Vec<DeadCommand>,
    pub missing: Vec<DeadCommand>,
}

impl Menu {
    pub fn load(root: &Path) -> Result<Self, Failure> {
        let path = root.join("package.json");
        let raw = fs::read_to_string(&path).map_err(|error| {
            Failure::configuration(format!("cannot read {}: {error}", path.display()))
        })?;
        let package: Package = serde_json::from_str(&raw).map_err(|error| {
            Failure::configuration(format!("cannot parse {}: {error}", path.display()))
        })?;

        let mut dead = Vec::new();
        let mut missing = Vec::new();
        for (name, value) in &package.scripts {
            let Some(command) = value.as_str() else {
                continue;
            };
            let files = referenced_files(root, command);
            if files.is_empty() {
                continue;
            }
            if files.iter().any(|entry| entry.state == State::Dead) {
                dead.push(DeadCommand {
                    name: name.clone(),
                    files: files
                        .iter()
                        .filter(|entry| entry.state == State::Dead)
                        .map(|entry| entry.file.clone())
                        .collect(),
                });
            } else if files.iter().any(|entry| entry.state == State::Missing) {
                missing.push(DeadCommand {
                    name: name.clone(),
                    files: files.iter().map(|entry| entry.file.clone()).collect(),
                });
            }
        }
        Ok(Self {
            scripts: package.scripts.len(),
            dead,
            missing,
        })
    }

    pub fn in_block(&self, index: usize) -> Vec<&DeadCommand> {
        self.dead
            .iter()
            .filter(|entry| block_of(&entry.name) == index)
            .collect()
    }
}

#[derive(Serialize)]
struct JsonCommand {
    command: String,
    files: Vec<String>,
}

#[derive(Serialize)]
struct JsonBlock {
    key: String,
    label: String,
    commands: Vec<JsonCommand>,
}

/// The shape the TypeScript's `JSON.stringify(…, null, 2)` produced, field for field.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonReport {
    total_scripts: usize,
    dead_commands: usize,
    baseline: usize,
    missing_file_commands: Vec<String>,
    blocks: Vec<JsonBlock>,
}

impl Menu {
    fn as_json(&self) -> JsonReport {
        JsonReport {
            total_scripts: self.scripts,
            dead_commands: self.dead.len(),
            baseline: BASELINE,
            missing_file_commands: self
                .missing
                .iter()
                .map(|entry| entry.name.clone())
                .collect(),
            blocks: BLOCKS
                .iter()
                .enumerate()
                .map(|(index, (key, label, _))| JsonBlock {
                    key: (*key).to_string(),
                    label: (*label).to_string(),
                    commands: self
                        .in_block(index)
                        .into_iter()
                        .map(|entry| JsonCommand {
                            command: entry.name.clone(),
                            files: entry.files.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

pub fn run(args: &[String]) -> Result<u8, Failure> {
    let mut check = false;
    let mut as_json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => {
                check = true;
                index += 1;
            }
            "--format" => {
                if args.get(index + 1).map(String::as_str) != Some("json") {
                    return Err(Failure::usage(
                        "usage: forge dead-commands [--check] [--format json]",
                    ));
                }
                as_json = true;
                index += 2;
            }
            "--format=json" => {
                as_json = true;
                index += 1;
            }
            other => {
                return Err(Failure::usage(format!(
                    "unknown argument `{other}`; usage: forge dead-commands [--check] [--format json]"
                )));
            }
        }
    }

    let menu = Menu::load(&repo_root())?;

    if as_json {
        let rendered = serde_json::to_string_pretty(&menu.as_json()).map_err(|error| {
            Failure::failed(format!("cannot render the report as JSON: {error}"))
        })?;
        println!("{rendered}");
    } else {
        println!("package.json scripts: {}", menu.scripts);
        println!(
            "commands naming a banner file: {} (baseline {BASELINE})",
            menu.dead.len()
        );
        for (index, (_, label, _)) in BLOCKS.iter().enumerate() {
            let commands = menu.in_block(index);
            println!("\n== {label} — {}", commands.len());
            for entry in commands {
                println!("  {}\t{}", entry.name, entry.files.join(" "));
            }
        }
        if !menu.missing.is_empty() {
            println!(
                "\n== commands naming a MISSING file (not a banner) — {}",
                menu.missing.len()
            );
            for entry in &menu.missing {
                println!("  {}\t{}", entry.name, entry.files.join(" "));
            }
        }
    }

    if !check {
        return Ok(0);
    }
    if menu.dead.len() > BASELINE {
        eprintln!(
            "\nREFUSING: {} commands name a banner file, up from {BASELINE}. A new one is a finding: \
             repoint it at Rust, or record why not in the same commit (docs/agent/DEAD-COMMANDS.md).",
            menu.dead.len()
        );
        return Ok(1);
    }
    if menu.dead.len() < BASELINE {
        eprintln!(
            "\nThe count FELL to {}, below the baseline of {BASELINE}. Lower BASELINE in this file and \
             the count in docs/agent/DEAD-COMMANDS.md.",
            menu.dead.len()
        );
        return Ok(1);
    }
    println!("\nok: {} dead commands, at the baseline.", menu.dead.len());
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Scratch inside the test, removed inside the test: nothing outlives the run.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("culebraluxe-dead-commands-{name}"));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("scratch directory");
            Self(dir)
        }

        fn write(&self, relative: &str, body: &str) {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("scratch parent");
            }
            fs::write(&path, body).expect("scratch file");
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_blocks_are_the_three_the_ledger_documents() {
        assert_eq!(BLOCKS.len(), 3);
        assert_eq!(block_of("forge:board"), 0);
        assert_eq!(block_of("story:status"), 0);
        assert_eq!(block_of("test:sprint-fences"), 0);
        assert_eq!(block_of("agent:work"), 1);
        assert_eq!(block_of("build"), 2);
        assert_eq!(
            block_of("broken:ts:sweep"),
            2,
            "the catch-all owns everything else"
        );
    }

    #[test]
    fn a_banner_is_read_from_the_head_only() {
        let scratch = Scratch::new("head");
        scratch.write("scripts/early.ts", &format!("{BANNER}\n"));
        scratch.write(
            "scripts/late.ts",
            &format!("{}\n{BANNER}\n", "x".repeat(HEAD_CHARS)),
        );

        assert_eq!(state_of(scratch.path(), "scripts/early.ts"), State::Dead);
        assert_eq!(
            state_of(scratch.path(), "scripts/late.ts"),
            State::Live,
            "the banner is at the top by construction, so prose further down is not a banner"
        );
        assert_eq!(state_of(scratch.path(), "scripts/gone.ts"), State::Missing);
    }

    #[test]
    fn only_scripts_or_agent_runtime_files_are_read_out_of_a_command() {
        let root = Path::new(".");
        assert!(referenced_files(root, "cargo test --workspace --all-targets").is_empty());
        let files = referenced_files(root, "node scripts/a.mjs agent-runtime/b.ts");
        assert_eq!(files.len(), 2, "{files:?}");
        assert_eq!(files[0].file, "scripts/a.mjs");
        assert_eq!(files[1].file, "agent-runtime/b.ts");
    }

    #[test]
    fn the_menu_is_read_in_the_documents_own_order() {
        let scratch = Scratch::new("order");
        scratch.write("scripts/one.ts", &format!("{BANNER}\n"));
        scratch.write(
            "package.json",
            r#"{ "scripts": { "zzz": "node scripts/one.ts", "aaa": "node scripts/one.ts" } }"#,
        );

        let menu = Menu::load(scratch.path()).expect("the scratch menu loads");

        // A sorted map would have said `aaa` first; the ledger's row order mirrors this list.
        assert_eq!(menu.scripts, 2);
        let names: Vec<&str> = menu.dead.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, vec!["zzz", "aaa"]);
    }

    #[test]
    fn every_dead_command_names_a_file_that_really_is_bannered() {
        let root = repo_root();
        let menu = Menu::load(&root).expect("the repository menu loads");

        for entry in &menu.dead {
            assert!(!entry.files.is_empty(), "{} names no file", entry.name);
            for file in &entry.files {
                assert_eq!(
                    state_of(&root, file),
                    State::Dead,
                    "{} names {file}",
                    entry.name
                );
            }
        }
        let per_block: usize = (0..BLOCKS.len())
            .map(|index| menu.in_block(index).len())
            .sum();
        assert_eq!(
            per_block,
            menu.dead.len(),
            "every command lands in exactly one block"
        );
    }

    #[test]
    fn the_json_report_keeps_the_names_and_the_shape() {
        let menu = Menu::load(&repo_root()).expect("the repository menu loads");
        let value = serde_json::to_value(menu.as_json()).expect("the report serialises");

        assert_eq!(value["baseline"], BASELINE);
        assert_eq!(value["deadCommands"], menu.dead.len());
        let blocks = value["blocks"].as_array().expect("blocks is an array");
        assert_eq!(blocks.len(), BLOCKS.len());
        assert_eq!(blocks[0]["key"], "forge");
        let listed: usize = blocks
            .iter()
            .map(|block| block["commands"].as_array().expect("commands").len())
            .sum();
        assert_eq!(listed, menu.dead.len());
    }
}
