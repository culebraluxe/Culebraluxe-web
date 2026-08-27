'use client'

import { useMemo } from 'react'
import {
  Tree,
  type NodeRendererProps,
  type RowRendererProps,
} from 'react-arborist'
import { ChevronRight } from 'lucide-react'

import { Panel } from '@/components/portal/panel'
import {
  buildCatchUpTaskTree,
  INITIALLY_OPEN_WORKSTREAMS,
} from '@/lib/catchup/task-tree'
import type { CatchUpTask } from '@/db/tasks'

// ---------------------------------------------------------------------------
// CATCH-UP — Task tree (left pane of the three-pane Catch-Up layout).
//
// The left pane is a calm NAVIGATOR / INDEX — the smallest and lightest of the
// three siblings. React Arborist owns the mechanics (expand/collapse,
// selection, keyboard navigation, virtualization); the node renderers below
// own the CulebraLuxe appearance.
//
//   LEVEL 1  workstream  — 14px semibold uppercase navy, small chevron
//   LEVEL 2  category    — 13px medium uppercase muted
//   LEVEL 3  task leaf   — 14px regular, task title only
//
// Category is a real branch (not metadata concatenated into the leaf). The
// task leaf renders ONLY its title. No counts / badges / owner / status / due /
// person / property / deal / description on the tree — that metadata lives in
// the sibling Task Detail pane.
//
// Selection is LIFTED to the parent so the middle pane shows the selected task:
// clicking a task leaf reports its id via onSelectTask; workstream/category
// clicks only expand/collapse. Default open: CLIENT, CORE, SUPPORT.
// ---------------------------------------------------------------------------

type WorkstreamNode = {
  kind: 'workstream'
  id: string
  name: string
  children: CategoryNode[]
}

type CategoryNode = {
  kind: 'category'
  id: string
  name: string
  children: TaskNode[]
}

type TaskNode = {
  kind: 'task'
  id: string
  title: string
}

type NodeData = WorkstreamNode | CategoryNode | TaskNode

const WORKSTREAM_HEIGHT = 30
const CATEGORY_HEIGHT = 28
const TASK_HEIGHT = 30

function Chevron({ node }: { node: NodeRendererProps<NodeData>['node'] }) {
  if (node.isLeaf) {
    return <span className="w-3 shrink-0" aria-hidden />
  }
  return (
    <button
      type="button"
      onClick={(event) => {
        event.stopPropagation()
        node.toggle()
      }}
      aria-label={node.isOpen ? 'Collapse' : 'Expand'}
      aria-expanded={node.isOpen}
      className="flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded text-[var(--portal-navy-soft)] transition hover:text-[var(--portal-navy)]"
    >
      <ChevronRight
        className={`h-3 w-3 transition-transform duration-150 ${
          node.isOpen ? 'rotate-90' : ''
        }`}
        aria-hidden
      />
    </button>
  )
}

function CatchUpRow({
  node,
  innerRef,
  attrs,
  children,
}: RowRendererProps<NodeData>) {
  return (
    <div
      {...attrs}
      ref={innerRef}
      onFocus={(event) => event.stopPropagation()}
      onClick={() => {
        // Branch clicks expand/collapse; only task leaves select.
        if (node.isInternal) node.toggle()
        else node.select()
      }}
    >
      {children}
    </div>
  )
}

function CatchUpTreeNode({ node, style }: NodeRendererProps<NodeData>) {
  const data = node.data

  if (data.kind === 'workstream') {
    return (
      <div
        style={style}
        className="flex h-full items-center gap-1.5 border-t border-black/[0.04] px-1 text-[var(--portal-navy)]"
      >
        <Chevron node={node} />
        <span className="truncate text-[14px] font-semibold uppercase tracking-[0.08em]">
          {data.name}
        </span>
      </div>
    )
  }

  if (data.kind === 'category') {
    return (
      <div
        style={style}
        className="flex h-full items-center gap-1.5 px-1 text-[var(--portal-navy-soft)]"
      >
        <Chevron node={node} />
        <span className="truncate text-[13px] font-medium uppercase tracking-[0.06em] text-[var(--portal-panel-heading-muted)]">
          {data.name}
        </span>
      </div>
    )
  }

  const selected = node.isSelected
  return (
    <div
      style={style}
      className={[
        'flex h-full items-center gap-1.5 px-1 transition-colors',
        selected
          ? 'rounded-[6px] bg-[var(--portal-gold-pale)] text-[var(--portal-navy)]'
          : 'text-[var(--portal-navy)] hover:bg-black/[0.03]',
      ].join(' ')}
    >
      <span className="w-3 shrink-0" aria-hidden />
      <span className="min-w-0 flex-1 truncate text-[14px] leading-none">
        {data.title}
      </span>
    </div>
  )
}


export function CatchUpTaskTree({
  tasks,
  selectedTaskId,
  onSelectTask,
}: {
  tasks: CatchUpTask[]
  selectedTaskId: string | null
  onSelectTask: (taskId: string) => void
}) {
  const parents: NodeData[] = useMemo(() => {
    return buildCatchUpTaskTree(
      tasks.map((task) => ({
        id: task.id,
        title: task.title,
        workstream: task.workstream ?? '',
        category: task.category ?? null,
      })),
    ).map((ws) => ({
      kind: 'workstream' as const,
      id: ws.id,
      name: ws.name,
      children: ws.children.map((cat) => ({
        kind: 'category' as const,
        id: cat.id,
        name: cat.name,
        children: cat.children.map((leaf) => ({
          kind: 'task' as const,
          id: leaf.id,
          title: leaf.title,
        })),
      })),
    }))
  }, [tasks])

  const initialOpenState = useMemo(() => {
    const open: Record<string, boolean> = {}
    for (const ws of parents) {
      if (ws.kind !== 'workstream') continue
      if (!INITIALLY_OPEN_WORKSTREAMS.includes(ws.id)) continue
      open[ws.id] = true
      for (const cat of ws.children) open[cat.id] = true
    }
    return open
  }, [parents])

  return (
    <Panel compact heading="Catch-Up" className="flex h-full min-h-0 flex-col">
      {parents.length === 0 ? (
        <p className="flex flex-1 items-center justify-center px-4 text-center text-sm font-light text-black/40">
          No active workstream tasks right now.
        </p>
      ) : (
        <div className="min-h-0 flex-1">
          <Tree
            data={parents}
            rowHeight={(node) =>
              node.level === 0
                ? WORKSTREAM_HEIGHT
                : node.level === 1
                  ? CATEGORY_HEIGHT
                  : TASK_HEIGHT
            }
            indent={14}
            initialOpenState={initialOpenState}
            disableDrag
            disableDrop
            disableEdit
            disableMultiSelection
            selection={selectedTaskId ?? undefined}
            onSelect={(nodes) => {
              const node = nodes[0]
              if (node?.data.kind === 'task') onSelectTask(node.data.id)
            }}
            className="h-full min-h-[420px] w-full"
            aria-label="Catch-Up task tree"
          >
            {CatchUpTreeNode}
          </Tree>
        </div>
      )}
    </Panel>
  )
}

