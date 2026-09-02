import type { NextRequest } from 'next/server'
import QRCode from 'qrcode'
import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'

export const dynamic = 'force-dynamic'

/**
 * QR PNG of a PUBLIC listing URL only (never a portal URL). Gated by portal.read
 * so the workbench can render a scannable code for the seller-facing page.
 */
export async function GET(request: NextRequest) {
  const access = await resolvePortalAccess(createAuthJsSessionAdapter(), 'portal.read')
  if (!access.ok) return new Response('Unauthorized', { status: 401 })

  const url = request.nextUrl.searchParams.get('url')
  if (!url || !/^https:\/\/(www\.)?culebraluxe\.com\/listings\/[A-Za-z0-9_-]+\/?$/.test(url)) {
    return new Response('Bad url', { status: 400 })
  }

  const buffer = await QRCode.toBuffer(url, {
    type: 'png',
    width: 360,
    margin: 2,
    errorCorrectionLevel: 'M',
  })
  return new Response(new Uint8Array(buffer), {
    headers: {
      'Content-Type': 'image/png',
      'Cache-Control': 'public, max-age=300',
    },
  })
}
