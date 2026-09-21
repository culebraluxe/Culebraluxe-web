"use client"

import Link from "next/link"
import { useRef, useState } from "react"
import {
  Activity,
  AlertCircle,
  AlertTriangle,
  Building2,
  CalendarDays,
  CheckCircle2,
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
import { PortalCombobox } from "@/components/portal/ui/portal-combobox"
import {
  PortalDialog,
  PortalDialogClose,
} from "@/components/portal/ui/portal-dialog"
import {
  PortalField,
  PortalFieldDescription,
  PortalFieldError,
  PortalFieldLabel,
  PortalFieldset,
  PortalInput,
  PortalLegend,
  PortalSelect,
  PortalTextarea,
} from "@/components/portal/ui/portal-field"
import { PortalRowMenu } from "@/components/portal/ui/portal-row-menu"
import {
  PortalPagination,
  PortalTable,
  PortalTableBody,
  PortalTableCell,
  PortalTableHead,
  PortalTableHeader,
  PortalTableRow,
} from "@/components/portal/ui/portal-table"
import { ActivityTimeline, ProcessSteps } from "@/components/portal/ui/portal-timeline"

// ---------------------------------------------------------------------------
// TECH UI LAB — /portal/design-lab
//
// Permanent component gallery for evaluating shared portal UI before promotion.
// Owned by the TECH operating surface. Reuses the existing --portal-* tokens,
// the shared Panel primitive, the operating shell, and the glass rail
// treatment. No business logic or data queries — every example uses demo data.
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

const demoClients = [
  { value: "morgan-reed", label: "Morgan Reed", description: "Villa Rosada · Active buyer" },
  { value: "alvarez-trust", label: "The Alvarez Trust", description: "Casa Palmera · Offer" },
  { value: "jamie-ellis", label: "Jamie Ellis", description: "Blue Marlin House · Showing" },
  { value: "dana-whitfield", label: "Dana Whitfield", description: "Solana Cottage · Qualified" },
]

const demoProcessSteps = [
  { id: "offer", label: "Offer accepted", note: "Recorded on Oct 12", state: "complete" as const },
  { id: "attorney", label: "Attorney review", note: "Draft contract", state: "complete" as const },
  { id: "inspection", label: "Inspection", note: "Scheduled Oct 28", state: "current" as const },
  { id: "financing", label: "Financing clear to close", note: "Waiting on lender", state: "blocked" as const },
  { id: "closing", label: "Closing", note: "Target Nov 15", state: "upcoming" as const },
]

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
  const [tab, setTab] = useState("All")
  const [search, setSearch] = useState("")
  const [toggled, setToggled] = useState(false)
  const [clientName, setClientName] = useState("")
  const [clientError, setClientError] = useState("")
  const [tablePage, setTablePage] = useState(1)
  const [lastAction, setLastAction] = useState("No demo action selected yet.")
  const clientNameRef = useRef<HTMLInputElement>(null)
  const filteredDeals = demoDeals.filter((deal) =>
    `${deal.property} ${deal.client} ${deal.stage}`.toLowerCase().includes(search.toLowerCase()),
  )
  const tablePageCount = Math.max(1, Math.ceil(filteredDeals.length / 2))
  const currentTablePage = Math.min(tablePage, tablePageCount)
  const visibleDeals = filteredDeals.slice((currentTablePage - 1) * 2, currentTablePage * 2)

  return (
    <div className="mx-auto max-w-[1400px]">
      <div className="mb-6 flex flex-wrap items-center gap-3">
        <PageHeader
          eyebrow="TECH · UI LAB"
          title="Component Gallery"
          subtitle="Permanent TECH gallery for evaluating shared portal primitives and candidate components before promotion. Every example uses demo data only."
        />
        <span className="rounded-full border border-[var(--portal-gold)]/40 bg-[var(--portal-gold-pale)] px-3 py-1 text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-gold-muted)]">
          TECH · Demo data
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
        heading="Searchable selection + field states"
        subtitle="Independent portal primitives: explicit labels, descriptions, validation, touch targets, and a keyboard-searchable client selector. Demo data only."
        className="mb-6"
      >
        <div className="grid gap-6 lg:grid-cols-2">
          <PortalFieldset className="rounded-[var(--portal-panel-radius)] border border-[var(--portal-panel-border)] bg-white/55 p-5">
            <PortalLegend>Field contract</PortalLegend>
            <PortalFieldDescription>
              Submit the empty required field to inspect its error state and announcement.
            </PortalFieldDescription>
            <PortalField>
              <PortalFieldLabel htmlFor="lab-client-name">Client name</PortalFieldLabel>
              <PortalInput
                ref={clientNameRef}
                id="lab-client-name"
                value={clientName}
                onChange={(event) => {
                  setClientName(event.target.value)
                  if (clientError) setClientError("")
                }}
                aria-describedby={clientError ? "lab-client-name-error" : "lab-client-name-help"}
                aria-invalid={Boolean(clientError) || undefined}
                placeholder="Client name…"
              />
              {clientError ? (
                <PortalFieldError id="lab-client-name-error">{clientError}</PortalFieldError>
              ) : (
                <PortalFieldDescription id="lab-client-name-help">
                  Required only because this demo exercises validation.
                </PortalFieldDescription>
              )}
            </PortalField>
            <div className="grid gap-4 sm:grid-cols-2">
              <PortalField>
                <PortalFieldLabel htmlFor="lab-stage">Deal stage</PortalFieldLabel>
                <PortalSelect id="lab-stage" defaultValue="">
                  <option value="">All stages</option>
                  <option>Under Contract</option>
                  <option>Offer</option>
                  <option>Showing</option>
                </PortalSelect>
              </PortalField>
              <PortalField>
                <PortalFieldLabel htmlFor="lab-date">Next date</PortalFieldLabel>
                <PortalInput id="lab-date" type="date" />
              </PortalField>
            </div>
            <PortalField>
              <PortalFieldLabel htmlFor="lab-notes">Notes</PortalFieldLabel>
              <PortalTextarea id="lab-notes" placeholder="Relationship context…" />
            </PortalField>
            <button
              type="button"
              className={btnPrimary}
              onClick={() => {
                if (clientName.trim()) {
                  setClientError("")
                  setLastAction("Field validation passed.")
                  return
                }
                setClientError("Enter a client name to continue.")
                clientNameRef.current?.focus()
              }}
            >
              Validate fields
            </button>
          </PortalFieldset>

          <div className="space-y-6">
            <PortalCombobox
              label="Client"
              description="Type a name or use the arrow keys. Selection is restricted to known clients."
              options={demoClients}
              defaultValue="morgan-reed"
              onValueChange={(value) =>
                setLastAction(value ? `Selected client: ${value}` : "Cleared client selection.")
              }
            />

            <PortalField>
              <PortalFieldLabel htmlFor="lab-search">Search deals</PortalFieldLabel>
              <div className="relative">
                <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-black/35" />
                <PortalInput
                  id="lab-search"
                  className="pl-9"
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="Search deals…"
                />
              </div>
            </PortalField>

            <div>
              <p className={`${labelCls} mb-2`}>Checkbox + toggle</p>
              <div className="flex flex-wrap items-center gap-6">
                <label className="flex min-h-11 items-center gap-2 text-sm font-light text-[var(--portal-text)]">
                  <input type="checkbox" className="h-4 w-4 accent-[var(--portal-navy)]" defaultChecked />
                  Include archived
                </label>
                <button type="button" onClick={() => setToggled((value) => !value)} aria-pressed={toggled} className="flex min-h-11 items-center gap-2 text-sm font-light text-[var(--portal-text)]">
                  <span className={["relative h-6 w-11 rounded-full transition-colors", toggled ? "bg-[var(--portal-navy)]" : "bg-[var(--portal-mist-4)]"].join(" ")}>
                    <span className={["absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-transform", toggled ? "translate-x-[22px]" : "translate-x-0.5"].join(" ")} />
                  </span>
                  Auto-assign
                </button>
              </div>
            </div>

            <div>
              <p className={`${labelCls} mb-2`}>Compact filter row</p>
              <div className="flex flex-wrap items-end gap-3">
                <PortalField className="w-full max-w-[200px]">
                  <PortalFieldLabel htmlFor="lab-status">Status</PortalFieldLabel>
                  <PortalSelect id="lab-status" defaultValue="all">
                    <option value="all">All</option>
                    <option>Active</option>
                    <option>Blocked</option>
                  </PortalSelect>
                </PortalField>
                <button type="button" className={btnSecondary}>Apply filters</button>
                <button type="button" className={btnGhost}>Reset</button>
              </div>
            </div>

            <p aria-live="polite" className="text-xs font-light text-black/45">
              {lastAction}
            </p>
          </div>
        </div>
      </Panel>


      {/* 7 — TABLE / LIST */}
      <Panel
        eyebrow="07 · TABLE"
        heading="Operational table + rare-action menu"
        subtitle="Readable columns, aligned numbers, bounded row actions, and pagination. The existing search field filters this fake data; mobile switches to compact cards instead of crushing columns."
        className="mb-6"
        flush
      >
        <div className="hidden md:block">
          <PortalTable>
            <PortalTableHead>
              <PortalTableRow className="hover:bg-transparent">
                {[
                  { label: "Property" },
                  { label: "Client" },
                  { label: "Stage" },
                  { label: "Value", className: "text-right" },
                  { label: "Next milestone" },
                  { label: "Status" },
                  { label: "Actions", className: "w-16 text-right" },
                ].map((heading) => (
                  <PortalTableHeader key={heading.label} className={heading.className}>
                    {heading.label}
                  </PortalTableHeader>
                ))}
              </PortalTableRow>
            </PortalTableHead>
            <PortalTableBody>
              {visibleDeals.length ? (
                visibleDeals.map((deal) => (
                  <PortalTableRow key={deal.id}>
                    <PortalTableCell className="font-medium text-[var(--portal-navy)]">
                      {deal.property}
                    </PortalTableCell>
                    <PortalTableCell>{deal.client}</PortalTableCell>
                    <PortalTableCell>{deal.stage}</PortalTableCell>
                    <PortalTableCell className="text-right tabular-nums text-black/70">
                      {deal.value}
                    </PortalTableCell>
                    <PortalTableCell className="text-black/50">{deal.next}</PortalTableCell>
                    <PortalTableCell>
                      <Badge tone={deal.status} label={deal.status} />
                    </PortalTableCell>
                    <PortalTableCell className="text-right">
                      <PortalRowMenu
                        ariaLabel={`Actions for ${deal.property}`}
                        items={[
                          {
                            label: "Open deal",
                            icon: Eye,
                            onSelect: () => setLastAction(`Opened ${deal.property}.`),
                          },
                          {
                            label: "Duplicate",
                            icon: FilePlus2,
                            onSelect: () => setLastAction(`Prepared a copy of ${deal.property}.`),
                          },
                          {
                            label: "Archive",
                            icon: Trash2,
                            tone: "danger",
                            onSelect: () => setLastAction(`Archive selected for ${deal.property}.`),
                          },
                        ]}
                      />
                    </PortalTableCell>
                  </PortalTableRow>
                ))
              ) : (
                <PortalTableRow>
                  <PortalTableCell colSpan={7} className="py-10 text-center text-black/45">
                    No demo deals match “{search}”.
                  </PortalTableCell>
                </PortalTableRow>
              )}
            </PortalTableBody>
          </PortalTable>
        </div>
        <div className="divide-y divide-[var(--portal-border)] md:hidden">
          {visibleDeals.length ? (
            visibleDeals.map((deal) => (
              <article key={deal.id} className="space-y-3 px-4 py-4">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <h3 className="font-serif text-lg font-light text-[var(--portal-navy)]">
                      {deal.property}
                    </h3>
                    <p className="mt-0.5 truncate text-xs font-light text-black/45">{deal.client}</p>
                  </div>
                  <PortalRowMenu
                    ariaLabel={`Actions for ${deal.property}`}
                    items={[
                      { label: "Open deal", icon: Eye, onSelect: () => setLastAction(`Opened ${deal.property}.`) },
                      { label: "Duplicate", icon: FilePlus2, onSelect: () => setLastAction(`Prepared a copy of ${deal.property}.`) },
                      { label: "Archive", icon: Trash2, tone: "danger", onSelect: () => setLastAction(`Archive selected for ${deal.property}.`) },
                    ]}
                  />
                </div>
                <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Stage</dt>
                    <dd className="mt-1 font-light text-black/65">{deal.stage}</dd>
                  </div>
                  <div className="text-right">
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Value</dt>
                    <dd className="mt-1 tabular-nums font-light text-black/70">{deal.value}</dd>
                  </div>
                  <div>
                    <dt className="font-light uppercase tracking-[0.12em] text-black/35">Next</dt>
                    <dd className="mt-1 font-light text-black/65">{deal.next}</dd>
                  </div>
                  <div className="text-right">
                    <dt className="sr-only">Status</dt>
                    <dd><Badge tone={deal.status} label={deal.status} /></dd>
                  </div>
                </dl>
              </article>
            ))
          ) : (
            <p className="px-4 py-10 text-center text-sm font-light text-black/45">
              No demo deals match “{search}”.
            </p>
          )}
        </div>
        <PortalPagination
          page={currentTablePage}
          pageCount={tablePageCount}
          totalLabel={`${filteredDeals.length} demo deals`}
          onPrevious={() => setTablePage((page) => Math.max(1, page - 1))}
          onNext={() => setTablePage((page) => Math.min(tablePageCount, page + 1))}
        />
        <p aria-live="polite" className="border-t border-[var(--portal-panel-border)] px-4 py-3 text-xs font-light text-black/45 sm:px-6">
          {lastAction}
        </p>
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
        heading="Activity timeline"
        subtitle="A reusable presentation primitive for source-driven relationship evidence: timestamp, actor, event, source context, and restrained status tone."
        className="mb-6"
      >
        <ActivityTimeline
          items={demoActivity.map((activity, index) => ({
            id: `activity-${index}`,
            actor: activity.actor,
            detail: activity.detail,
            icon:
              activity.kind === "call"
                ? User
                : activity.kind === "doc"
                  ? FileText
                  : activity.kind === "cal"
                    ? CalendarDays
                    : AlertCircle,
            text: activity.text,
            timestamp: activity.at,
            tone: activity.kind === "alert" ? ("attention" as const) : ("default" as const),
          }))}
        />
      </Panel>

      {/* 10 — WORKFLOW / PROCESS */}
      <Panel
        eyebrow="10 · PROCESS"
        heading="Process steps"
        subtitle="Completed, current, upcoming, and blocked states. The component explains an existing workflow; it does not create another workflow engine."
        className="mb-6"
      >
        <ProcessSteps steps={demoProcessSteps} />
      </Panel>

      {/* 11 — DIALOG / MENU */}
      <Panel
        eyebrow="11 · OVERLAYS"
        heading="Critical dialog + rare-action menu"
        subtitle="Two bounded jobs only: confirm a consequential action or expose infrequent row actions. The earlier global drawer candidate is intentionally rejected."
        className="mb-6"
      >
        <div className="flex flex-wrap items-center gap-3">
          <PortalDialog
            trigger="Open confirmation"
            title="Confirm archive"
            description="Archiving Casa Palmera hides it from the active portfolio. You can restore it later."
            actions={
              <>
                <PortalDialogClose className={btnGhost}>Cancel</PortalDialogClose>
                <PortalDialogClose
                  className={btnDestructive}
                  onClick={() => setLastAction("Archive confirmed for Casa Palmera.")}
                >
                  <Trash2 className="h-3.5 w-3.5" /> Archive
                </PortalDialogClose>
              </>
            }
          >
            <div className="rounded-[var(--portal-tab-radius)] bg-[var(--portal-archive-pale)] p-4 text-sm font-light leading-6 text-[var(--portal-text)]/75">
              This is deliberately narrow: no form wizard, no drawer, and no secondary navigation.
            </div>
          </PortalDialog>
          <PortalRowMenu
            ariaLabel="Demo actions"
            items={[
              { label: "Open deal", icon: Eye, onSelect: () => setLastAction("Open deal selected.") },
              { label: "Duplicate", icon: FilePlus2, onSelect: () => setLastAction("Duplicate selected.") },
              { label: "Archive", icon: Trash2, tone: "danger", onSelect: () => setLastAction("Archive selected.") },
            ]}
          />
          <span aria-live="polite" className="text-xs font-light text-black/45">{lastAction}</span>
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
        Review each section and mark KEEP / MODIFY / REJECT / LATER before promoting anything into the portal. This gallery remains in TECH as the durable visual reference.
      </Panel>
    </div>
  )
}
