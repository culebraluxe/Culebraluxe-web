import { type StoryBoardModel } from "@/lib/storyboard-data"
import type { StoryRun } from "@/db/storyboard"
import { StoryBoardTable } from "@/components/portal/write/story-board-table"

// ---------------------------------------------------------------------------
// Story Board — server view. Rollup counts and the Net-Net completion are
// computed from persisted stories by lib/storyboard-data.ts; the editable
// story table is a client component (create / edit / change status). No
// workflow_engine involvement.
// ---------------------------------------------------------------------------

function NetNetCard({ netNet }: { netNet: number }) {
  return (
    <div className="rounded-sm border border-[var(--portal-border)] bg-white p-6">
      <div className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">
        Net-Net Completion
      </div>
      <div className="mt-4 font-serif text-5xl font-light leading-none text-[var(--portal-navy)]">
        {netNet.toFixed(1)}%
      </div>
      <div className="mt-3 text-xs font-light leading-5 text-black/40">
        Weighted across the 8 workstreams — Σ (workstream completion ×
        weight) — computed from the stored Story Board state.
      </div>
    </div>
  )
}

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
            {ws.partialCount}
          </td>
          <td className="px-6 py-4 align-top font-light text-black/60">
            {ws.openCount}
          </td>
          <td className="px-6 py-4 align-top font-light text-black/60">
            {ws.blockedCount}
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
  runs,
}: {
  model: StoryBoardModel
  runs: StoryRun[]
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
          The authoritative CulebraLuxe master backlog — the human-authored
          8/21 board. Create, edit, and change status here; changes persist to
          Neon and drive the rollup. No repository-derived stories, no new
          items, no implementation.
        </p>
      </header>

      <NetNetCard netNet={model.netNet} />

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
                  Partial
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Open
                </th>
                <th className="px-6 py-4 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                  Blocked
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


      <StoryBoardTable stories={model.stories} runs={runs} />

      <footer className="mt-6 text-xs font-light leading-6 text-black/40">
        <p>
          Workstream completion is the average of the stored{" "}
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            completion
          </code>{" "}
          (0–100) over rollup-participating stories — status is categorical and
          does not drive the percentage. Net-Net = Σ (workstream completion ×
          weight) with weights Public Website 20, CRM Foundation 20, Portal 20,
          Transaction 15, Admin 10, Auth 5, Content 5, Hardening 5. Status
          buckets (Complete / In Progress + Partial / Planned + Deferred + Hold
          + Failed / Blocked) feed the count columns only. Execution runs live
          in{" "}
          <code className="rounded bg-black/5 px-1.5 py-0.5 text-xs">
            storyboard_story_run
          </code>{" "}
          (migration 023). No workflow_engine changes.
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
