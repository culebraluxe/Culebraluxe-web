//! Was the failure the engine's own plumbing, or the work?

use workflow::WorkflowError;

/// The same question, asked of a typed error first.
///
/// `WorkflowError::Unavailable` is the database seam saying the CONNECTION failed rather than the statement
/// (`is_connection_failure`), so that answer needs no reading at all. The message half is [`is_engine_fault`]'s
/// vocabulary, which exists because a turn can fail deep inside a writer or a socket whose error reaches a caller
/// already flattened to a string — the flattened form is what `bin/forge.rs` classifies a run's exit with, and it
/// is the same question. Kept here, once, so the job layer and the exit path cannot answer it differently.
pub fn is_engine_fault_error(error: &WorkflowError) -> bool {
    error.is_connection_failure() || is_engine_fault(&error.to_string())
}

/// The one question the run's exit path has to answer (captain, 2026-09-29): a failure that says nothing about the
/// story is the engine's, and it is cleared back into the queue rather than recorded against the story.
///
/// The vocabulary is taken from the layers below, as they already print it: the database seam's own label
/// (`DatabaseUnavailable`), the sqlstate the server sent (`25P03` idle-in-transaction, which is how a session the
/// engine held too long is taken away, `57P02`, `53300`), and the shape of a socket whose peer is gone. Nothing here
/// decides a story verdict — that is the board's and `settlement_pair`'s — so an unrecognised failure stays a
/// failure, which is the safe direction.
pub fn is_engine_fault(message: &str) -> bool {
    const MARKS: &[&str] = &[
        "databaseunavailable",
        // Failed ownership proof is retryable engine authority, never a story verdict. The settlement DAO still
        // fences this requeue against the current owner/generation before changing durable job state.
        "authority is unproven",
        "sqlstate 25p03",
        "sqlstate 57p02",
        "sqlstate 53300",
        "idle-in-transaction",
        "terminating connection",
        "broken pipe",
        "connection reset",
        "connection closed",
        "unexpected eof",
        "pool timed out",
        // The transport's own timeouts, by name. A bare "timed out" used to be here, and it matched any role error
        // that quoted one — a test that timed out, a vendor's message — turning a paid verdict into a free retry.
        "connection timed out",
        "operation timed out",
        "statement timeout",
        // The vendor's own plumbing, which says nothing about the story: its local SQLite store contended by a
        // concurrent run, and the provider account out of credit. Recorded against the story, an empty account held
        // every story the worker claimed (production 2026-10-01: 175 runs in one outage).
        "database is locked",
        "insufficient balance",
        // The vendor server failing, or a vendor that cannot be run as Forge needs it (C1, 2026-10-03: the architect
        // turn died on OpenCode's own HTTP 500 with zero tokens spent and was ruled the story's failure). A 5xx is the
        // server's fault, never the request's; a 4xx stays the work's.
        "unexpectedstatus: 5",
        "connection refused",
        "could not be run at all",
        "is not the opencode build",
    ];
    let message = message.to_ascii_lowercase();
    MARKS.iter().any(|mark| message.contains(mark))
}

#[cfg(test)]
mod tests {
    use super::is_engine_fault;

    /// The failures that must clear the claim: the session was taken away, the transport died, a statement was cut
    /// off, or the launch never happened.
    #[test]
    fn plumbing_failures_are_the_engines() {
        for message in [
            "DatabaseUnavailable during workflow.step (incident 5e575d72, sqlstate 25P03)",
            "Forge job job-1 authority is unproven after role execution; outcome preserved for reconciliation",
            "db: sqlstate 25P03 idle_in_transaction_session_timeout",
            "db error: sqlstate 53300 too many connections",
            "sqlstate 57P02 terminating connection due to crash of another server process",
            "io error: Broken pipe (os error 32)",
            "io error: Connection reset by peer (os error 54)",
            "unexpected EOF while reading message",
            "pool timed out while waiting for an open connection",
            "io error: Operation timed out (os error 60)",
            "opencode-harness failed for fast_smith exit=Some(1): Error: Unexpected error\n\ndatabase is locked",
            "opencode-harness failed for fast_smith exit=Some(1): Error: Insufficient Balance (request_id: abc)",
            "error connecting to server: Connection timed out (os error 110)",
            "statement timeout: query exceeded 300000 ms",
            "opencode-harness failed for architect exit=Some(1) (spent tokens_in=0): UnexpectedStatus: 500",
            "opencode-harness: `/usr/local/bin/opencode` is not the OpenCode build this adapter is written against.",
            "opencode-harness: the vendor CLI at `x` (unknown version) could not be run at all",
            "tcp connect error: Connection refused (os error 61)",
        ] {
            assert!(is_engine_fault(message), "{message}");
        }
    }

    /// A story's own failure is not the engine's, and an unrecognised message stays a failure: the safe direction is
    /// to keep a verdict rather than to clear one.
    #[test]
    fn a_work_failure_is_not_the_engines() {
        for message in [
            "QA verdict: FAIL - acceptance criterion 3 has no test",
            "Smith could not build: error[E0308] mismatched types",
            "no ready task for wave 2; the packet names one that does not exist",
            "unsupported work type ANYTHING",
            "QA verdict: FAIL - test forge_runtime::slow_case timed out after 60s",
            "opencode-harness failed for smith: the provider request timed out",
            "opencode-harness failed for smith: UnexpectedStatus: 400",
        ] {
            assert!(!is_engine_fault(message), "{message}");
        }
    }
}
