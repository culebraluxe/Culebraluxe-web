// ---------------------------------------------------------------------------
// Projection from the canonical Projects model into react-arborist tree nodes.
//
// The MVI PageModel stays authoritative. This module produces the CONTROLLED
// `data` arborist renders and helpers to reconcile MVI selection -> arborist
// selection and compute which nodes should be open so a selection is visible.
//
// Arborist node ids are COMPOSITE (unique across the tree): projects and WBS
// nodes carry the real ids on the data so selection can dispatch back to the
// controller without ambiguity.
// ---------------------------------------------------------------------------
import type { ProjectDomainKey, ProjectPole, ProjectWorkNode, ProjectWorkStatus } from "./model"

export type ProjectTreeNodeKind = "pole" | "project" | "work-node"

export type ProjectTreeNode = {
  /** Composite arborist id, unique across the tree. */
  id: string
  kind: ProjectTreeNodeKind
  label: string
  /** Real model id for dispatch. */
  poleId: string
  projectId?: string
  workNodeId?: string
  /** Domain (for pole icon). */
  domain?: ProjectDomainKey
  /** Pole richness. */
  subtitle?: string
  progress?: number
  /** WorkNode status. */
  status?: ProjectWorkStatus
  /** Secondary row text (work node status / due / owner). */
  meta?: string
  /** Precomputed search blob (node + descendants) so a pole/project stays
   *  visible when a deeper descendant matches. */
  searchText: string
  children?: ProjectTreeNode[]
}

const poleNode = (pole: ProjectPole): ProjectTreeNode => ({
  id: pole.id,
  kind: "pole",
  label: pole.label,
  poleId: pole.id,
  domain: pole.domain,
  subtitle: pole.subtitle,
  progress: pole.progress,
  searchText: `${pole.label} ${pole.subtitle}`.toLowerCase(),
  children: pole.projects.map((project) => {
    const projectId = `${pole.id}::${project.id}`
    return {
      id: projectId,
      kind: "project",
      label: project.title,
      poleId: pole.id,
      projectId: project.id,
      progress: project.progress,
      meta: `${project.kind} · ${project.phaseLabel}`,
      searchText: `${project.title} ${project.kind} ${project.phaseLabel} ${project.workNodes.map(wbsSearch).join(" ")}`.toLowerCase(),
      children: project.workNodes.map((node) =>
        workNode({ poleId: pole.id, projectId: project.id, parentId: projectId }, node),
      ),
    }
  }),
})

function wbsSearch(node: ProjectWorkNode): string {
  const own = `${node.title} ${node.type} ${node.inspector?.relatedItems?.map((r) => r.label).join(" ") ?? ""}`
  const child = (node.children ?? []).map(wbsSearch).join(" ")
  return `${own} ${child}`
}

type WorkNodeCtx = { poleId: string; projectId: string; parentId: string }

function workNode(ctx: WorkNodeCtx, node: ProjectWorkNode): ProjectTreeNode {
  const id = `${ctx.parentId}::${node.id}`
  const metaParts: string[] = [node.type]
  if (node.status) metaParts.push(node.status.replace(/-/g, " "))
  if (node.dueLabel) metaParts.push(node.dueLabel)
  return {
    id,
    kind: "work-node",
    label: node.title,
    poleId: ctx.poleId,
    projectId: ctx.projectId,
    workNodeId: node.id,
    status: node.status,
    meta: metaParts.join(" · "),
    searchText: wbsSearch(node),
    children: node.children?.length ? node.children.map((child) => workNode(ctx, child)) : undefined,
  }
}

export function buildProjectTree(poles: ProjectPole[]): ProjectTreeNode[] {
  return poles.map(poleNode)
}

/** Reconcile MVI selection -> composite arborist node id (or null). */
export function selectedTreeNodeId(
  nodes: ProjectTreeNode[],
  selection: { poleId: string | null; projectId: string | null; nodeId: string | null },
): string | null {
  const pole = nodes.find((n) => n.poleId === selection.poleId && n.kind === "pole")
  if (!pole) return null
  if (!selection.projectId) return pole.id
  const project = pole.children?.find((n) => n.projectId === selection.projectId && n.kind === "project")
  if (!project) return pole.id
  if (!selection.nodeId) return project.id
  const node = findWork(project, selection.nodeId)
  return node?.id ?? project.id
}

function findWork(project: ProjectTreeNode, realNodeId: string): ProjectTreeNode | null {
  for (const child of project.children ?? []) {
    if (child.workNodeId === realNodeId) return child
    const nested = child.children?.find((c) => c.workNodeId === realNodeId)
    if (nested) return nested
  }
  return null
}

/** Open-ancestors map so the selected node is visible without exploding the
 *  whole domain. Computed fresh per domain (arborist key-remount applies it). */
export function openAncestorsForSelected(
  nodes: ProjectTreeNode[],
  selectedCompositeId: string | null,
): Record<string, boolean> {
  const open: Record<string, boolean> = {}
  if (!selectedCompositeId) return open
  const path = findPath(nodes, selectedCompositeId)
  for (const node of path.slice(0, -1)) open[node.id] = true
  return open
}

function findPath(nodes: ProjectTreeNode[], id: string): ProjectTreeNode[] {
  for (const node of nodes) {
    if (node.id === id) return [node]
    const rest = node.children ? findPath(node.children, id) : []
    if (rest.length > 0) return [node, ...rest]
  }
  return []
}
