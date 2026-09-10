"use client"

import { useCallback, useEffect, useMemo, useRef, useState, useTransition, type ReactNode } from "react"
import { useRouter } from "next/navigation"
import type { LucideIcon } from "lucide-react"
import {
  AlertCircle,
  Banknote,
  Building2,
  CalendarDays,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  Clock3,
  FileText,
  Flag,
  GitBranch,
  Handshake,
  Home,
  Image,
  KeyRound,
  Megaphone,
  PenLine,
  Search,
  Users,
} from "lucide-react"

import type {
  ProjectDomainKey,
  ProjectPlan,
  ProjectPole,
  ProjectSecondaryViewProvenance,
  ProjectWorkNode,
  ProjectWorkStatus,
  ProjectsWorkspaceData,
  ProjectWorkspaceView,
} from "@/ui/projects"
import {
  InMemoryProjectsWorkspaceSource,
  PROJECTS_GEOMETRY,
  PROJECTS_LONG_CONTENT,
  PROJECTS_PRIMITIVES,
  PROJECTS_SCROLL_CLASS,
  PROJECTS_SURFACE,
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
import { instantiateProjectAction, updateProjectStatusAction } from "@/app/portal/projects/actions"
import { updateWbsItemAction } from "@/app/portal/wbs/actions"

const DOMAIN_ICON: Record<ProjectDomainKey, LucideIcon> = {
  properties: Home,
  people: Users,
  deals: Handshake,
  firm: Building2,
  marketing: Megaphone,
  accounting: Banknote,
}

const PROJECT_KIND_ICON: Record<string, LucideIcon> = {
  LISTING: KeyRound,
  MARKETING: Megaphone,
  CLOSING: Handshake,
  DEAL: Handshake,
  CLIENT: Users,
  BUYER_REP: Users,
  FIRM: Building2,
  ACCOUNTING: Banknote,
}
const WORK_TYPE_ICON: Record<string, LucideIcon> = {
  contract: FileText,
  approval: PenLine,
  media: Image,
  accounting: Banknote,
  marketing: Megaphone,
  workflow: GitBranch,
  task: CheckCircle2,
  milestone: Flag,
  document: FileText,
  appointment: CalendarDays,
}
function workTypeIcon(type: string, label: string): LucideIcon {
  const found = WORK_TYPE_ICON[type]
  if (found) return found
  if (type === "group") return /client|parties|people|person|seller/i.test(label) ? Users : Home
  return FileText
}
function projectKindIcon(kind: string): LucideIcon {
  return PROJECT_KIND_ICON[kind] ?? FileText
}

const STATUS_LABEL: Record<ProjectWorkStatus, string> = {
  complete: "Complete",
  waiting: "Waiting",
  "in-progress": "In progress",
  "not-started": "Not started",
  blocked: "Blocked",
  dismissed: "Dismissed",
}

const STATUS_BAR: Record<ProjectWorkStatus, string> = {
  complete: "bg-[var(--portal-success)]",
  waiting: "bg-[var(--portal-gold)]",
  "in-progress": "bg-[var(--portal-blue-gray)]",
  "not-started": "bg-black/20",
  blocked: "bg-[var(--portal-archive)]",
  dismissed: "bg-black/35",
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
  if (status === "dismissed") return <Circle className="h-3.5 w-3.5 shrink-0 text-black/35" aria-hidden />
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
  const visit = (nodes: readonly ProjectWorkNode[]): ProjectWorkNode | null => {
    for (const node of nodes) {
      if (node.id === nodeId) return node
      const child = node.children ? visit(node.children) : null
      if (child) return child
    }
    return null
  }
  return visit(project.workNodes)
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
  if (status === "dismissed") return "bg-white/20"
  return "bg-white/30"
}

function statusGlyphColor(status?: ProjectWorkStatus): string {
  if (status === "complete") return "text-[var(--portal-success)]"
  if (status === "blocked") return "text-[var(--portal-archive)]"
  if (status === "waiting") return "text-[var(--portal-gold)]"
  if (status === "in-progress") return "text-[var(--portal-gold)]/80"
  if (status === "dismissed") return "text-white/35"
  return "text-white/55"
}

function NodeGlyph({ icon, className }: { icon: LucideIcon; className?: string }) {
  const Icon = icon
  return <Icon className={`h-[18px] w-[18px] shrink-0 ${className ?? "text-[var(--portal-gold)]"}`} strokeWidth={1.6} aria-hidden />
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
          <span className="block truncate font-serif text-[23px] font-bold leading-tight text-white/95">{d.label}</span>
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
      <div style={style} className={`flex items-center gap-2 rounded-lg px-1 ${selected ? "bg-white/10" : ""} ${focus}`}>
        <ToggleButton node={node} />
        {kind ? <NodeGlyph icon={projectKindIcon(kind)} /> : null}
        <span className="min-w-0 flex-1 truncate text-[19px] font-light leading-tight text-white/95">{d.label}</span>
        {typeof d.progress === "number" ? <span className="shrink-0 pr-1 text-[14px] font-light text-white/55">{d.progress}%</span> : null}
      </div>
    )
  }

  const parts = (d.meta ?? "").split(" · ")
  return (
    <div style={style} className={`flex items-center gap-2 rounded-md px-1 ${selected ? "bg-white/15 ring-1 ring-inset ring-white/25" : ""} ${focus}`}>
      <ToggleButton node={node} />
      <NodeGlyph icon={workTypeIcon(parts[0] ?? "", d.label)} className={statusGlyphColor(d.status)} />
      <span className="min-w-0 flex-1 truncate text-[17px] font-light leading-tight text-white/95">{d.label}</span>
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
    <section className={PROJECTS_SURFACE.navigator.className}>
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
        className={PROJECTS_PRIMITIVES.row({ selected })}
      >
        <StatusIcon status={node.status} />
        <span className="min-w-0 flex-1">
          <span className={`block ${PROJECTS_LONG_CONTENT.label.classes} text-[13.5px] text-[var(--portal-navy)]`}>{node.title}</span>
          <span className="block truncate text-[10px] font-light text-black/40">{node.type}</span>
        </span>
        <span className="text-right">
          <span className="block text-[10px] font-light text-[var(--portal-blue-gray)]">{node.dueLabel ?? "—"}</span>
          <span className="block truncate text-[9px] font-light text-black/40">{node.owner ?? "—"}</span>
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
    <div className={`flex-1 ${PROJECTS_SCROLL_CLASS} rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 px-1.5 py-1`}>
      <div className="grid grid-cols-[22px_minmax(0,1fr)_72px_88px] gap-2 border-b border-[var(--portal-panel-border)]/70 px-2 py-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-black/40"><span /><span>Work item</span><span className="text-right">Due</span><span className="text-right">Owner</span></div>
      <ul className="space-y-0.5">
        {project.workNodes.map((node) => (
          <WorkPlanNode key={node.id} node={node} selectedNodeId={selectedNodeId} onSelectNode={onSelectNode} />
        ))}
      </ul>
    </div>
  )
}

function ProjectCalendar({ project }: { project: ProjectPlan }) {
  const items = project.calendarItems ?? []
  if (!items.length) return <ProjectionState view="calendar" provenance={project.provenance} />
  return (
    <div className="min-h-0 flex-1 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 p-2">
      <ul className="divide-y divide-[var(--portal-panel-border)]/70">
        {items.map((item) => (
          <li key={item.id} className="flex items-center gap-3 px-2 py-2.5">
            <time dateTime={item.startAt} className="w-24 shrink-0 text-[11px] font-medium text-[var(--portal-gold-muted)]">{new Date(item.startAt).toLocaleDateString(undefined, { month: "short", day: "numeric" })}</time>
            <span className="min-w-0 flex-1 truncate text-[13px] text-[var(--portal-navy)]">{item.title}</span>
            <span className="shrink-0 text-[10px] font-light text-black/40">{item.owner ?? "Unassigned"}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}

function ProjectDocuments({ project }: { project: ProjectPlan }) {
  const documents = project.documents ?? []
  if (!documents.length) return <ProjectionState view="documents" provenance={project.provenance} />
  return (
    <div className="min-h-0 flex-1 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 p-2">
      <ul className="divide-y divide-[var(--portal-panel-border)]/70">
        {documents.map((document) => (
          <li key={document.id} className="flex items-center gap-3 px-2 py-2.5">
            <FileText className="h-4 w-4 shrink-0 text-[var(--portal-gold-muted)]" aria-hidden />
            <span className="min-w-0 flex-1 truncate text-[13px] text-[var(--portal-navy)]">{document.title}</span>
            <span className="shrink-0 text-[10px] font-medium uppercase text-black/40">{document.state}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}

function ProjectActivity({ project }: { project: ProjectPlan }) {
  const activity = project.activity ?? []
  if (!activity.length) return <ProjectionState view="activity" provenance={project.provenance} />
  return (
    <div className="min-h-0 flex-1 overflow-y-auto rounded-[var(--portal-tab-radius)] border border-white/40 bg-white/20 p-2">
      <ul className="divide-y divide-[var(--portal-panel-border)]/70">
        {activity.map((entry) => (
          <li key={entry.id} className="px-2 py-2.5">
            <div className="flex items-center justify-between gap-3">
              <span className="text-[11px] font-medium uppercase tracking-[0.08em] text-[var(--portal-gold-muted)]">{entry.channel}{entry.direction ? ` · ${entry.direction}` : ""}</span>
              <time dateTime={entry.occurredAt} className="text-[10px] font-light text-black/40">{entry.occurredAtLabel}</time>
            </div>
            <p className="mt-1 text-[13px] text-[var(--portal-navy)]">{entry.title ?? entry.summary ?? "Activity recorded"}</p>
            {entry.summary && entry.title ? <p className="mt-0.5 truncate text-[11px] font-light text-black/45">{entry.summary}</p> : null}
          </li>
        ))}
      </ul>
    </div>
  )
}

function ProjectHeader({ pole, project, onStatusChange, statusPending }: { pole: ProjectPole; project: ProjectPlan; onStatusChange?: (status: "open" | "doing" | "done" | "archived") => void; statusPending?: boolean }) {
  return (
    <div>
      <p className="flex items-center gap-1 text-[10px] font-light uppercase tracking-[0.16em] text-[var(--portal-blue-gray)]">
        {pole.label} <ChevronRight className="h-3 w-3" aria-hidden /> {project.kind}
      </p>
      <div className="mt-1 flex flex-wrap items-end justify-between gap-3">
        <div className="min-w-0">
          <h2 className="font-serif text-[26px] font-light leading-tight text-[var(--portal-navy)]">{project.title}</h2>
          {project.contextLabels?.length ? <p className="mt-1 truncate text-[12px] font-light text-black/45">{project.contextLabels.join(" · ")}</p> : null}
        </div>
        <div className="flex items-center gap-2">
          <select
            value={project.status === "active" ? "doing" : project.status === "complete" ? "done" : project.status === "archived" ? "archived" : "open"}
            disabled={!onStatusChange || statusPending}
            onChange={(event) => onStatusChange?.(event.target.value as "open" | "doing" | "done" | "archived")}
            aria-label="Project status"
            className="rounded-full bg-white/50 px-2.5 py-1 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] outline-none"
          >
            <option value="open">Open</option>
            <option value="doing">In progress</option>
            <option value="done">Complete</option>
            <option value="archived">Archived</option>
          </select>
          <span className="text-[11px] font-light text-[var(--portal-blue-gray)]">{project.progress}%</span>
        </div>
      </div>
      <div className="mt-2 w-full max-w-[280px]">
        <Progress value={project.progress} />
      </div>
      {project.playbookId ? (
        <div className="mt-2 flex flex-wrap gap-1.5">
          {project.playbookId ? <span className="rounded-full bg-[var(--portal-gold)]/15 px-2 py-0.5 text-[10px] font-medium text-[var(--portal-gold-muted)]">{project.playbookId}{project.playbookVersion ? ` v${project.playbookVersion}` : ""}</span> : null}
        </div>
      ) : null}
      {project.nextAction ? (
        <div className="mt-3 rounded-xl border border-[var(--portal-gold)]/25 bg-[var(--portal-gold)]/10 px-3 py-2.5">
          <p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">Next action</p>
          <p className="mt-1 text-[13px] font-medium text-[var(--portal-navy)]">{project.nextAction}</p>
          {project.nextActionDetail ? <p className="mt-0.5 text-[11px] font-light text-black/45">{project.nextActionDetail}</p> : null}
        </div>
      ) : null}
      {project.blocker ? (
        <p className="mt-1.5 flex items-start gap-1.5 text-[11.5px] font-light text-[var(--portal-archive)]">
          <AlertCircle className="mt-0.5 h-3.5 w-3.5 shrink-0" aria-hidden /> {project.blocker}
        </p>
      ) : null}
    </div>
  )
}

function WorkspaceMessage({
  state,
  tone = "muted",
  children,
}: {
  state: "loading" | "empty" | "unauthorized" | "failure"
  tone?: "muted" | "notice" | "error"
  children: ReactNode
}) {
  const color = tone === "error" ? "text-[var(--portal-archive)]" : "text-black/45"
  return <p data-state={state} className={`px-4 py-6 text-sm font-light ${color}`}>{children}</p>
}

function ProjectionState({
  view,
  provenance,
}: {
  view: ProjectWorkspaceView
  provenance?: ProjectSecondaryViewProvenance
}) {
  const label = VIEW_LABEL[view]
  const status = view === "documents" ? provenance?.documents : view === "activity" ? provenance?.activity : undefined
  let message: string
  if (status === "unlinked") {
    const anchor = view === "documents" ? "property" : "contact"
    message = `This project is not anchored to a ${anchor}, so ${label.toLowerCase()} cannot be linked.`
  } else if (status === "empty") {
    message = `No ${label.toLowerCase()} are linked to this project yet.`
  } else {
    message = `${label} is not available in this workspace yet.`
  }
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center rounded-[var(--portal-tab-radius)] border border-dashed border-[var(--portal-panel-border)] bg-white/15 px-6 py-8 text-center">
      <p className="text-sm font-light text-black/45">
        <span className="font-medium text-[var(--portal-navy)]">{label}</span> — {message}
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
  onStatusChange?: (status: "open" | "doing" | "done" | "archived") => void
  statusPending?: boolean
}

/** Pane 2 — the dominant working surface. */
function PaneTwo({ pole, project, activeView, selectedNodeId, onSelectView, onSelectNode, onStatusChange, statusPending }: PaneTwoProps) {
  return (
    <section className={PROJECTS_SURFACE.canvas.className}>
      {!pole || !project ? (
        <div className="flex flex-1 items-center justify-center px-6 text-center text-sm font-light text-black/45">
          Choose a Pole and Project from the navigator.
        </div>
      ) : (
        <>
          <div className="border-b border-[var(--portal-panel-border)] px-4 pb-3 pt-4">
            <ProjectHeader pole={pole} project={project} onStatusChange={onStatusChange} statusPending={statusPending} />
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
            ) : activeView === "calendar" ? (
              <ProjectCalendar project={project} />
            ) : activeView === "documents" ? (
              <ProjectDocuments project={project} />
            ) : activeView === "activity" ? (
              <ProjectActivity project={project} />
            ) : (
              <ProjectionState view={activeView} provenance={project.provenance} />
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
  onSaved?: () => void
}

function PaneThreeHead() {
  return (
    <div className="border-b border-[var(--portal-panel-border)] px-3 py-3">
      <p className="text-[14px] font-medium uppercase tracking-[0.12em] text-[var(--portal-gold-muted)]">Selected work</p>
    </div>
  )
}

/** Pane 3 — persistent selected-WorkNode inspector (readable type sizes). */
function PaneThree({ pole, project, node, onSaved }: InspectorProps) {
  const [status, setStatus] = useState(node?.status ?? "not-started")
  const [dueAt, setDueAt] = useState(node?.dueAt?.slice(0, 10) ?? "")
  const [owner, setOwner] = useState(node?.owner ?? "")
  const [notes, setNotes] = useState(node?.note ?? "")
  const [saving, startSaving] = useTransition()
  const [saveError, setSaveError] = useState<string | null>(null)
  useEffect(() => {
    setStatus(node?.status ?? "not-started")
    setDueAt(node?.dueAt?.slice(0, 10) ?? "")
    setOwner(node?.owner ?? "")
    setNotes(node?.note ?? "")
    setSaveError(null)
  }, [node])
  if (!node) {
    return (
      <section className={PROJECTS_SURFACE.inspector.className}>
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
    <section className={PROJECTS_SURFACE.inspector.className}>
      <PaneThreeHead />
      <div className="flex min-h-0 flex-1 flex-col">
        <div className={`flex-1 ${PROJECTS_SCROLL_CLASS} px-4 py-3`}>
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

          <div className="mt-3 space-y-2 border-y border-[var(--portal-panel-border)] py-3">
            <label className="flex items-center justify-between gap-3 text-[13px] font-light text-black/45">
              Status
              <select value={status} onChange={(event) => setStatus(event.target.value as typeof status)} className="rounded border border-[var(--portal-panel-border)] bg-white/60 px-2 py-1 text-[13px] text-[var(--portal-navy)]">
                <option value="not-started">Not started</option><option value="in-progress">In progress</option><option value="complete">Complete</option><option value="dismissed">Dismissed</option>
              </select>
            </label>
            <label className="flex items-center justify-between gap-3 text-[13px] font-light text-black/45">
              Due date
              <input type="date" value={dueAt} onChange={(event) => setDueAt(event.target.value)} className="rounded border border-[var(--portal-panel-border)] bg-white/60 px-2 py-1 text-[13px] text-[var(--portal-navy)]" />
            </label>
            <label className="flex items-center justify-between gap-3 text-[13px] font-light text-black/45">
              Assignee
              <input value={owner} onChange={(event) => setOwner(event.target.value)} placeholder="Unassigned" className="w-32 rounded border border-[var(--portal-panel-border)] bg-white/60 px-2 py-1 text-right text-[13px] text-[var(--portal-navy)]" />
            </label>
            <label className="block text-[13px] font-light text-black/45">
              Notes
              <textarea value={notes} onChange={(event) => setNotes(event.target.value)} rows={3} className={`mt-1 resize-y ${PROJECTS_PRIMITIVES.input()}`} />
            </label>
            {saveError ? <p className="text-[12px] text-[var(--portal-archive)]">{saveError}</p> : null}
            <button type="button" disabled={saving} onClick={() => startSaving(async () => { const result = await updateWbsItemAction({ id: node.id, status: status === "complete" ? "done" : status === "dismissed" ? "dismissed" : status === "in-progress" ? "doing" : "open", dueAt: dueAt ? new Date(`${dueAt}T12:00:00`).toISOString() : null, owner: owner.trim() || null, notes }); if (!result.ok) setSaveError(result.message); else onSaved?.() })} className="rounded-lg bg-[var(--portal-navy)] px-3 py-2 text-[12px] font-medium text-white disabled:opacity-50">
              {saving ? "Saving…" : "Save work item"}
            </button>
            <div className="flex gap-2">
              <button type="button" disabled={saving || status === "complete"} onClick={() => startSaving(async () => { const result = await updateWbsItemAction({ id: node.id, status: "done" }); if (!result.ok) setSaveError(result.message); else onSaved?.() })} className="flex-1 rounded-lg border border-[var(--portal-success)]/40 px-3 py-2 text-[12px] font-medium text-[var(--portal-success)] disabled:opacity-40">Complete</button>
              <button type="button" disabled={saving || status === "dismissed"} onClick={() => startSaving(async () => { const result = await updateWbsItemAction({ id: node.id, status: "dismissed" }); if (!result.ok) setSaveError(result.message); else onSaved?.() })} className="flex-1 rounded-lg border border-[var(--portal-archive)]/40 px-3 py-2 text-[12px] font-medium text-[var(--portal-archive)] disabled:opacity-40">Dismiss</button>
            </div>
          </div>

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
export function ProjectsWorkspace({
  initialData,
  loadError,
}: {
  initialData: ProjectsWorkspaceData | null
  loadError?: string | null
}) {
  const router = useRouter()
  const [newProjectOpen, setNewProjectOpen] = useState(false)
  const [newProjectName, setNewProjectName] = useState("")
  const [newProjectError, setNewProjectError] = useState<string | null>(null)
  const [isCreating, startCreating] = useTransition()
  const [isUpdatingStatus, startUpdatingStatus] = useTransition()
  const source = useMemo(
    () => new InMemoryProjectsWorkspaceSource(initialData ?? { domains: [], poles: [] }),
    [initialData],
  )
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

  const createProject = useCallback(() => {
    const name = newProjectName.trim()
    if (!name) {
      setNewProjectError("Enter a project name.")
      return
    }
    setNewProjectError(null)
    startCreating(async () => {
      const result = await instantiateProjectAction({
        id: crypto.randomUUID(),
        name,
        projectType: "listing",
        playbookId: "listing-onboarding",
        playbookVersion: 1,
        areas: ["clients", "properties", "contracts", "media", "marketing", "accounting"],
      })
      if (!result.ok) {
        setNewProjectError(result.message)
        return
      }
      setNewProjectName("")
      setNewProjectOpen(false)
      router.refresh()
    })
  }, [newProjectName, router])

  const updateStatus = useCallback((status: "open" | "doing" | "done" | "archived") => {
    if (!selectedProject) return
    startUpdatingStatus(async () => {
      const result = await updateProjectStatusAction(selectedProject.id, status)
      if (!result.ok) setNewProjectError(result.message)
      else router.refresh()
    })
  }, [router, selectedProject])

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

  const loadState = model.data?.loadState ?? initialData?.loadState
  const serverStatus = loadState?.status

  if (loadError) {
    return <WorkspaceMessage state="failure" tone="error">{loadError}</WorkspaceMessage>
  }
  if (serverStatus === "unauthorized") {
    return <WorkspaceMessage state="unauthorized" tone="notice">{loadState?.message ?? "You are not authorized to view the Projects workspace."}</WorkspaceMessage>
  }
  if (serverStatus === "failure") {
    return <WorkspaceMessage state="failure" tone="error">{loadState?.message ?? "The Projects workspace could not be loaded."}</WorkspaceMessage>
  }
  if (model.error) {
    return <WorkspaceMessage state="failure" tone="error">Could not load the Projects workspace: {model.error}</WorkspaceMessage>
  }
  if (model.loading || !model.data) {
    return <WorkspaceMessage state="loading">Loading projects…</WorkspaceMessage>
  }
  if (serverStatus === "empty" || model.data.poles.length === 0) {
    return <WorkspaceMessage state="empty" tone="notice">No projects are available in this workspace yet.</WorkspaceMessage>
  }

  return (
    <div className="relative flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex shrink-0 justify-end">
        <button
          type="button"
          onClick={() => { setNewProjectError(null); setNewProjectOpen(true) }}
          className="rounded-full bg-[var(--portal-navy)] px-3.5 py-2 text-[12px] font-medium text-white shadow-sm transition hover:opacity-90"
        >
          New Project
        </button>
      </div>
      <div className={PROJECTS_GEOMETRY.gridClassName}>
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
        onStatusChange={updateStatus}
        statusPending={isUpdatingStatus}
      />
      <PaneThree pole={selectedPole} project={selectedProject} node={selectedNode} onSaved={() => router.refresh()} />
      </div>
      {newProjectOpen ? (
        <div className="absolute right-0 top-10 z-20 w-[min(360px,calc(100vw-2rem))] rounded-2xl border border-[var(--portal-panel-border)] bg-white p-4 shadow-xl">
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">New project</p>
              <h2 className="mt-1 font-serif text-[22px] font-light text-[var(--portal-navy)]">Listing onboarding</h2>
            </div>
            <button type="button" onClick={() => setNewProjectOpen(false)} className="text-xl font-light text-black/40" aria-label="Close">×</button>
          </div>
          <label className="mt-4 block text-[12px] font-medium text-[var(--portal-navy)]">
            Project name
            <input
              autoFocus
              value={newProjectName}
              onChange={(event) => setNewProjectName(event.target.value)}
              onKeyDown={(event) => { if (event.key === "Enter") createProject() }}
              placeholder="e.g. Sunset Point Listing"
              className={`mt-1.5 h-10 ${PROJECTS_PRIMITIVES.input()}`}
            />
          </label>
          {newProjectError ? <p className="mt-2 text-[12px] text-[var(--portal-archive)]">{newProjectError}</p> : null}
          <button type="button" disabled={isCreating} onClick={createProject} className="mt-4 w-full rounded-lg bg-[var(--portal-navy)] px-3 py-2.5 text-[13px] font-medium text-white disabled:opacity-50">
            {isCreating ? "Creating…" : "Create listing project"}
          </button>
        </div>
      ) : null}
    </div>
  )
}
