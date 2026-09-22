'use client'

import { useEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import type { LucideIcon } from 'lucide-react'
import {
  Banknote,
  Building2,
  CheckCircle2,
  ChevronDown,
  FileText,
  Flag,
  GitBranch,
  Handshake,
  Home,
  Image,
  KeyRound,
  ListChecks,
  Megaphone,
  Search,
  PenLine,
  Users,
} from 'lucide-react'
import { Tree } from 'react-arborist'
import type { NodeApi, NodeRendererProps } from 'react-arborist'

import type { ILink, ITask } from '@svar-ui/react-gantt'
import { FullCalendarCandidate } from '@/components/portal/fullcalendar-candidate'
import { ProjectFilemanager } from '@/components/portal/project-filemanager'
import { ProjectTimeline } from '@/components/portal/project-timeline'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'
import type { ProjectAssetBrowserItem } from '@/ui/projects/documents-projection'
import { mapProjectToTimeline } from '@/ui/projects/timeline-projection'
import type { ProjectPlan, ProjectWorkNode, ProjectWorkStatus } from '@/ui/projects/model'


type ProjectDomainKey =
  | 'properties'
  | 'people'
  | 'deals'
  | 'firm'
  | 'marketing'
  | 'accounting'

type NavigatorProject = {
  id: string
  name: string
  status: string
  areas: string[]
  projectType?: string | null
  personId?: string | null
  propertyId?: string | null
  contractId?: string | null
}

type NavigatorEntity = {
  entityType: string
  id: string
}

type NavigatorItem = {
  id: string
  title: string
  category: string
  status: string
  projectId?: string | null
  parentId?: string | null
  dueAt?: string | null
  owner?: string | null
  order?: number | null
  entity?: NavigatorEntity | null
}

type NavigatorPayload = {
  projects: NavigatorProject[]
  items: NavigatorItem[]
  identityNames: Record<string, string>
  activeDomain: ProjectDomainKey
  catchUp: boolean
  selectedProjectId?: string | null
  selectedNodeId?: string | null
  query: string
}

type NavigatorNode = {
  id: string
  kind: 'pole' | 'project' | 'work'
  label: string
  domain?: ProjectDomainKey
  projectId?: string
  workNodeId?: string
  status?: string
  meta?: string
  subtitle?: string
  searchText: string
  progress?: number
  children?: NavigatorNode[]
}

const DOMAIN_META: Array<{ key: ProjectDomainKey; label: string; icon: LucideIcon }> = [
  { key: 'properties', label: 'Properties', icon: Home },
  { key: 'people', label: 'People', icon: Users },
  { key: 'deals', label: 'Deals', icon: Handshake },
  { key: 'firm', label: 'Firm', icon: Building2 },
  { key: 'marketing', label: 'Marketing', icon: Megaphone },
  { key: 'accounting', label: 'Accounting', icon: Banknote },
]

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
}

function projectKindIcon(kind: string): LucideIcon {
  return PROJECT_KIND_ICON[kind.toUpperCase()] ?? FileText
}

function workType(category: string): string {
  if (category === 'contracts') return 'contract'
  if (category === 'media') return 'media'
  if (category === 'marketing') return 'workflow'
  if (category === 'accounting') return 'accounting'
  if (category === 'clients' || category === 'properties') return 'group'
  return 'task'
}

function workTypeIcon(type: string, label: string): LucideIcon {
  const found = WORK_TYPE_ICON[type]
  if (found) return found
  if (type === 'group') return /client|parties|people|person|seller/i.test(label) ? Users : Home
  return FileText
}

function workStatus(status: string): string {
  if (status === 'done') return 'complete'
  if (status === 'doing') return 'in-progress'
  if (status === 'dismissed') return 'dismissed'
  return 'not-started'
}

function statusLabel(status: string): string {
  if (status === 'done') return 'complete'
  if (status === 'doing') return 'in progress'
  if (status === 'dismissed') return 'dismissed'
  return 'not started'
}

function navigatorStatusClass(status?: string): string {
  if (status === 'complete') return 'text-[var(--portal-success)]'
  if (status === 'blocked') return 'text-[var(--portal-archive)]'
  if (status === 'waiting') return 'text-[var(--portal-gold)]'
  if (status === 'in-progress') return 'text-[var(--portal-gold)]/80'
  if (status === 'dismissed') return 'text-white/35'
  return 'text-white/55'
}

function categoryDomain(category: string): ProjectDomainKey {
  if (category === 'properties' || category === 'media') return 'properties'
  if (category === 'clients') return 'people'
  if (category === 'contracts') return 'deals'
  if (category === 'marketing') return 'marketing'
  if (category === 'accounting') return 'accounting'
  return 'firm'
}

function entityDomain(entityType: string): ProjectDomainKey | null {
  if (entityType === 'property') return 'properties'
  if (entityType === 'person') return 'people'
  if (entityType === 'contract' || entityType === 'deal') return 'deals'
  return null
}

function projectInDomain(project: NavigatorProject, items: NavigatorItem[], domain: ProjectDomainKey): boolean {
  if (domain === 'properties' && project.propertyId) return true
  if (domain === 'people' && project.personId) return true
  if (domain === 'deals' && project.contractId) return true
  if (project.areas.some((area) => categoryDomain(area) === domain)) return true
  return items.some((item) => item.entity && entityDomain(item.entity.entityType) === domain)
}

function dueLabel(value?: string | null): string {
  if (!value) return ''
  const key = /^(\d{4}-\d{2}-\d{2})/.exec(value)?.[1]
  if (!key) return ''
  const date = new Date(key + 'T12:00:00')
  if (Number.isNaN(date.getTime())) return ''
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}

function compareItems(left: NavigatorItem, right: NavigatorItem): number {
  const order = (left.order ?? Number.MAX_SAFE_INTEGER) - (right.order ?? Number.MAX_SAFE_INTEGER)
  if (order !== 0) return order
  return (left.dueAt ?? '9999').localeCompare(right.dueAt ?? '9999') || left.id.localeCompare(right.id)
}

function dispatchNavigatorIntent(intent: Record<string, string>) {
  const bridge = document.getElementById('project-island-bridge')
  if (!(bridge instanceof HTMLButtonElement)) return
  bridge.setAttribute('data-intent', JSON.stringify(intent))
  bridge.click()
}

function ProjectProgress({ value, className }: { value: number; className?: string }) {
  const clamped = Math.max(0, Math.min(100, value))
  return (
    <div className={['h-1 w-full overflow-hidden rounded-full bg-white/15', className ?? ''].join(' ')}>
      <div
        className="h-full rounded-full bg-[var(--portal-gold)] transition-[width] duration-300"
        style={{ width: String(clamped) + '%' }}
      />
    </div>
  )
}

function buildNavigatorTree(payload: NavigatorPayload): NavigatorNode[] {
  const itemsByProject = new Map<string, NavigatorItem[]>()
  for (const item of payload.items) {
    if (!item.projectId) continue
    const bucket = itemsByProject.get(item.projectId) ?? []
    bucket.push(item)
    itemsByProject.set(item.projectId, bucket)
  }

  const workChildren = (
    poleId: string,
    projectId: string,
    parentId: string | null,
    projectItems: NavigatorItem[],
    seen: Set<string>,
  ): NavigatorNode[] =>
    projectItems
      .filter((item) => (item.parentId ?? null) === parentId && !seen.has(item.id))
      .sort(compareItems)
      .map((item) => {
        const branchSeen = new Set(seen)
        branchSeen.add(item.id)
        const type = workType(item.category)
        const status = workStatus(item.status)
        const meta = [type, statusLabel(item.status), dueLabel(item.dueAt)].filter(Boolean).join(' · ')
        const children = workChildren(poleId, projectId, item.id, projectItems, branchSeen)
        return {
          id: poleId + '::' + projectId + '::' + item.id,
          kind: 'work' as const,
          label: item.title,
          projectId,
          workNodeId: item.id,
          status,
          meta,
          searchText: [
            item.title,
            type,
            status,
            item.owner ?? '',
            dueLabel(item.dueAt),
            ...children.map((child) => child.searchText),
          ]
            .filter(Boolean)
            .join(' ')
            .toLowerCase(),
          children,
        }
      })

  type PoleBucket = {
    id: string
    domain: ProjectDomainKey
    label: string
    subtitle: string
    projects: NavigatorProject[]
  }

  const buckets = new Map<string, PoleBucket>()
  const addToBucket = (bucket: PoleBucket, project: NavigatorProject) => {
    const existing = buckets.get(bucket.id)
    if (existing) {
      if (!existing.projects.some((candidate) => candidate.id === project.id)) existing.projects.push(project)
    } else {
      buckets.set(bucket.id, { ...bucket, projects: [project] })
    }
  }

  for (const project of payload.projects) {
    const projectItems = itemsByProject.get(project.id) ?? []
    if (!projectInDomain(project, projectItems, payload.activeDomain)) continue

    const anchors = new Map<string, { entityType: string; id: string }>()
    if (project.propertyId) anchors.set('property:' + project.propertyId, { entityType: 'property', id: project.propertyId })
    if (project.personId) anchors.set('person:' + project.personId, { entityType: 'person', id: project.personId })
    if (project.contractId) anchors.set('contract:' + project.contractId, { entityType: 'contract', id: project.contractId })
    for (const item of projectItems) {
      if (!item.entity) continue
      anchors.set(item.entity.entityType + ':' + item.entity.id, item.entity)
    }

    const matching = [...anchors.values()].filter((anchor) => entityDomain(anchor.entityType) === payload.activeDomain)
    if (matching.length > 0) {
      for (const anchor of matching) {
        const key = anchor.entityType + ':' + anchor.id
        addToBucket(
          {
            id: 'entity-' + anchor.entityType + '-' + anchor.id,
            domain: payload.activeDomain,
            label: payload.identityNames[key] ?? anchor.id,
            subtitle:
              anchor.entityType === 'person'
                ? 'Client'
                : anchor.entityType === 'property'
                  ? 'Property'
                  : anchor.entityType === 'contract'
                    ? 'Contract'
                    : 'Workspace',
            projects: [],
          },
          project,
        )
      }
      continue
    }

    const meta = DOMAIN_META.find((entry) => entry.key === payload.activeDomain)
    addToBucket(
      {
        id: 'collection-' + payload.activeDomain,
        domain: payload.activeDomain,
        label: meta?.label ?? 'Projects',
        subtitle: 'Project collection',
        projects: [],
      },
      project,
    )
  }

  const roots: NavigatorNode[] = []
  for (const bucket of buckets.values()) {
    if (bucket.id.startsWith('collection-')) {
      bucket.subtitle = String(bucket.projects.length) + (bucket.projects.length === 1 ? ' project' : ' projects')
    }
    const projectNodes = bucket.projects.map((project) => {
      const projectItems = itemsByProject.get(project.id) ?? []
      const planned = projectItems.filter((item) => item.status !== 'dismissed').length
      const done = projectItems.filter((item) => item.status === 'done').length
      const progress = planned ? Math.round((done / planned) * 100) : 0
      const kind = String(project.projectType ?? project.areas[0] ?? 'WORK').toUpperCase()
      const meta = kind + ' · ' + (project.status === 'doing' ? 'In progress' : project.status === 'done' ? 'Complete' : project.status === 'archived' ? 'Archived' : 'Open')
      const work = workChildren(bucket.id, project.id, null, projectItems, new Set<string>())
      return {
        id: bucket.id + '::' + project.id,
        kind: 'project' as const,
        label: project.name,
        projectId: project.id,
        progress,
        meta,
        searchText: [project.name, kind, meta, ...work.map((node) => node.searchText)].join(' ').toLowerCase(),
        children: work,
      }
    })
    const progress = projectNodes.length
      ? Math.round(projectNodes.reduce((sum, project) => sum + (project.progress ?? 0), 0) / projectNodes.length)
      : 0
    roots.push({
      id: bucket.id,
      kind: 'pole',
      label: bucket.label,
      domain: bucket.domain,
      subtitle: bucket.subtitle,
      progress,
      searchText: [bucket.label, bucket.subtitle, ...projectNodes.map((project) => project.searchText)].join(' ').toLowerCase(),
      children: projectNodes,
    })
  }

  return roots
}

function selectedNavigatorId(
  nodes: NavigatorNode[],
  selectedProjectId?: string | null,
  selectedNodeId?: string | null,
): string | undefined {
  if (!selectedProjectId) return undefined
  const visit = (node: NavigatorNode): string | undefined => {
    if (selectedNodeId && node.workNodeId === selectedNodeId && node.projectId === selectedProjectId) return node.id
    for (const child of node.children ?? []) {
      const found = visit(child)
      if (found) return found
    }
    if (!selectedNodeId && node.kind === 'project' && node.projectId === selectedProjectId) return node.id
    return undefined
  }
  for (const node of nodes) {
    const found = visit(node)
    if (found) return found
  }
  for (const node of nodes) {
    const project = node.children?.find((child) => child.kind === 'project' && child.projectId === selectedProjectId)
    if (project) return project.id
  }
  return undefined
}

function openAncestors(nodes: NavigatorNode[], selectedId?: string): Record<string, boolean> {
  const open: Record<string, boolean> = {}
  if (!selectedId) return open
  const findPath = (branch: NavigatorNode[]): NavigatorNode[] => {
    for (const node of branch) {
      if (node.id === selectedId) return [node]
      const nested = node.children ? findPath(node.children) : []
      if (nested.length) return [node, ...nested]
    }
    return []
  }
  const path = findPath(nodes)
  for (const node of path.slice(0, -1)) open[node.id] = true
  return open
}

function NavigatorToggle({ node }: { node: NodeApi<NavigatorNode> }) {
  if (node.isLeaf) return <span className="w-4 shrink-0" aria-hidden />
  return (
    <button
      type="button"
      onClick={(event) => {
        event.stopPropagation()
        node.toggle()
      }}
      aria-label={node.isOpen ? 'Collapse' : 'Expand'}
      className="flex h-6 w-4 shrink-0 items-center justify-center rounded text-white/55 transition hover:text-white"
    >
      <ChevronDown className={['h-3.5 w-3.5 transition', node.isOpen ? '' : '-rotate-90'].join(' ')} aria-hidden />
    </button>
  )
}

function NavigatorGlyph({ icon, className }: { icon: LucideIcon; className?: string }) {
  const Icon = icon
  return (
    <Icon
      className={['h-[18px] w-[18px] shrink-0', className ?? 'text-[var(--portal-gold)]'].join(' ')}
      strokeWidth={1.6}
      aria-hidden
    />
  )
}

function NavigatorNodeView({ node, style }: NodeRendererProps<NavigatorNode>) {
  const item = node.data
  const selected = node.isSelected
  const focused = node.isFocused ? 'ring-1 ring-inset ring-white/25' : ''

  if (item.kind === 'pole') {
    const Icon = DOMAIN_META.find((entry) => entry.key === item.domain)?.icon ?? Home
    return (
      <div
        style={style}
        className={[
          'flex items-center gap-2 rounded-xl px-1',
          selected ? 'bg-white/10 shadow-[0_2px_12px_rgba(0,0,0,0.16)]' : '',
          focused,
        ].join(' ')}
      >
        <NavigatorToggle node={node} />
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-[9px] bg-white/10 text-[var(--portal-gold)] ring-1 ring-inset ring-white/15">
          <Icon className="h-[18px] w-[18px]" strokeWidth={1.5} aria-hidden />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate font-serif text-[23px] font-bold leading-tight text-white/95">{item.label}</span>
          {item.subtitle ? (
            <span className="mt-0.5 block truncate text-[15px] font-light leading-snug text-white/60">{item.subtitle}</span>
          ) : null}
        </span>
        {typeof item.progress === 'number' ? (
          <span className="flex shrink-0 items-center gap-1.5 pr-1">
            <ProjectProgress value={item.progress} className="w-11" />
            <span className="text-[14px] font-light text-white/55">{item.progress}%</span>
          </span>
        ) : null}
      </div>
    )
  }

  if (item.kind === 'project') {
    const kind = (item.meta ?? '').split(' · ')[0] ?? ''
    return (
      <div
        style={style}
        className={[
          'flex items-center gap-2 rounded-lg px-1',
          selected ? 'bg-white/10' : '',
          focused,
        ].join(' ')}
      >
        <NavigatorToggle node={node} />
        {kind ? <NavigatorGlyph icon={projectKindIcon(kind)} /> : null}
        <span className="min-w-0 flex-1 truncate text-[19px] font-light leading-tight text-white/95">{item.label}</span>
        {typeof item.progress === 'number' ? (
          <span className="shrink-0 pr-1 text-[14px] font-light text-white/55">{item.progress}%</span>
        ) : null}
      </div>
    )
  }

  const type = (item.meta ?? '').split(' · ')[0] ?? ''
  return (
    <div
      style={style}
      className={[
        'flex items-center gap-2 rounded-md px-1',
        selected ? 'bg-white/15 ring-1 ring-inset ring-white/25' : '',
        focused,
      ].join(' ')}
    >
      <NavigatorToggle node={node} />
      <NavigatorGlyph icon={workTypeIcon(type, item.label)} className={navigatorStatusClass(item.status)} />
      <span className="min-w-0 flex-1 truncate text-[17px] font-light leading-tight text-white/95">{item.label}</span>
    </div>
  )
}

function ProjectDomainRail({ payload }: { payload: NavigatorPayload }) {
  return (
    <div className="flex w-[86px] shrink-0 flex-col items-center border-r border-white/10 py-3" aria-label="Project scope and domain">
      <button
        type="button"
        onClick={() => dispatchNavigatorIntent({ kind: 'catchup' })}
        title="Catch-Up"
        aria-current={payload.catchUp ? 'page' : undefined}
        className={['group relative flex w-full flex-col items-center gap-1.5 py-2.5 transition', payload.catchUp ? '' : 'opacity-95 hover:opacity-100'].join(' ')}
      >
        <span className={['absolute inset-y-2 left-0 w-[3px] rounded-r-full transition', payload.catchUp ? 'bg-[var(--portal-gold)]' : 'bg-transparent group-hover:bg-white/30'].join(' ')} />
        <span className={['flex h-10 w-10 items-center justify-center rounded-xl transition', payload.catchUp ? 'bg-black/25 text-[var(--portal-gold)] shadow-sm ring-1 ring-inset ring-white/25' : 'bg-white/[0.07] text-white/85 group-hover:bg-white/[0.16] group-hover:text-white'].join(' ')}>
          <ListChecks className="h-[23px] w-[23px]" strokeWidth={1.6} aria-hidden />
        </span>
        <span className={['text-center text-[14px] font-medium uppercase leading-tight tracking-[0.02em]', payload.catchUp ? 'text-white' : 'text-white/70 group-hover:text-white/95'].join(' ')}>
          Catch-Up
        </span>
      </button>
      <div className="my-1 w-[60%] border-b border-white/15" aria-hidden />
      {DOMAIN_META.map((domain) => {
        const Icon = domain.icon
        const active = !payload.catchUp && payload.activeDomain === domain.key
        return (
          <button
            key={domain.key}
            type="button"
            onClick={() => dispatchNavigatorIntent({ kind: 'domain', domain: domain.key })}
            title={domain.label}
            aria-current={active ? 'true' : undefined}
            className={['group relative flex w-full flex-col items-center gap-1.5 py-2.5 transition', active ? '' : 'opacity-95 hover:opacity-100'].join(' ')}
          >
            <span className={['absolute inset-y-2 left-0 w-[3px] rounded-r-full transition', active ? 'bg-[var(--portal-gold)]' : 'bg-transparent group-hover:bg-white/30'].join(' ')} />
            <span className={['flex h-10 w-10 items-center justify-center rounded-xl transition', active ? 'bg-black/25 text-[var(--portal-gold)] shadow-sm ring-1 ring-inset ring-white/25' : 'bg-white/[0.07] text-white/85 group-hover:bg-white/[0.16] group-hover:text-white'].join(' ')}>
              <Icon className="h-[23px] w-[23px]" strokeWidth={1.6} aria-hidden />
            </span>
            <span className={['text-center text-[14px] font-medium uppercase leading-tight tracking-[0.02em]', active ? 'text-white' : 'text-white/70 group-hover:text-white/95'].join(' ')}>
              {domain.label.slice(0, 12)}
            </span>
          </button>
        )
      })}
    </div>
  )
}

function ProjectNavigatorIsland({ payload }: { payload: NavigatorPayload }) {
  const wrapRef = useRef<HTMLDivElement | null>(null)
  const [height, setHeight] = useState(560)
  const data = useMemo(() => buildNavigatorTree(payload), [payload])
  const selection = useMemo(
    () => selectedNavigatorId(data, payload.selectedProjectId, payload.selectedNodeId),
    [data, payload.selectedNodeId, payload.selectedProjectId],
  )
  const initialOpenState = useMemo(() => openAncestors(data, selection), [data, selection])
  const activeLabel = DOMAIN_META.find((domain) => domain.key === payload.activeDomain)?.label ?? 'Projects'

  useEffect(() => {
    const element = wrapRef.current
    if (!element) return
    const update = () => setHeight(Math.max(200, element.clientHeight))
    update()
    const observer = new ResizeObserver(update)
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  return (
    <section className="flex h-full min-h-0 overflow-hidden bg-[var(--portal-navy)] text-white">
      <ProjectDomainRail payload={payload} />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <div className="border-b border-white/10 px-3 pb-2 pt-3">
          <p className="text-[14px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold)]">{activeLabel}</p>
          <label className="mt-2 flex h-11 items-center gap-2 rounded-[var(--portal-tab-radius)] border border-white/15 bg-white/10 px-3">
            <Search className="h-4 w-4 shrink-0 text-white/50" aria-hidden />
            <input
              value={payload.query}
              onChange={(event) => dispatchNavigatorIntent({ kind: 'query', query: event.target.value })}
              placeholder="Find work…"
              className="min-w-0 flex-1 bg-transparent text-[16px] font-light text-white outline-none placeholder:text-white/55"
            />
          </label>
        </div>
        <div ref={wrapRef} className="min-h-0 flex-1 overflow-hidden px-1 pt-1.5">
          {data.length === 0 ? (
            <div className="px-3 py-8 text-sm font-light text-white/45">No matching projects in this perspective.</div>
          ) : (
            <Tree<NavigatorNode>
              key={payload.activeDomain}
              data={data}
              selection={selection}
              initialOpenState={initialOpenState}
              openByDefault={false}
              searchTerm={payload.query}
              searchMatch={(node, term) =>
                term.trim() ? node.data.searchText.includes(term.trim().toLowerCase()) : true
              }
              width="100%"
              height={height}
              indent={7}
              rowHeight={(node) => (node.data.kind === 'pole' ? 70 : node.data.kind === 'project' ? 50 : 46)}
              overscanCount={6}
              disableDrag
              disableDrop
              disableEdit
              disableMultiSelection
              onSelect={(nodes) => {
                const selectedNode = nodes[0]?.data
                if (!selectedNode) return
                if (selectedNode.kind === 'project' && selectedNode.projectId) {
                  dispatchNavigatorIntent({ kind: 'project', projectId: selectedNode.projectId })
                } else if (selectedNode.kind === 'work' && selectedNode.projectId && selectedNode.workNodeId) {
                  dispatchNavigatorIntent({
                    kind: 'work',
                    projectId: selectedNode.projectId,
                    nodeId: selectedNode.workNodeId,
                  })
                }
              }}
              onActivate={(node) => {
                if (!node.isLeaf) node.toggle()
              }}
            >
              {NavigatorNodeView}
            </Tree>
          )}
        </div>
      </div>
    </section>
  )
}

type TimelinePayload = {
  project: {
    id: string
    name: string
    status: string
    projectType?: string | null
  }
  items: Array<{
    id: string
    title: string
    notes: string
    category: string
    status: string
    parentId?: string | null
    dueAt?: string | null
    owner?: string | null
    order?: number | null
  }>
  progress: number
}

type CalendarPayload = {
  events: CatchUpCalendarEvent[]
}

type DocumentsPayload = {
  files: Array<{
    id: string
    assetId: string
    kind: 'document' | 'photo'
    name: string
    source: 'vault' | 'property-media'
    href?: string | null
    state?: string | null
    caption?: string | null
    altText?: string | null
    mimeType?: string | null
    size?: number | null
    date?: string | null
  }>
}

type IslandState = {
  navigatorTarget: Element | null
  navigator: NavigatorPayload | null
  timelineTarget: Element | null
  timeline: TimelinePayload | null
  calendarTarget: Element | null
  calendar: CalendarPayload | null
  documentsTarget: Element | null
  documents: DocumentsPayload | null
}

const EMPTY: IslandState = {
  navigatorTarget: null,
  navigator: null,
  timelineTarget: null,
  timeline: null,
  calendarTarget: null,
  calendar: null,
  documentsTarget: null,
  documents: null,
}

function readPayload<T>(element: Element | null): T | null {
  const raw = element?.getAttribute('data-project-widget')
  if (!raw) return null
  try {
    return JSON.parse(raw) as T
  } catch {
    return null
  }
}

function sameIslandState(left: IslandState, right: IslandState): boolean {
  return (
    left.navigatorTarget === right.navigatorTarget &&
    left.timelineTarget === right.timelineTarget &&
    left.calendarTarget === right.calendarTarget &&
    left.documentsTarget === right.documentsTarget &&
    JSON.stringify(left.navigator) === JSON.stringify(right.navigator) &&
    JSON.stringify(left.timeline) === JSON.stringify(right.timeline) &&
    JSON.stringify(left.calendar) === JSON.stringify(right.calendar) &&
    JSON.stringify(left.documents) === JSON.stringify(right.documents)
  )
}

/**
 * Rendering adapter only.
 *
 * Yew owns the page, project selection, tabs, and commands. This component
 * observes Yew's React-island slots and portals the existing mature widgets
 * into them. Yew remains the application-state owner; only navigator intents
 * cross back through the reducer bridge.
 */
export function ProjectReactIslands() {
  const [state, setState] = useState<IslandState>(EMPTY)

  useEffect(() => {
    const root = document.getElementById('rust-ui')
    if (!root) return

    const scan = () => {
      const navigatorTarget = root.querySelector('#project-navigator-island')
      const timelineTarget = root.querySelector('#project-timeline-island')
      const calendarTarget = root.querySelector('#project-calendar-island')
      const documentsTarget = root.querySelector('#project-documents-island')
      const next: IslandState = {
        navigatorTarget,
        navigator: readPayload<NavigatorPayload>(navigatorTarget),
        timelineTarget,
        timeline: readPayload<TimelinePayload>(timelineTarget),
        calendarTarget,
        calendar: readPayload<CalendarPayload>(calendarTarget),
        documentsTarget,
        documents: readPayload<DocumentsPayload>(documentsTarget),
      }
      setState((current) => (sameIslandState(current, next) ? current : next))
    }

    scan()
    const observer = new MutationObserver(scan)
    observer.observe(root, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['data-project-widget'],
    })
    return () => observer.disconnect()
  }, [])

  const timeline = useMemo(() => {
    const payload = state.timeline
    if (!payload) return { tasks: [] as ITask[], links: [] as ILink[], synthetic: false }

    const byParent = (parentId: string | null): ProjectWorkNode[] =>
      payload.items
        .filter((item) => (item.parentId ?? null) === parentId)
        .sort((left, right) => {
          const order = (left.order ?? Number.MAX_SAFE_INTEGER) - (right.order ?? Number.MAX_SAFE_INTEGER)
          if (order !== 0) return order
          return (left.dueAt ?? '9999').localeCompare(right.dueAt ?? '9999') || left.id.localeCompare(right.id)
        })
        .map((item) => {
          const status: ProjectWorkStatus =
            item.status === 'done'
              ? 'complete'
              : item.status === 'doing'
                ? 'in-progress'
                : item.status === 'dismissed'
                  ? 'dismissed'
                  : 'not-started'
          return {
            id: item.id,
            title: item.title,
            type: workType(item.category) as ProjectWorkNode['type'],
            status,
            ...(item.dueAt ? { dueAt: item.dueAt } : {}),
            ...(item.owner ? { owner: item.owner } : {}),
            ...(item.notes ? { note: item.notes } : {}),
            children: byParent(item.id),
          }
        })

    const plan: ProjectPlan = {
      id: payload.project.id,
      title: payload.project.name,
      kind: String(payload.project.projectType ?? 'WORK').toUpperCase(),
      status:
        payload.project.status === 'doing'
          ? 'active'
          : payload.project.status === 'done'
            ? 'complete'
            : payload.project.status === 'archived'
              ? 'archived'
              : 'planning',
      progress: payload.progress,
      phaseLabel: payload.project.status,
      workNodes: byParent(null),
    }
    return mapProjectToTimeline(plan)
  }, [state.timeline])


  const documentFiles = useMemo<ProjectAssetBrowserItem[]>(() => {
    return (state.documents?.files ?? []).map((file) => ({
      id: file.id,
      assetId: file.assetId,
      kind: file.kind,
      name: file.name,
      source: file.source,
      ...(file.href ? { href: file.href } : {}),
      ...(file.state ? { state: file.state } : {}),
      ...(file.caption !== undefined ? { caption: file.caption } : {}),
      ...(file.altText !== undefined ? { altText: file.altText } : {}),
      ...(file.mimeType !== undefined ? { mimeType: file.mimeType } : {}),
      ...(file.size !== undefined ? { size: file.size } : {}),
      ...(file.date ? { date: new Date(file.date) } : {}),
    }))
  }, [state.documents])

  return (
    <>
      {state.navigatorTarget && state.navigator
        ? createPortal(<ProjectNavigatorIsland payload={state.navigator} />, state.navigatorTarget)
        : null}
      {state.timelineTarget && state.timeline
        ? createPortal(
            timeline.tasks.length === 0 ? (
              <div className="flex h-full min-h-64 items-center justify-center px-8 text-center text-sm font-light text-black/45">
                No WBS work exists for this project yet.
              </div>
            ) : (
              <div className="flex h-full min-h-0 flex-col gap-2">
                <div className="min-h-0 flex-1">
                  <ProjectTimeline tasks={timeline.tasks} links={timeline.links} />
                </div>
                {timeline.synthetic ? (
                  <p className="shrink-0 text-[11px] font-light text-black/45">
                    Sample schedule — bars use placeholder dates because these work items have no due dates yet.
                  </p>
                ) : null}
              </div>
            ),
            state.timelineTarget,
          )
        : null}
      {state.calendarTarget && state.calendar
        ? createPortal(
            <FullCalendarCandidate events={state.calendar.events ?? []} heading={null} />,
            state.calendarTarget,
          )
        : null}
      {state.documentsTarget && state.documents
        ? createPortal(<ProjectFilemanager files={documentFiles} />, state.documentsTarget)
        : null}
    </>
  )
}
