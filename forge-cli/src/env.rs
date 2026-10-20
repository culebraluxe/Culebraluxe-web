//! Repository-root and `.env.local` loading for the Forge operator CLI.
//!
//! `.env.local` is the repository's file, loaded by absolute path from the repository root rather
//! than the working directory. The app CLI keeps its own copy for app commands; this binary must not
//! depend on an app CLI module.

use std::path::PathBuf;

pub fn load_env() {
    let _ = dotenvy::from_path(repo_root().join(".env.local"));
}

pub fn repo_root() -> PathBuf {
    crate::forge::repo_root()
}
