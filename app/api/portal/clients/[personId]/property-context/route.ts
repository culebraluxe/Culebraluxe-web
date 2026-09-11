import { randomUUID } from 'node:crypto'
import { NextRequest, NextResponse } from 'next/server'

import { SqlPropertyRepository } from '@/db/property-service-repository'
import {
  PROPERTY_OPERATIONS,
  PropertyService,
} from '@/services/property'
import {
  AuthorizationService,
  StaticAuthorizationPolicyProvider,
} from '@/services/entitlement'
import { appServiceErrorSink } from '@/lib/service-error-sink'
import { withApiHandler } from '@/lib/error-capture-seam'

// Sidecar composition root for the new service architecture. The route owns
// transport only; PropertyService owns the operation contract and repository
// boundary. Forms can consume the same Property DTOs without depending on this
// HTTP adapter.
const propertyService = new PropertyService(new SqlPropertyRepository(), {
  authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
  errors: appServiceErrorSink(),
})

async function GETHandler(
  _request: NextRequest,
  { params }: { params: Promise<{ personId: string }> },
) {
  const { personId } = await params
  const result = await propertyService.execute({
    operation: PROPERTY_OPERATIONS.FOR_PERSON,
    payload: { personId },
    context: {
      actor: { id: null, kind: 'system' },
      correlationId: randomUUID(),
    },
  })

  if (!result.ok) {
    console.error('[property-context] PropertyService failed:', result.error.code, result.error.message)
    return NextResponse.json(
      { error: result.error.code, personId, properties: [] },
      { status: 500 },
    )
  }

  return NextResponse.json(result.value)
}

// ENG-FORGE error-capture: a throw is recorded durably and returns a 500.
export const GET = withApiHandler(
  { label: '/api/portal/clients/[personId]/property-context', route: '/api/portal/clients/[personId]/property-context' },
  GETHandler,
)
