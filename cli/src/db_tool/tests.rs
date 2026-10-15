// Test code: the parent's `mod tests;` carries the `#[cfg(test)]` fence, and the fence is repeated
// here because the structural scans read a file as production code up to its first `#[cfg(test)]` —
// unmarked, a test module would read as production code.
#[cfg(test)]
use super::{host_of, Failure};

/// The exit codes are CI's contract: 0 applied or clean, 1 refused or drift, 2 unstartable.
#[test]
fn failure_exit_codes_match_the_scripts_contract() {
    assert_eq!(Failure::Usage("bad arguments".to_string()).exit_code(), 2);
    assert_eq!(
        Failure::Configuration("missing URL".to_string()).exit_code(),
        2
    );
    assert_eq!(Failure::Other("drift".to_string()).exit_code(), 1);
}

/// Gate output gets pasted into reports and transcripts, so the banner must never carry credentials.
#[test]
fn host_of_reports_only_the_host() {
    assert_eq!(
            host_of("postgresql://neondb_owner:npg_SECRET@ep-cool-db-12345.us-east-2.aws.neon.tech/neondb?sslmode=require"),
            "ep-cool-db-12345.us-east-2.aws.neon.tech"
        );
    assert_eq!(
        host_of("postgres://user:pw@localhost:5432/db"),
        "localhost:5432"
    );
}
