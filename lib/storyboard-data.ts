// ---------------------------------------------------------------------------
// CulebraLuxe Story Board — read-only seed and dashboard model.
//
// The stories below are the EXISTING human-authored backlog, transcribed from
// docs/workflow/MASTER_STORYBOARD.md and docs/workflow/STORYBOARD_STATUS.md
// (seed date 2026-08-20, main @ fddcd26). Nothing here is derived from the
// repository and no backlog items are created. The board is read-only: no CRUD,
// no database access, no workflow_engine involvement.
//
// Statuses and priorities use the program vocabulary exactly as required.
// ---------------------------------------------------------------------------

export const WORKSTREAMS = [
  "CRM / Intake",
  "Portal / Operations",
  "Public Property / Buyer Experience",
  "Platform / Engineering / Data",
] as const

export type Workstream = (typeof WORKSTREAMS)[number]

export const STORY_STATUSES = [
  "Complete",
  "Read-side complete",
  "Partial",
  "Planned",
  "Open",
  "Blocked",
  "Deferred",
  "Hardware/content dependent",
  "Operationalized",
  "Minor remainder",
  "Readiness PASS",
] as const

export type StoryStatus = (typeof STORY_STATUSES)[number]

export const STORY_PRIORITIES = [
  "Critical",
  "High",
  "High-ish",
  "Medium-High",
  "Medium",
  "Low",
  "Later",
  "High-value polish",
] as const

export type StoryPriority = (typeof STORY_PRIORITIES)[number]

export type StoryRecord = {
  id: string
  workstream: Workstream
  title: string
  priority: StoryPriority
  status: StoryStatus
  notes: string
}

// ---------------------------------------------------------------------------
// Seed — the 41 existing human-authored stories.
// ---------------------------------------------------------------------------

export const STORIES: StoryRecord[] = [
  // ----- CRM / Intake -------------------------------------------------------
  {
    id: "S-001",
    workstream: "CRM / Intake",
    title: "CRM-01 Source-Idempotent Interaction Inputs",
    priority: "High",
    status: "Complete",
    notes:
      "Canonical (source_system, source_external_id) idempotency with database uniqueness backstop (migration 005).",
  },
  {
    id: "S-002",
    workstream: "CRM / Intake",
    title: "CRM-02 Neutral Inbound Events, Identity Normalization & Context Resolution",
    priority: "High",
    status: "Complete",
    notes:
      "Strict E.164 / canonical identity; exact trusted context only; advisory intents, no action inference.",
  },
  {
    id: "S-003",
    workstream: "CRM / Intake",
    title: "CRM-03 Explicitly Authorized Person Creation",
    priority: "High",
    status: "Complete",
    notes:
      "Atomic creation, existing-person-wins race recovery, source-token validation.",
  },
  {
    id: "S-004",
    workstream: "CRM / Intake",
    title: "CRM-04 Website / Self-Service Intake",
    priority: "High",
    status: "Complete",
    notes:
      "Canonical website intake pipeline (migration 006 recorded); Neon execution is operational.",
  },
  {
    id: "S-005",
    workstream: "CRM / Intake",
    title: "CRM-05 Provider-Neutral Email Intake",
    priority: "Medium",
    status: "Complete",
    notes:
      "Fixture POC reviewed PASS; live provider ingestion deferred to separate stories.",
  },
  {
    id: "S-006",
    workstream: "CRM / Intake",
    title: "CRM-06 Phone / SMS / iMessage Communications Intake",
    priority: "Medium",
    status: "Complete",
    notes:
      "Fixture POC reviewed PASS; live provider ingestion deferred to separate stories.",
  },
  {
    id: "S-007",
    workstream: "CRM / Intake",
    title: "CRM-07 WhatsApp Intake Architecture (Architecture Only)",
    priority: "Medium",
    status: "Readiness PASS",
    notes: "Architecture reviewed PASS; implementation blocked on S-008.",
  },
  {
    id: "S-008",
    workstream: "CRM / Intake",
    title: "Canonical WhatsApp Interaction Channel Decision",
    priority: "High",
    status: "Planned",
    notes:
      "Decision pending; narrow whatsapp channel addition recommended; unblocks S-009.",
  },
  {
    id: "S-009",
    workstream: "CRM / Intake",
    title: "WhatsApp Provider Connector Implementation",
    priority: "Medium",
    status: "Blocked",
    notes:
      "Blocked on S-008; only provider-neutral scaffolding exists (lib/crm-whatsapp-*).",
  },
  {
    id: "S-010",
    workstream: "CRM / Intake",
    title: "V1 DB Unblock M-1: WhatsApp Interaction Channel (migration 010)",
    priority: "Medium",
    status: "Complete",
    notes:
      "Channel recorded in schema; actors resolve via canonical phone; use gated by S-008.",
  },
  {
    id: "S-011",
    workstream: "CRM / Intake",
    title: "V1 DB Unblock M-2: General Enquiry Website Intake (migration 011)",
    priority: "Medium",
    status: "Complete",
    notes:
      "Generic /contact flows through the canonical pipeline; property-scoped intake unchanged.",
  },

  // ----- Portal / Operations -------------------------------------------------
  {
    id: "S-012",
    workstream: "Portal / Operations",
    title: "V1 DB Unblock M-3: Deal Participants (migration 012)",
    priority: "Medium",
    status: "Read-side complete",
    notes:
      "Migration and read models in place; legacy FKs remain source of truth; FK migration is later.",
  },
  {
    id: "S-013",
    workstream: "Portal / Operations",
    title: "V1 DB Unblock M-4: Showing Lifecycle (migration 013)",
    priority: "Medium",
    status: "Read-side complete",
    notes:
      "Migration and read models in place; showing→interaction write behavior is a later story.",
  },
  {
    id: "S-014",
    workstream: "Portal / Operations",
    title: "V1 DB Unblock M-5: Offer Model (migration 014)",
    priority: "Medium",
    status: "Read-side complete",
    notes:
      "Migration and read models in place; deal.offer_price untouched; no backfill.",
  },
  {
    id: "S-015",
    workstream: "Portal / Operations",
    title: "AUTH-01 Auth & Security Model Foundation",
    priority: "High",
    status: "Complete",
    notes:
      "Schema foundation plus docs; runtime enforcement not yet active (see S-016).",
  },
  {
    id: "S-016",
    workstream: "Portal / Operations",
    title: "AUTH-02 Auth Runtime Enforcement Activation",
    priority: "Critical",
    status: "Blocked",
    notes:
      "Blocked on human bootstrap order: provider login, subject link, break-glass proof.",
  },
  {
    id: "S-031",
    workstream: "Portal / Operations",
    title: "Portal Workflows Experience",
    priority: "Medium-High",
    status: "Open",
    notes:
      "Read-only summaries exist; task/deadline/action UX not built (held by no-UI constraint).",
  },
  {
    id: "S-032",
    workstream: "Portal / Operations",
    title: "CRM-14 Closing Orchestration",
    priority: "Critical",
    status: "Deferred",
    notes:
      "Deferred from V1 unblock; closing readiness, date reschedule, post-close recording.",
  },

  // ----- Public Property / Buyer Experience ----------------------------------
  // No stories are currently tracked under this workstream. The durable
  // storyboard does not contain any public-property / buyer stories, and no
  // new backlog items may be created here.

  // ----- Platform / Engineering / Data ---------------------------------------
  {
    id: "S-017",
    workstream: "Platform / Engineering / Data",
    title: "WF-01 Workflow Engine Preservation & Architecture Assessment",
    priority: "High",
    status: "Complete",
    notes:
      "Read-only archaeology; engine preserved generic and domain-neutral.",
  },
  {
    id: "S-018",
    workstream: "Platform / Engineering / Data",
    title: "WF-02 Ogden Integration Seam (Application-Side Contracts)",
    priority: "High",
    status: "Complete",
    notes:
      "lib/workflow contracts; the application never imports the engine.",
  },
  {
    id: "S-019",
    workstream: "Platform / Engineering / Data",
    title: "WF-03 CRM-14 Transaction Workflow Foundation",
    priority: "High",
    status: "Complete",
    notes:
      "Claim-first receipts (migration 018); compare-and-set deal stage transitions.",
  },
  {
    id: "S-020",
    workstream: "Platform / Engineering / Data",
    title: "WF-04 XML-Driven RE Supermodel (CRM-14E)",
    priority: "High",
    status: "Complete",
    notes:
      "XML is the authoritative definition source; RE_supermodel-v1.xml; no Neon deploy yet.",
  },
  {
    id: "S-021",
    workstream: "Platform / Engineering / Data",
    title: "WF-05 Neon Workflow Transaction Adapter",
    priority: "High",
    status: "Complete",
    notes: "One transaction per engine operation.",
  },
  {
    id: "S-022",
    workstream: "Platform / Engineering / Data",
    title: "WF-06 Workflow Task Completion Seam",
    priority: "High",
    status: "Complete",
    notes:
      "1:1 engine/canonical task correlation (migration 019); idempotent materialization.",
  },
  {
    id: "S-023",
    workstream: "Platform / Engineering / Data",
    title: "WF-07 Neon Interactive Transaction Handling",
    priority: "High",
    status: "Complete",
    notes: "WebSocket-pool interactive transactions corrected; proven live on DEV.",
  },

  {
    id: "S-024",
    workstream: "Platform / Engineering / Data",
    title: "WF-08 Workflow Command Receipts & Idempotent Replay",
    priority: "High",
    status: "Complete",
    notes:
      "Pending receipt maps to retryable conflict; never a terminal outcome.",
  },
  {
    id: "S-025",
    workstream: "Platform / Engineering / Data",
    title: "WF-09 Workflow Reset & IT Support Diagnostics",
    priority: "Medium",
    status: "Complete",
    notes:
      "Read-only anomaly detectors; live terminal-invariant sweep clean.",
  },
  {
    id: "S-026",
    workstream: "Platform / Engineering / Data",
    title: "WF-10 Workflow End-to-End Trust Validation",
    priority: "High",
    status: "Complete",
    notes:
      "Duplicate replay, closing-date reschedule, and correlation trust validated.",
  },
  {
    id: "S-027",
    workstream: "Platform / Engineering / Data",
    title: "WF-11 TUNIT Harvest Register",
    priority: "Medium",
    status: "Complete",
    notes: "16 proven mechanisms harvested for the future TUNIT suite.",
  },
  {
    id: "S-028",
    workstream: "Platform / Engineering / Data",
    title: "WF-12 Join Release Concurrency Regression Test",
    priority: "Medium",
    status: "Complete",
    notes: "Join releases exactly once under simultaneous branch completions.",
  },
  {
    id: "S-029",
    workstream: "Platform / Engineering / Data",
    title: "TUNIT Formal Suite",
    priority: "High",
    status: "Planned",
    notes:
      "Recommended next story; converts the harvest register into a runnable suite.",
  },
  {
    id: "S-030",
    workstream: "Platform / Engineering / Data",
    title: "RE Supermodel Deployment to Neon",
    priority: "High",
    status: "Planned",
    notes:
      "Deploy command exists; needs a reviewed environment and explicit authorization.",
  },
  {
    id: "S-033",
    workstream: "Platform / Engineering / Data",
    title: "Engine Error / Terminal Semantics (END / ERROR / CONFLICT)",
    priority: "Medium-High",
    status: "Open",
    notes:
      "Archaeology §12: typed error/cancelled/conflict outcomes; failed jobs propagate.",
  },
  {
    id: "S-034",
    workstream: "Platform / Engineering / Data",
    title: "Engine Job Lease Requeue & Timer Auto-Advance",
    priority: "Medium-High",
    status: "Open",
    notes:
      "Archaeology §8/§10: expired leases requeue; definition-level timers advance tokens.",
  },
  {
    id: "S-035",
    workstream: "Platform / Engineering / Data",
    title: "Engine Join Correlation & Optional Branch Hardening",
    priority: "High-ish",
    status: "Open",
    notes:
      "Archaeology §11: optional/cancelled branches; nested fork-join correlation.",
  },
  {
    id: "S-036",
    workstream: "Platform / Engineering / Data",
    title: "Engine Optimistic Concurrency Guard Enforcement",
    priority: "High-ish",
    status: "Open",
    notes:
      "Archaeology §7/§8: optimistic guards on move / complete / completeToken.",
  },
  {
    id: "S-037",
    workstream: "Platform / Engineering / Data",
    title: "Application Command Inventory Completion",
    priority: "High",
    status: "Open",
    notes: "Idempotency and preconditions for every command class (C/D).",
  },
  {
    id: "S-038",
    workstream: "Platform / Engineering / Data",
    title: "Operational Seams: Alerts / Deadlines / SME / Audit",
    priority: "Medium-High",
    status: "Open",
    notes:
      "operational-contracts.ts seams; a reviewed deferral of a seam is acceptable.",
  },
  {
    id: "S-039",
    workstream: "Platform / Engineering / Data",
    title: "Domain Event Persistence & Audit Trail",
    priority: "Medium",
    status: "Open",
    notes:
      "Decision required; no application_event table exists today.",
  },
  {
    id: "S-040",
    workstream: "Platform / Engineering / Data",
    title: "Media / Attachment & Retention Policy for Provider Ingestion",
    priority: "Medium-High",
    status: "Open",
    notes:
      "Policy must precede any byte ingestion; provider URLs are never media.",
  },
  {
    id: "S-041",
    workstream: "Platform / Engineering / Data",
    title: "Workflow Visual Modeler (legacy Story 129 Future Contract)",
    priority: "Later",
    status: "Open",
    notes:
      "Future; held by no-UI constraint; round-trips the same XML grammar.",
  },
]


// ---------------------------------------------------------------------------
// Dashboard model
//
// weight — the workstream's share of total program weight. Each story
//          contributes priority points (Critical 5 … Later 1); a workstream's
//          weight is its share of the summed points across all stories.
// completionPercent — priority-weighted completion within the workstream,
//          where each status maps to a completion fraction (below).
// weightedContribution — weight × completionPercent (percentage points);
//          summing across workstreams gives overall program completion.
//
// The three summary metrics aggregate the same story data:
//   Architecture / Foundation % — the Platform / Engineering / Data workstream.
//   Usable Product % — CRM / Intake + Portal / Operations + Public Property.
//   Brokerage-Ready % — the brokerage-operational cluster: Portal / Operations
//          workstream stories plus the operational platform stories that must
//          be done for the brokerage to run daily deals end-to-end.
// ---------------------------------------------------------------------------

/** Completion fraction per status. Human-readable policy, not repo-derived. */
export const STATUS_COMPLETION: Record<StoryStatus, number> = {
  Complete: 1,
  Operationalized: 1,
  "Minor remainder": 0.9,
  "Read-side complete": 0.85,
  "Readiness PASS": 0.85,
  Partial: 0.5,
  Planned: 0.15,
  "Hardware/content dependent": 0.15,
  Blocked: 0.1,
  Open: 0.05,
  Deferred: 0,
}

/** Relative weight per priority (used for workstream weights). */
export const PRIORITY_POINTS: Record<StoryPriority, number> = {
  Critical: 5,
  High: 4,
  "High-ish": 3.5,
  "Medium-High": 3,
  Medium: 2,
  Low: 1.5,
  Later: 1,
  "High-value polish": 1,
}

/** Display ordering for priorities (low number = higher on the page). */
export const PRIORITY_ORDER: Record<StoryPriority, number> = {
  Critical: 0,
  High: 1,
  "High-ish": 2,
  "Medium-High": 3,
  Medium: 4,
  Low: 5,
  Later: 6,
  "High-value polish": 7,
}

/** Stories whose completion determines the Brokerage-Ready metric. */
export const BROKERAGE_READY_STORY_IDS: ReadonlySet<string> = new Set([
  "S-012",
  "S-013",
  "S-014",
  "S-015",
  "S-016",
  "S-030",
  "S-031",
  "S-032",
  "S-037",
  "S-038",
])

export type WorkstreamMetric = {
  workstream: Workstream
  weight: number
  completionPercent: number
  weightedContribution: number
  storyCount: number
  completeCount: number
}

export type SummaryMetric = {
  label: string
  percent: number
  detail: string
}

export type StoryBoardModel = {
  stories: StoryRecord[]
  workstreams: WorkstreamMetric[]
  summary: SummaryMetric[]
  overallPercent: number
}

function round(value: number, digits = 1) {
  const factor = 10 ** digits
  return Math.round(value * factor) / factor
}

function completionPercentFor(stories: StoryRecord[]): number {
  const total = stories.reduce((sum, s) => sum + PRIORITY_POINTS[s.priority], 0)
  if (total === 0) return 0
  const done = stories.reduce(
    (sum, s) => sum + PRIORITY_POINTS[s.priority] * STATUS_COMPLETION[s.status],
    0,
  )
  return (done / total) * 100
}

export function buildStoryBoardModel(): StoryBoardModel {
  const stories = [...STORIES]

  const totalWeight = stories.reduce(
    (sum, s) => sum + PRIORITY_POINTS[s.priority],
    0,
  )

  const workstreams = WORKSTREAMS.map((workstream) => {
    const group = stories.filter((s) => s.workstream === workstream)
    const groupWeight = group.reduce(
      (sum, s) => sum + PRIORITY_POINTS[s.priority],
      0,
    )
    const completionPercent = completionPercentFor(group)
    const weight = totalWeight > 0 ? (groupWeight / totalWeight) * 100 : 0

    return {
      workstream,
      weight: round(weight),
      completionPercent: round(completionPercent),
      weightedContribution: round((weight / 100) * completionPercent),
      storyCount: group.length,
      completeCount: group.filter(
        (s) => s.status === "Complete" || s.status === "Operationalized",
      ).length,
    }
  })

  const overallPercent = round(
    workstreams.reduce(
      (sum, ws) => sum + ws.weightedContribution,
      0,
    ),
  )

  const foundationStories = stories.filter(
    (s) => s.workstream === "Platform / Engineering / Data",
  )
  const productStories = stories.filter(
    (s) =>
      s.workstream === "CRM / Intake" ||
      s.workstream === "Portal / Operations" ||
      s.workstream === "Public Property / Buyer Experience",
  )
  const brokerageStories = stories.filter((s) =>
    BROKERAGE_READY_STORY_IDS.has(s.id),
  )

  const summary: SummaryMetric[] = [
    {
      label: "Architecture / Foundation",
      percent: round(completionPercentFor(foundationStories)),
      detail: `${foundationStories.length} platform stories`,
    },
    {
      label: "Usable Product",
      percent: round(completionPercentFor(productStories)),
      detail: `${productStories.length} product stories`,
    },
    {
      label: "Brokerage-Ready",
      percent: round(completionPercentFor(brokerageStories)),
      detail: `${brokerageStories.length} brokerage-operational stories`,
    },
  ]

  return { stories, workstreams, summary, overallPercent }
}
