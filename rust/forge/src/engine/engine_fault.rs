//! Was the failure the engine's own plumbing, or the work?

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
        "timed out",
        "statement timeout",
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
            "db: sqlstate 25P03 idle_in_transaction_session_timeout",
            "db error: sqlstate 53300 too many connections",
            "sqlstate 57P02 terminating connection due to crash of another server process",
            "io error: Broken pipe (os error 32)",
            "io error: Connection reset by peer (os error 54)",
            "unexpected EOF while reading message",
            "pool timed out while waiting for an open connection",
            "statement timeout: query exceeded 300000 ms",
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
        ] {
            assert!(!is_engine_fault(message), "{message}");
        }
    }
}
