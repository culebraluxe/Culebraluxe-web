export const META_GRAPH_VERSION = 'v26.0'
export const META_APP_ID = '1573618894304413'

export type CoexistenceConfiguration = {
  appSecret: string
  phoneNumberId: string
}

export type CoexistenceCompletionInput = {
  code: string
  wabaId: string
  config: CoexistenceConfiguration
  systemUserAccessToken: string
  fetchImpl?: typeof fetch
}

type MetaError = { message?: string; type?: string; code?: number; error_subcode?: number }
type MetaTokenResponse = { access_token?: string; error?: MetaError }
type MetaSuccessResponse = { success?: boolean | string; error?: MetaError }
type MetaSystemUserResponse = { id?: string; error?: MetaError }
type MetaPhoneStatus = {
  id?: string
  display_phone_number?: string
  is_on_biz_app?: boolean
  platform_type?: string
  status?: string
  code_verification_status?: string
  error?: MetaError
}

export type CoexistenceCompletionResult =
  | { ok: true; body: { ok: true; subscribed: true; wabaId: string; phoneNumberId: string; phoneStatus: { id: string; displayPhoneNumber: string | null; isOnBusinessApp: boolean | null; platformType: string | null; status: string | null; codeVerificationStatus: string | null } | null; warning?: string } }
  | { ok: false; stage: 'code_exchange' | 'system_user_lookup' | 'system_user_assignment' | 'app_subscription'; error: string; metaStatus: number; metaCode?: number }

function metaSucceeded(payload: MetaSuccessResponse): boolean {
  return payload.success === true || payload.success === 'true'
}

function safeMetaError(prefix: string, payload: { error?: MetaError }): string {
  return payload.error?.code ? `${prefix} (Meta error ${payload.error.code}).` : prefix
}

/** Server-only Coexistence transaction; fetch is injected for a no-network fixture. */
export async function completeCoexistenceSignup(input: CoexistenceCompletionInput): Promise<CoexistenceCompletionResult> {
  const graphFetch = input.fetchImpl ?? fetch
  const tokenUrl = new URL(`https://graph.facebook.com/${META_GRAPH_VERSION}/oauth/access_token`)
  tokenUrl.searchParams.set('client_id', META_APP_ID)
  tokenUrl.searchParams.set('client_secret', input.config.appSecret)
  tokenUrl.searchParams.set('code', input.code)
  const tokenResponse = await graphFetch(tokenUrl, { method: 'GET', cache: 'no-store', headers: { accept: 'application/json' } })
  const tokenPayload = await tokenResponse.json() as MetaTokenResponse
  const businessToken = tokenPayload.access_token?.trim()
  if (!tokenResponse.ok || !businessToken) return { ok: false, stage: 'code_exchange', error: safeMetaError('Meta rejected the Embedded Signup authorization code.', tokenPayload), metaStatus: tokenResponse.status, metaCode: tokenPayload.error?.code }

  const systemUserResponse = await graphFetch(`https://graph.facebook.com/${META_GRAPH_VERSION}/me?fields=id`, { method: 'GET', cache: 'no-store', headers: { authorization: `Bearer ${input.systemUserAccessToken}`, accept: 'application/json' } })
  const systemUserPayload = await systemUserResponse.json() as MetaSystemUserResponse
  const systemUserId = systemUserPayload.id?.trim()
  if (!systemUserResponse.ok || !systemUserId) return { ok: false, stage: 'system_user_lookup', error: safeMetaError('Meta could not identify the CulebraLuxe system user.', systemUserPayload), metaStatus: systemUserResponse.status, metaCode: systemUserPayload.error?.code }

  const assignmentUrl = new URL(`https://graph.facebook.com/${META_GRAPH_VERSION}/${encodeURIComponent(input.wabaId)}/assigned_users`)
  assignmentUrl.searchParams.set('user', systemUserId)
  assignmentUrl.searchParams.set('tasks', "['MANAGE']")
  const assignmentResponse = await graphFetch(assignmentUrl, { method: 'POST', cache: 'no-store', headers: { authorization: `Bearer ${input.systemUserAccessToken}`, accept: 'application/json' } })
  const assignmentPayload = await assignmentResponse.json() as MetaSuccessResponse
  if (!assignmentResponse.ok || !metaSucceeded(assignmentPayload)) return { ok: false, stage: 'system_user_assignment', error: safeMetaError('Meta did not assign the CulebraLuxe system user to the WABA.', assignmentPayload), metaStatus: assignmentResponse.status, metaCode: assignmentPayload.error?.code }

  const subscriptionResponse = await graphFetch(`https://graph.facebook.com/${META_GRAPH_VERSION}/${encodeURIComponent(input.wabaId)}/subscribed_apps`, { method: 'POST', cache: 'no-store', headers: { authorization: `Bearer ${businessToken}`, accept: 'application/json' } })
  const subscriptionPayload = await subscriptionResponse.json() as MetaSuccessResponse
  if (!subscriptionResponse.ok || !metaSucceeded(subscriptionPayload)) return { ok: false, stage: 'app_subscription', error: safeMetaError('Meta did not complete the WABA subscription.', subscriptionPayload), metaStatus: subscriptionResponse.status, metaCode: subscriptionPayload.error?.code }

  const phoneUrl = new URL(`https://graph.facebook.com/${META_GRAPH_VERSION}/${encodeURIComponent(input.config.phoneNumberId)}`)
  phoneUrl.searchParams.set('fields', 'display_phone_number,is_on_biz_app,platform_type,status,code_verification_status')
  const phoneResponse = await graphFetch(phoneUrl, { method: 'GET', cache: 'no-store', headers: { authorization: `Bearer ${businessToken}`, accept: 'application/json' } })
  const phonePayload = await phoneResponse.json() as MetaPhoneStatus
  if (!phoneResponse.ok) return { ok: true, body: { ok: true, subscribed: true, wabaId: input.wabaId, phoneNumberId: input.config.phoneNumberId, phoneStatus: null, warning: 'Onboarding completed, but Meta did not return phone status yet.' } }

  return { ok: true, body: { ok: true, subscribed: true, wabaId: input.wabaId, phoneNumberId: input.config.phoneNumberId, phoneStatus: { id: phonePayload.id ?? input.config.phoneNumberId, displayPhoneNumber: phonePayload.display_phone_number ?? null, isOnBusinessApp: phonePayload.is_on_biz_app ?? null, platformType: phonePayload.platform_type ?? null, status: phonePayload.status ?? null, codeVerificationStatus: phonePayload.code_verification_status ?? null } } }
}
