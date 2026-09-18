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
      'file refuses by name — a changed migration is never re-run over an applied one. A DATA migration ' +
      '(insert or update) must also be idempotent or refuse to re-run: the checksum guard cannot make a ' +
      'migration safe that was never safe to repeat. A recording failure ' +
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
      'read as abandoned. THE UNPARSEABLE CASE IS DELIBERATE, AND THIS STORY REVERSES IT: ' +
      'db/forge-engine-task-execution.ts:121 reads `stale: !terminal && (!Number.isFinite(touched) || ' +
      'touched < staleBefore)`, so an unparseable timestamp AFFIRMATIVELY marks a non-terminal row stale — ' +
      'the Z-append is not a comparison that quietly fails, it is a green light to reap a live lane. A ' +
      'reaper must never reap what it cannot measure, so the unparseable case flips to NOT stale in this ' +
      'story, with the reason recorded beside it. This is the same surface ENG-FORGE-REAP-GUARD-01 (sprint ' +
      '94) protects from the other side: that story says the reaper cannot kill a live lane, and this is ' +
      'one way the reaper could believe a lane is dead. Verified rather than assumed: my first read of this ' +
      'bug was that NaN would read as NOT stale, and the code proved me wrong — Astra wording ("marks ' +
      'nonterminal rows stale") was exact. HONEST BOUNDARY: this fixes the parse and reverses the ' +
      'unparseable default; it does not change the staleness threshold and does not add the heartbeat that ' +
      'REAP-GUARD-01 owns.',
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
    id: 'ENG-FORGE-PROOF-NAMES-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 99,
    title:
      'An assertion ref names something the proof can print: a file-qualified ref resolves by its name tail',
    goal:
      'A ref written path#name (the Forge convention for a named thing inside a file) is satisfied by a ' +
      'marker line that names that assertion, so a correct mapping is never UNPROVEN because of how the ref ' +
      'was spelled.',
    scope:
      'assertion ref resolution in the QA adjudicator (workflow_app/forge/agents/qa/run.ts, the ' +
      'assertionOutcome reader), workflow_app/tests/assertion-executed.test.ts (extend with the ' +
      'file-qualified ref cases), and the directive/prompt text that states the accepted ref format where ' +
      'the mapping is declared.',
    acceptance:
      'A ref of the form <path>#<name> is satisfied by a marker line whose text contains <name>, and the ' +
      'clause is PASS. A ref of that form whose <name> appears only on a non-marker line (a suite header, ' +
      'echoed source) is still UNPROVEN, so the marker rule is not weakened to gain tolerance. A ref with ' +
      'no # behaves exactly as today. A failed marker line naming <name> is FAIL, never UNPROVEN. The ' +
      'accepted ref format is stated where the mapping is declared, so a lane can write a ref the proof is ' +
      'able to print. A test drives every case.',
    notes:
      'CORRECTED 2026-09-17 18:20 (Cline) — MY EARLIER DIAGNOSIS ON THIS STORY WAS WRONG AND THIS IS THE ' +
      'MEASURED CAUSE. I filed it at 18:06 saying the PROOF was too old to carry names. That was wrong: the ' +
      'proof for ENG-FORGE-MIGRATION-START-01 DOES name each assertion ("refuses when the repo list carries ' +
      'migrations the ledger lacks" appears on a marker line, and the file passes 10/10). The real defect is ' +
      'a GRAMMAR COLLISION: the mapping declared the ref ' +
      '"workflow_app/tests/migration-preflight.test.ts#refuses when the repo list carries migrations the ' +
      'ledger lacks", but the reader requires the whole ref to appear on a marker line, and a marker line ' +
      'carries the NAME, never the PATH — so a file-qualified ref can never match anything, no matter how ' +
      'recent or complete the proof is. It is UNPROVEN by SPELLING, not by AGE. Two facts made this easy to ' +
      'get wrong and are worth keeping: (1) the canonical ref form in the ACCEPTANCE-PROOF tests is a BARE ' +
      'name ("asserts-wired-flag" against marker line "asserts-wired-flag"), which is why ' +
      'ENG-FORGE-ASSERTION-RAN-01 (whose lane declared bare names) PASSed while MIGRATION-START-01 (whose ' +
      'handoff declared path#name) did not; (2) the Forge already uses path#symbol everywhere else (seams, ' +
      'scope), so a lane reaching for a file-qualified ref follows our own convention rather than making a ' +
      'mistake. So the fix is BOTH: the reader resolves a path#name ref by its name tail while still ' +
      'demanding that tail sit on a marker line, and the ref format is stated where lanes declare it. Sibling ' +
      'of ENG-FORGE-ASSERTION-RAN-01 (an assertion must have RUN) and ENG-FORGE-START-BASE-01 (a lane ' +
      'records the base it started from): all three are a ruling resting on something the run never ' +
      'established. HONEST BOUNDARY: this does not weaken the marker rule (a name echoed from source still ' +
      'does not count) and does not let a bare count buy a PASS.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/assertion-executed.test.ts`',
  },
  {
    id: 'ENG-FORGE-STALE-READER-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 98,
    title:
      'A ruling names the revision it was made by: a verdict from code older than the candidate is stale, not authoritative',
    goal:
      'A QA ruling carries the revision of the adjudicator that produced it, and a ruling made by code older ' +
      'than the change it judged is flagged as stale rather than reported as a verdict on that change.',
    scope:
      'the QA verdict record and the adjudicator revision stamp (workflow_app/forge/agents/qa/run.ts, ' +
      'workflow_app/forge/agents/qa/types.ts) plus workflow_app/tests/reader-revision.test.ts (new).',
    acceptance:
      'A QA ruling records the code revision the adjudicator ran, and a ruling made by code OLDER than the ' +
      'candidate change is flagged as stale rather than reported as a verdict on the candidate. A fresh ' +
      'process that judges the same candidate and the same proof is not flagged. A test drives both, ' +
      'including a reader loaded before the change it is judging.',
    notes:
      'LIVE EVIDENCE 2026-09-17 18:36 (Cline) — A RULING MADE BY CODE OLDER THAN THE FIX IT JUDGED. The ' +
      'engine process for ENG-FORGE-PROOF-NAMES-01 started 18:25:53; its lane committed the adjudicator fix ' +
      'at 18:29:58. QA then ruled UNPROVEN (18:31) using the PRE-FIX reader held in that process memory, ' +
      'reporting ASSERTION_NOT_RUN against refs the committed fix resolves. Restarting the engine and ' +
      're-verifying the same candidate against the same proof returned QA PASS at 18:36 with nothing else ' +
      'changed — the only difference was the code in the process. So an engine process silently adjudicates ' +
      'with the revision it started on, and its verdict is indistinguishable from one made by current code. ' +
      'Same family as ENG-FORGE-START-BASE-01 (a lane records the base it started from) and ' +
      'ENG-FORGE-ASSERTION-RAN-01 (an assertion must have run): a ruling must name the thing it actually ' +
      'rested on. It cost one full run and two door resumes to establish, and the fix is small: stamp the ' +
      'adjudicator revision into the verdict. HONEST BOUNDARY: this does not force a reload mid-run and does ' +
      'not change what a PASS means — it makes a stale ruling visible instead of authoritative.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/reader-revision.test.ts`',
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
    id: 'ENG-FORGE-QA-VERDICT-VOCAB-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 98,
    title:
      'A QA verdict fits the column it is written to: one vocabulary for the writer, the constraint and the reader',
    goal:
      'Every QA disposition the code writes is one the database accepts and the repair router can classify, so ' +
      'a verdict is never lost between the code that writes it and the column that guards it.',
    scope:
      'the disposition vocabulary and its writers (db/forge-repair-ledger.ts, workflow_app/forge/qa-repair-policy.ts) ' +
      'with the matching constraint, workflow_app/tests/qa-disposition-vocab.test.ts (new).',
    acceptance:
      'Every disposition the code writes is accepted by storyboard_story_forge_qa_disposition_check, proven by a ' +
      'test that drives each writer value against the constraint vocabulary rather than by inspection. A clean QA ' +
      'PASS is recorded and read back as a pass, and a REPAIR/REPLAN/ESCALATE failure is recorded and read back as ' +
      'that action. The reader never returns a value the repair router cannot classify: an unknown stored value is ' +
      'reported as unknown rather than defaulted into a repair action. A test drives a pass write, a failure write ' +
      'and an unknown stored value.',
    notes:
      'MEASURED 2026-09-17 20:30 (Cline), from a live failure then from the whole board. ' +
      'ENG-FORGE-LANE-FAILURE-01 failed QA three times, twice in a freshly started process, with the causes finally ' +
      'visible: lane failed with "new row for relation storyboard_story violates check constraint ' +
      'storyboard_story_forge_qa_disposition_check"; release failed with "Task cannot be released in status: ' +
      'completed". The constraint allows only REPAIR, REPLAN and ESCALATE (37bd3297). db/forge-repair-ledger.ts:129 ' +
      'writes PASS (added by f8629348, whose message says "A CLEAN QA PASS IS A DISPOSITION TOO"). So the PASS ' +
      'writer has NEVER been able to land: every story that passed QA tonight reads forge_last_qa_disposition = ' +
      'null, and the column is null on ALL 407 rows of the board. Consequence, stated plainly: the fix that was ' +
      'meant to STOP THE INFINITE QA LOOP by recording a clean pass never recorded anything in production, and the ' +
      'chain could not read the signal it was given. It stayed invisible because the write failure was swallowed; ' +
      'ENG-FORGE-QA-RACE-01 then made the disposition write awaited inside the completion unit, so the same ' +
      'failure now propagates as a worker crash AFTER the task completed and the instance advanced — which is why ' +
      'the run looks like a crash while the board shows progress. HONEST BOUNDARY: this is a vocabulary/writer ' +
      'disagreement, and the fix must settle the vocabulary in ONE place (the code that writes it, the constraint ' +
      'that guards it, and the reader that casts it) rather than widening the constraint alone — a PASS read back ' +
      'as a repair disposition would misroute the next lane.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/qa-disposition-vocab.test.ts`',
  },
  {
    id: 'ENG-FORGE-RELEASE-CLEANUP-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 98,
    title: 'A release that cannot apply because the task completed is a no-op, not a crash',
    goal:
      'A lane that fails after its task already completed reports the failure without turning a run that advanced ' +
      'into a reported crash, and the wave still settles its siblings.',
    scope:
      'the release cleanup path in workflow_app/forge/forge-executor.ts (runReady) and ' +
      'workflow_app/tests/release-after-completion.test.ts (new).',
    acceptance:
      'Releasing a task that is already completed returns without error, because completing is strictly stronger ' +
      'than releasing, and a test drives that exact case. A release that fails for any other reason still throws ' +
      'and is reported. A lane that fails after its task completed does not turn a run that advanced into a ' +
      'reported failure: the failure is reported, the wave settles its siblings, and the advance stands.',
    notes:
      'MEASURED 2026-09-17 20:30 (Cline) — THE CLEANUP IS LOUDER THAN THE FAILURE. When a lane fails AFTER its ' +
      'task was already completed, forge-executor.ts attempts releaseForgeRoleTask as cleanup, which throws ' +
      '"Task cannot be released in status: completed"; the two errors are then aggregated and the worker exits 1. ' +
      'The story had already ADVANCED past the node, so the process reported failure for a run that made ' +
      'progress. A release that cannot apply because the task completed is a no-op, not an error. ' +
      'HONEST BOUNDARY: this does not swallow real release failures (a task claimed by someone else, or a release ' +
      'that cannot be verified, still throws).',
    assayCommands: '- `node --import tsx --test workflow_app/tests/release-after-completion.test.ts`',
  },
  {
    id: 'ENG-FORGE-FINDING-DEDUPE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 98,
    title: 'Re-running a node retires what it superseded: a duplicate finding id cannot deadlock routing',
    goal:
      'An operator re-run of a node leaves exactly one set of findings in force for that story and node, so ' +
      'routing can never be blocked by rows a re-run created.',
    scope:
      'the finding write and the routing reader (workflow_app/forge/forge-lead-routing.ts, the architect ' +
      'findings writer in workflow_app/forge/agents/architect/persist.ts), workflow_app/tests/' +
      'finding-dedupe.test.ts (new).',
    acceptance:
      'After an operator re-run of the architect, reviewLeadProposal sees ONE set of finding ids for that story ' +
      'and node: either the superseded task rows are retired when the new one writes, or the reader dedupes by ' +
      'newest task, and a test drives the two-task case with overlapping ids. A route is never refused because ' +
      'of a row a re-run created. The supersede leaves an operator-legible record of which task was retired and ' +
      'why, so a manual cleanup is never the only remedy.',
    notes:
      'LIVE EVIDENCE 2026-09-17 20:48-21:02 (Cline) — A RE-RUN DEADLOCKED THE LEAD, AND THE LEAD NAMED IT. The ' +
      'operator door resumed ENG-FORGE-QA-VERDICT-VOCAB-01 at ARCHITECT (as the lead asked, to recut to <=2 units). ' +
      'That created a second architect task (faafc660) which wrote 5 findings, while the first task (203c226a) had ' +
      'already written 4 of the same ids. Both are attempt 1, so the attempt-based supersede rule from ' +
      'ENG-FORGE-FINDING-SUPERSEDE-01 cannot reach them, and lead_pre returned HOLD twice with: "the lead routing ' +
      'context carries nine finding rows for five ids ... reviewLeadProposal refuses duplicate finding ids ' +
      '(forge-lead-routing.ts:164) so no non-HOLD route can validate. The engine or operator must retire the ' +
      'superseded architect task\'s finding rows, or dedupe by newest task, before lead_pre can route." The only ' +
      'remedy was manual: deleting 4 rows by task id. After that the lead routed SMITH immediately. So the cost of ' +
      'this gap is (a) a stranded story and (b) an operator hand-editing prod rows to unblock the factory, for an ' +
      'action the door itself encourages. Sibling of ENG-FORGE-FINDING-SUPERSEDE-01 and ENG-FORGE-FINDING-DROP-01, ' +
      'and the same family as ENG-FORGE-START-BASE-01: a re-run must supersede, not accumulate. HONEST BOUNDARY: ' +
      'this does not change how findings are shaped or which of two competing findings wins - it removes the ' +
      'duplicate that leaves routing no legal route at all.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/finding-dedupe.test.ts`',
  },
  {
    id: 'ENG-FORGE-PROOF-SEAM-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'A finding owns its proof: a two-unit story can fan out because each unit declares the fence it will add',
    goal:
      'A finding that requires a NEW proof declares that proof path inside its own seams, and two units that add ' +
      'distinct files under one shared test directory are not treated as overlapping surfaces, so a story with two ' +
      'independent units can lawfully split.',
    scope:
      'the architect handoff and its seam rules (workflow_app/forge/forge-shaping.ts, forge-architect-contract.ts, ' +
      'the findings writer in workflow_app/forge/agents/architect/persist.ts) and the HARD SCOPE RULE reader in ' +
      'workflow_app/forge/agent-runtime-role-runner.ts, plus workflow_app/tests/proof-seam.test.ts (new).',
    acceptance:
      'Given two required findings whose units are disjoint but whose new proofs land in the same tests directory, ' +
      'the split is ALLOWED and each sibling is handed its own proof path as part of its seams; a test drives that ' +
      'exact shape and asserts two assignments. A finding whose required proof is NOT inside its seams is refused ' +
      'at shaping time by name, so the refusal arrives before the wave rather than as a HOLD after routing. Two ' +
      'units that would genuinely edit the SAME file remain refused, unchanged.',
    notes:
      'MEASURED 2026-09-18 by Cline, from a purpose-built I5 attempt. Grok\'s nightly review named I5 as the first ' +
      'of three reasons the factory is not at 100: "they tried a two-area story; it still came out one unit / one ' +
      'SMITH. That is intake, not the scheduler." ENG-FORGE-SPLIT-DOGFOOD-01 could not be the vehicle (its units ' +
      'are already implemented and green at baseRef 3e4b48c0, so the lead correctly HOLDed it). So ' +
      'ENG-FORGE-REVIEW-RESIDUALS-01 was cut deliberately with two independent units in disjoint files, run with a ' +
      'wave cap of 2. Result: the architect DID declare two required findings with disjoint seams ' +
      '(cli-adapter-allowed-scope, run-spend-source-closed) and the lead still chose SMITH, refusing SPLIT for a ' +
      'precise, mechanical reason in its own words: "F2\'s required proof workflow_app/tests/run-spend-source.test.ts ' +
      'is not inside F2\'s seams and both units\' tests share the workflow_app/tests/ parent, so a sibling split ' +
      'needs an undeclared seam and overlapping directories, refused by the HARD SCOPE RULE." This is the SECOND ' +
      'occurrence of the same mechanism tonight: ENG-FORGE-QA-VERDICT-VOCAB-01 HOLDed for "the mandatory new proof ' +
      'workflow_app/tests/qa-disposition-vocab.test.ts is not inside any declared finding seam". So the intake gap ' +
      'is systematic, not incidental: a unit is not given the path it must create. HONEST BOUNDARY: this does not ' +
      'raise the split cap, does not change which units are independent, and does not weaken the shared-file ' +
      'refusal - Grok\'s instruction stands: do not raise the cap, do not open PARALLEL-WAVE-02, and a postcard is ' +
      'owed only when the wave log actually shows two smith_split_work lanes.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/proof-seam.test.ts`',
  },
  {
    id: 'ENG-FORGE-REVIEW-RESIDUALS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'Two independent residuals: the cli adapter commits inside a scope, and every run carries a closed spend source',
    goal:
      'Close the two residuals the nightly review named as the reasons the factory is not at 100, each in its own ' +
      'surface: the cli agent adapter gains the same allowedScope discipline the factory path already has, and a run ' +
      'records WHICH kind of spend it carried so the dollars column can never hold widgets.',
    scope:
      'Unit A (adapter scope): agent-runtime/cli-agent-adapter.ts and the commit path it calls, plus ' +
      'workflow_app/tests/cli-adapter-scope.test.ts (new). Unit B (spend source): the run writer and reader in ' +
      'db/forge-run.ts and db/storyboard.ts plus workflow_app/tests/run-spend-source.test.ts (new). The two units ' +
      'share no file, so they are independent by construction.',
    acceptance:
      'Unit A: the cli adapter refuses a commit whose paths fall outside the declared scope, naming the offending ' +
      'path, and a test drives an in-scope commit and a refused one; the check reuses the same allowedScope rule as ' +
      'the factory path rather than a second implementation. Unit B: every run row carries a spend source from a ' +
      'closed vocabulary (vendor, widgets, none), absence is recorded as none rather than left null, widgets are ' +
      'never written to the dollars column, and a test drives all three sources including the none case.',
    notes:
      'FILED 2026-09-18 by Cline from Grok\'s nightly review (docs/agent/mailbox/GROK-2026-09-18-review.md, grade 95). ' +
      'His named reasons for not giving 100: "cli-agent-adapter still commits with no scope" and "spend coverage is ' +
      'still a floor until every run has a source". The third reason (I5 fan-out unobserved) is an observation, not ' +
      'a code change: ENG-FORGE-SPLIT-DOGFOOD-01 tried to be its vehicle but the lead correctly HOLDed it because ' +
      'both of its units are already implemented and green at baseRef 3e4b48c0 - no legitimate Smith edit remains, ' +
      'and the story needs a live SPLIT run rather than a code change. This story is deliberately built with TWO ' +
      'INDEPENDENT UNITS in disjoint surfaces so the scheduler has a lawful opportunity to fan out: if the wave log ' +
      'shows two smith_split_work lanes, I5 fires and the postcard can be written from the evidence; if it collapses ' +
      'to one unit and one Smith, that is the intake fact recorded instead. HONEST BOUNDARY: this story does not ' +
      'change the split cap or the scheduler - do not raise the cap, and do not open PARALLEL-WAVE-02.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/cli-adapter-scope.test.ts`\n' +
      '- `node --import tsx --test workflow_app/tests/run-spend-source.test.ts`',
  },
  {
    id: 'ENG-FORGE-SPLIT-SHAPE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'A SPLIT child is written in the shape the engine can claim: lane, size and its own assignment',
    goal:
      'A two-unit story can fan out end to end: its sibling work items are written with every column the parallel ' +
      'shape check requires, so both lanes are claimable and the per-slot index still keeps them from colliding.',
    scope:
      'the work-item writer (db/agent-work.ts, the agent_work_item insert and its enqueue preconditions) and the ' +
      'SPLIT materialization that calls it, plus workflow_app/tests/split-child-shape.test.ts (new).',
    acceptance:
      'Enqueuing a SPLIT child writes lane, parallel_group_id, parallel_slot, parallel_size and a non-empty ' +
      'split_assignment, so the row satisfies agent_work_item_parallel_shape_check and is claimable; two siblings ' +
      'share a group with distinct slots and the one-parallel-slot index still refuses a duplicate slot; a ' +
      'non-parallel row keeps writing NULL for all three columns and is unaffected. A test drives the child shape, ' +
      'the duplicate-slot refusal and the plain row, and the story is verified by a real two-unit wave whose two ' +
      'lanes both start.',
    notes:
      'MEASURED 2026-09-18 06:00 by Cline, and it is the reason the SPLIT lane has never actually fanned out. ' +
      'With intake fixed (PROOF-SEAM-01) a two-unit story finally routed SPLIT:2 and the wave log shows what Grok ' +
      'asked for - "wave: cap 2 - lanes smith_split_work, smith_split_work" and "running smith_split_work + ' +
      'smith_split_work concurrently (cap 2)" - and then BOTH lanes rejected with the same named cause: ' +
      '"new row for relation agent_work_item violates check constraint agent_work_item_parallel_shape_check". ' +
      'No task rows were persisted, so no smith ever ran. The constraint (migration 166) allows a parallel row ' +
      'only when parallel_group_id is set AND lane = smith AND slot is 1..3 AND size is 2..3 AND slot <= size AND ' +
      'split_assignment is non-empty; the writer at db/agent-work.ts:776-787 inserts parallel_group_id and ' +
      'parallel_slot and NOTHING ELSE - no lane, no parallel_size, no split_assignment, and none of the three ' +
      'has a column default. So the check can never pass for a child row, and the fan-out dies before it starts. ' +
      'Two blockers were hiding behind one another: intake could not produce two units, and the write path could ' +
      'not enqueue them. HONEST BOUNDARY: this does not raise the split cap, does not change which units are ' +
      'independent, and does not weaken the shape check - the check is right and the writer is wrong.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/split-child-shape.test.ts`',
  },
  {
    id: 'ENG-FORGE-TWO-UNIT-DOGFOOD-02',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 97,
    title: 'Two independent units of real work, filed as one story: a media download declares its length, and anonymous diagnostics cannot be flooded',
    goal:
      'Deliver the two app-hardening items in one story with two declared, disjoint units, and use the wave log as ' +
      'the evidence for whether the scheduler can fan a two-unit story out.',
    scope:
      'Unit A (media length): app/api/media/documents/[id]/route.ts and any sibling that sets Content-Length, plus ' +
      'workflow_app/tests/media-content-length.test.ts (new). Unit B (diagnostic throttle): ' +
      'app/api/portal/move-trace/route.ts and app/api/portal/client-error/route.ts, plus ' +
      'workflow_app/tests/diagnostic-throttle.test.ts (new). The two units share no file and each owns its own new ' +
      'proof path, which is the shape ENG-FORGE-PROOF-SEAM-01 made lawful.',
    acceptance:
      'Unit A: the Content-Length header is derived from the bytes actually sent; a row whose file_size disagrees ' +
      'with its bytes still streams completely; a test proves the mismatch case, because that is the case that ' +
      'breaks. Unit B: each endpoint bounds its writes per source per window and per body size, refuses over the ' +
      'bound with a plain response, and records the refusal as a count rather than a row per attempt, with ' +
      'anonymity preserved by design. The wave log for this run is retained as the I5 evidence: two ' +
      'smith_split_work lanes in one wave, or the record of what collapsed it to one unit.',
    notes:
      'CUT 2026-09-18 by Cline, deliberately, as the third attempt at Groks I5: "cut one story whose required ' +
      'findings are two independent seam groups. Cap 2. Postcard with two smith_split_work lines in one wave, or ' +
      'no postcard." Attempt 1 (ENG-FORGE-SPLIT-DOGFOOD-01) could not be the vehicle: its units are already ' +
      'implemented and green, so its lead correctly HOLDed rather than edit passing code. Attempt 2 ' +
      '(ENG-FORGE-REVIEW-RESIDUALS-01) had two units and the architect DID declare two findings, but the lead ' +
      'refused SPLIT because a required proof was not inside its own seams - which is what PROOF-SEAM-01 fixed. ' +
      'This story therefore pairs two REAL backlog items (APP-MEDIA-LENGTH-01 and APP-DIAGNOSTIC-THROTTLE-01) with ' +
      'disjoint surfaces and distinct new fences. When it lands, those two items are delivered here and are closed ' +
      'as such rather than worked twice. HONEST BOUNDARY: the cap stays 2, PARALLEL-WAVE-02 stays unopened, and if ' +
      'the wave shows one lane that is the fact recorded - the story still delivers its two units either way.',
    assayCommands:
      '- `node --import tsx --test workflow_app/tests/media-content-length.test.ts`\n' +
      '- `node --import tsx --test workflow_app/tests/diagnostic-throttle.test.ts`',
  },
  {
    id: 'ENG-FORGE-SCOPE-OWN-CHANGES-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'A lane is judged by its own changes, not by everything that landed while it ran',
    goal:
      'A lane that edits only its assignment is never refused because an unrelated commit landed on main during its ' +
      'run, so a SPLIT child cannot be held for the work of another lane or the operator.',
    scope:
      'the scope check that reads the candidate diff (workflow_app/forge/story-scope-base.ts and its callers in ' +
      'workflow_app/forge/agent-runtime-role-runner.ts), plus workflow_app/tests/scope-attribution.test.ts (new).',
    acceptance:
      'The changes judged against a lane assignment are the candidate own changes - its diff against its parent ' +
      'plus any commits the lane authored in the range - so a foreign commit that landed during the run is not ' +
      'attributed to the lane; a lane that genuinely edits a file outside its assignment is still refused by name; ' +
      'a candidate that is not a descendant of its recorded base is still refused; a test drives a range containing ' +
      'a foreign commit, a genuine out-of-scope edit, and the clean case.',
    notes:
      'MEASURED 2026-09-18 09:34 by Cline, while proving SPLIT-SHAPE-01 end to end. The shape fix worked - a ' +
      'parallel child row now exists with group, slot 2 of 2, lane smith and its own assignment, and it reached ' +
      'state Done, where previously EVERY insert was refused and no row existed at all. Then the lane was HOLDed: ' +
      '"candidate 5c373dc02 touched files outside its assignment (unit-b-diagnostic-throttle)" and the list was ' +
      'agent-runtime/repositories.ts, db/agent-work.ts, db/forge-artifact.ts, docs/agent/postcards/..., ' +
      'scripts/forge-test-stories.ts, workflow_app/forge/agent-runtime-role-runner.ts, ' +
      'workflow_app/forge/forge-executor.ts, workflow_app/tests/artifact-verdict.test.ts and ' +
      'workflow_app/tests/split-child-shape.test.ts - EVERY ONE OF THEM MINE, committed to main while the lane was ' +
      'held. So the refusal is correct about the range and wrong about the attribution: a lane is not responsible ' +
      'for commits that are not its own. This is the third layer of one family - START-BASE-01 recorded where a ' +
      'lane started, PROOF-SEAM-01 gave a unit its own proof, and this judges the lane own diff rather than the ' +
      'interval - and it is why a SPLIT child in a busy repository could not survive even after the shape fix. ' +
      'OPERATOR LESSON, recorded honestly: I committed six times to main while a lane was in flight tonight, which ' +
      'is what created the interval. The engine should be robust to that rather than depending on my timing, but ' +
      'until this lands the operator should not commit while a lane is mid-run. HONEST BOUNDARY: this does not ' +
      'loosen the refusal for a lane own edits, and a foreign commit that EDITS the assignment surfaces is still ' +
      'a foreign commit - the surfaces themselves must still be touched only by the lane.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/scope-attribution.test.ts`',
  },
  {
    id: 'ENG-FORGE-SECRET-HISTORY-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 101,
    title: 'The credentials captured by agent checkpoints are purged or rotated, and the capture path is closed',
    goal:
      'The environment-file backup that agent checkpoint refs captured is purged, or its credentials are ' +
      'rotated, and no future secret reaches a checkpoint ref un-noticed.',
    scope:
      'the nine refs/cline/checkpoints refs that hold the file (purged or kept, decided against the ' +
      'restore points it costs), the operator rotation decision at the providers, docs/agent/PERIMETER.md, ' +
      'and proof that the capture path is closed by the new .gitignore rule.',
    acceptance:
      'The exposure is stated with its true scope — local shadow refs, never pushed — rather than as a ' +
      'published leak; the operator records a rotate-or-accept decision per credential with the reason; ' +
      'either the nine checkpoint refs are pruned and the blob is shown unreachable afterwards, or ' +
      'keeping them is recorded with its cost; a fresh checkpoint of an untracked env file is shown to ' +
      'be ignored by the new rule; and `pnpm scan:secrets` stays green.',
    notes:
      'CORRECTED 2026-09-18, and the correction is the point. My first reading of the gitleaks report was ' +
      'wrong in the way that matters: it said `.env.local.before-icloud-username-fix` sat in 9 commits, ' +
      'I read "commits" as repository history, and I told the operator their production credentials were ' +
      'published and must be rotated. THEY WERE NEVER PUSHED. The nine commits are CLINE CHECKPOINT ' +
      'commits — shadow refs under `refs/cline/checkpoints/*`, message "untracked files on cline ' +
      'checkpoint" — and the blob (297c48e2) is reachable from exactly those nine checkpoint refs and ' +
      'from NO branch and NO remote ref. Confirmed against GitHub as well: ' +
      '`gh api repos/culebraluxe/Culebraluxe-web/contents/.env.local.before-icloud-username-fix` is 404 ' +
      'and the commit is not on the remote. A gitleaks `Link` field LOOKS like a GitHub URL and is not ' +
      'one — it is the tool constructing a plausible URL from the remote name, and I read it as evidence ' +
      'of publication. THE MECHANISM, which also answers how it "slipped in" with nobody git-adding it: ' +
      'Cline checkpoints snapshot UNTRACKED files, so a plaintext backup that no human ever staged was ' +
      'captured by a tool. The old `.gitignore` rule `.env*.local` could not match a name ending in ' +
      '`-fix`, so nothing stopped it; that rule is fixed in the same commit as this story (`.env*` with ' +
      '`!.env.example`). RISKS THAT REMAIN, stated without inflating them: (1) the secrets sit in local ' +
      'git objects that survive deleting the working file; (2) `git push --mirror` would publish every ' +
      'ref including refs/cline, while `git push --all` would not, since it carries refs/heads only; ' +
      '(3) any full copy of this clone carries them. Rotation is therefore prudent for the cheap-to-' +
      'regenerate crown jewels (AUTH_SECRET, the Neon passwords, the WhatsApp and BoldSign keys) but it ' +
      'is NOT an emergency, and the operator decides per credential. PURGE OPTION, with its real cost: ' +
      '`git for-each-ref refs/cline/checkpoints` can be pruned and the blob dropped with ' +
      '`git gc --prune=now`, at the price of the Cline restore points tied to the refs that hold it — ' +
      'nine of 1179 refs. HONEST BOUNDARY: rotation is the only remedy that ends the risk if the keys ' +
      'were ever shared or copied; purging local objects does not un-expose anything that has already ' +
      'left this machine.',
    assayCommands: '- `pnpm scan:secrets` (baseline must not grow); the blob unreachable after a purge',
  },
  {
    id: 'ENG-FORGE-CI-PRODUCTION-FENCE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 101,
    title: 'A failing gate cannot reach production: the deploy waits for the workflow',
    goal:
      'A commit that fails the static gates is never serving traffic, so the workflow protects the ' +
      'deployment boundary rather than merely reporting on it afterwards.',
    scope:
      '.github/workflows/gates.yml (landed gates-only), the Vercel project settings for the ' +
      'production branch, and docs/agent/PERIMETER.md (deploy-boundary section).',
    acceptance:
      'The workflow reports green or red on every push to main and every pull request; production ' +
      'deployment either waits for that result or is unreachable without it; a deliberately failing ' +
      'gate is shown to keep the previous production deployment serving; and the first real CI run ' +
      'is recorded with its outcome, including whether the gitleaks and osv-scanner action ' +
      'invocations were correct.',
    notes:
      'THE HOLE, STATED PLAINLY: the workflow as landed does not protect production. Vercel deploys ' +
      '`main` directly, so a push that fails the gates has already been deployed by the time the ' +
      'check reports — the check is observational. Two legitimate designs, and the operator picks: ' +
      '(a) disable the automatic production deployment for main and deploy from the workflow after ' +
      'the gates pass; or (b) require pull-request checks so nothing reaches main without a green ' +
      'run. The operator works main-only, which is why (a) is the smaller change. WHY THE WORKFLOW ' +
      'EXISTS AT ALL, measured: on 2026-09-18 three lint errors shipped through two clean QA passes ' +
      'because the engine static gate runs semgrep, knip and tsc but not eslint — the gate was ' +
      'independent evidence of a gap, not a theory. UNVERIFIED ON FIRST RUN, declared rather than ' +
      'assumed: the workflow was validated with actionlint 1.7.12 (exit 0) but has never executed; ' +
      'the two third-party action invocations (gitleaks/gitleaks-action@v2 and ' +
      'google/osv-scanner-action@v2) must be confirmed against their current README on the first ' +
      'push, and the osv step deliberately carries `--all-packages` because without it osv-scanner ' +
      'reported 17 packages against a lockfile holding 899. The database job is parked behind the ' +
      'repository variable FORGE_DB_CI until dev credentials exist as secrets.',
    assayCommands: '- `actionlint .github/workflows/gates.yml` (must exit 0)',
  },
  {
    id: 'ENG-FORGE-MIGRATION-LINT-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 101,
    title: 'A migration that can lock a live table is refused before it is applied',
    goal:
      'An unsafe migration statement is refused by the engine with the rule that found it, so no lane ' +
      'can apply a blocking index, an unvalidated constraint or a NOT NULL column to a live database ' +
      'without a recorded reason.',
    scope:
      'the engine static gate (workflow_app/forge/forge-static-gate.ts) and the Assay path that calls ' +
      'it, scripts/scan-migrations.sh (landed), plus a fence in workflow_app/tests/.',
    acceptance:
      'A migration adding an index without CONCURRENTLY, or a constraint without NOT VALID, is ' +
      'refused by the engine with the squawk rule id in the refusal; a migration that is safe passes; ' +
      'the check runs against changed migration files only, so the historical findings never block ' +
      'unrelated work; and the fence drives both cases.',
    notes:
      'LANDED PARTIALLY 2026-09-18 by Cline: the operator installed squawk-cli 2.65.0 and ' +
      '`pnpm scan:migrations` now lints changed migrations (committed range plus uncommitted, ' +
      'BASE_REF defaults to origin/main), with a CI step that does the same. What remains is the ' +
      'engine wiring: today a lane can still write an unsafe migration because the static gate does ' +
      'not consult squawk. MEASURED on the full historical set, worth keeping in view: 745 findings ' +
      'across 192 files — 145 indexes created without CONCURRENTLY, 161 missing lock_timeout and 161 ' +
      'missing statement_timeout, 50 constraints added without NOT VALID, 15 foreign-key additions, ' +
      '5 column-type changes and 3 dropped columns. The newest 25 migrations still carry 108 of ' +
      'them (10 concurrent-index, 13 not-valid), which is why this is worth a gate rather than a ' +
      'note. WHY CHANGED-ONLY: a gate that is red on day one is a gate everyone learns to ignore, ' +
      'and those files are already applied. HONEST BOUNDARY: squawk reasons about a statement in ' +
      'isolation — it cannot know a table is empty in production and therefore safe, so a legitimate ' +
      'exception needs a recorded reason, not a silenced rule.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/migration-lint.test.ts`\n- `node --import tsx scripts/forge-static-gate.ts`\n- `pnpm scan:migrations`',
  },
  {
    id: 'ENG-FORGE-LANE-SECRET-GATE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 101,
    title: 'A lane cannot publish a candidate that carries a credential',
    goal:
      'A candidate whose own changes contain a secret is refused before it is published, so an agent ' +
      'that pastes a token into source, a test or an artifact cannot move it into history.',
    scope:
      'the publish path in workflow_app/forge/agent-runtime-role-runner.ts (or the gate it consults), ' +
      'a secrets scan over the candidate own diff rather than the whole repository, and a fence that ' +
      'drives a planted key.',
    acceptance:
      'A candidate whose diff adds a credential-shaped string is refused with the rule that matched ' +
      'and the file named; a candidate with no credential passes; the scan reads the candidate own ' +
      'changes only, so an unrelated exposure in history never blocks a lane; and the fence proves ' +
      'both directions.',
    notes:
      'WHY THIS IS THE SHARP ONE, measured 2026-09-18: the repository already lost a credential file ' +
      'to history, and the factory is now the most likely source of the next one — a lane writes ' +
      'source, tests, artifacts and postcards, and the existing redactor (TRIAGE_REDACTOR_VERSION) ' +
      'guards only failure-triage payloads, not what an agent writes to disk. The instruments exist: ' +
      'gitleaks 8.30.1 with `.gitleaks.toml` (custom shapes for Neon URLs, WhatsApp EAA tokens, ' +
      'BoldSign keys, Apple app-specific passwords, xAI keys) and `pnpm scan:secrets`. The shape of ' +
      'the check mirrors SCOPE-OWN-CHANGES-01 on purpose: scan the candidate own diff, never the ' +
      'whole history, or every lane inherits the 147 findings this repository already knows about ' +
      'and refuses forever. HONEST BOUNDARY: entropy detection gives false positives and false ' +
      'negatives — this narrows the window, it does not replace rotation, and a refusal must be ' +
      'reviewable and overridable with a recorded reason rather than a silent pass.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/lane-secret-gate.test.ts`\n- `pnpm typecheck`',
  },
  {
    id: 'ENG-FORGE-BOUNDARY-SCHEMAS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 101,
    title: 'Anything crossing the process boundary is unknown until a schema validates it',
    goal:
      'Untrusted input is validated at runtime before it is used, so a public webhook, route body or ' +
      'provider response cannot reach the domain as an unchecked cast.',
    scope:
      'schema validation for the WhatsApp webhook payload (app/api/integrations/whatsapp/webhook/' +
      'route.ts), then the other route bodies and provider responses, with the library declared in ' +
      'package.json.',
    acceptance:
      'The webhook validates the parsed body against a schema and returns a 400 without touching the ' +
      'domain when validation fails; signature verification still runs before any payload handling; ' +
      'an inferred type replaces the hand-written payload type so the two cannot drift; a fence ' +
      'drives a malformed payload; and the repository records the rule that external input is ' +
      'unknown until validated.',
    notes:
      'MEASURED 2026-09-18 by Cline: the webhook parses untrusted input with a type assertion — ' +
      '`payload = JSON.parse(rawBody) as MetaWhatsAppWebhookPayload` at line 93 — a promise to the ' +
      'compiler rather than a check on the sender, on a PUBLIC endpoint. There is no schema ' +
      'validator anywhere in the repository (no zod, valibot or arktype). INSTALL NOTE, which cost ' +
      'the operator real time: `npm install zod` in this repository fails with `TypeError: Cannot ' +
      'read properties of null (reading matches)` from Link.matches in npm arborist, because npm ' +
      'cannot build its ideal tree over pnpm symlinks. This repository uses pnpm (pnpm-lock.yaml, ' +
      'a `pnpm` field in package.json, no package-lock.json), so the install is `pnpm add zod`. ' +
      'That also means the dependency must arrive WITH its first use: installing it unused would ' +
      'trip knip, which the engine treats as a gate. HONEST BOUNDARY: schema validation checks ' +
      'shape, not authenticity — signature verification is a separate and prior control, and a ' +
      'well-formed payload from an unverified sender is still untrusted.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/webhook-schema.test.ts`\n- `pnpm typecheck`',
  },
  {
    id: 'ENG-FORGE-FENCE-CAN-FAIL-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 101,
    title: 'A fence proves it can fail before its green counts as proof',
    goal:
      'A story cannot claim proof from a fence that has never been shown to discriminate, so every ' +
      'declared assertion carries a negative control that fails without the claimed behaviour.',
    scope:
      'a negative-control mode in the QA/assay path (workflow_app/forge/agents/qa/), the evidence record it writes (db/forge-workflow-evidence.ts), and its own fence: workflow_app/tests/fence-can-fail.test.ts (new).',
    acceptance:
      'For a story, the declared fence runs green; a negative control that withholds or inverts the ' +
      'claimed behaviour runs the SAME fence and requires at least one intended assertion to fail; ' +
      'the killing assertion is named in the evidence; the scratch state is destroyed in the same ' +
      'command; and a fence whose assertions all pass under the negative control is reported as ' +
      'UNPROVEN rather than PASS.',
    notes:
      'THE GAP, measured by its shape rather than by a failure: the whole QA model rests on a fence ' +
      'report of 6/6, and nothing anywhere checks that the six could fail. A test that asserts ' +
      'nothing, or that asserts whatever the code happens to do, passes trivially and reports ' +
      'identically to a real one. The gate already measures the PRESENCE of an assertion mapping — ' +
      'on 2026-09-18 it ruled `UNPROVEN acceptance-map-missing` on ENG-FORGE-SPLIT-SHAPE-01 for ' +
      'exactly that reason — but presence is not power, and today those two look the same in the ' +
      'evidence. The instinct already exists in this repository in one place: ' +
      'scripts/forge-sync-agents.test.ts SABOTAGES AGENTS.md text to prove the guardrail check can ' +
      'fail. This story generalises that instinct into the normal path. BUILD THIS, NOT STRIKER ' +
      'FIRST: StrykerJS mutates implementation details and reports test survival, which is a ' +
      'different and much broader question than "would this story declared proof fail if the ' +
      'claimed behaviour were absent". Stryker becomes useful later, scoped to pure policy modules ' +
      '(routing, eligibility, evidence projection), never across the whole Next.js application. ' +
      'HONEST BOUNDARY: reverting the whole candidate and accepting any red test does NOT prove ' +
      'the assertion has power — a compile error or a missing file goes red too — so the control ' +
      'must target the specific claimed behaviour and name the assertion that died.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/fence-can-fail.test.ts`\n- `pnpm test:forge:engine`',
  },
  {
    id: 'ENG-FORGE-PROPERTY-INVARIANTS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 101,
    title: 'The invariant families that keep breaking are tested with generated input',
    goal:
      'The value families that have repeatedly produced real bugs are covered by property tests that ' +
      'generate adversarial input, so a NULL, an empty string or a boundary value is found by a test ' +
      'rather than by production.',
    scope:
      'three families chosen from measured history — SQL null/three-valued semantics in the ' +
      'work-item and sprint writers, identifier and phone normalization, and story-state ' +
      'transitions — with the generator declared in package.json.',
    acceptance:
      'Each of the three families has property tests with generated input; every test shrinks a ' +
      'failure to a minimal counterexample; each test is proved to fail against the pre-fix ' +
      'behaviour of at least one real bug recorded in the story notes; and the suite runs in CI ' +
      'without a database, so it stays fast and reproducible.',
    notes:
      'WHY THIS IS NOT A BACKLOG WISH, measured twice in one night (2026-09-18): the same bug class ' +
      'bit twice — SQL three-valued logic, where a NULL in a CHECK constraint evaluates to NULL and ' +
      'a CHECK passes on NULL unless it says otherwise. First in the sprint-close rule (split_lane ' +
      'and parallel_size evaluated NULL, which passes) and then in ENG-FORGE-SPLIT-SHAPE-01, where ' +
      'only the missing split_assignment was effective because it was the one comparison that ' +
      'resolved to FALSE. Every instrument in this repository is static or search-shaped — tsc, ' +
      'eslint, knip, dependency-cruiser, semgrep, squawk, gitleaks — and example-based tests ' +
      'structurally cannot cover the input space that produced both bugs. A generator can, and ' +
      'fast-check shrinks a failing case to a minimal reproducible example, which is exactly what ' +
      'an artifact or a repair lane needs. INSTALL: `pnpm add -D fast-check` — never npm, which ' +
      'crashes in this repository (see ENG-FORGE-BOUNDARY-SCHEMAS-01). ORDER: pure functions first ' +
      '(normalization, routing, state transitions); database-backed generative testing is valuable ' +
      'later but hundreds of cases against remote Neon are slow and hard to reproduce. HONEST ' +
      'BOUNDARY: a property test proves what the property states, and a wrong property is a green ' +
      'test that documents a bug — each property needs its own sentence saying what must be true ' +
      'and why.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/property-invariants.test.ts`\n- `pnpm typecheck`',
  },
  {
    id: 'ENG-FORGE-DEPENDENCY-AUDIT-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 101,
    title: 'A vulnerable dependency is reported with its advisory and reachable or triaged',
    goal:
      'Known vulnerabilities in the dependency graph are surfaced on a schedule, each one is triaged ' +
      'as reachable or not, and the unreachable ones are recorded so the next run does not re-open ' +
      'the same question.',
    scope:
      'osv-scanner (installed, scripted as `pnpm scan:deps`, wired into the gates workflow), ' +
      'GitHub Dependabot alerts, and docs/agent/DEPENDENCY-TRIAGE.md (new).',
    acceptance:
      'A scheduled scan reports the affected packages with their advisory ids; each finding is ' +
      'triaged as reachable in production, dev-only, or not reachable, with the reason recorded; ' +
      'the scan is invoked with `--all-packages` so the full pnpm graph is read; and a new critical ' +
      'finding is visible without anyone remembering to run a command.',
    notes:
      'MEASURED 2026-09-18, the first run in this repository (osv-scanner 2.6.0): 17 packages ' +
      'affected by 45 known vulnerabilities — 2 critical, 22 high, 19 medium, 2 low — including ' +
      'sharp 0.35.3 (GHSA-rgj7-g3m4-5g8c, severity 8.9), qs 6.15.2 (two advisories at 6.3), ' +
      'next 16.3.0, postcss 8.5.6 and 8.5.19, brace-expansion 5.0.6 and nanoid 3.3.16. THE TRAP, ' +
      'and the reason the invoke is pinned in package.json: `osv-scanner scan source -L ' +
      'pnpm-lock.yaml` without `--all-packages` reported 17 packages against a lockfile holding ' +
      '899 — it resolved an unusable subset and would have been quietly believed. That is the same ' +
      'lesson as the eslint gate (a tool that runs is not a tool that covers) and it is why the ' +
      'counts above are quoted from the corrected invocation. HONEST BOUNDARY: an advisory means ' +
      'the INSTALLED version is in an affected range, not that this application is exploitable — ' +
      'the triage is the work, and a blanket upgrade is not a fix. Priority is Medium rather than ' +
      'High because nothing here is known-exploited, and because these are mostly build- and ' +
      'toolchain-level transitive dependencies; if triage shows a reachable critical path in a ' +
      'runtime dependency, raise it.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/dependency-triage.test.ts`\n- `pnpm scan:deps`',
  },
  {
    id: 'ENG-FORGE-SPLIT-SIBLING-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 101,
    title: 'A resume can no longer mark an unrun fork branch as done',
    goal:
      'The operator door refuses to resolve a hold over a split WORK branch that was never claimed, so ' +
      'a resume can never record work that did not happen and can never make a fork join unsatisfiable ' +
      'for good.',
    scope:
      'the door decision and the stop point it reads (workflow_app/forge/forge-hold-resolve.ts and ' +
      'findOpenForgeTask in workflow_app/forge/forge-engine-runtime.ts) plus workflow_app/tests/' +
      'resume-door.test.ts.',
    acceptance:
      'Resolving a hold whose open task is an unclaimed fork work branch is refused by name with ' +
      'nothing written and no task advanced; a claimed branch still moves; an unclaimed serial lane ' +
      'still moves; an unclaimed coordination task riding a branch token still moves; cancel still ' +
      'works on an unstarted branch; and the stop point reports whether it is a fork branch, whether it ' +
      'was ever claimed, and how many siblings are open.',
    notes:
      'ROOT CAUSE, CORRECTED after a solo deep dive — the original title of this story blamed the split ' +
      'materializer, and that was WRONG. Everything on the engine side is correct: the XML ' +
      'dynamic-fork (count-variable splitCount, plan-variable splitPlan, minimum 2), the process ' +
      'variables (splitCount 2, splitPlan length 2, leadDecision SPLIT), the fork kernel itself (it ' +
      'created BOTH children with branchIndex 0 and 1), the pure branch-assignment handoff, the ' +
      'enqueue shape, and the join. What killed the run was an OPERATOR action: the resume door at ' +
      '09:34 completed branch 0 of 2 of a fork whose lane had never run — task dc9c4757, claimed_at ' +
      'null, completed_by operator. From that moment the sibling was unrecoverable: the engine ' +
      'believed the branch was done so it never re-issued it, no work item ever existed for it, and ' +
      'lead_post refused the join with "split children never reached a terminal state: ' +
      'unit-a-media-length" — three times, reading as a lane failure that was not one. GARBAGE IN, ' +
      'GARBAGE OUT: the door fabricated the state and everything downstream inherited it faithfully. ' +
      'FIX (70420b46): findOpenForgeTask reports forkChild / claimedAt / openSiblings, and ' +
      'unstartedForkBranchRefusal is a pure decision that refuses an unclaimed fork WORK branch and ' +
      'points at --cancel as the honest alternative. NARROWED AFTER OVER-BLOCKING, and the live probe ' +
      'is how that was caught: forkChild is inherited by downstream tasks riding a branch token, so an ' +
      'unclaimed lead_post (exactly what the polluted instance returns) would have been refused too — ' +
      'only the fork work branch node (smith_split_work, per the XML branch-node attribute) can ' +
      'fabricate a unit completion. Five fences. REMAINING, and it belongs to the other story: the ' +
      'end-to-end two-lane observation is ENG-FORGE-TWO-UNIT-DOGFOOD-02, whose instance is still ' +
      'polluted by two operator-completed branches and needs a clean re-dispatch.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/resume-door.test.ts`',
  },
  {
    id: 'ENG-FORGE-VERIFY-EXISTING-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 101,
    title: 'A story whose work already exists can be verified without re-authoring it',
    goal:
      'A story whose candidate is already on the base can be routed to a direct Assay verification of ' +
      'that candidate, so finished work can be judged and closed without a Smith inventing edits it ' +
      'does not need.',
    scope:
      'the Lead routing decision in workflow_app/forge/forge-lead-routing.ts, the arrangement the ' +
      'runner can dispatch (workflow_app/forge/agent-runtime-role-runner.ts), and the fence ' +
      'workflow_app/tests/verify-existing.test.ts (new).',
    acceptance:
      'A story whose required findings already exist on the base can be routed to Assay verification ' +
      'of the existing candidate; the route names the candidate sha it verifies; a story with nothing ' +
      'to verify is still refused rather than passed; and a fence drives the existing-work case, the ' +
      'nothing-to-verify case, and the ordinary authoring case.',
    notes:
      'MEASURED 2026-09-18 by the engine itself, in the Lead own words on ENG-FORGE-MIGRATION-LINT-01: ' +
      '"all four required findings already exist on base commit b89519f7 (commit 74f3adc2) and this ' +
      'process forge_workflow_evidence row holds candidate_sha=null, so a Smith dispatch has no ' +
      'authoring to do and cannot produce a candidate - the runner only re-affirms a candidate the row ' +
      'already holds. The story needs direct Assay verification of 74f3adc2, an arrangement ' +
      'SOLO/SMITH/SPLIT cannot express." That is an honest refusal and a real gap in the vocabulary: ' +
      'the engine can author, split, hold and defer, but it cannot say "judge what is already there". ' +
      'The situation is not exotic either - it is created by the SANCTIONED recovery path: ' +
      'forge:story:reset aborts the instance and the fresh evidence row starts with candidate_sha ' +
      'null while the work sits finished on main, because the candidate was already published by the ' +
      'run that was reset. So the only way to finish such a story today is an operator close, which is ' +
      'what happened here. HONEST BOUNDARY: this is NOT a licence to skip QA - the route must verify ' +
      'the named sha with the story own proof and refuse when there is nothing to verify, or it ' +
      'becomes a way to close stories without evidence, which is worse than the gap.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/verify-existing.test.ts`',
  },
  {
    id: 'ENG-FORGE-LOCKFILE-COUPLING-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 101,
    title: 'A candidate that changes package.json cannot publish without pnpm-lock.yaml',
    goal:
      'A published candidate that declares a dependency change carries its lockfile, so main is never ' +
      'left with a package.json the frozen install cannot resolve.',
    scope:
      'the publish path that commits a candidate declared surface ' +
      '(workflow_app/forge/agent-runtime-role-runner.ts) and its fence ' +
      'workflow_app/tests/lockfile-coupling.test.ts (new).',
    acceptance:
      'A candidate whose diff includes package.json is refused or completed with pnpm-lock.yaml ' +
      'included, so a frozen install succeeds on the published tree; a candidate that changes only the ' +
      'lockfile is allowed; a candidate with no dependency change is untouched; and a fence drives the ' +
      'coupled, the lockfile-only and the unchanged cases.',
    notes:
      'MEASURED 2026-09-18, TWICE IN ONE EVENING, both times caught by CI and not by the engine. ' +
      'PROPERTY-INVARIANTS-01 added fast-check to package.json and installed it, and its published ' +
      'candidate did not include pnpm-lock.yaml; then DEPENDENCY-AUDIT-01 committed package.json ' +
      'without it again. Both left main red at `pnpm install --frozen-lockfile` with ' +
      'ERR_PNPM_OUTDATED_LOCKFILE, in 15-16 seconds, on work the lanes had every right to do. THE ' +
      'MECHANISM: the publish path commits the story DECLARED SURFACE, so a file outside that surface ' +
      'is left behind - and the lockfile is not outside a dependency change, it is part of it. Authoring ' +
      'discipline is not a sufficient fix either, which is why this is an engine-side guard: a lane can ' +
      'add a dependency DURING a run (that is what happened - the story anticipated the generator, the ' +
      'lane chose the package), and no scope written in advance can be expected to name every artifact ' +
      'that choice implies. story:preflight gained a third hard key for the authoring side (a story ' +
      'declaring a dependency change must name pnpm-lock.yaml), and this story is the publish-side ' +
      'guard, because the two together are what make the class impossible rather than merely ' +
      'discouraged. HONEST BOUNDARY: the guard must not force unrelated lockfile churn into every ' +
      'candidate - it fires only when the candidate own diff touches package.json.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/lockfile-coupling.test.ts`',
  },
  {
    id: 'ENG-FORGE-LINT-GATE-01',

    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 99,
    title: 'The static gate runs the linter: a lane cannot ship a lint error through a clean QA pass',
    goal:
      'A lint error in a lane change blocks QA with the rule, the file and the line named, so a story cannot be ' +
      'ruled clean while the repository lint is red.',
    scope:
      'the QA/architecture static gate (workflow_app/forge/forge-static-gate.ts and its callers) plus ' +
      'workflow_app/tests/lint-gate.test.ts (new).',
    acceptance:
      'A change carrying a lint ERROR fails the static gate and names the file, line and rule; a change carrying ' +
      'only lint WARNINGS passes; the gate covers the files the story changed rather than the whole repository, so ' +
      'it stays cheap enough to run per story; a test drives an erroring file, a warning-only file and a clean ' +
      'file. Existing warnings are not retroactively fatal.',
    notes:
      'MEASURED 2026-09-18 by Cline, twice in one night: ENG-FORGE-DEPLOY-NOMECH-01 and ENG-FORGE-ARTIFACT-RULING-01 ' +
      'each landed a no-useless-assignment error (and one unused import) through a FULL QA PASS, discovered only ' +
      'because the operator ran the linter by hand while verifying something else. The engine static gate runs ' +
      'semgrep, knip and tsc - not eslint - so lint is a check nobody enforces inside the factory, which makes it ' +
      'exactly the kind of gate this suite has been adding all night: the failure mode is real, the instrument ' +
      'exists, and it simply is not wired into the path that decides. HONEST BOUNDARY: this does not reformat code ' +
      'or fix existing warnings; it makes a NEW error stop the line, and the three instances found tonight are ' +
      'fixed by the commit that filed this.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/lint-gate.test.ts`',
  },
  {
    id: 'ENG-FORGE-TYPESAFE-TRIAGE-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Low',
    batch: 99,
    title: 'An outside judgment is advisory: a failure-triage pilot that can point but cannot decide',
    goal:
      'A recorded Forge failure can be sent to an outside classifier for an advisory cause, stored beside the ' +
      'existing verdict without touching it, so a confident-looking label can be compared with confirmed causes ' +
      'before anything acts on it.',
    scope:
      'the triage judgment (workflow_app/forge/typesafe-failure-triage.ts), its storage through the existing ' +
      'recordToolArtifact (db/forge-typesafe-triage.ts), the operator CLI (scripts/forge-triage.ts), the fence ' +
      '(workflow_app/tests/typesafe-failure-triage.test.ts) and docs/agent/typesafe-failure-triage.md.',
    acceptance:
      'The source verdict, workflow evidence, stage failure and engine routing are unchanged by an analysis; the ' +
      'observation carries the model, prompt version, redactor version, input hash, probabilities, confidence, ' +
      'evidence sufficiency, latency and token usage; only allowlisted fields are sent, after best-effort ' +
      'redaction; a malformed or model-mismatched response is rejected without saving; the report computes ' +
      'agreement only over reviewed cases and returns null rather than a number when there are none. Measured ' +
      '2026-09-18: 12/12 focused tests with no network or database, two live calls at 591-740 ms, and the report ' +
      'showed agreement null with the denominator visible.',
    notes:
      'LANDED 2026-09-18 by Cline, from a patch the captain had GPT prepare (alpha) and asked to be pulled into ' +
      'the right spot. Placement follows the house shape and reuses FORGE_FAILURE_CLASSES and recordToolArtifact ' +
      'rather than inventing a parallel vocabulary or table. THE INTENT, written into the doc so it cannot be ' +
      'quietly reinterpreted: a SENSOR, NEVER AN ORACLE - the useful output is the shape of its confusion (thin ' +
      'evidence, a class distribution that moved, or a confident class no stored evidence supports), and each ' +
      'cluster becomes a story where the truth is established with a fence and a QA ruling. GRADUATION BAR, ' +
      'written before the data: advisory -> mainline requires measured agreement on EVIDENCED labels at least ' +
      'equal to the existing classifier, at least 50 reviewed cases, and calibrated sufficiency. Until then it is ' +
      'operator-invoked and the API key is the off-switch. HONEST BOUNDARY: this claims NO accuracy - the report ' +
      'says observational sample, not a benchmark - and it is never cited as evidence, only as a pointer.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/typesafe-failure-triage.test.ts`',
  },
  {
    id: 'ENG-FORGE-ATLAS-SOURCES-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 99,
    title: 'The failure atlas can see the failures that cost hours, not only the tidy ones',
    goal:
      'Hold records and engine task failures join the triageable sources, so the atlas covers the failures that ' +
      'cost the most time rather than only those that recorded an artifact for themselves.',
    scope:
      'the source selection and normalization (db/forge-typesafe-triage.ts, workflow_app/forge/' +
      'typesafe-failure-triage.ts, the source list in scripts/forge-triage.ts) plus workflow_app/tests/' +
      'atlas-sources.test.ts (new).',
    acceptance:
      'A hold record and an engine task row carrying an error are both listable and triageable, each mapped to the ' +
      'same bounded, allowlisted, redacted evidence shape as an artifact (no free-form blobs, no filesystem logs); ' +
      'their story/run/sha provenance is preserved where it exists and recorded as absent where it does not; the ' +
      'report counts them in the same denominator. A test drives a hold source, a task-error source and a source ' +
      'with no run id.',
    notes:
      'MEASURED 2026-09-18 by Cline, from the first real use. Eligible sources are qa-assay-evidence, ' +
      'architecture-security and run-verdict: 28 not-passing rows on this machine. In the same window the engine ' +
      'produced 13 hold records and 44 engine task rows with an error recorded - and NONE are visible to the ' +
      'atlas. The sharpest illustration: ENG-FORGE-LANE-FAILURE-01 died on a constraint violation and its QA ' +
      'artifact says PASS, because the failure happened after the verdict. So the pilot currently sees the ' +
      'failures that were tidy enough to record themselves and misses the messy ones - exactly backwards from ' +
      'where the captain wants help: the weird stuff that is hard to diagnose. HONEST BOUNDARY: this widens what ' +
      'is READ; it does not change the model, the prompt, the thresholds or any routing, and a source with no run ' +
      'id is recorded as having none rather than guessed.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/atlas-sources.test.ts`',
  },
  {
    id: 'ENG-FORGE-ATLAS-LABELS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 99,
    title: 'A triage label is evidenced by the commit that fixed it, not by how plausible the model sounded',
    goal:
      'The first labels in the triage sample come from failures whose cause was later PROVEN by the commit that ' +
      'fixed them, so the accuracy number measures the model rather than agreement with the reviewer.',
    scope:
      'a labelling helper or documented procedure that ties a confirmed class to the fixing commit ' +
      '(scripts/forge-triage.ts review path or a small companion script) and the resulting sample, plus ' +
      'workflow_app/tests/atlas-label-evidence.test.ts (new).',
    acceptance:
      'A review records the evidence for its confirmed class with the commit or file that proves it, and the ' +
      'review is refused when that evidence is missing or empty; the report distinguishes evidenced labels from ' +
      'bare ones and can compute agreement over evidenced labels alone. A test drives an evidenced label, a bare ' +
      'label and a review with no evidence. At least six historical failures whose cause is proven by a merge ' +
      'commit are labelled, and the report over them is published as the pilot baseline.',
    notes:
      'MEASURED 2026-09-18 by Cline. The pilot reports agreement null with zero reviewed cases, which is the right ' +
      'discipline but leaves it a demo rather than a measurement. The binding constraint is LABELS, not the model: ' +
      'a review that confirms whatever the model said is self-confirming, and one that confirms what happened ' +
      'requires knowing what happened. Tonight produced at least six failures whose cause is already proven by a ' +
      'fixing commit (2a014527, f29b2b4c, 150ec8f7, e6f21de1, b5925057, 81eb5a25), so a first baseline is ' +
      'available without guessing. HONEST BOUNDARY: this does not change the model, prompt or thresholds, and six ' +
      'cases cannot establish production accuracy - the report denominator stays visible so a tiny sample reads ' +
      'as a tiny sample.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/atlas-label-evidence.test.ts`',
  },
  {
    id: 'ENG-FORGE-ATLAS-SPEND-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Low',
    batch: 99,
    title: 'A judgment call is a vendor cost: the atlas records what it spent',
    goal:
      'Each triage call records its token usage as a vendor cost in the same vocabulary the runs use, so judgment ' +
      'spend appears in the sprint accounting instead of being invisible.',
    scope:
      'the triage observation writer (db/forge-typesafe-triage.ts) and the spend vocabulary already defined for ' +
      'runs (db/storyboard.ts SPEND_SOURCES, migration 190), plus workflow_app/tests/atlas-spend.test.ts (new).',
    acceptance:
      'A triage call with recorded token usage produces a spend fact carrying source vendor with its token counts, ' +
      'and a call with no usage recorded is recorded as none rather than left null; the sprint accounting can ' +
      'include judgment spend, and widgets are never written into the dollars column. A test drives a call with ' +
      'usage, a call without, and the none case.',
    notes:
      'FILED 2026-09-18 by Cline from the same first use: each call is ~780 input tokens at $0.042/M, so ' +
      '~$0.00003 - negligible individually and exactly the kind of spend that hides as a floor when it is ' +
      'aggregated and unrecorded. The vocabulary already exists (migration 190: vendor | widgets | none) and the ' +
      'observation already stores token counts, so this is a wiring job, not a new concept. HONEST BOUNDARY: the ' +
      'provider does not return a cost field today, so the cost is derived from published pricing and must be ' +
      'labelled as derived rather than measured.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/atlas-spend.test.ts`',
  },
  {
    id: 'ENG-FORGE-SPRINT-BOARD-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 100,
    title: 'The board shows its sprint: goal, status and outcome beside the stories',
    goal:
      'The storyboard renders the sprint parent — goal, status, percent complete, and the outcome it was ' +
      'closed with — so a sprint can be read and judged instead of reconstructed from story counts.',
    scope:
      'the storyboard read path (db/storyboard.ts, lib/storyboard-data.ts) and the portal page ' +
      '(app/portal/storyboard), workflow_app/tests/sprint-board.test.ts (new).',
    acceptance:
      'The storyboard renders each sprint from storyboard_sprint_rollup rather than recomputing counts ' +
      'in the page: its goal, status, percent complete, open and held counts, and its outcome once ' +
      'closed. A sprint with no goal says so instead of rendering blank, and a sprint with no stories ' +
      'shows no percentage rather than 0%. A closed sprint shows the outcome it was closed with. The ' +
      'batch axis keeps working unchanged for every existing reader. A test drives the projection.',
    notes:
      'FILED 2026-09-17 (Cline, at the captain direction that sprints are how this work will be run). ' +
      'Migration 187 gives the board its parent: storyboard_sprint (goal, theme, status, owner, started, ' +
      'target end, closed at, outcome) with storyboard_story.sprint_id derived from batch by trigger, ' +
      'plus two views — storyboard_sprint_rollup and storyboard_sprint_story — and an operator CLI ' +
      '(pnpm sprint list|show|open|goal|close). What does NOT yet exist is a surface: /portal/storyboard ' +
      'still renders the batch integer, so a sprint goal or a closure outcome is invisible where the ' +
      'work is read. This story is that surface. HONEST BOUNDARY: the tables and the CLI are complete ' +
      'and verified; this is the rendering, and it deliberately does not change how batch is written or ' +
      'read anywhere else.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/sprint-board.test.ts`',
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
  {
    id: 'ENG-FORGE-ACCEPTANCE-SUPPLIER-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'The acceptance mapping has a producer, so a covered story can PASS',
    goal:
      'A clause-to-assertion mapping is DECLARED before the work — by the Architect or Lead through the ' +
      'handoff, or by the story author on the story row — so QA returns PASS for a story whose clauses are ' +
      'all covered instead of UNPROVEN for every story in the factory.',
    scope:
      'scripts/forge-handoff.mjs (a contract flag carrying the mapping), the architect/lead directive that ' +
      'asks for it, workflow_app/forge/forge-architect-contract.ts and the ready-gate facts supplier (one ' +
      'reader that prefers the handoff and falls back to a story-declared mapping, naming which it used), a ' +
      'migration if the story row needs the column, workflow_app/tests/acceptance-supplier.test.ts (new).',
    acceptance:
      'A story whose every clause maps to an assertion that ran and passed returns PASS — not UNPROVEN. The ' +
      'mapping is declarable in BOTH places named above, one reader resolves them in a stated order, and ' +
      'the record says which source was used. A story with no mapping at all still returns UNPROVEN with ' +
      'the blocker acceptance-map-missing, unchanged. A partially mapped story returns UNPROVEN naming the ' +
      'uncovered clause. A test drives all four cases: fully mapped PASS, partial UNPROVEN by name, absent ' +
      'UNPROVEN, and handoff-declared beating story-declared.',
    notes:
      'FOUND 2026-09-17 at 09:38, by the instrument it concerns — the first production run after ' +
      'ENG-FORGE-ACCEPTANCE-PROOF-01 landed: `forge.qa-verdict: qa_verify: QA UNPROVEN: blockers=' +
      '[UNPROVEN acceptance-map-missing] failed=[none]` on ENG-FORGE-MIGRATION-REPLAY-01, whose work had ' +
      'shipped and whose fence passed 8/8. Verified: the mapping has THREE readers ' +
      '(forge-ready-gate.ts:36, lead-routing-context.ts:159, forge-architect-contract.ts:12/59) and NO ' +
      'writer — no CLI flag carries it and no directive text asks the Architect or the Lead for it (a grep ' +
      'for acceptance plus assert|map across the architect agent, its directive and the contract returns ' +
      'nothing). So every story filed on 2026-09-17 reports UNPROVEN until this lands. This is the SAME ' +
      'failure mode the night was spent fixing in other code — a mechanism with no supplier — committed by ' +
      'the story meant to close that class, and it is recorded plainly because that is the point of the ' +
      'doctrine. HONEST BOUNDARY: this supplies the fact; it does not change what UNPROVEN means, does not ' +
      'weaken the gate that refuses a DECLARED mapping leaving a clause unmapped, and does not retro-act ' +
      'on stories already ruled UNPROVEN — those become replayable once this lands.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/acceptance-supplier.test.ts`',
  },
  {
    id: 'ENG-FORGE-READ-TOOLS-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'Medium',
    batch: 92,
    title: 'The reads an operator keeps hand-writing are sanctioned tools',
    goal:
      'The five or six read shapes used every day — board counts, a story full picture, run receipts, hold ' +
      'reasons, ledger state — are served by a sanctioned read path instead of ad-hoc SQL written per ' +
      'investigation, so a reader cannot invent a second interpretation of a fact.',
    scope:
      'read-only SQL VIEWS for the recurring shapes (board by batch, story run receipt, open holds, story ' +
      'findings/contract, migration ledger state) plus one CLI over them (for example forge:story:show <id> ' +
      'and forge:board), and workflow_app/tests/read-tools.test.ts (new).',
    acceptance:
      'Each named shape is served by a VIEW (read-only by construction, so it can never become a second ' +
      'writer) and by one CLI command that prints it, with the row ids and timestamps in the driver form ' +
      'the readers already normalise. The CLI resolves a story by id and prints board state, the latest run ' +
      'receipt with its verdict and commit, open holds with their reasons, and the migration state for its ' +
      'change set. A test asserts the views exist with the expected columns and that the CLI output for a ' +
      'fixture story matches the rows the individual readers return.',
    notes:
      'FROM THE CAPTAIN 2026-09-17: "i saw you doing some inline sql in your work yesterday and thought we ' +
      'should make you some helper stored procedures for things around loading stories etc." Measured: I ' +
      'wrote roughly twenty ad-hoc query scripts across 2026-09-16/17 (board counts by batch, per-story run ' +
      'receipts, hold reasons, findings rows, spend by story, migration ledger, residue counts), and the ' +
      'same joins were rewritten several times — which is exactly how two readers drift. SHAPING, stated as ' +
      'a departure from the suggestion: VIEWS plus one CLI rather than stored procedures, because a ' +
      'procedure is a place logic can live a second time and is invisible to the test tier, while a view ' +
      'is read-only by construction and the CLI can be asserted against the existing readers. A procedure ' +
      'is still the right tool if a write ever needs several statements in one transaction — that is a ' +
      'different story. HONEST BOUNDARY: read-only; no new writes, no caching, and it does not replace the ' +
      'sanctioned writers that already exist (story:status, forge:story:run, db:migrate).',
    assayCommands: '- `node --import tsx --test workflow_app/tests/read-tools.test.ts`',
  },
  {
    id: 'ENG-FORGE-MIGRATION-START-01',
    workstream: 'ENGINEERING',
    operatingSurface: 'TECH',
    priority: 'High',
    batch: 92,
    title: 'No run starts while a migration in the repo is unapplied on PROD',
    goal:
      'A run refuses to START — before any model turn — when a migration file in the repo is absent from ' +
      'the PROD ledger, naming the unapplied files, so a story parked before DEV_OPS cannot ship code ahead ' +
      'of the schema it writes.',
    scope:
      'the engine start path (scripts/forge-engine-worker.ts before the first wave, reusing ' +
      'migration-applied-guard), the ledger reader already used by the completion guard, ' +
      'workflow_app/tests/migration-preflight.test.ts (new).',
    acceptance:
      'Given one or more db/migrations/*.sql files present in the repo and absent from the PROD ' +
      'schema_migration ledger, the run refuses to start, names EVERY unapplied file, and dispatches no ' +
      'lane — nothing is spent on a turn. Given all repo migrations ledgered, the run proceeds unaffected. ' +
      'The check reads the LEDGER TABLE and the repo file LIST only, never a story diff (the work has not ' +
      'happened yet), and an unreadable ledger fails CLOSED to the named refusal rather than to a silent ' +
      'start. A test drives both directions with a fixture repo list and a fixture ledger, including the ' +
      'three-file case measured tonight.',
    notes:
      'FOUND 2026-09-17 by the operator, three times in one night, from his own shortcut: parking every ' +
      'story at qa_verify with --until skips DEV_OPS, and DEV_OPS owns migrations. Migration 184 shipped ' +
      'code writing l_whatsapp.context_id before the column existed; 185 shipped the supersede writer before ' +
      'its table existed; 186 shipped the acceptance-mapping writer before the column existed — and the ' +
      'third one failed LOUDLY at the next run start (DbFailureError SCHEMA_MISMATCH on storyboard_story), ' +
      'which is how it was found. ENG-FORGE-MIGRATION-APPLIED-01 (complete) guards the COMPLETION path; a ' +
      'story parked at QA never reaches completion, so that guard cannot see these. This story puts the same ' +
      'fact in front of the run instead of behind it: a preflight at the start seam, which is where an ' +
      'operator shortcut is actually taken. HONEST BOUNDARY: it does not apply migrations, does not change ' +
      'DEV_OPS ownership, and does not backfill the ledger for migrations already applied by hand.',
    assayCommands: '- `node --import tsx --test workflow_app/tests/migration-preflight.test.ts`',
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
