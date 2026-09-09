"use client"

import { useCallback, useEffect, useMemo, useRef, useState } from "react"
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
import { Tree } from "react-arborist"
import type { NodeApi, NodeRendererProps } from "react-arborist"
import {
  buildProjectTree,
  openAncestorsForSelected,
  selectedTreeNodeId,
  type ProjectTreeNode,
} from "@/ui/projects/tree-projection"

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
    <div className="flex w-[86px] shrink-0 flex-col items-center border-r border-white/10 py-3" aria-label="Project domain">
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
            className={`group relative flex w-full flex-col items-center gap-1.5 py-2.5 transition ${isActive ? "" : "opacity-95 hover:opacity-100"}`}
          >
            <span className={`absolute inset-y-2 left-0 w-[3px] rounded-r-full transition ${isActive ? "bg-[var(--portal-gold)]" : "bg-transparent group-hover:bg-white/30"}`} />
            <span
              className={`flex h-10 w-10 items-center justify-center rounded-xl transition ${
                isActive
                  ? "bg-black/25 text-[var(--portal-gold)] shadow-sm ring-1 ring-inset ring-white/25"
                  : "bg-white/[0.07] text-white/85 group-hover:bg-white/[0.16] group-hover:text-white"
              }`}
            >
              <Icon className="h-[23px] w-[23px]" strokeWidth={1.6} aria-hidden />
            </span>
            <span
              className={`text-center text-[14px] font-medium uppercase leading-tight tracking-[0.02em] ${
                isActive ? "text-white" : "text-white/70 group-hover:text-white/95"
              }`}
            >
              {domain.shortLabel.slice(0, 12)}
            </span>
          </button>
        )
      })}
    </div>
  )
}

function useMeasuredHeight() {
  const ref = useRef<HTMLDivElement | null>(null)
  const [height, setHeight] = useState(560)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const update = () => setHeight(Math.max(200, el.clientHeight))
    update()
    const ro = new ResizeObserver(update)
    ro.observe(el)
    return () => ro.disconnect()
  }, [])
  return [ref, height] as const
}

function treeRowHeight(node: NodeApi<ProjectTreeNode>): number {
  return node.data.kind === "pole" ? 70 : node.data.kind === "project" ? 50 : 46
}

function navyWorkDot(status?: ProjectWorkStatus): string {
  if (status === "complete") return "bg-[var(--portal-success)]"
  if (status === "waiting") return "bg-[var(--portal-gold)]"
  if (status === "blocked") return "bg-[var(--portal-archive)]"
  if (status === "in-progress") return "bg-[var(--portal-gold)]/70"
  return "bg-white/30"
}

function ToggleButton({ node }: { node: NodeApi<ProjectTreeNode> }) {
  if (node.isLeaf) return <span className="w-4 shrink-0" aria-hidden />
  return (
    <button
      type="button"
      onClick={(event) => {
        event.stopPropagation()
        node.toggle()
      }}
      aria-label={node.isOpen ? "Collapse" : "Expand"}
      className="flex h-6 w-4 shrink-0 items-center justify-center rounded text-white/55 transition hover:text-white"
    >
      <ChevronDown className={`h-3.5 w-3.5 transition ${node.isOpen ? "" : "-rotate-90"}`} aria-hidden />
    </button>
  )
}

function ProjectTreeNodeView({ node, style }: NodeRendererProps<ProjectTreeNode>) {
  const d = node.data
  const selected = node.isSelected
  const focus = node.isFocused ? "ring-1 ring-inset ring-white/25" : ""

  if (d.kind === "pole") {
    const Icon = DOMAIN_ICON[d.domain ?? "properties"]
    return (
      <div style={style} className={`flex items-center gap-2 rounded-xl px-1 ${selected ? "bg-white/10 shadow-[0_2px_12px_rgba(0,0,0,0.16)]" : ""} ${focus}`}>
        <ToggleButton node={node} />
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[9px] bg-white/10 text-[var(--portal-gold)] ring-1 ring-inset ring-white/15">
          <Icon className="h-[18px] w-[18px]" strokeWidth={1.5} aria-hidden />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate font-serif text-[19px] font-light leading-tight text-white/95">{d.label}</span>
          {d.subtitle ? <span className="mt-0.5 block truncate text-[15px] font-light leading-snug text-white/60">{d.subtitle}</span> : null}
        </span>
        {typeof d.progress === "number" ? (
          <span className="flex shrink-0 items-center gap-1.5 pr-1">
            <Progress value={d.progress} className="w-11" />
            <span className="text-[14px] font-light text-white/55">{d.progress}%</span>
          </span>
        ) : null}
      </div>
    )
  }

  if (d.kind === "project") {
    const kind = (d.meta ?? "").split(" · ")[0] ?? ""
    return (
      <div style={style} className={`flex items-center gap-1.5 rounded-lg px-1 ${selected ? "bg-white/10" : ""} ${focus}`}>
        <ToggleButton node={node} />
        {kind ? <span className="shrink-0 text-[13px] font-medium uppercase tracking-[0.08em] text-[var(--portal-gold)]">{kind}</span> : null}
        <span className="min-w-0 flex-1 truncate text-[18px] font-light leading-tight text-white/95">{d.label}</span>
        {typeof d.progress === "number" ? <span className="shrink-0 pr-1 text-[14px] font-light text-white/55">{d.progress}%</span> : null}
      </div>
    )
  }

  const parts = (d.meta ?? "").split(" · ")
  return (
    <div style={style} className={`flex items-center gap-2 rounded-md px-1 ${selected ? "bg-white/15 ring-1 ring-inset ring-white/25" : ""} ${focus}`}>
      <ToggleButton node={node} />
      <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${navyWorkDot(d.status)}`} />
      <span className="min-w-0 flex-1 truncate text-[18px] font-light leading-tight text-white/95">{d.label}</span>
      {parts[0] ? <span className="shrink-0 text-[14px] font-medium uppercase tracking-[0.05em] text-[var(--portal-gold)]">{parts[0]}</span> : null}
    </div>
  )
}

type PaneOneProps = {
  domains: ReadonlyArray<{ key: ProjectDomainKey; label: string; shortLabel: string }>
  activeDomain: ProjectDomainKey
  query: string
  treeData: ProjectTreeNode[]
  selectedCompositeId: string | null
  onSelectDomain: (d: ProjectDomainKey) => void
  onQuery: (q: string) => void
  onSelectData: (node: ProjectTreeNode | null) => void
  onActivateData?: (node: ProjectTreeNode) => void
}

/** Pane 1 — ONE glass object: vertical domain tabs fused with the arborist tree. */
function PaneOne(props: PaneOneProps) {
  const activeLabel = props.domains.find((d) => d.key === props.activeDomain)?.label ?? ""
  const [treeWrapRef, height] = useMeasuredHeight()
  const initialOpen = useMemo(
    () => openAncestorsForSelected(props.treeData, props.selectedCompositeId),
    [props.treeData, props.selectedCompositeId],
  )
  const searchMatch = useCallback(
    (node: NodeApi<ProjectTreeNode>, term: string) =>
      term.trim() ? node.data.searchText.includes(term.trim().toLowerCase()) : true,
    [],
  )
  const handleSelect = useCallback(
    (nodes: NodeApi<ProjectTreeNode>[]) => {
      const first = nodes[0]
      if (first) props.onSelectData(first.data)
    },
    [props],
  )
  return (
    <section
      className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]"
      style={{ backgroundColor: "color-mix(in srgb, var(--portal-navy) 90%, transparent)" }}
    >
      <div className="flex min-h-0 flex-1">
        <DomainRail domains={props.domains} active={props.activeDomain} onSelect={props.onSelectDomain} />
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <div className="border-b border-white/10 px-3 pb-2 pt-3">
            <p className="text-[14px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)]">{activeLabel}</p>
            <label className="mt-2 flex h-11 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-white/15 bg-white/10 px-3">
              <Search className="h-4 w-4 shrink-0 text-white/50" aria-hidden />
              <input
                value={props.query}
                onChange={(e) => props.onQuery(e.target.value)}
                placeholder="Find work…"
                className="min-w-0 flex-1 bg-transparent text-[16px] font-light text-white outline-none placeholder:text-white/55"
              />
            </label>
          </div>
          <div ref={treeWrapRef} className="min-h-0 flex-1 overflow-hidden px-1 pt-1.5">
            <Tree<ProjectTreeNode>
              key={props.activeDomain}
              data={props.treeData}
              selection={props.selectedCompositeId ?? undefined}
              initialOpenState={initialOpen}
              openByDefault={false}
              searchTerm={props.query}
              searchMatch={searchMatch}
              width="100%"
              height={height}
              indent={7}
              rowHeight={treeRowHeight}
              overscanCount={6}
              disableDrag
              disableDrop
              disableEdit
              disableMultiSelection
              onSelect={handleSelect}
              onActivate={(node) => {
                if (props.onActivateData) props.onActivateData(node.data)
                if (!node.isLeaf) node.toggle()
              }}
            >
              {ProjectTreeNodeView}
            </Tree>
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

function PaneThreeHead() {
  return (
    <div className="border-b border-[var(--portal-panel-border)] px-3 py-3">
      <p className="text-[14px] font-medium uppercase tracking-[0.12em] text-[var(--portal-gold-muted)]">Selected work</p>
    </div>
  )
}

/** Pane 3 — persistent selected-WorkNode inspector (readable type sizes). */
function PaneThree({ pole, project, node }: InspectorProps) {
  if (!node) {
    return (
      <section className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
        <PaneThreeHead />
        <div className="flex flex-1 items-center justify-center px-6 text-center text-[16px] font-light text-black/45">
          Select a work item to inspect it.
        </div>
      </section>
    )
  }
  const related = node.inspector?.relatedItems ?? []
  const summary = node.inspector?.summary ?? node.note
  return (
    <section className="portal-glass-panel flex min-h-0 flex-col overflow-hidden rounded-[var(--portal-panel-radius)]">
      <PaneThreeHead />
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
          <p className="text-[15px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">{node.type}</p>
          <h3 className="mt-1 font-serif text-[22px] font-light leading-tight text-[var(--portal-navy)]">{node.title}</h3>
          <span className="mt-2 inline-flex items-center gap-2 rounded-full bg-white/50 px-3 py-1 text-[15px] font-medium text-[var(--portal-navy-soft)]">
            <StatusDot status={node.status} /> {STATUS_LABEL[node.status]}
          </span>

          {pole && project ? (
            <p className="mt-2.5 text-[15px] font-light text-black/45">
              {pole.label} <ChevronRight className="inline h-3.5 w-3.5" aria-hidden /> {project.title}
            </p>
          ) : null}

          {summary ? <p className="mt-3 text-[16px] font-light leading-relaxed text-[var(--portal-navy)]">{summary}</p> : null}

          <dl className="mt-3 space-y-2 border-y border-[var(--portal-panel-border)] py-3 text-[16px] font-light">
            {node.dueLabel ? (
              <div className="flex justify-between gap-2">
                <dt className="text-black/40">Due</dt>
                <dd className="text-right font-normal text-[var(--portal-navy)]">{node.dueLabel}</dd>
              </div>
            ) : null}
            {node.owner ? (
              <div className="flex justify-between gap-2">
                <dt className="text-black/40">Assignee</dt>
                <dd className="text-right font-normal text-[var(--portal-navy)]">{node.owner}</dd>
              </div>
            ) : null}
          </dl>

          {related.length > 0 ? (
            <div className="mt-3">
              <p className="text-[14px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">Related to</p>
              <ul className="mt-1 divide-y divide-[var(--portal-panel-border)]/70">
                {related.map((item) => (
                  <li key={item.label} className="flex items-baseline justify-between gap-3 py-2">
                    <span className="min-w-0 truncate text-[16px] font-light text-[var(--portal-navy)]">{item.label}</span>
                    {item.caption ? <span className="shrink-0 text-[15px] font-light text-black/40">{item.caption}</span> : null}
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>

        {node.actions && node.actions.length > 0 ? (
          <div className="flex flex-col gap-2 border-t border-[var(--portal-panel-border)] px-4 py-3">
            <p className="text-[14px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-blue-gray)]">Actions</p>
            {node.actions.map((action) => (
              <button
                key={action}
                type="button"
                className="w-full rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 py-2.5 text-left text-[16px] font-medium text-white transition hover:opacity-90"
              >
                {action}
              </button>
            ))}
          </div>
        ) : null}
      </div>
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
  const domainPoles = (model.data?.poles ?? []).filter((pole) => pole.domain === model.activeDomain)
  const treeData = useMemo(() => buildProjectTree(domainPoles), [domainPoles])
  const selectedCompositeId = useMemo(
    () =>
      selectedTreeNodeId(treeData, {
        poleId: model.selectedPoleId,
        projectId: model.selectedProjectId,
        nodeId: model.selectedNodeId,
      }),
    [treeData, model.selectedPoleId, model.selectedProjectId, model.selectedNodeId],
  )
  const selectedPole = (model.data?.poles ?? []).find((p) => p.id === model.selectedPoleId) ?? null
  const selectedProject = selectedPole?.projects.find((p) => p.id === model.selectedProjectId) ?? null
  const selectedNode = findWorkNode(selectedProject, model.selectedNodeId)

  const handleTreeSelect = useCallback(
    (node: ProjectTreeNode | null) => {
      if (!node) return
      if (node.kind === "pole") {
        void controller.dispatch({ operation: "projects.selectPole", payload: { poleId: node.poleId } })
      } else if (node.kind === "project") {
        void controller.dispatch({
          operation: "projects.selectProject",
          payload: { poleId: node.poleId, projectId: node.projectId ?? "" },
        })
      } else {
        void controller.dispatch({
          operation: "projects.selectProject",
          payload: { poleId: node.poleId, projectId: node.projectId ?? "" },
        })
        void controller.dispatch({ operation: "projects.selectNode", payload: { nodeId: node.workNodeId ?? null } })
      }
    },
    [controller],
  )

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
        treeData={treeData}
        selectedCompositeId={selectedCompositeId}
        onSelectDomain={(domain) => void controller.dispatch({ operation: "projects.selectDomain", payload: { domain } })}
        onQuery={(query) => void controller.dispatch({ operation: "projects.queryChanged", payload: { query } })}
        onSelectData={handleTreeSelect}
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
