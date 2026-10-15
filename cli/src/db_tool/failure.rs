//! The exit-code contract's failure type, moved out of `db_tool.rs` on 2026-10-14 to keep that file under
//! the boundedness ceiling. Move-only: the type, its `Display`/`Error` impls and the two `From` conversions
//! are text-exact, and the parent re-exports them, so `Failure` is still constructed as `Failure::Usage(…)`
//! from `db_tool`.

use std::io;

use super::*;

/// A failure that carries the exit code the scripts' contract gives it.
///
/// The retired scripts distinguished two kinds of failure, and CI still reads the difference: 0 applied or
/// clean, 1 refused or drift, 2 "cannot even start" — a missing connection URL, an absent ledger, bad
/// arguments. `check-schema-parity.ts` exited 2 exactly this way when `DATABASE_URL_DEV` /
/// `DATABASE_URL_PROD` were unset, and the distinction is worth keeping: a clean run and an unstartable run
/// must never look alike.
#[derive(Debug)]
pub enum Failure {
    /// Bad arguments: nothing was attempted.
    Usage(String),
    /// The environment or the database cannot support the run at all.
    Configuration(String),
    /// The work ran and failed: a refused apply, drift, a database error.
    Other(String),
}

impl Failure {
    pub fn exit_code(&self) -> u8 {
        match self {
            Failure::Usage(_) | Failure::Configuration(_) => 2,
            Failure::Other(_) => 1,
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Usage(message) | Failure::Configuration(message) | Failure::Other(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for Failure {}

impl From<db::DbFailure> for Failure {
    fn from(failure: db::DbFailure) -> Self {
        // Configuration is checked before any connection is attempted (`require_env`), so anything that
        // reaches here is work that started and failed. `DbFailure`'s own text already carries the
        // driver's message and the sqlstate, which is the half an operator can act on.
        Failure::Other(failure.to_string())
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Failure::Other(error.to_string())
    }
}
