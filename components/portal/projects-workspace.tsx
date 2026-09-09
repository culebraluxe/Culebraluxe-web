"use client"

import { useEffect, useMemo } from "react"
import type { LucideIcon } from "lucide-react"
import {
  AlertCircle,
  Banknote,
  Building2,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Clock3,
  FileText,
  Handshake,
  Home,
  Megaphone,
  Search,
  Users,
} from "lucide-react"

import type {
  ProjectDomainKey,
  ProjectPlan,
  ProjectPole,
  ProjectWorkNode,
  ProjectWorkStatus,
  ProjectWorkspaceView,
} from "@/ui/projects"
import {
  InMemoryProjectsWorkspaceSource,
  ProjectsWorkspaceController,
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

const STATUS_BAR: Record<ProjectWorkStatus, string> = {
  complete: "bg-[var(--portal-success)]",
  waiting: "bg-[var(--portal-gold)]",
  "in-progress": "bg-[var(--portal-blue-gray)]",
  "not-started": "bg-black/20",
  blocked: "bg-[var(--portal-archive)]",
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

function StatusDot({ status, className }: { status: ProjectWorkStatus; className?: string }) {
  return <span aria-hidden className={`h-1.5 w-1.5 shrink-0 rounded-full ${STATUS_BAR[status]} ${className ?? ""}`} />
}

function StatusIcon({ status }: { status: ProjectWorkStatus }) {
  if (status === "complete") return <CheckCircle2 className="h-3.5 w-3.5 shrink-0 text-[var(--portal-success)]" aria-hidden />
  if (status === "waiting") return <Clock3 className="h-3.5 w-3.5 shrink-0 text-[var(--portal-gold)]" aria-hidden />
  if (status === "blocked") return <AlertCircle className="h-3.5 w-3.5 shrink-0 text-[var(--portal-archive)]" aria-hidden />
  if (status === "in-progress") return <Circle className="h-3.5 w-3.5 shrink-0 text-[var(--portal-blue-gray)]" aria-hidden />
  return <Circle className="h-3.5 w-3.5 shrink-0 text-black/25" aria-hidden />
}

function Progress({ value, className }: { value: number; className?: string }) {
  const clamped = Math.max(0, Math.min(100, value))
  return (
    <div className={`h-1 w-full overflow-hidden rounded-full bg-[var(--portal-mist-3)]/70 ${className ?? ""}`}>
      <div className="h-full rounded-full bg-[var(--portal-gold)] transition-[width] duration-300" style={{ width: `${clamped}%` }} />
    </div>
  )
}

function nodeMatches(node: ProjectWorkNode, query: string): boolean {
  if (node.title.toLowerCase().includes(query)) return true
  return node.children?.some((child) => nodeMatches(child, query)) ?? false
}

function poleMatches(pole: ProjectPole, query: string): boolean {
  if (!query) return true
  const q = query.toLowerCase()
  return (
    pole.label.toLowerCase().includes(q) ||
    pole.subtitle.toLowerCase().includes(q) ||
    pole.projects.some(
      (p) => p.title.toLowerCase().includes(q) || p.workNodes.some((n) => nodeMatches(n, q)),
    )
  )
}

/** Find a WorkNode (searching children) under a project. */
function findWorkNode(project: ProjectPlan | null, nodeId: string | null): ProjectWorkNode | null {
  if (!project || !nodeId) return null
  for (const node of project.workNodes) {
    if (node.id === nodeId) return node
    const child = node.children?.find((c) => c.id === nodeId)
    if (child) return child
  }
  return null
}

type DomainRailProps = {
  domains: ReadonlyArray<{ key: ProjectDomainKey; shortLabel: string }>
  active: ProjectDomainKey
  onSelect: (domain: ProjectDomainKey) => void
}

/** Vertical domain tabs that live INSIDE Pane 1 — not global navigation. */
function DomainRail({ domains, active, onSelect }: DomainRailProps) {
  return (
    <div className="flex w-[52px] shrink-0 flex-col items-center border-r border-white/15 bg-white/10 py-2" aria-label="Project domain">
      {domains.map((domain) => {
        const Icon = DOMAIN_ICON[domain.key]
        const isActive = domain.key === active
        return (
          <button
            key={domain.key}
            type="button"
            onClick={() => onSelect(domain.key)}
            title={domain.shortLabel}
            aria-current={isActive ? "true" : undefined}
            className={`group relative flex w-full flex-col items-center gap-1.5 py-2 transition ${isActive ? "text-white" : "text-[var(--portal-blue-gray)] hover:text-[var(--portal-navy)]"}`}
          >
            <span className={`absolute inset-y-1 left-0 w-0.5 rounded-r-full transition ${isActive ? "bg-[var(--portal-gold)]" : "bg-transparent group-hover:bg-[var(--portal-gold)]/40"}`} />
            <span className={`flex h-8 w-8 items-center justify-center rounded-[var(--portal-tab-radius)] transition ${isActive ? "bg-[var(--portal-navy)]/85 text-[var(--portal-gold-soft)] shadow-sm" : "bg-white/[0.08] text-current group-hover:bg-white/25"}`}>
              <Icon className="h-[18px] w-[18px]" strokeWidth={1.6} aria-hidden />
            </span>
            <span className="max-w-[44px] text-center text-[7px] font-medium uppercase leading-none tracking-[0.06em]">{domain.shortLabel.slice(0, 9)}</span>
          </button>
        )
      })}
    </div>
  )
}

type NodeRowProps = {
  node: ProjectWorkNode
  depth: number
  selectedNodeId: string | null
  onSelectNode: (id: string | null) => void
}

function NodeRow({ node, depth, selectedNodeId, onSelectNode }: NodeRowProps) {
  const selected = node.id === selectedNodeId
  return (
    <li>
      <button
        type="button"
        onClick={() => onSelectNode(selected ? null : node.id)}
        className={`flex w-full items-center gap-2 rounded-[var(--portal-tab-radius)] px-2 py-1.5 text-left transition ${selected ? "bg-[var(--portal-navy)]/5 ring-1 ring-inset ring-[var(--portal-navy)]/10" : "hover:bg-white/40"}`}
        style={{ paddingLeft: `${10 + depth * 14}px` }}
      >
        <StatusDot status={node.status} />
        <span className="min-w-0 flex-1 truncate text-[12.5px] font-light text-[var(--portal-navy)]">{node.title}</span>
        {node.children?.length ? <ChevronRight className="h-3 w-3 shrink-0 text-black/25" aria-hidden /> : null}
      </button>
      {node.children?.length ? (
        <ul>
          {node.children.map((child) => (
            <NodeRow key={child.id} node={child} depth={depth + 1} selectedNodeId={selectedNodeId} onSelectNode={onSelectNode} />
          ))}
        </ul>
      ) : null}
    </li>
  )
}

type NavigatorProps = {
  poles: ProjectPole[]
  expanded: string[]
  selectedPoleId: string | null
  selectedProjectId: string | null
  selectedNodeId: string | null
  onTogglePole: (poleId: string) => void
  onSelectProject: (poleId: string, projectId: string) => void
  onSelectNode: (nodeId: string | null) => void
}

function Navigator({ poles, expanded, selectedPoleId, selectedProjectId, selectedNodeId, onTogglePole, onSelectProject, onSelectNode }: NavigatorProps) {
  if (poles.length === 0) {
    return <p className="px-3 py-6 text-center text-sm font-light text-black/45">No matches in this domain.</p>
  }
  return (
    <ul className="space-y-1 px-1.5 pb-2">
      {poles.map((pole) => {
        const isExpanded = expanded.includes(pole.id)
        const poleActive = pole.id === selectedPoleId
        const Icon = DOMAIN_ICON[pole.domain]
        return (
          <li key={pole.id}>
            <div className={`rounded-[var(--portal-tab-radius)] transition ${poleActive ? "bg-white/55 shadow-[0_1px_8px_rgba(3,15,35,0.06)] ring-1 ring-inset ring-[var(--portal-navy)]/8" : "hover:bg-white/40"}`}>
              <button type="button" onClick={() => onTogglePole(pole.id)} className="flex w-full items-start gap-2.5 px-2 py-2 text-left">
                <span className={`mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-[9px] shadow-sm ${poleActive ? "bg-[var(--portal-navy)] text-[var(--portal-gold-soft)]" : "bg-white/50 text-[var(--portal-navy)] ring-1 ring-inset ring-[var(--portal-navy)]/10"}`}>
                  <Icon className="h-[18px] w-[18px]" strokeWidth={1.5} aria-hidden />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="flex items-center gap-1.5">
                    <ChevronDown className={`h-3.5 w-3.5 shrink-0 text-black/35 transition ${isExpanded ? "" : "-rotate-90"}`} aria-hidden />
                    <span className="truncate font-serif text-[17px] font-light leading-tight text-[var(--portal-navy)]">{pole.label}</span>
                  </span>
                  <span className="mt-0.5 block truncate pl-[18px] text-[10.5px] font-light text-black/45">{pole.subtitle}</span>
                  <span className="mt-1.5 flex items-center gap-2 pl-[18px]">
                    <Progress value={pole.progress} className="max-w-[64px]" />
                    <span className="text-[9px] font-light uppercase tracking-[0.1em] text-[var(--portal-blue-gray)]">{pole.progress}%</span>
                  </span>
                </span>
              </button>

              {isExpanded && pole.projects.length > 0 ? (
                <div className="pb-1.5 pl-[13px]">
                  {pole.projects.map((project) => {
                    const projectActive = pole.id === selectedPoleId && project.id === selectedProjectId
                    return (
                      <div key={project.id} className="mb-0.5 border-l border-[var(--portal-mist-3)]/80 pl-2.5">
                        <button
                          type="button"
                          onClick={() => onSelectProject(pole.id, project.id)}
                          className={`mt-1 flex w-full items-center gap-1.5 rounded-[var(--portal-tab-radius)] px-2 py-1.5 text-left transition ${projectActive ? "bg-[var(--portal-navy)]/5" : "hover:bg-white/40"}`}
                        >
                          <span className="text-[8.5px] font-medium uppercase tracking-[0.12em] text-[var(--portal-gold-muted)]">{project.kind}</span>
                          <span className="min-w-0 flex-1 truncate text-[13.5px] text-[var(--portal-navy)]">{project.title}</span>
                          <span className="text-[9px] font-light text-black/35">{project.progress}%</span>
                        </button>
                        {projectActive ? (
                          <ul className="mt-0.5 space-y-px">
                            {project.workNodes.map((node) => (
                              <NodeRow key={node.id} node={node} depth={1} selectedNodeId={selectedNodeId} onSelectNode={onSelectNode} />
                            ))}
                          </ul>
                        ) : null}
                      </div>
                    )
                  })}
                </div>
              ) : null}
            </div>
          </li>
        )
      })}
    </ul>
  )
}

type PaneOneProps = {
  domains: ReadonlyArray<{ key: ProjectDomainKey; label: string; shortLabel: string }>
  activeDomain: ProjectDomainKey
  query: string
  poles: ProjectPole[]
  expanded: string[]
  selectedPoleId: string | null
  selectedProjectId: string | null
  selectedNodeId: string | null
  onSelectDomain: (d: ProjectDomainKey) => void
  onQuery: (q: string) => void
  onTogglePole: (id: string) => void
  onSelectProject: (poleId: string, projectId: string) => void
  onSelectNode: (id: string | null) => void
}

/** Pane 1 — ONE glass object: vertical domain tabs fused with the Pole→Project→WBS tree. */
function PaneOne(props: PaneOneProps) {
  const activeLabel = props.domains.find((d) => d.key === props.activeDomain)?.label ?? ""
  return (
    <section className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
      <div className="flex min-h-0 flex-1">
        <DomainRail domains={props.domains} active={props.activeDomain} onSelect={props.onSelectDomain} />
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <div className="border-b border-[var(--portal-panel-border)] px-3 pb-2 pt-3">
            <p className="text-[9px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">{activeLabel}</p>
            <label className="mt-2 flex h-9 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/30 px-2.5">
              <Search className="h-3.5 w-3.5 shrink-0 text-black/35" aria-hidden />
              <input
                value={props.query}
                onChange={(e) => props.onQuery(e.target.value)}
                placeholder="Find work…"
                className="min-w-0 flex-1 bg-transparent text-[12.5px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35"
              />
            </label>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto py-1.5">
            <Navigator
              poles={props.poles}
              expanded={props.expanded}
              selectedPoleId={props.selectedPoleId}
              selectedProjectId={props.selectedProjectId}
              selectedNodeId={props.selectedNodeId}
              onTogglePole={props.onTogglePole}
              onSelectProject={props.onSelectProject}
              onSelectNode={props.onSelectNode}
            />
          </div>
        </div>
      </div>
    </section>
  )
}

type WorkPlanProps = {
  project: ProjectPlan
  selectedNodeId: string | null
  onSelectNode: (id: string | null) => void
}

function WorkPlanNode({ node, selectedNodeId, onSelectNode }: { node: ProjectWorkNode; selectedNodeId: string | null; onSelectNode: (id: string | null) => void }) {
  const selected = node.id === selectedNodeId
  return (
    <li>
      <button
        type="button"
        onClick={() => onSelectNode(selected ? null : node.id)}
        className={`flex w-full items-center gap-2.5 rounded-[var(--portal-tab-radius)] px-2 py-2 text-left transition ${selected ? "bg-[var(--portal-navy)]/[0.04] ring-1 ring-inset ring-[var(--portal-navy)]/10" : "hover:bg-white/40"}`}
      >
        <StatusIcon status={node.status} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-[13.5px] text-[var(--portal-navy)]">{node.title}</span>
          <span className="block truncate text-[10px] font-light text-black/40">{node.type} · {node.owner ?? "—"}</span>
        </span>
        <span className="text-right">
          <span className="block text-[10px] font-light text-[var(--portal-blue-gray)]">{STATUS_LABEL[node.status]}</span>
          {node.dueLabel ? <span className="block text-[9px] font-light text-black/40">{node.dueLabel}</span> : null}
        </span>
      </button>
      {node.children?.length ? (
        <ul className="ml-5 border-l border-[var(--portal-mist-3)]/70 pl-1.5">
          {node.children.map((child) => (
            <WorkPlanNode key={child.id} node={child} selectedNodeId={selectedNodeId} onSelectNode={onSelectNode} />
          ))}
        </ul>
      ) : null}
    </li>
  )
}

function WorkPlan({ project, selectedNodeId, onSelectNode }: WorkPlanProps) {
  return (
    <div className="min-h-0 flex-1 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 px-1.5 py-1">
      <ul className="space-y-0.5">
        {project.workNodes.map((node) => (
          <WorkPlanNode key={node.id} node={node} selectedNodeId={selectedNodeId} onSelectNode={onSelectNode} />
        ))}
      </ul>
    </div>
  )
}

function ProjectHeader({ pole, project }: { pole: ProjectPole; project: ProjectPlan }) {
  return (
    <div>
      <p className="flex items-center gap-1 text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
        {pole.label} <ChevronRight className="h-3 w-3" aria-hidden /> {project.kind}
      </p>
      <div className="mt-1 flex flex-wrap items-center justify-between gap-3">
        <h2 className="min-w-0 font-serif text-[26px] font-light leading-tight text-[var(--portal-navy)]">{project.title}</h2>
        <div className="flex items-center gap-2">
          <span className="rounded-full bg-white/50 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)]">{project.phaseLabel}</span>
          <span className="text-[11px] font-light text-[var(--portal-blue-gray)]">{project.progress}%</span>
        </div>
      </div>
      <div className="mt-2 w-full max-w-[280px]">
        <Progress value={project.progress} />
      </div>
      {project.nextAction ? (
        <p className="mt-2 text-[12.5px] font-light text-[var(--portal-navy)]">
          <span className="font-medium text-[var(--portal-gold-muted)]">Next&nbsp;·&nbsp;</span>
          {project.nextAction}
          {project.nextActionDetail ? <span className="text-black/45"> — {project.nextActionDetail}</span> : null}
        </p>
      ) : null}
      {project.blocker ? (
        <p className="mt-1.5 flex items-start gap-1.5 text-[11.5px] font-light text-[var(--portal-archive)]">
          <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden /> {project.blocker}
        </p>
      ) : null}
    </div>
  )
}

function ProjectionPlaceholder({ view }: { view: ProjectWorkspaceView }) {
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center rounded-[var(--portal-tab-radius)] border border-dashed border-[var(--portal-panel-border)] bg-white/15 px-6 py-8 text-center">
      <p className="text-sm font-light text-black/45">
        <span className="font-medium text-[var(--portal-navy)]">{VIEW_LABEL[view]}</span> is next in the prototype. Work Plan is the active surface.
      </p>
    </div>
  )
}

type PaneTwoProps = {
  pole: ProjectPole | null
  project: ProjectPlan | null
  activeView: ProjectWorkspaceView
  selectedNodeId: string | null
  onSelectView: (v: ProjectWorkspaceView) => void
  onSelectNode: (id: string | null) => void
}

/** Pane 2 — the dominant working surface. */
function PaneTwo({ pole, project, activeView, selectedNodeId, onSelectView, onSelectNode }: PaneTwoProps) {
  return (
    <section className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
      {!pole || !project ? (
        <div className="flex flex-1 items-center justify-center px-6 text-center text-sm font-light text-black/45">
          Choose a Pole and Project from the navigator.
        </div>
      ) : (
        <>
          <div className="border-b border-[var(--portal-panel-border)] px-4 pb-3 pt-4">
            <ProjectHeader pole={pole} project={project} />
          </div>
          <div className="overflow-x-auto px-2 pt-2">
            <nav aria-label="Project workspace views" className="portal-glass-rail flex min-w-max gap-1 p-1">
              {VIEWS.map((view) => {
                const isActive = view === activeView
                return (
                  <button
                    key={view}
                    type="button"
                    onClick={() => onSelectView(view)}
                    aria-current={isActive ? "page" : undefined}
                    className={`portal-glass-tab ${isActive ? "bg-[var(--portal-navy)] text-white shadow-sm" : ""}`}
                  >
                    {VIEW_LABEL[view]}
                  </button>
                )
              })}
            </nav>
          </div>
          <div className="flex min-h-0 flex-1 flex-col px-3 pb-3 pt-2">
            {activeView === "work-plan" ? (
              <WorkPlan project={project} selectedNodeId={selectedNodeId} onSelectNode={onSelectNode} />
            ) : (
              <ProjectionPlaceholder view={activeView} />
            )}
          </div>
        </>
      )}
    </section>
  )
}

type InspectorProps = {
  pole: ProjectPole | null
  project: ProjectPlan | null
  node: ProjectWorkNode | null
}

/** Pane 3 — persistent selected-WorkNode inspector. */
function PaneThree({ pole, project, node }: InspectorProps) {
  const context =
    node?.relations && node.relations.length > 0
      ? node.relations
      : node?.relatedLabel
        ? [node.relatedLabel]
        : []
  return (
    <section className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
      <div className="border-b border-[var(--portal-panel-border)] px-3 py-2.5">
        <p className="text-[9px] font-medium uppercase tracking-[0.18em] text-[var(--portal-gold-muted)]">Selected work</p>
      </div>
      {!node ? (
        <div className="flex flex-1 items-center justify-center px-5 text-center text-sm font-light text-black/45">
          Select a work item to inspect it.
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-3 py-3">
          <div className="flex items-start justify-between gap-2">
            <div className="min-w-0">
              <p className="truncate text-[10px] font-light uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">{node.type}</p>
              <h3 className="mt-0.5 font-serif text-lg font-light leading-tight text-[var(--portal-navy)]">{node.title}</h3>
            </div>
            <span className="flex shrink-0 items-center gap-1.5 rounded-full bg-white/50 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.1em] text-[var(--portal-navy-soft)]">
              <StatusDot status={node.status} /> {STATUS_LABEL[node.status]}
            </span>
          </div>

          {pole && project ? (
            <p className="mt-1.5 text-[10px] font-light text-black/40">
              {pole.label} <ChevronRight className="inline h-2.5 w-2.5" aria-hidden /> {project.title}
            </p>
          ) : null}

          {node.note ? <p className="mt-3 text-[12.5px] font-light leading-relaxed text-[var(--portal-navy)]">{node.note}</p> : null}

          <dl className="mt-3 space-y-2 border-t border-[var(--portal-panel-border)] pt-2 text-[12.5px] font-light">
            {node.dueLabel ? (
              <div className="flex justify-between gap-2">
                <dt className="text-black/40">Due</dt>
                <dd className="text-right text-[var(--portal-navy)]">{node.dueLabel}</dd>
              </div>
            ) : null}
            {node.owner ? (
              <div className="flex justify-between gap-2">
                <dt className="text-black/40">Owner</dt>
                <dd className="text-right text-[var(--portal-navy)]">{node.owner}</dd>
              </div>
            ) : null}
          </dl>

          {context.length > 0 ? (
            <div className="mt-3">
              <p className="text-[9px] font-medium uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">Related to</p>
              <ul className="mt-1.5 space-y-1">
                {context.map((rel) => (
                  <li key={rel} className="flex items-start gap-1.5 text-[11.5px] font-light text-[var(--portal-navy)]">
                    <FileText className="mt-0.5 h-3 w-3 shrink-0 text-black/30" aria-hidden />
                    {rel}
                  </li>
                ))}
              </ul>
            </div>
          ) : null}

          {node.actions && node.actions.length > 0 ? (
            <div className="mt-4 flex flex-wrap gap-1.5 border-t border-[var(--portal-panel-border)] pt-3">
              {node.actions.map((action) => (
                <button
                  key={action}
                  type="button"
                  className="rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 py-1.5 text-[11px] font-medium text-white transition hover:opacity-90"
                >
                  {action}
                </button>
              ))}
            </div>
          ) : null}
        </div>
      )}
    </section>
  )
}

export function ProjectsWorkspace() {
  const source = useMemo(() => new InMemoryProjectsWorkspaceSource(), [])
  const controller = useMemo(() => new ProjectsWorkspaceController(source), [source])
  const model = usePageController(controller)

  useEffect(() => {
    void controller.dispatch({ operation: "projects.load", payload: {} })
  }, [controller])

  const domains = model.data?.domains ?? []
  const poles = (model.data?.poles ?? [])
    .filter((pole) => pole.domain === model.activeDomain)
    .filter((pole) => poleMatches(pole, model.query))
  const selectedPole = (model.data?.poles ?? []).find((p) => p.id === model.selectedPoleId) ?? null
  const selectedProject = selectedPole?.projects.find((p) => p.id === model.selectedProjectId) ?? null
  const selectedNode = findWorkNode(selectedProject, model.selectedNodeId)

  if (model.error) {
    return <p className="px-4 py-6 text-sm font-light text-[var(--portal-archive)]">Could not load the Projects workspace: {model.error}</p>
  }
  if (model.loading || !model.data) {
    return <p className="px-4 py-6 text-sm font-light text-black/45">Loading projects…</p>
  }

  return (
    <div className="grid min-h-0 flex-1 gap-3 lg:h-[calc(100dvh-8.5rem)] lg:grid-cols-[minmax(350px,375px)_minmax(0,1fr)_minmax(295px,315px)]">
      <PaneOne
        domains={domains}
        activeDomain={model.activeDomain}
        query={model.query}
        poles={poles}
        expanded={model.expandedPoleIds}
        selectedPoleId={model.selectedPoleId}
        selectedProjectId={model.selectedProjectId}
        selectedNodeId={model.selectedNodeId}
        onSelectDomain={(domain) => void controller.dispatch({ operation: "projects.selectDomain", payload: { domain } })}
        onQuery={(query) => void controller.dispatch({ operation: "projects.queryChanged", payload: { query } })}
        onTogglePole={(poleId) => void controller.dispatch({ operation: "projects.togglePole", payload: { poleId } })}
        onSelectProject={(poleId, projectId) =>
          void controller.dispatch({ operation: "projects.selectProject", payload: { poleId, projectId } })
        }
        onSelectNode={(nodeId) => void controller.dispatch({ operation: "projects.selectNode", payload: { nodeId } })}
      />
      <PaneTwo
        pole={selectedPole}
        project={selectedProject}
        activeView={model.activeView}
        selectedNodeId={model.selectedNodeId}
        onSelectView={(view) => void controller.dispatch({ operation: "projects.selectView", payload: { view } })}
        onSelectNode={(nodeId) => void controller.dispatch({ operation: "projects.selectNode", payload: { nodeId } })}
      />
      <PaneThree pole={selectedPole} project={selectedProject} node={selectedNode} />
    </div>
  )
}
