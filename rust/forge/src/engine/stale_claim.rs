//! Port of forge-stale-claim.ts.

pub fn is_forge_claim_stale(
    status: &str,
    claimed_at_ms: Option<i64>,
    now_ms: i64,
    stale_after_ms: i64,
) -> bool {
    if status == "ready" {
        return false;
    }
    let Some(claimed) = claimed_at_ms else {
        return false;
    };
    if stale_after_ms <= 0 {
        return false;
    }
    now_ms - claimed >= stale_after_ms
}

pub fn stale_after_ms_from_env(raw: Option<&str>) -> i64 {
    let minutes: i64 = raw
        .and_then(|s| s.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(60);
    minutes * 60_000
}
