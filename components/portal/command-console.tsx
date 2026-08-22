"use client"

import { useMemo, useState } from "react"
import Link from "next/link"
import { useRouter } from "next/navigation"
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  Clock,
  FileText,
  History,
  Layers,
  Pause,
  Play,
  Power,
  Search,
  TerminalSquare,
  XCircle,
} from "lucide-react"

import {
  cancelCommandAction,
  pauseCommandAction,
  queueCommandAction,
  resumeCommandAction,
} from "@/app/portal/command-console/actions"
import type { ConsoleSnapshot, ConsoleStory } from "@/lib/command-console-data"
import { formatTime, runResultPill, shortId, statePill } from "@/lib/command-console-ui"

// ---------------------------------------------------------------------------
// SDLC Command Console — thin operational cockpit over ENG-18.
// Dark/navy operational styling on the portal design tokens. Functional-first;
// visual polish deferred. No vendor-specific terminology.
// ---------------------------------------------------------------------------

const ROLE_OPTIONS = ["architect", "builder", "reviewer", "verifier"]
const PROFILE_OPTIONS = ["architect-pro", "builder-flash", "reviewer", "local-builder"]
const ENV_OPTIONS = ["DEV", "PROD", "TEST", "LOCAL"]
const TEST_MODE_OPTIONS = [
  { value: "SCOPED", hint: "targeted + adjacent tests only; full regression forbidden (default)" },
  { value: "FULL", hint: "full regression explicitly authorized" },
  { value: "NONE", hint: "no test execution — narrow non-code verification" },
] as const
const POLICY_OPTIONS = [
  { value: "Unattended OK", hint: "deterministic backend/infra/test work — safe for overnight dispatch" },
  { value: "Daytime Only", hint: "needs human observation/judgment (UI/UX/polish/browser-heavy)" },
  { value: "Human Gate", hint: "external account/provider/credential/production approval" },
  { value: "Manual Only", hint: "explicitly excluded from autonomous dispatch" },
] as const

type Tab = "activity" | "evidence" | "history"



export function CommandConsole({ snapshot }: { snapshot: ConsoleSnapshot }) {
  const [selectedStoryId, setSelectedStoryId] = useState<string | null>(
    snapshot.stories[0]?.id ?? null,
  )
  const [tab, setTab] = useState<Tab>("activity")
  const [notice, setNotice] = useState<{ ok: boolean; text: string } | null>(null)

  const selectedStory = snapshot.stories.find((s) => s.id === selectedStoryId) ?? null
  const activeCommand = snapshot.commands.find((c) =>
    c.state === "Claimed" || c.state === "Running" || c.state === "Paused",
  ) ?? null
  const commandsForStory = snapshot.commands.filter((c) => c.storyId === selectedStory?.id)

  if (!snapshot.ready) {
    return (
      <div className="min-h-[70vh] rounded-sm border border-[var(--portal-border)] bg-[var(--portal-surface)] px-10 py-16 text-center">
        <Layers className="mx-auto h-9 w-9 text-[var(--portal-blue-gray)]" />
        <h1 className="mt-5 font-serif text-2xl font-light text-[var(--portal-navy)]">
          Command console not ready
        </h1>
        <p className="mx-auto mt-3 max-w-lg text-sm font-light leading-6 text-black/55">
          The Story Board control-plane tables are not present. Apply migrations
          021–029 (DEV) or verify the production control-plane before using the
          console.
        </p>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-[var(--portal-navy-deep)] text-slate-200">
      <HealthStrip snapshot={snapshot} />

      {notice && (
        <div
          className={`mx-auto mt-4 flex max-w-[1600px] items-center justify-between gap-4 rounded-sm border px-4 py-2.5 text-sm font-light ${
            notice.ok
              ? "border-emerald-400/30 bg-emerald-400/10 text-emerald-200"
              : "border-red-400/40 bg-red-400/10 text-red-200"
          }`}
        >
          <span className="flex items-center gap-2">
            {notice.ok ? (
              <CheckCircle2 className="h-4 w-4" />
            ) : (
              <AlertTriangle className="h-4 w-4" />
            )}
            {notice.text}
          </span>
          <button onClick={() => setNotice(null)} className="text-xs text-slate-400 hover:text-white">
            Dismiss
          </button>
        </div>
      )}

      <div className="mx-auto flex max-w-[1600px] flex-col gap-5 px-5 py-5 xl:flex-row">
        <aside className="xl:w-[26%] xl:min-w-[300px]">
          <StorySelector
            stories={snapshot.stories}
            selectedId={selectedStoryId}
            onSelect={setSelectedStoryId}
          />
        </aside>

        <main className="min-w-0 flex-1">
          {selectedStory ? (
            <StoryWorkspace
              story={selectedStory}
              commandsForStory={commandsForStory}
              notice={notice}
              setNotice={setNotice}
            />
          ) : (
            <EmptyState text="Select a story from the left to inspect and queue work." />
          )}
        </main>

        <aside className="xl:w-[26%] xl:min-w-[300px]">
          <ExecutionPanel activeCommand={activeCommand} notice={notice} setNotice={setNotice} />
        </aside>
      </div>

      <div className="mx-auto max-w-[1600px] px-5 pb-10">
        <ExecutionLog
          tab={tab}
          setTab={setTab}
          story={selectedStory}
          commandsForStory={commandsForStory}
        />
      </div>
    </div>
  )
}

function HealthStrip({ snapshot }: { snapshot: ConsoleSnapshot }) {
  const workerPill = {
    idle: { label: "Worker idle", cls: "border-slate-500/40 text-slate-400" },
    busy: { label: "Worker busy", cls: "border-emerald-400/40 text-emerald-300" },
    paused: { label: "Worker paused", cls: "border-slate-400/40 text-slate-300" },
    stale: { label: "Worker stale", cls: "border-red-400/50 text-red-300" },
  }[snapshot.workerState]

  return (
    <header className="border-b border-white/10 bg-[var(--portal-navy-deep-2)]">
      <div className="mx-auto flex max-w-[1600px] flex-wrap items-center gap-x-8 gap-y-3 px-5 py-4">
        <div className="flex items-center gap-3">
          <TerminalSquare className="h-5 w-5 text-[var(--portal-blue-gray)]" />
          <h1 className="font-serif text-xl font-light uppercase tracking-[0.14em] text-white">
            SDLC Command Console
          </h1>
        </div>

        <div className="flex items-center gap-2 text-xs font-light text-slate-400">
          <span className="inline-block h-2 w-2 rounded-full bg-emerald-400" />
          engine healthy
        </div>

        <div className={`rounded-full border px-3 py-1 text-[11px] font-light tracking-wide ${workerPill.cls}`}>
          {workerPill.label}
        </div>

        <div className="flex items-center gap-2 text-xs font-light text-slate-300">
          <Activity className="h-3.5 w-3.5 text-slate-500" />
          <span className="tabular-nums">{snapshot.activeCommandCount}</span> active
        </div>

        <div className="flex items-center gap-2 text-xs font-light text-slate-300">
          <Clock className="h-3.5 w-3.5 text-slate-500" />
          <span className="tabular-nums">{snapshot.queuedReadyCount}</span> queued ready
        </div>

        <div className={`rounded-full border px-3 py-1 text-[11px] font-light tracking-wide ${
          snapshot.scheduler === "unattended"
            ? "border-emerald-400/30 text-emerald-300"
            : "border-white/10 text-slate-400"
        }`}>
          scheduler: {snapshot.scheduler}
        </div>
      </div>
    </header>
  )
}

function StorySelector({
  stories,
  selectedId,
  onSelect,
}: {
  stories: ConsoleStory[]
  selectedId: string | null
  onSelect: (id: string) => void
}) {
  const router = useRouter()
  const [query, setQuery] = useState("")
  const [filter, setFilter] = useState<"Ready" | "Planned" | "Active" | "Complete" | "All">("All")

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    return stories.filter((s) => {
      if (q && !s.id.toLowerCase().includes(q) && !s.title.toLowerCase().includes(q)) return false
      switch (filter) {
        case "Ready":
          return s.status === "Ready"
        case "Planned":
          return s.status === "Planned"
        case "Active":
          return s.status === "In Progress" || s.status === "Ready"
        case "Complete":
          return s.status === "Complete"
        default:
          return true
      }
    })
  }, [stories, query, filter])

  return (
    <section className="flex h-full flex-col overflow-hidden rounded-sm border border-white/10 bg-[var(--portal-navy-deep-2)]">
      <div className="border-b border-white/10 px-4 py-3">
        <div className="flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.2em] text-slate-500">
          <Search className="h-3.5 w-3.5" />
          Story Board
        </div>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search by story ID or title…"
          className="mt-2 w-full rounded-sm border border-white/10 bg-[var(--portal-navy-deep)] px-3 py-1.5 text-sm text-slate-200 placeholder:text-slate-600 focus:border-[var(--portal-blue-gray)]/60 focus:outline-none"
        />
        <div className="mt-3 flex flex-wrap gap-1.5">
          {(["All", "Ready", "Planned", "Active", "Complete"] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`rounded-full border px-2.5 py-1 text-[11px] font-light transition-colors ${
                filter === f
                  ? "border-[var(--portal-blue-gray)]/70 bg-[var(--portal-blue-gray)]/15 text-[var(--portal-on-navy)]"
                  : "border-white/10 text-slate-500 hover:border-white/25 hover:text-slate-300"
              }`}
            >
              {f}
            </button>
          ))}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {filtered.length === 0 && (
          <div className="px-4 py-8 text-center text-xs font-light text-slate-600">
            No stories match.
          </div>
        )}
        {filtered.map((s) => {
          const pill = statePill(s.status)
          const active = s.id === selectedId
          return (
            <button
              key={s.id}
              onClick={() => onSelect(s.id)}
              onDoubleClick={() =>
                router.push(`/portal/command-console/${encodeURIComponent(s.id)}`)
              }
              className={`block w-full border-b border-white/5 px-4 py-3 text-left transition-colors ${
                active ? "bg-[var(--portal-blue-gray)]/10" : "hover:bg-white/5"
              }`}
            >
              <div className="flex items-center justify-between gap-2">
                <span className="font-mono text-xs font-medium tracking-wide text-[var(--portal-on-navy)]">
                  {s.id}
                </span>
                <span className={`rounded-full border px-2 py-0.5 text-[10px] font-light ${pill.cls}`}>
                  {pill.label}
                </span>
              </div>
              <div className="mt-1 truncate text-sm font-light text-slate-300">{s.title}</div>
              <div className="mt-1.5 flex items-center gap-3 text-[11px] font-light text-slate-500">
                <span className="uppercase tracking-wide">{s.priority}</span>
                <span className="tabular-nums">{s.completion}%</span>
                {s.latestRun && (
                  <span className="flex items-center gap-1 text-emerald-400/80">
                    <CheckCircle2 className="h-3 w-3" />
                    {s.latestRun.resultStatus}
                  </span>
                )}
              </div>
            </button>
          )
        })}
      </div>
    </section>
  )
}

function EmptyState({ text }: { text: string }) {
  return (
    <div className="flex h-full min-h-[40vh] items-center justify-center rounded-sm border border-white/10 bg-[var(--portal-navy-deep-2)]">
      <div className="max-w-sm text-center">
        <TerminalSquare className="mx-auto h-8 w-8 text-slate-600" />
        <p className="mt-4 text-sm font-light text-slate-500">{text}</p>
      </div>
    </div>
  )
}


function StoryWorkspace({
  story,
  commandsForStory,
  notice,
  setNotice,
}: {
  story: ConsoleStory
  commandsForStory: Array<import("@/db/agent-work").AgentWorkItem>
  notice: { ok: boolean; text: string } | null
  setNotice: (n: { ok: boolean; text: string } | null) => void
}) {
  const [role, setRole] = useState(ROLE_OPTIONS[1])
  const [profile, setProfile] = useState(PROFILE_OPTIONS[1])
  const [instructions, setInstructions] = useState("")
  const [policy, setPolicy] = useState<string>(POLICY_OPTIONS[0].value)
  const [environment, setEnvironment] = useState("DEV")
  const [testMode, setTestMode] = useState<string>("SCOPED")
  const [busy, setBusy] = useState(false)

  const storyStatus = story.status
  const storyPill = statePill(storyStatus)
  const hasArchitectBrief = Boolean(story.architectBrief && story.architectBrief.length > 0)
  const hasAcceptance = Boolean(story.acceptanceCriteria && story.acceptanceCriteria.length > 0)
  const isReady = storyStatus === "Ready"

  async function queue() {
    setBusy(true)
    const res = await queueCommandAction({
      storyId: story.id,
      role,
      modelProfile: profile,
      specialInstructions: instructions,
      executionPolicy: policy,
      executionEnvironment: environment,
      testMode,
    })
    setBusy(false)
    if (res.ok) {
      setNotice({ ok: true, text: `Command queued for ${story.id}.` })
      setInstructions("")
    } else {
      setNotice({ ok: false, text: res.error })
    }
  }

  return (
    <section className="flex h-full flex-col rounded-sm border border-white/10 bg-[var(--portal-navy-deep-2)]">
      {/* Header */}
      <div className="border-b border-white/10 px-5 py-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="flex items-center gap-3">
              <span className="font-mono text-sm font-medium tracking-wide text-[var(--portal-on-navy)]">
                {story.id}
              </span>
              <span className={`rounded-full border px-2 py-0.5 text-[10px] font-light ${storyPill.cls}`}>
                {storyPill.label}
              </span>
              <span className="rounded-full border border-white/10 px-2 py-0.5 text-[10px] font-light uppercase tracking-wide text-slate-500">
                {story.priority}
              </span>
            </div>
            <h2 className="mt-1.5 font-serif text-xl font-light text-white">{story.title}</h2>
          </div>
          <div className="text-right">
            <div className="text-[10px] font-light uppercase tracking-[0.2em] text-slate-500">
              Completion
            </div>
            <div className="mt-0.5 text-2xl font-light tabular-nums text-slate-200">
              {story.completion}
              <span className="text-sm text-slate-500">%</span>
            </div>
            <Link
              href={`/portal/command-console/${encodeURIComponent(story.id)}`}
              className="mt-2 inline-block rounded-sm border border-[var(--portal-blue-gray)]/40 px-3 py-1.5 text-[11px] font-light text-[var(--portal-on-navy)] transition hover:border-[var(--portal-blue-gray)] hover:bg-[var(--portal-blue-gray)]/10"
            >
              Open Cockpit ↗
            </Link>
          </div>
        </div>

        <div className="mt-3 flex flex-wrap items-center gap-x-5 gap-y-1 text-[11px] font-light text-slate-500">
          <span className={hasArchitectBrief ? "text-emerald-400/80" : "text-slate-600"}>
            {hasArchitectBrief ? "architect brief available" : "no architect brief"}
          </span>
          <span className={hasAcceptance ? "text-emerald-400/80" : "text-slate-600"}>
            {hasAcceptance ? "acceptance criteria present" : "no acceptance criteria"}
          </span>
          <span>{story.dependencies ? `depends on: ${story.dependencies}` : "no declared dependencies"}</span>
        </div>
      </div>

      {/* Lifecycle strip (projection of the real command lifecycle) */}
      <div className="border-b border-white/10 px-5 py-3">
        <div className="flex flex-wrap items-center gap-1.5">
          {["ARCHITECTED", "READY", "QUEUED", "CLAIMED", "RUNNING", "RESULT"].map((stage, i) => {
            const reached =
              (stage === "ARCHITECTED" && hasArchitectBrief) ||
              (stage === "READY" && storyStatus === "Ready") ||
              (stage === "QUEUED" && commandsForStory.some((c) => c.state === "Ready")) ||
              (stage === "CLAIMED" && commandsForStory.some((c) => c.state === "Claimed")) ||
              (stage === "RUNNING" &&
                commandsForStory.some((c) => c.state === "Running" || c.state === "Paused")) ||
              (stage === "RESULT" &&
                commandsForStory.some((c) => c.state === "Done" || c.state === "Error" || c.state === "Cancelled"))
            return (
              <div key={stage} className="flex items-center gap-1.5">
                {i > 0 && <ArrowRight className="h-3 w-3 text-slate-700" />}
                <span
                  className={`rounded-full border px-2.5 py-1 text-[10px] font-light tracking-wide ${
                    reached
                      ? "border-emerald-400/40 bg-emerald-400/10 text-emerald-300"
                      : "border-white/10 text-slate-600"
                  }`}
                >
                  {stage}
                </span>
              </div>
            )
          })}
        </div>
      </div>


      {/* Story context panel (read-only canonical truth) */}
      <div className="flex-1 overflow-y-auto px-5 py-4">
        <div className="grid gap-4 lg:grid-cols-2">
          <ContextCard label="Goal" body={story.goal ?? "Not provided."} />
          <ContextCard label="Dependencies" body={story.dependencies ?? "None declared."} />
          <ContextCard label="Acceptance criteria" body={story.acceptanceCriteria ?? "Not provided."} />
          <ContextCard label="Architect brief" body={story.architectBrief ?? "Not provided."} />
        </div>

        {/* Command configuration */}
        <div className="mt-5 rounded-sm border border-white/10 bg-[var(--portal-navy-deep)] p-4">
          <div className="flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.2em] text-slate-500">
            <TerminalSquare className="h-3.5 w-3.5" />
            Queue command
          </div>

          <div className="mt-3 grid gap-4 sm:grid-cols-2">
            <Field label="Role">
              <select value={role} onChange={(e) => setRole(e.target.value)} className={inputCls}>
                {ROLE_OPTIONS.map((r) => <option key={r} value={r}>{r}</option>)}
              </select>
            </Field>
            <Field label="Model profile (logical)">
              <select value={profile} onChange={(e) => setProfile(e.target.value)} className={inputCls}>
                {PROFILE_OPTIONS.map((p) => <option key={p} value={p}>{p}</option>)}
              </select>
            </Field>
            <Field label="Execution target">
              <select value={environment} onChange={(e) => setEnvironment(e.target.value)} className={inputCls}>
                {ENV_OPTIONS.map((e) => <option key={e} value={e}>{e}</option>)}
              </select>
            </Field>
            <Field label="Test execution mode (runtime-authoritative)">
              <select value={testMode} onChange={(e) => setTestMode(e.target.value)} className={inputCls}>
                {TEST_MODE_OPTIONS.map((t) => <option key={t.value} value={t.value}>{t.value}</option>)}
              </select>
            </Field>
          </div>

          <div className="mt-2 grid gap-2 sm:grid-cols-3">
            {TEST_MODE_OPTIONS.map((t) => (
              <span key={t.value} className="rounded-sm border border-white/10 px-2 py-1 text-[10px] font-light text-slate-500">
                {t.value}: {t.hint}
              </span>
            ))}
          </div>

          <div className="mt-3">
            <Field label="Special instructions (optional)">
              <textarea
                value={instructions}
                onChange={(e) => setInstructions(e.target.value)}
                rows={2}
                placeholder="Additive instructions for this run — never replaces the architect brief."
                className={`${inputCls} resize-none`}
              />
            </Field>
          </div>

          <div className="mt-3">
            <div className="mb-1.5 text-[11px] font-light uppercase tracking-[0.16em] text-slate-500">
              Execution policy
            </div>
            <div className="grid gap-2 sm:grid-cols-2">
              {POLICY_OPTIONS.map((p) => (
                <label
                  key={p.value}
                  className={`flex cursor-pointer items-start gap-2 rounded-sm border px-3 py-2 transition-colors ${
                    policy === p.value
                      ? "border-[var(--portal-blue-gray)]/60 bg-[var(--portal-blue-gray)]/10"
                      : "border-white/10 hover:border-white/25"
                  }`}
                >
                  <input
                    type="radio"
                    name="policy"
                    checked={policy === p.value}
                    onChange={() => setPolicy(p.value)}
                    className="mt-0.5 accent-[var(--portal-blue-gray)]"
                  />
                  <span>
                    <span className="block text-xs font-light text-slate-200">{p.value}</span>
                    <span className="mt-0.5 block text-[10px] font-light leading-4 text-slate-500">
                      {p.hint}
                    </span>
                  </span>
                </label>
              ))}
            </div>
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-3">
            <button
              onClick={queue}
              disabled={busy || !isReady}
              className={`inline-flex items-center gap-2 rounded-sm px-5 py-2 text-sm font-light transition-colors ${
                busy
                  ? "cursor-wait bg-slate-700 text-slate-300"
                  : isReady
                    ? "bg-[var(--portal-blue-gray)] text-[var(--portal-navy-deep)] hover:bg-[var(--portal-gold-soft)]"
                    : "cursor-not-allowed bg-white/5 text-slate-600"
              }`}
            >
              <TerminalSquare className="h-4 w-4" />
              {busy ? "Queuing…" : "Queue command"}
            </button>
            {!isReady && (
              <span className="text-[11px] font-light text-slate-300/80">
                Story must be Ready to queue. Set it Ready on the Story Board first.
              </span>
            )}
          </div>
        </div>
      </div>
    </section>
  )
}

const inputCls =
  "w-full rounded-sm border border-white/10 bg-[var(--portal-navy-deep)] px-3 py-1.5 text-sm text-slate-200 focus:border-[var(--portal-blue-gray)]/60 focus:outline-none"

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1 text-[11px] font-light uppercase tracking-[0.16em] text-slate-500">{label}</div>
      {children}
    </div>
  )
}

function ContextCard({ label, body }: { label: string; body: string }) {
  return (
    <div className="rounded-sm border border-white/10 bg-[var(--portal-navy-deep)] p-4">
      <div className="text-[10px] font-light uppercase tracking-[0.2em] text-slate-500">{label}</div>
      <p className="mt-1.5 text-sm font-light leading-5 text-slate-300">{body}</p>
    </div>
  )
}


function ExecutionPanel({
  activeCommand,
  notice,
  setNotice,
}: {
  activeCommand: import("@/db/agent-work").AgentWorkItem | null
  notice: { ok: boolean; text: string } | null
  setNotice: (n: { ok: boolean; text: string } | null) => void
}) {
  const [busy, setBusy] = useState(false)

  if (!activeCommand) {
    return (
      <section className="flex h-full flex-col rounded-sm border border-white/10 bg-[var(--portal-navy-deep-2)]">
        <div className="border-b border-white/10 px-4 py-3">
          <div className="flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.2em] text-slate-500">
            <Activity className="h-3.5 w-3.5" />
            Current execution
          </div>
        </div>
        <div className="flex flex-1 items-center justify-center px-6 py-14 text-center">
          <div>
            <TerminalSquare className="mx-auto h-7 w-7 text-slate-700" />
            <p className="mt-3 text-sm font-light text-slate-500">No active execution</p>
            <p className="mt-1 text-[11px] font-light text-slate-700">
              Queue a command for a Ready story to begin.
            </p>
          </div>
        </div>
      </section>
    )
  }

  const pill = statePill(activeCommand.state)
  const cmd = activeCommand
  const canPause = cmd.state === "Running"
  const canResume = cmd.state === "Paused"
  const canCancel = cmd.state === "Claimed" || cmd.state === "Running" || cmd.state === "Paused"

  async function run(action: "pause" | "resume" | "cancel") {
    setBusy(true)
    const res =
      action === "pause"
        ? await pauseCommandAction(cmd.id)
        : action === "resume"
          ? await resumeCommandAction(cmd.id)
          : await cancelCommandAction(cmd.id)
    setBusy(false)
    if (res.ok) {
      setNotice({ ok: true, text: `Command ${action} requested.` })
    } else {
      setNotice({ ok: false, text: res.error })
    }
  }

  return (
    <section className="flex h-full flex-col rounded-sm border border-white/10 bg-[var(--portal-navy-deep-2)]">
      <div className="border-b border-white/10 px-4 py-3">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[11px] font-light uppercase tracking-[0.2em] text-slate-500">
            <Activity className="h-3.5 w-3.5" />
            Current execution
          </div>
          <span className={`rounded-full border px-2 py-0.5 text-[10px] font-light ${pill.cls}`}>
            {pill.label}
          </span>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-4">
        <dl className="space-y-3 text-sm font-light">
          <Row label="Story">
            <Link
              href={`/portal/command-console/${encodeURIComponent(activeCommand.storyId)}`}
              className="inline-block rounded-sm border border-[var(--portal-blue-gray)]/40 px-2 py-1 font-mono text-xs text-[var(--portal-on-navy)] transition hover:border-[var(--portal-blue-gray)] hover:bg-[var(--portal-blue-gray)]/10"
              title="Open Story Execution Cockpit"
            >
              {activeCommand.storyId} ↗
            </Link>
          </Row>
          <Row label="Worker">{activeCommand.claimedBy ?? "—"}</Row>
          <Row label="Runtime adapter">{activeCommand.runtimeAdapter ?? "not started"}</Row>
          <Row label="Model profile">{activeCommand.modelProfile ?? "—"}</Row>
          <Row label="State">
            <span className={`rounded-full border px-2 py-0.5 text-[10px] font-light ${pill.cls}`}>
              {pill.label}
            </span>
          </Row>
          <Row label="Attempts">
            <span className="tabular-nums">
              {activeCommand.attempts} / {activeCommand.maxAttempts}
            </span>
          </Row>
          <Row label="Claimed">{formatTime(activeCommand.claimedAt)}</Row>
          <Row label="Started">{formatTime(activeCommand.startedAt)}</Row>
          <Row label="Last activity">{formatTime(activeCommand.updatedAt)}</Row>
          {activeCommand.externalRunId && (
            <Row label="External run">
              <span className="font-mono text-[11px] text-slate-500">{shortId(activeCommand.externalRunId)}</span>
            </Row>
          )}
        </dl>

        <div className="mt-5 flex flex-wrap gap-2">
          {canPause && (
            <button
              onClick={() => run("pause")}
              disabled={busy}
              className="inline-flex items-center gap-1.5 rounded-sm border border-white/20 px-3 py-1.5 text-xs font-light text-slate-300 hover:bg-white/10 disabled:opacity-50"
            >
              <Pause className="h-3.5 w-3.5" /> Pause
            </button>
          )}
          {canResume && (
            <button
              onClick={() => run("resume")}
              disabled={busy}
              className="inline-flex items-center gap-1.5 rounded-sm border border-emerald-400/40 px-3 py-1.5 text-xs font-light text-emerald-300 hover:bg-emerald-400/10 disabled:opacity-50"
            >
              <Play className="h-3.5 w-3.5" /> Resume
            </button>
          )}
          {canCancel && (
            <button
              onClick={() => run("cancel")}
              disabled={busy}
              className="inline-flex items-center gap-1.5 rounded-sm border border-red-400/40 px-3 py-1.5 text-xs font-light text-red-300 hover:bg-red-400/10 disabled:opacity-50"
            >
              <XCircle className="h-3.5 w-3.5" /> Cancel
            </button>
          )}
        </div>
      </div>
    </section>
  )
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <dt className="text-[11px] font-light uppercase tracking-[0.14em] text-slate-500">{label}</dt>
      <dd className="text-right text-xs text-slate-300">{children}</dd>
    </div>
  )
}


function ExecutionLog({
  tab,
  setTab,
  story,
  commandsForStory,
}: {
  tab: Tab
  setTab: (t: Tab) => void
  story: ConsoleStory | null
  commandsForStory: Array<import("@/db/agent-work").AgentWorkItem>
}) {
  return (
    <section className="rounded-sm border border-white/10 bg-[var(--portal-navy-deep-2)]">
      <div className="flex items-center gap-1 border-b border-white/10 px-4">
        {(
          [
            { id: "activity", label: "Activity", icon: Activity },
            { id: "evidence", label: "Evidence", icon: FileText },
            { id: "history", label: "Run History", icon: History },
          ] as const
        ).map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`inline-flex items-center gap-1.5 border-b-2 px-4 py-3 text-xs font-light tracking-wide transition-colors ${
              tab === t.id
                ? "border-[var(--portal-blue-gray)] text-slate-200"
                : "border-transparent text-slate-500 hover:text-slate-300"
            }`}
          >
            <t.icon className="h-3.5 w-3.5" />
            {t.label}
          </button>
        ))}
      </div>

      <div className="max-h-[420px] overflow-y-auto px-4 py-4">
        {!story ? (
          <p className="py-8 text-center text-sm font-light text-slate-600">
            Select a story to see its execution log.
          </p>
        ) : tab === "activity" ? (
          <ActivityLog story={story} commandsForStory={commandsForStory} />
        ) : tab === "evidence" ? (
          <EvidenceView story={story} />
        ) : (
          <RunHistory story={story} />
        )}
      </div>
    </section>
  )
}

function ActivityLog({
  story,
  commandsForStory,
}: {
  story: ConsoleStory
  commandsForStory: Array<import("@/db/agent-work").AgentWorkItem>
}) {
  const events: Array<{ time: string; text: string; kind: string }> = []

  for (const c of commandsForStory) {
    if (c.claimedAt) events.push({ time: c.claimedAt, text: `Command claimed by ${c.claimedBy ?? "worker"}`, kind: "claimed" })
    if (c.startedAt) events.push({ time: c.startedAt, text: "Runtime started", kind: "started" })
  }

  if (story.latestRun?.notes) {
    const lines = story.latestRun.notes.split("\n").filter(Boolean)
    for (const line of lines) {
      const m = line.match(/^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}) — (.*)$/)
      events.push({
        time: m ? m[1] : "",
        text: m ? m[2] : line,
        kind: /complete|cancelled|failed/.test(line) ? "terminal" : "step",
      })
    }
  }

  if (events.length === 0) {
    return (
      <div className="py-8 text-center text-sm font-light text-slate-600">
        No activity recorded for this story yet.
      </div>
    )
  }

  return (
    <ul className="space-y-2">
      {events.map((e, i) => (
        <li key={i} className="flex items-start gap-3 text-sm font-light">
          <span className="mt-1 inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--portal-blue-gray)]/70" />
          <div className="min-w-0 flex-1">
            <p className="break-words text-slate-300">{e.text}</p>
            {e.time && (
              <p className="mt-0.5 text-[10px] font-light tabular-nums text-slate-600">{e.time}</p>
            )}
          </div>
        </li>
      ))}
    </ul>
  )
}

function EvidenceView({ story }: { story: ConsoleStory }) {
  const run = story.latestRun
  if (!run) {
    return (
      <div className="py-8 text-center text-sm font-light text-slate-600">
        No evidence recorded for this story yet.
      </div>
    )
  }
  return (
    <dl className="grid gap-3 sm:grid-cols-2">
      <Row label="Result status">{run.resultStatus ?? "—"}</Row>
      <Row label="Completion">{run.completion != null ? `${run.completion}%` : "—"}</Row>
      <Row label="Tests">{run.testsSummary ?? "—"}</Row>
      <Row label="Commit">{run.commitHash ? shortId(run.commitHash) : "—"}</Row>
      <div className="sm:col-span-2">
        <dt className="mb-1 text-[11px] font-light uppercase tracking-[0.14em] text-slate-500">Notes</dt>
        <dd className="whitespace-pre-wrap rounded-sm border border-white/10 bg-[var(--portal-navy-deep)] p-3 text-xs font-light leading-5 text-slate-300">
          {run.notes ?? "—"}
        </dd>
      </div>
    </dl>
  )
}

function RunHistory({ story }: { story: ConsoleStory }) {
  // The read model carries the latest run; run history for prior runs is
  // served from storyboard_story_run (the canonical evidence table).
  if (!story.latestRun) {
    return (
      <div className="py-8 text-center text-sm font-light text-slate-600">
        No prior runs for this story.
      </div>
    )
  }
  return (
    <ul className="space-y-2">
      <li className="flex items-center justify-between gap-4 rounded-sm border border-white/10 bg-[var(--portal-navy-deep)] px-4 py-3 text-sm font-light">
        <span className="flex items-center gap-2">
          <span
            className={`rounded-full border px-2 py-0.5 text-[10px] font-light ${statePill(runResultPill(story.latestRun.resultStatus)).cls}`}
          >
            {runResultPill(story.latestRun.resultStatus)}
          </span>
          <span className="font-mono text-[11px] text-slate-400">{shortId(story.latestRun.id)}</span>
        </span>
        <span className="text-[11px] font-light tabular-nums text-slate-500">
          {story.latestRun.completion != null ? `${story.latestRun.completion}%` : "—"} · {formatTime(story.latestRun.endedAt ?? story.latestRun.startedAt)}
        </span>
      </li>
    </ul>
  )
}
