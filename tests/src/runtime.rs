//! The runtime-environment boundary, executable against a controlled process environment.
//!
//! WHY THIS EXISTS. The Forge runtime reads the process environment directly (`std::env::var`) because that is how a
//! real engine process learns what it is: which environment it may run against (`execution_target`) and, on top of
//! that, whether it even has a production database to write story state to (`db_writer::DbForgeStateWriter`). Those
//! reads are the production-owned boundary; a contract test for them must present an exact environment and then call
//! the production function, not a re-declared copy of its logic.
//!
//! This harness is the smallest honest seam. It does not implement any resolution itself: it takes the process
//! environment lock, lets a test override or remove the handful of variables the boundary reads, and restores the
//! machine's own values when the scope is dropped — even on a panic. It performs no external I/O and never opens a
//! connection; the boundaries under test refuse before any socket is reached.
//!
//! Level: L0 Pure — process-local state only. No database, no network, no process spawned.

use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// One lock for the whole process, so two scopes in one test binary can never interleave their overrides.
static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn env_lock() -> &'static Mutex<()> {
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

/// A scoped, serialised override of the process environment, restored on drop.
pub struct RuntimeHarness {
    _lock: MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl RuntimeHarness {
    /// Take the environment lock and begin a scope in which variables may be overridden.
    ///
    /// The scope is exclusive for its lifetime; the returned harness restores every variable it touched when it is
    /// dropped, so a test leaves the machine's environment exactly as it found it.
    pub fn acquire() -> Self {
        let lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self {
            _lock: lock,
            saved: Vec::new(),
        }
    }

    /// Set `key` to `value`, remembering the machine's own value the first time the key is touched.
    pub fn set(&mut self, key: &'static str, value: &str) {
        self.remember(key);
        std::env::set_var(key, value);
    }

    /// Remove `key`, remembering the machine's own value the first time the key is touched.
    pub fn remove(&mut self, key: &'static str) {
        self.remember(key);
        std::env::remove_var(key);
    }

    fn remember(&mut self, key: &'static str) {
        if self.saved.iter().any(|(saved, _)| *saved == key) {
            return;
        }
        self.saved.push((key, std::env::var_os(key)));
    }
}

impl Drop for RuntimeHarness {
    fn drop(&mut self) {
        for (key, original) in self.saved.drain(..) {
            match original {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
