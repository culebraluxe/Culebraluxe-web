import Link from "next/link"
import { redirect } from "next/navigation"

import { createAuthJsSessionAdapter } from "@/lib/auth/authjs-session-adapter"
import { resolvePortalAccess } from "@/lib/auth/require-portal-access"
import { listStoryboardRuns, listStoryboardStories } from "@/db/storyboard"

export const dynamic = "force-dynamic"

// ---------------------------------------------------------------------------
// RUN OUTCOMES — why did it stop?
//
// The observer records every role attempt durably, and the runner appends the
// reason a HOLD was thrown to `storyboard_story_run.evidence_detail`. Until now
// that record was write-only: answering "why did lead_pre stop?" meant reading a
// log file on the machine that ran it. This page is the read side — newest runs
// first, with the REASON on the card instead of buried in prose.
// ---------------------------------------------------------------------------

const RUN_LIMIT = 40

function asTime(value: unknown): string {
  if (value instanceof Date) return value.toLocaleString()
  if (typeof value === "string") return new Date(value).toLocaleString()
  return ""
}

/** The most recent detail line is the reason the run stopped (the runner appends
 * timestamped lines, so the last one is the latest fact). */
function lastDetailLine(detail: string | null | undefined): string | null {
  const text = (detail ?? "").trim()
  if (!text) return null
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
  return lines.length ? lines[lines.length - 1] : null
}

function statusTone(status: string | null, ended: boolean): string {
  const value = (status ?? "").toLowerCase()
  if (!ended) return "bg-[var(--portal-navy)]/8 text-[var(--portal-navy)]"
  if (value.includes("hold") || value.includes("fail") || value.includes("error")) {
    return "bg-red-500/10 text-red-700"
  }
  if (value.includes("complete") || value.includes("pass") || value.includes("success")) {
    return "bg-emerald-500/10 text-emerald-700"
  }
  return "bg-black/5 text-black/55"
}

export default async function TechRunsPage() {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), "tech.access")
  if (!access.ok) redirect(access.redirectTo)

  const [runs, stories] = await Promise.all([listStoryboardRuns(), listStoryboardStories()])
  const titles = new Map((stories ?? []).map((story) => [story.id, story.title] as const))
  const recent = (runs ?? []).slice(0, RUN_LIMIT)

  return (
    <div className="mx-auto max-w-5xl space-y-5 p-4 lg:p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="font-serif text-2xl font-light text-[var(--portal-navy)]">Run outcomes</h1>
          <p className="mt-1 max-w-2xl text-sm font-light leading-6 text-black/55">
            The newest {RUN_LIMIT} engine runs, with the reason each one stopped. The last
            line of a run&apos;s evidence detail is the fact that ended it — a HOLD, a scope
            miss, a failed gate — so a stop is diagnosable here instead of from a log file.
          </p>
        </div>
        <Link
          href="/portal/tech"
          className="text-xs font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)]/60 hover:text-[var(--portal-navy)]"
        >
          ← Engineering
        </Link>
      </div>

      {recent.length === 0 ? (
        <p className="rounded border border-black/10 bg-white/50 p-6 text-sm font-light text-black/55">
          No runs recorded yet.
        </p>
      ) : (
        <ul className="space-y-3">
          {recent.map((run) => {
            const ended = Boolean(run.endedAt)
            const reason = lastDetailLine(run.evidenceDetail)
            const title = titles.get(run.storyId)
            return (
              <li
                key={run.id}
                className="rounded border border-black/10 bg-white/60 p-4 shadow-[0_1px_2px_rgba(3,15,35,0.04)]"
              >
                <div className="flex flex-wrap items-baseline justify-between gap-2">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium text-[var(--portal-navy)]">{run.storyId}</span>
                      <span
                        className={`rounded px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] ${statusTone(run.resultStatus, ended)}`}
                      >
                        {ended ? (run.resultStatus ?? "ended") : "running"}
                      </span>
                      {run.runPhase ? (
                        <span className="text-[11px] uppercase tracking-[0.12em] text-black/40">
                          {run.runPhase}
                        </span>
                      ) : null}
                    </div>
                    {title ? (
                      <p className="mt-0.5 truncate text-sm font-light text-black/60">{title}</p>
                    ) : null}
                  </div>
                  <div className="text-right text-[11px] text-black/45">
                    <div>{asTime(run.startedAt)}</div>
                    <div>
                      completion {run.completion ?? 0}%
                      {run.failureCode ? ` · ${run.failureCode}` : ""}
                    </div>
                  </div>
                </div>

                {reason ? (
                  <p className="mt-3 rounded bg-[var(--portal-navy)]/[0.04] px-3 py-2 font-mono text-[11px] leading-5 text-[var(--portal-navy)]/85">
                    {reason}
                  </p>
                ) : (
                  <p className="mt-3 text-[11px] text-black/40">No detail recorded for this run.</p>
                )}

                {run.evidenceDetail ? (
                  <details className="mt-2">
                    <summary className="cursor-pointer text-[11px] uppercase tracking-[0.12em] text-black/40 hover:text-black/60">
                      full detail
                    </summary>
                    <pre className="mt-2 max-h-64 overflow-auto whitespace-pre-wrap rounded bg-black/[0.03] p-3 font-mono text-[11px] leading-5 text-black/70">
                      {run.evidenceDetail}
                    </pre>
                  </details>
                ) : null}
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
