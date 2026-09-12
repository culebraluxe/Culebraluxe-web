// ---------------------------------------------------------------------------
// The ONE seam. Static for iteration 1 — layout before wiring, on purpose.
//
// Every number below is REAL and read from PROD on 2026-09-12, not invented:
//   tiles — /portal/tech Engineering Cockpit, "Story data as of 2026-09-12 18:38 UTC"
//   stats — 454 runs over 122 stories, Aug 21 -> Sep 12: 80.2% complete, 7.3% hold,
//           6.4% interrupted, 2.9% failed, 73.1% of runs are a story's 2nd+ attempt,
//           worst single story 76 runs.
// The CARDS are a representative sample drawn from the real board: iteration 1 is
// about the MECHANIC (which queue, who owns it, what moves), not the data.
//
// When this is wired, loadEngineeringQueues() reads:
//   bench / ready — the new membership table (one queue per story, unique on story_id)
//   running       — agent_work_item (Ready / Claimed / Running)
//   results       — storyboard_story_run (result_status IS the outcome; the attempt
//                   number comes from the run order per story)
// ---------------------------------------------------------------------------

import type { EngineeringQueuesModel } from './types'

export const ENGINEERING_QUEUES_FIXTURE: EngineeringQueuesModel = {
  eyebrow: 'TECH / ENGINEERING',
  title: 'Engineering Queues',
  subtitle:
    'One screen: what you are working on, what the engine has been given, what it is running, and how the last runs ended.',
  asOf: '2026-09-12 18:38 UTC',

  tiles: [
    { label: 'TOTAL STORIES', value: '313', caption: 'All canonical rows' },
    { label: 'ACTIVE QUEUE', value: '12', caption: 'Selected today' },
    { label: 'OPEN', value: '18', caption: 'Current work queue' },
    { label: 'BACKLOG', value: '34', caption: 'Current-version planned' },
    { label: 'CLOSED', value: '228', caption: 'Finished history' },
    { label: 'COMPLETION', value: '94.4%', caption: 'Net-net' },
  ],

  stats: {
    asOf: '2026-09-12',
    runs: 454,
    stories: 122,
    pctComplete: 80.2,
    pctHold: 7.3,
    pctRerun: 73.1,
    pctFail: 2.9,
    worstOffender: { id: 'ENG-FORGE-OPENCODE-DOGFOOD-01', runs: 76 },
  },

  cards: [
    {
      id: 'CRM-28',
      title: 'P&S Amendment -> Canonical Term Delta + Timer Reschedule',
      workstream: 'CRM / INTAKE',
      status: 'Planned',
      priority: 'MEDIUM-HIGH',
      completion: 0,
      queue: 'bench',
    },
    {
      id: 'PROJECTS-WORKSPACE-14',
      title: 'DEV/PROD rollout and real-project acceptance',
      workstream: 'FRAMEWORKS',
      status: 'Planned',
      priority: 'CRITICAL',
      completion: 0,
      queue: 'bench',
    },
    {
      id: 'FORGE-PARITY-DEFAULTS-01',
      title: 'Parity axis six: column defaults',
      workstream: 'FRAMEWORKS',
      status: 'Planned',
      priority: 'MEDIUM-HIGH',
      completion: 0,
      queue: 'ready',
    },
    {
      id: 'FORGE-OBS-HYDRATE-COMPLETE-01',
      title: 'Persist the identity fields rehydration cannot recover',
      workstream: 'FRAMEWORKS',
      status: 'Planned',
      priority: 'MEDIUM',
      completion: 0,
      queue: 'ready',
    },
    {
      id: 'FORGE-DOGFOOD-RUN-01',
      title: 'One serial story, end to end, on the honest machine',
      workstream: 'FRAMEWORKS',
      status: 'Planned',
      priority: 'HIGH',
      completion: 0,
      queue: 'ready',
    },
    {
      id: 'ENG-FORGE-V5-25',
      title: 'dependency-cruiser Architecture Invariant Gate',
      workstream: 'FRAMEWORKS',
      status: 'In Progress',
      priority: 'HIGH',
      completion: 0,
      queue: 'running',
    },
    {
      id: 'ENG-PROJECTS-ANCHOR-02',
      title: 'Projects projection: report the effective anchor source',
      workstream: 'FRAMEWORKS',
      status: 'In Progress',
      priority: 'MEDIUM',
      completion: 100,
      queue: 'running',
    },
    {
      id: 'FORGE-SMITH-DOOR-01',
      title: 'Both serial Smith doors stay shut together',
      workstream: 'FRAMEWORKS',
      status: 'Complete',
      priority: 'HIGH',
      completion: 100,
      queue: 'results',
      outcome: 'DONE',
      attempt: 1,
      endedOn: 'qa_verify',
      instanceId: 'FORGE-SMITH-DOOR-01-demo',
    },
    {
      id: 'FORGE-PARITY-CHECK-01',
      title: 'Parity compares check constraints',
      workstream: 'FRAMEWORKS',
      status: 'Complete',
      priority: 'MEDIUM-HIGH',
      completion: 100,
      queue: 'results',
      outcome: 'DONE',
      attempt: 2,
      endedOn: 'deploy_gate',
      instanceId: 'FORGE-PARITY-CHECK-01-demo',
    },
    {
      id: 'FORGE-OBS-LIST-01',
      title: 'Alert rules can see prior attempts',
      workstream: 'FRAMEWORKS',
      status: 'Complete',
      priority: 'MEDIUM-HIGH',
      completion: 100,
      queue: 'results',
      outcome: 'DONE',
      attempt: 1,
      endedOn: 'qa_verify',
      instanceId: 'FORGE-OBS-LIST-01-demo',
    },
    {
      id: 'ENG-FORGE-OPENCODE-DOGFOOD-01',
      title: 'OpenCode Smith through Forge',
      workstream: 'FRAMEWORKS',
      status: 'Hold',
      priority: 'HIGH',
      completion: 100,
      queue: 'results',
      outcome: 'HOLD',
      attempt: 76,
      endedOn: 'smith',
      instanceId: 'ENG-FORGE-OPENCODE-DOGFOOD-01-demo',
    },
    {
      id: 'ENG-FORGE-V5-11',
      title: 'Lead / Dev / QA Serial Topology',
      workstream: 'FRAMEWORKS',
      status: 'Hold',
      priority: 'HIGH',
      completion: 100,
      queue: 'results',
      outcome: 'HOLD',
      attempt: 9,
      endedOn: 'lead_pre',
      instanceId: 'ENG-FORGE-V5-11-demo',
    },
    {
      id: 'ENG-FORGE-SPLIT-DOGFOOD-01',
      title: 'Forge SPLIT lane: sibling-scope contract + duplicate join accounting',
      workstream: 'FRAMEWORKS',
      status: 'In Progress',
      priority: 'MEDIUM',
      completion: 100,
      queue: 'results',
      outcome: 'INTERRUPTED',
      attempt: 4,
      endedOn: 'smith_split_work',
      instanceId: 'ENG-FORGE-SPLIT-DOGFOOD-01-demo',
    },
    {
      id: 'ENG-DB-RESILIENCE-01',
      title: 'DB resilience — boundary timeout, retry, circuit-breaker review',
      workstream: 'OTHER',
      status: 'Hold',
      priority: 'P2',
      completion: 0,
      queue: 'results',
      outcome: 'ERROR',
      attempt: 3,
      endedOn: 'smith',
      instanceId: 'ENG-DB-RESILIENCE-01-demo',
    },
  ],

  lifecycle: [
    {
      key: 'open',
      label: 'CURRENT WORK QUEUE',
      count: 18,
      caption: 'Open',
      sample: [
        { id: 'CRM-25', title: 'Outbound External Action / Correlation Loop', status: 'HOLD' },
        { id: 'ENG-FORGE-FASTLANE-01', title: 'Make FORGE_SDLC FAST lane actually fast', status: 'HOLD' },
      ],
    },
    {
      key: 'backlog',
      label: 'CURRENT-VERSION WAITING',
      count: 34,
      caption: 'Backlog',
      sample: [
        {
          id: 'ENG-FORGE-SYNC-01',
          title: 'Ship-time board sync: shipped work always shows its numbers in PROD',
          status: 'PLANNED',
        },
        { id: 'TECH-DEBT-01', title: '[TECH DEBT] Wire the PROD guard into the Forge run lane', status: 'PLANNED' },
      ],
    },
    {
      key: 'closed',
      label: 'FINISHED HISTORY',
      count: 228,
      caption: 'Closed',
      sample: [
        { id: 'DB-HARDEN-02', title: 'Centralized Database Connection Pooling', status: 'COMPLETE' },
        { id: 'ENV-EXPLICIT-01', title: 'Environments are declared, never inferred', status: 'COMPLETE' },
      ],
    },
    {
      key: 'next',
      label: 'INTENTIONALLY FUTURE',
      count: 33,
      caption: 'Next Version',
      sample: [
        { id: 'CRM-07', title: 'WhatsApp Intake', status: 'DEFERRED' },
        { id: 'CRM-29', title: 'V2 — Buyer-to-Property Matching + Curated Collections', status: 'DEFERRED' },
      ],
    },
  ],
}

/** The single seam. Replace this body with real reads; the screen does not change. */
export function loadEngineeringQueues(): EngineeringQueuesModel {
  return ENGINEERING_QUEUES_FIXTURE
}
