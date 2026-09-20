//! Port of forge-spend-cap.ts.

pub fn parse_forge_spend_cap_usd(raw: Option<&str>) -> Option<f64> {
    let v = raw?.trim();
    if v.is_empty() {
        return None;
    }
    let n: f64 = v.parse().ok()?;
    if n.is_finite() && n >= 0.0 {
        Some(n)
    } else {
        None
    }
}

pub fn forge_spend_should_hold(spend_usd: Option<f64>, cap_usd: Option<f64>) -> bool {
    match (spend_usd, cap_usd) {
        (Some(s), Some(c)) => s > c,
        _ => false,
    }
}
