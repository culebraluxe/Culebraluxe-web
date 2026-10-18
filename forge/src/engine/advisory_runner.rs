//! Phase 2 advisory panel runner — the part of the GPT contract that fits the current stack.
//!
//! Fits: `plan_swarm` routing, the twelve read-only profiles, independent Round 1 seats,
//! Council's three-round protocol (Round 1 independent → Round 2 cross-examination → Round 3
//! synthesis, as already defined in `docs/agent/executive/COUNCIL.md`), structured `ExpertVerdict`
//! validation, one Chief-of-Staff `DecisionSynthesis`, and durable persistence of the decision,
//! its HOLD (when story-bound) and its `ForgeDecisionHandoff`.
//!
//! Transport: each seat runs through the DSH harness in a fresh read-only session. The seat
//! executor is a trait so the protocol is testable without a model call, and so an OpenCode or
//! Maestro materializer can drive the same rounds later.
//!
//! Deliberately not here: web research (TD), per-seat DSH agent presets (TD), micro-agent children
//! (TD), auto-creating a Story Board story from a handoff (product call; the handoff is persisted as
//! a story artifact when a story id is supplied).

use std::path::PathBuf;

use crate::engine::advisory_profiles::advisory_profile;
use crate::engine::decisions;
use crate::engine::dsh::advisory;
use crate::engine::executive_swarm::{
    handoff_to_forge, plan_swarm, validate_verdict, CouncilGate, DecisionSignal, DecisionSynthesis,
    ExpertVerdict, ForgeDecisionHandoff, SwarmAgent, SwarmMode, SwarmPlan, SwarmRequest, Verdict,
};

/// One model turn for a seat. `tokens` is the seat's measured tokens when the transport can report
/// them, `0` otherwise (never fabricated cost).
#[derive(Debug, Clone, Default)]
pub struct SeatTurn {
    pub text: String,
    pub tokens: u64,
}

/// How a seat is executed. Production uses [`DshSeatRunner`]; tests use a deterministic fake.
pub trait SeatRunner {
    fn run(&self, agent: SwarmAgent, prompt: &str) -> Result<SeatTurn, String>;
}

/// The DSH-backed seat: one read-only headless session per seat.
pub struct DshSeatRunner {
    pub workspace: PathBuf,
    pub model: Option<String>,
}

impl SeatRunner for DshSeatRunner {
    fn run(&self, _agent: SwarmAgent, prompt: &str) -> Result<SeatTurn, String> {
        let turn = advisory::run_read_only_prompt(&self.workspace, prompt, self.model.as_deref())
            .map_err(|error| error.to_string())?;
        let tokens = turn
            .tokens
            .map(|tokens| {
                tokens.input_tokens
                    + tokens.output_tokens
                    + tokens.cache_read_tokens
                    + tokens.cache_write_tokens
            })
            .unwrap_or(0);
        Ok(SeatTurn {
            text: turn.text,
            tokens,
        })
    }
}

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

/// What a completed panel produced. `cross_examination` is empty in advisory mode.
#[derive(Debug, Clone)]
pub struct PanelOutcome {
    pub plan: SwarmPlan,
    pub verdicts: Vec<ExpertVerdict>,
    pub cross_examination: Vec<ExpertVerdict>,
    pub synthesis: DecisionSynthesis,
    pub total_tokens: u64,
}

/// Where a completed panel should be recorded. All optional: a purely strategic panel records only
/// the decision row; a story-bound decision can also carry the handoff artifact and a HOLD.
#[derive(Debug, Clone, Copy, Default)]
pub struct PanelRecordContext<'a> {
    pub story_id: Option<&'a str>,
    /// Required together with `story_id` for a HOLD: `forge_hold_record` is workflow-bound.
    pub process_instance_id: Option<&'a str>,
}

/// What `record` actually wrote.
#[derive(Debug, Clone)]
pub struct RecordedPanel {
    pub decision_key: String,
    pub handoff: ForgeDecisionHandoff,
    pub hold_id: Option<String>,
    pub handoff_artifact_id: Option<String>,
}

/// The role prompt for one Round 1 seat: same packet, role-specific lens, machine-readable verdict.
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
         Form your verdict independently. Do not speculate about what other seats will say.\n\
         Reply with ONLY one JSON object with exactly these keys:\n\
         {{\"agent\":\"{id}\",\"verdict\":\"PROCEED|REJECT|CONDITIONAL\",\"recommendation\":\"...\",\
         \"confidence\":0-100,\"key_reasons\":[\"...\"],\"primary_risk\":\"...\",\
         \"assumptions\":[\"...\"],\"evidence\":[\"...\"],\"what_would_change_my_mind\":\"...\"}}",
        id = agent.id(),
        role = role,
        question = question,
        context = context
    )
}

/// Round 2 (Council only): each seat challenges the strongest opposing position and restates its
/// final verdict. Round 1 positions are visible here — that is the point of cross-examination.
pub fn cross_examination_prompt(
    agent: SwarmAgent,
    question: &str,
    round_one: &[ExpertVerdict],
) -> String {
    let positions = round_one
        .iter()
        .filter(|verdict| verdict.agent != agent)
        .map(|verdict| {
            format!(
                "- {}: {:?} (confidence {}); recommendation: {}; primary risk: {}",
                verdict.agent.id(),
                verdict.verdict,
                verdict.confidence,
                verdict.recommendation,
                verdict.primary_risk
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You are the Agent OS Phase 2 Council seat `{id}` in Round 2 (cross-examination).\n\
         QUESTION:\n{question}\n\n\
         ROUND 1 POSITIONS FROM THE OTHER SEATS:\n{positions}\n\n\
         Challenge the STRONGEST OPPOSING position: name the unsupported assumption, missing data,\n\
         second-order effect, or failure mode. Do not merely restate your Round 1 answer.\n\
         You are read-only. Then give your FINAL verdict as ONLY one JSON object with the keys:\n\
         {{\"agent\":\"{id}\",\"verdict\":\"PROCEED|REJECT|CONDITIONAL\",\"recommendation\":\"...\",\
         \"confidence\":0-100,\"key_reasons\":[\"...\"],\"primary_risk\":\"...\",\
         \"assumptions\":[\"...\"],\"evidence\":[\"...\"],\"what_would_change_my_mind\":\"...\"}}",
        id = agent.id(),
        question = question,
        positions = positions
    )
}

/// Prompt the Chief of Staff for Round 3: synthesize both rounds into the durable decision shape.
pub fn synthesis_prompt(
    question: &str,
    round_one: &[ExpertVerdict],
    round_two: &[ExpertVerdict],
) -> String {
    let first = serde_json::to_string_pretty(round_one).unwrap_or_default();
    let second = serde_json::to_string_pretty(round_two).unwrap_or_default();
    format!(
        "You are the Agent OS Chief of Staff. Synthesize the panel into ONE decision (Round 3).\n\
         Preserve dissent; do not majority-vote. QUESTION:\n{question}\n\n\
         ROUND 1 (independent):\n{first}\n\nROUND 2 (cross-examination):\n{second}\n\n\
         Reply with ONLY one JSON object: {{\"decision_id\":\"slug\",\"question\":\"...\",\
         \"recommendation\":\"...\",\"confidence\":0-100,\"consensus\":[\"...\"],\"dissent\":[\"...\"],\
         \"accepted_risks\":[\"...\"],\"next_action\":\"...\",\"source_agents\":[\"...\"]}}"
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

/// Parse the Round 3 synthesis JSON.
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

impl PanelOutcome {
    /// Record the panel durably: decision row (always), handoff artifact (when a story id is
    /// supplied), HOLD (when a story and its workflow instance are supplied).
    pub fn record(&self, context: &PanelRecordContext<'_>) -> Result<RecordedPanel, String> {
        let decision_key = decisions::record_panel_decision(&self.synthesis)?;
        let handoff = handoff_to_forge(&self.synthesis);
        let handoff_artifact_id = match context.story_id {
            Some(story_id) => {
                decisions::record_handoff_artifact(&handoff, story_id, &decision_key)?
            }
            None => None,
        };
        let hold_id = match (context.story_id, context.process_instance_id) {
            (Some(story_id), Some(process_instance_id)) => {
                let reason = format!(
                    "advisory decision `{decision_key}` needs owner authority: {}",
                    self.synthesis.recommendation
                );
                Some(decisions::open_panel_hold(
                    process_instance_id,
                    story_id,
                    &reason,
                    Some(self.synthesis.next_action.as_str()).filter(|target| !target.is_empty()),
                )?)
            }
            _ => None,
        };
        Ok(RecordedPanel {
            decision_key,
            handoff,
            hold_id,
            handoff_artifact_id,
        })
    }
}

/// Run the panel with the DSH transport.
pub fn run_panel(request: &AdvisoryPanelRequest) -> Result<PanelOutcome, String> {
    let model = advisory::advisory_model().map_err(|error| error.to_string())?;
    let runner = DshSeatRunner {
        workspace: request.workspace.clone(),
        model,
    };
    run_panel_with(request, &runner)
}

/// Run the protocol: plan → Round 1 (always) → Round 2 (Council only) → Round 3 synthesis.
pub fn run_panel_with(
    request: &AdvisoryPanelRequest,
    runner: &dyn SeatRunner,
) -> Result<PanelOutcome, String> {
    let swarm_request = SwarmRequest {
        mode: request.mode,
        signals: request.signals.clone(),
        council_gate: request.council_gate,
    };
    let plan = plan_swarm(&swarm_request).map_err(|error| error.to_string())?;
    let mut verdicts = Vec::with_capacity(plan.participants.len());
    let mut total_tokens = 0u64;
    for agent in &plan.participants {
        let prompt = seat_prompt(*agent, &request.question, &request.context);
        let turn = runner
            .run(*agent, &prompt)
            .map_err(|error| format!("seat {}: {error}", agent.id()))?;
        total_tokens += turn.tokens;
        verdicts.push(parse_verdict(*agent, &turn.text)?);
    }

    let mut cross_examination = Vec::new();
    if request.mode == SwarmMode::Council {
        for agent in &plan.participants {
            let prompt = cross_examination_prompt(*agent, &request.question, &verdicts);
            let turn = runner
                .run(*agent, &prompt)
                .map_err(|error| format!("cross-examination {}: {error}", agent.id()))?;
            total_tokens += turn.tokens;
            cross_examination.push(parse_verdict(*agent, &turn.text)?);
        }
    }

    let prompt = synthesis_prompt(&request.question, &verdicts, &cross_examination);
    let turn = runner
        .run(SwarmAgent::Executive(plan.synthesis_owner), &prompt)
        .map_err(|error| format!("chief of staff: {error}"))?;
    total_tokens += turn.tokens;
    let synthesis = parse_synthesis(&turn.text)?;
    Ok(PanelOutcome {
        plan,
        verdicts,
        cross_examination,
        synthesis,
        total_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::executive_swarm::{
        CouncilGate, DecisionSignal, ExpertAgent, SwarmAgent, SwarmMode,
    };
    use std::sync::Mutex;

    /// A deterministic seat: valid verdicts, recorded prompts, no model call.
    struct FakeSeat {
        prompts: Mutex<Vec<String>>,
        calls: Mutex<usize>,
    }

    impl FakeSeat {
        fn new() -> Self {
            Self {
                prompts: Mutex::new(Vec::new()),
                calls: Mutex::new(0),
            }
        }
    }

    impl SeatRunner for FakeSeat {
        fn run(&self, agent: SwarmAgent, prompt: &str) -> Result<SeatTurn, String> {
            self.prompts.lock().unwrap().push(prompt.to_string());
            *self.calls.lock().unwrap() += 1;
            if prompt.contains("Chief of Staff") {
                return Ok(SeatTurn {
                    text: r#"{"decision_id":"panel-test","question":"q","recommendation":"go",
                        "confidence":80,"consensus":["agree"],"dissent":["dissent kept"],
                        "accepted_risks":["risk"],"next_action":"do the work",
                        "source_agents":["exec-advisor"]}"#
                        .to_string(),
                    tokens: 5,
                });
            }
            let verdict = if prompt.contains("Round 2") {
                "CONDITIONAL"
            } else {
                "PROCEED"
            };
            Ok(SeatTurn {
                text: format!(
                    "{{\"agent\":\"{}\",\"verdict\":\"{verdict}\",\"recommendation\":\"go\",\
                     \"confidence\":70,\"key_reasons\":[\"reason\"],\"primary_risk\":\"risk\",\
                     \"assumptions\":[],\"evidence\":[\"e\"],\"what_would_change_my_mind\":\"data\"}}",
                    agent.id()
                ),
                tokens: 5,
            })
        }
    }

    fn request(mode: SwarmMode, gate: Option<CouncilGate>) -> AdvisoryPanelRequest {
        AdvisoryPanelRequest {
            question: "Should we adopt DSH?".into(),
            context: "context".into(),
            workspace: std::path::PathBuf::from("/tmp"),
            signals: vec![DecisionSignal::Security],
            mode,
            council_gate: gate,
        }
    }

    fn council_gate() -> Option<CouncilGate> {
        Some(CouncilGate {
            credible_paths: 2,
            material_consequence: true,
            difficult_to_reverse: true,
        })
    }

    #[test]
    fn advisory_round_one_is_independent_and_has_no_cross_examination() {
        let fake = FakeSeat::new();
        let outcome = run_panel_with(&request(SwarmMode::Advisory, None), &fake).unwrap();
        assert!(outcome.cross_examination.is_empty());
        assert_eq!(outcome.verdicts.len(), 2); // security signal: principal architect + security
        let prompts = fake.prompts.lock().unwrap().clone();
        for prompt in &prompts[..2] {
            assert!(!prompt.contains("ROUND 1 POSITIONS"));
            assert!(!prompt.contains("Round 2"));
        }
        assert!(outcome.total_tokens >= 15); // 2 seats + synthesis
    }

    #[test]
    fn council_runs_cross_examination_after_round_one_and_keeps_dissent() {
        let fake = FakeSeat::new();
        let outcome = run_panel_with(&request(SwarmMode::Council, council_gate()), &fake).unwrap();
        assert_eq!(
            outcome.cross_examination.len(),
            outcome.plan.participants.len()
        );
        let prompts = fake.prompts.lock().unwrap().clone();
        assert!(prompts
            .iter()
            .any(|prompt| prompt.contains("ROUND 1 POSITIONS")));
        assert!(prompts
            .iter()
            .any(|prompt| prompt.contains("ROUND 2 (cross-examination)")));
        assert_eq!(outcome.synthesis.dissent, vec!["dissent kept".to_string()]);
    }

    #[test]
    fn council_without_the_full_gate_never_calls_a_seat() {
        let fake = FakeSeat::new();
        let result = run_panel_with(
            &request(
                SwarmMode::Council,
                Some(CouncilGate {
                    credible_paths: 1,
                    material_consequence: true,
                    difficult_to_reverse: true,
                }),
            ),
            &fake,
        );
        assert!(result.is_err());
        assert_eq!(*fake.calls.lock().unwrap(), 0);
    }

    #[test]
    fn verdict_parsing_rejects_invalid_and_accepts_valid() {
        let agent = SwarmAgent::Expert(ExpertAgent::Rust);
        let valid = r#"{"verdict":"CONDITIONAL","recommendation":"do it","confidence":70,
            "key_reasons":["a"],"primary_risk":"risk","assumptions":[],
            "evidence":["e"],"what_would_change_my_mind":"new data"}"#;
        let verdict = parse_verdict(agent, valid).expect("valid verdict");
        assert_eq!(verdict.verdict, Verdict::Conditional);
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
}
