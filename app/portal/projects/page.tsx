import { ProjectsWorkspace } from "@/components/portal/projects-workspace"
import { SqlWbsRepository } from "@/db/wbs-service-repository"
import { captureServerError } from "@/lib/server-error-capture"
import type { ProjectsWorkspaceData } from "@/ui/projects/model"
import { mapRealProjectsToWorkspace } from "@/ui/projects/service-projection"

export const dynamic = "force-dynamic"

/** Load the Projects workspace from the REAL Neon Project + WBS repositories
 *  (the MVI source is swapped to a preloaded source backed by this data). On
 *  any read failure the page falls back to the fixture so it never hard-crashes;
 *  the failure is still captured durably. */
async function loadRealProjectsData(): Promise<ProjectsWorkspaceData | undefined> {
  try {
    const wbs = new SqlWbsRepository()
    const projects = await wbs.listProjects()
    const items = await wbs.listDue({})
    return mapRealProjectsToWorkspace(projects, items)
  } catch (error) {
    captureServerError("projects:load-workspace-data", error, { level: "error" })
    return undefined
  }
}

// PROJECTS-UX page — real MVI runtime backed by Neon test data (service repos).
export default async function ProjectsPage() {
  const initialData = await loadRealProjectsData()
  return <ProjectsWorkspace initialData={initialData} />
}
