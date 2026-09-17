// ---------------------------------------------------------------------------
// forge-test-stories — seed the board with stories whose purpose is to DRIVE
// Forge and to leave real work behind, so the token spend buys something.
//
//   pnpm forge:test-stories            # dry run (default): says what it would write
//   pnpm forge:test-stories --apply    # write
//
// Each story is bounded, each one exercises a seam that is still unproven in a
// live run, and each one delivers a capability the operator asked for. The
// proof is a test file the story itself creates, so the proof FAILS at base
// (a story whose proof is already green cannot be evidence of work) and passes
// only when the change exists.
//
// IDEMPOTENT: an existing story is reported, never rewritten — history is not a
// scratchpad. PROD ONLY: the same declaration the other board writers use.
// ---------------------------------------------------------------------------
import { createStoryboardStory } from '../db/storyboard'
import { describeControlPlane } from '../lib/execution-target'

type TestStory = {
  id: string
  workstream: string
  operatingSurface: string
  priority: string
  title: string
  goal: string
  scope: string
  acceptance: string
  notes: string
  assayCommands: string
  /** The sprint the deployment defers to. Wave 2 is batch 2. */
  batch?: number
}


const STORIES: TestStory[] = [
  {
    id: 'ENG-FORGE-RECEIPT-KIND-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'A release receipt is recorded by its kind, never by elimination',
    goal:
      'A receipt whose kind is not `deployment` can never set `deploymentReceipt` or `deployedSha`, so a ' +
      'publish or an integration is never recorded as a deployment.',
    scope:
      'workflow_app/forge/agents/role-agents.ts (DevOpsAgent.collect — decide by kind, not by "not ' +
      'production_verification"), workflow_app/tests/forge-receipt-kind.test.ts (new).',
    acceptance:
      'Given a release receipt of kind `integration`, DevOpsAgent records neither deploymentReceipt nor ' +
      'deployedSha. Given kind `deployment`, it records both. Given `production_verification`, it records ' +
      'the production receipt and productionVerifiedSha only. No branch may treat an unlisted kind as a ' +
      'deployment.',
    notes:
      'FOUND 2026-09-16 by reading Neon: artifact 806dd3aa and the durable evidence row it mirrors carried ' +
      '`deploymentReceipt=push:08b569f8…` and `deployedSha=08b569f8…` for a publish that was never a ' +
      'deployment. Root cause: `DevOpsAgent.collect` recorded ANY receipt that was not ' +
      '`production_verification` as a deployment, so `integration` lands there too; and the runner feeds a ' +
      'stored receipt back in as `kind: "deployment"`, so one bad receipt re-armed itself on every pass. ' +
      'The kinds are `deployment | production_verification | integration`.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-receipt-kind.test.ts`',
  },
  {
    id: 'ENG-FORGE-HOLD-VISIBLE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'A story on Hold says why, and where it stopped',
    goal:
      'A held story reports its hold reason and the node it held at, read from the durable hold record, so ' +
      'an operator never has to infer why a chain parked.',
    scope:
      'workflow_app/forge/forge-visibility.ts (add the hold line to the snapshot), ' +
      'workflow_app/tests/forge-hold-visibility.test.ts (new).',
    acceptance:
      'Given an open hold record for a story, the visibility snapshot names the reason and the originating ' +
      'node. Given no hold record, it reports none. A hold record with no reason reads as unknown — never ' +
      'as an empty string that reads like "no problem".',
    notes:
      'WRITTEN 2026-09-16 after a live drive parked at the hold gate: the reason existed only in ' +
      'forge_hold_record and process_events, so answering "why is this story held?" took a database query ' +
      'rather than a look at the board.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-hold-visibility.test.ts`',
  },
  {
    id: 'ENG-FORGE-RUN-SPEND-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    title: 'A story reports what it cost, per lane and in total',
    goal:
      'A story reports the spend of its own runs and lanes from the durable run rows, with unknown spend ' +
      'as null, so the cost of a story is readable without a separate report.',
    scope:
      'workflow_app/forge/forge-visibility.ts (spend block on the snapshot), ' +
      'workflow_app/tests/forge-run-spend.test.ts (new).',
    acceptance:
      'Per-lane spend is read from the durable run rows (tokens_input, tokens_output, cost_usd, ' +
      'cost_widgets, cost_source). A run with no recorded spend reports null and NEVER 0 — an unmeasured ' +
      'zero and a measured zero are different facts. The total is the sum of what is known, or null when ' +
      'nothing is known.',
    notes:
      'WRITTEN 2026-09-16 for the operator rule that a story should return something for the tokens it ' +
      'spends: the spend columns already exist on storyboard_story_run and are already read by ' +
      'db/storyboard.ts, so this makes them visible per story rather than only in aggregate.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-run-spend.test.ts`',
  },
  {
    id: 'ENG-FORGE-QA-VERDICT-VISIBLE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    title: 'The QA verdict is readable per story, and a run that measured nothing says so',
    goal:
      'A story view reports qaPassed with each frozen command and its exit code, and reports NO verdict — ' +
      'rather than a historical one — for a run that measured nothing.',
    scope:
      'workflow_app/forge/forge-visibility.ts (verdict block), workflow_app/forge/forge-evidence-db.ts ' +
      '(read only, no behaviour change), workflow_app/tests/forge-qa-verdict-visible.test.ts (new).',
    acceptance:
      'Given a run whose QA passed, the view reports qaPassed=true with each frozen command and its exit ' +
      'code. Given a run with no rows of its own, it reports no verdict at all rather than a verdict from ' +
      'another run. A failed command is named with its exit code. The verdict carries no SHA.',
    notes:
      'WRITTEN 2026-09-16 to exercise the one seam that is still unproven in a live run: the QA verdict ' +
      'and the durable row it writes. It also pins the rule that a story verdict belongs to its own run ' +
      'only, so this test fails if the scoping regresses.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-qa-verdict-visible.test.ts`',
  },
  // --- WAVE 2 — run these like a stranger would: drive, watch, touch nothing --------------------
  {
    id: 'ENG-FORGE-RELEASE-MARKERS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 2,
    title: 'A resolved release stops reporting the failure it resolved',
    goal:
      'When a release stage succeeds after failing, the story stops reporting the old failure, so a ' +
      'router that reads the failed stage cannot send a resolved story back to the stage that succeeded.',
    scope:
      'workflow_app/forge/db-release-executor.ts (clear the markers with the success it records), ' +
      'workflow_app/tests/forge-release-markers.test.ts (new).',
    acceptance:
      'Given a release stage that failed and then succeeded, the durable evidence reports failureClass ' +
      'null and failedReleaseStage null after the success, and the success flag true. A failure NOT yet ' +
      'followed by a success keeps its markers — the record is corrected, never blanked.',
    notes:
      'FOUND 2026-09-16 while closing ENG-FORGE-RECEIPT-KIND-01: the publish succeeded, origin/main ' +
      'advanced, and the row still read failure_class=ENVIRONMENT with failed_release_stage=PUBLISH. ' +
      'devops_resume_router routes ON failedReleaseStage, so a resolved failure can send a story back to ' +
      'the stage that already succeeded.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-release-markers.test.ts`',
  },
  {
    id: 'ENG-FORGE-LANE-LABELS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 2,
    title: 'Each run is named by its lane, in words',
    goal:
      'A story view names each run by the lane that produced it — Architect, Lead, Smith, QA, DEV_OPS — ' +
      'instead of a raw run_type token.',
    scope:
      'workflow_app/forge/forge-visibility.ts (additive label per run), ' +
      'workflow_app/tests/forge-lane-labels.test.ts (new).',
    acceptance:
      'Every run_type the engine writes maps to a human label. An unknown run_type is reported verbatim ' +
      'rather than guessed, and never as an empty string.',
    notes:
      'WRITTEN 2026-09-16: reading a story history meant translating run_type tokens by hand, so the ' +
      'operator could not see at a glance which lane did what.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-lane-labels.test.ts`',
  },
  {
    id: 'ENG-FORGE-QA-NO-GIT-GUARD-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 2,
    title: 'A guard test keeps QA out of git',
    goal:
      'A test fails if a QA module reads git or carries a sha, so the rule that QA answers only "did the ' +
      'tests pass" cannot regress silently.',
    scope: 'workflow_app/tests/forge-qa-no-git.test.ts (new).',
    acceptance:
      'The test reads the QA modules as text and fails if any of them names a sha field, runs a git ' +
      'command, or checks lineage; it passes on the current tree, and it fails if such a reference is ' +
      'added back.',
    notes:
      'WRITTEN 2026-09-16 after the publish gate demanded a QA-held sha and refused every release ' +
      '(instance 8fc792a6: qa_passed=true, qa_verified_sha=null, publish_succeeded=false). QA answers ' +
      'for the tests; the release path asks for that answer.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-qa-no-git.test.ts`\n' +
      '- `node --import tsx --test workflow_app/tests/forge-qa-seam.test.ts`',
  },
  {
    id: 'ENG-FORGE-REPAIR-BUDGET-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 2,
    title: 'A story reports the repair budget it has left',
    goal:
      'A story view reports the repairs and replans it has used and how many remain, so a story about to ' +
      'exhaust its budget is visible before it holds.',
    scope:
      'workflow_app/forge/forge-visibility.ts (additive), ' +
      'workflow_app/tests/forge-repair-budget.test.ts (new).',
    acceptance:
      'Used and remaining counts are read from the durable counters against the engine caps. An unknown ' +
      'cap reports remaining as null, NEVER as zero — "we do not know" and "none left" are different ' +
      'facts, and only one of them is a hold.',
    notes:
      'WRITTEN 2026-09-16: a story that runs out of repair budget holds, and nothing on the board showed ' +
      'how close it was until it did.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-repair-budget.test.ts`',
  },
  // --- WAVE 3 — the same test again, on the fixed machine: drive, watch, touch NOTHING -----------
  {
    id: 'ENG-FORGE-BATCH-SLICE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 3,
    title: 'The batch release can see the slice it is releasing',
    goal:
      'The sprint release lists every story whose deployment is deferred to its batch, so the stories ' +
      'waiting to be released can actually be found.',
    scope:
      'workflow_app/forge/forge-batch-slice.ts (new, pure predicate), scripts/forge-batch-release.mjs ' +
      '(use it), workflow_app/tests/forge-batch-slice.test.ts (new).',
    acceptance:
      'A story whose deployment is deferred to batch N is IN the slice for batch N even when published_sha ' +
      'is null. A story with no deferral and nothing published is not. A story published but deferred is ' +
      'reported as published-and-undeployed. A row with no batch number belongs to no slice — null, never a ' +
      'guess.',
    notes:
      'FOUND 2026-09-16: forge-batch-release.mjs listed waiting stories by published_sha, but a batch story ' +
      'defers publish as well as deploy (qa_result routes releaseDeferred straight to complete), so ' +
      'published_sha is always null and the tool could never see the very stories waiting for it.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-batch-slice.test.ts`',
  },
  {
    id: 'ENG-FORGE-DOC-QA-RULE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 3,
    title: 'The architecture doc stops describing the QA rule that was deleted',
    goal:
      'The workflow architecture document states the QA rule that the machine actually implements — the ' +
      'verdict is PASS or FAIL, and no sha rides along with it.',
    scope:
      'docs/agent/WORKFLOW-ARCHITECTURE.md (the four places that still state the sha-era invariant), ' +
      'workflow_app/tests/forge-qa-doc-rule.test.ts (new fence test).',
    acceptance:
      'The document no longer states that a QA pass freezes a candidate sha, that the candidate sha is ' +
      'carried into release, or that the candidate module QA verified is re-checked. A fence test reads the ' +
      'document and fails if that wording returns.',
    notes:
      'FOUND 2026-09-16: the doc still told a reader to compare the frozen candidateSha against what QA ' +
      'verified — a check that no longer exists — at lines 114, 142, 385 and 521. Lanes read these docs.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-qa-doc-rule.test.ts`',
  },
  {
    id: 'ENG-FORGE-FAILURE-LABEL-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 3,
    title: 'A release failure keeps the class its own stage recorded',
    goal:
      'When a release stage fails and records what failed, a later classifier may add its own label as ' +
      'metadata but may not replace the stage-accurate class.',
    scope:
      'workflow_app/forge/forge-role-mapping.ts (the failure_classifier branch), ' +
      'workflow_app/tests/forge-failure-label.test.ts (new).',
    acceptance:
      'Given a failed release stage that recorded PUBLISH_CONFLICT with failedReleaseStage PUBLISH, the ' +
      'failure class after the classifier runs is still PUBLISH_CONFLICT, and the classifier label is ' +
      'preserved as metadata. With no stage-recorded class, the classifier label stands as it does today.',
    notes:
      'FOUND 2026-09-16 on ENG-FORGE-RECEIPT-KIND-01: the publish failed and the executor recorded ' +
      'failureClass=PUBLISH_CONFLICT with failedReleaseStage=PUBLISH, then the classifier replaced it with ' +
      'ENVIRONMENT. The router reads failedReleaseStage, so a wrong label is a wrong record of why.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-failure-label.test.ts`',
  },
  {
    id: 'ENG-FORGE-QA-CONSISTENCY-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 3,
    title: 'A QA run and its verdict disagreeing is reported, not discovered by hand',
    goal:
      'A read-only check reports when a QA run row and the durable verdict disagree, so the class of bug ' +
      'found on 2026-09-16 is caught by the doctor instead of by a person reading rows.',
    scope:
      'workflow_app/forge/forge-qa-consistency.ts (new, pure), scripts/forge-doctor.ts (report the line), ' +
      'workflow_app/tests/forge-qa-consistency.test.ts (new).',
    acceptance:
      'Given a run status and a verdict, the check reports agree or names both values. A run with no ' +
      'verdict reports unknown — never agree. It writes nothing: it is a report, not a repair.',
    notes:
      'WRITTEN 2026-09-16: every QA run that night recorded Hold while its verdict was PASS, because the ' +
      'run finalizer demanded a sha QA does not own. Nothing compared the two records; a reading person did.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-qa-consistency.test.ts`',
  },
  // --- WAVE 4 — the security findings, as stories. APP surface, not the engine --------------------
  {
    id: 'SEC-MEDIA-DOC-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'NEXUS',
    priority: 'High',
    batch: 4,
    title: 'An executed contract is not served to whoever holds its id',
    goal:
      'Downloading a document requires an authenticated portal session, and the decision is ONE named ' +
      'function the tests interrogate.',
    scope:
      'lib/auth/document-access.ts (new, pure decision), app/api/media/documents/[id]/route.ts (call it), ' +
      'workflow_app/tests/document-access.test.ts (new).',
    acceptance:
      'No session → no bytes, and the response is 401 rather than a 404 that hides the rule. An ' +
      'authenticated portal session → bytes, with the decision made by the named function. A non-document ' +
      'media type stays 404. The function is pure: session facts in, allow|deny with a reason out, so the ' +
      'rule cannot disagree with itself in two places.',
    notes:
      'FOUND 2026-09-16 by a code review, then verified by reading the route: it validated the UUID and ' +
      'served any row with media_type = document — no session, no authority, no publication gate — while ' +
      'its sibling under /api/media/[id] at least requires a session. What lands in that table with that ' +
      'type: db/signature-reconciliation.ts:154 (the signed artifact from BoldSign) and ' +
      'db/issued-document.ts:158 (issued PDFs). HONEST BOUNDARY: this story adds the session gate and the ' +
      'pure decision; binding a document to its OWNING record authority is bigger (the media row may carry ' +
      'no owner link) and belongs in its own story — say so in the notes rather than implying this story ' +
      'solved it. The sibling conflating authentication with authorization is NOT fixed here either.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/document-access.test.ts`',
  },
  {
    id: 'SEC-SILENT-CATCH-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 4,
    title: 'A swallowed catch cannot pass as an empty success',
    goal:
      'Silent-failure detection blocks instead of filing a nightly note, and the routes that return an ' +
      'empty success from a bare catch are fixed rather than exempted.',
    scope:
      'agent-runtime/silent-failure-patterns.ts (the detector), the blocking entry point (script/lint), the ' +
      'six routes named in the notes, workflow_app/tests/silent-failure-gate.test.ts (new).',
    acceptance:
      'The check exits non-zero and names file and line when a handler returns an empty success — null, [], ' +
      'or {rows: [], total: 0} — out of a catch. A catch that reports the error and returns an error ' +
      'response is NOT flagged. The known offenders are fixed, not exempted: an exemption list is the same ' +
      'lie moved to a new file.',
    notes:
      'FOUND 2026-09-16 by a code review, and it matches AGENTS.md, which calls a swallowed catch a defect, ' +
      'and agent-runtime/silent-failure-patterns.ts, which already detects it. Known offenders, all bare ' +
      'catches returning empty success with a comment calling it safe: ' +
      'app/api/portal/clients/[personId]/route.ts:33, .../clients/[personId]/history/route.ts:37, ' +
      '.../clients/[personId]/relationship-channels/route.ts:69, app/api/portal/clients/agents/route.ts:28, ' +
      'app/api/portal/issues/route.ts:56, app/api/portal/relationship-evidence-review/route.ts:51. In a CRM ' +
      'an empty list reads as "this client has nothing", which is indistinguishable from the truth and ' +
      'would never be reported as an outage.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/silent-failure-gate.test.ts`',
  },
  {
    id: 'SEC-ROUTE-MANIFEST-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 4,
    title: 'Every route declares its authority, and drift fails',
    goal:
      'Generate a manifest of every route handler with the authority it requires, and fail on a handler ' +
      'that decides nothing and on drift between the declaration and the code.',
    scope:
      'scripts/route-authority-manifest.ts (new, generator + check), workflow_app/tests/route-authority.test.ts (new).',
    acceptance:
      'Every file under app/api/**/route.ts appears in the manifest. A route with no authority decision ' +
      'must be declared UNGUARDED with a reason and the story that will fix it. A route that is neither ' +
      'guarded nor declared fails. The manifest is GENERATED, never hand-edited — a hand-kept table drifts ' +
      'silently, which is the bug this exists to catch.',
    notes:
      'WRITTEN 2026-09-16 after a code review found three authorization holes by reading the code, one of ' +
      'them Critical. A table plus a conformance test catches the class mechanically, the same way the ' +
      'dead-citation lint is what makes agent evidence auditable. The highest-leverage item in that review: ' +
      'it would have caught the documents route, the sibling route, and the inert middleware branch.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/route-authority.test.ts`',
  },
  {
    id: 'AUTH-CAPABILITIES-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'NEXUS',
    priority: 'Medium-High',
    batch: 4,
    title: 'The middleware stops describing a capability check that never runs',
    goal:
      'Delete the edge branch that reads a claim nothing stamps, and make the file describe what it ' +
      'actually does. Stamping the claim is a separate story, named, not implied.',
    scope:
      'middleware.ts, lib/auth/middleware-policy.ts, workflow_app/tests/middleware-capabilities.test.ts (new).',
    acceptance:
      'No branch remains on a claim that no writer produces, and the comment describes what the file does. ' +
      'A test fails if a capability check reappears without a writer for that claim. The deletion is ' +
      'recorded in the notes as the decision this story took.',
    notes:
      'FOUND 2026-09-16 by a code review, then verified: capabilities is read at middleware.ts:79 and ' +
      'consumed at lib/auth/middleware-policy.ts:44, and written NOWHERE. It arrived with bf45e1aa (AUTH-02 ' +
      'Portal authorization, 2026-08-22): the design stamped a capability snapshot at sign-in, while the jwt ' +
      'callback stamps only sub and provider — so the Edge layer has been inert since it landed and every ' +
      'authenticated session passes it. A security file that claims a control it does not have is how the ' +
      'next reader stops checking. Related opening for a later story: the sibling media route checks only ' +
      'token.sub and never the authority it names.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/middleware-capabilities.test.ts`',
  },
  // --- WAVE 5 — correctness: fields nobody writes, lanes nothing can satisfy, records that disagree ---
  {
    id: 'ENG-FORGE-DEPLOY-NOMECH-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 5,
    title: 'Forge records no deployment it did not perform',
    goal:
      'A story never enters a lane that cannot be satisfied: deployment is not something Forge does, so a ' +
      'story whose deployment is deferred completes, and one that demands a deployment is held AT THE ' +
      'DECISION with a named reason instead of walking into a lane with no producer.',
    scope:
      'workflow_app/forge/agent-runtime-role-runner.ts (the deploy entry), workflow_app/forge/forge-facts.ts ' +
      '(the decision input), workflow_app/tests/forge-deploy-entry.test.ts (new).',
    acceptance:
      'A story whose deployment is deferred to its batch completes without any deployment receipt. A story ' +
      'whose deployment is NOT deferred and has no configured producer is held at the decision, naming ' +
      'what is missing, and never enters the deploy lane. No run row or artifact may record a deployment, ' +
      'deployed sha, or deployment receipt unless a deployment actually happened.',
    notes:
      'FOUND 2026-09-16 by measurement: the deploy lane requires a devops-receipt ' +
      '(deploymentReceipt | productionVerificationReceipt | deploymentDeferredToBatch) and NOTHING in the ' +
      'repo can mint one — releaseReceiptFromDeploymentSignal has no callers, and release-operations.ts ' +
      'has no deploy operation. So every release-bearing story held at deploy forever, and when that lane ' +
      'failed the chain routed to repair_devops, which needs the same impossible receipt. Observed live: ' +
      'instance 8fc792a6 held with "role did not deliver devops-receipt" twice; a false ' +
      'deploymentReceipt=push:08b569f8 was also found on a row, minted from a PUBLISH receipt months ' +
      'earlier. The deploy script itself is scripts/vercel-deploy-prod.sh, run by the captain at sprint ' +
      'release — not by Forge.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-deploy-entry.test.ts`',
  },
  {
    id: 'ENG-FORGE-START-BASE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 5,
    title: 'A lane records the base it started from',
    goal:
      'START carries its own base: every code-writing run records the commit HEAD stood on when the lane ' +
      'began, so the scope gate reads a fact instead of deriving one.',
    scope:
      'the lane start path (agent-runtime adapters / repositories), db/storyboard.ts (the run writer), ' +
      'workflow_app/tests/run-base-commit.test.ts (new).',
    acceptance:
      'Every run of a code-writing lane carries base_commit_hash — the commit HEAD stood on when that lane ' +
      'started — and the scope check prefers it over any derived base. A lane that cannot read its base ' +
      'records none rather than a guess, and the reader falls back rather than inventing one.',
    notes:
      'WRITTEN 2026-09-16: base_commit_hash is NULL on every run row in PROD because nothing supplies it, ' +
      'so the scope gate had to derive a base and derived the wrong one (origin/main, a whole sprint ' +
      'behind — see docs/agent/decisions/start-ruling-end.md). The value is already read at lane start as ' +
      'headAtStart; it simply never reaches the row. START, RULING, END: start writes its base, end writes ' +
      'its commit.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/run-base-commit.test.ts`',
  },
  {
    id: 'ENG-FORGE-ARTIFACT-RULING-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 5,
    title: 'An artifact carries the ruling, never a second opinion',
    goal:
      'A lane artifact records the verdict that was ruled for its run, so an artifact cannot report Hold ' +
      'for a run whose ruling was PASS.',
    scope:
      'the artifact writers (agent-runtime base adapter, db/forge-artifact.ts), ' +
      'workflow_app/tests/artifact-verdict.test.ts (new).',
    acceptance:
      'For a given run, the artifact verdict and the durable ruling agree, or the artifact says it has no ' +
      'verdict. A test proves an artifact cannot be written with a verdict that contradicts its run. The ' +
      'artifact summary may carry detail; it may not carry a second answer.',
    notes:
      'FOUND 2026-09-16 by reading rows: run 5a1494f6 produced an artifact with verdict=Hold whose own ' +
      'summary read "Assay PASS | candidate=(none) | verified=(none) | … -> exit 0", while the durable ' +
      'verdict for that run was qa_passed=true — three records of one ruling, disagreeing. The root cause ' +
      'is fixed (703d3d63), but the artifact layer still mirrors a status rather than the ruling.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/artifact-verdict.test.ts`',
  },
  {
    id: 'ENG-FORGE-HOTFIX-LANE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 5,
    title: 'A HOTFIX story reaches its first lane with the contract it needs',
    goal:
      'A HOTFIX story actually starts: the route and the lane policy agree about who authors the contract ' +
      'a Lead reads, so the story does not die on its first lane.',
    scope:
      'the hotfix route in the FORGE_SDLC definition and/or agent-runtime/lane-policy.ts, ' +
      'workflow_app/tests/forge-hotfix-route.test.ts (new).',
    acceptance:
      'A HOTFIX story reaches lead_pre with the contract the lane policy requires, and a test pins the ' +
      'route so the two cannot diverge again. The decision — the hotfix branch runs the Architect — is ' +
      'recorded in the notes, with the rejected alternative named.',
    notes:
      'FOUND 2026-09-16 by running one: hotfix_architecture_check routes to lead_pre when ' +
      'architectureSuspect is false (the default), which skips the Architect, while ' +
      'agent-runtime/lane-policy.ts:203 refuses a Lead without the frozen Architect contract — ' +
      '"Lead requires the frozen Architect contract. Write the brief or run the Architect lane first." ' +
      'So a HOTFIX story dies at its first lane, every time. DECISION TAKEN BY THIS STORY: the hotfix ' +
      'branch runs the Architect (one contract source, one author). REJECTED: letting the lane policy ' +
      'accept a lead-authored brief for hotfixes, because that is a second author of the same fact.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-hotfix-route.test.ts`',
  },
  {
    id: 'ENG-FORGE-COLUMN-WRITER-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 5,
    title: 'A field nothing writes is a lie',
    goal:
      'Every column of the run and evidence tables is classified as written-by-something, drop-it, or ' +
      'kept-with-a-reason — so a column can no longer describe a machine that does not exist.',
    scope:
      'docs/agent/manifest/ (the audit table), scripts/column-writer-audit.ts (new generator), ' +
      'workflow_app/tests/column-writer-audit.test.ts (new).',
    acceptance:
      'A generated audit lists every column of storyboard_story, storyboard_story_run and ' +
      'forge_workflow_evidence with a classification: WRITTEN (naming the writer), DEAD-DROP, or ' +
      'DEAD-KEEP with the reason it stays. No column is unclassified. A test fails when a new column ' +
      'appears that the audit does not classify. The audit is generated, never hand-kept.',
    notes:
      'WRITTEN 2026-09-16 after two columns lied in one night: architect_brief_updated_at was null on ' +
      'every engine-written brief (fixed in 2936c5e8) and base_commit_hash is null on every run row in ' +
      'PROD. qa_verified_sha is a third: dead by rule since QA stopped owning a sha, still in the schema ' +
      'and still read by forge-consistency as though it meant something. A column only one of its ' +
      'writers maintains is worse than a missing one, because it reads as a fact.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/column-writer-audit.test.ts`',
  },
  {
    id: 'ENG-FORGE-BATCH-RECEIPT-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 5,
    title: 'A sprint release records what it carried',
    goal:
      'The batch release leaves a durable receipt naming the stories it published and deployed and the ' +
      'commit it released, so a sprint can be audited afterwards instead of remembered.',
    scope:
      'scripts/forge-batch-release.mjs and its record path, db/forge-workflow-evidence.ts (the receipt ' +
      'fields), workflow_app/tests/forge-batch-receipt.test.ts (new).',
    acceptance:
      'After a batch is released, a durable record names the batch, the stories it carried, the commit ' +
      'released, and when — written from the ACTUAL release result, never asserted. A batch with no ' +
      'release has no receipt. A test proves the receipt is built from the release outcome and cannot be ' +
      'written without one.',
    notes:
      'WRITTEN 2026-09-16: the captain runs the sprint release by hand (scripts/vercel-release-prod.sh ' +
      'builds and deploys from local main after a clean-tree check), and nothing records what a batch ' +
      'carried. forge-batch-release.mjs lists the slice and says so: "refuses to pretend a deployment ' +
      'happened". That is the right refusal; this story adds the other half — a receipt for what the ' +
      'release actually did. Related: this is the missing end of the batch_deploy deferral that the chain ' +
      'already records per story.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-batch-receipt.test.ts`',
  },
]



/**
 * Write one story as Planned. A story that already exists is REPORTED, not rewritten: its notes are the
 * record of what happened, not a place to keep a seed file in sync.
 */
async function recordOne(story: TestStory, apply: boolean): Promise<void> {
  if (!apply) {
    console.log(`[dry-run] would record ${story.id} as Planned — ${story.title}`)
    return
  }
  try {
    const created = await createStoryboardStory({
      id: story.id,
      workstream: story.workstream,
      operatingSurface: story.operatingSurface,
      title: story.title,
      priority: story.priority,
      status: 'Planned',
      notes: story.notes,
      batch: story.batch ?? null,
      goal: story.goal,
      scope: story.scope,
      dependencies: null,
      preconditions: null,
      architectBrief: null,
      contextRefs: null,
      acceptanceCriteria: story.acceptance,
      postconditions: null,
      testMode: 'SCOPED',
      assayCommands: story.assayCommands,
      packetSha: null,
      completion: 0,
      rollup: false,
      plannedStartAt: null,
      actualStartAt: null,
      completedAt: null,
    })
    console.log(`[created] ${created.id} — Planned, completion 0`)
    return
  } catch (error) {
    const message = (error as Error).message
    if (!/already exists/i.test(message)) throw error
  }
  console.log(`[kept] ${story.id} already exists — left exactly as it is`)
}

async function main(): Promise<void> {
  const apply = process.argv.includes('--apply')
  const declared = describeControlPlane(process.env)
  if (declared.target !== 'prod') {
    console.error(
      `forge-test-stories: refusing to write to ${String(declared.target ?? 'an undeclared')} ` +
        'environment — the board lives in PROD. Declare APP_ENV=production.',
    )
    process.exit(2)
  }
  // The ready gate refuses a story with no acceptance criteria or no assay plan, so a seeded story that is
  // missing either could never start. Fail here instead of leaving a story that cannot run.
  for (const story of STORIES) {
    if (!story.acceptance.trim()) throw new Error(`${story.id}: no acceptance criteria — the ready gate refuses it`)
    if (!story.assayCommands.trim()) throw new Error(`${story.id}: no assay plan — the ready gate refuses it`)
  }
  for (const story of STORIES) await recordOne(story, apply)
  if (!apply) console.log('dry run only: re-run with --apply to write')
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
