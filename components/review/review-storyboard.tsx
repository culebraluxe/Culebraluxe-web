// ═══════════════════════════════════════════════════════════════════════════
// ⚠️ TEMP ARCHITECT REVIEW SEAM — REMOVE WHEN EXTERNAL REVIEW ACCESS IS NO
//    LONGER NEEDED.
//
// Read-only Story Board preview: the SAME live data projection as
// /portal/storyboard (stories + Forge execution + domain rollup model), but
// rendered WITHOUT the editable story table / filter controls / server
// actions. Reuses the portal data layer (lib/storyboard-data.ts,
// db/storyboard.ts) and the presentational helpers from
// story-detail-sections.tsx only. No mutation surface.
// ═══════════════════════════════════════════════════════════════════════════

import { PageHeader } from "@/components/portal/page-header"
import {
  dateLabel,
  statusPillClasses,
} from "@/components/portal/storyboard/story-detail-sections"
import {
  listStoryboardStories,
  listStoryExecutionSummaries,
} from "@/db/storyboard"
import {
  buildStoryBoardModel,
  isExecutionActive,
  isExecutionError,
  selectNextWork,
  workstreamName,
  type StoryBoardModel,
  type StoryDomainRollup,
  type StoryRecord,
} from "@/lib/storyboard-data"

const badgeBase =
  "inline-block whitespace-nowrap rounded-full px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.14em]"

function ExecutionBadge({ story }: { story: StoryRecord }) {
  const exec = story.execution ?? null
  const state = exec?.workItemState ?? null
  const result = exec?.latestRunResult ?? null

  if (isExecutionActive(state)) {
    return (
      <span className={`${badgeBase} bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]`}>
        {state}
      </span>
    )
  }
  if (isExecutionError({ workItemState: state, latestRunResult: result })) {
    return (
      <span className={`${badgeBase} bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]`}>
        {state ?? result}
      </span>
    )
  }
  if (state === "Done") {
    return (
      <span className={`${badgeBase} bg-[var(--portal-success-pale)] text-[var(--portal-success)]`}>
        Done
      </span>
    )
  }
  if (state) {
    return (
      <span className={`${badgeBase} bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]`}>
        {state}
      </span>
    )
  }
  if (result) {
    return (
      <span className={`${badgeBase} bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]`}>
        {result}
      </span>
    )
  }
  return <span className={`${badgeBase} border border-black/10 text-black/35`}>Not run</span>
}

function SummaryStrip({ model }: { model: StoryBoardModel }) {
  const stats: Array<{ label: string; value: string | number }> = [
    { label: "Net-Net", value: `${model.netNet.toFixed(1)}%` },
    { label: "Stories", value: model.totalStories },
    { label: "Complete", value: model.totalComplete },
    { label: "In Progress / Partial", value: model.totalInProgressPartial },
    { label: "Blocked / Failed", value: model.totalBlockedFailed },
    { label: "Ready", value: model.totalReady },
    { label: "Running", value: model.totalRunning },
    { label: "Error / Cancelled", value: model.totalError },
    { label: "Unclassified", value: model.unclassifiedCount },
  ]
  return (
    <section className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-9">
      {stats.map((s) => (
        <div
          key={s.label}
          className="rounded-sm border border-[var(--portal-border)] bg-white px-4 py-3"
        >
          <div className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
            {s.label}
          </div>
          <div className="mt-1 font-serif text-xl font-light text-[var(--portal-navy)]">
            {s.value}
          </div>
        </div>
      ))}
    </section>
  )
}

function DomainCard({ domain }: { domain: StoryDomainRollup }) {
  return (
    <div className="rounded-sm border border-[var(--portal-border)] bg-white p-5">
      <div className="flex items-baseline justify-between gap-2">
        <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
          {domain.label}
        </div>
        <div className="font-serif text-2xl font-light leading-none text-[var(--portal-navy)]">
          {domain.storyCount}
        </div>
      </div>
      <div className="mt-3">
        <div className="h-1.5 w-full overflow-hidden bg-[var(--portal-blue-pale)]">
          <div
            className="h-full bg-[var(--portal-navy)]"
            style={{ width: `${domain.completionPercent}%` }}
          />
        </div>
        <div className="mt-1 text-[11px] font-light tabular-nums text-black/45">
          {domain.completionPercent.toFixed(1)}% complete
        </div>
      </div>
      <div className="mt-3 flex flex-wrap gap-1.5">
        {domain.subgroups.map((sg) => (
          <span
            key={sg.subgroup}
            className="rounded-full border border-black/10 px-2 py-0.5 text-[10px] font-light text-black/50"
          >
            {sg.subgroup} · {sg.storyCount}
          </span>
        ))}
      </div>
    </div>
  )
}

export async function ReviewStoryBoard() {
  const [stories, executions] = await Promise.all([
    listStoryboardStories(),
    listStoryExecutionSummaries(),
  ])

  if (!stories) {
    return (
      <div>
        <PageHeader
          eyebrow="Portal Preview"
          title="Story Board"
          subtitle="Read-only preview — same live projection as /portal/storyboard."
        />
        <section className="rounded-sm border border-[var(--portal-border)] bg-white px-10 py-16 text-center">
          <h2 className="font-serif text-2xl font-light text-[var(--portal-navy)]">
            Story Board storage not ready
          </h2>
          <p className="mx-auto mt-3 max-w-lg text-sm font-light leading-6 text-black/55">
            The storyboard tables have not been applied to this database yet.
          </p>
        </section>
      </div>
    )
  }

  const execMap = new Map(executions.map((e) => [e.storyId, e]))
  const withExecution = stories.map((s) => ({
    ...s,
    execution: execMap.get(s.id) ?? null,
  }))
  const model = buildStoryBoardModel(withExecution)
  const nextWork = selectNextWork(withExecution)

  return (
    <div>
      <PageHeader
        eyebrow="Portal Preview"
        title="Story Board"
        subtitle="Read-only preview — same live projection as /portal/storyboard."
      />

      <SummaryStrip model={model} />

      <section className="mt-6 grid gap-4 sm:grid-cols-2 xl:grid-cols-5">
        {model.domains.map((d) => (
          <DomainCard key={d.domain} domain={d} />
        ))}
      </section>

      {nextWork.entries.length > 0 ? (
        <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white p-5">
          <h2 className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-blue-gray)]">
            Next Work
          </h2>
          <ul className="mt-3 space-y-2">
            {nextWork.entries.map(({ story, rank }) => (
              <li
                key={story.id}
                className="flex items-baseline gap-3 text-sm font-light text-black/70"
              >
                <span className="font-mono text-xs text-[var(--portal-blue-gray)]">{rank}.</span>
                <span className="font-mono text-xs text-[var(--portal-navy-soft)]">{story.id}</span>
                <span className="min-w-0 flex-1 truncate">{story.title}</span>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <section className="mt-6 overflow-x-auto rounded-sm border border-[var(--portal-border)] bg-white">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-[var(--portal-border)] text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
              <th className="px-4 py-3">ID</th>
              <th className="px-4 py-3">Title</th>
              <th className="px-4 py-3">Workstream</th>
              <th className="px-4 py-3">Status</th>
              <th className="px-4 py-3">Completion</th>
              <th className="px-4 py-3">Execution</th>
              <th className="px-4 py-3">Latest run</th>
            </tr>
          </thead>
          <tbody>
            {model.stories.map((story) => (
              <tr key={story.id} className="border-b border-[var(--portal-border)] last:border-b-0">
                <td className="px-4 py-3 font-mono text-xs text-[var(--portal-navy-soft)]">
                  {story.id}
                </td>
                <td className="max-w-xs truncate px-4 py-3 font-light text-black/70">
                  {story.title}
                </td>
                <td className="px-4 py-3 text-xs font-light text-black/50">
                  {workstreamName(story.workstream)}
                </td>
                <td className="px-4 py-3">
                  <span className={`${badgeBase} ${statusPillClasses(story.status)}`}>
                    {story.status}
                  </span>
                </td>
                <td className="px-4 py-3 tabular-nums text-xs text-black/55">
                  {story.completion}%
                </td>
                <td className="px-4 py-3">
                  <ExecutionBadge story={story} />
                </td>
                <td className="px-4 py-3 text-xs font-light tabular-nums text-black/45">
                  {dateLabel(
                    story.execution?.latestRunAt
                      ? String(story.execution.latestRunAt)
                      : null,
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  )
}
