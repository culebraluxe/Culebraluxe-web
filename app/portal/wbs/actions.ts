"use server"

import { randomUUID } from "node:crypto"

import { getActingUser } from "@/lib/auth/get-acting-user"
import { getPortalSessionAdapter } from "@/lib/auth/portal-session"
import { SqlWbsRepository } from "@/db/wbs-service-repository"
import { resolveSecurityLevel } from "@/services/security"
import { WbsService } from "@/services/wbs"
import type { WbsCategoryId } from "@/services/wbs"
import { appServiceErrorSink } from "@/lib/service-error-sink"
import {
  AuthorizationService,
  StaticAuthorizationPolicyProvider,
} from "@/services/entitlement"
import type { ServiceContext } from "@/services/core"

export type WbsActionResult =
  | { ok: true; id: string }
  | { ok: false; code: string; message: string }

function wbsService(): WbsService {
  return new WbsService(new SqlWbsRepository(), {
    authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
    errors: appServiceErrorSink(),
  })
}

/** Resolve the acting user to an authorized principal (edge resolution). */
async function runContext(): Promise<ServiceContext> {
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

export async function createWbsItemAction(input: {
  title: string
  category: WbsCategoryId
  projectId?: string | null
}): Promise<WbsActionResult> {
  try {
    const context = await runContext()
    const res = await wbsService().execute({
      operation: "wbs.create",
      payload: { id: randomUUID(), title: input.title, category: input.category, projectId: input.projectId ?? null },
      context,
    })
    if (!res.ok) return { ok: false, code: res.error.code, message: res.error.message }
    return { ok: true, id: res.value.id }
  } catch (caught) {
    return { ok: false, code: "auth", message: caught instanceof Error ? caught.message : "Could not create follow-up." }
  }
}

export async function completeWbsItemAction(id: string): Promise<WbsActionResult> {
  try {
    const context = await runContext()
    const res = await wbsService().execute({ operation: "wbs.complete", payload: { id }, context })
    if (!res.ok) return { ok: false, code: res.error.code, message: res.error.message }
    return { ok: true, id: res.value.id }
  } catch (caught) {
    return { ok: false, code: "auth", message: caught instanceof Error ? caught.message : "Could not complete follow-up." }
  }
}

export async function dismissWbsItemAction(id: string): Promise<WbsActionResult> {
  try {
    const context = await runContext()
    const res = await wbsService().execute({ operation: "wbs.dismiss", payload: { id }, context })
    if (!res.ok) return { ok: false, code: res.error.code, message: res.error.message }
    return { ok: true, id: res.value.id }
  } catch (caught) {
    return { ok: false, code: "auth", message: caught instanceof Error ? caught.message : "Could not dismiss follow-up." }
  }
}
