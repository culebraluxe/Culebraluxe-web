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
  Megaphone,
  PenLine,
  Users,
} from 'lucide-react'
import { Tree } from 'react-arborist'
import type { NodeApi, NodeRendererProps } from 'react-arborist'

import type { ILink, ITask } from '@svar-ui/react-gantt'
import { FullCalendarCandidate } from '@/components/portal/fullcalendar-candidate'
import { ProjectTimeline } from '@/components/portal/project-timeline'
import type { CatchUpCalendarEvent } from '@/lib/catchup/calendar-adapter'


type NavigatorProject = {
  id: string
  name: string
  status: string
  projectType?: string | null
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
}

type NavigatorPayload = {
  projects: NavigatorProject[]
  items: NavigatorItem[]
  selectedProjectId?: string | null
  selectedNodeId?: string | null
  query?: string
}

type NavigatorNode = {
  id: string
  kind: 'project' | 'work'
  label: string
  projectId: string
  workNodeId?: string
  status?: string
  meta?: string
  searchText: string
  progress?: number
  children?: NavigatorNode[]
}

const PROJECT_KIND_ICON: Record<string, LucideIcon> = {
  listing: KeyRound,
  marketing: Megaphone,
  closing: Handshake,
  deal: Handshake,
  client: Users,
  buyer_rep: Users,
  firm: Building2,
  accounting: Banknote,
}

const WORK_TYPE_ICON: Record<string, LucideIcon> = {
  clients: Users,
  contracts: FileText,
  properties: Home,
  media: Image,
  marketing: Megaphone,
  accounting: Banknote,
  management: GitBranch,
  approval: PenLine,
  workflow: GitBranch,
  task: CheckCircle2,
  milestone: Flag,
  document: FileText,
}

function navigatorStatusClass(status?: string): string {
  if (status === 'done') return 'text-[var(--portal-success)]'
  if (status === 'doing') return 'text-[var(--portal-gold)]'
  if (status === 'dismissed') return 'text-white/35'
  return 'text-white/55'
}

function dispatchNavigatorIntent(intent: Record<string, string>) {
  const bridge = document.getElementById('project-island-bridge')
  if (!(bridge instanceof HTMLButtonElement)) return
  bridge.setAttribute('data-intent', JSON.stringify(intent))
  bridge.click()
}

function navigatorTree(payload: NavigatorPayload): NavigatorNode[] {
  const itemsByProject = new Map<string, NavigatorItem[]>()
  for (const item of payload.items) {
    if (!item.projectId) continue
    const bucket = itemsByProject.get(item.projectId) ?? []
    bucket.push(item)
    itemsByProject.set(item.projectId, bucket)
  }

  const childrenFor = (
    projectId: string,
    parentId: string | null,
    projectItems: NavigatorItem[],
    seen: Set<string>,
  ): NavigatorNode[] => {
    return projectItems
      .filter((item) => (item.parentId ?? null) === parentId && !seen.has(item.id))
      .sort((left, right) => (left.order ?? Number.MAX_SAFE_INTEGER) - (right.order ?? Number.MAX_SAFE_INTEGER))
      .map((item) => {
        const branchSeen = new Set(seen)
        branchSeen.add(item.id)
        const meta = [item.category, item.owner ?? '', item.dueAt?.slice(0, 10) ?? '']
          .filter(Boolean)
          .join(' · ')
        return {
          id: 'work:' + item.id,
          kind: 'work' as const,
          label: item.title,
          projectId,
          workNodeId: item.id,
          status: item.status,
          meta,
          searchText: (item.title + ' ' + meta).toLowerCase(),
          children: childrenFor(projectId, item.id, projectItems, branchSeen),
        }
      })
  }

  return payload.projects.map((project) => {
    const projectItems = itemsByProject.get(project.id) ?? []
    const planned = projectItems.filter((item) => item.status !== 'dismissed').length
    const done = projectItems.filter((item) => item.status === 'done').length
    const progress = planned ? Math.round((done / planned) * 100) : 0
    return {
      id: 'project:' + project.id,
      kind: 'project' as const,
      label: project.name,
      projectId: project.id,
      meta: project.projectType ?? 'project',
      searchText: (project.name + ' ' + (project.projectType ?? '')).toLowerCase(),
      progress,
      children: childrenFor(project.id, null, projectItems, new Set<string>()),
    }
  })
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

function NavigatorNodeView({ node, style }: NodeRendererProps<NavigatorNode>) {
  const item = node.data
  const selected = node.isSelected
  const focused = node.isFocused ? 'ring-1 ring-inset ring-white/25' : ''

  if (item.kind === 'project') {
    const Icon = PROJECT_KIND_ICON[(item.meta ?? '').toLowerCase()] ?? FileText
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
        <Icon className="h-[18px] w-[18px] shrink-0 text-[var(--portal-gold)]" strokeWidth={1.6} aria-hidden />
        <span className="min-w-0 flex-1 truncate text-[19px] font-light leading-tight text-white/95">{item.label}</span>
        <span className="shrink-0 pr-1 text-[14px] font-light text-white/55">{item.progress ?? 0}%</span>
      </div>
    )
  }

  const Icon = WORK_TYPE_ICON[(item.meta ?? '').split(' · ')[0] ?? ''] ?? FileText
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
      <Icon className={['h-[18px] w-[18px] shrink-0', navigatorStatusClass(item.status)].join(' ')} strokeWidth={1.6} aria-hidden />
      <span className="min-w-0 flex-1 truncate text-[17px] font-light leading-tight text-white/95">{item.label}</span>
    </div>
  )
}

function ProjectNavigatorIsland({ payload }: { payload: NavigatorPayload }) {
  const wrapRef = useRef<HTMLDivElement | null>(null)
  const [height, setHeight] = useState(520)
  const data = useMemo(() => navigatorTree(payload), [payload])
  const selection = payload.selectedNodeId
    ? 'work:' + payload.selectedNodeId
    : payload.selectedProjectId
      ? 'project:' + payload.selectedProjectId
      : undefined

  const initialOpenState = useMemo(() => {
    const open: Record<string, boolean> = {}
    if (payload.selectedProjectId) open['project:' + payload.selectedProjectId] = true
    let current = payload.selectedNodeId
    while (current) {
      const item = payload.items.find((candidate) => candidate.id === current)
      if (!item) break
      open['work:' + item.id] = true
      current = item.parentId ?? null
    }
    return open
  }, [payload.items, payload.selectedNodeId, payload.selectedProjectId])

  useEffect(() => {
    const element = wrapRef.current
    if (!element) return
    const update = () => setHeight(Math.max(220, element.clientHeight))
    update()
    const observer = new ResizeObserver(update)
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  return (
    <div ref={wrapRef} className="h-full min-h-0 overflow-hidden px-1 pt-1.5">
      <Tree<NavigatorNode>
        key={payload.selectedProjectId ?? 'projects'}
        data={data}
        selection={selection}
        initialOpenState={initialOpenState}
        openByDefault={false}
        searchTerm={payload.query ?? ''}
        searchMatch={(node, term) => node.data.searchText.includes(term.trim().toLowerCase())}
        width="100%"
        height={height}
        indent={7}
        rowHeight={(node) => (node.data.kind === 'project' ? 50 : 46)}
        overscanCount={6}
        disableDrag
        disableDrop
        disableEdit
        disableMultiSelection
        onSelect={(nodes) => {
          const selectedNode = nodes[0]?.data
          if (!selectedNode) return
          if (selectedNode.kind === 'project') {
            dispatchNavigatorIntent({ kind: 'project', projectId: selectedNode.projectId })
          } else if (selectedNode.workNodeId) {
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
    </div>
  )
}

type TimelinePayload = {
  tasks: Array<{
    id: number
    text: string
    type?: string
    parent?: number
    open?: boolean
    progress?: number
    start?: string
    duration?: number
    details?: string
  }>
  links: ILink[]
}

type CalendarPayload = {
  events: CatchUpCalendarEvent[]
}

type IslandState = {
  navigatorTarget: Element | null
  navigator: NavigatorPayload | null
  timelineTarget: Element | null
  timeline: TimelinePayload | null
  calendarTarget: Element | null
  calendar: CalendarPayload | null
}

const EMPTY: IslandState = {
  navigatorTarget: null,
  navigator: null,
  timelineTarget: null,
  timeline: null,
  calendarTarget: null,
  calendar: null,
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
    JSON.stringify(left.navigator) === JSON.stringify(right.navigator) &&
    JSON.stringify(left.timeline) === JSON.stringify(right.timeline) &&
    JSON.stringify(left.calendar) === JSON.stringify(right.calendar)
  )
}

/**
 * Rendering adapter only.
 *
 * Yew owns the page, project selection, tabs, and commands. This component
 * observes Yew's two vendor slots and portals the existing React-only widgets
 * into them. No application state crosses back from these read-only widgets.
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
      const next: IslandState = {
        navigatorTarget,
        navigator: readPayload<NavigatorPayload>(navigatorTarget),
        timelineTarget,
        timeline: readPayload<TimelinePayload>(timelineTarget),
        calendarTarget,
        calendar: readPayload<CalendarPayload>(calendarTarget),
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

  const timelineTasks = useMemo<ITask[]>(() => {
    return (state.timeline?.tasks ?? []).map((task) => ({
      ...task,
      ...(task.start
        ? {
            // Due dates are calendar dates. Local noon avoids a UTC-midnight
            // shift to the prior day in Puerto Rico.
            start: new Date(`${task.start}T12:00:00`),
          }
        : {}),
    })) as ITask[]
  }, [state.timeline])

  return (
    <>
      {state.navigatorTarget && state.navigator
        ? createPortal(<ProjectNavigatorIsland payload={state.navigator} />, state.navigatorTarget)
        : null}
      {state.timelineTarget && state.timeline
        ? createPortal(
            <ProjectTimeline tasks={timelineTasks} links={state.timeline.links ?? []} />,
            state.timelineTarget,
          )
        : null}
      {state.calendarTarget && state.calendar
        ? createPortal(
            <FullCalendarCandidate events={state.calendar.events ?? []} heading={null} />,
            state.calendarTarget,
          )
        : null}
    </>
  )
}
