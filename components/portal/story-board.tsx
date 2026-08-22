import { type StoryBoardFilter, type StoryBoardModel, type StoryRecord, type NextWorkSelection } from "@/lib/storyboard-data"
import { StoryBoardControls } from "@/components/portal/storyboard/story-board-controls"
import { StoryBoardTable } from "@/components/portal/write/story-board-table"
import { statusPillClasses } from "@/components/portal/storyboard/story-detail-sections"
import Link from "next/link"

// ---------------------------------------------------------------------------
// Story Board — server view. Rollup counts and the Net-Net completion are
// computed from persisted stories by lib/storyboard-data.ts; the editable
// story table is a client component (create / edit / change status). No
// workflow_engine involvement.
// ---------------------------------------------------------------------------

function CompletionBar({ percent }: { percent: number }) {
  return (
    <div className="flex items-center gap-3">
      <div className="h-1.5 w-28 overflow-hidden bg-[var(--portal-blue-pale)]">
        <div
          className="h-full bg-[var(--portal-navy)]"
          style={{ width: `${percent}%` }}
        />
      </div>
      <span className="font-serif text-lg font-light text-[var(--portal-navy)]">
        {percent.toFixed(1)}%
      </span>
    </div>
  )
}

function SummaryStat({
  label,
  value,
  note,
}: {
  label: string
  value: string
  note?: string
}) {
  return (
    <div className="rounded-sm border border-[var(--portal-border)] bg-white p-5">
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
        {label}
      </div>
      <div className="mt-3 font-serif text-3xl font-light leading-none text-[var(--portal-navy)]">
        {value}
      </div>
      {note && (
        <div className="mt-2 text-xs font-light leading-5 text-black/40">
          {note}
        </div>
      )}
    </div>
  )
}

function SummaryStrip({ model }: { model: StoryBoardModel }) {
  return (
    <div className="mt-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-5">
      <div className="rounded-sm border border-[#c6a15b]/40 bg-white p-5">
        <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[#8a6d2f]">
          Net-Net Completion
        </div>
        <div className="mt-3 font-serif text-3xl font-light leading-none text-[var(--portal-navy)]">
          {model.netNet.toFixed(1)}%
        </div>
        <div className="mt-2 text-xs font-light leading-5 text-black/40">
          Σ (workstream completion × weight)
        </div>
      </div>
      <SummaryStat
        label="Authoritative stories"
        value={String(model.totalStories)}
        note="All stored stories"
      />
      <SummaryStat
        label="Complete"
        value={String(model.totalComplete)}
        note="Status Complete"
      />
      <SummaryStat
        label="In Progress / Partial"
        value={String(model.totalInProgressPartial)}
        note="Actively being worked"
      />
      <SummaryStat
        label="Blocked / Failed"
        value={String(model.totalBlockedFailed)}
        note="Requires attention"
      />
      <SummaryStat
        label="Ready for execution"
        value={String(model.totalReady)}
        note="Authorized for the coding agent"
      />
    </div>
  )
}

const SURFACE_JOB_NOTE: Record<string, string> = {
  NEXUS: 'do the real-estate work',
  OPS: 'administer the business',
  SUPPORT: 'operate & secure the product',
  TECH: 'build the platform',
}

/**
 * SB-01 — the four operating-surface completion projections. One canonical
 * board, additional projection: the same stored completion / AVG semantics as
 * the workstream dashboard, grouped by operating surface. Reference /
 * rollup=false rows never pollute the percentage; NULL rows are excluded.
 */
function SurfaceCompletionStrip({ model }: { model: StoryBoardModel }) {
  return (
    <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="border-b border-[var(--portal-border)] px-6 py-6">
        <h2 className="font-serif text-2xl font-light">
          Operating Surface Completion
        </h2>
        <p className="mt-1 text-sm font-light leading-6 text-black/50">
          Projection over the authoritative backlog, classified by primary job.
          Workstream grouping, weights and the Net-Net formula are unchanged.
        </p>
      </div>
      <div className="grid gap-4 p-6 sm:grid-cols-2 lg:grid-cols-4">
        {model.surfaceRollups.map((surface) => (
          <div
            key={surface.surface}
            className="rounded-sm border border-[var(--portal-border)] p-5"
          >
            <div className="flex items-baseline justify-between gap-2">
              <span className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
                {surface.surface}
              </span>
              <span className="text-[10px] font-light uppercase tracking-[0.14em] text-black/35">
                {surface.storyCount} in rollup
              </span>
            </div>
            <div className="mt-3 font-serif text-3xl font-light leading-none text-[var(--portal-navy)]">
              {surface.completionPercent.toFixed(1)}%
            </div>
            <div className="mt-2 text-xs font-light leading-5 text-black/40">
              {SURFACE_JOB_NOTE[surface.surface]} — {surface.storedCount} stored
            </div>
          </div>
        ))}
      </div>
      <div className="border-t border-[var(--portal-border)] px-6 py-4 text-xs font-light leading-5 text-black/40">
        {model.unclassifiedCount > 0 ? (
          <>
            {model.unclassifiedCount} stored story
            {model.unclassifiedCount === 1 ? ' is' : 's are'} deliberately
            unclassified (reference / undecided) and excluded from every surface
            projection.
          </>
        ) : (
          <>Every stored story is deliberately classified.</>
        )}
      </div>
    </section>
  )
}

function NextWorkSection({ selection }: { selection: NextWorkSelection }) {
  const { entries, totalEligible, totalBlockedByDependency, limit, truncated } =
    selection

  return (
    <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="border-b border-[var(--portal-border)] px-6 py-6">
        <div className="flex flex-wrap items-baseline justify-between gap-3">
          <h2 className="font-serif text-2xl font-light">Next Work</h2>
          <span className="rounded-full bg-[#c6a15b]/15 px-3 py-1 text-xs font-light uppercase tracking-[0.16em] text-[#8a6d2f]">
            Next {limit}
          </span>
        </div>
        <p className="mt-1 text-sm font-light leading-6 text-black/50">
          Bounded, deterministic work-selection projection (OPS-08) — no Jira.
          Actionable stories (rollup-participating, not Complete/Blocked/
          Failed/Deferred/Hold) ranked by batch → priority → planned start.
          {totalBlockedByDependency > 0 && (
            <>
              {" "}
              {totalBlockedByDependency} more wait on unmet dependencies.
            </>
          )}{" "}
          {totalEligible} eligible
          {truncated ? ` — capped at ${limit}.` : "."}
        </p>
      </div>

      {entries.length === 0 ? (
        <div className="px-6 py-10 text-center text-sm font-light italic text-black/40">
          No actionable next work right now — the board has no eligible
          stories.
        </div>
      ) : (
        <ol className="divide-y divide-[var(--portal-border)]">
          {entries.map(({ story, rank }) => (
            <li
              key={story.id}
              className="flex flex-wrap items-center gap-x-5 gap-y-2 px-6 py-4"
            >
              <span className="w-7 font-serif text-lg font-light leading-none text-black/30">
                {rank}
              </span>
              <div className="min-w-56 flex-1">
                <Link
                  href={`/portal/storyboard/${encodeURIComponent(story.id)}`}
                  className="font-mono text-xs text-[var(--portal-navy)] transition hover:text-[#8a4b2a]"
                >
                  {story.id}
                </Link>
                <div className="mt-0.5 text-sm font-light leading-5 text-black/70">
                  {story.title}
                </div>
              </div>
              <span className="text-xs font-light uppercase tracking-[0.14em] text-black/35">
                {story.workstream}
              </span>
              {story.batch !== null && (
                <span className="rounded-full border border-[#c6a15b]/40 px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.14em] text-[#8a6d2f]">
                  Batch {story.batch}
                </span>
              )}
              <span className="rounded-full border border-black/10 px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
                {story.priority}
              </span>
              <span
                className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${statusPillClasses(story.status)}`}
              >
                {story.status}
              </span>
            </li>
          ))}
        </ol>
      )}

      <div className="border-t border-[var(--portal-border)] px-6 py-4 text-xs font-light leading-5 text-black/40">
        Reference / parent rows (rollup = false) are never selected as work;
        stories referenced in a candidate's dependencies must be Complete
        before the candidate becomes eligible. The selection is derived at
        render time from the stored stories — open any entry for its full
        execution specification.
      </div>
    </section>
  )
}

function WorkstreamRow({ model }: { model: StoryBoardModel }) {
  return (
    <tbody>
      {model.workstreams.map((ws) => (
        <tr
          key={ws.code}
          className="border-b border-[var(--portal-border)] last:border-b-0"
        >
          <td className="px-6 py-4 align-top">
            <div className="text-sm font-light text-[var(--portal-navy)]">
              {ws.workstream}
            </div>
            <div className="mt-1 text-xs font-light text-black/40">
              {ws.code} · weight {ws.weight}%
            </div>
          </td>
          <td className="px-6 py-4 align-top">
            <div className="font-serif text-xl font-light text-[var(--portal-navy)]">
              {ws.storyCount}
            </div>
            <div className="mt-1 text-xs font-light text-black/40">
              {ws.storedCount} stored
            </div>
          </td>
          <td className="px-6 py-4 align-top font-light text-black/60">
            {ws.completeCount}
          </td>
          <td className="px-6 py-4 align-top font-light text-black/60">
            {ws.inProgressPartialCount}
          </td>
          <td className="px-6 py-4 align-top font-light text-black/60">
            {ws.blockedFailedCount}
          </td>
          <td className="px-6 py-4 align-top">
            <CompletionBar percent={ws.completionPercent} />
          </td>
        </tr>
      ))}
    </tbody>
  )
}

export function StoryBoard({
  model,
  filter,
  visibleStories,
  nextWork,
}: {
  model: StoryBoardModel
  filter: StoryBoardFilter
  visibleStories: StoryRecord[]
  nextWork: NextWorkSelection
}) {
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
          The authoritative CulebraLuxe development backlog. Story state,
          completion, architecture guidance, and execution history persist to
          Neon and drive the project rollup.
        </p>
      </header>

      <SummaryStrip model={model} />

      <SurfaceCompletionStrip model={model} />

      <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
        <div className="border-b border-[var(--portal-border)] px-6 py-6">
          <h2 className="font-serif text-2xl font-light">
            Executive Workstream Dashboard
          </h2>
          <p className="mt-1 text-sm font-light text-black/50">
            Rollup counts and completion derived from the stored stories.
            Parent stories (rollup = false) are stored but excluded from the
            counts; their children carry the rollup weight.
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
                  Stories
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Complete
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  In Progress / Partial
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Blocked / Failed
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Completion
                </th>
              </tr>
            </thead>
            <WorkstreamRow model={model} />
          </table>
        </div>
      </section>

      <NextWorkSection selection={nextWork} />

      <StoryBoardControls
        filter={filter}
        visibleCount={visibleStories.length}
        totalCount={model.totalStories}
      />

      <StoryBoardTable stories={visibleStories} totalStories={model.totalStories} />

      <footer className="mt-6 text-xs font-light leading-6 text-black/40">
        <p>
          Workstream completion is the average of the stored{" "}
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            completion
          </code>{" "}
          (0–100) over rollup-participating stories — status is categorical and
          does not drive the percentage. Net-Net = Σ (workstream completion ×
          weight) with weights Public Website 20, CRM Foundation 20, Portal 20,
          Transaction 15, Admin 10, Auth 5, Content 5, Hardening 5. Operating
          Surface Completion uses the same stored completion average over the
          rollup-participating stories classified to that surface (SB-01);
          reference / rollup=false rows never pollute the percentage and
          deliberately unclassified (NULL) stories are excluded from every
          surface projection. Each story carries its execution specification
          (goal, dependencies, preconditions, architect brief, context refs,
          acceptance criteria, postconditions); runs snapshot that specification
          when they start (migration 024). Open any story to inspect the full
          detail and execution history, or filter/search the list above. No
          workflow_engine changes.
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
          the reviewed migrations (
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            db/migrations/021_storyboard_story.sql
          </code>{" "}
          and{" "}
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            db/migrations/022_storyboard_authoritative_seed.sql
          </code>
          ) to enable the board and seed the authoritative master stories.
        </p>
      </section>
    </div>
  )
}
