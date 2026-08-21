import { WORKSTREAMS, type StoryBoardModel } from "@/lib/storyboard-data"
import { StoryBoardTable } from "@/components/portal/write/story-board-table"

// ---------------------------------------------------------------------------
// Story Board — server view. Metrics stay server-computed from Neon data; the
// editable story table is a client component (create / edit / change status).
// No workflow_engine involvement.
// ---------------------------------------------------------------------------

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
            Editable
          </span>
        </div>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          The authoritative home for the existing human-authored backlog —
          create, edit, and change status here. Changes are persisted to Neon.
          No repository-derived stories, no new items, no implementation.
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

      <StoryBoardTable stories={model.stories} />

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
          S-038). Statuses and priorities use the program vocabulary. Stories
          are stored in{" "}
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            storyboard_story
          </code>{" "}
          (migration 021) — no workflow_engine changes.
        </p>
      </footer>
    </div>
  )
}

export function StoryBoardNotReady() {
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
            Not ready
          </span>
        </div>
      </header>

      <section className="rounded-sm border border-[var(--portal-border)] bg-white px-10 py-16 text-center">
        <h2 className="font-serif text-2xl font-light text-[var(--portal-navy)]">
          Story Board storage not ready
        </h2>
        <p className="mx-auto mt-3 max-w-lg text-sm font-light leading-6 text-black/55">
          The storyboard table has not been applied to this database yet. Apply
          the reviewed migration (
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            db/migrations/021_storyboard_story.sql
          </code>
          ) to enable the board. The migration also seeds the existing 41
          human-authored stories.
        </p>
      </section>
    </div>
  )
}
