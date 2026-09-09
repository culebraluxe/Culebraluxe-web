import { randomUUID } from "node:crypto"

import { ProjectsWorkspace } from "@/components/portal/projects-workspace"
import { SqlProjectRepository } from "@/db/project-service-repository"
import { SqlWbsRepository } from "@/db/wbs-service-repository"
import { captureServerError } from "@/lib/server-error-capture"
import { appServiceErrorSink } from "@/lib/service-error-sink"
import { getActingUser } from "@/lib/auth/get-acting-user"
import { getPortalSessionAdapter } from "@/lib/auth/portal-session"
import { sql } from "@/db/client"
import { ProjectService } from "@/services/project"
import type { ServiceContext } from "@/services/core"
import { resolveSecurityLevel } from "@/services/security"
import { WbsService } from "@/services/wbs"
import {
  AuthorizationService,
  StaticAuthorizationPolicyProvider,
} from "@/services/entitlement"
import type { WbsItem } from "@/services/wbs"
import type { ProjectsWorkspaceData } from "@/ui/projects/model"
import { mapRealProjectsToWorkspace } from "@/ui/projects/service-projection"

export const dynamic = "force-dynamic"

async function serviceContext(): Promise<ServiceContext> {
  const acting = await getActingUser(getPortalSessionAdapter())
  return {
    actor: { id: acting.appUserId, kind: "user" },
    correlationId: randomUUID(),
    principal: {
      appUserId: acting.appUserId,
      level: resolveSecurityLevel(acting.roleCodes),
      roleCodes: acting.roleCodes,
    },
  }
}

/** Resolve real display names for the entity anchors (name-resolution seam).
 *  Property names come from the real property table; other anchors fall back
 *  to their id until the person/contract name seam is wired. */
async function resolveIdentityNames(items: WbsItem[]): Promise<Record<string, string>> {
  const names: Record<string, string> = {}
  const idsFor = (type: string) =>
    Array.from(new Set(items.map((i) => (i.entity?.type === type ? i.entity.id : "")).filter(Boolean)))
  for (const id of idsFor("property")) {
    const rows = await sql`select name from property where id = ${id} limit 1`
    const name = rows[0]?.name as string | undefined
    if (name) names[`property:${id}`] = name
  }
  for (const id of idsFor("person")) {
    const rows = await sql`select display_name from mv_client_directory where person_id = ${id} limit 1`
    const name = rows[0]?.display_name as string | undefined
    if (name) names[`person:${id}`] = name
  }
  return names
}

type ProjectsLoadResult =
  | { data: ProjectsWorkspaceData; error: null }
  | { data: null; error: string }

/** Load the Projects MVI through the canonical Project + WBS service contracts. */
async function loadRealProjectsData(): Promise<ProjectsLoadResult> {
  try {
    const infrastructure = {
      authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
      errors: appServiceErrorSink(),
    }
    const wbs = new WbsService(new SqlWbsRepository(), infrastructure)
    const project = new ProjectService(new SqlProjectRepository(), infrastructure)
    const context = await serviceContext()
    const [itemsResult, projectsResult] = await Promise.all([
      wbs.execute({ operation: "wbs.listProjectItems", payload: {}, context }),
      project.execute({ operation: "project.list", payload: {}, context }),
    ])
    if (!itemsResult.ok) throw new Error(`WBS read failed: ${itemsResult.error.code}`)
    if (!projectsResult.ok) throw new Error(`Project read failed: ${projectsResult.error.code}`)
    const items = itemsResult.value
    const projects = projectsResult.value
    const identityNames = await resolveIdentityNames(items)
    return { data: mapRealProjectsToWorkspace(projects, items, identityNames), error: null }
  } catch (error) {
    captureServerError("projects:load-workspace-data", error, { level: "error" })
    return { data: null, error: "Projects are temporarily unavailable. The service read failed and no fixture data was substituted." }
  }
}

// PROJECTS-UX page — real MVI runtime backed by Neon (Project + WBS services).
export default async function ProjectsPage() {
  const result = await loadRealProjectsData()
  return <ProjectsWorkspace initialData={result.data} loadError={result.error} />
}
