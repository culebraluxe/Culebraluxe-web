import {
  type NextWorkSelection,
  type StoryBoardFilter,
  type StoryBoardModel,
  type StoryDomainRollup,
  type StoryDomainSubgroup,
  type StoryRecord,
  isExecutionActive,
  isExecutionError,
} from "@/lib/storyboard-data"
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

function ExecutionBadge({ story }: { story: StoryRecord }) {
  const exec = story.execution ?? null
  const state = exec?.workItemState ?? null
  const result = exec?.latestRunResult ?? null
  const base =
    "inline-block whitespace-nowrap rounded-full px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.14em]"
  if (isExecutionActive(state)) {
    return (
      <span className={`${base} bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]`}>
        {state}
      </span>
    )
  }
  if (isExecutionError({ workItemState: state, latestRunResult: result })) {
    return (
      <span className={`${base} bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]`}>
        {state ?? result}
      </span>
    )
  }
  if (state === 'Done') {
    return (
      <span className={`${base} bg-[var(--portal-success-pale)] text-[var(--portal-success)]`}>
        Done
      </span>
    )
  }
  if (state) {
    return (
      <span className={`${base} bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]`}>
        {state}
      </span>
    )
  }
  if (result) {
    return (
      <span className={`${base} bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]`}>
        {result}
      </span>
    )
  }
  return (
    <span className={`${base} border border-black/10 text-black/35`}>
      Not run
    </span>
  )
}

function MiniCompletion({ percent }: { percent: number }) {
  return (
    <div className="mt-3">
      <div className="h-1.5 w-full overflow-hidden bg-[var(--portal-blue-pale)]">
        <div
          className="h-full bg-[var(--portal-navy)]"
          style={{ width: `${percent}%` }}
        />
      </div>
      <div className="mt-1 text-[11px] font-light tabular-nums text-black/45">
        {percent.toFixed(1)}% complete
      </div>
    </div>
  )
}

function DomainCard({ domain }: { domain: StoryDomainRollup }) {
  return (
    <div className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white p-5 shadow-[var(--portal-panel-shadow)]">
      <div className="flex items-baseline justify-between gap-2">
        <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
          {domain.domain}
        </div>
        <div className="font-serif text-2xl font-light leading-none text-[var(--portal-navy)]">
          {domain.storyCount}
        </div>
      </div>
      <MiniCompletion percent={domain.completionPercent} />
      <div className="mt-3 grid grid-cols-2 gap-x-3 gap-y-1 text-[11px] font-light text-black/50">
        <span>{domain.completeCount} complete</span>
        <span>{domain.inProgressPartialCount} active</span>
        <span>{domain.blockedFailedCount} blocked/failed</span>
        <span>{domain.runningCount} running</span>
      </div>
    </div>
  )
}

function SummaryStrip({ model }: { model: StoryBoardModel }) {
  return (
    <div className="mt-6 space-y-4">
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
        <div className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-feature-border)] [background:var(--portal-feature-gradient)] p-5 text-white shadow-[var(--portal-feature-shadow)]">
          <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-feature-eyebrow)]">
            Net-Net Completion
          </div>
          <div className="mt-2 font-serif text-3xl font-light leading-none">
            {model.netNet.toFixed(1)}%
          </div>
          <div className="mt-2 text-xs font-light leading-5 text-white/55">
            Simple mean of the five domain completion percentages — no legacy
            weight table.
          </div>
        </div>
        <div className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white p-5 shadow-[var(--portal-panel-shadow)]">
          <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
            Authoritative stories
          </div>
          <div className="mt-2 font-serif text-3xl font-light leading-none text-[var(--portal-navy)]">
            {model.totalStories}
          </div>
          <div className="mt-2 text-xs font-light leading-5 text-black/40">
            {model.unclassifiedCount > 0
              ? `${model.unclassifiedCount} awaiting human domain classification`
              : "All stories classified across the five domains"}
          </div>
        </div>
        <div className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white p-5 shadow-[var(--portal-panel-shadow)]">
          <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
            Forge execution
          </div>
          <div className="mt-2 font-serif text-3xl font-light leading-none text-[var(--portal-navy)]">
            {model.totalRunning}
          </div>
          <div className="mt-2 text-xs font-light leading-5 text-black/40">
            running · {model.totalError} error/cancelled · {model.totalReady}{" "}
            ready
          </div>
        </div>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-5">
        {model.domains.map((d) => (
          <DomainCard key={d.domain} domain={d} />
        ))}
      </div>
    </div>
  )
}

/**
 * OPS-08 — Next Work projection. A bounded, deterministic slice of actionable
 * work derived from the authoritative stored stories ("Next 20 without
 * building Jira"). Pure projection: no assignments, no sprints, no writes.
 */
function NextWorkSection({ selection }: { selection: NextWorkSelection }) {
  const { entries, totalEligible, totalBlockedByDependency, limit, truncated } =
    selection

  return (
    <section className="mt-6 rounded-sm border border-[var(--portal-border)] bg-white">
      <div className="border-b border-[var(--portal-border)] px-6 py-6">
        <div className="flex flex-wrap items-baseline justify-between gap-3">
          <h2 className="font-serif text-2xl font-light">Next Work</h2>
          <span className="rounded-full bg-[var(--portal-gold-pale)] px-3 py-1 text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">
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
                  className="font-mono text-xs text-[var(--portal-navy)] transition hover:text-[var(--portal-archive)]"
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
                <span className="rounded-full border border-[var(--portal-gold)]/40 px-2.5 py-1 text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">
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

function SubgroupSection({
  subgroup,
}: {
  subgroup: StoryDomainSubgroup
}) {
  return (
    <div className="border-b border-[var(--portal-border)] px-6 py-5 last:border-b-0">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h3 className="text-xs font-light uppercase tracking-[0.2em] text-[var(--portal-navy-soft)]">
          {subgroup.subgroup}
        </h3>
        <div className="flex items-center gap-4 text-[11px] font-light text-black/45">
          <span>{subgroup.storyCount} stories</span>
          <span>{subgroup.completeCount} complete</span>
          <span>{subgroup.completionPercent.toFixed(1)}%</span>
        </div>
      </div>

      {subgroup.stories.length === 0 ? (
        <p className="mt-3 text-xs font-light italic text-black/35">
          No stories in this subgroup yet.
        </p>
      ) : (
        <div className="mt-3 overflow-x-auto">
          <table className="w-full min-w-[760px] border-collapse text-left text-sm">
            <thead>
              <tr className="border-b border-[var(--portal-border)]">
                <th className="px-4 py-2 text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
                  Story
                </th>
                <th className="px-4 py-2 text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
                  Status
                </th>
                <th className="px-4 py-2 text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
                  Execution
                </th>
                <th className="px-4 py-2 text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
                  Latest run
                </th>
                <th className="px-4 py-2 text-[10px] font-light uppercase tracking-[0.18em] text-black/40">
                  Completion
                </th>
              </tr>
            </thead>
            <tbody>
              {subgroup.stories.map((story) => (
                <tr
                  key={story.id}
                  className="border-b border-[var(--portal-border)] transition-colors last:border-b-0 hover:bg-[var(--portal-blue-pale)]/50"
                >
                  <td className="px-4 py-3">
                    <Link
                      href={`/portal/storyboard/${encodeURIComponent(story.id)}`}
                      className="font-mono text-xs text-[var(--portal-navy)] transition hover:text-[var(--portal-archive)]"
                    >
                      {story.id}
                    </Link>
                    <div className="mt-0.5 max-w-[280px] truncate text-sm font-light text-black/65">
                      {story.title}
                    </div>
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${statusPillClasses(story.status)}`}
                    >
                      {story.status}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <ExecutionBadge story={story} />
                  </td>
                  <td className="px-4 py-3 text-xs font-light text-black/45">
                    {story.execution?.latestRunAt
                      ? new Date(story.execution.latestRunAt).toLocaleDateString()
                      : "—"}
                  </td>
                  <td className="px-4 py-3 font-serif text-base font-light text-[var(--portal-navy)]">
                    {story.completion}%
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

function DomainRollupSection({ domain }: { domain: StoryDomainRollup }) {
  return (
    <section className="overflow-hidden rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white shadow-[var(--portal-panel-shadow)]">
      <div className="flex flex-wrap items-baseline justify-between gap-3 border-b border-[var(--portal-panel-border)] px-6 py-5">
        <div>
          <h2 className="font-serif text-2xl font-light text-[var(--portal-navy)]">
            {domain.domain}
          </h2>
          <p className="mt-1 text-xs font-light text-black/45">{domain.label}</p>
        </div>
        <div className="flex flex-wrap items-center gap-4 text-[11px] font-light text-black/50">
          <span>{domain.storyCount} stories</span>
          <span>{domain.completeCount} complete</span>
          <span>{domain.inProgressPartialCount} active</span>
          <span>{domain.blockedFailedCount} blocked/failed</span>
          <span>{domain.runningCount} running</span>
          <span className="font-serif text-lg font-light text-[var(--portal-navy)]">
            {domain.completionPercent.toFixed(1)}%
          </span>
        </div>
      </div>

      {domain.subgroups.map((sg) => (
        <SubgroupSection key={sg.subgroup} subgroup={sg} />
      ))}
    </section>
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
          <span className="rounded-full bg-[var(--portal-gold-pale)] px-3 py-1 text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">
            Editable
          </span>
        </div>

        <p className="mt-3 max-w-3xl text-sm font-light leading-6 text-black/50">
          The authoritative CulebraLuxe development backlog, rolled up by the
          five operating domains (NEXUS / MAIN / OPPS / SUPPORT / TECH). Story
          status, Forge execution state, and stored completion stay distinct.
        </p>
      </header>

      <SummaryStrip model={model} />

      <section className="mt-8 space-y-6">
        {model.domains.map((d) => (
          <DomainRollupSection key={d.domain} domain={d} />
        ))}
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
          The board rolls up by the five operating domains — NEXUS, MAIN, OPPS,
          SUPPORT, TECH — with subgroup sections beneath each parent.{" "}
          <span className="text-black/55">Rollup formula</span>: domain /
          subgroup completion is the average of the stored{" "}
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            completion
          </code>{" "}
          (0–100) over rollup-participating stories (rollup = false parents are
          counted but carry no completion weight). Net-Net = the simple mean of
          the five domain completion percentages — the legacy workstream weight
          table is gone. Story status (categorical), Forge execution state
          (latest work item / latest run), and completion (stored 0–100) are
          kept distinct. Domain classification is deterministic from story
          prefix / workstream / operating surface; unclassified stories are
          reported explicitly. Open any story for the full detail, execution
          specification, and run history, or filter/search the editable list
          below.
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
          <span className="rounded-full bg-[var(--portal-gold-pale)] px-3 py-1 text-xs font-light uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">
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
