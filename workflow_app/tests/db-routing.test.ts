// ---------------------------------------------------------------------------
// DB-ROUTING — production database target resolution.
//
// Vercel sets VERCEL_ENV, not APP_ENV. Routing must never let a Vercel
// Production runtime silently fall through to the DEV database (that was the
// production-incident root cause: `env=development` in prod runtime logs).
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import { resolveDbTarget, getDatabaseUrl, DbConfigError } from '../../db/client'

test('routing: VERCEL_ENV=production -> PROD', () => {
  assert.equal(resolveDbTarget({ VERCEL_ENV: 'production' }), 'prod')
})

test('routing: VERCEL_ENV=preview -> DEV', () => {
  assert.equal(resolveDbTarget({ VERCEL_ENV: 'preview' }), 'dev')
})

test('routing: VERCEL_ENV=development (vercel dev) -> DEV', () => {
  assert.equal(resolveDbTarget({ VERCEL_ENV: 'development' }), 'dev')
})

test('routing: local APP_ENV=development -> DEV', () => {
  assert.equal(resolveDbTarget({ APP_ENV: 'development' }), 'dev')
  assert.equal(resolveDbTarget({}), 'dev') // missing APP_ENV defaults to dev locally
})

test('routing: local APP_ENV=production -> PROD (explicitly supported)', () => {
  assert.equal(resolveDbTarget({ APP_ENV: 'production' }), 'prod')
})

test('routing: Vercel production + APP_ENV missing -> PROD, never DEV', () => {
  assert.equal(resolveDbTarget({ VERCEL_ENV: 'production' }), 'prod')
})

test('routing: Vercel production + APP_ENV=development -> MUST be PROD, never DEV', () => {
  // The exact dangerous combination from the incident.
  assert.equal(
    resolveDbTarget({ VERCEL_ENV: 'production', APP_ENV: 'development' }),
    'prod',
  )
})

test('routing: Vercel production + APP_ENV=production -> PROD', () => {
  assert.equal(
    resolveDbTarget({ VERCEL_ENV: 'production', APP_ENV: 'production' }),
    'prod',
  )
})

test('routing: missing required PROD URL -> configuration failure, NOT DEV fallback', () => {
  const saved: Record<string, string | undefined> = {
    VERCEL_ENV: process.env.VERCEL_ENV,
    APP_ENV: process.env.APP_ENV,
    DATABASE_URL_PROD: process.env.DATABASE_URL_PROD,
    DATABASE_URL_DEV: process.env.DATABASE_URL_DEV,
  }
  try {
    process.env.VERCEL_ENV = 'production'
    process.env.APP_ENV = 'development' // must NOT win
    process.env.DATABASE_URL_PROD = ''
    process.env.DATABASE_URL_DEV = 'postgres://dev-should-not-be-used'
    assert.throws(() => getDatabaseUrl(), DbConfigError)
  } finally {
    for (const [k, v] of Object.entries(saved)) {
      if (v === undefined) delete process.env[k]
      else process.env[k] = v
    }
  }
})
