use super::*;

#[test]
fn maestro_response_envelope_parsing() {
    let stdout = r#"{"agentId":"x","agentName":"forge-nemotron","sessionId":"session-123","response":"done","success":true,"error":null,"usage":{"inputTokens":100,"outputTokens":25,"cacheReadInputTokens":0,"cacheCreationInputTokens":0,"totalCostUsd":0.012,"contextWindow":131072,"contextUsagePercent":1}}"#;
    let parsed = parse_maestro_output(stdout).expect("the documented envelope parses");
    assert_eq!(parsed.assistant_text, "done");
    assert_eq!(parsed.session_id.as_deref(), Some("session-123"));
    let usage = parsed
        .usage
        .expect("usage is recorded when cost is authoritative");
    assert_eq!(usage.tokens_input, 100);
    assert_eq!(usage.tokens_output, 25);
    assert!((usage.cost_usd - 0.012).abs() < 1e-12);
}

#[test]
fn maestro_response_failure_is_failure() {
    let stdout = r#"{"agentId":"x","sessionId":"s-1","response":"","success":false,"error":"agent is busy"}"#;
    let error = parse_maestro_output(stdout).expect_err("success:false must fail");
    assert!(error.to_string().contains("agent is busy"), "{error}");
}

#[test]
fn maestro_response_malformed_is_failure() {
    assert!(
        parse_maestro_output("not json at all\n").is_err(),
        "garbage must never read as a successful turn"
    );
    let no_text = r#"{"success":true,"sessionId":"s-1"}"#;
    assert!(
        parse_maestro_output(no_text).is_err(),
        "an envelope with no response text is not a turn"
    );
}

#[test]
fn maestro_response_usage_without_cost_is_unmeasured() {
    // Tokens without an authoritative cost: usage stays None (unmeasured), never a fake $0.
    let stdout = r#"{"success":true,"sessionId":"s-1","response":"done","usage":{"inputTokens":10,"outputTokens":5}}"#;
    let parsed = parse_maestro_output(stdout).expect("parses");
    assert!(parsed.usage.is_none(), "costless usage must not become $0");
}
