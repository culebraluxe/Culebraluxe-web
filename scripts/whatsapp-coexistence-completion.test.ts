import assert from 'node:assert/strict'
import test from 'node:test'

import { completeCoexistenceSignup } from '../lib/whatsapp-cloud/coexistence-completion'

test('fake Coexistence transaction sends assigned_users then subscribed_apps', async () => {
  const calls: Array<{ method: string; url: URL; authorization: string | null }> = []

  const fakeGraphFetch: typeof fetch = async (input, init) => {
    const url = new URL(String(input))
    const headers = new Headers(init?.headers)
    calls.push({
      method: init?.method ?? 'GET',
      url,
      authorization: headers.get('authorization'),
    })

    if (url.pathname === '/v26.0/oauth/access_token') {
      assert.equal(url.searchParams.get('code'), 'fixture-one-time-code')
      return Response.json({ access_token: 'fixture-business-token' })
    }
    if (url.pathname === '/v26.0/me') {
      return Response.json({ id: 'fixture-system-user-id' })
    }
    if (url.pathname.endsWith('/assigned_users')) {
      assert.equal(url.searchParams.get('user'), 'fixture-system-user-id')
      assert.equal(url.searchParams.get('tasks'), "['MANAGE']")
      return Response.json({ success: true })
    }
    if (url.pathname.endsWith('/subscribed_apps')) {
      return Response.json({ success: true })
    }
    if (url.pathname === '/v26.0/fixture-phone-id') {
      return Response.json({
        id: 'fixture-phone-id',
        display_phone_number: '+1 787-638-3333',
        is_on_biz_app: true,
        platform_type: 'CLOUD_API',
        status: 'CONNECTED',
      })
    }
    throw new Error(`Unexpected fake Meta request: ${url}`)
  }

  const result = await completeCoexistenceSignup({
    code: 'fixture-one-time-code',
    wabaId: 'fixture-waba-id',
    config: { appSecret: 'fixture-app-secret', phoneNumberId: 'fixture-phone-id' },
    systemUserAccessToken: 'fixture-system-user-token',
    fetchImpl: fakeGraphFetch,
  })

  assert.equal(result.ok, true)
  if (!result.ok) return
  assert.equal(result.body.subscribed, true)

  const outboundPosts = calls.filter((call) => call.method === 'POST')
  assert.deepEqual(
    outboundPosts.map((call) => call.url.pathname),
    [
      '/v26.0/fixture-waba-id/assigned_users',
      '/v26.0/fixture-waba-id/subscribed_apps',
    ],
  )
  assert.equal(outboundPosts[0]?.authorization, 'Bearer fixture-system-user-token')
  assert.equal(outboundPosts[1]?.authorization, 'Bearer fixture-business-token')
})
