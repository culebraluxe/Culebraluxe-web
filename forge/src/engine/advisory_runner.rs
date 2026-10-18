//! Phase 2 advisory panel runner — the part of the GPT contract that fits today.
//!
//! Fits: the contract's routing (`plan_swarm`), the twelve read-only profiles, independent seats,
//! structured `ExpertVerdict` validation, and one Chief-of-Staff synthesis — executed through the
//! DSH harness (`FORGE_HARNESS=dsh`'s module), each seat in a fresh read-only session.
//!
//! Deliberately deferred (see docs/TECH_DEBT.md): OpenCode/Maestro materializers, durable decision
//! persistence through `forge_decision`/HOLD/artifacts, Council challenge rounds, web research, and
//! `ForgeDecisionHandoff` into Story Board. This module returns a decision artifact to its caller;
//! it writes no canonical state.

use std::path::PathBuf;

use crate::engine::advisory_profiles::advisory_profile;
use crate::engine::dsh::advisory;
use crate::engine::executive_swarm::{
    plan_swarm, validate_verdict, CouncilGate, DecisionSignal, DecisionSynthesis, ExpertVerdict,
    SwarmAgent, SwarmMode, SwarmPlan, SwarmRequest, Verdict,
};

/// One panel request: the bounded packet plus the routing facts.
#[derive(Debug, Clone)]
pub struct AdvisoryPanelRequest {
    pub question: String,
    pub context: String,
    pub workspace: PathBuf,
    pub signals: Vec<DecisionSignal>,
    pub mode: SwarmMode,
    pub council_gate: Option<CouncilGate>,
}

/// What a completed panel produced.
#[derive(Debug, Clone)]
pub struct PanelOutcome {
    pub plan: SwarmPlan,
    pub verdicts: Vec<ExpertVerdict>,
    pub synthesis: DecisionSynthesis,
    pub total_tokens: u64,
}

/// The role prompt for one seat: same packet, role-specific lens, machine-readable verdict shape.
pub fn seat_prompt(agent: SwarmAgent, question: &str, context: &str) -> String {
    let role = advisory_profile(agent)
        .map(|profile| profile.description)
        .unwrap_or("bounded expert");
    format!(
        "You are the Agent OS Phase 2 seat `{id}`.\n\
         ROLE: {role}\n\n\
         QUESTION:\n{question}\n\n\
         CONTEXT:\n{context}\n\n\
         You are read-only: inspect and reason only; do not edit, commit, publish, migrate or deploy.\n\
         Form your verdict independently. Reply with ONLY one JSON object with exactly these keys:\n\
         {{\"agent\":\"{id}\",\"verdict\":\"PROCEED|REJECT|CONDITIONAL\",\"recommendation\":\"...\",\
         \"confidence\":0-100,\"key_reasons\":[\"...\"],\"primary_risk\":\"...\",\
         \"assumptions\":[\"...\"],\"evidence\":[\"...\"],\"what_would_change_my_mind\":\"...\"}}",
        id = agent.id(),
        role = role,
        question = question,
        context = context
    )
}

/// Parse a seat's JSON verdict. Markdown fences and surrounding prose are tolerated; an invalid
/// verdict is an error, never a silently coerced one.
pub fn parse_verdict(agent: SwarmAgent, text: &str) -> Result<ExpertVerdict, String> {
    let start = text
        .find('{')
        .ok_or_else(|| "verdict has no JSON object".to_string())?;
    let end = text
        .rfind('}')
        .ok_or_else(|| "verdict JSON is unterminated".to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text[start..=end])
        .map_err(|error| format!("verdict JSON is invalid: {error}"))?;
    let string = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let list = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let verdict = match string("verdict").to_ascii_uppercase().as_str() {
        "PROCEED" => Verdict::Proceed,
        "REJECT" => Verdict::Reject,
        "CONDITIONAL" => Verdict::Conditional,
        other => return Err(format!("unknown verdict {other:?}")),
    };
    let confidence = value
        .get("confidence")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "verdict confidence must be an integer 0..=100".to_string())?;
    let confidence = u8::try_from(confidence).map_err(|_| "confidence out of range".to_string())?;
    let verdict = ExpertVerdict {
        agent,
        verdict,
        recommendation: string("recommendation"),
        confidence,
        key_reasons: list("key_reasons"),
        primary_risk: string("primary_risk"),
        assumptions: list("assumptions"),
        evidence: list("evidence"),
        what_would_change_my_mind: string("what_would_change_my_mind"),
    };
    validate_verdict(&verdict).map_err(|error| format!("invalid verdict: {error:?}"))?;
    Ok(verdict)
}

/// Prompt the Chief of Staff to synthesize the independent verdicts into the durable shape.
pub fn synthesis_prompt(question: &str, verdicts: &[ExpertVerdict]) -> String {
    let panel = serde_json::to_string_pretty(verdicts).unwrap_or_default();
    format!(
        "You are the Agent OS Chief of Staff. Synthesize the independent panel into ONE decision.\n\
         Preserve dissent; do not majority-vote. QUESTION:\n{question}\n\nPANEL:\n{panel}\n\n\
         Reply with ONLY one JSON object: {{\"decision_id\":\"slug\",\"question\":\"...\",\
         \"recommendation\":\"...\",\"confidence\":0-100,\"consensus\":[\"...\"],\"dissent\":[\"...\"],\
         \"accepted_risks\":[\"...\"],\"next_action\":\"...\",\"source_agents\":[\"...\"]}}"
    )
}

/// Parse the synthesis JSON.
pub fn parse_synthesis(text: &str) -> Result<DecisionSynthesis, String> {
    let start = text
        .find('{')
        .ok_or_else(|| "synthesis has no JSON object".to_string())?;
    let end = text
        .rfind('}')
        .ok_or_else(|| "synthesis JSON is unterminated".to_string())?;
    let value: serde_json::Value = serde_json::from_str(&text[start..=end])
        .map_err(|error| format!("synthesis JSON is invalid: {error}"))?;
    let string = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let list = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let confidence = value
        .get("confidence")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "synthesis confidence must be an integer 0..=100".to_string())?;
    let confidence = u8::try_from(confidence).map_err(|_| "confidence out of range".to_string())?;
    let synthesis = DecisionSynthesis {
        decision_id: string("decision_id"),
        question: string("question"),
        recommendation: string("recommendation"),
        confidence,
        consensus: list("consensus"),
        dissent: list("dissent"),
        accepted_risks: list("accepted_risks"),
        next_action: string("next_action"),
        source_agents: list("source_agents"),
    };
    if synthesis.recommendation.trim().is_empty() {
        return Err("synthesis recommendation is empty".to_string());
    }
    Ok(synthesis)
}

/// Run the panel: plan, independent read-only seats, then one synthesis turn.
pub fn run_panel(request: &AdvisoryPanelRequest) -> Result<PanelOutcome, String> {
    let swarm_request = SwarmRequest {
        mode: request.mode,
        signals: request.signals.clone(),
        council_gate: request.council_gate,
    };
    let plan = plan_swarm(&swarm_request).map_err(|error| error.to_string())?;
    let model = advisory::advisory_model().map_err(|error| error.to_string())?;
    let mut verdicts = Vec::with_capacity(plan.participants.len());
    let mut total_tokens = 0u64;
    for agent in &plan.participants {
        let prompt = seat_prompt(*agent, &request.question, &request.context);
        let turn = advisory::run_read_only_prompt(&request.workspace, &prompt, model.as_deref())
            .map_err(|error| format!("seat {}: {error}", agent.id()))?;
        if let Some(tokens) = turn.tokens {
            total_tokens += tokens.input_tokens
                + tokens.output_tokens
                + tokens.cache_read_tokens
                + tokens.cache_write_tokens;
        }
        verdicts.push(parse_verdict(*agent, &turn.text)?);
    }
    let prompt = synthesis_prompt(&request.question, &verdicts);
    let turn = advisory::run_read_only_prompt(&request.workspace, &prompt, model.as_deref())
        .map_err(|error| format!("chief of staff: {error}"))?;
    if let Some(tokens) = turn.tokens {
        total_tokens += tokens.input_tokens
            + tokens.output_tokens
            + tokens.cache_read_tokens
            + tokens.cache_write_tokens;
    }
    let synthesis = parse_synthesis(&turn.text)?;
    Ok(PanelOutcome {
        plan,
        verdicts,
        synthesis,
        total_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::executive_swarm::{CouncilGate, DecisionSignal};

    #[test]
    fn seat_prompt_names_the_canonical_id_and_verdict_shape() {
        let prompt = seat_prompt(
            SwarmAgent::Expert(crate::engine::executive_swarm::ExpertAgent::Security),
            "Should we relax the write grant?",
            "context",
        );
        assert!(prompt.contains("expert-security"));
        assert!(prompt.contains("\"verdict\""));
        assert!(prompt.contains("read-only"));
    }

    #[test]
    fn verdict_parsing_rejects_invalid_and_accepts_valid() {
        let agent = SwarmAgent::Expert(crate::engine::executive_swarm::ExpertAgent::Rust);
        let valid = r#"{"verdict":"CONDITIONAL","recommendation":"do it","confidence":70,
            "key_reasons":["a"],"primary_risk":"risk","assumptions":[],
            "evidence":["e"],"what_would_change_my_mind":"new data"}"#;
        let verdict = parse_verdict(agent, valid).expect("valid verdict");
        assert_eq!(verdict.verdict, Verdict::Conditional);
        assert_eq!(verdict.confidence, 70);
        assert!(parse_verdict(agent, "no json here").is_err());
        let bad = r#"{"verdict":"PROCEED","recommendation":"","confidence":101}"#;
        assert!(parse_verdict(agent, bad).is_err());
    }

    #[test]
    fn synthesis_parsing_requires_a_recommendation() {
        let good = r#"{"decision_id":"d1","question":"q","recommendation":"go","confidence":80,
            "consensus":["x"],"dissent":["y"],"accepted_risks":[],"next_action":"n",
            "source_agents":["exec-advisor"]}"#;
        assert_eq!(parse_synthesis(good).unwrap().decision_id, "d1");
        let bad = r#"{"decision_id":"d1","recommendation":"","confidence":80}"#;
        assert!(parse_synthesis(bad).is_err());
    }

    #[test]
    fn panel_planning_uses_the_contract_router() {
        let request = AdvisoryPanelRequest {
            question: "q".into(),
            context: "c".into(),
            workspace: std::path::PathBuf::from("/tmp"),
            signals: vec![DecisionSignal::Security],
            mode: SwarmMode::Advisory,
            council_gate: None,
        };
        let swarm = SwarmRequest {
            mode: request.mode,
            signals: request.signals.clone(),
            council_gate: request.council_gate,
        };
        let plan = plan_swarm(&swarm).unwrap();
        assert_eq!(plan.participants.len(), 2);
        let _ = CouncilGate {
            credible_paths: 2,
            material_consequence: true,
            difficult_to_reverse: true,
        };
    }
}
