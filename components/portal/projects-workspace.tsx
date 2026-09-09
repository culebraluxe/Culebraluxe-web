"use client"

import { useEffect, useMemo } from "react"
import type { LucideIcon } from "lucide-react"
import {
  Activity,
  AlertCircle,
  Banknote,
  Building2,
  CalendarDays,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Clock3,
  FileText,
  FolderOpen,
  Handshake,
  Home,
  Megaphone,
  MoreHorizontal,
  Search,
  SlidersHorizontal,
  Users,
} from "lucide-react"

import { Panel } from "@/components/portal/panel"
import {
  InMemoryProjectsWorkspaceSource,
  ProjectsWorkspaceController,
  type ProjectDomainKey,
  type ProjectPlan,
  type ProjectPole,
  type ProjectWorkNode,
  type ProjectWorkStatus,
  type ProjectsWorkspaceSource,
  type ProjectWorkspaceView,
} from "@/ui/projects"
import { usePageController } from "@/ui/runtime"

const DOMAIN_ICON: Record<ProjectDomainKey, LucideIcon> = {
  properties: Home,
  people: Users,
  deals: Handshake,
  firm: Building2,
  marketing: Megaphone,
  accounting: Banknote,
}

const STATUS_LABEL: Record<ProjectWorkStatus, string> = {
  complete: "Complete",
  waiting: "Waiting",
  "in-progress": "In progress",
  "not-started": "Not started",
  blocked: "Blocked",
}

const STATUS_CLASS: Record<ProjectWorkStatus, string> = {
  complete: "bg-[var(--portal-success-pale)] text-[var(--portal-success)]",
  waiting: "bg-[var(--portal-gold-pale)] text-[var(--portal-gold-muted)]",
  "in-progress": "bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]",
  "not-started": "bg-white/40 text-black/45",
  blocked: "bg-[var(--portal-archive-pale)] text-[var(--portal-archive)]",
}

const VIEW_LABEL: Record<ProjectWorkspaceView, string> = {
  "work-plan": "Work Plan",
  timeline: "Timeline",
  calendar: "Calendar",
  documents: "Documents",
  financials: "Financials",
  activity: "Activity",
}

const VIEWS = Object.keys(VIEW_LABEL) as ProjectWorkspaceView[]

function nodeMatches(node: ProjectWorkNode, query: string): boolean {
  if (node.title.toLowerCase().includes(query)) return true
  return node.children?.some((child) => nodeMatches(child, query)) ?? false
}

function poleMatches(pole: ProjectPole, query: string): boolean {
  if (!query) return true
  const normalized = query.toLowerCase()
  return (
    pole.label.toLowerCase().includes(normalized) ||
    pole.subtitle.toLowerCase().includes(normalized) ||
    pole.projects.some(
      (project) =>
        project.title.toLowerCase().includes(normalized) ||
        project.workNodes.some((node) => nodeMatches(node, normalized)),
    )
  )
}

function flattenNodes(nodes: ProjectWorkNode[]): Array<{ node: ProjectWorkNode; depth: number }> {
  const result: Array<{ node: ProjectWorkNode; depth: number }> = []
  for (const node of nodes) {
    result.push({ node, depth: 0 })
    for (const child of node.children ?? []) result.push({ node: child, depth: 1 })
  }
  return result
}

function findNode(nodes: ProjectWorkNode[], nodeId: string | null): ProjectWorkNode | null {
  if (!nodeId) return null
  for (const node of nodes) {
    if (node.id === nodeId) return node
    const child = node.children?.find((candidate) => candidate.id === nodeId)
    if (child) return child
  }
  return null
}

function statusIcon(status: ProjectWorkStatus) {
  if (status === "complete") {
    return <CheckCircle2 className="h-4 w-4 text-[var(--portal-success)]" aria-hidden />
  }
  if (status === "waiting") {
    return <Clock3 className="h-4 w-4 text-[var(--portal-gold-muted)]" aria-hidden />
  }
  if (status === "blocked") {
    return <AlertCircle className="h-4 w-4 text-[var(--portal-archive)]" aria-hidden />
  }
  if (status === "in-progress") {
    return <Circle className="h-4 w-4 text-[var(--portal-navy-soft)]" aria-hidden />
  }
  return <Circle className="h-4 w-4 text-black/30" aria-hidden />
}

function ProgressBar({ value }: { value: number }) {
  return (
    <div className="h-1.5 overflow-hidden rounded-full bg-[var(--portal-mist-3)]/70">
      <div
        className="h-full rounded-full bg-[var(--portal-success)] transition-[width] duration-300"
        style={{ width: `${Math.max(0, Math.min(100, value))}%` }}
      />
    </div>
  )
}

function DomainRail({
  domains,
  activeDomain,
  onSelect,
}: {
  domains: Array<{ key: ProjectDomainKey; label: string }>
  activeDomain: ProjectDomainKey
  onSelect(domain: ProjectDomainKey): void
}) {
  return (
    <aside className="portal-glass-panel portal-glass-panel-feature flex min-h-[620px] flex-col overflow-hidden rounded-l-[var(--portal-panel-radius)] border-r-0">
      <div className="flex h-16 items-center justify-center border-b border-white/10">
        <span className="font-serif text-2xl font-light text-white">CL</span>
      </div>
      <nav aria-label="Project domains" className="flex flex-1 flex-col py-2">
        {domains.map((domain) => {
          const Icon = DOMAIN_ICON[domain.key]
          const active = domain.key === activeDomain
          return (
            <button
              key={domain.key}
              type="button"
              onClick={() => onSelect(domain.key)}
              aria-pressed={active}
              className={[
                "relative flex min-h-[72px] flex-col items-center justify-center gap-1.5 px-1 text-center transition",
                active
                  ? "bg-white/10 text-[var(--portal-gold-soft)]"
                  : "text-white/65 hover:bg-white/10 hover:text-white",
              ].join(" ")}
            >
              {active ? (
                <span
                  className="absolute inset-y-3 left-0 w-0.5 rounded-full bg-[var(--portal-gold)]"
                  aria-hidden
                />
              ) : null}
              <Icon className="h-5 w-5" strokeWidth={1.6} aria-hidden />
              <span className="max-w-[64px] text-[8px] font-medium uppercase tracking-[0.12em]">
                {domain.label}
              </span>
            </button>
          )
        })}
      </nav>
      <div className="border-t border-white/10 px-2 py-3 text-center text-[8px] font-light uppercase tracking-[0.14em] text-white/30">
        Projects
      </div>
    </aside>
  )
}

function WorkNodeTreeRow({
  node,
  depth,
  selected,
  onSelect,
}: {
  node: ProjectWorkNode
  depth: 0 | 1
  selected: boolean
  onSelect(nodeId: string): void
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(node.id)}
      className={[
        "flex min-h-9 w-full items-center gap-2 rounded-[var(--portal-tab-radius)] pr-2 text-left text-xs transition",
        depth === 0 ? "pl-8" : "pl-12",
        selected
          ? "bg-[var(--portal-gold-pale)] text-[var(--portal-navy)]"
          : "text-black/60 hover:bg-white/30 hover:text-[var(--portal-navy)]",
      ].join(" ")}
    >
      {statusIcon(node.status)}
      <span className="min-w-0 flex-1 truncate">{node.title}</span>
      {node.status !== "not-started" ? (
        <span className="text-[9px] font-light text-black/35">{STATUS_LABEL[node.status]}</span>
      ) : null}
    </button>
  )
}

function NavigatorPane({
  poles,
  query,
  expandedPoleIds,
  selectedPoleId,
  selectedProjectId,
  selectedNodeId,
  onQuery,
  onTogglePole,
  onSelectPole,
  onSelectProject,
  onSelectNode,
}: {
  poles: ProjectPole[]
  query: string
  expandedPoleIds: string[]
  selectedPoleId: string | null
  selectedProjectId: string | null
  selectedNodeId: string | null
  onQuery(query: string): void
  onTogglePole(poleId: string): void
  onSelectPole(poleId: string): void
  onSelectProject(poleId: string, projectId: string): void
  onSelectNode(nodeId: string): void
}) {
  return (
    <aside className="portal-glass-panel portal-glass-panel-lifted min-h-[620px] overflow-hidden rounded-r-[var(--portal-panel-radius)] border-l border-l-white/20">
      <div className="border-b border-[var(--portal-panel-border)] px-4 py-4">
        <div className="flex items-center justify-between gap-3">
          <div>
            <p className="text-[10px] font-light uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">
              Projects
            </p>
            <h1 className="mt-1 font-serif text-2xl font-light text-[var(--portal-navy)]">
              Work Navigator
            </h1>
          </div>
          <button
            type="button"
            aria-label="Project filters"
            className="inline-flex h-10 w-10 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 text-[var(--portal-navy-soft)] transition hover:bg-white/40"
          >
            <SlidersHorizontal className="h-4 w-4" aria-hidden />
          </button>
        </div>
        <label className="mt-3 flex h-11 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 px-3">
          <Search className="h-4 w-4 shrink-0 text-black/35" aria-hidden />
          <input
            value={query}
            onChange={(event) => onQuery(event.target.value)}
            placeholder="Search poles, projects, or work…"
            className="min-w-0 flex-1 bg-transparent text-sm font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35"
          />
        </label>
      </div>

      <div className="max-h-[calc(100vh-275px)] space-y-2 overflow-y-auto p-3">
        {poles.length === 0 ? (
          <div className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/20 px-3 py-6 text-center text-sm font-light text-black/45">
            No matching work.
          </div>
        ) : null}

        {poles.map((pole) => {
          const expanded = expandedPoleIds.includes(pole.id)
          const selectedPole = selectedPoleId === pole.id
          return (
            <section
              key={pole.id}
              className={[
                "overflow-hidden rounded-[var(--portal-tab-radius)] border transition",
                selectedPole
                  ? "border-[var(--portal-gold)]/50 bg-white/40"
                  : "border-[var(--portal-panel-border)] bg-white/20",
              ].join(" ")}
            >
              <div className="flex items-center gap-2 px-2.5 py-2.5">
                <button
                  type="button"
                  onClick={() => onTogglePole(pole.id)}
                  className="inline-flex h-8 w-6 shrink-0 items-center justify-center text-black/40 hover:text-[var(--portal-navy)]"
                  aria-label={expanded ? `Collapse ${pole.label}` : `Expand ${pole.label}`}
                >
                  {expanded ? (
                    <ChevronDown className="h-4 w-4" aria-hidden />
                  ) : (
                    <ChevronRight className="h-4 w-4" aria-hidden />
                  )}
                </button>
                <button
                  type="button"
                  onClick={() => onSelectPole(pole.id)}
                  className="flex min-w-0 flex-1 items-center gap-3 text-left"
                >
                  <span className="flex h-11 w-14 shrink-0 items-center justify-center rounded-[9px] bg-[var(--portal-navy)] text-[var(--portal-gold-soft)] shadow-sm">
                    <Home className="h-5 w-5" strokeWidth={1.5} aria-hidden />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-serif text-lg font-light leading-tight text-[var(--portal-navy)]">
                      {pole.label}
                    </span>
                    <span className="mt-0.5 block truncate text-[10px] font-light text-black/40">
                      {pole.subtitle}
                    </span>
                  </span>
                </button>
                <div className="w-14 shrink-0 text-right">
                  <span className="font-serif text-base font-light text-[var(--portal-navy)]">
                    {pole.progress}%
                  </span>
                  <div className="mt-1">
                    <ProgressBar value={pole.progress} />
                  </div>
                </div>
              </div>

              {expanded ? (
                <div className="border-t border-[var(--portal-panel-border)] px-2 pb-2 pt-1.5">
                  {pole.projects.map((project) => {
                    const selectedProject = selectedProjectId === project.id
                    return (
                      <div key={project.id}>
                        <button
                          type="button"
                          onClick={() => onSelectProject(pole.id, project.id)}
                          className={[
                            "flex min-h-10 w-full items-center gap-2 rounded-[var(--portal-tab-radius)] px-2.5 text-left transition",
                            selectedProject ? "bg-white/30" : "hover:bg-white/20",
                          ].join(" ")}
                        >
                          <FolderOpen
                            className="h-4 w-4 shrink-0 text-[var(--portal-navy-soft)]"
                            strokeWidth={1.6}
                            aria-hidden
                          />
                          <span className="min-w-0 flex-1 truncate font-serif text-[15px] font-light text-[var(--portal-navy)]">
                            {project.title}
                          </span>
                          <div className="w-14 shrink-0">
                            <ProgressBar value={project.progress} />
                          </div>
                          <span className="w-8 shrink-0 text-right text-[10px] font-light text-black/45">
                            {project.progress}%
                          </span>
                          <ChevronRight className="h-3.5 w-3.5 shrink-0 text-black/30" aria-hidden />
                        </button>

                        {selectedProject ? (
                          <div className="relative ml-3 border-l border-[var(--portal-mist-3)] py-1">
                            {project.workNodes.map((node) => (
                              <div key={node.id}>
                                <WorkNodeTreeRow
                                  node={node}
                                  depth={0}
                                  selected={selectedNodeId === node.id}
                                  onSelect={onSelectNode}
                                />
                                {node.children?.map((child) => (
                                  <WorkNodeTreeRow
                                    key={child.id}
                                    node={child}
                                    depth={1}
                                    selected={selectedNodeId === child.id}
                                    onSelect={onSelectNode}
                                  />
                                ))}
                              </div>
                            ))}
                          </div>
                        ) : null}
                      </div>
                    )
                  })}
                </div>
              ) : null}
            </section>
          )
        })}
      </div>
    </aside>
  )
}

function ProjectHeader({ pole, project }: { pole: ProjectPole; project: ProjectPlan }) {
  return (
    <Panel compact lifted className="overflow-visible">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-[10px] font-light uppercase tracking-[0.17em] text-[var(--portal-blue-gray)]">
            <span>{pole.label}</span>
            <ChevronRight className="h-3 w-3" aria-hidden />
            <span>{project.title}</span>
          </div>
          <div className="mt-2 flex flex-wrap items-end gap-x-4 gap-y-2">
            <div>
              <p className="text-[10px] font-medium uppercase tracking-[0.2em] text-[var(--portal-gold-muted)]">
                {project.kind}
              </p>
              <h2 className="mt-0.5 font-serif text-3xl font-light text-[var(--portal-navy)]">
                {project.title}
              </h2>
            </div>
            <span className="mb-1 rounded-full bg-[var(--portal-success-pale)] px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-success)]">
              {project.status}
            </span>
          </div>
          <div className="mt-2 flex flex-wrap gap-2 text-[10px] font-light text-black/55">
            <span className="rounded-full border border-[var(--portal-panel-border)] bg-white/30 px-2.5 py-1">
              {pole.subtitle}
            </span>
            <span className="rounded-full border border-[var(--portal-panel-border)] bg-white/30 px-2.5 py-1">
              Phase · {project.phaseLabel}
            </span>
            <span className="rounded-full border border-[var(--portal-panel-border)] bg-white/30 px-2.5 py-1">
              {project.progress}% complete
            </span>
          </div>
        </div>
        <button
          type="button"
          className="inline-flex h-9 w-9 items-center justify-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/20 text-[var(--portal-navy-soft)] hover:bg-white/40"
          aria-label="Project menu"
        >
          <MoreHorizontal className="h-4 w-4" aria-hidden />
        </button>
      </div>
    </Panel>
  )
}

function ProjectProgress({ project }: { project: ProjectPlan }) {
  const topNodes = project.workNodes.slice(0, 6)
  return (
    <Panel
      compact
      heading="Project Progress"
      action={
        <span className="font-serif text-lg font-light text-[var(--portal-navy)]">
          {project.progress}%
        </span>
      }
    >
      <div className="mt-1 grid grid-cols-3 gap-x-2 gap-y-4 sm:grid-cols-6">
        {topNodes.map((node, index) => (
          <div key={node.id} className="relative text-center">
            {index > 0 ? (
              <div
                className="absolute right-1/2 top-3.5 h-px w-full bg-[var(--portal-mist-3)]"
                aria-hidden
              />
            ) : null}
            <div
              className={[
                "relative z-10 mx-auto flex h-7 w-7 items-center justify-center rounded-full border bg-white/70",
                node.status === "complete"
                  ? "border-[var(--portal-success)] bg-[var(--portal-success)] text-white"
                  : node.status === "waiting"
                    ? "border-[var(--portal-gold)] text-[var(--portal-gold-muted)]"
                    : "border-[var(--portal-mist-3)] text-black/35",
              ].join(" ")}
            >
              {node.status === "complete" ? (
                <Check className="h-3.5 w-3.5" aria-hidden />
              ) : (
                <span className="text-[10px]">{index + 1}</span>
              )}
            </div>
            <div className="mt-1.5 truncate text-[9px] font-light text-black/55">{node.title}</div>
          </div>
        ))}
      </div>
    </Panel>
  )
}

function WorkPlan({
  project,
  selectedNodeId,
  onSelectNode,
}: {
  project: ProjectPlan
  selectedNodeId: string | null
  onSelectNode(nodeId: string): void
}) {
  const rows = flattenNodes(project.workNodes)
  return (
    <div className="space-y-3">
      <ProjectProgress project={project} />
      <div className="grid gap-3 md:grid-cols-2">
        <Panel
          compact
          variant="attention"
          eyebrow="Next Action"
          heading={project.nextAction ?? "No immediate action"}
        >
          <p className="text-xs font-light leading-5 text-black/55">
            {project.nextActionDetail ?? "The current project plan has no queued action."}
          </p>
          {project.nextAction ? (
            <button
              type="button"
              onClick={() => {
                const waiting = rows.find(
                  ({ node }) => node.status === "waiting" || node.status === "in-progress",
                )
                if (waiting) onSelectNode(waiting.node.id)
              }}
              className="mt-3 inline-flex min-h-10 items-center justify-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-medium uppercase tracking-[0.14em] text-white transition hover:text-[var(--portal-gold-soft)]"
            >
              Take Action
            </button>
          ) : null}
        </Panel>
        <Panel
          compact
          eyebrow="Blockers / Missing"
          heading={project.blocker ? "Needs attention" : "Clear"}
        >
          <p className="text-xs font-light leading-5 text-black/55">
            {project.blocker ?? "No blockers recorded for this project."}
          </p>
        </Panel>
      </div>

      <Panel
        flush
        compact
        heading="Work Breakdown Structure"
        action={
          <span className="text-[9px] font-light uppercase tracking-[0.12em] text-black/35">
            {rows.length} work items
          </span>
        }
      >
        <div className="overflow-x-auto border-t border-[var(--portal-panel-border)]">
          <table className="w-full min-w-[640px] text-left">
            <thead className="bg-white/20 text-[9px] font-medium uppercase tracking-[0.12em] text-black/35">
              <tr>
                <th className="px-4 py-2.5">Item</th>
                <th className="px-3 py-2.5">Status</th>
                <th className="px-3 py-2.5">Due</th>
                <th className="px-3 py-2.5">Owner</th>
                <th className="px-4 py-2.5">Notes</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(({ node, depth }) => {
                const selected = selectedNodeId === node.id
                return (
                  <tr
                    key={node.id}
                    onClick={() => onSelectNode(node.id)}
                    className={[
                      "cursor-pointer border-t border-[var(--portal-panel-border)] text-xs transition hover:bg-white/20",
                      selected ? "bg-[var(--portal-gold-pale)]" : "",
                    ].join(" ")}
                  >
                    <td className="px-4 py-2.5">
                      <div className="flex items-center gap-2" style={{ paddingLeft: depth * 16 }}>
                        {statusIcon(node.status)}
                        <span className="font-medium text-[var(--portal-navy)]">{node.title}</span>
                      </div>
                    </td>
                    <td className="px-3 py-2.5">
                      <span
                        className={`rounded-full px-2 py-1 text-[9px] font-medium ${STATUS_CLASS[node.status]}`}
                      >
                        {STATUS_LABEL[node.status]}
                      </span>
                    </td>
                    <td className="px-3 py-2.5 font-light text-black/50">{node.dueLabel ?? "—"}</td>
                    <td className="px-3 py-2.5 font-light text-black/50">{node.owner ?? "—"}</td>
                    <td className="px-4 py-2.5 font-light text-black/45">
                      {node.note ?? node.relatedLabel ?? "—"}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      </Panel>
    </div>
  )
}

function ProjectionPlaceholder({
  view,
  project,
}: {
  view: ProjectWorkspaceView
  project: ProjectPlan
}) {
  const Icon =
    view === "calendar"
      ? CalendarDays
      : view === "documents"
        ? FileText
        : view === "financials"
          ? Banknote
          : Activity

  return (
    <Panel
      variant="soft"
      className="min-h-[420px]"
      eyebrow={project.title}
      heading={VIEW_LABEL[view]}
      headingSize="xl"
    >
      <div className="mt-10 flex flex-col items-center justify-center text-center">
        <div className="flex h-14 w-14 items-center justify-center rounded-full border border-[var(--portal-panel-border)] bg-white/30 text-[var(--portal-navy-soft)]">
          <Icon className="h-6 w-6" strokeWidth={1.5} aria-hidden />
        </div>
        <p className="mt-4 max-w-md text-sm font-light leading-6 text-black/50">
          This projection is wired through the page model and ready for its real service-backed view. The prototype keeps the Work Plan as the fully rendered operating surface.
        </p>
      </div>
    </Panel>
  )
}

function Inspector({
  pole,
  project,
  node,
}: {
  pole: ProjectPole
  project: ProjectPlan
  node: ProjectWorkNode | null
}) {
  const target = node ?? project.workNodes[0] ?? null
  return (
    <Panel
      compact
      lifted
      className="2xl:sticky 2xl:top-[150px]"
      eyebrow="Inspector"
      heading={target?.title ?? project.title}
      headingSize="xl"
    >
      {target ? (
        <div className="space-y-4">
          <div>
            <div className="text-[9px] font-light uppercase tracking-[0.14em] text-black/35">Status</div>
            <div className="mt-1.5 flex items-center gap-2">
              {statusIcon(target.status)}
              <span className={`rounded-full px-2 py-1 text-[9px] font-medium ${STATUS_CLASS[target.status]}`}>
                {STATUS_LABEL[target.status]}
              </span>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <div className="text-[9px] font-light uppercase tracking-[0.14em] text-black/35">Due</div>
              <div className="mt-1 text-xs font-light text-[var(--portal-navy)]">{target.dueLabel ?? "—"}</div>
            </div>
            <div>
              <div className="text-[9px] font-light uppercase tracking-[0.14em] text-black/35">Owner</div>
              <div className="mt-1 text-xs font-light text-[var(--portal-navy)]">{target.owner ?? "—"}</div>
            </div>
          </div>
          <div className="border-t border-[var(--portal-panel-border)] pt-4">
            <div className="text-[9px] font-light uppercase tracking-[0.14em] text-black/35">Context</div>
            <div className="mt-2 space-y-2 text-xs font-light text-black/55">
              <div className="flex items-center gap-2">
                <Home className="h-3.5 w-3.5 text-[var(--portal-gold-muted)]" aria-hidden />
                <span>{pole.label}</span>
              </div>
              <div className="flex items-center gap-2">
                <FolderOpen className="h-3.5 w-3.5 text-[var(--portal-gold-muted)]" aria-hidden />
                <span>{project.title}</span>
              </div>
              {target.relatedLabel ? (
                <div className="flex items-center gap-2">
                  <FileText className="h-3.5 w-3.5 text-[var(--portal-gold-muted)]" aria-hidden />
                  <span>{target.relatedLabel}</span>
                </div>
              ) : null}
            </div>
          </div>
          {target.note ? (
            <div className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/20 p-3 text-xs font-light leading-5 text-black/55">
              {target.note}
            </div>
          ) : null}
          <div className="grid gap-2 border-t border-[var(--portal-panel-border)] pt-4">
            <button
              type="button"
              className="min-h-10 rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.13em] text-white transition hover:text-[var(--portal-gold-soft)]"
            >
              Open Work Item
            </button>
            <button
              type="button"
              className="min-h-10 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/20 px-3 text-[10px] font-medium uppercase tracking-[0.13em] text-[var(--portal-navy)] transition hover:bg-white/40"
            >
              View Activity
            </button>
          </div>
        </div>
      ) : (
        <p className="text-sm font-light text-black/45">Select a work item to inspect it.</p>
      )}
    </Panel>
  )
}

export function ProjectsWorkspace({ source }: { source?: ProjectsWorkspaceSource } = {}) {
  const controller = useMemo(
    () => new ProjectsWorkspaceController(source ?? new InMemoryProjectsWorkspaceSource()),
    [source],
  )
  const model = usePageController(controller)

  useEffect(() => {
    void controller.dispatch({ operation: "projects.load", payload: {} })
    // Mirror Clients: keep the memoized controller alive through StrictMode setup/cleanup.
  }, [controller])

  const domains = model.data?.domains ?? []
  const poles = useMemo(
    () =>
      (model.data?.poles ?? []).filter(
        (pole) =>
          pole.domain === model.activeDomain &&
          poleMatches(pole, model.query.trim().toLowerCase()),
      ),
    [model.activeDomain, model.data?.poles, model.query],
  )

  const selectedPole =
    model.data?.poles.find((pole) => pole.id === model.selectedPoleId) ?? poles[0] ?? null
  const selectedProject =
    selectedPole?.projects.find((project) => project.id === model.selectedProjectId) ??
    selectedPole?.projects[0] ??
    null
  const selectedNode = selectedProject
    ? findNode(selectedProject.workNodes, model.selectedNodeId)
    : null

  if (model.loading) {
    return (
      <Panel lifted className="min-h-[420px]" heading="Projects" subtitle="Loading the project workspace…">
        <div className="h-2 w-40 animate-pulse rounded-full bg-[var(--portal-mist-3)]" />
      </Panel>
    )
  }

  if (model.error || !model.data) {
    return (
      <Panel variant="attention" heading="Projects unavailable">
        <p className="text-sm font-light text-black/55">
          {model.error ?? "Project workspace data is unavailable."}
        </p>
      </Panel>
    )
  }

  return (
    <div className="grid min-h-[calc(100vh-165px)] gap-3 lg:grid-cols-[410px_minmax(0,1fr)]">
      <div className="grid min-h-0 grid-cols-[74px_minmax(0,1fr)]">
        <DomainRail
          domains={domains}
          activeDomain={model.activeDomain}
          onSelect={(domain) =>
            void controller.dispatch({ operation: "projects.selectDomain", payload: { domain } })
          }
        />
        <NavigatorPane
          poles={poles}
          query={model.query}
          expandedPoleIds={model.expandedPoleIds}
          selectedPoleId={model.selectedPoleId}
          selectedProjectId={model.selectedProjectId}
          selectedNodeId={model.selectedNodeId}
          onQuery={(query) =>
            void controller.dispatch({ operation: "projects.queryChanged", payload: { query } })
          }
          onTogglePole={(poleId) =>
            void controller.dispatch({ operation: "projects.togglePole", payload: { poleId } })
          }
          onSelectPole={(poleId) =>
            void controller.dispatch({ operation: "projects.selectPole", payload: { poleId } })
          }
          onSelectProject={(poleId, projectId) =>
            void controller.dispatch({
              operation: "projects.selectProject",
              payload: { poleId, projectId },
            })
          }
          onSelectNode={(nodeId) =>
            void controller.dispatch({ operation: "projects.selectNode", payload: { nodeId } })
          }
        />
      </div>

      {selectedPole && selectedProject ? (
        <div className="min-w-0">
          <ProjectHeader pole={selectedPole} project={selectedProject} />

          <div className="mt-3 overflow-x-auto">
            <nav aria-label="Project workspace views" className="portal-glass-rail">
              {VIEWS.map((view) => (
                <button
                  key={view}
                  type="button"
                  onClick={() =>
                    void controller.dispatch({ operation: "projects.selectView", payload: { view } })
                  }
                  aria-current={model.activeView === view ? "page" : undefined}
                  className="portal-glass-tab"
                >
                  {VIEW_LABEL[view]}
                </button>
              ))}
            </nav>
          </div>

          <div className="mt-3 grid gap-3 2xl:grid-cols-[minmax(0,1fr)_310px]">
            <main className="min-w-0">
              {model.activeView === "work-plan" ? (
                <WorkPlan
                  project={selectedProject}
                  selectedNodeId={model.selectedNodeId}
                  onSelectNode={(nodeId) =>
                    void controller.dispatch({ operation: "projects.selectNode", payload: { nodeId } })
                  }
                />
              ) : (
                <ProjectionPlaceholder view={model.activeView} project={selectedProject} />
              )}
            </main>
            <aside className="min-w-0">
              <Inspector pole={selectedPole} project={selectedProject} node={selectedNode} />
            </aside>
          </div>
        </div>
      ) : (
        <Panel lifted className="min-h-[420px]" heading="No project selected">
          <p className="text-sm font-light text-black/45">
            Choose a Pole and Project from the navigator.
          </p>
        </Panel>
      )}
    </div>
  )
}
