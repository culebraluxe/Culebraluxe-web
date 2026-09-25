import { getToken } from 'next-auth/jwt'

import {
  rustApiPrivateMedia,
  rustApiPublicMedia,
} from '@/lib/rust-api/client'
import { withApiHandler } from '@/lib/error-capture-seam'

async function GETHandler(
  request: Request,
  context: { params: Promise<{ id: string }> },
) {
  const { id } = await context.params

  const secret = process.env.AUTH_SECRET
  const token = secret
    ? await getToken({ req: request, secret, secureCookie: true }).catch(() => null)
    : null
  const fallbackToken =
    secret && !token
      ? await getToken({ req: request, secret }).catch(() => null)
      : null
  const authenticated = Boolean(token?.sub ?? fallbackToken?.sub)

  const media = authenticated
    ? await rustApiPrivateMedia(id).catch(() => null)
    : await rustApiPublicMedia(id).catch(() => null)

  if (!media) return new Response('Not found', { status: 404 })

  return new Response(media.bytes, {
    headers: {
      'Content-Type': media.contentType,
      'Cache-Control': authenticated
        ? 'private, no-store'
        : 'public, max-age=3600',
    },
  })
}

export const GET = withApiHandler(
  { label: '/api/media/[id]', route: '/api/media/[id]' },
  GETHandler,
)
