import assert from 'node:assert/strict'
import test from 'node:test'

import { buildGatewayChildEnv } from './cli-agent-adapter'

const ENV_KEYS = [
  'APP_ENV',
  'EXECUTION_ENV',
  'DATABASE_URL',
  'DATABASE_URL_DEV',
  'DATABASE_URL_PROD',
] as const

function withExecutionEnv(
  values: Partial<Record<(typeof ENV_KEYS)[number], string | undefined>>,
  fn: () => void,
): void {
  const before = Object.fromEntries(ENV_KEYS.map((key) => [key, process.env[key]]))
  try {
    for (const key of ENV_KEYS) {
      const value = values[key]
      if (value === undefined) delete process.env[key]
      else process.env[key] = value
    }
    fn()
  } finally {
    for (const key of ENV_KEYS) {
      const value = before[key]
      if (value === undefined) delete process.env[key]
      else process.env[key] = value
    }
  }
}

test('gateway DEV child uses canonical DEV database and hides PROD URL', () => {
  withExecutionEnv(
    {
      DATABASE_URL_DEV: 'postgres://dev',
      DATABASE_URL_PROD: 'postgres://prod',
      DATABASE_URL: 'postgres://dev',
    },
    () => {
      const env = buildGatewayChildEnv('DEV')
      assert.equal(env.APP_ENV, 'development')
      assert.equal(env.EXECUTION_ENV, 'DEV')
      assert.equal(env.DATABASE_URL, 'postgres://dev')
      assert.equal(env.DATABASE_URL_DEV, 'postgres://dev')
      assert.equal(env.DATABASE_URL_PROD, undefined)
    },
  )
})

test('provider credentials pass through gateway child environment', () => {
  withExecutionEnv({ DATABASE_URL_DEV: 'postgres://dev' }, () => {
    const env = buildGatewayChildEnv('DEV', {
      OPENCLAW_TOKEN: 'token-value',
      PROVIDER_SETTING: 'enabled',
    })
    assert.equal(env.OPENCLAW_TOKEN, 'token-value')
    assert.equal(env.PROVIDER_SETTING, 'enabled')
  })
})

test('provider cannot override Forge-owned execution target keys', () => {
  withExecutionEnv(
    {
      DATABASE_URL_DEV: 'postgres://dev',
      DATABASE_URL_PROD: 'postgres://prod',
      DATABASE_URL: 'postgres://dev',
    },
    () => {
      const env = buildGatewayChildEnv('LOCAL', {
        APP_ENV: 'production',
        EXECUTION_ENV: 'PROD',
        DATABASE_URL: 'postgres://attacker',
        DATABASE_URL_DEV: 'postgres://attacker-dev',
        DATABASE_URL_PROD: 'postgres://prod',
      })
      assert.equal(env.APP_ENV, 'development')
      assert.equal(env.EXECUTION_ENV, 'LOCAL')
      assert.equal(env.DATABASE_URL, 'postgres://dev')
      assert.equal(env.DATABASE_URL_DEV, 'postgres://dev')
      assert.equal(env.DATABASE_URL_PROD, undefined)
    },
  )
})

test('gateway child fails closed when DEV and PROD database URLs are identical', () => {
  withExecutionEnv(
    {
      DATABASE_URL_DEV: 'postgres://same',
      DATABASE_URL_PROD: 'postgres://same',
      DATABASE_URL: 'postgres://same',
    },
    () => {
      assert.throws(
        () => buildGatewayChildEnv('DEV'),
        /refusing to start work \(fail-fast\)/,
      )
    },
  )
})
