import { NextResponse, type NextRequest } from 'next/server'

import { createAuthJsSessionAdapter } from '@/lib/auth/authjs-session-adapter'
import { resolvePortalAccess } from '@/lib/auth/require-portal-access'
import { captureServerError } from '@/lib/server-error-capture'
import { completeCoexistenceSignup } from '@/lib/whatsapp-cloud/coexistence-completion'
import { loadMetaWhatsAppConfiguration } from '@/lib/whatsapp-cloud/config'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

const DEFAULT_WABA_ID = '1605543247626812'

type CompletionRequest = {
  code?: unknown
  wabaId?: unknown
  phoneNumberId?: unknown
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
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
    const result = await completeCoexistenceSignup({
      code,
      wabaId,
      config: {
        appSecret: config.appSecret,
        phoneNumberId: config.phoneNumberId,
      },
      systemUserAccessToken,
    })

    if (!result.ok) {
      console.error('[whatsapp-coexistence] completion step failed', {
        stage: result.stage,
        status: result.metaStatus,
        metaCode: result.metaCode,
      })
      return NextResponse.json(
        { ok: false, error: result.error },
        { status: 502 },
      )
    }

    return NextResponse.json(result.body)
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
