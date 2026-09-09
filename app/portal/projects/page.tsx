import { ProjectsWorkspace } from "@/components/portal/projects-workspace"

export const dynamic = "force-dynamic"

// PROJECTS-UX prototype — real MVI page runtime with a DB-free source.
// Intentionally not added to CORE navigation until the real service source replaces the fixture.
export default function ProjectsPage() {
  return <ProjectsWorkspace />
}
