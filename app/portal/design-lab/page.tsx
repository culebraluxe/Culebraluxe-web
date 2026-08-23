"use client"

import Link from "next/link"
import { useRef, useState } from "react"
import {
  Activity,
  AlertCircle,
  AlertTriangle,
  ArrowUpRight,
  Ban,
  Building2,
  CalendarDays,
  CheckCircle2,
  Circle,
  Clock,
  Eye,
  FilePlus2,
  FileText,
  Home,
  Pencil,
  Plus,
  Search,
  Settings,
  Trash2,
  TrendingUp,
  User,
  Workflow,
  XCircle,
} from "lucide-react"

import { PageHeader } from "@/components/portal/page-header"
import { Panel } from "@/components/portal/panel"

// ---------------------------------------------------------------------------
// TEMPORARY PORTAL DESIGN LAB — /portal/design-lab
//
// Component playground for evaluating shared portal UI before promotion.
// Direct URL only (not in any nav). Reuses the existing --portal-* tokens,
// the shared Panel primitive, the operating shell, and the glass rail
// treatment. No new design system, no business logic, no data queries —
// everything below is fake demo data. Delete this folder when done.
// ---------------------------------------------------------------------------

const btnPrimary =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-white transition hover:bg-[var(--portal-navy-soft)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60 disabled:cursor-not-allowed disabled:opacity-40"
const btnSecondary =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"
const btnGhost =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--portal-blue-gray)] transition hover:bg-[var(--portal-rail-hover-bg)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"
const btnDestructive =
  "inline-flex min-h-11 items-center justify-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-danger)] px-4 text-[11px] font-medium uppercase tracking-[0.14em] text-[var(--portal-archive)] transition hover:bg-[var(--portal-archive)] hover:text-white focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-danger)]/40"
const btnIcon =
  "inline-flex h-11 w-11 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60"

const labelCls = "text-[10px] font-light uppercase tracking-[0.18em] text-black/40"
const inputCls =
  "min-h-11 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 text-sm font-light text-[var(--portal-text)] outline-none placeholder:text-black/35 focus:border-[var(--portal-navy-soft)]"
const selectCls = `${inputCls} appearance-none`

const badgeTones: Record<string, string> = {
  active: "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]",
  pending: "bg-[var(--portal-neutral-pale)] text-[var(--portal-neutral)]",
  complete: "bg-[var(--portal-success-pale)] text-[var(--portal-success)]",
  blocked: "bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]",
  hold: "bg-black/5 text-black/50",
  failed: "bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]",
  needsReview: "bg-[var(--portal-gold-pale)] text-[var(--portal-gold-muted)]",
  draft: "border border-black/10 text-black/45",
  issued: "border border-[var(--portal-blue-gray)]/40 text-[var(--portal-navy-soft)]",
}

function Badge({ tone, label }: { tone: keyof typeof badgeTones; label: string }) {
  return (
    <span
      className={`inline-block whitespace-nowrap rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em] ${badgeTones[tone]}`}
    >
      {label}
    </span>
  )
}

function Tab({ label, active = false }: { label: string; active?: boolean }) {
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      className="portal-glass-tab"
    >
      {label}
    </button>
  )
}
const demoDeals = [
  { id: "d-1", property: "Villa Rosada", client: "Morgan & Reed", stage: "Under Contract", value: "$1,240,000", next: "Closing docs", owner: "LP", status: "active" },
  { id: "d-2", property: "Casa Palmera", client: "The Alvarez Trust", stage: "Offer", value: "$875,000", next: "Counter review", owner: "LP", status: "pending" },
  { id: "d-3", property: "Blue Marlin House", client: "Jamie Ellis", stage: "Showing", value: "$1,650,000", next: "Second showing", owner: "LP", status: "blocked" },
  { id: "d-4", property: "Solana Cottage", client: "Dana Whitfield", stage: "Qualified", value: "$520,000", next: "Pre-approval", owner: "LP", status: "complete" },
] as const

const demoActivity = [
  { at: "Today · 9:41", kind: "call", actor: "Morgan Reed", text: "Accepted the revised offer terms on Villa Rosada.", detail: "Deal · Villa Rosada" },
  { at: "Today · 8:15", kind: "doc", actor: "Lisa Penfield", text: "Issued Offer Letter v1 for Casa Palmera.", detail: "Deal · Casa Palmera" },
  { at: "Yesterday", kind: "cal", actor: "Jamie Ellis", text: "Scheduled a second showing for Blue Marlin House.", detail: "Showing · Sat 2:00pm" },
  { at: "Yesterday", kind: "alert", actor: "System", text: "Financing deadline approaching for Solana Cottage.", detail: "Deal · Solana Cottage" },
]

function ActivityIcon({ kind }: { kind: string }) {
  const cls = "h-3.5 w-3.5"
  switch (kind) {
    case "call":
      return <User className={`${cls} text-[var(--portal-blue-gray)]`} />
    case "doc":
      return <FileText className={`${cls} text-[var(--portal-gold-muted)]`} />
    case "cal":
      return <CalendarDays className={`${cls} text-[var(--portal-blue-gray)]`} />
    default:
      return <AlertCircle className={`${cls} text-[var(--portal-archive)]`} />
  }
}

const iconSet: Array<{ icon: typeof User; label: string }> = [
  { icon: User, label: "Client" },
  { icon: Building2, label: "Deal" },
  { icon: Home, label: "Property" },
  { icon: CalendarDays, label: "Showing" },
  { icon: FileText, label: "Document" },
  { icon: FilePlus2, label: "Form" },
  { icon: Workflow, label: "Workflow" },
  { icon: Activity, label: "Activity" },
  { icon: AlertTriangle, label: "Attention" },
  { icon: Settings, label: "Settings" },
  { icon: Search, label: "Search" },
  { icon: Plus, label: "Add" },
  { icon: Pencil, label: "Edit" },
  { icon: Trash2, label: "Delete" },
  { icon: CheckCircle2, label: "Success" },
  { icon: XCircle, label: "Error" },
  { icon: Clock, label: "Pending" },
]

export default function DesignLabPage() {
  const [tab, setTab] = useState("overview")
  const [search, setSearch] = useState("")
  const [toggled, setToggled] = useState(false)

  return (
    <div className="mx-auto max-w-[1400px]">
      <div className="mb-6 flex flex-wrap items-center gap-3">
        <PageHeader
          eyebrow="PORTAL DESIGN LAB"
          title="Shared UI Playground"
          subtitle="Temporary, direct-URL-only playground for evaluating shared portal primitives before promotion. Nothing here ships to production pages."
        />
        <span className="rounded-full border border-[var(--portal-gold)]/40 bg-[var(--portal-gold-pale)] px-3 py-1 text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">
          Temporary — not in nav
        </span>
      </div>

      {/* 1 — NAVIGATION / TABS */}
      <Panel
        eyebrow="01 · NAVIGATION"
        heading="Secondary tab rail + segmented controls"
        subtitle="The exact glass rail treatment used by the operating shell, plus a compact segmented variant. Active / inactive / hover / focus."
        className="mb-6"
      >
        <div className="space-y-6">
          <div>
            <p className={`${labelCls} mb-2`}>Surface rail (as in the shell)</p>
            <div className="overflow-x-auto pb-1 [scrollbar-width:none]">
              <div className="portal-glass-rail">
                {["Overview", "Clients", "Deals", "Forms", "Documents", "Activity"].map((t) => (
                  <Tab key={t} label={t} active={t === "Deals"} />
                ))}
              </div>
            </div>
          </div>

          <div>
            <p className={`${labelCls} mb-2`}>Compact segmented control</p>
            <div className="flex w-max items-center gap-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white p-1 shadow-sm">
              {(["All", "Active", "Closed"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => setTab(t)}
                  className={[
                    "flex min-h-11 items-center whitespace-nowrap rounded-md px-4 text-[11px] font-medium uppercase tracking-[0.12em] transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--portal-gold)]/60",
                    tab === t
                      ? "bg-[var(--portal-navy)] text-white"
                      : "text-[var(--portal-navy-soft)] hover:bg-[var(--portal-rail-hover-bg)]",
                  ].join(" ")}
                >
                  {t}
                </button>
              ))}
            </div>
            <p className="mt-2 text-xs font-light text-black/45">
              Pressed: “{tab}” — keyboard focus uses the gold ring.
            </p>
          </div>
        </div>
      </Panel>

      {/* 2 — PANELS / CARDS */}
      <Panel
        eyebrow="02 · PANELS"
        heading="Shared panel variants + layout forms"
        subtitle="Standard, Soft, Feature (navy) and Attention — plus useful card compositions built on the same primitive."
        className="mb-6"
      >
        <div className="grid gap-4 md:grid-cols-2">
          <Panel variant="standard" heading="Standard panel" eyebrow="NEXUS · Dashboard">
            Frosted glass surface, light specular edge, layered shadow, serif heading.
          </Panel>
          <Panel variant="soft" heading="Soft panel" eyebrow="Break up white space">
            Cooler, more translucent glass for secondary content.
          </Panel>
          <Panel variant="feature" heading="Feature panel" eyebrow="High priority">
            Deep navy gradient, white text, restrained gold eyebrow. Used sparingly.
          </Panel>
          <Panel variant="attention" heading="Attention panel" eyebrow="Needs review">
            Pale cool surface, navy boundary, gold top accent. Not warning-yellow.
          </Panel>
        </div>

        <div className="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <Panel heading="Card with header + action" action={<Link className="text-[11px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)] hover:text-[var(--portal-navy)]" href="/portal/deals">View all →</Link>}>
            A compact card with a heading and a trailing action slot.
          </Panel>
          <Panel heading="Compact card" className="p-5">
            Tighter padding for dense lists and side rails.
          </Panel>
          <Panel heading="Card with icon" action={<Eye className="h-4 w-4 text-[var(--portal-blue-gray)]" />}>
            An icon in the action slot for quick-recognition card types.
          </Panel>
          <Panel heading="Flush body card" divider flush>
            <div className="px-6 py-4 text-sm font-light text-black/55">
              Header separated by a divider; body sits flush to the edges — for tables and row lists.
            </div>
          </Panel>
          <Panel variant="soft" heading="Card with footer / action row">
            <div className="mt-4 flex flex-wrap gap-2">
              <button type="button" className={btnSecondary}>Open deal</button>
              <button type="button" className={btnGhost}>Dismiss</button>
            </div>
          </Panel>
        </div>
      </Panel>


      {/* 3 — BUTTONS / ACTIONS */}
      <Panel
        eyebrow="03 · BUTTONS"
        heading="Action treatments"
        subtitle="The portal's established button language: navy primary, bordered secondary, quiet ghost, destructive, icon-only."
        className="mb-6"
      >
        <div className="flex flex-wrap items-center gap-3">
          <button type="button" className={btnPrimary}>
            <Plus className="h-3.5 w-3.5" /> Primary
          </button>
          <button type="button" className={btnSecondary}>Secondary</button>
          <button type="button" className={btnGhost}>Ghost / quiet</button>
          <button type="button" className={btnDestructive}>
            <Trash2 className="h-3.5 w-3.5" /> Destructive
          </button>
          <button type="button" className={btnIcon} aria-label="Edit">
            <Pencil className="h-4 w-4" />
          </button>
          <button type="button" className={btnIcon} aria-label="Delete">
            <Trash2 className="h-4 w-4 text-[var(--portal-archive)]" />
          </button>
        </div>
        <p className="mt-4 text-xs font-light text-black/45">
          44px min height on every action; hover, pressed and focus-visible states are token-driven.
        </p>
      </Panel>

      {/* 4 — ICONS */}
      <Panel
        eyebrow="04 · ICONS"
        heading="Curated icon strip"
        subtitle="Only icons that map to real portal jobs, from the existing lucide-react set — no decorative clutter."
        className="mb-6"
      >
        <div className="flex flex-wrap gap-2">
          {iconSet.map(({ icon: Icon, label }) => (
            <div
              key={label}
              className="flex min-h-11 w-28 flex-col items-center justify-center gap-1.5 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-2 py-3 text-[var(--portal-navy-soft)]"
            >
              <Icon className="h-4 w-4" />
              <span className="text-[9px] font-light uppercase tracking-[0.1em] text-black/45">
                {label}
              </span>
            </div>
          ))}
        </div>
      </Panel>

      {/* 5 — BADGES / STATUS */}
      <Panel
        eyebrow="05 · BADGES"
        heading="Application status pills"
        subtitle="The exact badge language used for real portal states — deal stages, story statuses, form and document states."
        className="mb-6"
      >
        <div className="flex flex-wrap gap-2">
          <Badge tone="active" label="Active" />
          <Badge tone="pending" label="Pending" />
          <Badge tone="complete" label="Complete" />
          <Badge tone="blocked" label="Blocked" />
          <Badge tone="hold" label="Hold" />
          <Badge tone="failed" label="Failed" />
          <Badge tone="needsReview" label="Needs Review" />
          <Badge tone="draft" label="Draft" />
          <Badge tone="issued" label="Issued" />
        </div>
      </Panel>


      {/* 6 — INPUTS / FILTERS */}
      <Panel
        eyebrow="06 · INPUTS"
        heading="Input + filter row"
        subtitle="Token-styled text, search, select, date, textarea, checkbox/toggle, and one compact filter row. No form logic."
        className="mb-6"
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
          <label className="block">
            <span className={labelCls}>Text input</span>
            <input className={`${inputCls} mt-2`} placeholder="Client name…" />
          </label>
          <label className="block">
            <span className={labelCls}>Search</span>
            <div className="relative mt-2">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-black/35" />
              <input className={`${inputCls} pl-9`} value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Search deals…" />
            </div>
          </label>
          <label className="block">
            <span className={labelCls}>Select</span>
            <select className={`${selectCls} mt-2`} defaultValue="">
              <option value="">All stages</option>
              <option>Under Contract</option>
              <option>Offer</option>
              <option>Showing</option>
            </select>
          </label>
          <label className="block">
            <span className={labelCls}>Date</span>
            <input type="date" className={`${inputCls} mt-2`} />
          </label>
          <label className="block md:col-span-2">
            <span className={labelCls}>Textarea</span>
            <textarea className={`${inputCls} mt-2 min-h-24 resize-y py-2`} placeholder="Notes…" />
          </label>
        </div>

        <div className="mt-6">
          <p className={`${labelCls} mb-2`}>Checkbox + toggle</p>
          <div className="flex flex-wrap items-center gap-6">
            <label className="flex min-h-11 items-center gap-2 text-sm font-light text-[var(--portal-text)]">
              <input type="checkbox" className="h-4 w-4 accent-[var(--portal-navy)]" defaultChecked />
              Include archived
            </label>
            <button type="button" onClick={() => setToggled((v) => !v)} aria-pressed={toggled} className="flex min-h-11 items-center gap-2 text-sm font-light text-[var(--portal-text)]">
              <span className={["relative h-6 w-11 rounded-full transition-colors", toggled ? "bg-[var(--portal-navy)]" : "bg-[var(--portal-mist-4)]"].join(" ")}>
                <span className={["absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform", toggled ? "translate-x-[22px]" : "translate-x-0.5"].join(" ")} />
              </span>
              Auto-assign
            </button>
          </div>
        </div>

        <div className="mt-6">
          <p className={`${labelCls} mb-2`}>Compact filter row</p>
          <div className="flex flex-wrap items-end gap-3">
            <label className="block w-full max-w-[200px]">
              <span className={labelCls}>Status</span>
              <select className={`${selectCls} mt-2`} defaultValue="all">
                <option value="all">All</option>
                <option>Active</option>
                <option>Blocked</option>
              </select>
            </label>
            <label className="block w-full max-w-[200px]">
              <span className={labelCls}>Owner</span>
              <select className={`${selectCls} mt-2`} defaultValue="lp">
                <option value="lp">Lisa Penfield</option>
                <option>Any</option>
              </select>
            </label>
            <button type="button" className={btnSecondary}>Apply filters</button>
            <button type="button" className={btnGhost}>Reset</button>
          </div>
        </div>
      </Panel>


      {/* 7 — TABLE / LIST */}
      <Panel
        eyebrow="07 · TABLE"
        heading="Table + operational list row"
        subtitle="Strong header, row separation, hover state, aligned numbers, status badge column, action column — plus one simpler list-row pattern. Fake data."
        className="mb-6"
      >
        <div className="overflow-x-auto">
          <table className="w-full min-w-[880px] border-collapse text-left text-sm">
            <thead>
              <tr className="border-b border-[var(--portal-panel-border)] bg-[var(--portal-blue-pale)]">
                {["Property", "Client", "Stage", "Value", "Next milestone", "Status", ""].map((h) => (
                  <th key={h} className="px-6 py-3 text-[10px] font-light uppercase tracking-[0.2em] text-black/40">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {demoDeals.map((d) => (
                <tr key={d.id} className="border-b border-[var(--portal-border)] transition-colors last:border-b-0 hover:bg-[var(--portal-blue-pale)]/50">
                  <td className="px-6 py-4 font-medium text-[var(--portal-navy)]">{d.property}</td>
                  <td className="px-6 py-4 font-light text-black/60">{d.client}</td>
                  <td className="px-6 py-4 font-light text-black/60">{d.stage}</td>
                  <td className="px-6 py-4 text-right tabular-nums font-light text-black/70">{d.value}</td>
                  <td className="px-6 py-4 font-light text-black/50">{d.next}</td>
                  <td className="px-6 py-4">
                    <Badge tone={d.status} label={d.status} />
                  </td>
                  <td className="px-6 py-4">
                    <button type="button" className={btnIcon} aria-label={`Open ${d.property}`}>
                      <Eye className="h-4 w-4" />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <p className={`${labelCls} mb-2 mt-8`}>Operational list row (simpler)</p>
        <div className="divide-y divide-[var(--portal-border)]">
          {demoActivity.slice(0, 2).map((a) => (
            <div key={a.text} className="flex min-h-12 items-center gap-3 px-1 py-3">
              <ActivityIcon kind={a.kind} />
              <span className="min-w-0 flex-1 truncate text-sm font-light text-black/65">{a.text}</span>
              <span className="shrink-0 text-xs font-light text-black/40">{a.at}</span>
            </div>
          ))}
        </div>
      </Panel>

      {/* 8 — KPI / DATA DISPLAY */}
      <Panel
        eyebrow="08 · KPI"
        heading="KPI / data display"
        subtitle="Number + label, number + trend, one compact metric row, and one navy Feature treatment. No analytics dashboard."
        className="mb-6"
      >
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <div className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
            <p className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">Active deals</p>
            <div className="mt-3 font-serif text-3xl font-light text-[var(--portal-navy)]">7</div>
            <div className="mt-1 text-xs font-light text-black/45">Across 4 properties</div>
          </div>
          <div className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
            <p className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">Volume in motion</p>
            <div className="mt-3 font-serif text-3xl font-light text-[var(--portal-navy)]">$4.3M</div>
            <div className="mt-1 flex items-center gap-1.5 text-xs font-light text-[var(--portal-success)]">
              <TrendingUp className="h-3.5 w-3.5" /> +12% this quarter
            </div>
          </div>
          <div className="portal-glass-panel rounded-[var(--portal-panel-radius)] p-5">
            <p className="text-[10px] font-light uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]">Needs attention</p>
            <div className="mt-3 font-serif text-3xl font-light text-[var(--portal-navy)]">3</div>
            <div className="mt-1 text-xs font-light text-[var(--portal-archive)]">1 financing deadline</div>
          </div>
          <Panel variant="feature" heading="Next closing" eyebrow="Priority">
            <div className="font-serif text-2xl font-light">Villa Rosada</div>
            <div className="mt-1 text-xs font-light text-white/55">Closing in 9 days</div>
          </Panel>
        </div>
      </Panel>


      {/* 9 — TIMELINE / ACTIVITY */}
      <Panel
        eyebrow="09 · TIMELINE"
        heading="Activity / timeline pattern"
        subtitle="Timestamp + icon/status + actor + event text + optional secondary detail. Fake activity."
        className="mb-6"
      >
        <div className="relative">
          <span className="absolute bottom-2 left-[5px] top-2 w-px bg-[var(--portal-border)]" aria-hidden />
          <div className="space-y-5">
            {demoActivity.map((a) => (
              <div key={a.text} className="relative flex gap-4 pl-6">
                <span className="absolute left-0 top-1 flex h-3 w-3 items-center justify-center rounded-full border border-[var(--portal-border)] bg-white">
                  <span className="h-1.5 w-1.5 rounded-full bg-[var(--portal-navy)]" />
                </span>
                <div className="min-w-0">
                  <div className="text-xs font-light text-black/40">{a.at}</div>
                  <div className="mt-1 text-sm font-medium text-[var(--portal-navy)]">{a.actor}</div>
                  <div className="mt-0.5 flex items-start gap-1.5 text-sm font-light leading-6 text-black/65">
                    <ActivityIcon kind={a.kind} />
                    <span>{a.text}</span>
                  </div>
                  <div className="mt-1 text-xs font-light text-black/40">{a.detail}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </Panel>

      {/* 10 — WORKFLOW / PROCESS */}
      <Panel
        eyebrow="10 · PROCESS"
        heading="Simple process step pattern"
        subtitle="Completed / current / upcoming / blocked — pure visual, no workflow engine behavior."
        className="mb-6"
      >
        <ol className="space-y-0">
          {[
            { label: "Offer accepted", note: "Recorded on Oct 12", state: "done" as const },
            { label: "Attorney review", note: "Draft contract", state: "done" as const },
            { label: "Inspection", note: "Scheduled Oct 28", state: "current" as const },
            { label: "Financing clear to close", note: "Waiting on lender", state: "blocked" as const },
            { label: "Closing", note: "Target Nov 15", state: "upcoming" as const },
          ].map((s) => (
            <li key={s.label} className="relative flex gap-4 pb-6 last:pb-0">
              {s.state !== "done" && s.state !== "current" && (
                <span className="absolute left-[11px] top-7 h-full w-px bg-[var(--portal-border)]" />
              )}
              <span
                className={[
                  "relative z-10 flex h-6 w-6 shrink-0 items-center justify-center rounded-full border",
                  s.state === "done"
                    ? "border-[var(--portal-success)] bg-[var(--portal-success)] text-white"
                    : s.state === "current"
                      ? "border-[var(--portal-navy)] bg-[var(--portal-navy)] text-white"
                      : s.state === "blocked"
                        ? "border-[var(--portal-archive)] bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]"
                        : "border-[var(--portal-border)] bg-white text-black/30",
                ].join(" ")}
              >
                {s.state === "done" ? (
                  <CheckCircle2 className="h-3.5 w-3.5" />
                ) : s.state === "current" ? (
                  <Clock className="h-3.5 w-3.5" />
                ) : s.state === "blocked" ? (
                  <Ban className="h-3.5 w-3.5" />
                ) : (
                  <Circle className="h-3 w-3" />
                )}
              </span>
              <div className="min-w-0 pt-0.5">
                <div className="text-sm font-medium text-[var(--portal-navy)]">{s.label}</div>
                <div className="mt-0.5 text-xs font-light text-black/45">{s.note}</div>
              </div>
            </li>
          ))}
        </ol>
      </Panel>

      {/* 11 — DRAWER / DIALOG / POPOVER */}
      <Panel
        eyebrow="11 · OVERLAYS"
        heading="Dialog / drawer / popover"
        subtitle="Native, token-styled implementations (no new dependency). If these look good, they can become shared primitives later."
        className="mb-6"
      >
        <div className="flex flex-wrap items-center gap-3">
          <DialogDemo />
          <DrawerDemo />
          <PopoverDemo />
        </div>
      </Panel>


      {/* 12 — EMPTY / LOADING / ATTENTION */}
      <Panel
        eyebrow="12 · STATES"
        heading="Empty / loading / attention / error"
        subtitle="The portal's non-data states, token-styled."
        className="mb-6"
      >
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          <Panel heading="Empty state">
            <div className="py-8 text-center">
              <div className="mx-auto flex h-10 w-10 items-center justify-center rounded-full bg-[var(--portal-blue-pale)] text-[var(--portal-blue-gray)]">
                <Search className="h-4 w-4" />
              </div>
              <p className="mt-3 text-sm font-light text-black/50">No matching records yet.</p>
            </div>
          </Panel>
          <Panel heading="Loading / skeleton">
            <div className="space-y-3 py-2">
              <div className="h-3 w-3/4 animate-pulse rounded bg-[var(--portal-mist-4)]" />
              <div className="h-3 w-1/2 animate-pulse rounded bg-[var(--portal-mist-3)]" />
              <div className="h-8 w-full animate-pulse rounded-[var(--portal-tab-radius)] bg-[var(--portal-blue-pale)]" />
            </div>
          </Panel>
          <Panel variant="attention" heading="Needs review">
            <div className="mt-2 flex items-start gap-2 text-sm font-light leading-6 text-[var(--portal-text)]/80">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-[var(--portal-gold-muted)]" />
              3 website enquiries awaiting a human decision.
            </div>
          </Panel>
          <Panel heading="Error state">
            <div className="flex items-start gap-2 text-sm font-light leading-6 text-[var(--portal-text)]/80">
              <XCircle className="mt-0.5 h-4 w-4 shrink-0 text-[var(--portal-archive)]" />
              Could not reach the calendar provider. Check the connection and retry.
            </div>
          </Panel>
        </div>
      </Panel>

      {/* 13 — MEDIA / PROPERTY CARD */}
      <Panel
        eyebrow="13 · PROPERTY"
        heading="Real-estate card pattern"
        subtitle="Thumbnail, address/title, price/status, compact metadata, action. Placeholder image only — public-site components untouched."
        className="mb-6"
      >
        <div className="grid gap-4 md:grid-cols-2">
          {[
            { name: "Villa Rosada", area: "Ensenada Honda · 3 bd · 2.5 ba", price: "$1,240,000", status: "For Sale", image: "from-[var(--portal-blue-pale)] via-[var(--portal-mist-4)] to-[var(--portal-navy-soft)]" },
            { name: "Casa Palmera", area: "Flamenco · 4 bd · 3 ba", price: "$875,000", status: "Under Contract", image: "from-[var(--portal-mist-2)] via-[var(--portal-blue-pale)] to-[var(--portal-navy)]" },
          ].map((p) => (
            <div key={p.name} className="overflow-hidden rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white shadow-[var(--portal-panel-shadow)]">
              <div className={`h-40 bg-gradient-to-br ${p.image}`} />
              <div className="p-5">
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <div className="font-serif text-xl font-light text-[var(--portal-navy)]">{p.name}</div>
                    <div className="mt-1 text-xs font-light text-black/45">{p.area}</div>
                  </div>
                  <Badge tone={p.status === "For Sale" ? "active" : "pending"} label={p.status} />
                </div>
                <div className="mt-4 flex items-center justify-between">
                  <span className="font-serif text-lg font-light text-[var(--portal-navy)]">{p.price}</span>
                  <button type="button" className={btnSecondary}>
                    <Eye className="h-3.5 w-3.5" /> View
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      </Panel>

      <Panel variant="feature" heading="Review decision" eyebrow="NEXT STEP" className="mb-4">
        Review each section and mark KEEP / MODIFY / REJECT / LATER before promoting anything into the portal. Delete this folder when done.
      </Panel>
    </div>
  )
}


// ---------------------------------------------------------------------------
// Overlay demos — native, token-styled. No new dependency.
// ---------------------------------------------------------------------------

function DialogDemo() {
  const ref = useRef<HTMLDialogElement>(null)
  return (
    <>
      <button type="button" className={btnSecondary} onClick={() => ref.current?.showModal()}>
        Open dialog
      </button>
      <dialog
        ref={ref}
        onClick={(e) => {
          if (e.target === ref.current) ref.current?.close()
        }}
        className="m-auto w-[min(92vw,26rem)] rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white p-6 shadow-[var(--portal-panel-shadow)] backdrop:bg-[var(--portal-navy)]/40"
      >
        <h2 className="font-serif text-2xl font-light">Confirm archive</h2>
        <p className="mt-2 text-sm font-light leading-6 text-black/55">
          Archiving “Casa Palmera” hides it from the active portfolio. You can restore it later.
        </p>
        <div className="mt-6 flex flex-wrap justify-end gap-2">
          <button type="button" className={btnGhost} onClick={() => ref.current?.close()}>
            Cancel
          </button>
          <button type="button" className={btnDestructive} onClick={() => ref.current?.close()}>
            <Trash2 className="h-3.5 w-3.5" /> Archive
          </button>
        </div>
      </dialog>
    </>
  )
}

function DrawerDemo() {
  const [open, setOpen] = useState(false)
  return (
    <>
      <button type="button" className={btnSecondary} onClick={() => setOpen(true)}>
        Open drawer
      </button>
      {open ? (
        <div className="fixed inset-0 z-50">
          <div className="absolute inset-0 bg-[var(--portal-navy)]/40" onClick={() => setOpen(false)} />
          <aside className="absolute right-0 top-0 flex h-full w-[min(88vw,24rem)] flex-col bg-white shadow-[var(--portal-feature-shadow)]">
            <header className="flex items-center justify-between border-b border-[var(--portal-panel-border)] px-6 py-5">
              <h2 className="font-serif text-xl font-light">Quick actions</h2>
              <button type="button" className={btnIcon} onClick={() => setOpen(false)} aria-label="Close drawer">
                <XCircle className="h-4 w-4" />
              </button>
            </header>
            <div className="flex-1 space-y-3 overflow-y-auto px-6 py-5">
              {["New client", "New deal", "Create form", "Log interaction"].map((a) => (
                <button
                  key={a}
                  type="button"
                  onClick={() => setOpen(false)}
                  className="flex min-h-11 w-full items-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-3 text-left text-sm font-light text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]"
                >
                  <Plus className="h-3.5 w-3.5" />
                  {a}
                </button>
              ))}
            </div>
          </aside>
        </div>
      ) : null}
    </>
  )
}

function PopoverDemo() {
  const [open, setOpen] = useState(false)
  return (
    <div className="relative">
      <button
        type="button"
        className={btnSecondary}
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
      >
        Actions <ArrowUpRight className="h-3.5 w-3.5" />
      </button>
      {open ? (
        <div className="absolute left-0 top-full z-20 mt-2 w-52 rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white p-1.5 shadow-[var(--portal-panel-shadow)]">
          {[
            { label: "Open deal", icon: Eye },
            { label: "Duplicate", icon: FilePlus2 },
            { label: "Archive", icon: Trash2 },
          ].map(({ label, icon: Icon }) => (
            <button
              key={label}
              type="button"
              onClick={() => setOpen(false)}
              className="flex min-h-11 w-full items-center gap-2 rounded-md px-3 text-sm font-light text-[var(--portal-navy-soft)] transition hover:bg-[var(--portal-rail-hover-bg)] hover:text-[var(--portal-navy)]"
            >
              <Icon className="h-3.5 w-3.5 text-[var(--portal-blue-gray)]" />
              {label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  )
}

