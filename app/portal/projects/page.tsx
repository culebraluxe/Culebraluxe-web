import { ProjectsWorkspace } from "@/components/portal/projects-workspace"
import { SqlProjectRepository } from "@/db/project-service-repository"
import { SqlWbsRepository } from "@/db/wbs-service-repository"
import { captureServerError } from "@/lib/server-error-capture"
import { sql } from "@/db/client"
import type { WbsItem } from "@/services/wbs"
import type { ProjectsWorkspaceData } from "@/ui/projects/model"
import { mapRealProjectsToWorkspace } from "@/ui/projects/service-projection"

export const dynamic = "force-dynamic"

/** Resolve real display names for the entity anchors (name-resolution seam).
 *  Property names come from the real property table; other anchors fall back
 *  to their id until the person/contract name seam is wired. */
async function resolveIdentityNames(items: WbsItem[]): Promise<Record<string, string>> {
  const names: Record<string, string> = {}
  const idsFor = (type: string) =>
    Array.from(new Set(items.map((i) => (i.entity?.type === type ? i.entity.id : "")).filter(Boolean)))
  for (const id of idsFor("property")) {
    try {
      const rows = await sql`select name from property where id = ${id} limit 1`
      const name = rows[0]?.name as string | undefined
      if (name) names[`property:${id}`] = name
    } catch {
      /* unresolved anchor -> id fallback in the projection */
    }
  }
  for (const id of idsFor("person")) {
    try {
      const rows = await sql`select display_name from mv_client_directory where person_id = ${id} limit 1`
      const name = rows[0]?.display_name as string | undefined
      if (name) names[`person:${id}`] = name
    } catch {
      /* unresolved anchor -> id fallback in the projection */
    }
  }
  return names
}

/** Load the Projects workspace from the REAL Project + WBS service
 *  repositories (Project is the canonical parent). On read failure the page
 *  falls back to the fixture and the failure is captured durably. */
async function loadRealProjectsData(): Promise<ProjectsWorkspaceData | undefined> {
  try {
    const wbs = new SqlWbsRepository()
    const items = await wbs.listDue({})
    const projects = await new SqlProjectRepository().list()
    const identityNames = await resolveIdentityNames(items)
    return mapRealProjectsToWorkspace(projects, items, identityNames)
  } catch (error) {
    captureServerError("projects:load-workspace-data", error, { level: "error" })
    return undefined
  }
}

// PROJECTS-UX page — real MVI runtime backed by Neon (Project + WBS services).
export default async function ProjectsPage() {
  const initialData = await loadRealProjectsData()
  return <ProjectsWorkspace initialData={initialData} />
}

