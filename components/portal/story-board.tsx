import {
  WORKSTREAMS,
  type StoryBoardModel,
  type StoryPriority,
  type StoryStatus,
} from "@/lib/storyboard-data"

// ---------------------------------------------------------------------------
// Read-only Story Board. No CRUD, no database, no workflow_engine involvement.
// ---------------------------------------------------------------------------

const statusClasses: Record<StoryStatus, string> = {
  Complete: "bg-emerald-50 text-emerald-700",
  Operationalized: "bg-emerald-50 text-emerald-700",
  "Minor remainder": "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]",
  "Read-side complete": "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]",
  "Readiness PASS": "bg-amber-50 text-amber-700",
  Partial: "bg-black/5 text-black/55",
  Planned: "bg-black/5 text-black/55",
  Open: "bg-black/5 text-black/55",
  Blocked: "bg-red-50 text-red-700",
  Deferred: "bg-black/5 text-black/40",
  "Hardware/content dependent": "bg-black/5 text-black/55",
}

const priorityClasses: Record<StoryPriority, string> = {
  Critical: "bg-red-50 text-red-700",
  High: "bg-[var(--portal-blue-pale)] text-[var(--portal-navy)]",
  "High-ish": "bg-[#c6a15b]/15 text-[#8a6d2f]",
  "Medium-High": "bg-black/5 text-black/60",
  Medium: "bg-black/5 text-black/50",
  Low: "bg-black/5 text-black/40",
  Later: "bg-black/5 text-black/40",
  "High-value polish": "bg-[#c6a15b]/15 text-[#8a6d2f]",
}

function StatusPill({ status }: { status: StoryStatus }) {
  return (
    <span
      className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-xs font-light ${statusClasses[status]}`}
    >
      {status}
    </span>
  )
}

function PriorityPill({ priority }: { priority: StoryPriority }) {
  return (
    <span
      className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-xs font-light ${priorityClasses[priority]}`}
    >
      {priority}
    </span>
  )
}

function MetricCard({
  label,
  percent,
  detail,
}: {
  label: string
  percent: number
  detail: string
}) {
  return (
    <div className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
        {label}
      </div>
      <div className="mt-4 font-serif text-3xl font-light text-[var(--portal-navy)]">
        {percent.toFixed(0)}%
      </div>
      <div className="mt-2 text-xs font-light text-black/40">{detail}</div>
    </div>
  )
}

function WorkstreamRow({ model }: { model: StoryBoardModel }) {
  const totalStories = model.stories.length

  return (
    <tbody>
      {WORKSTREAMS.map((workstream) => {
        const metric = model.workstreams.find(
          (ws) => ws.workstream === workstream,
        )!

        return (
          <tr
            key={workstream}
            className="border-b border-[var(--portal-border)] last:border-b-0"
          >
            <td className="px-6 py-4 align-top">
              <div className="text-sm font-light text-[var(--portal-navy)]">
                {workstream}
              </div>
              <div className="mt-1 text-xs font-light text-black/40">
                {metric.storyCount} story
                {metric.storyCount === 1 ? "" : "s"} ·{" "}
                {metric.completeCount} complete
              </div>
            </td>

            <td className="px-6 py-4 align-top">
              <div className="font-serif text-xl font-light text-[var(--portal-navy)]">
                {metric.weight.toFixed(1)}%
              </div>
              <div className="mt-1 text-xs font-light text-black/40">
                share of program weight
              </div>
            </td>

            <td className="px-6 py-4 align-top">
              <div className="mb-2 flex items-center gap-3">
                <div className="h-1.5 flex-1 overflow-hidden bg-[var(--portal-blue-pale)]">
                  <div
                    className="h-full bg-[var(--portal-navy)]"
                    style={{ width: `${metric.completionPercent}%` }}
                  />
                </div>
                <span className="font-serif text-lg font-light text-[var(--portal-navy)]">
                  {metric.completionPercent.toFixed(1)}%
                </span>
              </div>
              <div className="text-xs font-light text-black/40">
                priority-weighted completion
              </div>
            </td>

            <td className="px-6 py-4 align-top text-right">
              <div className="font-serif text-xl font-light text-[var(--portal-navy)]">
                {metric.weightedContribution.toFixed(1)}
                <span className="ml-1 text-xs font-light text-black/40">pp</span>
              </div>
              <div className="mt-1 text-xs font-light text-black/40">
                {totalStories > 0 && model.overallPercent > 0
                  ? `${((metric.weightedContribution / model.overallPercent) * 100).toFixed(0)}% of overall`
                  : "0% of overall"}
              </div>
            </td>
          </tr>
        )
      })}
    </tbody>
  )
}

function StoryTable({ model }: { model: StoryBoardModel }) {
  return (
    <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="border-b border-[var(--portal-border)] px-6 py-6">
        <div className="flex items-baseline justify-between gap-4">
          <h2 className="font-serif text-2xl font-light">Story Backlog</h2>
          <span className="text-xs font-light uppercase tracking-[0.18em] text-black/40">
            {model.stories.length} stories · read-only
          </span>
        </div>
        <p className="mt-1 text-sm font-light text-black/50">
          The existing human-authored backlog, grouped by workstream. No
          repository-derived stories; nothing here is implemented by this view.
        </p>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full border-collapse text-left text-sm">
          <thead>
            <tr className="border-b border-[var(--portal-border)]">
              <th className="w-24 px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                ID
              </th>
              <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                Story
              </th>
              <th className="w-40 px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                Priority
              </th>
              <th className="w-48 px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                Status
              </th>
              <th className="min-w-72 px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                Notes
              </th>
            </tr>
          </thead>

          {WORKSTREAMS.map((workstream) => {
            const rows = model.stories.filter(
              (s) => s.workstream === workstream,
            )

            return (
              <tbody key={workstream}>
                <tr className="bg-[var(--portal-blue-pale)]/50">
                  <td
                    colSpan={5}
                    className="px-6 py-3 text-[10px] font-light uppercase tracking-[0.24em] text-[var(--portal-navy)]"
                  >
                    {workstream}
                    <span className="ml-3 text-black/40">
                      {rows.length} story{rows.length === 1 ? "" : "s"}
                    </span>
                  </td>
                </tr>

                {rows.length === 0 ? (
                  <tr className="border-b border-[var(--portal-border)]">
                    <td
                      colSpan={5}
                      className="px-6 py-6 text-sm font-light italic text-black/40"
                    >
                      No stories tracked under this workstream yet.
                    </td>
                  </tr>
                ) : (
                  rows.map((story) => (
                    <tr
                      key={story.id}
                      className="border-b border-[var(--portal-border)] last:border-b-0"
                    >
                      <td className="px-6 py-4 align-top font-mono text-xs text-[var(--portal-navy)]">
                        {story.id}
                      </td>
                      <td className="px-6 py-4 align-top">
                        <div className="font-light leading-6 text-[var(--portal-navy)]">
                          {story.title}
                        </div>
                      </td>
                      <td className="px-6 py-4 align-top">
                        <PriorityPill priority={story.priority} />
                      </td>
                      <td className="px-6 py-4 align-top">
                        <StatusPill status={story.status} />
                      </td>
                      <td className="px-6 py-4 align-top text-sm font-light leading-6 text-black/55">
                        {story.notes}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            )
          })}
        </table>
      </div>
    </section>
  )
}


export function StoryBoard({ model }: { model: StoryBoardModel }) {
  return (
    <div>
      <header className="mb-8">
        <p className="text-xs font-light uppercase tracking-[0.28em] text-black/40">
          Portal
        </p>

        <div className="mt-3 flex items-baseline gap-4">
          <h1 className="font-serif text-4xl font-light leading-[1.1]">
            Story Board
          </h1>
          <span className="rounded-full bg-[#c6a15b]/15 px-3 py-1 text-xs font-light uppercase tracking-[0.16em] text-[#8a6d2f]">
            Read-only
          </span>
        </div>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          Program view of the existing human-authored backlog — no
          repository-derived stories, no new items, no implementation.
        </p>
      </header>

      <section className="grid gap-4 md:grid-cols-3">
        {model.summary.map((metric) => (
          <MetricCard
            key={metric.label}
            label={metric.label}
            percent={metric.percent}
            detail={metric.detail}
          />
        ))}
      </section>

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="border-b border-[var(--portal-border)] px-6 py-6">
          <h2 className="font-serif text-2xl font-light">
            Executive Workstream Dashboard
          </h2>
          <p className="mt-1 text-sm font-light text-black/50">
            Weight reflects each workstream&apos;s share of program priority
            points; completion is priority-weighted by status; weighted
            contribution is weight × completion. Overall program completion is{" "}
            <span className="font-normal text-[var(--portal-navy)]">
              {model.overallPercent.toFixed(1)}%
            </span>
            .
          </p>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full border-collapse text-left text-sm">
            <thead>
              <tr className="border-b border-[var(--portal-border)]">
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Workstream
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Weight
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Completion
                </th>
                <th className="px-6 py-4 text-right text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Weighted Contribution
                </th>
              </tr>
            </thead>
            <WorkstreamRow model={model} />
          </table>
        </div>
      </section>

      <StoryTable model={model} />

      <footer className="mt-6 text-xs font-light leading-6 text-black/40">
        <p>
          Seed: existing human-authored backlog from{" "}
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            docs/workflow/MASTER_STORYBOARD.md
          </code>{" "}
          and{" "}
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            docs/workflow/STORYBOARD_STATUS.md
          </code>{" "}
          (2026-08-20, main @ fddcd26). Summary metrics: Architecture /
          Foundation aggregates the Platform / Engineering / Data workstream;
          Usable Product aggregates CRM / Intake, Portal / Operations and Public
          Property / Buyer Experience; Brokerage-Ready aggregates the
          brokerage-operational cluster (S-012…S-016, S-030…S-032, S-037,
          S-038). Statuses and priorities use the program vocabulary. This page
          is read-only — no CRUD, no database changes, no workflow_engine
          changes.
        </p>
      </footer>
    </div>
  )
}
