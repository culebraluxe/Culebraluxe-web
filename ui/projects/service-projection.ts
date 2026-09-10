// ---------------------------------------------------------------------------
// Projection from canonical Project + WBS service results into the MVI
// ProjectsWorkspaceData, built as DOMAIN PERSPECTIVES.
//
// Intent: the left domain tab is a perspective. A People tab shows each CLIENT
// as a pole with that client's projects; a Properties tab shows each PROPERTY
// as a pole with its projects. A project whose work items are anchored to both
// a person and a property appears under BOTH poles (same project, two views).
//
// The real model anchors via WbsItem.entity {type: person|property|contract|deal,
// id}. Name resolution is a SEAM: identityNames maps `${type}:${id}` -> display
// name. The server-side workspace loader supplies resolved canonical names.
// Projects with no person/property anchor fall back to a category-collection
// pole so nothing is lost.
// ---------------------------------------------------------------------------
import type { WbsItem } from "@/services/wbs"
import type { Project } from "@/services/project"
import type { ActivityFeedEntry } from "@/db/activity-feed"
import type { ProjectWorkStatus } from "./model"
import { mapProjectCalendarItems } from "./secondary-projection"

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

const ENTITY_TO_DOMAIN: Record<string, ProjectDomainKey> = {
  person: "people",
  property: "properties",
  contract: "deals",
  deal: "deals",
}

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
  if (status === "dismissed") return "dismissed"
  return "not-started"
}

function projectStatus(status: string): ProjectPlan["status"] {
  if (status === "doing") return "active"
  if (status === "done") return "complete"
  if (status === "archived") return "archived"
  return "planning"
}

function statusLabel(status: string): string {
  if (status === "doing") return "In progress"
  if (status === "done") return "Complete"
  if (status === "archived") return "Archived"
  return "Open"
}

function compareItems(a: WbsItem, b: WbsItem): number {
  const aOrder = a.order ?? Number.MAX_SAFE_INTEGER
  const bOrder = b.order ?? Number.MAX_SAFE_INTEGER
  if (aOrder !== bOrder) return aOrder - bOrder
  const aDue = a.dueAt ?? "9999"
  const bDue = b.dueAt ?? "9999"
  const due = aDue.localeCompare(bDue)
  return due || a.id.localeCompare(b.id)
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
    ...(item.dueAt ? { dueAt: item.dueAt } : {}),
    ...(dueLabel(item.dueAt) ? { dueLabel: dueLabel(item.dueAt) } : {}),
    ...(item.notes ? { note: item.notes } : {}),
    children: items
      .filter((candidate) => candidate.parentId === item.id)
      .sort(compareItems)
      .map((child) => attach(items, child)),
  }
}

function firstAction(nodes: ProjectWorkNode[]): ProjectWorkNode | null {
  for (const node of nodes) {
    if (node.status !== "complete" && node.status !== "dismissed") return node
    const child = node.children ? firstAction(node.children) : null
    if (child) return child
  }
  return null
}

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

export function mapRealProjectsToWorkspace(
  projects: Project[],
  items: WbsItem[],
  identityNames: Record<string, string> = {},
  documents: Array<{ id: string; title: string | null; state: string; propertyId: string | null; createdAt: string }> = [],
  activity: ActivityFeedEntry[] = [],
): ProjectsWorkspaceData {
  const itemsByProject = new Map<string, WbsItem[]>()
  for (const item of items) {
    if (!item.projectId) continue
    const list = itemsByProject.get(item.projectId) ?? []
    list.push(item)
    itemsByProject.set(item.projectId, list)
  }

  const anchored = new Map<string, { type: string; id: string; domain: ProjectDomainKey; plans: ProjectPlan[] }>()
  const fallbackByDomain = new Map<ProjectDomainKey, ProjectPlan[]>()

  for (const project of projects) {
    const projectItems = itemsByProject.get(project.id) ?? []
    const top = projectItems.filter((i) => !i.parentId).sort(compareItems)
    const done = projectItems.filter((i) => i.status === "done").length
    const planned = projectItems.filter((i) => i.status !== "dismissed").length
    const plan: ProjectPlan = {
      id: project.id,
      title: project.name,
      kind: String(project.projectType ?? project.areas?.[0] ?? "WORK").toUpperCase(),
      status: projectStatus(project.status),
      progress: planned ? Math.round((done / planned) * 100) : 0,
      phaseLabel: statusLabel(project.status),
      ...(project.playbookId ? { playbookId: project.playbookId } : {}),
      ...(project.playbookVersion != null ? { playbookVersion: project.playbookVersion } : {}),
      ...((project.personId || project.propertyId || project.contractId) ? {
        contextLabels: [
          project.personId ? identityNames[`person:${project.personId}`] ?? project.personId : null,
          project.propertyId ? identityNames[`property:${project.propertyId}`] ?? project.propertyId : null,
          project.contractId ? identityNames[`contract:${project.contractId}`] ?? project.contractId : null,
        ].filter((label): label is string => Boolean(label)),
      } : {}),
      calendarItems: mapProjectCalendarItems(projectItems),
      documents: documents.filter((document) => project.propertyId != null && document.propertyId === project.propertyId).map((document) => ({ id: document.id, title: document.title ?? 'Document', state: document.state, propertyId: document.propertyId, createdAt: document.createdAt })),
      activity: activity.filter((entry) => (project.personId ? entry.personId === project.personId : false) || (project.propertyId ? Boolean(entry.propertyName && identityNames[`property:${project.propertyId}`] === entry.propertyName) : false)).map((entry) => ({ id: entry.id, channel: entry.channel, direction: entry.direction, occurredAt: entry.occurredAt, occurredAtLabel: entry.occurredAtLabel, title: entry.title, summary: entry.summary, personName: entry.personName, propertyName: entry.propertyName })),
      workNodes: top.map((node) => attach(projectItems, node)),
    }
    const action = firstAction(plan.workNodes)
    if (action) {
      plan.nextAction = action.title
      plan.nextActionDetail = action.status === "in-progress" ? "In progress" : "Ready to work"
    }

    // Unique person/property anchors among this project's items.
    const anchors = new Map<string, { type: string; id: string }>()
    for (const item of projectItems) {
      const entity = item.entity
      if (!entity) continue
      if (entity.type !== "person" && entity.type !== "property") continue
      anchors.set(`${entity.type}:${entity.id}`, entity)
    }

    if (anchors.size > 0) {
      for (const anchor of anchors.values()) {
        const key = `${anchor.type}:${anchor.id}`
        const domain = ENTITY_TO_DOMAIN[anchor.type] ?? "firm"
        const existing = anchored.get(key)
        if (existing) existing.plans.push(plan)
        else anchored.set(key, { type: anchor.type, id: anchor.id, domain, plans: [plan] })
      }
    } else {
      const domain = categoryToDomain(String(project.areas?.[0] ?? "management"))
      fallbackByDomain.set(domain, [...(fallbackByDomain.get(domain) ?? []), plan])
    }
  }

  const poles: ProjectPole[] = []
  const avg = (plans: ProjectPlan[]) =>
    plans.length ? Math.round(plans.reduce((sum, p) => sum + p.progress, 0) / plans.length) : 0

  for (const domain of DOMAINS) {
    for (const entry of anchored.values()) {
      if (entry.domain !== domain.key) continue
      poles.push({
        id: `entity-${entry.type}-${entry.id}`,
        domain: domain.key,
        label: identityNames[`${entry.type}:${entry.id}`] ?? entry.id,
        subtitle: entry.type === "person" ? "Client" : entry.type === "property" ? "Property" : "Workspace",
        progress: avg(entry.plans),
        statusLabel: "Active",
        projects: entry.plans,
      })
    }
    const fallback = fallbackByDomain.get(domain.key)
    if (fallback && fallback.length > 0) {
      poles.push({
        id: `collection-${domain.key}`,
        domain: domain.key,
        label: domain.label,
        subtitle: `${fallback.length} ${fallback.length === 1 ? "project" : "projects"}`,
        progress: avg(fallback),
        statusLabel: "Collection",
        projects: fallback,
      })
    }
  }

  return { domains: DOMAINS, poles }
}
