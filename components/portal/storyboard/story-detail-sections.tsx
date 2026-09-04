import { type ReactNode } from "react"

import type { StoryRun, StoryboardStory } from "@/db/storyboard"
import {
  statusBucket,
  workstreamName,
  type StoryRecord,
} from "@/lib/storyboard-data"

// ---------------------------------------------------------------------------
// Story detail sections — shared, server-safe presentational component.
//
// Used both on the direct story URL (/portal/storyboard/[id]) and by the board
// inline Inspect panel. All story contract content (goal, notes, architect
// brief, context refs, acceptance criteria, postconditions, execution history
// with immutable run snapshots) is rendered as plain server HTML — no
// client-only expansion is required to obtain the substantive story contract.
// ---------------------------------------------------------------------------

export function dateLabel(value: string | null): string {
  if (!value) return "—"
  return value.slice(0, 10)
}

export function statusPillClasses(status: string): string {
  switch (statusBucket(status)) {
    case "complete":
      return "bg-emerald-50 text-emerald-700"
    case "partial":
      return "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]"
    case "blocked":
      return "bg-red-50 text-red-700"
    default:
      return "bg-black/5 text-black/55"
  }
}

function DetailSection({
  title,
  children,
  hint,
}: {
  title: string
  children: ReactNode
  hint?: string
}) {
  return (
    <div className="rounded-sm border border-[var(--portal-border)] bg-white p-4">
      <div className="flex items-baseline justify-between gap-4">
        <h5 className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
          {title}
        </h5>
        {hint && (
          <span className="text-[10px] font-light text-black/35">{hint}</span>
        )}
      </div>
      <div className="mt-2 space-y-2 text-sm font-light leading-6 text-black/70">
        {children}
      </div>
    </div>
  )
}

function DetailField({
  label,
  value,
  distinct,
}: {
  label: string
  value: string | null
  distinct?: boolean
}) {
  if (!value) return null
  return (
    <div>
      <div
        className={`text-[10px] font-light uppercase tracking-[0.18em] ${
          distinct ? "text-[var(--portal-blue-gray)]" : "text-black/35"
        }`}
      >
        {label}
      </div>
      <p className={`mt-1 whitespace-pre-wrap ${distinct ? "text-[var(--portal-blue-gray)]" : ""}`}>
        {value}
      </p>
    </div>
  )
}

function SnapshotField({
  label,
  value,
}: {
  label: string
  value: string | null
}) {
  return (
    <div>
      <span className="text-black/35">{label}: </span>
      <span className="whitespace-pre-wrap">{value ?? "—"}</span>
    </div>
  )
}

// ENG-FORGE-V6-VIS — one truth, Portal lens. Frozen Run contract vs the live
// parent Story: any drift means the Run executed against older truth.
// Rendered server-side from already-loaded rows; no new queries.
function RunContractPanel({ story, run }: { story: StoryboardStory; run: StoryRun }) {
  const drifts: Array<{ field: string; frozen: string | null; live: string | null }> = []
  const pairs: Array<{ field: string; frozen?: string | null; live?: string | null }> = [
    { field: 'Goal', frozen: run.goalSnapshot, live: story.goal },
    { field: 'Scope', frozen: run.scopeSnapshot, live: story.scope },
    { field: 'Dependencies', frozen: run.dependenciesSnapshot, live: story.dependencies },
    { field: 'Preconditions', frozen: run.preconditionsSnapshot, live: story.preconditions },
    { field: 'Architect brief', frozen: run.architectBriefSnapshot, live: story.architectBrief },
    { field: 'Context refs', frozen: run.contextRefsSnapshot, live: story.contextRefs },
    { field: 'Acceptance criteria', frozen: run.acceptanceCriteriaSnapshot, live: story.acceptanceCriteria },
    { field: 'Postconditions', frozen: run.postconditionsSnapshot, live: story.postconditions },
    { field: 'Test mode', frozen: run.testModeSnapshot, live: story.testMode ?? null },
    { field: 'Assay commands', frozen: run.assayCommandsSnapshot, live: story.assayCommands ?? null },
  ]
  for (const pair of pairs) {
    const frozen = (pair.frozen ?? '').trim()
    const live = (pair.live ?? '').trim()
    if (frozen !== live) {
      drifts.push({ field: pair.field, frozen: pair.frozen ?? null, live: pair.live ?? null })
    }
  }
  const packetStale =
    (run.packetShaSnapshot ?? '').trim() !== '' &&
    (story.packetSha ?? '').trim() !== '' &&
    run.packetShaSnapshot!.trim() !== story.packetSha!.trim()

  const counters: Array<{ label: string; value: number | null }> = [
    { label: 'Commands', value: run.commandsTotal ?? null },
    { label: 'Passed', value: run.commandsPassed ?? null },
    { label: 'Failed', value: run.commandsFailed ?? null },
    { label: 'Tests', value: run.testsTotal ?? null },
    { label: 'Tests passed', value: run.testsPassed ?? null },
    { label: 'Tests failed', value: run.testsFailed ?? null },
    { label: 'Policy violations', value: run.policyViolationCount ?? null },
  ]
  const hasCounters = counters.some(({ value }) => value !== null)

  return (
    <DetailSection
      title="Run Contract (frozen vs live)"
      hint={packetStale ? 'Packet stale — Neon sha != live sha' : undefined}
    >
      <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
        <span className="text-black/40">Run phase</span>
        <span>{run.runPhase ?? '—'}</span>
        <span className="text-black/40">Lead decision</span>
        <span>
          {run.leadDecision ?? '—'}
          {run.leadSplitCount !== null && run.leadSplitCount !== undefined
            ? ` (${run.leadSplitCount})`
            : ''}
        </span>
        <span className="text-black/40">Base commit</span>
        <span className="font-mono">
          {run.baseCommitHash ? run.baseCommitHash.slice(0, 12) : '—'}
        </span>
        <span className="text-black/40">Frozen packet sha</span>
        <span className="font-mono">
          {run.packetShaSnapshot ? run.packetShaSnapshot.slice(0, 12) : '—'}
        </span>
        <span className="text-black/40">Failure code</span>
        <span>{run.failureCode ?? '—'}</span>
        <span className="text-black/40">Model used</span>
        <span className="font-mono">{run.modelUsed ?? '—'}</span>
        {(run.tokensInput !== null && run.tokensInput !== undefined) ||
        (run.tokensOutput !== null && run.tokensOutput !== undefined) ||
        (run.costUsd !== null && run.costUsd !== undefined) ? (
          <>
            <span className="text-black/40">Tokens in/out</span>
            <span className="font-mono">
              {run.tokensInput ?? '—'} / {run.tokensOutput ?? '—'}
            </span>
            <span className="text-black/40">Cost (USD)</span>
            <span className="font-mono">
              {run.costUsd !== null && run.costUsd !== undefined ? `$${Number(run.costUsd).toFixed(4)}` : '—'}
            </span>
          </>
        ) : null}
      </div>
      {hasCounters && (
        <div className="mt-2 grid grid-cols-3 gap-x-4 gap-y-1 text-xs">
          {counters.map(({ label, value }) => (
            <span key={label}>
              <span className="text-black/40">{label}: </span>
              {value ?? '—'}
            </span>
          ))}
        </div>
      )}
      {run.evidenceDetail && (
        <p className="mt-2 whitespace-pre-wrap text-xs font-light leading-5 text-black/55">
          {run.evidenceDetail}
        </p>
      )}
      {drifts.length === 0 ? (
        <p className="mt-2 text-xs font-light italic text-black/45">
          Frozen contract matches the live story — this run executed against current truth.
        </p>
      ) : (
        <div className="mt-2 space-y-2 border-t border-[var(--portal-border)] pt-2">
          <p className="text-[10px] font-light uppercase tracking-[0.18em] text-red-700">
            {drifts.length} drifted field{drifts.length === 1 ? '' : 's'} — run used frozen (left), live story now says (right)
          </p>
          {drifts.map(({ field, frozen, live }) => (
            <div key={field} className="grid gap-2 text-xs sm:grid-cols-2">
              <div className="rounded-sm bg-black/[0.03] p-2">
                <div className="text-[10px] uppercase tracking-[0.14em] text-black/40">
                  Frozen · {field}
                </div>
                <p className="mt-1 whitespace-pre-wrap">{frozen ?? '—'}</p>
              </div>
              <div className="rounded-sm bg-red-50 p-2">
                <div className="text-[10px] uppercase tracking-[0.14em] text-red-700/70">
                  Live · {field}
                </div>
                <p className="mt-1 whitespace-pre-wrap">{live ?? '—'}</p>
              </div>
            </div>
          ))}
        </div>
      )}
    </DetailSection>
  )
}

function RunHistory({ runs }: { runs: StoryRun[] }) {
  if (runs.length === 0) {
    return (
      <p className="text-sm font-light italic text-black/40">
        No execution runs for this story yet.
      </p>
    )
  }
  return (
    <div className="space-y-3">
      {runs.map((run) => (
        <div
          key={run.id}
          className="rounded-sm border border-[var(--portal-border)] bg-white p-4"
        >
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
            <span
              className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-xs font-light ${statusPillClasses(
                run.resultStatus ?? "Hold",
              )}`}
            >
              {run.resultStatus ?? "Started"}
            </span>
            <span className="text-xs font-light text-black/45">
              {dateLabel(run.startedAt.slice(0, 10))}
              {run.endedAt
                ? ` → ${dateLabel(run.endedAt.slice(0, 10))}`
                : " → open"}
            </span>
            {run.completion !== null && (
              <span className="font-serif text-lg font-light text-[var(--portal-navy)]">
                {run.completion}%
              </span>
            )}
            {run.commitHash && (
              <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
                {run.commitHash.slice(0, 12)}
              </code>
            )}
          </div>
          {run.notes && (
            <p className="mt-2 text-sm font-light leading-6 text-black/60">
              {run.notes}
            </p>
          )}
          {run.testsSummary && (
            <p className="mt-1 text-xs font-light leading-5 text-black/45">
              Tests: {run.testsSummary}
            </p>
          )}
          <details className="mt-2">
            <summary className="cursor-pointer text-[10px] font-light uppercase tracking-[0.18em] text-black/40 hover:text-black/60">
              Specification snapshot used for this run
            </summary>
            <div className="mt-2 space-y-2 border-t border-[var(--portal-border)] pt-2 text-xs font-light leading-5 text-black/60">
              <SnapshotField label="Goal" value={run.goalSnapshot} />
              <SnapshotField label="Preconditions" value={run.preconditionsSnapshot} />
              <SnapshotField label="Architect brief" value={run.architectBriefSnapshot} />
              <SnapshotField label="Context refs" value={run.contextRefsSnapshot} />
              <SnapshotField label="Acceptance criteria" value={run.acceptanceCriteriaSnapshot} />
              <SnapshotField label="Postconditions" value={run.postconditionsSnapshot} />
            </div>
          </details>
        </div>
      ))}
    </div>
  )
}

export function StoryDetailSections({
  story,
  runs,
}: {
  story: StoryboardStory
  runs: StoryRun[]
}) {
  const newestRun = runs[0] ?? null
  return (
    <div className="grid gap-4 lg:grid-cols-2">
      {newestRun && <RunContractPanel story={story} run={newestRun} />}
      <DetailSection title="Overview">
        <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
          <span className="text-black/40">ID</span>
          <span className="font-mono">{story.id}</span>
          <span className="text-black/40">Workstream</span>
          <span>
            {workstreamName(story.workstream)} ({story.workstream})
          </span>
          <span className="text-black/40">Priority</span>
          <span>{story.priority}</span>
          <span className="text-black/40">Status</span>
          <span>{story.status}</span>
          <span className="text-black/40">Completion</span>
          <span>{story.completion}%</span>
          <span className="text-black/40">Rollup</span>
          <span>{story.rollup ? "Yes" : "No"}</span>
        </div>
        <p className="mt-2 text-xs font-light text-black/45">{story.title}</p>
      </DetailSection>

      <DetailSection title="Dates">
        <div className="grid grid-cols-3 gap-x-4 gap-y-1 text-xs">
          <span className="text-black/40">Planned</span>
          <span className="text-black/40">Actual</span>
          <span className="text-black/40">Completed</span>
          <span>{dateLabel(story.plannedStartAt)}</span>
          <span>{dateLabel(story.actualStartAt)}</span>
          <span>{dateLabel(story.completedAt)}</span>
        </div>
        <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
          <span className="text-black/40">Created</span>
          <span>{dateLabel(story.createdAt)}</span>
          <span className="text-black/40">Updated</span>
          <span>{dateLabel(story.updatedAt)}</span>
        </div>
      </DetailSection>

      <DetailSection title="Product Context">
        <DetailField label="Goal" value={story.goal} />
        <DetailField label="Human notes" value={story.notes || null} />
        <DetailField label="Dependencies" value={story.dependencies} />
        <DetailField label="Preconditions" value={story.preconditions} />
        {!story.goal &&
          !story.notes &&
          !story.dependencies &&
          !story.preconditions && (
            <p className="text-sm font-light italic text-black/40">
              No product context recorded yet.
            </p>
          )}
      </DetailSection>

      <DetailSection
        title="Architect Handoff"
        hint={
          story.architectBriefUpdatedAt
            ? `Updated ${dateLabel(story.architectBriefUpdatedAt)}`
            : undefined
        }
      >
        {story.architectBrief ? (
          <DetailField
            label="Architect brief"
            value={story.architectBrief}
            distinct
          />
        ) : (
          <p className="text-sm font-light italic text-black/40">
            No architect brief yet.
          </p>
        )}
        <DetailField label="Context references" value={story.contextRefs} />
        {story.architectBriefUpdatedAt && (
          <p className="text-[10px] font-light uppercase tracking-[0.18em] text-black/35">
            Architect brief updated at {story.architectBriefUpdatedAt}
          </p>
        )}
      </DetailSection>

      <DetailSection title="Definition of Done">
        <DetailField
          label="Acceptance criteria"
          value={story.acceptanceCriteria}
        />
        <DetailField label="Postconditions" value={story.postconditions} />
        {!story.acceptanceCriteria && !story.postconditions && (
          <p className="text-sm font-light italic text-black/40">
            No definition of done recorded yet.
          </p>
        )}
      </DetailSection>

      <DetailSection title="Execution History">
        <div className="mb-1 text-[10px] font-light uppercase tracking-[0.18em] text-black/35">
          {runs.length} run{runs.length === 1 ? "" : "s"} · newest first
        </div>
        <RunHistory runs={runs} />
      </DetailSection>
    </div>
  )
}
