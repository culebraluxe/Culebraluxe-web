//! Production role runner control plane.
//! Model/worktree execution is injected via `RoleHarness` — same door as the TS runner.

use crate::engine::assay::CommandResult;
use crate::engine::executor::drive::{ForgeRoleOutcome, ForgeRoleRunner};
use crate::engine::facts::ForgeGateEvidence;
use crate::engine::harness::{HarnessUsage, TurnTermination};
use crate::engine::runtime::ActiveForgeRoleTask;
use crate::engine::writer::{ForgeEvidenceReader, ForgeStateWriter};
use std::sync::Arc;
use workflow::{Result, WorkflowError};

pub struct HarnessOutput {
    pub raw: String,
    pub candidate_sha: Option<String>,
    pub assay_commands: Vec<String>,
    pub acceptance_mapped: bool,
    /// Why the harness refused this turn's work as a candidate (`candidate_sha` is then `None`). The work is
    /// still paid code: the runner snapshots it into `forge_tool_artifact` instead of letting the refusal void it.
    pub refusal: Option<String>,
    /// The base the harness measured Smith's work against — its declared execution base, or the HEAD it saw
    /// before the turn when it declared none. Without it a run with no declared base had nothing to diff from
    /// and its code was never captured at all.
    pub execution_base: Option<String>,
    /// What this turn spent, as the harness's own session store reports it. `None` is unmeasured, never zero.
    pub usage: Option<HarnessUsage>,
}

/// What a code-delivering lane needs to JUDGE a candidate: git in the workspace the turn ran in, and the test mode
/// the packet declared. The harness supplies these facts; the rules that read them are Smith's
/// (`roles::smith::judge_delivered_candidate`), never the transport's.
pub trait CandidateProbe {
    /// `git <args>` in the turn's workspace: trimmed stdout, or `None` when git failed.
    fn git(&self, args: &[&str]) -> Option<String>;
    /// The test mode the packet the harness was started with declared (`RUST_CONTRACT`, …).
    fn declared_test_mode(&self) -> Option<&str>;
}

/// What production is running, as Forge-owned code reads it (`engine::production_probe`). The DevOps lane's
/// receipt is this answer; the deploy agent is read-only and can present none.
pub trait ProductionProbe {
    /// The base URL probed, named in the receipt and in any refusal.
    fn production_url(&self) -> String;
    /// The commit production reports it is running.
    fn deployed_sha(&self) -> std::result::Result<String, String>;
}

pub trait RoleHarness: Send + Sync {
    /// `self_heal` is this attempt's corrective directive: `None` on the first attempt, and — when the runner
    /// retries a role that missed a required deliverable — the directive naming exactly what was missed.
    ///
    /// It is part of the prompt contract, not decoration. The legacy runner built this text and handed it to the
    /// next attempt; the port built it and threw it away (`let _directive = …`), so a "bounded corrective retry"
    /// re-sent the same prompt and could only produce the same omission (2026-09-29).
    fn run_role(
        &self,
        node_id: &str,
        task: &ActiveForgeRoleTask,
        self_heal: Option<&str>,
    ) -> Result<HarnessOutput>;
    /// Stop the turn this harness is running RIGHT NOW, if it is running one, and say what was stopped.
    ///
    /// The default is `Ok(None)`, and that is a real answer rather than a stub: a harness with no subprocess, or
    /// one whose turn already ended, has nothing to stop, and a harness that cannot stop its own work must not
    /// report that it did. The OpenCode harness overrides it, because a turn there is a real process tree — and a
    /// budget cap that cannot stop one is advice, not a control (see `engine::opencode_client::RunningTurn`).
    ///
    /// `reason` is carried into the turn's own record, so an interruption is reportable instead of
    /// indistinguishable from a crash.
    fn interrupt_execution(&self, _reason: &str) -> Result<Option<TurnTermination>> {
        Ok(None)
    }
    fn exists_on_base_ref(&self, base_ref: &str, path: &str) -> bool;
    /// Where this harness runs commands from — the assay workspace.
    ///
    /// DECLARED HERE (2026-09-20) because the OpenCode harness implemented it without the trait knowing: it sat
    /// inside `impl RoleHarness for OpenCodeHarness` as an undeclared extra, which is `E0407`, while a caller
    /// reached for it through the harness and got `E0599`. Both errors were the same missing line. The full path
    /// is spelled out rather than imported so this file needs no new `use`.
    fn assay_cwd(&self) -> &std::path::Path;
    /// Versioned identity for the command environment policy. The current production harnesses
    /// inherit the Forge process environment for scoped assay shells; receipts store this policy
    /// name and never persist environment values.
    fn assay_environment_identity(&self) -> &str {
        "forge-inherited-shell-v1"
    }
    fn execution_base_commit(&self) -> Option<&str> {
        None
    }
    fn run_command(&self, command: &str) -> CommandResult;
    /// The repository facts a delivered candidate is judged against. `None` for a harness that runs no real
    /// repository (every double), whose candidate is taken as given.
    fn candidate_probe(&self) -> Option<&dyn CandidateProbe> {
        None
    }
    /// Production's answer for the DevOps release nodes. `None` for a harness with no production behind it (every
    /// double), whose release turns run as model turns.
    fn production_probe(&self) -> Option<&dyn ProductionProbe> {
        None
    }
}

/// Control-plane runner: harness produces raw output; collect + gates decide evidence.
pub struct ProductionRoleRunner {
    pub harness: Arc<dyn RoleHarness>,
    pub current: ForgeGateEvidence,
    pub writer: Option<Arc<dyn ForgeStateWriter>>,
    /// The Story Run this lane is executing (the row a claim opened). Every artifact the lane produces is keyed to
    /// it, so a later reader can see which execution a reading came out of instead of re-deriving it.
    pub story_run_id: Option<String>,
    /// The bench intent the dispatch carried (migration 167 `launch_intent`) — the Cockpit's cap on the Lead,
    /// travelling with the run it caps.
    pub bench_intent: Option<String>,
    /// Storyboard-declared test policy. RUST_CONTRACT means this story authors a test artifact; runtime assertion
    /// failures are findings about the application, not a reason to rewrite the test until green.
    pub test_mode: Option<String>,
    /// Authoritative Storyboard assay commands for test-authoring mode. These let RUST_CONTRACT QA stay
    /// deterministic and model-free: the test command is evidence about the application, not another model turn.
    pub contract_assay_commands: Vec<String>,
    pub contract_acceptance_mapped: bool,
    pub require_prod: bool,
    /// The durable evidence each turn starts from. Without it every turn of a process saw `current` — the evidence
    /// the process was WOKEN with — and never what earlier turns produced (candidate, decision, published commit).
    pub evidence_reader: Option<Arc<dyn ForgeEvidenceReader>>,
}

/// The envelope a turn runs under, as whoever hosts it exposes it.
///
/// `ProductionRoleRunner` is the production answer, and the reason this trait exists at all: the shared
/// lifecycle needs a turn's envelope (harness, starting evidence, writer, dispatch knobs) and a lane service
/// must inherit the sequence *without* owning the ports its host holds. The service names the runner, the
/// runner names its ports, and neither has to know the other's shape.
pub trait ForgeTurnPorts {
    fn harness(&self) -> &dyn RoleHarness;
    /// An owned handle to the harness for callbacks that outlive the runner borrow
    /// (FIX-006-SOUNDNESS: the interrupt handle is `'static`, so it clones this
    /// `Arc` instead of transmuting a short borrow).
    fn harness_arc(&self) -> Arc<dyn RoleHarness>;
    fn current(&self) -> &ForgeGateEvidence;
    fn writer(&self) -> Option<&dyn ForgeStateWriter>;
    fn story_run_id(&self) -> Option<&str>;
    fn bench_intent(&self) -> Option<&str>;
    fn test_mode(&self) -> Option<&str>;
    fn contract_assay_commands(&self) -> &[String];
    fn contract_acceptance_mapped(&self) -> bool;
    fn require_prod(&self) -> bool;
    /// The evidence a turn on `story_id` starts from: the story's latest durable evidence over the wake evidence.
    /// The default is the wake evidence alone, for hosts with no durable store behind them.
    ///
    /// Returns `Err(WorkflowError::Unavailable)` when the database cannot be reached, which the engine classifies
    /// as an engine fault (retryable). Returns `Err(WorkflowError::Generic)` for other database errors.
    fn current_for(&self, story_id: &str) -> Result<ForgeGateEvidence> {
        Ok(self.current().clone())
    }
}

impl ForgeTurnPorts for ProductionRoleRunner {
    fn harness(&self) -> &dyn RoleHarness {
        self.harness.as_ref()
    }

    fn harness_arc(&self) -> Arc<dyn RoleHarness> {
        Arc::clone(&self.harness)
    }

    fn current(&self) -> &ForgeGateEvidence {
        &self.current
    }

    fn writer(&self) -> Option<&dyn ForgeStateWriter> {
        self.writer.as_deref()
    }

    fn story_run_id(&self) -> Option<&str> {
        self.story_run_id.as_deref()
    }

    fn bench_intent(&self) -> Option<&str> {
        self.bench_intent.as_deref()
    }

    fn test_mode(&self) -> Option<&str> {
        self.test_mode.as_deref()
    }

    fn contract_assay_commands(&self) -> &[String] {
        &self.contract_assay_commands
    }

    fn contract_acceptance_mapped(&self) -> bool {
        self.contract_acceptance_mapped
    }

    fn require_prod(&self) -> bool {
        self.require_prod
    }

    fn current_for(&self, story_id: &str) -> Result<ForgeGateEvidence> {
        match &self.evidence_reader {
            Some(reader) => {
                let evidence = reader.read(story_id).map_err(|db_failure| {
                    // Convert DbFailure to WorkflowError: connection failures become Unavailable (retryable),
                    // others become Generic.
                    if db_failure.retryable {
                        WorkflowError::unavailable(db_failure.to_string())
                    } else {
                        WorkflowError::generic(db_failure.to_string())
                    }
                })?;
                Ok(evidence.merge_over(&self.current))
            }
            None => Ok(self.current.clone()),
        }
    }
}

impl ProductionRoleRunner {
    pub fn new(harness: Arc<dyn RoleHarness>, current: ForgeGateEvidence) -> Self {
        Self {
            harness,
            current,
            writer: None,
            story_run_id: None,
            bench_intent: None,
            test_mode: None,
            contract_assay_commands: Vec::new(),
            contract_acceptance_mapped: false,
            require_prod: false,
            evidence_reader: None,
        }
    }

    /// Name the run this lane is executing. Without it an artifact is still recorded (the measurement happened) but
    /// it hangs on the story alone, and no run's ruling can be read against it.
    pub fn with_story_run(mut self, story_run_id: Option<String>) -> Self {
        self.story_run_id = story_run_id;
        self
    }

    /// Hand the lane the state writer its records go through.
    ///
    /// DECLARED HERE (2026-10-01) because production never set the field at all: `new` leaves `writer: None`,
    /// `bin/forge.rs` built the runner without naming one, and the executor passes what it is given straight
    /// through (`opts.runner.unwrap_or(&synthetic)`). Everything behind `self.writer` was therefore skipped
    /// silently on every production lane — the run/candidate stamp and the capture that carries the patch with it.
    /// A lane that records nothing looks exactly like a lane that had nothing to record, which is why this stayed
    /// invisible until a real run was watched from launch to artifact. The writer itself was never missing: the
    /// same `Arc` is handed to the runtime one line above, and only the runner was left holding `None`.
    pub fn with_writer(mut self, writer: Arc<dyn ForgeStateWriter>) -> Self {
        self.writer = Some(writer);
        self
    }

    /// Carry the dispatch's bench intent into the lane. `None` means the Lead decides, which is the row's own
    /// answer — never a default this code invents.
    pub fn with_bench_intent(mut self, bench_intent: Option<String>) -> Self {
        self.bench_intent = bench_intent;
        self
    }

    pub fn with_test_mode(mut self, test_mode: Option<String>) -> Self {
        self.test_mode = test_mode;
        self
    }

    pub fn with_contract_assay_commands(mut self, assay_commands: Vec<String>) -> Self {
        self.contract_assay_commands = assay_commands;
        self
    }

    pub fn with_evidence_reader(mut self, reader: Option<Arc<dyn ForgeEvidenceReader>>) -> Self {
        self.evidence_reader = reader;
        self
    }

    pub fn with_contract_acceptance_mapped(mut self, acceptance_mapped: bool) -> Self {
        self.contract_acceptance_mapped = acceptance_mapped;
        self
    }
}

/// The compatibility adapter: the established runner seam, now a composition of the lane services.
///
/// A compatibility caller that still holds `ProductionRoleRunner` may call `run(node_id, task)` and keeps
/// getting that lane's behavior, because this hands the turn to the
/// service that owns the node and lets that service run the shared lifecycle with its own reading. No role
/// policy lives here any more: no node name, no lane name and no reading appears in this file. Which node
/// belongs to which lane is `role_mapping`'s answer, applied by `AbstractForgeService::supports_node`.
impl ForgeRoleRunner for ProductionRoleRunner {
    fn run(&self, node_id: &str, task: &ActiveForgeRoleTask) -> Result<ForgeRoleOutcome> {
        crate::roles::service::ForgeLaneServices::new(self).run(node_id, task)
    }

    /// This is the runner that HAS the envelope. It is where a turn's harness, its starting evidence, its
    /// writer and its dispatch knobs live, so a lane service reads its turn from here instead of being
    /// handed the ports again — and the shared lifecycle reads the same ports the runner has always held.
    fn turn_ports(&self) -> Option<&dyn ForgeTurnPorts> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::writer::RecordingWriter;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A harness that counts the turns it was asked for and passes every command it is asked to run.
    struct CountingHarness {
        turns: AtomicUsize,
    }

    impl CountingHarness {
        fn new() -> Self {
            Self {
                turns: AtomicUsize::new(0),
            }
        }

        fn turns(&self) -> usize {
            self.turns.load(Ordering::SeqCst)
        }
    }

    impl RoleHarness for CountingHarness {
        fn run_role(
            &self,
            node_id: &str,
            _: &ActiveForgeRoleTask,
            _: Option<&str>,
        ) -> Result<HarnessOutput> {
            self.turns.fetch_add(1, Ordering::SeqCst);
            Ok(HarnessOutput {
                raw: format!("{node_id} answered\n"),
                candidate_sha: None,
                assay_commands: vec![],
                acceptance_mapped: false,
                refusal: None,
                execution_base: None,
                usage: None,
            })
        }
        fn exists_on_base_ref(&self, _: &str, _: &str) -> bool {
            true
        }
        fn assay_cwd(&self) -> &std::path::Path {
            std::path::Path::new(".")
        }
        fn run_command(&self, command: &str) -> CommandResult {
            CommandResult {
                cancelled: false,
                command: command.into(),
                exit_code: 0,
                passed: true,
                excerpt: String::new(),
                unmeasurable: false,
                output: String::new(),
            }
        }
    }

    fn role_task(node_id: &str) -> ActiveForgeRoleTask {
        ActiveForgeRoleTask {
            task_id: format!("task-{node_id}"),
            process_instance_id: "proc-1".into(),
            story_id: "TST-COMPAT-ADAPTER-001".into(),
            token_id: None,
            node_id: Some(node_id.into()),
            status: workflow::TaskStatus::Ready,
            assignee: None,
            candidates: vec![node_id.into()],
        }
    }

    /// THIS FILE'S WHOLE REMAINING JOB, and the test that keeps it that way: the compatibility runner hands
    /// the node to the service that owns it, so the behavior a caller sees is the lane's own reading.
    ///
    /// `qa_verify` under RUST_CONTRACT is the sharpest case to prove it with, because that lane answers
    /// WITHOUT asking a harness for a turn at all (its reading takes the whole turn). A turn spent on the
    /// harness here would mean the adapter had run some other lane's reading — or carried a reading of its
    /// own — and the measurement this lane exists for would have been described by a model instead of taken.
    #[test]
    fn the_compatibility_runner_hands_the_turn_to_the_lane_service() {
        let harness = Arc::new(CountingHarness::new());
        let writer = Arc::new(RecordingWriter::default());
        let runner = ProductionRoleRunner::new(harness.clone(), ForgeGateEvidence::default())
            .with_writer(writer.clone())
            .with_story_run(Some("11111111-2222-3333-4444-555555555555".into()))
            .with_test_mode(Some("RUST_CONTRACT".into()))
            .with_contract_assay_commands(vec!["cargo test".into()]);

        let outcome = ForgeRoleRunner::run(&runner, "qa_verify", &role_task("qa_verify"))
            .expect("the measurement lane takes the whole turn");

        assert_eq!(
            harness.turns(),
            0,
            "the measurement lane must not spend a model turn"
        );
        assert_eq!(outcome.transition_name.as_deref(), Some("complete"));
        let artifacts = writer.artifacts.lock().unwrap();
        assert!(
            artifacts.iter().any(|a| a.kind == "qa-assay-evidence"),
            "the lane's own reading ran and wrote what it measured: {artifacts:?}"
        );
    }

    /// The other half of the adapter's contract: a lane whose work comes from a model still gets its turn,
    /// and still gets it once per attempt under the shared lifecycle.
    #[test]
    fn a_lane_whose_work_is_a_model_turn_still_gets_one() {
        let harness = Arc::new(CountingHarness::new());
        let runner = ProductionRoleRunner::new(harness.clone(), ForgeGateEvidence::default());

        let _ = ForgeRoleRunner::run(&runner, "smith", &role_task("smith"));

        assert!(
            harness.turns() >= 1,
            "Smith's work is a model turn, and the adapter must reach the lane that asks for it"
        );
    }
}
