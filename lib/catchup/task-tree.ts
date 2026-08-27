// ---------------------------------------------------------------------------
// CATCH-UP — task tree projection (pure).
//
// Builds the TRUE THREE-LEVEL Catch-Up workstream tree:
//   LEVEL 1  workstream branch  (CLIENT | CORE | OPPS | SUPPORT | TECH)
//   LEVEL 2  category branch
//   LEVEL 3  task leaf          (renders ONLY the task title)
//
// Category is a real branch, NOT metadata concatenated into the leaf. A task
// with a blank category still needs a branch to hang under, so it falls back
// to FALLBACK_CATEGORY to preserve the three-level invariant.
//
// Pure and dependency-free so the projection is unit-testable without a DOM.
// The React Arborist wrapper (components/portal/catch-up-task-tree.tsx) owns
// tree mechanics; this module owns the data shape + ordering.
// ---------------------------------------------------------------------------

export const CATCHUP_WORKSTREAMS = [
  'CLIENT',
  'CORE',
  'OPPS',
  'SUPPORT',
  'TECH',
] as const

export type CatchUpWorkstream = (typeof CATCHUP_WORKSTREAMS)[number]

/** Workstreams that open by default on first render (vertical calm elsewhere). */
export const INITIALLY_OPEN_WORKSTREAMS: string[] = [
  'CLIENT',
  'CORE',
  'SUPPORT',
]

/** Category branch used when a task has no category (keeps the tree 3-level). */
export const FALLBACK_CATEGORY = 'General'

export type CatchUpTaskLeaf = {
  id: string
  title: string
  workstream: string
  category: string | null
}

export type CatchUpCategoryNode = {
  id: string
  name: string
  children: CatchUpTaskLeaf[]
}

export type CatchUpWorkstreamNode = {
  id: string
  name: string
  children: CatchUpCategoryNode[]
}

const WS_ORDER: Map<string, number> = new Map(
  CATCHUP_WORKSTREAMS.map((ws, i) => [ws, i]),
)

/** Normalize a raw task.workstream value into a canonical parent key. */
export function normalizeWorkstream(
  value: string | null | undefined,
): string {
  return (value ?? '').trim().toUpperCase()
}

/** Normalize a task category into a branch name (blank → FALLBACK_CATEGORY). */
function categoryBranch(category: string | null): string {
  const value = (category ?? '').trim().toUpperCase()
  return value || FALLBACK_CATEGORY
}

/**
 * Project flat canonical task rows into the three-level Catch-Up tree.
 *
 * - tasks with a blank/unknown workstream are excluded (nothing to group under)
 * - categories sort alphabetically; leaves sort by title within a category
 * - parents are ordered by the canonical CLIENT/CORE/OPPS/SUPPORT/TECH ladder;
 *   any unrecognized workstream sorts after, alphabetically
 */
export function buildCatchUpTaskTree(
  tasks: readonly CatchUpTaskLeaf[],
): CatchUpWorkstreamNode[] {
  const byWorkstream = new Map<string, Map<string, CatchUpTaskLeaf[]>>()

  for (const task of tasks) {
    const ws = normalizeWorkstream(task.workstream)
    if (!ws) continue
    const cat = categoryBranch(task.category)
    let byCategory = byWorkstream.get(ws)
    if (!byCategory) {
      byCategory = new Map()
      byWorkstream.set(ws, byCategory)
    }
    const list = byCategory.get(cat)
    if (list) list.push(task)
    else byCategory.set(cat, [task])
  }

  const parents: CatchUpWorkstreamNode[] = []
  for (const [ws, byCategory] of byWorkstream) {
    const categories: CatchUpCategoryNode[] = [...byCategory.entries()]
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([name, leaves]) => ({
        id: `${ws}::${name}`,
        name,
        children: [...leaves].sort((a, b) => a.title.localeCompare(b.title)),
      }))
    parents.push({ id: ws, name: ws, children: categories })
  }

  parents.sort((a, b) => {
    const ai = WS_ORDER.get(a.id)
    const bi = WS_ORDER.get(b.id)
    if (ai !== undefined && bi !== undefined) return ai - bi
    if (ai !== undefined) return -1
    if (bi !== undefined) return 1
    return a.id.localeCompare(b.id)
  })

  return parents
}

/**
 * The first task id under the first available workstream (canonical order).
 * Used as the default Task Workspace selection for the flat work-queue
 * navigator; returns null when there are no tasks.
 */
export function firstCatchUpTaskId(
  tasks: readonly CatchUpTaskLeaf[],
): string | null {
  const ws = buildCatchUpTaskTree(tasks)[0]
  return ws?.children[0]?.children[0]?.id ?? null
}

/**
 * A flat, pre-grouped navigator row. Category is a quiet SECTION HEADER; task
 * rows are the selectable items. No indentation — the navigator is flat.
 */
export type CatchUpNavRow =
  | { kind: 'category'; id: string; name: string }
  | { kind: 'task'; id: string; title: string }

/** The workstreams that actually have tasks, in canonical order. */
export function getCatchUpWorkstreams(
  tasks: readonly CatchUpTaskLeaf[],
): string[] {
  return buildCatchUpTaskTree(tasks).map((ws) => ws.id)
}

/**
 * Flat rows for a single workstream: a category section header followed by its
 * task rows, in the canonical grouping order. Category headers carry no tasks
 * and are NOT part of the task-selection sequence.
 */
export function buildCatchUpNavRows(
  tasks: readonly CatchUpTaskLeaf[],
  workstream: string,
): CatchUpNavRow[] {
  const ws = buildCatchUpTaskTree(tasks).find((w) => w.id === workstream)
  if (!ws) return []
  const rows: CatchUpNavRow[] = []
  for (const cat of ws.children) {
    rows.push({ kind: 'category', id: cat.id, name: cat.name })
    for (const leaf of cat.children) {
      rows.push({ kind: 'task', id: leaf.id, title: leaf.title })
    }
  }
  return rows
}
