import 'server-only'

import { randomUUID } from 'node:crypto'

import { coreServices } from '@/lib/service-runtime'
import { getActingUser } from '@/lib/auth/get-acting-user'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { resolveSecurityLevel } from '@/services/security'
import { CONTRACT_OPERATIONS, type ContractSummaryDto } from '@/services/contract'
import { PROPERTY_OPERATIONS, type PropertyIntro } from '@/services/property'
import type { Result } from '@/db/client'

/**
 * The Contracts portfolio's reads — through the CONTRACT SERVICE, never the
 * database (docs/REAL-ESTATE-TRANSACTION-DESIGN.md section 7.5).
 *
 * The nav already says *Contracts* while the page below it lists Deals. A deal is
 * really the *transaction*; a contract is its artifact. This module makes the
 * artifacts visible without removing the transaction view: the page composes
 * both, so nothing disappears when the contract count is still low.
 *
 * The kernel authorizes off `principal`, so the context is built from the portal
 * session. Without one the kernel sees GUEST and refuses the command-side reads.
 */

export type { ContractSummaryDto }

/** One contract row, enriched with the property label the service does not own. */
export type ContractPortfolioRow = ContractSummaryDto & {
  propertyLabel: string | null
}

const service = coreServices.contract
const properties = coreServices.property

async function portalContext() {
  const correlationId = randomUUID()
  try {
    const acting = await getActingUser(getPortalSessionAdapter())
    return {
      actor: { id: acting.appUserId, kind: 'user' as const },
      correlationId,
      principal: {
        appUserId: acting.appUserId,
        level: resolveSecurityLevel(acting.roleCodes),
        roleCodes: acting.roleCodes,
      },
    }
  } catch {
    // No session: the read fails closed (the kernel returns FORBIDDEN).
    return { actor: { id: null, kind: 'system' as const }, correlationId }
  }
}

function failure(operation: string, correlationId: string, code: string, detail: string): Result<never> {
  return {
    ok: false,
    error: { kind: 'UNKNOWN', operation, incidentId: correlationId, code, detail },
  }
}

/**
 * The Contracts portfolio, newest first, each row carrying its property label.
 *
 * Property labels come from the Property service (one intro read per distinct
 * property — a handful of rows). A label that cannot be read leaves the row
 * intact with a null label: the contract is still true.
 */
export async function listContractPortfolio(): Promise<Result<ContractPortfolioRow[]>> {
  const context = await portalContext()
  const operation = CONTRACT_OPERATIONS.LIST

  const result = await service.execute({ operation, payload: {}, context })
  if (!result.ok) return failure(operation, context.correlationId, result.error.code, result.error.message)

  const contracts = result.value as ContractSummaryDto[]
  const propertyIds = [...new Set(contracts.map((contract) => contract.propertyId).filter(Boolean))]

  const labels = new Map<string, string | null>()
  await Promise.all(
    propertyIds.map(async (propertyId) => {
      const intro = await properties.execute({
        operation: PROPERTY_OPERATIONS.INTRO,
        payload: { propertyId },
        context,
      })
      if (!intro.ok) return
      // Property reads return a Result inside the envelope (the contract read
      // returns its rows directly) — unwrap it rather than treat it as data.
      const unwrapped = intro.value as unknown as Result<PropertyIntro | null>
      if (!unwrapped.ok) return
      const property = unwrapped.data
      labels.set(propertyId, property ? property.name ?? property.location ?? null : null)
    }),
  )

  return {
    ok: true,
    data: contracts.map((contract) => ({
      ...contract,
      propertyLabel: labels.get(contract.propertyId) ?? null,
    })),
  }
}
