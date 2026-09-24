import { randomUUID } from 'node:crypto'
import { getToken } from 'next-auth/jwt'

import { withApiHandler } from '@/lib/error-capture-seam'
import {
  buildRustBridgeHeaders,
  resolveInternalApiKey,
  resolveRustApiBaseUrl,
} from '@/lib/rust-api/contract'

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

async function GETHandler(request: Request, context: { params: Promise<{ id: string }> }) {
  const { id } = await context.params
  if (!UUID_PATTERN.test(id)) return new Response('Not found', { status: 404 })

  const base = resolveRustApiBaseUrl(process.env.RUST_API_BASE_URL, process.env.NODE_ENV)
  const key = resolveInternalApiKey(process.env.CULEBRA_INTERNAL_API_KEY, process.env.AUTH_SECRET)
  if (!base || !key) return new Response('Service Unavailable', { status: 503 })

  const headers = new Headers({
    'x-culebra-internal-key': key,
    'x-culebra-correlation-id': randomUUID(),
  })
  const download = new URL(request.url).searchParams.get('download') === '1' ? '?download=1' : ''

  async function readVault(path: string, bridgeHeaders: HeadersInit): Promise<Response> {
    let upstream: Response
    try {
      upstream = await fetch(`${base}${path}/${id}${download}`, {
        headers: bridgeHeaders,
        cache: 'no-store',
      })
    } catch {
      return new Response('Service Unavailable', { status: 503 })
    }
    if (!upstream.ok) {
      if (upstream.status === 404) return new Response('Not found', { status: 404 })
      return new Response('Document service unavailable', { status: 503 })
    }
    const responseHeaders = new Headers()
    for (const name of ['content-type', 'content-disposition', 'cache-control', 'x-content-type-options']) {
      const value = upstream.headers.get(name)
      if (value) responseHeaders.set(name, value)
    }
    return new Response(upstream.body, { status: 200, headers: responseHeaders })
  }

  // Public Vault authorization needs no Auth.js initialization or portal session.
  const publicDocument = await readVault('/v1/vault/public-listing-documents', headers)
  if (publicDocument.status !== 404) return publicDocument

  // A private document must go through the authenticated Vault operation.
  const secret = process.env.AUTH_SECRET
  const token = secret ? await getToken({ req: request, secret }).catch(() => null) : null
  if (!token?.sub) return new Response('Unauthorized', { status: 401 })

  const { createAuthJsSessionAdapter } = await import('@/lib/auth/authjs-session-adapter')
  const identity = await createAuthJsSessionAdapter().getSession()
  if (!identity) return new Response('Unauthorized', { status: 401 })
  const privateHeaders = buildRustBridgeHeaders({
    identity,
    internalApiKey: key,
    correlationId: randomUUID(),
  })
  return readVault('/v1/vault/document-bytes', privateHeaders)
}

export const GET = withApiHandler(
  { label: '/api/media/documents/[id]', route: '/api/media/documents/[id]' },
  GETHandler,
)
