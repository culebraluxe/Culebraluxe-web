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
import type { ProjectsWorkspaceData, ProjectsWorkspaceLoadState } from "@/ui/projects/model"
import { mapRealProjectsToWorkspace } from "@/ui/projects/service-projection"
import { listIssuedDocuments } from "@/lib/vault-io"
import { getActivityFeed } from "@/db/activity-feed"

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

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

/** Resolve real display names for the entity anchors (name-resolution seam).
 *  Property names come from the real property table; other anchors fall back
 *  to their id until the person/contract name seam is wired.
 *
 *  property.id / contract.id are UUID columns, so a text anchor that is not a
 *  UUID must be SKIPPED (never passed to the uuid comparison - that raised
 *  Postgres 22P02 and 500'd the whole page). */
async function resolveIdentityNames(items: WbsItem[], projects: { personId: string | null; propertyId: string | null; contractId: string | null }[]): Promise<Record<string, string>> {
  const names: Record<string, string> = {}
  const idsFor = (type: string) =>
    Array.from(new Set([
      ...items.map((i) => (i.entity?.type === type ? i.entity.id : "")),
      ...projects.map((p) => type === "person" ? p.personId : type === "property" ? p.propertyId : p.contractId),
    ].filter(Boolean)))
  const uuidOnly = (ids: (string | null | undefined)[]) => ids.filter((id): id is string => Boolean(id) && UUID_RE.test(id as string))
  // Each lookup is best-effort: a missing/failed anchor degrades to the id
  // fallback in the projection. Failures are durably captured, never swallowed,
  // and never allowed to take down the whole Projects page.
  for (const id of uuidOnly(idsFor("property"))) {
    try {
      const rows = await sql`select name from property where id = ${id} limit 1`
      const name = rows[0]?.name as string | undefined
      if (name) names[`property:${id}`] = name
    } catch (error) {
      captureServerError("projects:resolve-identity:property", error, { level: "warn" })
    }
  }
  for (const id of uuidOnly(idsFor("person"))) {
    try {
      const rows = await sql`select display_name from mv_client_directory where person_id = ${id} limit 1`
      const name = rows[0]?.display_name as string | undefined
      if (name) names[`person:${id}`] = name
    } catch (error) {
      captureServerError("projects:resolve-identity:person", error, { level: "warn" })
    }
  }
  for (const id of uuidOnly(idsFor("contract"))) {
    try {
      const rows = await sql`select contract_type from contract where id = ${id} limit 1`
      const type = rows[0]?.contract_type as string | undefined
      if (type) names[`contract:${id}`] = type.replaceAll("_", " ")
    } catch (error) {
      captureServerError("projects:resolve-identity:contract", error, { level: "warn" })
    }
  }
  return names
}

type ProjectsLoadResult = {
  data: ProjectsWorkspaceData
  error: string | null
}

/** A truthful workspace payload for a non-ready outcome: no fabricated poles,
 *  only the discriminated load state. */
function stateData(loadState: ProjectsWorkspaceLoadState): ProjectsWorkspaceData {
  return { domains: [], poles: [], loadState }
}

const AUTH_DENIAL_CODES = new Set(["FORBIDDEN", "UNAUTHORIZED", "UNAUTHENTICATED"])

/** Load the Projects MVI through the canonical Project + WBS service contracts.
 *  The result is always a complete page model: `ready`/`empty` carry the
 *  projection, `unauthorized`/`failure` carry a distinct state and no panes. */
async function loadRealProjectsData(): Promise<ProjectsLoadResult> {
  try {
    const infrastructure = {
      authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
      errors: appServiceErrorSink(),
    }
    const wbs = new WbsService(new SqlWbsRepository(), infrastructure)
    const project = new ProjectService(new SqlProjectRepository(), infrastructure)
    const context = await serviceContext()
    const acting = await getActingUser(getPortalSessionAdapter())
    const [itemsResult, projectsResult, documents, activity] = await Promise.all([
      wbs.execute({ operation: "wbs.listProjectItems", payload: {}, context }),
      project.execute({ operation: "project.list", payload: {}, context }),
      listIssuedDocuments({ accountType: acting.accountType, personId: acting.personId }),
      getActivityFeed(200),
    ])
    if (!itemsResult.ok) {
      if (AUTH_DENIAL_CODES.has(itemsResult.error.code)) {
        return { data: stateData({ status: "unauthorized", message: itemsResult.error.message }), error: null }
      }
      throw new Error(`WBS read failed: ${itemsResult.error.code}`)
    }
    if (!projectsResult.ok) {
      if (AUTH_DENIAL_CODES.has(projectsResult.error.code)) {
        return { data: stateData({ status: "unauthorized", message: projectsResult.error.message }), error: null }
      }
      throw new Error(`Project read failed: ${projectsResult.error.code}`)
    }
    const items = itemsResult.value
    const projects = projectsResult.value
    const identityNames = await resolveIdentityNames(items, projects)
    return { data: mapRealProjectsToWorkspace(projects, items, identityNames, documents, activity), error: null }
  } catch (error) {
    captureServerError("projects:load-workspace-data", error, { level: "error" })
    const message = "Projects are temporarily unavailable. The service read failed and no fixture data was substituted."
    return { data: stateData({ status: "failure", message }), error: message }
  }
}

// PROJECTS-UX page — real MVI runtime backed by Neon (Project + WBS services).
export default async function ProjectsPage() {
  const result = await loadRealProjectsData()
  return <ProjectsWorkspace initialData={result.data} loadError={result.error} />
}
