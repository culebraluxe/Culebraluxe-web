// ---------------------------------------------------------------------------
// DEV bypass TECH-nav correction.
//
// The synthetic DEV bypass actor must expose the same authority the TECH nav
// gate and TECH pages/actions require (tech.access), so bypass mode shows and
// uses the TECH engineering surfaces. Normal auth/authorization is unchanged;
// production bypass stays impossible.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  isPortalAuthBypass,
  portalAuthBypassActor,
} from '../../lib/auth/dev-bypass'

type Env = Record<string, string | undefined>

function withEnv(env: Env, fn: () => boolean): boolean {
  const saved: Env = {}
  for (const k of Object.keys(env)) saved[k] = process.env[k]
  for (const [k, v] of Object.entries(env)) {
    if (v === undefined) delete process.env[k]
    else process.env[k] = v
  }
  try {
    return fn()
  } finally {
    for (const [k, v] of Object.entries(saved)) {
      if (v === undefined) delete process.env[k]
      else process.env[k] = v
    }
  }
}

test('DEV bypass actor exposes tech.access (TECH nav gate)', () => {
  const actor = portalAuthBypassActor()
  assert.ok(actor.authorityCodes.includes('tech.access'), 'tech.access present')
  assert.ok(actor.authorityCodes.includes('portal.read'), 'portal.read present')
})

test('bypass is active only when PORTAL_AUTH_BYPASS=1 in non-production', () => {
  const active = withEnv(
    { PORTAL_AUTH_BYPASS: '1', NODE_ENV: 'development', APP_ENV: 'development', NODE_TEST_CONTEXT: undefined },
    () => isPortalAuthBypass(),
  )
  assert.equal(active, true)
  const off = withEnv(
    { PORTAL_AUTH_BYPASS: '0', NODE_ENV: 'development', APP_ENV: 'development', NODE_TEST_CONTEXT: undefined },
    () => isPortalAuthBypass(),
  )
  assert.equal(off, false)
})

test('production bypass is impossible even if the flag is set', () => {
  const nodeProd = withEnv(
    { PORTAL_AUTH_BYPASS: '1', NODE_ENV: 'production', APP_ENV: 'development', NODE_TEST_CONTEXT: undefined },
    () => isPortalAuthBypass(),
  )
  assert.equal(nodeProd, false)
  const appProd = withEnv(
    { PORTAL_AUTH_BYPASS: '1', NODE_ENV: 'development', APP_ENV: 'production', NODE_TEST_CONTEXT: undefined },
    () => isPortalAuthBypass(),
  )
  assert.equal(appProd, false)
})
