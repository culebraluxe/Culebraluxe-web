import { CatchUpBoard } from "@/components/portal/wbs/catch-up-board"
import { SqlWbsRepository } from "@/db/wbs-service-repository"
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
  const wbs = new WbsService(new SqlWbsRepository(), {
    authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
  })

  const [dueRes, projectRes] = await Promise.all([
    wbs.execute({ operation: "wbs.listDue", payload: {}, context: SYS_CONTEXT }),
    wbs.execute({ operation: "project.list", payload: {}, context: SYS_CONTEXT }),
  ])

  return (
    <CatchUpBoard
      items={dueRes.ok ? dueRes.value : []}
      projects={projectRes.ok ? projectRes.value : []}
    />
  )
}
