"use server"

import { randomUUID } from "node:crypto"

import { SqlProjectRepository } from "@/db/project-service-repository"
import { SqlWbsRepository } from "@/db/wbs-service-repository"
import { captureServerError } from "@/lib/server-error-capture"
import { appServiceErrorSink } from "@/lib/service-error-sink"
import { getActingUser } from "@/lib/auth/get-acting-user"
import { getPortalSessionAdapter } from "@/lib/auth/portal-session"
import { AuthorizationService, StaticAuthorizationPolicyProvider } from "@/services/entitlement"
import { ServiceRegistry, type ServiceContext } from "@/services/core"
import { ProjectService, type InstantiateProjectRequest } from "@/services/project"
import type { ProjectStatus } from "@/services/project"
import { WbsService } from "@/services/wbs"
import { resolveSecurityLevel } from "@/services/security"

export type InstantiateProjectActionResult =
  | { ok: true; projectId: string; itemIds: string[] }
  | { ok: false; code: string; message: string }

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

/**
 * Authorized Projects write path. ProjectService owns the parent and routes
 * every playbook node to WbsService; this action never writes either table.
 */
export async function instantiateProjectAction(
  input: InstantiateProjectRequest,
): Promise<InstantiateProjectActionResult> {
  try {
    const context = await serviceContext()
    const infrastructure = {
      authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
      errors: appServiceErrorSink(),
    }
    const registry = new ServiceRegistry()
    registry.register(new WbsService(new SqlWbsRepository(), { ...infrastructure, router: registry }))
    const project = registry.register(new ProjectService(new SqlProjectRepository(), { ...infrastructure, router: registry }))
    const result = await project.execute({ operation: "project.instantiate", payload: input, context })
    if (!result.ok) return { ok: false, code: result.error.code, message: result.error.message }
    return { ok: true, projectId: result.value.project.id, itemIds: result.value.itemIds }
  } catch (error) {
    captureServerError("projects:instantiate", error, { level: "error" })
    return { ok: false, code: "PROJECT_INSTANTIATE_FAILED", message: "The project could not be instantiated." }
  }
}

export async function updateProjectStatusAction(
  projectId: string,
  status: ProjectStatus,
): Promise<InstantiateProjectActionResult> {
  try {
    const context = await serviceContext()
    const project = new ProjectService(new SqlProjectRepository(), {
      authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
      errors: appServiceErrorSink(),
    })
    const result = await project.execute({
      operation: "project.update",
      payload: { id: projectId, status },
      context,
    })
    if (!result.ok) return { ok: false, code: result.error.code, message: result.error.message }
    return { ok: true, projectId: result.value.id, itemIds: [] }
  } catch (error) {
    captureServerError("projects:update-status", error, { level: "error" })
    return { ok: false, code: "PROJECT_UPDATE_FAILED", message: "The project status could not be updated." }
  }
}
