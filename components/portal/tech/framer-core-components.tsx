"use client"

import {
  AlertTriangle,
  ArrowUpDown,
  Check,
  ChevronDown,
  CircleDollarSign,
  Clock3,
  Command,
  FileText,
  Files,
  LayoutDashboard,
  Mail,
  Phone,
  Plus,
  Search,
  Sparkles,
  Users,
} from "lucide-react"
import { useEffect, useRef } from "react"

import {
  FRAMER_LAB_COMMANDS,
  FRAMER_LAB_DEALS,
  FRAMER_LAB_PROGRESS_STEPS,
  type FramerLabCoreCardId,
  type FramerLabCoreTab,
  type FramerLabDeal,
  type FramerLabFilterStatus,
  type FramerLabQuickAction,
  type FramerLabSortKey,
  type FramerUiLabController,
  type FramerUiLabPageModel,
} from "@/ui/framer-ui-lab"

type FramerCoreComponentsProps = {
  controller: FramerUiLabController
  model: FramerUiLabPageModel
}

const CORE_TABS: readonly {
  id: FramerLabCoreTab
  label: string
  icon: typeof LayoutDashboard
}[] = [
  { id: "overview", label: "Overview", icon: LayoutDashboard },
  { id: "activity", label: "Activity", icon: Clock3 },
  { id: "documents", label: "Documents", icon: Files },
]

const TAB_CONTENT: Record<
  FramerLabCoreTab,
  { eyebrow: string; title: string; body: string; primary: string; secondary: string }
> = {
  overview: {
    eyebrow: "At a glance",
    title: "Villa Rosada",
    body: "One compact state switcher can replace a row of visually unrelated page tabs.",
    primary: "72% ready",
    secondary: "3 open items",
  },
  activity: {
    eyebrow: "Recent movement",
    title: "4 meaningful touches",
    body: "The content swaps without changing the frame, preserving spatial continuity.",
    primary: "18 min ago",
    secondary: "2-way contact",
  },
  documents: {
    eyebrow: "Deal file",
    title: "12 documents",
    body: "The active indicator remains the anchor while the underlying model changes.",
    primary: "10 complete",
    secondary: "2 pending",
  },
}

const CORE_CARDS: readonly {
  id: FramerLabCoreCardId
  kicker: string
  title: string
  summary: string
  detail: string
  icon: typeof Users
}[] = [
  {
    id: "client-brief",
    kicker: "People",
    title: "Client brief",
    summary: "Identity, relationship signal and the one fact that matters now.",
    detail:
      "Use this pattern in Clients and Cockpit when a dense row needs one deeper layer without opening a modal or stealing the whole screen.",
    icon: Users,
  },
  {
    id: "deal-readiness",
    kicker: "Transaction",
    title: "Deal readiness",
    summary: "A compact roll-up of blockers, signatures and launch readiness.",
    detail:
      "The reveal keeps the summary scannable while making the supporting facts available in place. It is a good fit for Projects, Contracts and Catch-Up.",
    icon: CircleDollarSign,
  },
  {
    id: "next-action",
    kicker: "Attention",
    title: "Next best action",
    summary: "One recommended operational move with its evidence attached.",
    detail:
      "This is deliberately a disclosure surface, not an autonomous action. The user stays in control and can inspect why the action is being surfaced.",
    icon: Sparkles,
  },
]

const QUICK_ACTIONS: readonly {
  id: FramerLabQuickAction
  label: string
  icon: typeof Phone
}[] = [
  { id: "call", label: "Call", icon: Phone },
  { id: "email", label: "Email", icon: Mail },
  { id: "task", label: "Task", icon: Plus },
  { id: "document", label: "Document", icon: FileText },
]

function statusClasses(status: FramerLabDeal["status"]) {
  if (status === "attention") return "border-amber-300/30 bg-amber-300/10 text-amber-100"
  if (status === "closed") return "border-white/10 bg-white/[0.04] text-white/42"
  return "border-emerald-300/25 bg-emerald-300/10 text-emerald-100"
}

function compareDeals(left: FramerLabDeal, right: FramerLabDeal, key: FramerLabSortKey) {
  if (key === "value") return left.value - right.value
  return left[key].localeCompare(right[key])
}

function CandidateHeader({
  number,
  title,
  description,
  promote,
}: {
  number: string
  title: string
  description: string
  promote: string
}) {
  return (
    <div className="mb-5 flex flex-col gap-3 border-b border-white/10 pb-5 lg:flex-row lg:items-end lg:justify-between">
      <div>
        <p className="text-xs font-medium uppercase tracking-[0.2em] text-[#c6a15b]">
          {number}
        </p>
        <h3 className="mt-2 font-serif text-3xl font-light text-white">{title}</h3>
        <p className="mt-2 max-w-3xl text-[15px] font-light leading-6 text-white/58">
          {description}
        </p>
      </div>
      <div className="w-fit rounded-full border border-[#c6a15b]/25 bg-[#c6a15b]/10 px-3 py-2 text-xs text-[#e8c983]">
        Promote → {promote}
      </div>
    </div>
  )
}

export function FramerCoreComponents({
  controller,
  model,
}: FramerCoreComponentsProps) {
  const commandInputRef = useRef<HTMLInputElement | null>(null)

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault()
        void controller.dispatch({
          operation: "framerLab.setCommandPaletteOpen",
          payload: { open: !model.commandPaletteOpen },
        })
        return
      }

      if (event.key === "Escape" && model.commandPaletteOpen) {
        void controller.dispatch({
          operation: "framerLab.setCommandPaletteOpen",
          payload: { open: false },
        })
      }
    }

    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [controller, model.commandPaletteOpen])

  useEffect(() => {
    if (!model.commandPaletteOpen) return
    const frame = window.requestAnimationFrame(() => commandInputRef.current?.focus())
    return () => window.cancelAnimationFrame(frame)
  }, [model.commandPaletteOpen])

  const commandNeedle = model.commandQuery.trim().toLowerCase()
  const matchingCommands = FRAMER_LAB_COMMANDS.filter((command) => {
    if (!commandNeedle) return true
    return [command.label, command.detail, command.group]
      .join(" ")
      .toLowerCase()
      .includes(commandNeedle)
  })

  const filterNeedle = model.filterQuery.trim().toLowerCase()
  const filteredDeals = FRAMER_LAB_DEALS.filter((deal) => {
    const matchesStatus =
      model.dealStatusFilter === "all" || deal.status === model.dealStatusFilter
    const matchesQuery =
      !filterNeedle ||
      [deal.client, deal.property, deal.stage, deal.valueLabel]
        .join(" ")
        .toLowerCase()
        .includes(filterNeedle)
    return matchesStatus && matchesQuery
  })

  const sortedDeals = [...filteredDeals].sort((left, right) => {
    const value = compareDeals(left, right, model.tableSortKey)
    return model.tableSortDirection === "asc" ? value : -value
  })

  const selectedDeal =
    FRAMER_LAB_DEALS.find((deal) => deal.id === model.selectedDealId) ??
    FRAMER_LAB_DEALS[0]
  const selectedStep =
    FRAMER_LAB_PROGRESS_STEPS.find(
      (step) => step.id === model.selectedProgressStepId,
    ) ?? FRAMER_LAB_PROGRESS_STEPS[0]
  const activeTabIndex = Math.max(
    0,
    CORE_TABS.findIndex((tab) => tab.id === model.coreTab),
  )
  const activeTab = TAB_CONTENT[model.coreTab]

  function sortLabel(key: FramerLabSortKey) {
    if (model.tableSortKey !== key) return ""
    return model.tableSortDirection === "asc" ? " ↑" : " ↓"
  }

  return (
    <section className="mt-16 border-t border-white/12 pt-12">
      <div className="mb-10 grid gap-4 xl:grid-cols-[0.78fr_1.22fr] xl:items-end">
        <div>
          <p className="text-xs font-medium uppercase tracking-[0.22em] text-[#c6a15b]">
            CORE COMPONENT LAB
          </p>
          <h2 className="mt-2 font-serif text-4xl font-light text-white sm:text-5xl">
            Useful primitives, not decoration.
          </h2>
        </div>
        <p className="max-w-3xl text-[15px] font-light leading-7 text-white/58">
          These candidates translate the best current Framer interaction ideas
          into operational CulebraLuxe building blocks. Every meaningful state
          below is owned by the same page MVI controller, so an approved pattern
          can move into the shared component layer without changing the state model.
        </p>
      </div>

      <div className="grid gap-6">
        <article className="rounded-[24px] border border-white/10 bg-white/[0.045] p-6 sm:p-8">
          <CandidateHeader
            number="07 · COMMAND PALETTE"
            title="One keyboard surface for the whole portal."
            description="A Spotlight / Linear-style command surface is much more useful here than another animated hero. Use ⌘K or Ctrl+K to try it."
            promote="PortalCommandPalette"
          />

          <div className="grid gap-5 lg:grid-cols-[0.8fr_1.2fr]">
            <div className="rounded-[20px] border border-white/10 bg-black/15 p-5">
              <p className="text-sm font-medium text-white">Global trigger</p>
              <p className="mt-2 text-sm leading-6 text-white/48">
                Navigation, creation and find actions share one searchable surface.
                The production version could bind to your canonical command registry
                instead of hard-coded fixture commands.
              </p>
              <button
                type="button"
                onClick={() =>
                  void controller.dispatch({
                    operation: "framerLab.setCommandPaletteOpen",
                    payload: { open: true },
                  })
                }
                className="mt-5 flex min-h-12 w-full items-center justify-between rounded-[14px] border border-white/12 bg-white/[0.06] px-4 text-left transition hover:border-[#c6a15b]/45 hover:bg-white/[0.09]"
              >
                <span className="flex items-center gap-3 text-sm text-white/80">
                  <Search className="h-4 w-4 text-[#c6a15b]" />
                  Search or run a command
                </span>
                <span className="rounded-md border border-white/10 bg-black/20 px-2 py-1 text-xs text-white/45">
                  ⌘ K
                </span>
              </button>
            </div>

            <div className="grid gap-3 sm:grid-cols-3">
              {[
                ["Navigate", "Jump anywhere without traversing the nav tree."],
                ["Create", "Start client, task or document flows from context."],
                ["Find", "Search portal entities through one consistent shell."],
              ].map(([title, body]) => (
                <div
                  key={title}
                  className="rounded-[18px] border border-white/10 bg-black/10 p-4"
                >
                  <Command className="h-4 w-4 text-[#c6a15b]" />
                  <p className="mt-4 text-base font-medium text-white">{title}</p>
                  <p className="mt-2 text-sm leading-5 text-white/45">{body}</p>
                </div>
              ))}
            </div>
          </div>
        </article>

        <article className="rounded-[24px] border border-white/10 bg-white/[0.045] p-6 sm:p-8">
          <CandidateHeader
            number="08 · SEGMENTED SWITCHER"
            title="Tabs that feel like one object."
            description="Framer-style variants are especially good for compact state switches. The active surface glides instead of each tab behaving like an unrelated button."
            promote="PortalSegmentedTabs"
          />

          <div className="grid gap-6 lg:grid-cols-[0.72fr_1.28fr]">
            <div>
              <div
                role="tablist"
                aria-label="Core component tab demo"
                className="relative grid grid-cols-3 rounded-[16px] border border-white/10 bg-black/15 p-1"
              >
                <span
                  aria-hidden="true"
                  className="absolute bottom-1 left-1 top-1 rounded-[12px] border border-white/12 bg-white/[0.12] shadow-lg transition-transform duration-300 ease-out"
                  style={{
                    width: "calc((100% - 8px) / 3)",
                    transform: "translateX(" + String(activeTabIndex * 100) + "%)",
                  }}
                />
                {CORE_TABS.map((tab) => {
                  const Icon = tab.icon
                  const active = model.coreTab === tab.id
                  return (
                    <button
                      key={tab.id}
                      type="button"
                      role="tab"
                      aria-selected={active}
                      onClick={() =>
                        void controller.dispatch({
                          operation: "framerLab.selectCoreTab",
                          payload: { tab: tab.id },
                        })
                      }
                      className={
                        "relative z-10 flex min-h-11 items-center justify-center gap-2 rounded-[12px] px-3 text-sm transition " +
                        (active ? "text-white" : "text-white/42 hover:text-white/75")
                      }
                    >
                      <Icon className="h-4 w-4" />
                      <span className="hidden sm:inline">{tab.label}</span>
                    </button>
                  )
                })}
              </div>
              <p className="mt-3 text-sm leading-6 text-white/42">
                Good fit for project center tabs, client subviews and scoped
                document/activity switching.
              </p>
            </div>

            <div className="rounded-[20px] border border-[#c6a15b]/18 bg-[#c6a15b]/[0.07] p-6">
              <p className="text-xs font-medium uppercase tracking-[0.18em] text-[#c6a15b]">
                {activeTab.eyebrow}
              </p>
              <div className="mt-3 flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
                <div>
                  <h4 className="font-serif text-3xl font-light text-white">
                    {activeTab.title}
                  </h4>
                  <p className="mt-2 max-w-xl text-sm leading-6 text-white/52">
                    {activeTab.body}
                  </p>
                </div>
                <div className="flex gap-2">
                  <span className="rounded-full border border-white/10 bg-black/15 px-3 py-2 text-xs text-white/65">
                    {activeTab.primary}
                  </span>
                  <span className="rounded-full border border-white/10 bg-black/15 px-3 py-2 text-xs text-white/45">
                    {activeTab.secondary}
                  </span>
                </div>
              </div>
            </div>
          </div>
        </article>

        <article className="rounded-[24px] border border-white/10 bg-white/[0.045] p-6 sm:p-8">
          <CandidateHeader
            number="09 · SEARCH + FILTER BAR"
            title="One reusable command strip for dense work."
            description="Live search, state chips and result count belong together. This is a stronger primitive for Clients, Contracts, Cabinet and project lists than page-specific filter controls."
            promote="PortalFilterBar"
          />

          <div className="flex flex-col gap-3 rounded-[18px] border border-white/10 bg-black/15 p-3 lg:flex-row lg:items-center">
            <label className="flex min-h-12 flex-1 items-center gap-3 rounded-[13px] border border-white/10 bg-white/[0.055] px-4">
              <Search className="h-4 w-4 shrink-0 text-[#c6a15b]" />
              <input
                value={model.filterQuery}
                onChange={(event) =>
                  void controller.dispatch({
                    operation: "framerLab.setFilterQuery",
                    payload: { query: event.target.value },
                  })
                }
                placeholder="Search client, property, stage..."
                className="w-full bg-transparent text-sm text-white outline-none placeholder:text-white/30"
              />
            </label>

            <div className="flex flex-wrap items-center gap-2">
              {(["all", "active", "attention", "closed"] as const).map((status) => {
                const active = model.dealStatusFilter === status
                return (
                  <button
                    key={status}
                    type="button"
                    aria-pressed={active}
                    onClick={() =>
                      void controller.dispatch({
                        operation: "framerLab.selectDealStatus",
                        payload: { status },
                      })
                    }
                    className={
                      "min-h-10 rounded-full border px-3 text-sm capitalize transition " +
                      (active
                        ? "border-[#c6a15b]/55 bg-[#c6a15b]/15 text-white"
                        : "border-white/10 bg-white/[0.035] text-white/45 hover:text-white/75")
                    }
                  >
                    {status}
                  </button>
                )
              })}
            </div>

            <div className="shrink-0 px-2 text-sm text-white/38">
              {sortedDeals.length} result{sortedDeals.length === 1 ? "" : "s"}
            </div>
          </div>
        </article>

        <article className="rounded-[24px] border border-white/10 bg-white/[0.045] p-6 sm:p-8">
          <CandidateHeader
            number="10 · DENSE DATA TABLE"
            title="A table that still feels premium."
            description="Search and filters above feed this table. Sorting, row selection and status are subtle enough for operational density without falling back to generic admin UI."
            promote="PortalDataTable"
          />

          <div className="overflow-hidden rounded-[18px] border border-white/10 bg-black/10">
            <div className="hidden overflow-x-auto md:block">
              <table className="w-full min-w-[760px] border-collapse text-left">
                <thead>
                  <tr className="border-b border-white/10 bg-white/[0.035]">
                    {([
                      ["client", "Client"],
                      ["property", "Property"],
                      ["stage", "Stage"],
                      ["value", "Value"],
                    ] as const).map(([key, label]) => (
                      <th key={key} className="px-4 py-3 text-xs font-medium text-white/45">
                        <button
                          type="button"
                          onClick={() =>
                            void controller.dispatch({
                              operation: "framerLab.sortDeals",
                              payload: { key },
                            })
                          }
                          className="inline-flex items-center gap-2 hover:text-white"
                        >
                          {label}
                          <ArrowUpDown className="h-3.5 w-3.5" />
                          <span className="sr-only">{sortLabel(key)}</span>
                        </button>
                      </th>
                    ))}
                    <th className="px-4 py-3 text-xs font-medium text-white/45">
                      Status
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-medium text-white/45">
                      Last touch
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {sortedDeals.map((deal) => {
                    const selected = selectedDeal.id === deal.id
                    return (
                      <tr
                        key={deal.id}
                        className={
                          "border-b border-white/[0.07] transition last:border-0 " +
                          (selected ? "bg-[#c6a15b]/[0.08]" : "hover:bg-white/[0.035]")
                        }
                      >
                        <td className="px-4 py-4">
                          <button
                            type="button"
                            onClick={() =>
                              void controller.dispatch({
                                operation: "framerLab.selectDeal",
                                payload: { dealId: deal.id },
                              })
                            }
                            className="text-[15px] font-medium text-white hover:text-[#e8c983]"
                          >
                            {deal.client}
                          </button>
                        </td>
                        <td className="px-4 py-4 text-sm text-white/58">{deal.property}</td>
                        <td className="px-4 py-4 text-sm text-white/58">{deal.stage}</td>
                        <td className="px-4 py-4 text-sm font-medium text-white">{deal.valueLabel}</td>
                        <td className="px-4 py-4">
                          <span
                            className={
                              "inline-flex rounded-full border px-2.5 py-1 text-xs capitalize " +
                              statusClasses(deal.status)
                            }
                          >
                            {deal.status}
                          </span>
                        </td>
                        <td className="px-4 py-4 text-right text-sm text-white/38">{deal.lastTouch}</td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>

            <div className="grid gap-2 p-2 md:hidden">
              {sortedDeals.map((deal) => {
                const selected = selectedDeal.id === deal.id
                return (
                  <button
                    key={deal.id}
                    type="button"
                    onClick={() =>
                      void controller.dispatch({
                        operation: "framerLab.selectDeal",
                        payload: { dealId: deal.id },
                      })
                    }
                    className={
                      "rounded-[14px] border p-4 text-left transition " +
                      (selected
                        ? "border-[#c6a15b]/35 bg-[#c6a15b]/10"
                        : "border-white/8 bg-white/[0.03]")
                    }
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div>
                        <p className="text-base font-medium text-white">{deal.client}</p>
                        <p className="mt-1 text-sm text-white/48">{deal.property}</p>
                      </div>
                      <span className="text-sm font-medium text-white">{deal.valueLabel}</span>
                    </div>
                    <div className="mt-3 flex items-center justify-between">
                      <span
                        className={
                          "rounded-full border px-2.5 py-1 text-xs capitalize " +
                          statusClasses(deal.status)
                        }
                      >
                        {deal.status}
                      </span>
                      <span className="text-xs text-white/35">{deal.stage} · {deal.lastTouch}</span>
                    </div>
                  </button>
                )
              })}
            </div>

            {sortedDeals.length === 0 ? (
              <div className="p-10 text-center">
                <Search className="mx-auto h-5 w-5 text-white/25" />
                <p className="mt-3 text-sm text-white/45">No deals match this filter.</p>
              </div>
            ) : null}
          </div>

          <div className="mt-4 flex flex-wrap items-center gap-2 text-sm text-white/45">
            <span>Selected:</span>
            <span className="rounded-full border border-[#c6a15b]/25 bg-[#c6a15b]/10 px-3 py-1.5 text-white/75">
              {selectedDeal.client} · {selectedDeal.property}
            </span>
          </div>
        </article>

        <article className="rounded-[24px] border border-white/10 bg-white/[0.045] p-6 sm:p-8">
          <CandidateHeader
            number="11 · PROGRESS RAIL"
            title="Workflow state without a giant project diagram."
            description="A compact step rail gives you a reusable summary for listing, signature, onboarding and transaction progress. It can sit inside a card or stretch across a detail pane."
            promote="PortalProgressRail"
          />

          <div className="grid gap-6 xl:grid-cols-[1.3fr_0.7fr]">
            <div className="grid gap-2 sm:grid-cols-5">
              {FRAMER_LAB_PROGRESS_STEPS.map((step, index) => {
                const selected = selectedStep.id === step.id
                const complete = step.state === "complete"
                const current = step.state === "current"
                return (
                  <button
                    key={step.id}
                    type="button"
                    onClick={() =>
                      void controller.dispatch({
                        operation: "framerLab.selectProgressStep",
                        payload: { stepId: step.id },
                      })
                    }
                    className={
                      "group relative rounded-[16px] border p-4 text-left transition " +
                      (selected
                        ? "border-[#c6a15b]/45 bg-[#c6a15b]/10"
                        : "border-white/10 bg-black/10 hover:bg-white/[0.04]")
                    }
                  >
                    <div
                      className={
                        "grid h-8 w-8 place-items-center rounded-full border text-xs font-medium " +
                        (complete
                          ? "border-emerald-300/30 bg-emerald-300/10 text-emerald-100"
                          : current
                            ? "border-[#c6a15b]/45 bg-[#c6a15b]/15 text-[#e8c983]"
                            : "border-white/10 bg-white/[0.035] text-white/35")
                      }
                    >
                      {complete ? <Check className="h-4 w-4" /> : index + 1}
                    </div>
                    <p className="mt-3 text-sm font-medium text-white">{step.label}</p>
                    <p className="mt-1 text-xs capitalize text-white/35">{step.state}</p>
                  </button>
                )
              })}
            </div>

            <div className="rounded-[18px] border border-white/10 bg-black/15 p-5">
              <div className="flex items-start gap-3">
                {selectedStep.state === "current" ? (
                  <Clock3 className="mt-0.5 h-4 w-4 text-[#c6a15b]" />
                ) : selectedStep.state === "complete" ? (
                  <Check className="mt-0.5 h-4 w-4 text-emerald-200" />
                ) : (
                  <AlertTriangle className="mt-0.5 h-4 w-4 text-white/35" />
                )}
                <div>
                  <p className="text-base font-medium text-white">{selectedStep.label}</p>
                  <p className="mt-2 text-sm leading-6 text-white/48">{selectedStep.detail}</p>
                </div>
              </div>
            </div>
          </div>
        </article>

        <article className="rounded-[24px] border border-white/10 bg-white/[0.045] p-6 sm:p-8">
          <CandidateHeader
            number="12 · DISCLOSURE CARDS + QUICK ACTIONS"
            title="More information without another pane."
            description="Framer's expand/reveal pattern is useful when it protects density. Pair it with a compact action dock and you get a strong primitive for Clients, Cockpit and Project detail."
            promote="PortalDisclosureCard + PortalQuickActions"
          />

          <div className="grid gap-6 xl:grid-cols-[1.2fr_0.8fr]">
            <div className="grid gap-3">
              {CORE_CARDS.map((card) => {
                const Icon = card.icon
                const expanded = model.expandedCoreCardId === card.id
                return (
                  <div
                    key={card.id}
                    className={
                      "overflow-hidden rounded-[18px] border transition " +
                      (expanded
                        ? "border-[#c6a15b]/35 bg-[#c6a15b]/[0.07]"
                        : "border-white/10 bg-black/10")
                    }
                  >
                    <button
                      type="button"
                      aria-expanded={expanded}
                      onClick={() =>
                        void controller.dispatch({
                          operation: "framerLab.toggleCoreCard",
                          payload: { cardId: card.id },
                        })
                      }
                      className="flex w-full items-center gap-4 p-5 text-left"
                    >
                      <span className="grid h-10 w-10 shrink-0 place-items-center rounded-[12px] border border-white/10 bg-white/[0.05]">
                        <Icon className="h-4 w-4 text-[#c6a15b]" />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="text-xs uppercase tracking-[0.16em] text-white/32">
                          {card.kicker}
                        </span>
                        <span className="mt-1 block text-base font-medium text-white">{card.title}</span>
                        <span className="mt-1 block text-sm leading-5 text-white/45">{card.summary}</span>
                      </span>
                      <ChevronDown
                        className={
                          "h-4 w-4 shrink-0 text-white/35 transition-transform duration-300 " +
                          (expanded ? "rotate-180" : "")
                        }
                      />
                    </button>
                    <div
                      className={
                        "grid transition-[grid-template-rows,opacity] duration-300 " +
                        (expanded ? "grid-rows-[1fr] opacity-100" : "grid-rows-[0fr] opacity-0")
                      }
                    >
                      <div className="overflow-hidden">
                        <p className="mx-5 mb-5 border-t border-white/10 pt-4 text-sm leading-6 text-white/52">
                          {card.detail}
                        </p>
                      </div>
                    </div>
                  </div>
                )
              })}
            </div>

            <div className="rounded-[20px] border border-white/10 bg-black/15 p-5">
              <p className="text-xs font-medium uppercase tracking-[0.18em] text-[#c6a15b]">
                Contextual action dock
              </p>
              <h4 className="mt-2 font-serif text-2xl font-light text-white">
                Keep frequent actions one reach away.
              </h4>
              <p className="mt-2 text-sm leading-6 text-white/45">
                Compact enough to live under a client or deal summary. The selected
                action is MVI-owned here so the visual state can survive a Rust port.
              </p>

              <div className="mt-6 grid grid-cols-4 gap-2 rounded-[18px] border border-white/10 bg-white/[0.035] p-2">
                {QUICK_ACTIONS.map((action) => {
                  const Icon = action.icon
                  const active = model.selectedQuickAction === action.id
                  return (
                    <button
                      key={action.id}
                      type="button"
                      aria-pressed={active}
                      title={action.label}
                      onClick={() =>
                        void controller.dispatch({
                          operation: "framerLab.selectQuickAction",
                          payload: { action: action.id },
                        })
                      }
                      className={
                        "flex min-h-16 flex-col items-center justify-center gap-1.5 rounded-[13px] border text-xs transition " +
                        (active
                          ? "border-[#c6a15b]/45 bg-[#c6a15b]/15 text-white"
                          : "border-transparent text-white/42 hover:border-white/10 hover:bg-white/[0.05] hover:text-white/75")
                      }
                    >
                      <Icon className="h-4 w-4" />
                      {action.label}
                    </button>
                  )
                })}
              </div>

              <div className="mt-4 rounded-[14px] border border-[#c6a15b]/20 bg-[#c6a15b]/[0.07] px-4 py-3 text-sm text-white/62">
                Selected action:{" "}
                <span className="font-medium capitalize text-[#e8c983]">
                  {model.selectedQuickAction}
                </span>
              </div>
            </div>
          </div>
        </article>
      </div>

      {model.commandPaletteOpen ? (
        <div className="fixed inset-0 z-[120] grid place-items-start px-4 pt-[12vh] sm:px-6">
          <button
            type="button"
            aria-label="Close command palette"
            onClick={() =>
              void controller.dispatch({
                operation: "framerLab.setCommandPaletteOpen",
                payload: { open: false },
              })
            }
            className="absolute inset-0 bg-[#020712]/80 backdrop-blur-md"
          />

          <div
            role="dialog"
            aria-modal="true"
            aria-label="CulebraLuxe command palette"
            className="relative z-10 mx-auto w-full max-w-2xl overflow-hidden rounded-[22px] border border-white/15 bg-[#081424]/95 shadow-[0_40px_120px_rgba(0,0,0,0.55)]"
          >
            <label className="flex min-h-16 items-center gap-3 border-b border-white/10 px-5">
              <Search className="h-5 w-5 shrink-0 text-[#c6a15b]" />
              <input
                ref={commandInputRef}
                value={model.commandQuery}
                onChange={(event) =>
                  void controller.dispatch({
                    operation: "framerLab.setCommandQuery",
                    payload: { query: event.target.value },
                  })
                }
                placeholder="Search commands..."
                className="w-full bg-transparent text-base text-white outline-none placeholder:text-white/30"
              />
              <span className="rounded-md border border-white/10 bg-white/[0.04] px-2 py-1 text-xs text-white/35">
                ESC
              </span>
            </label>

            <div className="max-h-[52vh] overflow-y-auto p-2">
              {matchingCommands.map((command) => (
                <button
                  key={command.id}
                  type="button"
                  onClick={() =>
                    void controller.dispatch({
                      operation: "framerLab.setCommandPaletteOpen",
                      payload: { open: false },
                    })
                  }
                  className="flex w-full items-center gap-4 rounded-[14px] px-4 py-3 text-left transition hover:bg-white/[0.07]"
                >
                  <span className="grid h-9 w-9 shrink-0 place-items-center rounded-[10px] border border-white/10 bg-white/[0.04]">
                    <Command className="h-4 w-4 text-[#c6a15b]" />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="flex items-center gap-2">
                      <span className="text-sm font-medium text-white">{command.label}</span>
                      <span className="rounded-full border border-white/8 px-2 py-0.5 text-[11px] text-white/32">
                        {command.group}
                      </span>
                    </span>
                    <span className="mt-1 block text-xs text-white/38">{command.detail}</span>
                  </span>
                  {command.shortcut ? (
                    <span className="rounded-md border border-white/10 bg-black/20 px-2 py-1 text-xs text-white/35">
                      {command.shortcut}
                    </span>
                  ) : null}
                </button>
              ))}

              {matchingCommands.length === 0 ? (
                <div className="px-4 py-10 text-center text-sm text-white/40">
                  No commands match “{model.commandQuery}”.
                </div>
              ) : null}
            </div>

            <div className="flex items-center justify-between border-t border-white/10 px-5 py-3 text-xs text-white/30">
              <span>{matchingCommands.length} commands</span>
              <span>Prototype only · no business mutation</span>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  )
}
