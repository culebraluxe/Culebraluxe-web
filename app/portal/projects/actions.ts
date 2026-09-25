"use server"

import { captureServerError } from "@/lib/server-error-capture"
import {
  RustApiError,
  rustApiCreateProject,
  rustApiCreateWbs,
  rustApiRead,
  rustApiUpdateProject,
} from "@/lib/rust-api/client"

export type ProjectStatus = "open" | "doing" | "done" | "archived"
export type WbsCategoryId =
  | "clients"
  | "contracts"
  | "properties"
  | "media"
  | "marketing"
  | "accounting"
  | "management"

export type ProjectPlaybookNode = {
  key: string
  title: string
  category: WbsCategoryId
  parentKey?: string
  order: number
}

export type InstantiateProjectRequest = {
  id: string
  name: string
  owner?: string | null
  description?: string
  areas?: readonly WbsCategoryId[]
  startsAt?: string | null
  endsAt?: string | null
  projectType?: string | null
  playbookId: string
  playbookVersion: number
  personId?: string | null
  propertyId?: string | null
  contractId?: string | null
  nodes?: readonly ProjectPlaybookNode[]
}

type RustProject = {
  id: string
  name: string
  playbookId: string | null
  playbookVersion: number | null
}

type RustWbsItem = {
  id: string
  projectId: string | null
}

export type InstantiateProjectActionResult =
  | { ok: true; projectId: string; itemIds: string[] }
  | { ok: false; code: string; message: string }

const LISTING_ONBOARDING_V1 = {
  id: "listing-onboarding",
  version: 1,
  projectType: "listing",
  nodes: [
    { key: "parties", title: "Clients / Parties", category: "clients", order: 1 },
    { key: "property", title: "Property", category: "properties", order: 2 },
    { key: "agreement", title: "Listing Agreement", category: "contracts", order: 3 },
    { key: "signature", title: "Seller Signature", category: "contracts", parentKey: "agreement", order: 1 },
    { key: "media", title: "Cabinet + Photos", category: "media", order: 4 },
    { key: "marketing", title: "Coming Soon / Marketing", category: "marketing", order: 5 },
    { key: "accounting", title: "Commission / Accounting", category: "accounting", order: 6 },
  ] satisfies ProjectPlaybookNode[],
} as const

function playbook(id: string, version: number) {
  if (id === LISTING_ONBOARDING_V1.id && version === LISTING_ONBOARDING_V1.version) {
    return LISTING_ONBOARDING_V1
  }
  return null
}

function validateNodes(nodes: readonly ProjectPlaybookNode[]): string | null {
  const keys = new Set<string>()
  for (const node of nodes) {
    if (!node.key.trim() || keys.has(node.key)) {
      return `Playbook node key must be unique: ${node.key}`
    }
    if (node.parentKey && !nodes.some((candidate) => candidate.key === node.parentKey)) {
      return `Parent node is missing: ${node.parentKey}`
    }
    keys.add(node.key)
  }

  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (key: string): string | null => {
    if (visiting.has(key)) return `Playbook parent cycle includes: ${key}`
    if (visited.has(key)) return null
    visiting.add(key)
    const parent = nodes.find((candidate) => candidate.key === key)?.parentKey
    if (parent) {
      const error = visit(parent)
      if (error) return error
    }
    visiting.delete(key)
    visited.add(key)
    return null
  }
  for (const node of nodes) {
    const error = visit(node.key)
    if (error) return error
  }
  return null
}

async function existingProject(id: string): Promise<RustProject | null> {
  try {
    return (await rustApiRead<RustProject>(
      (`/v1/projects/${encodeURIComponent(id)}`) as `/v1/${string}`,
    )).value
  } catch (error) {
    if (error instanceof RustApiError && error.status === 404) return null
    throw error
  }
}

/**
 * Compatibility action for the older React Projects workspace.
 * All persistence and authorization are Rust-owned; this file only preserves
 * the existing playbook orchestration until the remaining Projects UI is Yew.
 */
export async function instantiateProjectAction(
  input: InstantiateProjectRequest,
): Promise<InstantiateProjectActionResult> {
  try {
    const definition = playbook(input.playbookId, input.playbookVersion)
    if (!definition) {
      return {
        ok: false,
        code: "PROJECT_PLAYBOOK_NOT_FOUND",
        message: `Unknown playbook ${input.playbookId}@${input.playbookVersion}.`,
      }
    }

    const existing = await existingProject(input.id)
    if (existing) {
      if (
        existing.playbookId !== definition.id ||
        existing.playbookVersion !== definition.version
      ) {
        return {
          ok: false,
          code: "PROJECT_PLAYBOOK_CONFLICT",
          message: `Project ${input.id} already exists with a different playbook.`,
        }
      }
      const items = (
        await rustApiRead<RustWbsItem[]>("/v1/wbs/project-items")
      ).value
      return {
        ok: true,
        projectId: existing.id,
        itemIds: items
          .filter((item) => item.projectId === existing.id)
          .map((item) => item.id),
      }
    }

    const nodes = input.nodes ?? definition.nodes
    const nodeError = validateNodes(nodes)
    if (nodeError) {
      return {
        ok: false,
        code: "PROJECT_PLAYBOOK_NODE_INVALID",
        message: nodeError,
      }
    }

    const project = (
      await rustApiCreateProject<RustProject>({
        id: input.id,
        name: input.name,
        owner: input.owner ?? null,
        description: input.description ?? "",
        areas: input.areas ?? [],
        startsAt: input.startsAt ?? null,
        endsAt: input.endsAt ?? null,
        projectType: definition.projectType,
        playbookId: definition.id,
        playbookVersion: definition.version,
        personId: input.personId ?? null,
        propertyId: input.propertyId ?? null,
        contractId: input.contractId ?? null,
      })
    ).value

    const itemIds: string[] = []
    for (const node of nodes) {
      const id = `${project.id}-${node.key}`
      const created = (
        await rustApiCreateWbs<RustWbsItem>({
          id,
          title: node.title,
          category: node.category,
          projectId: project.id,
          parentId: node.parentKey
            ? `${project.id}-${node.parentKey}`
            : null,
          order: node.order,
        })
      ).value
      itemIds.push(created.id)
    }

    return { ok: true, projectId: project.id, itemIds }
  } catch (error) {
    captureServerError("projects:instantiate", error, { level: "error" })
    return {
      ok: false,
      code: error instanceof RustApiError ? error.code : "PROJECT_INSTANTIATE_FAILED",
      message:
        error instanceof Error
          ? error.message
          : "The project could not be instantiated.",
    }
  }
}

export async function updateProjectStatusAction(
  projectId: string,
  status: ProjectStatus,
): Promise<InstantiateProjectActionResult> {
  try {
    const result = await rustApiUpdateProject<RustProject>(projectId, { status })
    return { ok: true, projectId: result.value.id, itemIds: [] }
  } catch (error) {
    captureServerError("projects:update-status", error, { level: "error" })
    return {
      ok: false,
      code: error instanceof RustApiError ? error.code : "PROJECT_UPDATE_FAILED",
      message:
        error instanceof Error
          ? error.message
          : "The project status could not be updated.",
    }
  }
}
