// ---------------------------------------------------------------------------
// AUTH-04 — environment & secret readiness probe (non-secret).
// SCOPED tests only: boolean surface, production fail-closed posture, DEV/PROD
// database separation, demo-key misconfiguration flag, break-glass reuse.
// Pure — no database, no network.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { getEnvironmentReadiness } from '../../lib/environment-readiness'

const ENV_KEYS = [
  'NODE_ENV',
  'APP_ENV',
  'DATABASE_URL_PROD',
  'DATABASE_URL_DEV',
  'AUTH_SECRET',
  'AUTH_PROVIDER',
  'AUTH_GOOGLE_ID',
  'AUTH_GOOGLE_SECRET',
  'AUTH_ISSUER',
  'AUTH_BREAK_GLASS_ENABLED',
  'AUTH_BREAK_GLASS_APP_USER_ID',
  'AUTH_BREAK_GLASS_SECRET_HASH',
  'GOOGLE_MAPS_API_KEY',
  'GOOGLE_MAPS_DEMO_KEY',
  'MUX_TOKEN_ID_PROD',
  'MUX_TOKEN_SECRET_PROD',
  'MUX_TOKEN_ID_DEV',
  'MUX_TOKEN_SECRET_DEV',
  'BROKER_SIGNATURE_ENABLED',
  'BROKER_SIGNATURE_APP_USER_ID',
  'BROKER_SIGNATURE_MEDIA_ID',
  'BROKER_SIGNATURE_SIGNER_NAME',
  'BROKER_SIGNATURE_LICENSE_NUMBER',
]

function withEnv(
  patch: Record<string, string | undefined>,
  fn: () => void,
): void {
  const saved = new Map<string, string | undefined>()
  for (const key of ENV_KEYS) {
    saved.set(key, process.env[key])
    delete process.env[key]
  }
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) delete process.env[key]
    else process.env[key] = value
  }
  try {
    fn()
  } finally {
    for (const [key, value] of saved) {
      if (value === undefined) delete process.env[key]
      else process.env[key] = value
    }
  }
}

const PROD_SECRETS: Record<string, string> = {
  NODE_ENV: 'production',
  APP_ENV: 'production',
  DATABASE_URL_PROD: 'postgres://prod.example/prod',
  DATABASE_URL_DEV: 'postgres://dev.example/dev',
  AUTH_SECRET: 'auth-secret-present',
  AUTH_GOOGLE_ID: 'client-id-present',
  AUTH_GOOGLE_SECRET: 'client-secret-present',
  GOOGLE_MAPS_API_KEY: 'maps-prod-key-present',
  MUX_TOKEN_ID_PROD: 'mux-token-id-present',
  MUX_TOKEN_SECRET_PROD: 'mux-token-secret-present',
  BROKER_SIGNATURE_ENABLED: 'true',
  BROKER_SIGNATURE_APP_USER_ID: 'owner-user-present',
  BROKER_SIGNATURE_MEDIA_ID: 'signature-media-present',
  BROKER_SIGNATURE_SIGNER_NAME: 'Lisa Penfield',
  BROKER_SIGNATURE_LICENSE_NUMBER: 'C-9931',
}

test('AUTH-04: readiness surface is booleans only, even with no env configured', () => {
  withEnv({}, () => {
    const readiness = getEnvironmentReadiness()
    for (const [key, value] of Object.entries(readiness)) {
      assert.equal(
        typeof value,
        'boolean',
        `readiness.${key} must be a boolean, got ${typeof value}`,
      )
    }
  })
})

test('PLAT-01: Sanity is no longer a property source — readiness surface does not expose it', () => {
  withEnv({}, () => {
    const readiness = getEnvironmentReadiness()
    assert.equal(
      'sanityProjectConfigured' in readiness,
      false,
      'Sanity was retired as a property source; the readiness probe must not report it',
    )
  })
})

test('AUTH-04: production fails closed when required secrets are missing', () => {
  withEnv(
    { NODE_ENV: 'production', APP_ENV: 'production', DATABASE_URL_PROD: 'postgres://prod' },
    () => {
      const readiness = getEnvironmentReadiness()
      assert.equal(readiness.isProduction, true)
      assert.equal(readiness.databaseConfigured, true)
      assert.equal(readiness.authSecretConfigured, false)
      assert.equal(readiness.authProviderConfigured, false)
      assert.equal(readiness.googleMapsKeyConfigured, false)
      assert.equal(readiness.muxConfigured, false)
      assert.equal(readiness.brokerSignatureConfigured, false)
      assert.equal(readiness.brokerSignatureEnabled, false)
      assert.equal(readiness.allProductionRequiredConfigured, false)
    },
  )
})

test('AUTH-04: production fully configured reports ready', () => {
  withEnv(PROD_SECRETS, () => {
    const readiness = getEnvironmentReadiness()
    assert.equal(readiness.isProduction, true)
    assert.equal(readiness.databaseConfigured, true)
    assert.equal(readiness.databaseDevProdSeparated, true)
    assert.equal(readiness.authSecretConfigured, true)
    assert.equal(readiness.authProviderConfigured, true)
    assert.equal(readiness.googleMapsKeyConfigured, true)
    assert.equal(readiness.googleMapsDemoKeyAbsentInProduction, true)
    assert.equal(readiness.muxConfigured, true)
    assert.equal(readiness.brokerSignatureConfigured, true)
    assert.equal(readiness.brokerSignatureEnabled, true)
    assert.equal(readiness.allProductionRequiredConfigured, true)
  })
})

test('AUTH-04: demo key present in production is flagged as misconfiguration', () => {
  withEnv({ ...PROD_SECRETS, GOOGLE_MAPS_DEMO_KEY: 'demo-key-present' }, () => {
    const readiness = getEnvironmentReadiness()
    assert.equal(readiness.googleMapsDemoKeyAbsentInProduction, false)
    assert.equal(readiness.allProductionRequiredConfigured, false)
  })
})

test('AUTH-04: DEV/PROD URLs that are identical break separation', () => {
  withEnv(
    {
      NODE_ENV: 'production',
      APP_ENV: 'production',
      DATABASE_URL_PROD: 'postgres://same.example/db',
      DATABASE_URL_DEV: 'postgres://same.example/db',
    },
    () => {
      const readiness = getEnvironmentReadiness()
      assert.equal(readiness.databaseConfigured, true)
      assert.equal(readiness.databaseDevProdSeparated, false)
    },
  )
})

test('AUTH-04: development resolves the DEV database and accepts the demo key', () => {
  withEnv(
    {
      NODE_ENV: 'development',
      APP_ENV: 'development',
      DATABASE_URL_DEV: 'postgres://dev.example/dev',
      GOOGLE_MAPS_DEMO_KEY: 'demo-key-present',
    },
    () => {
      const readiness = getEnvironmentReadiness()
      assert.equal(readiness.isProduction, false)
      assert.equal(readiness.databaseConfigured, true)
      assert.equal(readiness.googleMapsKeyConfigured, true)
      assert.equal(readiness.googleMapsDemoKeyAbsentInProduction, true)
      assert.equal(readiness.allProductionRequiredConfigured, true)
    },
  )
})

test('AUTH-04: break-glass booleans mirror the AUTH-02 config', () => {
  withEnv(
    {
      AUTH_BREAK_GLASS_ENABLED: 'true',
      AUTH_BREAK_GLASS_APP_USER_ID: 'app-user-present',
      AUTH_BREAK_GLASS_SECRET_HASH: 'hash-present',
    },
    () => {
      const readiness = getEnvironmentReadiness()
      assert.equal(readiness.breakGlassConfigured, true)
      assert.equal(readiness.breakGlassEnabled, true)
    },
  )
  withEnv({}, () => {
    const readiness = getEnvironmentReadiness()
    assert.equal(readiness.breakGlassConfigured, false)
    assert.equal(readiness.breakGlassEnabled, false)
  })
})
