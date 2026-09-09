// ---------------------------------------------------------------------------
// Projection from the REAL WBS repositories (wbs_project + wbs_item) into the
// MVI ProjectsWorkspaceData (pole -> project -> WBS) for the test wiring.
//
// The real model is flat (a WbsProject with WbsItem children, each categorized)
// and has no "Pole". Best-effort (refine later): synthesize one collection Pole
// per area/category mapped to an MVI domain and nest the real projects + items.
// ---------------------------------------------------------------------------
import type { WbsItem, WbsProject } from "@/services/wbs"
import type { ProjectWorkStatus } from "./model"

import type {
  ProjectDomain,
  ProjectDomainKey,
  ProjectPlan,
  ProjectPole,
  ProjectWorkNode,
  ProjectWorkNodeType,
  ProjectsWorkspaceData,
} from "./model"

const DOMAINS: ProjectDomain[] = [
  { key: "properties", label: "Properties", shortLabel: "Properties" },
  { key: "people", label: "People", shortLabel: "People" },
  { key: "deals", label: "Deals", shortLabel: "Deals" },
  { key: "firm", label: "Firm", shortLabel: "Firm" },
  { key: "marketing", label: "Marketing", shortLabel: "Marketing" },
  { key: "accounting", label: "Accounting", shortLabel: "Accounting" },
]

/** Real WbsItem category -> MVI domain (best effort). */
function categoryToDomain(category: string): ProjectDomainKey {
  if (category === "properties" || category === "media") return "properties"
  if (category === "clients") return "people"
  if (category === "contracts") return "deals"
  if (category === "accounting") return "accounting"
  if (category === "marketing") return "marketing"
  return "firm"
}

function wbsStatus(status: string): ProjectWorkStatus {
  if (status === "done") return "complete"
  if (status === "doing") return "in-progress"
  if (status === "dismissed") return "complete"
  return "not-started"
}

function categoryToType(category: string): ProjectWorkNodeType {
  if (category === "contracts") return "contract"
  if (category === "media") return "media"
  if (category === "marketing") return "workflow"
  if (category === "accounting") return "accounting"
  if (category === "clients" || category === "properties") return "group"
  return "task"
}

function dueLabel(dueAt: string | null | undefined): string | undefined {
  if (!dueAt) return undefined
  const d = new Date(dueAt)
  if (Number.isNaN(d.getTime())) return undefined
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" })
}

function attach(items: WbsItem[], item: WbsItem): ProjectWorkNode {
  return {
    id: item.id,
    title: item.title,
    type: categoryToType(item.category),
    status: wbsStatus(item.status),
    ...(item.owner ? { owner: item.owner } : {}),
    ...(dueLabel(item.dueAt) ? { dueLabel: dueLabel(item.dueAt) } : {}),
    ...(item.notes ? { note: item.notes } : {}),
    children: items.filter((candidate) => candidate.parentId === item.id).map((child) => attach(items, child)),
  }
}

/** Pick the project's dominant area from its item categories (best effort). */
function dominantCategory(items: WbsItem[], fallback: string): string {
  const counts = new Map<string, number>()
  for (const item of items) counts.set(item.category, (counts.get(item.category) ?? 0) + 1)
  let best = fallback
  let bestCount = -1
  for (const [category, count] of counts) {
    if (count > bestCount) {
      best = category
      bestCount = count
    }
  }
  return best
}

export function mapRealProjectsToWorkspace(projects: WbsProject[], items: WbsItem[]): ProjectsWorkspaceData {
  const itemsByProject = new Map<string, WbsItem[]>()
  for (const item of items) {
    if (!item.projectId) continue
    const list = itemsByProject.get(item.projectId) ?? []
    list.push(item)
    itemsByProject.set(item.projectId, list)
  }

  const byDomain = new Map<ProjectDomainKey, ProjectPlan[]>()
  for (const project of projects) {
    const projectItems = itemsByProject.get(project.id) ?? []
    const domain = categoryToDomain(dominantCategory(projectItems, "management"))
    const top = projectItems.filter((i) => !i.parentId)
    const done = projectItems.filter((i) => i.status === "done").length
    const plan: ProjectPlan = {
      id: project.id,
      title: project.name,
      kind: dominantCategory(projectItems, "WORK").toUpperCase(),
      status: "active",
      progress: projectItems.length ? Math.round((done / projectItems.length) * 100) : 0,
      phaseLabel: project.status,
      workNodes: top.map((node) => attach(projectItems, node)),
    }
    byDomain.set(domain, [...(byDomain.get(domain) ?? []), plan])
  }

  const poles: ProjectPole[] = []
  for (const domain of DOMAINS) {
    const plans = byDomain.get(domain.key)
    if (!plans || plans.length === 0) continue
    const total = plans.reduce((sum, p) => sum + p.progress, 0)
    poles.push({
      id: `collection-${domain.key}`,
      domain: domain.key,
      label: domain.label,
      subtitle: `${plans.length} ${plans.length === 1 ? "project" : "projects"}`,
      progress: Math.round(total / plans.length),
      statusLabel: "Collection",
      projects: plans,
    })
  }

  return { domains: DOMAINS, poles }
}
