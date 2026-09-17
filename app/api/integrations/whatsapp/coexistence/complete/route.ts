import { NextResponse, type NextRequest } from 'next/server'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { captureServerError } from '@/lib/server-error-capture'
import { loadMetaWhatsAppConfiguration } from '@/lib/whatsapp-cloud/config'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

const GRAPH_VERSION = 'v26.0'
const META_APP_ID = '1573618894304413'
const DEFAULT_WABA_ID = '1605543247626812'

type CompletionRequest = {
  code?: unknown
  wabaId?: unknown
  phoneNumberId?: unknown
}

type MetaTokenResponse = {
  access_token?: string
  token_type?: string
  expires_in?: number
  error?: {
    message?: string
    type?: string
    code?: number
    error_subcode?: number
  }
}

type MetaSubscriptionResponse = {
  success?: boolean | string
  error?: MetaTokenResponse['error']
}

type MetaSystemUserResponse = {
  id?: string
  error?: MetaTokenResponse['error']
}

function metaSucceeded(payload: MetaSubscriptionResponse): boolean {
  return payload.success === true || payload.success === 'true'
}

type MetaPhoneStatus = {
  id?: string
  display_phone_number?: string
  is_on_biz_app?: boolean
  platform_type?: string
  status?: string
  code_verification_status?: string
  error?: MetaTokenResponse['error']
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}

function safeMetaError(prefix: string, payload: { error?: MetaTokenResponse['error'] }): string {
  const code = payload.error?.code
  return code ? `${prefix} (Meta error ${code}).` : prefix
}

async function POSTHandler(request: NextRequest) {
  const access = await resolvePortalAccess(
    createAuthJsSessionAdapter(),
    'portal.read',
  )
  if (!access.ok) {
    return NextResponse.json(
      { ok: false, error: 'Portal authorization required.' },
      { status: access.redirectTo === '/login' ? 401 : 403 },
    )
  }

  let body: CompletionRequest
  try {
    body = await request.json() as CompletionRequest
  } catch {
    return NextResponse.json(
      { ok: false, error: 'Invalid completion request.' },
      { status: 400 },
    )
  }

  const code = stringValue(body.code)
  const wabaId = stringValue(body.wabaId)
  const sessionPhoneNumberId = stringValue(body.phoneNumberId)
  const expectedWabaId = process.env.WHATSAPP_WABA_ID?.trim() || DEFAULT_WABA_ID

  if (!code || !wabaId) {
    return NextResponse.json(
      { ok: false, error: 'Embedded Signup code and WABA ID are required.' },
      { status: 400 },
    )
  }

  // This endpoint is deliberately pinned to CulebraLuxe's one approved WABA.
  // Never let a browser-provided Embedded Signup result redirect our app secret
  // toward an arbitrary WhatsApp Business Account.
  if (wabaId !== expectedWabaId) {
    return NextResponse.json(
      { ok: false, error: 'Meta returned an unexpected WhatsApp Business Account.' },
      { status: 409 },
    )
  }

  let config: ReturnType<typeof loadMetaWhatsAppConfiguration>
  try {
    config = loadMetaWhatsAppConfiguration()
  } catch (error) {
    captureServerError('/api/integrations/whatsapp/coexistence/complete', error, {
      route: '/api/integrations/whatsapp/coexistence/complete',
    })
    return NextResponse.json(
      { ok: false, error: 'WhatsApp production configuration is incomplete.' },
      { status: 500 },
    )
  }

  if (sessionPhoneNumberId && sessionPhoneNumberId !== config.phoneNumberId) {
    return NextResponse.json(
      { ok: false, error: 'Meta returned an unexpected WhatsApp phone-number asset.' },
      { status: 409 },
    )
  }

  const systemUserAccessToken = process.env.WHATSAPP_ACCESS_TOKEN?.trim()
  if (!systemUserAccessToken) {
    return NextResponse.json(
      { ok: false, error: 'WhatsApp system-user access token is not configured.' },
      { status: 500 },
    )
  }

  try {
    // Coexistence Embedded Signup returns a one-time authorization code. Meta's
    // onboarding contract requires the app secret to exchange it server-to-server.
    // The app secret and resulting business token never enter browser-visible JSON.
    const tokenUrl = new URL(`https://graph.facebook.com/${GRAPH_VERSION}/oauth/access_token`)
    tokenUrl.searchParams.set('client_id', META_APP_ID)
    tokenUrl.searchParams.set('client_secret', config.appSecret)
    tokenUrl.searchParams.set('code', code)

    const tokenResponse = await fetch(tokenUrl, {
      method: 'GET',
      cache: 'no-store',
      headers: { accept: 'application/json' },
    })
    const tokenPayload = await tokenResponse.json() as MetaTokenResponse
    const businessToken = tokenPayload.access_token?.trim()

    if (!tokenResponse.ok || !businessToken) {
      console.error('[whatsapp-coexistence] authorization-code exchange failed', {
        status: tokenResponse.status,
        metaCode: tokenPayload.error?.code,
        metaType: tokenPayload.error?.type,
      })
      return NextResponse.json(
        { ok: false, error: safeMetaError('Meta rejected the Embedded Signup authorization code.', tokenPayload) },
        { status: 502 },
      )
    }

    // Meta requires an admin system-user token for the WABA assignment. Resolve
    // the existing CulebraLuxe system user's ID from the already-configured token
    // so this flow needs no additional Vercel variable.
    const systemUserResponse = await fetch(
      `https://graph.facebook.com/${GRAPH_VERSION}/me?fields=id`,
      {
        method: 'GET',
        cache: 'no-store',
        headers: {
          authorization: `Bearer ${systemUserAccessToken}`,
          accept: 'application/json',
        },
      },
    )
    const systemUserPayload = await systemUserResponse.json() as MetaSystemUserResponse
    const systemUserId = systemUserPayload.id?.trim()

    if (!systemUserResponse.ok || !systemUserId) {
      console.error('[whatsapp-coexistence] system-user identity lookup failed', {
        status: systemUserResponse.status,
        metaCode: systemUserPayload.error?.code,
      })
      return NextResponse.json(
        { ok: false, error: safeMetaError('Meta could not identify the CulebraLuxe system user.', systemUserPayload) },
        { status: 502 },
      )
    }

    // Required Embedded Signup authorization step: grant the provider system
    // user MANAGE authority on the WABA before subscribing the app.
    const assignmentUrl = new URL(
      `https://graph.facebook.com/${GRAPH_VERSION}/${encodeURIComponent(wabaId)}/assigned_users`,
    )
    assignmentUrl.searchParams.set('user', systemUserId)
    assignmentUrl.searchParams.set('tasks', "['MANAGE']")

    const assignmentResponse = await fetch(assignmentUrl, {
      method: 'POST',
      cache: 'no-store',
      headers: {
        authorization: `Bearer ${systemUserAccessToken}`,
        accept: 'application/json',
      },
    })
    const assignmentPayload = await assignmentResponse.json() as MetaSubscriptionResponse

    if (!assignmentResponse.ok || !metaSucceeded(assignmentPayload)) {
      console.error('[whatsapp-coexistence] system-user WABA assignment failed', {
        status: assignmentResponse.status,
        metaCode: assignmentPayload.error?.code,
      })
      return NextResponse.json(
        { ok: false, error: safeMetaError('Meta did not assign the CulebraLuxe system user to the WABA.', assignmentPayload) },
        { status: 502 },
      )
    }

    // Required Tech Provider onboarding step: subscribe this app to the WABA
    // using the business token generated by THIS Embedded Signup transaction.
    // This is intentionally NOT phone registration. Coexistence keeps the same
    // WhatsApp Business App number and skips the phone-registration step.
    const subscriptionResponse = await fetch(
      `https://graph.facebook.com/${GRAPH_VERSION}/${encodeURIComponent(wabaId)}/subscribed_apps`,
      {
        method: 'POST',
        cache: 'no-store',
        headers: {
          authorization: `Bearer ${businessToken}`,
          accept: 'application/json',
        },
      },
    )
    const subscriptionPayload = await subscriptionResponse.json() as MetaSubscriptionResponse

    if (!subscriptionResponse.ok || !metaSucceeded(subscriptionPayload)) {
      console.error('[whatsapp-coexistence] WABA subscription failed', {
        status: subscriptionResponse.status,
        metaCode: subscriptionPayload.error?.code,
      })
      return NextResponse.json(
        { ok: false, error: safeMetaError('Meta did not complete the WABA subscription.', subscriptionPayload) },
        { status: 502 },
      )
    }

    // Read back the exact existing business line so the launcher can show
    // whether Meta has actually flipped the number into Coexistence. No phone
    // registration, migration, disconnect, replacement, or deletion is called.
    const phoneUrl = new URL(
      `https://graph.facebook.com/${GRAPH_VERSION}/${encodeURIComponent(config.phoneNumberId)}`,
    )
    phoneUrl.searchParams.set(
      'fields',
      'display_phone_number,is_on_biz_app,platform_type,status,code_verification_status',
    )

    const phoneResponse = await fetch(phoneUrl, {
      method: 'GET',
      cache: 'no-store',
      headers: {
        authorization: `Bearer ${businessToken}`,
        accept: 'application/json',
      },
    })
    const phonePayload = await phoneResponse.json() as MetaPhoneStatus

    if (!phoneResponse.ok) {
      console.warn('[whatsapp-coexistence] onboarding completed but phone status read failed', {
        status: phoneResponse.status,
        metaCode: phonePayload.error?.code,
      })
      return NextResponse.json({
        ok: true,
        subscribed: true,
        wabaId,
        phoneNumberId: config.phoneNumberId,
        phoneStatus: null,
        warning: 'Onboarding completed, but Meta did not return phone status yet.',
      })
    }

    return NextResponse.json({
      ok: true,
      subscribed: true,
      wabaId,
      phoneNumberId: config.phoneNumberId,
      phoneStatus: {
        id: phonePayload.id ?? config.phoneNumberId,
        displayPhoneNumber: phonePayload.display_phone_number ?? null,
        isOnBusinessApp: phonePayload.is_on_biz_app ?? null,
        platformType: phonePayload.platform_type ?? null,
        status: phonePayload.status ?? null,
        codeVerificationStatus: phonePayload.code_verification_status ?? null,
      },
    })
  } catch (error) {
    captureServerError('/api/integrations/whatsapp/coexistence/complete', error, {
      route: '/api/integrations/whatsapp/coexistence/complete',
    })
    console.error('[whatsapp-coexistence] server completion failed', {
      error: error instanceof Error ? error.message : 'unknown error',
    })
    return NextResponse.json(
      { ok: false, error: 'Coexistence server completion failed.' },
      { status: 502 },
    )
  }
}

export const POST = POSTHandler
