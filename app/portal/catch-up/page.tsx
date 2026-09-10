import { CatchUpBoard } from "@/components/portal/wbs/catch-up-board"
import { SqlWbsRepository } from "@/db/wbs-service-repository"
import { SqlProjectRepository } from "@/db/project-service-repository"
import { ProjectService } from "@/services/project"
import { appServiceErrorSink } from "@/lib/service-error-sink"
import { WbsService } from "@/services/wbs"
import {
  AuthorizationService,
  StaticAuthorizationPolicyProvider,
} from "@/services/entitlement"

export const dynamic = "force-dynamic"

const SYS_CONTEXT = {
  actor: { id: null, kind: "system" as const },
  correlationId: "catchup-page",
}

// CATCH-UP — WBS daily board over services/wbs (DB-backed). Legacy Catch-Up
// components remain under components/portal as reference.
export default async function CatchUpPage() {
  const infrastructure = {
    authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
    errors: appServiceErrorSink(),
  }
  const wbs = new WbsService(new SqlWbsRepository(), infrastructure)
  const project = new ProjectService(new SqlProjectRepository(), infrastructure)

  const [dueRes, projectRes] = await Promise.all([
    wbs.execute({ operation: "wbs.listDue", payload: {}, context: SYS_CONTEXT }),
    project.execute({ operation: "project.list", payload: {}, context: SYS_CONTEXT }),
  ])

  if (!dueRes.ok || !projectRes.ok) {
    return <p role="alert">Catch-Up is temporarily unavailable. Its service read did not complete.</p>
  }

  return (
    <CatchUpBoard
      items={dueRes.value}
      projects={projectRes.value}
    />
  )
}
