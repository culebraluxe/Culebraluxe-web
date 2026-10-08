//! FORGE.RELEASE — smoke verifies the DEPLOYED SHA (TST-FORGE-RELEASE-005).
//!
//! Contract: the production smoke compares production's build stamp against the commit the DEPLOY recorded,
//! not against the commit that was merely published. After an in-chain deploy those are the same commit — but
//! they are two facts, and a smoke that verified the published sha would call a release verified while
//! production was serving something the deploy never put there.
//!
//! Four production seams carry the fact, and they may not disagree:
//!
//!   1. **the smoke gate itself** — `check_production` (`forge/src/roles/dev_ops.rs:139-152`): at
//!        `production_smoke` with `deployment_required == true` the EXPECTED sha is `deployed_sha`, and only
//!        otherwise is it `published_sha`. That conditional is this story, and it is the reason the two facts
//!        are kept apart at all.
//!   2. **the lineage** — `forge_lineage_error(evidence, "production")`
//!        (`forge/src/engine/facts.rs:216-247`): the sha production is said to have verified is compared
//!        against `deployed_sha` when a deployment was required, and against `published_sha` when it was not.
//!   3. **the fact the decision reads** — `project_forge_gate_facts` publishes `productionVerified` only when
//!        `production_verified == Some(true)` AND that lineage is clean, and publishes `productionVerifiedSha`
//!        so the answer names what was verified.
//!   4. **the decision after it** — `production_result` in the REAL `FORGE_SDLC-v6.xml` completes the story only
//!        on `productionVerified == true`, and `production_smoke` is the node whose responsibility is `dev_ops`.
//!
//! The negative cases are the story. If the smoke verified the published sha instead, every one of them would
//! still pass — so the proof is built around the case where they DIFFER: a repository-only release (no
//! in-chain deploy, so the published sha IS the right thing to look for) with a production that serves it,
//! against an in-chain release where production serves the PUBLISHED sha but not the DEPLOYED one, which must
//! not verify. Add a production that cannot be read, and one with nothing published at all.
//!
//! Deterministic and isolated: no database, no network, no external provider, no environment mutation.
//! Production is the scripted `ProductionProbe` at the boundary Forge reads it through, and the state writer is
//! the production `RecordingWriter`.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness \
//!     --test forge_release__005__smoke_verifies_deployed_sha

use std::sync::atomic::{AtomicUsize, Ordering};

use forge::engine::assay::CommandResult;
use forge::engine::definition::forge_sdlc_definition;
use forge::engine::facts::{forge_lineage_error, project_forge_gate_facts, ForgeGateEvidence};
use forge::engine::runner::{HarnessOutput, ProductionProbe, RoleHarness};
use forge::engine::runtime::ActiveForgeRoleTask;
use forge::engine::writer::RecordingWriter;
use forge::roles::dev_ops::DevOpsHooks;
use forge::roles::hooks::ForgeRoleHooks;
use forge::roles::lifecycle::ForgeRoleContext;
use workflow::{TaskStatus, Value};

/// The taxonomy name and level, carried in every assertion message so a failure names its boundary.
const HARNESS: &str = "ForgeHarness/L3 Composition";
/// The story this canonical file and function are named for.
const STORY_ID: &str = "TST-FORGE-RELEASE-005";
/// The commit QA approved and the release published.
const PUBLISHED: &str = "0123456789abcdef0123456789abcdef01234567";
/// A DIFFERENT commit — what a second candidate published under the same run would be. It is a legitimate
/// commit id in every respect; the ONLY thing that separates it from `PUBLISHED` is that the deploy never
/// recorded it, which is exactly what this story is about.
const NOT_DEPLOYED: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// Production, as Forge reads it: the build stamp it reports, and a count of how often it was asked. `probes`
/// exists so a case that must not consult production can be shown not to have consulted it.
struct Production {
    live: Result<String, String>,
    probes: AtomicUsize,
}

// The lifecycle shares the harness across threads in production; this double is only ever read here.
unsafe impl Sync for Production {}

impl ProductionProbe for Production {
    fn production_url(&self) -> String {
        "https://prod.test".into()
    }
    fn deployed_sha(&self) -> Result<String, String> {
        self.probes.fetch_add(1, Ordering::SeqCst);
        self.live.clone()
    }
}

impl RoleHarness for Production {
    fn run_role(
        &self,
        _node_id: &str,
        _task: &ActiveForgeRoleTask,
        _self_heal: Option<&str>,
    ) -> workflow::Result<HarnessOutput> {
        unreachable!("a production check is never a model turn")
    }
    fn exists_on_base_ref(&self, _base_ref: &str, _path: &str) -> bool {
        true
    }
    fn assay_cwd(&self) -> &std::path::Path {
        std::path::Path::new(".")
    }
    fn run_command(&self, _command: &str) -> CommandResult {
        unreachable!("a production check runs no command")
    }
    fn production_probe(&self) -> Option<&dyn ProductionProbe> {
        Some(self)
    }
}

/// Production answering with the commit it is running.
fn running(live: &str) -> Production {
    Production {
        live: Ok(live.to_string()),
        probes: AtomicUsize::new(0),
    }
}

/// Production that cannot be read at all.
fn unreadable(why: &str) -> Production {
    Production {
        live: Err(why.to_string()),
        probes: AtomicUsize::new(0),
    }
}

fn probes(production: &Production) -> usize {
    production.probes.load(Ordering::SeqCst)
}

/// An in-chain release: QA passed, the candidate published, production delivery required, and the deploy
/// recorded the commit it deployed.
fn at_smoke_after_deploy() -> ForgeGateEvidence {
    ForgeGateEvidence {
        qa_passed: Some(true),
        publish_succeeded: Some(true),
        deployment_required: Some(true),
        candidate_sha: Some(PUBLISHED.into()),
        published_sha: Some(PUBLISHED.into()),
        deployed_sha: Some(PUBLISHED.into()),
        ..ForgeGateEvidence::default()
    }
}

/// The same release, with the deploy having recorded a commit that is NOT the one that was published — the
/// shape a second publisher inside one run produces, and the case the published-sha smoke would wave through.
fn at_smoke_with_a_diverged_deploy() -> ForgeGateEvidence {
    ForgeGateEvidence {
        deployed_sha: Some(NOT_DEPLOYED.into()),
        ..at_smoke_after_deploy()
    }
}

/// A repository-only release: published, and production delivery is CI's job, so there is no deploy record.
fn repository_only_release() -> ForgeGateEvidence {
    // Stated in full rather than derived from the in-chain case: a repository-only release has NO deploy
    // record at all, and inheriting `deployed_sha` from a helper that assumed one would make this a test of
    // the wrong release shape.
    ForgeGateEvidence {
        qa_passed: Some(true),
        publish_succeeded: Some(true),
        deployment_required: Some(false),
        candidate_sha: Some(PUBLISHED.into()),
        published_sha: Some(PUBLISHED.into()),
        deployed_sha: None,
        ..ForgeGateEvidence::default()
    }
}

fn task(node: &str) -> ActiveForgeRoleTask {
    ActiveForgeRoleTask {
        task_id: format!("task-{node}"),
        process_instance_id: "instance-release-005".into(),
        story_id: STORY_ID.into(),
        token_id: Some("token-release-005".into()),
        node_id: Some(node.into()),
        status: TaskStatus::Ready,
        assignee: None,
        candidates: vec![],
    }
}

/// Drive the production production-check gate for `production_smoke`, returning the evidence it produced and
/// what the state writer recorded.
fn smoke(
    current: &ForgeGateEvidence,
    production: &Production,
) -> (ForgeGateEvidence, RecordingWriter) {
    let writer = RecordingWriter::default();
    let ctx = ForgeRoleContext {
        harness: production,
        current,
        writer: Some(&writer as &dyn forge::engine::writer::ForgeStateWriter),
        story_run_id: None,
        bench_intent: None,
        test_mode: None,
        contract_assay_commands: &[],
        contract_acceptance_mapped: false,
        require_prod: false,
    };
    match DevOpsHooks.turn_without_model(&ctx, "production_smoke", &task("production_smoke")) {
        Some(Ok(outcome)) => (outcome.evidence, writer),
        Some(Err(error)) => {
            panic!("{HARNESS}: the smoke gate decides hold through evidence: {error}")
        }
        None => {
            panic!("{HARNESS}: the smoke node is answered by production itself, with no model turn")
        }
    }
}

/// Read a projected boolean fact. A missing key is `false`, never a default that flips the contract.
fn projected_bool(facts: &Value, key: &str) -> bool {
    matches!(facts.get(key), Some(Value::Bool(true)))
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-FORGE-RELEASE-005); the file and the assay use it.
fn forge_release_005__smoke_verifies_deployed_sha() {
    // -----------------------------------------------------------------------------------------------------------
    // 0. PROCESS COMPOSITION. The node under test is the shipped production verification node, owned by the
    //    lane that classifies a smoke failure, and the decision after it completes the story on its verdict.
    // -----------------------------------------------------------------------------------------------------------
    let nodes = &forge_sdlc_definition().definition.nodes;
    let smoke_node = nodes
        .get("production_smoke")
        .expect("the production definition owns a production_smoke node");
    assert_eq!(
        smoke_node.node_type, "task",
        "{HARNESS}: production verification is a task node the engine runs"
    );
    assert_eq!(
        smoke_node.responsibility.as_deref(),
        Some("dev_ops"),
        "{HARNESS}: the lane that verifies production is the lane that can fix production"
    );
    let production_result = nodes
        .get("production_result")
        .expect("the production definition owns a production_result decision");
    let arms = production_result
        .decisions
        .as_ref()
        .expect("production_result is a decision");
    let complete = arms
        .iter()
        .find(|arm| arm.condition.contains("productionVerified == true"))
        .unwrap_or_else(|| {
            panic!("{HARNESS}: production_result must branch on the verification, arms: {arms:?}")
        });
    assert_eq!(
        complete.transition, "complete",
        "{HARNESS}: only a verified production completes the story"
    );
    let fail = arms
        .iter()
        .find(|arm| arm.condition.contains("productionVerified == false"))
        .expect("production_result must branch on the unverified case");
    assert_eq!(
        fail.transition, "fail",
        "{HARNESS}: an unverified production enters repair, it does not complete"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 1. THE CONTRACT — THE SMOKE VERIFIES THE DEPLOYED SHA. Production is running the deploy's commit, so the
    //    smoke verifies, names the sha it verified, and completes the node.
    // -----------------------------------------------------------------------------------------------------------
    let production = running(PUBLISHED);
    let (verified, writer) = smoke(&at_smoke_after_deploy(), &production);
    assert_eq!(
        probes(&production),
        1,
        "{HARNESS}: the smoke asks production what it is running, once"
    );
    assert_eq!(
        verified.production_verified,
        Some(true),
        "{HARNESS}: production running the DEPLOYED commit IS the production verification"
    );
    assert_eq!(
        verified.production_verified_sha.as_deref(),
        Some(PUBLISHED),
        "{HARNESS}: and the verification is recorded against the deployed sha"
    );
    let receipt = verified
        .production_verification_receipt
        .as_deref()
        .unwrap_or_default();
    assert!(
        receipt.contains("sha=") && receipt.contains(PUBLISHED),
        "{HARNESS}: the receipt names the commit it was issued for, got: {receipt:?}"
    );
    assert_eq!(
        forge_lineage_error(&verified, "production"),
        None,
        "{HARNESS}: the production lineage is clean: the verified sha IS the deployed sha"
    );
    assert!(
        projected_bool(&project_forge_gate_facts(&verified), "productionVerified"),
        "{HARNESS}: so the decision that completes the story reads it as verified"
    );
    assert!(
        writer.holds.lock().expect("not poisoned").is_empty(),
        "{HARNESS}: a verified smoke opens no hold"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 2. THE STORY'S ACTUAL CLAIM — THE PUBLISHED SHA IS NOT THE EXPECTED SHA ONCE A DEPLOY HAPPENED. Here the
    //    deploy recorded a DIFFERENT commit from the one published, and production is serving exactly the
    //    published one. A smoke that verified `published_sha` would call this release verified; the real gate
    //    holds, because it verifies the sha the DEPLOY recorded.
    // -----------------------------------------------------------------------------------------------------------
    let diverged = at_smoke_with_a_diverged_deploy();
    assert_eq!(
        diverged.published_sha.as_deref(),
        Some(PUBLISHED),
        "{HARNESS}: production really is serving the published commit — the tempting wrong answer"
    );
    assert_eq!(
        diverged.deployed_sha.as_deref(),
        Some(NOT_DEPLOYED),
        "{HARNESS}: while the deploy recorded a different one, which is what the smoke must look for"
    );
    let serving_published = running(PUBLISHED);
    let (held, held_writer) = smoke(&diverged, &serving_published);
    assert!(
        !projected_bool(&project_forge_gate_facts(&held), "productionVerified"),
        "{HARNESS}: production serving the PUBLISHED sha is not verification of the DEPLOYED sha"
    );
    assert_eq!(
        held.production_verified, None,
        "{HARNESS}: and no verification is recorded at all"
    );
    let refusal = held.deliverable_rejection.clone().unwrap_or_default();
    assert!(
        refusal.contains(PUBLISHED) && refusal.contains(NOT_DEPLOYED),
        "{HARNESS}: the refusal names both commits, got: {refusal}"
    );
    assert!(
        held_writer
            .holds
            .lock()
            .expect("not poisoned")
            .iter()
            .any(|(story, reason)| story == STORY_ID && reason.contains(PUBLISHED)),
        "{HARNESS}: the refusal is recorded against the story"
    );

    // …and the lineage says the same thing independently of the probe: the verified sha is compared with the
    // DEPLOYED one when a deployment was required, never with the published one.
    let lineage = forge_lineage_error(
        &ForgeGateEvidence {
            deployment_required: Some(true),
            candidate_sha: Some(PUBLISHED.into()),
            qa_passed: Some(true),
            published_sha: Some(PUBLISHED.into()),
            deployed_sha: Some(NOT_DEPLOYED.into()),
            production_verified: Some(true),
            production_verified_sha: Some(PUBLISHED.into()),
            ..ForgeGateEvidence::default()
        },
        "production",
    )
    .expect("verifying the published sha after a deploy breaks the production lineage");
    assert!(
        lineage.contains("expected") && lineage.contains(NOT_DEPLOYED),
        "{HARNESS}: the lineage expects the DEPLOYED sha, got: {lineage}"
    );

    // The reverse case: production running the DEPLOYED sha verifies, even though the published sha differs —
    // so the rule is keyed to the deploy record, not to "sha differs means refuse".
    let running_deployed = running(NOT_DEPLOYED);
    let (verified_diverged, clean_writer) = smoke(&diverged, &running_deployed);
    assert!(
        projected_bool(&project_forge_gate_facts(&verified_diverged), "productionVerified"),
        "{HARNESS}: production running the sha the DEPLOY recorded IS the verification, whatever was published"
    );
    assert_eq!(
        verified_diverged.production_verified_sha.as_deref(),
        Some(NOT_DEPLOYED),
        "{HARNESS}: and it is recorded against the deployed sha"
    );
    assert!(
        clean_writer.holds.lock().expect("not poisoned").is_empty(),
        "{HARNESS}: a verified smoke opens no hold"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 3. CONTROL — A REPOSITORY-ONLY RELEASE LOOKS FOR THE PUBLISHED SHA. With no in-chain deploy there is no
    //    `deployedSha`, and the thing to find in production is what was published. A gate that insisted on the
    //    deployed sha would hold every story the process finishes right after the publish.
    // -----------------------------------------------------------------------------------------------------------
    let ci_deployed = repository_only_release();
    assert_eq!(
        ci_deployed.deployed_sha, None,
        "{HARNESS}: a repository-only release has no deploy record"
    );
    let repository_production = running(PUBLISHED);
    let (repository_verified, repository_writer) = smoke(&ci_deployed, &repository_production);
    assert!(
        projected_bool(
            &project_forge_gate_facts(&repository_verified),
            "productionVerified"
        ),
        "{HARNESS}: production serving the published sha verifies a repository-only release"
    );
    assert_eq!(
        forge_lineage_error(&repository_verified, "production"),
        None,
        "{HARNESS}: and the lineage agrees, because with no deployment required the published sha is expected"
    );
    assert!(
        repository_writer
            .holds
            .lock()
            .expect("not poisoned")
            .is_empty(),
        "{HARNESS}: a verified repository-only release opens no hold"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 4. NEGATIVE — PRODUCTION SERVING NEITHER COMMIT IS NOT VERIFIED. The obvious case, kept because a gate
    //    keyed only to "the deployed sha" could otherwise pass on any answer.
    // -----------------------------------------------------------------------------------------------------------
    let serving_neither = running("1111111111111111111111111111111111111111");
    let (neither, neither_writer) = smoke(&at_smoke_after_deploy(), &serving_neither);
    assert!(
        !projected_bool(&project_forge_gate_facts(&neither), "productionVerified"),
        "{HARNESS}: production serving an unrelated commit is not verification"
    );
    assert!(
        neither_writer
            .holds
            .lock()
            .expect("not poisoned")
            .iter()
            .any(|(story, reason)| story == STORY_ID && reason.contains("1111111111111111")),
        "{HARNESS}: the refusal names what production is actually running"
    );

    // -----------------------------------------------------------------------------------------------------------
    // 5. FAULT — A PRODUCTION THAT CANNOT BE READ IS NOT A VERIFIED PRODUCTION, AND NOT A PASS EITHER. An
    //    unreachable build-info endpoint is the commonest real shape of a failed release, and reading it as
    //    "verified" would complete a release that never happened.
    // -----------------------------------------------------------------------------------------------------------
    let broken = unreadable("https://prod.test/api/build-info answered 503");
    let (unread, unread_writer) = smoke(&at_smoke_after_deploy(), &broken);
    assert_eq!(
        probes(&broken),
        1,
        "{HARNESS}: production was consulted — it simply could not answer"
    );
    assert!(
        !projected_bool(&project_forge_gate_facts(&unread), "productionVerified"),
        "{HARNESS}: an unreadable production is not a verified production"
    );
    assert!(
        unread_writer
            .holds
            .lock()
            .expect("not poisoned")
            .iter()
            .any(|(story, reason)| story == STORY_ID && reason.contains("503")),
        "{HARNESS}: the refusal carries the endpoint's own answer, holds: {:?}",
        unread_writer.holds.lock().expect("not poisoned")
    );

    // And a release with nothing published at all cannot be verified — there is nothing to look for.
    let nothing_published = ForgeGateEvidence {
        deployment_required: Some(true),
        ..ForgeGateEvidence::default()
    };
    let silent = running(PUBLISHED);
    let (nothing, nothing_writer) = smoke(&nothing_published, &silent);
    assert!(
        !projected_bool(&project_forge_gate_facts(&nothing), "productionVerified"),
        "{HARNESS}: a release with nothing published cannot be verified"
    );
    assert!(
        nothing_writer
            .holds
            .lock()
            .expect("not poisoned")
            .iter()
            .any(|(_, reason)| reason.contains("no published commit")),
        "{HARNESS}: and the refusal names the missing commit"
    );
}
