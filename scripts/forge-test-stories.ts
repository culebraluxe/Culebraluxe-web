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
    batch: 91,
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
    batch: 91,
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
    batch: 91,
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
    batch: 91,
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
    batch: 92,
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
    batch: 92,
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
    batch: 92,
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
    batch: 92,
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
    batch: 92,
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
    batch: 92,
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
  // --- WAVE 6 — RECORDS AND WRITER DISCIPLINE: one writer per fact, no tree-era residue -----------
  {
    id: 'ENG-FORGE-WRITER-MAP-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 93,
    title: 'One writer per fact, and a map that proves it',
    goal:
      'Every durable fact in the run, story and evidence tables names exactly one writer in a GENERATED ' +
      'map, so a second writer is caught by a check rather than by a night of holding finished work.',
    scope:
      'scripts/fact-writer-map.ts (new, generator + check), docs/agent/manifest/FACT-WRITERS.md (generated), ' +
      'workflow_app/tests/fact-writer-map.test.ts (new).',
    acceptance:
      'The map names each durable fact with its writer (file + symbol) or marks it READ-ONLY with the ' +
      'writer named elsewhere. A fact with two writers FAILS unless the map records it as multi-writer ' +
      'with the reason it must be. A new fact appears automatically or the check fails. Generated, never ' +
      'hand-kept.',
    notes:
      'A QA verdict was recorded FOUR ways on 2026-09-16 and disagreed two ways: run row Hold, artifact ' +
      'Hold, its own summary "Assay PASS", durable evidence qa_passed=true. One writer per fact is already ' +
      'the rule in AGENTS.md; this makes it checkable, as the dead-citation lint made evidence auditable.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/fact-writer-map.test.ts`',
  },
  {
    id: 'ENG-FORGE-ARTIFACT-RESIDUE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 93,
    title: 'No record carries a worktree path or a tree-era field',
    goal:
      'Records describe the machine that exists: no artifact, evidence row or run detail names a per-lane ' +
      'worktree, a pinned worktree base, or any field that only made sense when lanes had trees.',
    scope:
      'the record writers (base adapter, db/forge-artifact.ts, forge run detail), a one-off sweep of ' +
      'existing rows, workflow_app/tests/no-tree-residue.test.ts (new).',
    acceptance:
      'A generated scan finds no worktree path (Culebraluxe-worktrees, /worktrees/, worktreePath) and no ' +
      'tree-era field in records written after this story. Older rows are swept once or marked LEGACY with ' +
      'the date written. A test fails when a writer reintroduces one.',
    notes:
      'An artifact from an older generation still names ' +
      '`/Users/…/Culebraluxe-worktrees/eng-qa-single-verdict-01-…`, months after NO TREES removed the ' +
      'worktrees — the record outlived the thing it described. `worktreePath` is still passed as a label ' +
      'in the runner run identities today. Trees are gone; the vocabulary is not.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/no-tree-residue.test.ts`',
  },
  {
    id: 'ENG-FORGE-STATUS-WRITERS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 93,
    title: 'A story status has sanctioned writers and no others',
    goal:
      'Story status and completion are written only by the run lifecycle, the board editor and the one ' +
      'sanctioned hold marker — and a fence fails when a third path starts writing them.',
    scope:
      'db/storyboard.ts (finishStoryRun, updateStoryboardStory, setStoryboardStatus), ' +
      'db/forge-story-state.ts, workflow_app/tests/story-status-writers.test.ts (new).',
    acceptance:
      'Every write of storyboard_story.status or .completion names a sanctioned writer; a scan fails on a ' +
      'new one. The pair rule holds: 100 only with Complete, and a non-Complete run never writes 100. The ' +
      'hold marker is either sanctioned in the map or moved behind the lifecycle.',
    notes:
      'An invented completion (99) and a migration-182 constraint crash both came from a writer deciding ' +
      'the pair itself on 2026-09-16. The pair rule is fixed; who may write it was never pinned, and ' +
      'db/forge-story-state.ts marks a story HOLD straight from the runner.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/story-status-writers.test.ts`',
  },
  // --- WAVE 7 — ROBUSTNESS AND OPERATIONS: liveness, reaping, and a door out of a hold ------------
  {
    id: 'ENG-FORGE-HEARTBEAT-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 94,
    title: 'Liveness is a timer, not the model reporting',
    goal:
      'A running lane is provably alive because the RUNNER heartbeats on a timer, not because the model ' +
      'remembered to report progress.',
    scope:
      'the lane execution path (agent-runtime/repositories.ts, the runner), db/agent-work.ts ' +
      '(updateAgentWorkProgress), workflow_app/tests/lane-heartbeat.test.ts (new).',
    acceptance:
      'While a lane runs, its work item heartbeat advances on a fixed interval regardless of what the ' +
      'model says or does not say. A lane quiet for longer than the reap window still holds a fresh ' +
      'heartbeat. A test proves the heartbeat advances with no caller reporting anything.',
    notes:
      'updateAgentWorkProgress has exactly two call sites, both inside lane activity, while ' +
      'scripts/forge-story-reset.ts reaps at --stale-minutes 15 — so a quiet live lane and a dead worker ' +
      'look identical. Two rows from one night sat in `claimed` until closed by hand; the ledger holds 41 ' +
      '`interrupted`.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/lane-heartbeat.test.ts`',
  },
  {
    id: 'ENG-FORGE-REAP-GUARD-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 94,
    title: 'The reaper cannot kill a lane that is alive',
    goal:
      'Reaping is a decision about a fact (is the worker gone) rather than a guess from elapsed time, and ' +
      'a lane that is alive can never be reaped.',
    scope:
      'scripts/forge-story-reset.ts (the clean/recover paths) and whatever reports worker liveness, ' +
      'workflow_app/tests/reap-guard.test.ts (new).',
    acceptance:
      'A claim is released only when the worker that holds it is provably gone. A live worker survives any ' +
      'amount of elapsed time; a dead worker is released without waiting for a timeout. A test proves both ' +
      'directions with the elapsed time held constant.',
    notes:
      'The current guard is time-based and says so: only claims older than --stale-minutes (15) are ' +
      'touched, "so a live peer survives" — which is true only if the peer is reporting. With WAVE 7 ' +
      'heartbeat in place this becomes checkable rather than hopeful.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/reap-guard.test.ts`',
  },
  {
    id: 'ENG-FORGE-RESUME-DOOR-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 94,
    title: 'A held or failed run has a door out',
    goal:
      'Any instance sitting on a hold or a failed lane can be resumed at a named node or cancelled, from ' +
      'one operator door, and the decision is recorded as the reason it moved.',
    scope:
      'workflow_app/forge/forge-hold-resolve.ts (extend to a run with no open hold task), a script or board ' +
      'action, workflow_app/tests/resume-door.test.ts (new).',
    acceptance:
      'Given a run stopped at a hold OR at a failed lane task, the door can resume it at a named node or ' +
      'cancel it, and records who, when and why. A run that cannot be resumed says what is missing instead ' +
      'of failing silently. Resuming never fabricates a ruling: it moves the token, it does not write a ' +
      'verdict.',
    notes:
      'Measured 2026-09-16: story 4 sat with a ready task at a lane that had errored and the hold door ' +
      'could not touch it (resolveForgeHold requires an open hold TASK); the workaround was to complete ' +
      'the dead lane by hand and then cancel. Five stories are on Hold today, three of them from testing, ' +
      'with no door to clear them.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/resume-door.test.ts`',
  },
  // --- WAVE 8 — TESTS THAT WOULD HAVE CAUGHT TONIGHT: black box, not glass box ---------------------
  {
    id: 'ENG-FORGE-REPLAY-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 95,
    title: 'Replay a recorded generation and assert what it decided',
    goal:
      'Take the durable rows of a finished generation and replay the routing decisions offline, with no ' +
      'model — so a change to the middle cannot silently re-route work that already ran.',
    scope:
      'a new replay harness (workflow_app/forge/replay.ts + a runner), fixtures built from real rows, ' +
      'workflow_app/tests/forge-replay.test.ts (new).',
    acceptance:
      'Given a recorded generation (runs, evidence, engine tasks), the replay reproduces the decisions ' +
      'that were taken — pass, fail, deferred, held — and fails loudly when the code would now decide ' +
      'differently. The fixtures are built from REAL rows, never hand-written ideals. No model is called.',
    notes:
      'Tonight three separate defects changed what the machine decided about work that had already run: ' +
      'the sha conjunct, the scope base, the candidate equality. Each was found by running a story and ' +
      'reading rows afterwards; a replay fixture turns one of those nights into a regression test that ' +
      'costs nothing to run. This is the cheap half of black-box testing.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-replay.test.ts`',
  },
  {
    id: 'ENG-FORGE-BLACKBOX-STORY-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 95,
    title: 'One story, end to end, asserted only on its records',
    goal:
      'A black-box test drives a real story through its whole chain without reaching inside it, and ' +
      'asserts only what the records say: started, ruled, ended.',
    scope:
      'a scratch-database harness plus one story driven end to end, ' +
      'workflow_app/tests/forge-blackbox-story.test.ts (new).',
    acceptance:
      'The test drives one story from start to completion against a scratch database and asserts: one run ' +
      'per lane with a start and an end, one ruling per measuring lane, a story status that matches the ' +
      'ruling, and a QA run whose recorded status equals its verdict. It touches no internals, stubs no ' +
      'gate, and fails if any of those records disagree.',
    notes:
      'Tonight proved the gap: 736 engine tests, 93 forge test files, and every one of the night’s bugs ' +
      'lived where no test looked — in what a RUN records. Glass-box tests check that a function returns ' +
      'what it says; this checks that the machine told the truth about what it did.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-blackbox-story.test.ts`',
  },
  {
    id: 'ENG-FORGE-MIDDLE-AUDIT-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 95,
    title: 'Every middle step is justified or deleted',
    goal:
      'Inventory every step between START and END that can decide, refuse or override, and either ' +
      'justify it in writing or delete it — the captain rule made checkable.',
    scope:
      'docs/agent/decisions/middle-steps.md (new inventory), the removals it implies, ' +
      'workflow_app/tests/middle-step-inventory.test.ts (new).',
    acceptance:
      'Each middle step is listed with what it can override, what it costs when it is wrong, and a ' +
      'JUSTIFIED or DELETE verdict. Anything marked DELETE is gone in the same change. A test fails when ' +
      'a step that can refuse a ruling exists without an entry in the inventory.',
    notes:
      'The captain: "you get Start, RULING, END — anything more you have to justify very very very ' +
      'strongly." Tonight deleted three middle deciders (the sha conjunct, the scope base’s remote ' +
      'fallback, the candidate equality) and each one had held finished work. Known candidates for the ' +
      'inventory: the deliverable re-ask loop, the classifier label, the QA review lane, the repair ' +
      'budgets, the observer’s scope check.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/middle-step-inventory.test.ts`',
  },
  // --- WAVE 9 — SIMPLICITY: the steps that decide something nobody asked them to decide -----------
  {
    id: 'ENG-FORGE-QA-REVIEW-LANE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 96,
    title: 'The QA review lane either joins the ruling or goes',
    goal:
      'There is one answer to "did the tests pass". The independent review step either becomes part of ' +
      'that ruling or is deleted — it may not be a third opinion sitting beside it.',
    scope:
      'the qa_policy / qa_review route in the FORGE_SDLC definition, workflow_app/forge/forge-facts.ts ' +
      '(qaReviewRequired / qaReviewPassed), workflow_app/tests/qa-review-lane.test.ts (new).',
    acceptance:
      'Either the review becomes a required input to the single QA ruling (and a review failure is a QA ' +
      'failure with the same vocabulary), or the lane and its two facts are deleted. The decision is ' +
      'recorded in the notes with what it costs and what it protects. No third verdict token exists.',
    notes:
      'qa_policy routes on qaReviewRequired and qaReviewResult produces qaReviewPassed, so a story can be ' +
      'failed by a step that never runs a test — a second answer to the same question, which is the shape ' +
      'tonight cost us four holds over. The captain’s rule: the verdict is the tests. Written up rather ' +
      'than deleted on my own judgement because it may carry a real protection I cannot see.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/qa-review-lane.test.ts`',
  },
  {
    id: 'ENG-FORGE-REASK-BUDGET-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 96,
    title: 'The deliverable re-ask is bounded, explained and visible',
    goal:
      'The loop that re-runs a lane for a missing deliverable is bounded, spends money knowingly, and ' +
      'says in the record that it re-asked — so a retry is never mistaken for a second attempt at the work.',
    scope:
      'the attempt loop in workflow_app/forge/agent-runtime-role-runner.ts, the budget env ' +
      '(FORGE_DELIVERABLE_RETRIES), workflow_app/tests/deliverable-reask.test.ts (new).',
    acceptance:
      'The budget is a number in one place, the record shows each attempt and what was missing, and ' +
      'exhaustion holds with the named miss. An untrusted or missing budget falls back to one attempt, ' +
      'never to unlimited. The record distinguishes a re-ask from a repair.',
    notes:
      'Tonight two lanes spent a second attempt each (smith on story 4) and the only trace was "after 2 ' +
      'attempt(s)" in an error string. It is the clearest middle step that spends money: 1 + retries per ' +
      'lane per story, invisible in the tokens view.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/deliverable-reask.test.ts`',
  },
  {
    id: 'APP-SPRINT-DOOR-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'NEXUS',
    priority: 'High',
    batch: 96,
    title: 'A story can be assigned to a sprint from the board',
    goal:
      'Assigning a story to a sprint — and so deferring its deployment to the release — is an operator ' +
      'action in the board, not a SQL statement typed by a helper.',
    scope:
      'the board write path (db/storyboard.ts, the portal action), the batch fields (batch, batch_deploy), ' +
      'workflow_app/tests/sprint-door.test.ts (new).',
    acceptance:
      'An operator can put a story in a sprint and mark its deployment deferred, singly or as a group, and ' +
      'undo it. The door validates the sprint number, records who did it, and refuses to defer a story ' +
      'that is already released. No helper script is required to run a sprint.',
    notes:
      'WRITTEN 2026-09-16: I set batch and batch_deploy for three waves with a throwaway script in /tmp, ' +
      'which is not a door — it is me being the API. The engine half already works (a flagged story ' +
      'records an honest deferral and completes at the release boundary); this is the missing operator half ' +
      'of the captain’s still-unsolved sprint management. Pairs with ENG-FORGE-BATCH-RECEIPT-01.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/sprint-door.test.ts`',
  },
  // --- WAVE 10 — THE APP, THE RELEASE, AND THE TOOLS WE ALREADY OWN ------------------------------
  {
    id: 'APP-MEDIA-LENGTH-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'NEXUS',
    priority: 'Medium',
    batch: 97,
    title: 'A download declares the length of the bytes it sends',
    goal:
      'Content-Length is the length of the buffer being sent, never a number from a column that can ' +
      'disagree with it.',
    scope:
      'app/api/media/documents/[id]/route.ts and any sibling that sets Content-Length, ' +
      'workflow_app/tests/media-content-length.test.ts (new).',
    acceptance:
      'The header is derived from the bytes in the response. A row whose file_size disagrees with its ' +
      'bytes still streams completely. A test proves the mismatch case, which is the case that breaks.',
    notes:
      'FOUND 2026-09-16 by reading `app/api/media/documents/[id]/route.ts:72` — ' +
      '`headers.set(Content-Length, String(row.file_size))` while the body comes from `file_data`. When ' +
      'they disagree the client truncates or hangs, and the row is the only thing that knew.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/media-content-length.test.ts`',
  },
  {
    id: 'APP-INPUT-VALIDATION-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'NEXUS',
    priority: 'Medium-High',
    batch: 97,
    title: 'Input validation is declared, not hand-rolled per handler',
    goal:
      'One declared way to validate a request body, query and params, so a route cannot quietly accept ' +
      'whatever it is handed.',
    scope:
      'a chosen validation mechanism (a library or a small in-repo module), the guard seam the routes ' +
      'already use, workflow_app/tests/input-validation.test.ts (new).',
    acceptance:
      'A route declares its accepted shape and an invalid request is refused with a named reason, not ' +
      'coerced. The mechanism is ONE — a second hand-rolled validator in a route fails the check. The ' +
      'decision (library or in-repo) is recorded in the notes with what it costs.',
    notes:
      'WRITTEN 2026-09-16: there is no validation library in the project (0 hits for zod), so 27 routes ' +
      'validate by hand — `intParam`, a UUID regex, `clip()` helpers. That is how the documents route came ' +
      'to validate a UUID and nothing else.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/input-validation.test.ts`',
  },
  {
    id: 'APP-DIAGNOSTIC-THROTTLE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'NEXUS',
    priority: 'Low-Medium',
    batch: 97,
    title: 'The anonymous diagnostics cannot be flooded',
    goal:
      'The two anonymous write endpoints stay deliberately anonymous AND stop being able to fill the ' +
      'error table.',
    scope:
      'app/api/portal/move-trace/route.ts, app/api/portal/client-error/route.ts, ' +
      'workflow_app/tests/diagnostic-throttle.test.ts (new).',
    acceptance:
      'Each endpoint bounds its writes per source per window and per body size, refuses over the bound ' +
      'with a plain response, and records the refusal as a count rather than a row per attempt. ' +
      'Anonymity is preserved — no session is required, by design.',
    notes:
      'READ 2026-09-16: both are documented as anonymous and deliberately tiny (no comment, no user, no ' +
      'free text), which is a defensible design — but nothing bounds the rate, so anyone can write rows ' +
      'into app_error at will and drown the signal that got the board debugged in the first place.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/diagnostic-throttle.test.ts`',
  },
  {
    id: 'APP-CI-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 97,
    title: 'The tests run without anyone remembering to run them',
    goal:
      'A push runs the test tiers automatically, so "green" stops meaning "someone typed pnpm tonight".',
    scope:
      'a CI workflow (test tiers only — the release build stays local), a pre-push hook, ' +
      'workflow_app/tests/ci-contract.test.ts (new).',
    acceptance:
      'A push runs the engine, agent-runtime and harness tiers and reports pass or fail without a human. ' +
      'The release build is NOT rebuilt in CI: this repo builds locally and deploys prebuilt, and a ' +
      'second build path is a second truth. A test asserts the workflow exists and names those tiers.',
    notes:
      'FOUND 2026-09-16: no .github, and every gate that ran all night ran because I typed it. The ' +
      'captain’s own MEMORY entry is the evidence — three stories skipped `next build` with an honest ' +
      'reason that later stopped being true, and a release blocker sat behind green tests, green tsc and ' +
      'a green harness. His release path already verifies the live sha and runs a smoke; the gap is ' +
      'before the release, not during it.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/ci-contract.test.ts`',
  },
  {
    id: 'APP-OBJECT-STORAGE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'NEXUS',
    priority: 'Medium',
    batch: 97,
    title: 'Documents leave the database for object storage',
    goal:
      'Document bytes stop living in Postgres and stop streaming through a serverless function: storage ' +
      'plus short-lived signed links, with an access record per read.',
    scope:
      'the media read/write path (app/api/media/**, db/media), a storage adapter, ' +
      'workflow_app/tests/media-storage.test.ts (new).',
    acceptance:
      'Uploads and downloads go through storage; the media row keeps metadata and a storage key. Reads ' +
      'are short-lived and recorded. A migration path exists for existing rows, or the story records why ' +
      'some are left behind. Downloads no longer consume Neon egress.',
    notes:
      'READ 2026-09-16: media is bytea in Postgres, downloaded through a serverless function, with ' +
      'Content-Length taken from a column (see APP-MEDIA-LENGTH-01). For executed contracts this is also ' +
      'where an access log belongs — a signed URL with an expiry gives you one for free, and gives the ' +
      'media routes a real authorization story instead of a publication flag. Biggest item in this wave; ' +
      'parked deliberately, not forgotten.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/media-storage.test.ts`',
  },
  {
    id: 'WORKSHOP-TOOLS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 97,
    title: 'The workshop tools the lanes are told about actually work',
    goal:
      'Every tool named in the workshop docs is wired and reachable from a lane — or it is removed from ' +
      'the docs, so no lane is told to use a backhoe that is not in the corner.',
    scope:
      'docs/agent/FORGE-WORKSHOP.md, the MCP/serena configuration, a check that each named tool resolves, ' +
      'workflow_app/tests/workshop-tools.test.ts (new).',
    acceptance:
      'The check reads the SAME source `pnpm forge:tools` reads. A tool whose wired flag is false and ' +
      'which is still listed for positions must either become wired, or be removed from those positions ' +
      '— the doc follows the table, never the other way round. `serena` is false today while five ' +
      'positions are told they have it; that is the defect this story closes. A tool named as available ' +
      'but unwired FAILS the check, and the fence fails if the doc and the table drift apart again.',
    notes:
      'The captain, 2026-09-16: "i brought in those tools and they keep dogging my score… they were ' +
      'supposed to be in Workshop MD — use them if you want. Digging a hole with a garden spade when ' +
      'there is a backhoe in the corner is your prerogative." Correct. A lane that cannot see the tool ' +
      'list will re-derive it badly, and the doc claiming tools that are not wired is the same defect ' +
      'class as the middleware claiming a capability nobody stamps.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/workshop-tools.test.ts`',
  },
  {
    id: 'ENG-FORGE-ACCEPTANCE-PROOF-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 95,
    title: 'The proof asserts the acceptance, not the cheapest reading of it',
    goal:
      'A story is judged against its OWN acceptance: a clause with no assertion behind it is reported ' +
      'UNPROVEN rather than passed, and the lane that did the work does not get to choose what is tested.',
    scope:
      'the acceptance→assertion mapping (written before the work, by the Architect or the Lead), the QA ' +
      'collector and adjudicator, workflow_app/tests/acceptance-proof.test.ts (new).',
    acceptance:
      'For a story, each acceptance condition maps to at least one assertion in the frozen proof. A ' +
      'condition with no assertion makes the verdict UNPROVEN, and the report names the condition. The ' +
      'mapping is written BEFORE the work and a lane that changes it is recorded as having done so. A test ' +
      'proves the case that bit: a proof that passes while an acceptance condition is unmet comes back ' +
      'UNPROVEN, not PASS.',
    notes:
      'MEASURED 2026-09-16 on WORKSHOP-TOOLS-01: qaPassed=true, story Complete 100, and the acceptance — ' +
      '"a tool whose wired flag is false and which is still listed for positions must become wired, or be ' +
      'removed from those positions" — was NOT met. `serena` still reads wired=false with five positions. ' +
      'The lane authored the proof, so the proof tested the cheaper reading (the docs), and QA honestly ' +
      'ruled that the tests passed. QA measures what it is given; nothing checked that the given thing was ' +
      'the acceptance. This is the sharpest form of "verdicts are self-reported", and it is a hole in the ' +
      'way every story in sprints 91-97 will be judged, including this one.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/acceptance-proof.test.ts`',
  },
  {
    id: 'ENG-FORGE-PARALLEL-WAVE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'Every ready node may run, and two lanes cannot collide',
    goal:
      'The wave loop runs EVERY ready task up to one declared cap instead of only smith_split_work siblings, ' +
      'and a lane commits only the surfaces it declared — so concurrency cannot sweep another writer\'s work.',
    scope:
      'workflow_app/forge/forge-executor.ts (the wave loop and its cap), the lane commit boundary (staging and ' +
      'the post-commit surface check), workflow_app/tests/forge-executor-contract.test.ts (a fence for a ' +
      'non-split fan-out AND for the surface refusal), docs/agent/WORKFLOW-ARCHITECTURE.md if it states the ' +
      'concurrency rule.',
    acceptance:
      'Two ready nodes that are not smith_split_work run in the same wave, bounded by one declared cap that is ' +
      'stated in the log by lane label, and a wave with one ready task still runs it exactly once (no ' +
      'behaviour change for a chain). A commit made while another writer has dirty files in the same checkout ' +
      'contains ONLY the paths in the committing lane\'s declared surface: given a declared set and a commit ' +
      'that also carries an undeclared path, the wave is refused by name and the extra paths are listed ' +
      'rather than the commit being accepted. The disjoint-surface rule that SPLIT already relies on is ' +
      'applied to any tasks that run concurrently, and a violation names the two lanes and the shared path.',
    notes:
      'FOUND 2026-09-17 while reading the wave loop: forge-executor.ts:286 filters ready tasks into ' +
      'smith_split_work siblings and `others`, then runs others in a SERIAL for-loop — so no other node type ' +
      'can overlap no matter what is ready. It has cost little so far because tonight\'s graphs were chains ' +
      '(one ready task per wave: architect, lead_pre, lead_solo_implement, lead_post, qa_verify), which is ' +
      'why removing the concurrency governor did not raise throughput. The real lever is running STORIES ' +
      'concurrently, and that is what the shared checkout breaks. HONEST BOUNDARY: this story does NOT ' +
      'restore the per-execution worktree — agent-runtime-role-runner.ts:929 records that ' +
      'buildAgentInvokerWorkspaces no longer produces one, so lanes write in the PRIMARY checkout, and ' +
      'lib/worker-workspace/provisioner.ts (default ../Culebraluxe-worktrees, with a guard that the root ' +
      'must sit outside the checkout) is unused. Restoring that isolation is a bigger change and belongs to ' +
      'ENG-FORGE-ARTIFACT-RESIDUE-01\'s family, not here. Until it lands, concurrency safety rests on ' +
      'declared surfaces plus path-scoped staging, which is why this story refuses an over-wide commit ' +
      'instead of trusting the lane not to stage the tree.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-executor-contract.test.ts`',
  },
  {
    id: 'ENG-FORGE-FINDING-DROP-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'A chunk write cannot silently drop a finding from its assignment',
    goal:
      'One pure decision owns what a plan write does to an assignment: additive writes union, a replay is ' +
      'idempotent, and a write that would DROP a finding is refused by name — so a multi-chunk plan can bind ' +
      'every finding it declares.',
    scope:
      'a new pure decision module next to the plan reader (decideAssignmentWrite: existing row + incoming ' +
      'write in, allow|refuse with the dropped ids out), scripts/forge-handoff.mjs (call it on the ' +
      'forge_role_assignment upsert instead of the `cardinality > 0` replace), workflow_app/tests/ ' +
      'handoff-assignment-write.test.ts (new).',
    acceptance:
      'A write that ADDS findings is allowed and the stored set is the UNION of what the assignment and the ' +
      'write declare. Replaying the identical write changes nothing and is allowed. A write that DROPS a ' +
      'finding the assignment already holds is REFUSED, and the refusal names the assignment id, the attempt ' +
      'and every dropped finding id; the row is left untouched. The exact failure of 2026-09-17 is a test: ' +
      'three chunk writes with disjoint finding sets against ONE assignment leave all of them assigned, and ' +
      'no finding reads as unassigned. The refusal is a decider, not an advisory: the write path calls it ' +
      'and a write it refuses cannot reach the database.',
    notes:
      'FOUND 2026-09-17 on SEC-SILENT-CATCH-01, first hand-off the Lead ever won. Attempt 1 wrote the plan ' +
      'the doctrine wants — 3 bounded chunks (detector+test+learn-loop | scripts/package.json/tests | the six ' +
      'portal routes) — and it was REFUSED with "each chunk write overwrote the assignment row, leaving ' +
      'F1/F2/F5/F6 unassigned and the gate-unit surfaces outside the route-only scope". The cause is ' +
      'mechanical: scripts/forge-handoff.mjs:217-221 upserts forge_role_assignment on every --chunk call ' +
      'with `finding_ids = case when cardinality(excluded.finding_ids) > 0 then excluded.finding_ids else ' +
      '<existing> end` — last non-empty write wins, and nothing refuses a write that shrinks a fact set. ' +
      'Attempt 2 then collapsed everything into ONE chunk carrying all twelve surfaces, which is the only ' +
      'shape that survives: so the surviving plan shapes are a single chunk on a single assignment, which ' +
      'is why a Lead that reasons correctly still ends up doing the work itself, and why SPLIT (two or more ' +
      'assignments, each with its own findings and chunks) has never been reachable. HONEST BOUNDARY: this ' +
      'story does not make the Lead choose differently and does not add SPLIT; it removes one structural ' +
      'reason a correct multi-chunk plan fails, and it does not retrofit rows already written. It is also ' +
      'not the same defect as ENG-FORGE-PARALLEL-WAVE-01: that one is about running nodes concurrently, ' +
      'this one is about a plan surviving its own writing.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/handoff-assignment-write.test.ts`',
  },
  {
    id: 'SEC-ROUTE-MANIFEST-02',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 92,
    title: 'A route that requires a session is not public',
    goal:
      'The route-authority manifest gets a word for "requires an authenticated portal session, no portal ' +
      'authority decision yet", and a `public` declaration whose handler checks a session becomes a ' +
      'violation instead of a loophole.',
    scope:
      'scripts/route-authority-manifest.ts (the decision vocabulary and the guard detection), the ' +
      'regenerated docs/agent/route-authority-manifest.md, workflow_app/tests/route-authority.test.ts.',
    acceptance:
      'A handler that admits only an authenticated session but makes no portal authority decision is ' +
      'declared `session`, not `public`, and the two media routes read `session` in the generated manifest. ' +
      'A `public` declaration whose handler contains a session check is a NAMED violation — a handler that ' +
      'turns a caller away for lacking a session is not deliberately reachable without one. The generated ' +
      'table still covers every handler under app/api/**/route.ts exactly once, and the fence asserts both ' +
      'media rows plus the public-with-session refusal.',
    notes:
      'FOUND 2026-09-17 while verifying SEC-ROUTE-MANIFEST-01 the hour it landed. The generated table ' +
      'declares app/api/media/[id]/route.ts#GET and app/api/media/documents/[id]/route.ts#GET as `public`, ' +
      'which the generator defines as "deliberately reachable without a Portal authority" — while BOTH ' +
      'reasons say otherwise: "authenticated portal escape hatch is JWT-only" and "Document access decided ' +
      'by decideDocumentAccess against publication state and portal session". The documents route began ' +
      'requiring a session earlier the same night (SEC-MEDIA-DOC-01: no session is a 401), so the record ' +
      'now claims a control is absent where it exists — the exact reader-misleading failure ' +
      'AUTH-CAPABILITIES-01 was written to remove, in the opposite direction. Cause: the guard detector ' +
      'knows resolvePortalAccess | runAuthorized | guardPortalUpload | guardPortalRoute | requireAuthority, ' +
      'and the session gate is none of those, so no word fit and `public` was the least-wrong choice. ' +
      'HONEST BOUNDARY: this is the RECORD, not the control. It does not add a portal authority decision to ' +
      'the sibling media route, which still checks only token.sub (the opening AUTH-CAPABILITIES-01 names), ' +
      'and it does not bind a document to its owning record.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/route-authority.test.ts`',
  },
  {
    id: 'ENG-FORGE-CONTRACT-ONE-WRITER-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'The contract row has one writer, and a pair conflict does not stall the wave',
    goal:
      'No column of forge_role_contract is written by two rules, and planWave refuses to CO-SCHEDULE two ' +
      'overlapping lanes instead of refusing the whole wave — so a disjoint third lane still runs.',
    scope:
      'scripts/forge-handoff.mjs (the forge_role_contract upsert: finding_ids, merge_checks and ' +
      'surface_scope), workflow_app/forge/forge-executor.ts (planWave overlap handling and its log line), ' +
      'workflow_app/tests/forge-executor-contract.test.ts, and the removal of the source-grep assertion in ' +
      'workflow_app/tests/handoff-assignment-write.test.ts.',
    acceptance:
      'A write that would SHRINK finding_ids, merge_checks or surface_scope on forge_role_contract is ' +
      'refused by name with the dropped entries, exactly like the assignment row — one decision, reused, not ' +
      'a second SQL case. planWave with ready {A, B sharing a path} plus C disjoint schedules C and defers ' +
      'the pair, and the plan states the refusal (both lanes and the shared path) rather than silently ' +
      'reordering. A wave with one ready lane behaves exactly as it does today. The source-grep assertion is ' +
      'gone and the folded three-write test is the only verdict on the writer.',
    notes:
      'FROM GROK 2026-09-17 review (B3, I3, I6, I8), verified here before filing. B3: ' +
      'scripts/forge-handoff.mjs:467-472 still carries `case when cardinality(excluded.x) > 0 then ' +
      'excluded.x else forge_role_contract.x end` for finding_ids, merge_checks AND surface_scope — the ' +
      'assignment row was fixed on 2026-09-17 (88761a95) and the contract row kept the old writer, so one ' +
      'fact has two writers. surface_scope is the load-bearing one: it is what ENG-FORGE-SURFACE-SUPPLIER-01 ' +
      'would feed planWave from, and a silently shrunken surface can make two genuinely overlapping lanes ' +
      'read as disjoint — the same batch, the same path. B4/I3: forge-executor.ts:206-217 returns ' +
      '`{ ok: false }` for the ENTIRE wave on any pairwise overlap among known surfaces. It cannot fire ' +
      'today because no lane declares a surface, which is exactly why it must be fixed before the supplier ' +
      'is wired, not after. I8: a fold over three writes is the test; indexOf on the script is a second ' +
      'verdict, and the handbook forbids two. HONEST BOUNDARY: this story does not wire a supplier (that is ' +
      'ENG-FORGE-SURFACE-SUPPLIER-01) and it does not raise the concurrency cap.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-executor-contract.test.ts workflow_app/tests/handoff-assignment-write.test.ts`',
  },
  {
    id: 'ENG-FORGE-SURFACE-SUPPLIER-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'The lane\'s declared surface reaches the wave and the commit, and the commit never lies',
    goal:
      'The surface a lane declared is actually SUPPLIED to planWave and to the commit helper — one list, two ' +
      'call sites — so fan-out becomes a factory behaviour instead of a tested function, and a commit the ' +
      'helper made is never reported as nothing.',
    scope:
      'scripts/forge-engine-worker.ts and workflow_app/forge/agent-runtime-role-runner.ts (supply surfaceOf ' +
      'from the lane assignment/chunk surface), lib/worker-workspace/commit.ts (pass it as allowedScope and ' +
      'stop returning commitHash null for a commit that exists), workflow_app/forge/forge-shaping.ts (the ' +
      'MEDIUM floor from the shaper), workflow_app/tests/forge-executor-contract.test.ts.',
    acceptance:
      'surfaceOf has a production supplier: two non-fanout ready lanes whose declared surfaces are disjoint ' +
      'are co-scheduled in one wave, witnessed by the wave log naming both lanes, and the same list reaches ' +
      'the commit helper as allowedScope so `git add -A` is no longer the shared-checkout default. When the ' +
      'post-commit backstop finds paths outside the surface it does NOT return commitHash null — it returns ' +
      'the sha WITH the refusal, or undoes the commit it just made — and a test proves no path returns null ' +
      'while a commit exists on the branch. MEDIUM comes from the shaper\'s seam groups (required findings ' +
      'spanning two or more groups) rather than the model\'s all-1 feature self-rating.',
    notes:
      'FROM GROK 2026-09-17 review (B1, B2, B5, I1, I2, I4), each claim verified here first. B1: `surfaceOf` ' +
      'occurs exactly twice in the tree — the option at forge-executor.ts:105 and the call at :383 ' +
      '(`opts.surfaceOf?.(task) ?? null`) — with NO supplier in app, workflow_app, scripts, db, lib or ' +
      'agent-runtime, so every non-fanout lane is surface:null, runs alone, and the wave is live only for ' +
      'smith_split_work, which the old code already batched. PARALLEL-WAVE-01 (a7560803) is therefore ' +
      'mechanism whose runtime effect is currently zero: the honest deduction on that story, recorded here ' +
      'rather than quietly. B2: commitWorkerWorkspaceChanges IS called in production (agent-runtime/' +
      'factory.ts:155, opencode-harness-adapter.ts, gateway/cli-agent-adapter.ts) but never with a scope, so ' +
      'allowedScope is undefined and the helper takes its legacy branch — `git add -A` at commit.ts:102. ' +
      'B5: commit.ts:88-99 commits at :89, then checks `git show --name-only HEAD` at :91 and returns ' +
      '`{ commitHash: null, changed: false, refused: extras }` at :97 — a commit that exists on the branch ' +
      'reported as nothing. DEPENDS ON ENG-FORGE-CONTRACT-ONE-WRITER-01: that story makes surface_scope ' +
      'honest, and wiring a supplier onto a last-write-wins surface would let two overlapping lanes read as ' +
      'disjoint. HONEST BOUNDARY: this story does not restore per-execution worktrees, does not raise ' +
      'FORGE_SPLIT_CONCURRENCY above 2, and does not claim fan-out is observed — the observation is the ' +
      'two-unit postcard, not this change.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/forge-executor-contract.test.ts`',
  },
  {
    id: 'ENG-FORGE-RECEIPT-COLUMNS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 92,
    title: 'A run receipt carries its own facts: a named cost source and its sha',
    goal:
      'A run receipt can be joined to its artifact and its spend without reading prose: the sha is in the ' +
      'column, the cost source is a closed set, and a total spend states the coverage it was computed from.',
    scope:
      'db/forge-run.ts and the writer that fills cost_source / commit_hash / candidate sha for a run, the ' +
      'QA-and-Assay receipt writer, workflow_app/tests/run-receipt-facts.test.ts (new).',
    acceptance:
      'Every completed run row carries the candidate sha it produced in commit_hash, so the release receipt ' +
      'can be resolved to a commit without parsing notes. cost_source holds a value from a closed set ' +
      '(vendor:<id> | tokens*weight | unrecorded) and unrecorded is WRITTEN rather than NULL, so a spend ' +
      'total can state how many runs it covers. A receipt whose sha exists only inside its notes text is a ' +
      'named defect in the test, and a run that produced no commit says so explicitly rather than leaving ' +
      'the column empty.',
    notes:
      'FROM GROK 2026-09-17 review (B6/I7) — WITH A CORRECTION TO HIS EVIDENCE AND TO MINE. His B6 read ' +
      '`cost_source = ?,vendor`. There is no stored `?`: my spend query was ' +
      '`string_agg(distinct coalesce(cost_source,\'?\'), \',\')`, the `?` was MY placeholder for NULL, and he ' +
      'read it as a value. What the column actually holds across storyboard_story_run: vendor 340 runs / ' +
      '$8.3132 (all carry a cost), widgets 278 runs (no cost), NULL 317 runs (no cost) — so 595 of 935 runs ' +
      'record no cost at all and the $2.26 quoted for the 2026-09-16 night is a FLOOR, not the spend. That is ' +
      'a bigger defect than the bad word: a total that cannot state its coverage. The sha half is the same ' +
      'family, measured live: tonight\'s runs report `commit (none recorded)` and ' +
      '`candidate=(none) | verified=(none)`, the sha exists only in prose, and a rebase orphaned it — the QA ' +
      'receipt names cbd05839 while the shipped commit is f8535364 with a byte-identical tree. HONEST ' +
      'BOUNDARY: this makes receipts joinable and honest; it does not re-price historical runs and it does ' +
      'not backfill rows already written.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/run-receipt-facts.test.ts`',
  },
  {
    id: 'ENG-FORGE-RELEASE-RECEIPT-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'The release gate can pass on a receipt the repo actually produces',
    goal:
      'releaseEvidence has a PRODUCER: a story that requires no deployment finishes on an explicit ' +
      'no-deployment attestation, and a story that requires one still needs a real deployment receipt — so ' +
      'no story wedges on "role did not deliver devops-receipt".',
    scope:
      'workflow_app/forge/forge-release-receipt.ts (the producer), the dev_ops evidence wiring in ' +
      'workflow_app/forge/agent-runtime-role-runner.ts and forge-role-mapping.ts, docs/agent/MEMORY.md:202 ' +
      '(the entry this closes), workflow_app/tests/forge-release-receipt.test.ts.',
    acceptance:
      'A story with no deployment requirement completes its release stage on an attestation built from ' +
      'machine evidence (the published sha comparable to origin, the build that actually ran, the frozen ' +
      'proofs) with the receipt kind recorded as such — never a fabricated deployment id, never a ' +
      'placeholder. A story that DOES require a deployment still fails closed without a real deployment ' +
      'receipt. The deploy gate no longer depends on a receipt that only a prior writer could have set: a ' +
      'test drives the gate with a publish-only evidence set and asserts it passes for a no-deployment ' +
      'story and fails for a deployment story. MEMORY.md:202 is updated because its claim is no longer ' +
      'true.',
    notes:
      'FOUND 2026-09-17 by the dev_ops lane on ENG-FORGE-CONTRACT-ONE-WRITER-01, which had already shipped ' +
      '519eb2b3 and passed QA, then HOLDed twice on the release stage with "role did not deliver ' +
      'devops-receipt". The lane refused to fabricate and reported machine evidence instead: ' +
      '`git ls-remote origin refs/heads/main` = 519eb2b3 = published_sha; no migration and no derived ' +
      'refresh were needed; nothing imports `releaseReceiptFromDeploymentSignal`; the runner builds ' +
      '`rolePorts.releaseEvidence` from the durable `current.deploymentReceipt`, which no writer sets, so ' +
      'the gate can only pass on a receipt that already exists; `deployed_sha`, `deployment_receipt` and ' +
      '`production_verified` are all null. It tried `vercel --prod --yes` and the upload aborted (no ' +
      'deployment created), and `vercel.json` sets `git.deploymentEnabled=false`, so the publish push does ' +
      'not deploy either. It then cited docs/agent/MEMORY.md:202 as the documented blocker and named ' +
      'forge-release-receipt.ts + the dev_ops evidence wiring as the fix, which is out of its declared ' +
      'surfaces. This is the highest-leverage open defect in the factory: EVERY release-bearing story ' +
      'wedges here, which is why stories finish their work, ship it, and still sit on the board as HOLD. ' +
      'HONEST BOUNDARY: this story does not deploy anything and does not make Vercel observable; it makes ' +
      'the gate honest about what was and was not performed, and it does not retro-fix stories already ' +
      'held at the release stage.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-release-receipt.test.ts`',
  },
  {
    id: 'APP-WHATSAPP-ATTRIBUTION-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'NEXUS',
    priority: 'High',
    batch: 97,
    title: 'A WhatsApp message says which line received it and what thread it belongs to',
    goal:
      'An inbound WhatsApp message lands with the RECEIVING line, a stable thread key and the envelope facts ' +
      'it was read from — so the CRM can answer "who last spoke, on which line, in which thread" from the ' +
      'record instead of from a phone.',
    scope:
      'app/api/integrations/whatsapp/webhook/route.ts (the landing mapping), db/landing.ts (the writer and its ' +
      'input), lib/whatsapp-cloud/ (a NEW pure mapping module — attribution.ts — is its home: a route.ts may ' +
      'NOT export a non-HTTP helper, because Next builds checkFields<Diff<{GET?: Function, ...}, TEntry>> and ' +
      'any extra export fails next build/next typegen), a migration only if a column is required (184 is ' +
      'free), workflow_app/tests/whatsapp-attribution.test.ts (new).',
    acceptance:
      'For an inbound message, to_address holds the RECEIVING business line read from the envelope ' +
      '(metadata.display_phone_number, with phone_number_id still on source_account) — never inferred from a ' +
      'field Meta does not send on inbound. conversation_id is a STABLE thread key derived from the receiving ' +
      'line and the contact, so two messages between the same pair share one thread, while a quoted reply ' +
      'context id is still recorded where present. The contact name from contacts[].profile.name is captured. ' +
      'The landed raw carries the envelope facts actually read (metadata and the matching contact), so the ' +
      'row stops claiming the raw Meta payload while holding one message object. A test drives the mapping ' +
      'from a realistic payload — envelope metadata, a contacts entry and an inbound text — and asserts every ' +
      'clause above, plus that two messages with the same pair share a conversation_id and that the newest ' +
      'sent_at per conversation_id is the answer to "when did we last hear from this thread".',
    notes:
      'FOUND 2026-09-17 by verifying the captain\'s first real inbound message, the night the WABA ' +
      'subscription fix landed (a genuine Meta wamid, "Handshake test 11:14 pm", on Coexistence WABA ' +
      '1330394873483351). The row was REAL and the attribution was not: source_account held the ' +
      'phone_number_id (so the line was half-answered), but to_address was null, conversation_id was null ' +
      'and media_id was null, and raw held only the normalized message object ' +
      '(id/from/text/type/timestamp/from_user_id) — no metadata, no contact, no profile name. Cause, read ' +
      'in the route: conversationId comes from message.context?.id, which only a QUOTED REPLY carries; ' +
      'toAddress comes from message.to, which Meta does not send on inbound (it sends from); and raw is the ' +
      'message, not the envelope, while the comment above the write claims the raw payload lands before any ' +
      'normalization. The captain\'s need is small and specific: the METADATA is the point ("just to know ' +
      'what is last time a message happened"), which the thread key plus max(sent_at) answers. HONEST ' +
      'BOUNDARY: this does not add delivery/status receipts, does not download media, does not touch the ' +
      'Meta-side subscription, and does not backfill the one historical real row (it is a record of what ' +
      'arrived, not a reconstruction).',
    assayCommands: '- `node --import tsx --test workflow_app/tests/whatsapp-attribution.test.ts`',
  },
  {
    id: 'ENG-FORGE-FINDING-SUPERSEDE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'A later architect attempt cannot silently drop a seam the earlier one declared',
    goal:
      'A re-issued findings set that loses a seam the previous attempt declared is refused by name or ' +
      'recorded as an explicit supersede — never a silent drop — and the Lead can see which attempt is in ' +
      'force and what it lost.',
    scope:
      'scripts/forge-handoff.mjs (the --findings writer), db/forge-role-finding.ts (listStoryForgeFindings: ' +
      'what "the newest attempt" means and how a supersede is recorded), the routing context built in ' +
      'workflow_app/forge/agent-runtime-role-runner.ts:519-533, workflow_app/tests/' +
      'forge-role-finding-supersede.test.ts (new).',
    acceptance:
      'Writing findings for attempt N that omits a seam declared by attempt N-1 for the same node and ' +
      'process instance either REFUSES the write naming the dropped seam, or records a supersede that names ' +
      'it — the two are distinguishable in the row and in the routing context, and a silent drop is not a ' +
      'reachable outcome. The Lead routing context states which attempt is in force and, where a seam was ' +
      'dropped, names the dropped seam and the attempt that declared it, so a HOLD blames the right thing. ' +
      'A test drives two attempts where the second drops a seam and asserts the refusal/supersede and the ' +
      'visible seam loss; a second test keeps the behaviour the newest-attempt rule exists for (a story-wide ' +
      'read resurrecting earlier attempts must still not produce duplicate finding ids).',
    notes:
      'FOUND 2026-09-17 on APP-WHATSAPP-ATTRIBUTION-01, by the captain asking why an amended Architect ' +
      'artifact did not reach the Lead. Measured in forge_role_finding: attempt 1 wrote four findings, TWO ' +
      'of which carried the seam lib/whatsapp-cloud/ (c6206dcb, e867e9f8); attempt 2 wrote SIX findings and ' +
      'NONE carried it (they name app/api/integrations/whatsapp/webhook/route.ts, db/landing.ts, ' +
      'db/migrations/ and workflow_app/tests/). The reader takes the NEWEST attempt per node ' +
      '(agent-runtime-role-runner.ts:519-529, deliberately, so a story-wide read does not resurrect earlier ' +
      'attempts duplicates), so the Lead routed against the worse set and HOLDed twice: first because a pure ' +
      'mapper cannot be exported from route.ts under Next route-export typing, then because "the routing ' +
      'validator allowed seams are the pre-correction set and exclude lib/whatsapp-cloud/". A ' +
      'forge-story-reset produced a fresh instance whose architect DID declare the seam, and the story then ' +
      'routed SMITH and shipped. CONTRIBUTING CAUSE, not this story defect: a resumed instance replans ' +
      'against its frozen snapshots, so amending the story row mid-flight changes nothing until the instance ' +
      'is reset — that is now the documented operational path, and the same applies to any future edit. ' +
      'RELATION: this is the SAME law as decideAssignmentWrite and decideContractWrite (add unions, empty ' +
      'is a no-op, a shrink is refused by name), applied to the table routing actually reads. It is also why ' +
      'a correct Architect can look wrong from the Lead seat. HONEST BOUNDARY: this does not make an ' +
      'instance re-read an amended story, does not change how the Architect plans, and does not retrofit ' +
      'rows already written.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/forge-role-finding-supersede.test.ts`',
  },
  {
    id: 'ENG-FORGE-MIGRATION-APPLIED-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'A story that ships a migration is not complete while the ledger lacks it',
    goal:
      'A story whose change set adds a file under db/migrations/ cannot reach Complete — and cannot be ' +
      'reported shipped — while the control plane migration ledger lacks that file, so code never lands ' +
      'ahead of the column it writes.',
    scope:
      'the completion/release path that decides a story is done (agent-runtime-role-runner.ts deploy branch ' +
      'and the story completion seam), the migration ledger reader used by scripts/migration-status.mjs, ' +
      'workflow_app/tests/migration-applied-guard.test.ts (new).',
    acceptance:
      'When a story change set adds a file under db/migrations/, completion requires the LEDGER ' +
      '(schema_migration, PROD) to carry that filename; otherwise the story is held or refused with a ' +
      'reason that names every unapplied file. A story with no migration in its change set is unaffected, ' +
      'and a story whose migration is ledgered is unaffected — no false positives in either direction. The ' +
      'check reads the LEDGER TABLE, never the filesystem, and a test proves both directions: a fixture ' +
      'change set adding a migration with an empty ledger is refused BY NAME, and the same change set with ' +
      'the ledger row is allowed.',
    notes:
      'FOUND 2026-09-17, twice in one night, by the operator parking every story at QA with --until ' +
      'node:qa_verify to avoid production deploys — which SKIPS DEV_OPS, and DEV_OPS owns migrations. ' +
      'APP-WHATSAPP-ATTRIBUTION-01 shipped code writing l_whatsapp.context_id (migration 184) while the ' +
      'column did not exist; ENG-FORGE-FINDING-SUPERSEDE-01 shipped the supersede writer while its table ' +
      '(migration 185) did not exist. Both were applied by hand afterwards (ledger 07:16:52 and 07:49:10) ' +
      'and both would have failed at RUNTIME on the next deploy — the WhatsApp case by failing to land a ' +
      'message, which is the captain only working feature. Nothing in the engine noticed: the story read ' +
      'Complete, QA passed, and the migration file existed, so every receipt was green while the schema was ' +
      'absent. This is the same disease as the rest of the night (a fact that exists while the runtime does ' +
      'not have it), one layer lower, and it is the cost of the QA-park shortcut rather than a defect in ' +
      'DEV_OPS. HONEST BOUNDARY: this does not apply migrations automatically, does not change DEV_OPS ' +
      'ownership of them, and does not retro-check stories already completed.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/migration-applied-guard.test.ts`',
  },
  {
    id: 'ENG-FORGE-RELEASE-ORDER-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 98,
    title: 'A schema release cannot block itself on the difference it creates',
    goal:
      'Cross-environment schema parity is verified AFTER the PROD migration runs, so a new table or column ' +
      'does not create the very drift that prevents its own promotion.',
    scope:
      'workflow_app/forge/release-operations.ts (the DEV_OPS verification gate and checkSchemaParity call ' +
      'site), workflow_app/tests/release-order.test.ts (new).',
    acceptance:
      'A story adding a table or column migrates PROD and THEN verifies parity: the parity check runs after ' +
      'the PROD migration and reports clean for the change it just applied. Parity drift NOT explained by ' +
      'the migration still fails the gate, naming the drift. A failed PROD migration stops before parity, ' +
      'and no path exists where a successfully applied migration is reported as a parity failure. A test ' +
      'drives the ordering for both a clean apply and a failing apply.',
    notes:
      'FROM ASTRA (ChatGPT) CODE REVIEW 2026-09-17 at main 2920742a, bug 1 of 4 CONFIRMED IN SOURCE, and the ' +
      'captain confirms it has bitten him before: "he is right about dev burning a prod release if the ' +
      'database does not go too". Verified here: release-operations.ts:198 calls ' +
      'checkSchemaParity(devUrl, prodUrl) inside DEV verification, and the workflow only migrates PROD ' +
      'after DEV verification passes — so the new table IS the reported drift (lines 201-208 build the list ' +
      'from tablesOnlyDev/tablesOnlyProd/columnDrift/indexDrift/fkDrift) and the release blocks on its own ' +
      'change. Astra order: fix this before anything else in the review. HONEST BOUNDARY: this reorders a ' +
      'release gate; it does not make DEV and PROD converge by itself and it does not touch the two-ledger ' +
      'split that ENG-FORGE-MIGRATION-REPLAY-01 records.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/release-order.test.ts`',
  },
  {
    id: 'ENG-FORGE-MIGRATION-REPLAY-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 98,
    title: 'A migration that already ran is not executed again',
    goal:
      'Migration execution is checksum-aware and replay-safe: an already-applied migration is SKIPPED by ' +
      'name, and a recording failure cannot cause the SQL to run twice.',
    scope:
      'workflow_app/forge/release-operations.ts (applyMigrations), the forge_migration_execution ledger ' +
      'reader, workflow_app/tests/migration-replay.test.ts (new).',
    acceptance:
      'Before executing a migration file its content checksum is compared with the ledger: a match SKIPS ' +
      'execution and reports already-applied (idempotent replay), while a checksum MISMATCH for the same ' +
      'file refuses by name — a changed migration is never re-run over an applied one. A recording failure ' +
      'after execution leaves a state a retry can resolve without repeating data changes, and a test fails ' +
      'the record write and then retries to prove it.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, bug 2 of 4 CONFIRMED IN SOURCE. Verified here: ' +
      'release-operations.ts:92 runs `await pool.query(migration.sql)` FIRST and records into ' +
      'forge_migration_execution afterwards as a separate statement, with no already-applied check before ' +
      'execution — so a failure between the two repeats the SQL on retry (data changed twice, or an ' +
      'existing-object error). Related, found while verifying: the repo holds TWO migration ledgers — ' +
      'schema_migration (written by scripts/apply-migration.mjs, read by ENG-FORGE-MIGRATION-APPLIED-01) and ' +
      'forge_migration_execution (written by this lane) — so one fact has two homes and the guards read ' +
      'different ones. HONEST BOUNDARY: this makes execution replay-safe; it does not merge the ledgers in ' +
      'this change and does not retro-repair a migration already applied twice.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/migration-replay.test.ts`',
  },
  {
    id: 'ENG-FORGE-CRASH-WINDOW-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 98,
    title: 'Workflow completion, evidence and repair counts survive a crash between them',
    goal:
      'Advancing the engine, persisting evidence and counting a repair attempt are recoverable or ' +
      'idempotent, so a crash between them leaves a state a later run can FINISH rather than an advanced ' +
      'workflow with no result and an undercounted repair.',
    scope:
      'workflow_app/forge/forge-engine-runtime.ts (completeForgeRoleTask and the evidence write after it), ' +
      'workflow_app/forge/forge-executor.ts (the repair counter near :356), workflow_app/tests/' +
      'completion-crash-window.test.ts (new).',
    acceptance:
      'Given a crash injected after the transition and before the evidence write, a later run detects the ' +
      'advanced-without-evidence state from durable records and COMPLETES it instead of re-running the ' +
      'role, with the evidence written exactly once. A crash between the evidence and the repair counter ' +
      'leaves the counter recoverable, so attempts are never undercounted. Both sequences are simulated and ' +
      'then resumed by a test.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, bug 3 of 4 CONFIRMED IN SOURCE, and the engine already states the ' +
      'residual itself: forge-engine-runtime.ts:221 calls engine.completeTask BEFORE the evidence write, and ' +
      'the comment there says "If this write fails, the run advances with evidence missing — fail-closed, ' +
      'because the next role gate reports the absent deliverable and HOLDs — which is strictly better than ' +
      'the previous" behaviour. Repair counters are incremented afterwards in forge-executor.ts:356, which a ' +
      'crash can undercount. So this story closes a STATED residual rather than discovering one. HONEST ' +
      'BOUNDARY: it does not merge the two ledger writers into one transaction where the engine cannot (the ' +
      'same comment explains why), and it does not change the QA disposition ordering owned by ' +
      'ENG-FORGE-QA-RACE-01.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/completion-crash-window.test.ts`',
  },
  {
    id: 'ENG-FORGE-CLAIM-CLOCK-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 98,
    title: 'A live claim is not read as abandoned because of a timestamp suffix',
    goal:
      'A Postgres timestamp is parsed to the instant it names, in every format the driver emits, so a claim ' +
      'touched seconds ago can never be treated as stale.',
    scope:
      'db/forge-engine-task-execution.ts (the stale-claim read around :110), workflow_app/tests/' +
      'claim-clock.test.ts (new).',
    acceptance:
      'The same instant is parsed correctly from all the forms the driver emits — with a bare offset ' +
      '(`2026-09-17 07:16:52.653+00`), with a colon offset (`+00:00`), with `Z`, and with no offset at all — ' +
      'and an unparseable value is treated as NOT stale rather than as stale. A claim updated seconds ago ' +
      'is never reported stale at any of those formats, and the test asserts each form against a fixed ' +
      'instant.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, bug 4 of 4 CONFIRMED IN SOURCE. Verified here: ' +
      'db/forge-engine-task-execution.ts:110 does ' +
      '`Date.parse(updatedAt.replace(String " " -> "T") + "Z")`, and a timestamptz string arrives with its ' +
      'own offset (for example `2026-09-17 07:16:52.653+00`), so the built string is ' +
      '`2026-09-17T07:16:52.653+00Z` and Date.parse returns NaN — which is exactly how a fresh claim can ' +
      'read as abandoned. This is the same surface ENG-FORGE-REAP-GUARD-01 (sprint 94) protects from the ' +
      'other side: that story says the reaper cannot kill a live lane, and this is one way the reaper could ' +
      'believe a lane is dead. HONEST BOUNDARY: this fixes the parse and the unparseable case; it does not ' +
      'change the staleness threshold and does not add the heartbeat that REAP-GUARD-01 owns.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/claim-clock.test.ts`',
  },
  {
    id: 'ENG-FORGE-QA-RACE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium-High',
    batch: 98,
    title: 'A losing QA worker cannot contaminate the verdict, and a verdict names its commands',
    goal:
      'Verify and close two QA adjudication risks: a worker that loses the task-completion race must not ' +
      'write a failure disposition, and a verdict must compare command IDENTITIES, not merely their count.',
    scope:
      'workflow_app/forge/agents/qa/run.ts and the QA disposition writer in forge-executor.ts, ' +
      'workflow_app/tests/qa-race-and-identity.test.ts (new).',
    acceptance:
      'The competing-worker case is reproduced first and the finding recorded as confirmed or refuted: if a ' +
      'loser can write its disposition before losing the race, the write is made conditional on winning (or ' +
      'the disposition is scoped so a loser cannot overwrite the winner), and a test drives two competing ' +
      'completions and asserts only the winner result survives. The adjudicator compares the SET of ' +
      'commands behind a verdict, so two runs with the same COUNT but different commands cannot be accepted ' +
      'as the same evidence, and a test proves the substituted case is refused by name.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, bug candidates 5 and 10 (UNCONFIRMED — this story verifies before ' +
      'it fixes, and its record states which way the verification went). Candidate 5 is the residual the ' +
      'engine already admits in forge-engine-runtime.ts:214-221: "a losing QA worker can still record a ' +
      'disposition. Closing that needs one shared transaction across the engine and the ledger writers" — ' +
      'the QA-failure ledger write deliberately stays before the transition because qa_failure_route reads ' +
      'the durable disposition while the transition runs. Candidate 10: the adjudicator compares command ' +
      'counts without identities, so same-length substitution would be invisible. HONEST BOUNDARY: this ' +
      'does not force the shared transaction the engine comment calls a larger change; it makes the loser ' +
      'write conditional and the identity comparison explicit, or records why each is not reachable.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/qa-race-and-identity.test.ts`',
  },
  {
    id: 'ENG-FORGE-LANE-FAILURE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 98,
    title: 'A failing sibling lane is awaited, and a shared pool outlives one operation',
    goal:
      'Verify and close two runner risks: one lane rejecting must not report driver failure while sibling ' +
      'lanes keep writing, and no individual operation may close a database pool another caller is using.',
    scope:
      'the concurrent lane pump in workflow_app/forge/forge-executor.ts, the release and parity helpers in ' +
      'workflow_app/forge/release-operations.ts, workflow_app/tests/lane-failure-and-pool.test.ts (new).',
    acceptance:
      'The two cases are reproduced first and each recorded as confirmed or refuted. When one lane in a ' +
      'concurrent batch rejects, the failure is reported only after every sibling has settled, and the ' +
      'records of the siblings that did run are preserved — never a driver failure returned while lanes are ' +
      'still writing. No single operation closes a shared pool: the pool lifecycle is owned by process ' +
      'shutdown, and a test proves a caller can still run a query after another caller finished.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, bug candidates 6 and 7 (UNCONFIRMED — verify, then fix or record ' +
      'why not reachable). Candidate 6: the wave pump uses Promise.all, so a first rejection can return ' +
      'before siblings finish while they keep writing. Candidate 7: release and parity helpers close a ' +
      'shared database pool, which can disrupt a concurrent caller. Related, measured tonight: two ' +
      'migrations were applied by hand and the repo holds two migration ledgers, so pool/ledger ownership is ' +
      'live territory rather than theoretical. HONEST BOUNDARY: this does not add lane cancellation or ' +
      'timeouts (that is ENG-FORGE-REAP-GUARD-01/HEARTBEAT-01 territory) and does not change when the ' +
      'concurrency cap applies.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/lane-failure-and-pool.test.ts`',
  },
  {
    id: 'ENG-FORGE-VERIFY-IDENTITY-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 98,
    title: 'A verification names the exact object and the exact attempt it verified',
    goal:
      'Verify and close two verification-identity risks: derived-model verification must not match a view ' +
      'by name in the wrong schema, and a successful receipt from an earlier attempt must not satisfy a ' +
      'later verification.',
    scope:
      'the derived-model verification path and the receipt lookup used by verification (forge/release and ' +
      'forge-evidence readers), workflow_app/tests/verify-identity.test.ts (new).',
    acceptance:
      'The two cases are reproduced first and each recorded as confirmed or refuted. A materialized view is ' +
      'matched by SCHEMA and name, so a same-named view in another schema cannot be accepted as the ' +
      'verified object, and a test drives the duplicate-name case and asserts the wrong one is refused. ' +
      'Verification is bound to the CURRENT release attempt (its receipt must be minted for that attempt or ' +
      'its sha), so an old successful receipt cannot satisfy a later verification, proven by a test with a ' +
      'stale receipt and a fresh expectation.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, bug candidates 8 and 9 (UNCONFIRMED — verify, then fix or record ' +
      'why not reachable). Candidate 8: derived-model verification matches materialized views by name ' +
      'without schema. Candidate 9: a successful old refresh receipt can satisfy a later verification ' +
      'because verification is story-scoped rather than tied to the current release attempt. This is the ' +
      'same family as ENG-FORGE-ARTIFACT-RULING-01 and RECEIPT-COLUMNS-01 (a receipt must name the exact ' +
      'artifact and attempt it belongs to) and the same family as tonight measured sha-in-prose loss. ' +
      'HONEST BOUNDARY: this does not re-verify past releases and does not change what a receipt contains, ' +
      'only which object and attempt it is allowed to certify.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/verify-identity.test.ts`',
  },
  {
    id: 'ENG-FORGE-ASSERTION-RAN-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 99,
    title: 'A referenced assertion is verified to have RUN, not merely listed',
    goal:
      'An acceptance clause is satisfied only by an assertion that actually executed and passed in the ' +
      'frozen proof: a named-but-unrun assertion is UNPROVEN, the same as a clause with no assertion.',
    scope:
      'the acceptance-to-assertion mapping reader and the QA adjudicator (workflow_app/forge/agents/qa/run.ts ' +
      'and the mapping reader added by ENG-FORGE-ACCEPTANCE-PROOF-01), workflow_app/tests/' +
      'assertion-executed.test.ts (new).',
    acceptance:
      'A clause whose mapped assertion name does not appear in the executed proof output is reported ' +
      'UNPROVEN, naming the clause and the missing assertion. A mapped assertion that ran and passed ' +
      'satisfies the clause. A clause mapped to an assertion that ran and FAILED is FAIL, not UNPROVEN. A ' +
      'test drives all three cases against a recorded proof output, including a mapping that references an ' +
      'assertion the proof never mentions.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, improvement 6 of 10 ("verify that referenced assertions actually ' +
      'ran, rather than merely being listed"). This is the next step past ENG-FORGE-ACCEPTANCE-PROOF-01, ' +
      'which binds each acceptance clause to at least one assertion and reports an unmapped clause as ' +
      'UNPROVEN: binding by NAME is still a claim, and a lane could satisfy the mapping while the proof never ' +
      'executes the named assertion. Astra improvement 3 (QA evidence bound to the exact code and criteria) ' +
      'is covered by ACCEPTANCE-PROOF-01 plus ENG-FORGE-RECEIPT-COLUMNS-01 and is NOT re-filed. HONEST ' +
      'BOUNDARY: this does not re-run proofs, does not change how the mapping is written, and depends on the ' +
      'proof output being captured verbatim.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/assertion-executed.test.ts`',
  },
  {
    id: 'ENG-FORGE-FAILURE-STAGE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 99,
    title: 'A durable failure record carries its stage, cause and recovery action',
    goal:
      'Every durable failure record names the STAGE that failed, the cause it recorded, and the smallest ' +
      'recovery action an operator can take — so a reader does not have to reconstruct any of the three.',
    scope:
      'the failure writers (forge_hold_record, the failure disposition, the run result) and the reader that ' +
      'renders them, workflow_app/tests/failure-stage-record.test.ts (new).',
    acceptance:
      'For a failure of each stage (gate refusal, lane failure, release failure, migration failure) the ' +
      'record carries a stage, a cause string, and a recovery action, and a test asserts one of each. A ' +
      'record missing any of the three is refused at the writer with a named reason rather than stored ' +
      'half-formed. The stage vocabulary is closed, so an unknown stage is refused by name.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, improvement 7 of 10 ("preserve stage, cause, and recovery action ' +
      'in durable records"). Measured tonight, three times: the deploy HOLD said only "role did not deliver ' +
      'devops-receipt" until the lane itself explained the cause in prose, and the two Lead HOLDs on the ' +
      'WhatsApp story each needed a run-note read to find the stage. ENG-FORGE-FAILURE-LABEL-01 (complete) ' +
      'keeps the failure CLASS with its own stage — this story adds cause and recovery action beside it, one ' +
      'record, no second home. HONEST BOUNDARY: it does not reclassify existing failures and does not ' +
      'replace the operator notes on a hold.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/failure-stage-record.test.ts`',
  },
  {
    id: 'ENG-FORGE-CRASH-TESTS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 99,
    title: 'Crashes between steps and real driver value formats are tested, not assumed',
    goal:
      'The interrupted-sequence class is exercised by tests: a crash between any two durable writes of a ' +
      'story run is simulated and the recovery asserted, and value formats are taken from the real driver ' +
      'rather than from a hand-written fixture.',
    scope:
      'a shared test harness for interrupted sequences (workflow_app/tests/helpers/), the value-format ' +
      'fixtures used by the run and claim readers, workflow_app/tests/interrupted-sequences.test.ts (new).',
    acceptance:
      'At least the completion/evidence pair and the publish/receipt pair are exercised as interrupted ' +
      'sequences with a durable-state assertion after each crash point, and each test names the write it ' +
      'interrupts. Timestamp and numeric values in the fixtures are captured from the driver in the exact ' +
      'form it emits (offset-bearing timestamptz, string numerics), and a test fails if a fixture is ' +
      'hand-normalised into a shape the driver never produces — the class of fixture that hid the ' +
      'Z-append defect in ENG-FORGE-CLAIM-CLOCK-01.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, improvement 8 of 10 ("test crashes between steps and real ' +
      'database value formats alongside pure policy tests"). Directly motivated: ENG-FORGE-CLAIM-CLOCK-01 ' +
      'was invisible to the pure tests because they used clean ISO strings, and ENG-FORGE-CRASH-WINDOW-01 ' +
      'cannot be proven without interrupted sequences. HONEST BOUNDARY: this adds tests and a harness; it ' +
      'does not change production behaviour and does not attempt full chaos engineering.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/interrupted-sequences.test.ts`',
  },
  {
    id: 'ENG-FORGE-COMMENT-DIET-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Low',
    batch: 99,
    title: 'Long historical commentary is separated from the contract it describes',
    goal:
      'A reader can tell the CURRENT contract from the history of how it got there: commentary that ' +
      'narrates a superseded behaviour is moved behind a dated marker or into the story record, and what ' +
      'remains beside the code states the rule in force.',
    scope:
      'a measurable rule applied to the forge modules (an agreed maximum lines of commentary before a ' +
      'declaration, with history allowed only under a dated heading), the files it is applied to, and ' +
      'workflow_app/tests/comment-contract.test.ts (new).',
    acceptance:
      'A rule is stated and enforced: commentary above a declaration is at most N lines and states a rule ' +
      'in force, and dated history appears only under a marker that names the date and, where one exists, ' +
      'the story or commit that made the change. The test asserts the rule on the files the story touches ' +
      'and FAILS on a file that regrows history outside a marker. The rule is recorded with the reason it ' +
      'was chosen, so a later reader can change it deliberately instead of drifting past it.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, improvement 9 of 10 ("shorten historical commentary and keep ' +
      'current contracts easy to distinguish from superseded behavior"). Honest framing from the inside: ' +
      'much of tonight long comments were written by me and by lanes as a deliberate honesty device (the ' +
      'residuals in forge-engine-runtime.ts:214-221 are load-bearing), so the rule must separate history ' +
      'from contract rather than delete either. This is a LOW-priority maintainability story and it must ' +
      'not be used to erase a stated residual. HONEST BOUNDARY: no production behaviour changes.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/comment-contract.test.ts`',
  },
  {
    id: 'ENG-FORGE-RESUME-PREVIEW-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Low',
    batch: 100,
    title: 'A held story previews what will run and what will be skipped before it resumes',
    goal:
      'Before a held story is resumed, the operator sees the exact node that will run, the prerequisites ' +
      'already satisfied, and what will be skipped — without spending a model turn.',
    scope:
      'a read-only preview over the engine instance (the resume path in agent-runtime-role-runner.ts and the ' +
      'engine task reader), workflow_app/tests/resume-preview.test.ts (new).',
    acceptance:
      'Given a held instance, the preview names the node that would run next, the evidence already in force ' +
      'for it, and any step that would be skipped (with the reason it is skipped), using only durable ' +
      'records and no model call. A preview for an instance whose hold was resolved states what changed ' +
      'since the hold was recorded.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, feature 1 of 10 (proposed capability, not verified absent). ' +
      'Motivated by tonight: resolving three holds required reading run notes to learn what would happen ' +
      'next, and one resume re-ran an Architect against frozen snapshots before failing again. Feature 4 ' +
      '(failure replay) is NOT filed here — it is ENG-FORGE-REPLAY-01 in sprint 95. HONEST BOUNDARY: ' +
      'read-only; it does not change resume semantics.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/resume-preview.test.ts`',
  },
  {
    id: 'ENG-FORGE-RELEASE-RECONCILE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 100,
    title: 'A successful external action whose receipt was lost is reconciled',
    goal:
      'When an external action (a publish or a migration) succeeded but its local receipt is missing, the ' +
      'engine can DETECT that from the external system and recover the receipt instead of assuming failure.',
    scope:
      'a reconciliation path over the publish and migration ledgers (git remote state, the migration ' +
      'ledger) plus the release lane, workflow_app/tests/release-reconciliation.test.ts (new).',
    acceptance:
      'Given a publish whose sha is present on the remote and a local receipt that is missing, ' +
      'reconciliation records the receipt from the observed remote state and names the evidence it used; ' +
      'the story then proceeds instead of HOLDing. Given a genuinely absent publish, reconciliation reports ' +
      'absence and nothing is recorded. A test drives both, and neither fabricates a receipt the external ' +
      'system does not support.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, feature 2 of 10. Directly motivated: the dev_ops lane proved a ' +
      'published sha with `git ls-remote` while the release gate held for a missing devops-receipt, and ' +
      'Astra bug 1/bug 2 are both about a release state diverging from its receipt. Astra priority: this and ' +
      'resume preview after bugs 1-4. HONEST BOUNDARY: it recovers receipts for actions that CAN be ' +
      'observed; it never invents evidence for an action that cannot.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/release-reconciliation.test.ts`',
  },
  {
    id: 'ENG-FORGE-STUCK-EXPLAIN-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 100,
    title: 'A stuck story explains its blocking condition, its owner and the smallest next action',
    goal:
      'One view answers why a story is not moving: the blocking condition in machine terms, who owns it ' +
      '(a lane, an operator, an external system), and the smallest action that clears it.',
    scope:
      'a read-only explanation over the durable records (hold record, engine task, run receipts, ledger), ' +
      'workflow_app/tests/stuck-story-explanation.test.ts (new).',
    acceptance:
      'For each blocking cause the explanation names the condition, the owner and one smallest action, from ' +
      'durable records only: an unresolved hold names the originating node and the resume target that would ' +
      'clear it; a stale claim names the claiming worker and when it was last touched; an unapplied ' +
      'migration names the file and the ledger; a missing receipt names the external check that can ' +
      'confirm it. A story that is not blocked says so rather than guessing.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, feature 9 of 10. Motivated by tonight: every one of the four ' +
      'stops needed a hand investigation (a deploy HOLD, two Lead HOLDs, a zombie still reading In ' +
      'Progress). Related and NOT re-filed: ENG-FORGE-HOLD-VISIBLE-01 (complete, hold reason visible), ' +
      'ENG-FORGE-RESUME-DOOR-01 (a door out of a hold), ENG-FORGE-MIGRATION-APPLIED-01 (the migration ' +
      'case). HONEST BOUNDARY: read-only, and it never claims a cause the records do not show.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/stuck-story-explanation.test.ts`',
  },
  {
    id: 'ENG-FORGE-ACCEPTANCE-EXPLORER-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Low',
    batch: 100,
    title: 'Each acceptance requirement shows its assertions, results and uncovered cases',
    goal:
      'For a story, every acceptance clause is clickable to the assertions that cover it, whether they ran ' +
      'and passed, and which clauses remain uncovered — so coverage is read, not inferred.',
    scope:
      'a read-only projection of the acceptance-to-assertion mapping plus proof results (the QA records and ' +
      'the mapping reader), workflow_app/tests/acceptance-explorer.test.ts (new).',
    acceptance:
      'Each clause lists its mapped assertions with their observed result (passed, failed, not run) and ' +
      'clauses with no assertion are listed as uncovered, with the count of each stated. The projection ' +
      'reads durable records only and a test asserts the three states for one clause of each kind.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, feature 3 of 10. It is the read side of ' +
      'ENG-FORGE-ACCEPTANCE-PROOF-01 (which binds clauses to assertions) and ENG-FORGE-ASSERTION-RAN-01 ' +
      '(which verifies they ran). Astra improvement 6 is the enforcement half and IS filed; this is the ' +
      'visibility half. HONEST BOUNDARY: read-only projection; no scoring and no new verdict.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/acceptance-explorer.test.ts`',
  },
  {
    id: 'ENG-FORGE-PREFLIGHT-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Low',
    batch: 100,
    title: 'A story is preflighted for route, prerequisites and likely holds before it spends tokens',
    goal:
      'Before a run starts, the engine can state the route the story would take, which prerequisites are ' +
      'already satisfied, and the named conditions that would HOLD it — from durable records, with no model ' +
      'call.',
    scope:
      'a read-only preflight over the ready gate, the shaper and the held conditions, workflow_app/tests/' +
      'preflight.test.ts (new).',
    acceptance:
      'For a story, preflight reports the route it would take, each prerequisite with satisfied or not, and ' +
      'every condition that would HOLD it by name (missing acceptance mapping, unapplied migration, ' +
      'undeclared seam, unresolved dependency). A story that would pass all gates is reported as runnable, ' +
      'and a test drives one runnable and one held case.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, feature 5 of 10. Motivated by tonight: three stories were held ' +
      'for conditions that were knowable before the first model turn (an undeclared seam, a stale findings ' +
      'set, an unapplied migration), and one whole run was spent discovering one of them. HONEST BOUNDARY: ' +
      'predictive and read-only; it never blocks a run and it does not guess at conditions the records ' +
      'cannot show.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/preflight.test.ts`',
  },
  {
    id: 'ENG-FORGE-IMPACT-EXPLORER-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Low',
    batch: 100,
    title: 'A proposed change shows what it touches before it is run',
    goal:
      'Given a story or a declared surface, the engine lists the services, routes, schemas, tables, proofs ' +
      'and records the change would touch, so blast radius is read from the repo rather than guessed.',
    scope:
      'a read-only impact projection over the declared surfaces, the route manifest, the migration ledger ' +
      'and the proof set, workflow_app/tests/impact-explorer.test.ts (new).',
    acceptance:
      'For a story with declared surfaces, the projection names the routes that serve them, the tables and ' +
      'columns written, the migrations involved, and the frozen proofs that cover them, each with the ' +
      'record it was read from. A surface matching nothing is reported as such rather than silently empty, ' +
      'and a test drives a surface that maps to a route and one that does not.',
    notes:
      'FROM ASTRA CODE REVIEW 2026-09-17, feature 8 of 10. It reuses instruments that now exist: ' +
      'SEC-ROUTE-MANIFEST-01 (every handler and its authority), ' +
      'ENG-FORGE-COLUMN-WRITER-01 (every column and its writer, DEAD-DROP included), ' +
      'ENG-FORGE-MIGRATION-APPLIED-01 (the ledger), and the declared-surface reader. HONEST BOUNDARY: ' +
      'read-only; it does not decide whether a change is safe, and it reports what the records can show.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/impact-explorer.test.ts`',
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
