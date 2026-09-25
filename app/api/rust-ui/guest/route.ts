import { NextResponse, type NextRequest } from 'next/server'

import { auth, AUTH_PROVIDER_EMAIL_CODE, AUTH_PROVIDER_GOOGLE } from '@/auth'
import { withApiHandler } from '@/lib/error-capture-seam'
import { RustApiError, rustApiProvisionGuest, rustApiRequestGuestCode } from '@/lib/rust-api/client'

// ---------------------------------------------------------------------------
// THE GUEST SIGN-IN SCREEN'S TRANSPORT, FOR THE RUST UI (`/account`, rust/ui/src/yew_views/account.rs).
//
// The Yew screen holds no session and no key, so it asks here. Nothing is decided here: Rust sends and checks codes
// and provisions the guest (security/guest.rs). The code check itself is Auth.js's email-code provider, which the
// screen's form posts to directly.
//
//   GET  → who is signed in: provisions the external guest on a first sign-in, then answers with the name.
//   POST → { email } — email a sign-in code.
// ---------------------------------------------------------------------------

async function GETHandler(): Promise<Response> {
  const user = (await auth())?.user
  if (!user?.sub || (user.provider !== AUTH_PROVIDER_GOOGLE && user.provider !== AUTH_PROVIDER_EMAIL_CODE)) {
    return NextResponse.json({ signedIn: false })
  }
  const resolution = await rustApiProvisionGuest(
    { provider: user.provider, providerSubject: user.sub },
    { email: user.email ?? null, emailVerified: user.emailIsVerified, displayName: user.name ?? null },
  )
  if (resolution.kind !== 'known') return NextResponse.json({ signedIn: false })
  return NextResponse.json({
    signedIn: true,
    displayName: resolution.actingUser.displayName,
    email: resolution.actingUser.email ?? user.email ?? null,
  })
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const body = (await req.json().catch(() => null)) as { email?: unknown } | null
  const email = typeof body?.email === 'string' ? body.email : ''
  const clientIp = req.headers.get('x-forwarded-for')?.split(',')[0]?.trim() || req.headers.get('x-real-ip')
  try {
    await rustApiRequestGuestCode(email, clientIp ?? null)
  } catch (error) {
    // A refusal (bad address, too many codes) is the visitor's to fix; anything else is a failure the seam records.
    if (error instanceof RustApiError && error.status < 500) {
      return NextResponse.json({ sent: false, message: error.message }, { status: 422 })
    }
    throw error
  }
  return NextResponse.json({ sent: true })
}

export const GET = withApiHandler({ label: '/api/rust-ui/guest', route: '/api/rust-ui/guest' }, GETHandler)
export const POST = withApiHandler({ label: '/api/rust-ui/guest', route: '/api/rust-ui/guest' }, POSTHandler)
