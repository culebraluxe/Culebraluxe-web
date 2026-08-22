"use client"

import { useState } from "react"
import Link from "next/link"
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  Clock,
  FileText,
  Gauge,
  History,
  Server,
  ShieldAlert,
  TerminalSquare,
  XCircle,
} from "lucide-react"

import type { StoryExecutionCockpit } from "@/lib/command-console-data"
import type { AgentWorkItem } from "@/db/agent-work"
import type { StoryRun } from "@/db/storyboard"
import { formatTime, runResultPill, shortId, statePill } from "@/lib/command-console-ui"

// ---------------------------------------------------------------------------
// Story Execution Cockpit (ENG-20) — a functional per-story detail projection
// over REAL control-plane data. Read-only: it never writes, never queues, and
// derives NO new lifecycle/state machine. Dark/navy styling consistent with
// the SDLC Command Console. No visual-polish scope tonight.
// ---------------------------------------------------------------------------

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="text-[10px] font-light uppercase tracking-[0.16em] text-slate-500">{label}</dt>
      <dd className="mt-0.5 break-words text-[13px] font-light leading-5 text-slate-200">{children}</dd>
    </div>
  )
}

function Section({
  title,
  icon,
  children,
}: {
  title: string
  icon: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <section className="rounded-sm border border-white/10 bg-[#0d1424]">
      <header className="flex items-center gap-2 border-b border-white/10 px-4 py-3">
        <span className="text-[#c6a15b]">{icon}</span>
        <h2 className="text-[11px] font-light uppercase tracking-[0.2em] text-slate-400">{title}</h2>
      </header>
      <div className="px-4 py-4">{children}</div>
    </section>
  )
}

function MonoSpan({ children, title }: { children: React.ReactNode; title?: string }) {
  return (
    <span title={title} className="font-mono text-[11px] text-[#e3c98a]">
      {children}
    </span>
  )
}

function WorkItemGrid({ item }: { item: AgentWorkItem }) {
  const state = statePill(item.state)
  const terminal = item.state === "Error" || item.state === "Cancelled"
  return (
    <dl className="grid gap-x-6 gap-y-3 sm:grid-cols-2 lg:grid-cols-3">
      <Row label="State">
        <span className={`inline-block rounded-full border px-2 py-0.5 text-[10px] font-light ${state.cls}`}>
          {state.label}
        </span>
      </Row>
      <Row label="Worker">{item.claimedBy ?? "—"}</Row>
      <Row label="Role">{item.role ?? "—"}</Row>
      <Row label="Model profile (logical)">{item.modelProfile ?? "—"}</Row>
      <Row label="Runtime adapter">{item.runtimeAdapter ?? "—"}</Row>
      <Row label="Execution target">
        <span className="inline-flex items-center gap-1.5">
          <Server className="h-3 w-3 text-slate-500" />
          <span className="font-mono text-xs text-[#e3c98a]">{item.executionEnvironment ?? "unset"}</span>
        </span>
      </Row>
      <Row label="Claimed at">{formatTime(item.claimedAt)}</Row>
      <Row label="Started">{formatTime(item.startedAt)}</Row>
      <Row label="Heartbeat / last activity">{formatTime(item.updatedAt)}</Row>
      <Row label="Attempts">
        <span className="tabular-nums">
          {item.attempts}/{item.maxAttempts}
        </span>
      </Row>
      <Row label="External session / run correlation">
        {item.externalRunId ? <MonoSpan title={item.externalRunId}>{shortId(item.externalRunId)}</MonoSpan> : "—"}
      </Row>
      <Row label="Execution policy">{item.executionPolicy ?? "—"}</Row>
      {item.errorText && (
        <div className="sm:col-span-2 lg:col-span-3">
          <Row label={terminal ? "Terminal error" : "Last error"}>
            <span className="inline-block whitespace-pre-wrap rounded-sm border border-red-400/30 bg-red-400/10 px-2 py-1 font-mono text-[11px] text-red-300">
              {item.errorText}
            </span>
          </Row>
        </div>
      )}
    </dl>
  )
}

function RunSummary({ run }: { run: StoryRun }) {
  const pill = runResultPill(run.resultStatus)
  const pillCls = statePill(pill).cls
  return (
    <dl className="grid gap-x-6 gap-y-3 sm:grid-cols-2 lg:grid-cols-3">
      <Row label="Run id">
        <MonoSpan title={run.id}>{shortId(run.id)}</MonoSpan>
      </Row>
      <Row label="Result status">
        <span className={`inline-block rounded-full border px-2 py-0.5 text-[10px] font-light ${pillCls}`}>
          {pill}
        </span>
      </Row>
      <Row label="Completion">
        <span className="tabular-nums">{run.completion != null ? `${run.completion}%` : "—"}</span>
      </Row>
      <Row label="Started">{formatTime(run.startedAt)}</Row>
      <Row label="Ended">{formatTime(run.endedAt)}</Row>
      <Row label="Execution target">
        <span className="font-mono text-xs text-[#e3c98a]">{run.executionEnvironment ?? "—"}</span>
      </Row>
      <Row label="Tests">{run.testsSummary ?? "—"}</Row>
      <Row label="Commit hash">
        {run.commitHash ? <MonoSpan title={run.commitHash}>{shortId(run.commitHash)}</MonoSpan> : "—"}
      </Row>
    </dl>
  )
}


// ---------------------------------------------------------------------------
// Lifecycle projection (READ-ONLY derivation — no new state machine):
//   READY → CLAIMED → RUNNING → DONE | ERROR | CANCELLED
// ---------------------------------------------------------------------------
function LifecycleProjection({ item }: { item: AgentWorkItem | null }) {
  const state = item?.state ?? null
  const activeIndex =
    state === "Ready"
      ? 0
      : state === "Claimed"
        ? 1
        : state === "Running" || state === "Paused"
          ? 2
          : state === "Done" || state === "Error" || state === "Cancelled"
            ? 3
            : -1
  const terminal = state === "Done" || state === "Error" || state === "Cancelled"
  const errorish = state === "Error" || state === "Cancelled"

  const steps = ["READY", "CLAIMED", "RUNNING"]
  const terminalLabel =
    state === "Done" ? "DONE" : state === "Error" ? "ERROR" : state === "Cancelled" ? "CANCELLED" : "DONE/ERR/CANCEL"

  return (
    <div className="flex flex-wrap items-center gap-2">
      {steps.map((label, i) => {
        const done = activeIndex > i
        const active = activeIndex === i && !terminal
        return (
          <div key={label} className="flex items-center gap-2">
            <span
              className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-[11px] font-light tracking-wide ${
                active
                  ? "border-emerald-400/50 bg-emerald-400/15 text-emerald-300"
                  : done
                    ? "border-emerald-400/30 bg-emerald-400/5 text-emerald-400/70"
                    : "border-white/10 text-slate-500"
              }`}
            >
              {done ? <CheckCircle2 className="h-3 w-3" /> : <span className="h-1.5 w-1.5 rounded-full bg-current" />}
              {label}
            </span>
            {i < steps.length - 1 && <ArrowRight className="h-3 w-3 text-slate-600" />}
          </div>
        )
      })}
      <ArrowRight className="h-3 w-3 text-slate-600" />
      <span
        className={`inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-[11px] font-light tracking-wide ${
          activeIndex === 3
            ? errorish
              ? "border-red-400/50 bg-red-400/15 text-red-300"
              : "border-emerald-400/50 bg-emerald-400/15 text-emerald-300"
            : "border-white/10 text-slate-500"
        }`}
      >
        {activeIndex === 3 ? (
          errorish ? (
            <XCircle className="h-3 w-3" />
          ) : (
            <CheckCircle2 className="h-3 w-3" />
          )
        ) : (
          <span className="h-1.5 w-1.5 rounded-full bg-current" />
        )}
        {terminalLabel}
      </span>
      {state === "Paused" && (
        <span className="inline-flex items-center gap-1.5 rounded-full border border-amber-400/40 bg-amber-400/10 px-3 py-1 text-[11px] font-light tracking-wide text-amber-300">
          PAUSED
        </span>
      )}
    </div>
  )
}

function latestStep(run: StoryRun | null): string {
  // The run narrative is the persisted heartbeat trail (timestamped lines).
  // "Current step" is derived only when available — never invented.
  if (!run?.notes) return "—"
  const lines = run.notes
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
  if (lines.length === 0) return "—"
  return lines[lines.length - 1].slice(0, 160)
}

export function StoryExecutionCockpit({ model }: { model: StoryExecutionCockpit }) {
  const story = model.story
  if (!story) return null

  const latestWork = model.workItems[0] ?? null
  const latestRun = model.runs[0] ?? null
  const storyPill = statePill(story.status)
  const blocked =
    story.status === "Blocked" ||
    story.status === "Failed" ||
    latestWork?.state === "Error" ||
    latestWork?.state === "Cancelled"
  const executionPolicy = latestWork?.executionPolicy ?? null
  const currentStep = latestRun ? latestStep(latestRun) : "—"
  const hasBrief = Boolean(story.architectBrief && story.architectBrief.length > 0)
  const hasAcceptance = Boolean(story.acceptanceCriteria && story.acceptanceCriteria.length > 0)

  return (
    <div className="min-h-screen bg-[#0a0f1a] text-slate-200">
      <header className="border-b border-white/10 bg-[#0d1424]">
        <div className="mx-auto flex max-w-[1600px] flex-wrap items-center gap-x-6 gap-y-3 px-5 py-4">
          <div className="flex items-center gap-3">
            <Gauge className="h-5 w-5 text-[#c6a15b]" />
            <h1 className="font-serif text-xl font-light uppercase tracking-[0.14em] text-white">
              Story Execution Cockpit
            </h1>
          </div>
          <Link
            href="/portal/command-console"
            className="rounded-sm border border-white/10 px-3 py-1.5 text-xs font-light text-slate-400 transition hover:border-[#c6a15b]/50 hover:text-[#e3c98a]"
          >
            ← Command Console
          </Link>
          <div className="ml-auto flex items-center gap-3 text-xs font-light text-slate-400">
            <Activity className="h-3.5 w-3.5 text-slate-500" />
            <span className="tabular-nums">{model.workItems.length}</span> command(s) ·{" "}
            <span className="tabular-nums">{model.runs.length}</span> run(s)
          </div>
        </div>
      </header>

      <div className="mx-auto max-w-[1600px] space-y-5 px-5 py-5">
        {/* Story */}
        <Section title="Story" icon={<FileText className="h-3.5 w-3.5" />}>
          <div className="flex flex-wrap items-center gap-3">
            <MonoSpan>{story.id}</MonoSpan>
            <span className={`rounded-full border px-2 py-0.5 text-[10px] font-light ${storyPill.cls}`}>
              {storyPill.label}
            </span>
            <span className="rounded-full border border-white/10 px-2 py-0.5 text-[10px] font-light uppercase tracking-wide text-slate-500">
              {story.priority}
            </span>
            <span className="rounded-full border border-white/10 px-2 py-0.5 text-[10px] font-light tabular-nums text-slate-500">
              {story.completion}%
            </span>
          </div>
          <h2 className="mt-2 font-serif text-2xl font-light text-white">{story.title}</h2>
          <dl className="mt-4 grid gap-x-6 gap-y-3 sm:grid-cols-2 lg:grid-cols-3">
            <Row label="Goal">
              <span className="line-clamp-4">{story.goal ?? "—"}</span>
            </Row>
            <Row label="Architect brief">
              {hasBrief ? (
                <span className="inline-flex items-center gap-1 text-emerald-300">
                  <CheckCircle2 className="h-3 w-3" /> available
                </span>
              ) : (
                "—"
              )}
            </Row>
            <Row label="Acceptance criteria">
              {hasAcceptance ? (
                <span className="inline-flex items-center gap-1 text-emerald-300">
                  <CheckCircle2 className="h-3 w-3" /> available
                </span>
              ) : (
                "—"
              )}
            </Row>
            <Row label="Dependencies">{story.dependencies ?? "—"}</Row>
            <Row label="Execution policy">{executionPolicy ?? "—"}</Row>
            <Row label="Postconditions">{story.postconditions ?? "—"}</Row>
          </dl>
          {hasAcceptance && (
            <div className="mt-3">
              <p className="text-[10px] font-light uppercase tracking-[0.16em] text-slate-500">Acceptance criteria</p>
              <p className="mt-1 whitespace-pre-wrap rounded-sm border border-white/10 bg-[#0a0f1a] p-3 text-xs font-light leading-5 text-slate-300">
                {story.acceptanceCriteria}
              </p>
            </div>
          )}
        </Section>

        {/* Lifecycle projection */}
        <Section title="Lifecycle projection (read-only)" icon={<History className="h-3.5 w-3.5" />}>
          <LifecycleProjection item={latestWork} />
          {latestWork && (
            <div className="mt-3 flex flex-wrap items-center gap-x-6 gap-y-1 text-[11px] font-light text-slate-500">
              <span>
                current step: <span className="text-slate-300">{currentStep}</span>
              </span>
              <span>
                heartbeat: <span className="tabular-nums text-slate-300">{formatTime(latestWork.updatedAt)}</span>
              </span>
            </div>
          )}
        </Section>

        {/* Blocker / escalation */}
        {blocked && (
          <Section title="Blocker / escalation" icon={<ShieldAlert className="h-3.5 w-3.5" />}>
            <div className="rounded-sm border border-red-400/30 bg-red-400/10 p-3">
              <p className="flex items-center gap-2 text-sm font-light text-red-300">
                <AlertTriangle className="h-4 w-4" />
                This story is in a blocked / failed / cancelled state.
              </p>
              <p className="mt-2 text-xs font-light leading-5 text-slate-300">
                {latestWork?.errorText
                  ? `Last error: ${latestWork.errorText}`
                  : latestWork?.state === "Cancelled"
                    ? "The run was cancelled by an operator; the story was set to Hold."
                    : `Story status is ${story.status}.`}
              </p>
              <p className="mt-2 text-xs font-light leading-5 text-amber-200/90">
                Recommended human action: review the run notes and error below, resolve the blocker, then set the story
                back to Ready to re-queue deliberate retry.
              </p>
            </div>
          </Section>
        )}

        {/* Current / latest work item */}
        <Section title="Current / latest work item" icon={<TerminalSquare className="h-3.5 w-3.5" />}>
          {latestWork ? (
            <WorkItemGrid item={latestWork} />
          ) : (
            <p className="py-4 text-center text-sm font-light text-slate-600">
              No active execution. This story has no work item yet — no active execution renders cleanly.
            </p>
          )}
        </Section>

        {/* Current / latest run */}
        <Section title="Current / latest run" icon={<Activity className="h-3.5 w-3.5" />}>
          {latestRun ? (
            <div className="space-y-3">
              <RunSummary run={latestRun} />
              <div>
                <p className="mb-1 text-[10px] font-light uppercase tracking-[0.16em] text-slate-500">
                  Evidence / notes
                </p>
                <p className="whitespace-pre-wrap rounded-sm border border-white/10 bg-[#0a0f1a] p-3 text-xs font-light leading-5 text-slate-300">
                  {latestRun.notes ?? "—"}
                </p>
              </div>
            </div>
          ) : (
            <p className="py-4 text-center text-sm font-light text-slate-600">No run evidence recorded for this story yet.</p>
          )}
        </Section>


        {/* Run history */}
        <Section title="Run history (newest first)" icon={<History className="h-3.5 w-3.5" />}>
          {model.runs.length === 0 ? (
            <p className="py-4 text-center text-sm font-light text-slate-600">No prior runs for this story.</p>
          ) : (
            <ul className="space-y-2">
              {model.runs.map((run) => (
                <RunHistoryRow key={run.id} run={run} />
              ))}
            </ul>
          )}
        </Section>
      </div>
    </div>
  )
}

function RunHistoryRow({ run }: { run: StoryRun }) {
  const [open, setOpen] = useState(false)
  const pill = runResultPill(run.resultStatus)
  return (
    <li className="rounded-sm border border-white/10 bg-[#0a0f1a]">
      <button
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between gap-4 px-4 py-3 text-left transition-colors hover:bg-white/5"
      >
        <span className="flex items-center gap-2">
          <span className={`rounded-full border px-2 py-0.5 text-[10px] font-light ${statePill(pill).cls}`}>{pill}</span>
          <MonoSpan title={run.id}>{shortId(run.id)}</MonoSpan>
          {run.executionEnvironment && (
            <span className="rounded-full border border-white/10 px-2 py-0.5 font-mono text-[10px] text-slate-500">
              {run.executionEnvironment}
            </span>
          )}
          {run.commitHash && <MonoSpan title={run.commitHash}>@{shortId(run.commitHash)}</MonoSpan>}
        </span>
        <span className="flex items-center gap-3 text-[11px] font-light tabular-nums text-slate-500">
          <span className="hidden sm:inline">
            {formatTime(run.startedAt)} → {formatTime(run.endedAt)}
          </span>
          <span className="text-slate-400">{open ? "collapse" : "expand"}</span>
        </span>
      </button>
      {open && (
        <div className="border-t border-white/10 px-4 py-3">
          <dl className="grid gap-x-6 gap-y-2 sm:grid-cols-2 lg:grid-cols-3">
            <Row label="Result">{run.resultStatus ?? "—"}</Row>
            <Row label="Completion">{run.completion != null ? `${run.completion}%` : "—"}</Row>
            <Row label="Tests">{run.testsSummary ?? "—"}</Row>
            <Row label="Started">{formatTime(run.startedAt)}</Row>
            <Row label="Ended">{formatTime(run.endedAt)}</Row>
            <Row label="Commit hash">{run.commitHash ? <MonoSpan title={run.commitHash}>{shortId(run.commitHash)}</MonoSpan> : "—"}</Row>
          </dl>
          <div className="mt-3">
            <p className="mb-1 text-[10px] font-light uppercase tracking-[0.16em] text-slate-500">Evidence / notes</p>
            <p className="whitespace-pre-wrap rounded-sm border border-white/10 bg-[#0a0f1a] p-3 text-xs font-light leading-5 text-slate-300">
              {run.notes ?? "—"}
            </p>
          </div>
        </div>
      )}
    </li>
  )
}

