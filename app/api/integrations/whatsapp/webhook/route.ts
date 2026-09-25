import { NextRequest } from 'next/server'

import { resolveRustApiBaseUrl } from '@/lib/rust-api/contract'
import { withApiHandler } from '@/lib/error-capture-seam'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

function rustBaseUrl(): string | null {
  return resolveRustApiBaseUrl(process.env.RUST_API_BASE_URL, process.env.NODE_ENV)
}

async function proxy(request: NextRequest): Promise<Response> {
  const base = rustBaseUrl()
  if (!base) {
    return new Response('Webhook service is unavailable.', { status: 503 })
  }

  const target = new URL('/api/integrations/whatsapp/webhook', base)
  target.search = request.nextUrl.search

  const headers: Record<string, string> = {}
  const signature = request.headers.get('x-hub-signature-256')
  if (signature) headers['x-hub-signature-256'] = signature
  const contentType = request.headers.get('content-type')
  if (contentType) headers['content-type'] = contentType

  const response = await fetch(target, {
    method: request.method,
    headers,
    body: request.method === 'POST' ? await request.text() : undefined,
    cache: 'no-store',
  })
  const body = await response.arrayBuffer()
  return new Response(body, {
    status: response.status,
    headers: {
      'content-type':
        response.headers.get('content-type') ?? 'application/json',
    },
  })
}

async function GETHandler(request: NextRequest): Promise<Response> {
  return proxy(request)
}

async function POSTHandler(request: NextRequest): Promise<Response> {
  return proxy(request)
}

export const GET = withApiHandler(
  {
    label: '/api/integrations/whatsapp/webhook',
    route: '/api/integrations/whatsapp/webhook',
  },
  GETHandler,
)

export const POST = withApiHandler(
  {
    label: '/api/integrations/whatsapp/webhook',
    route: '/api/integrations/whatsapp/webhook',
  },
  POSTHandler,
)
