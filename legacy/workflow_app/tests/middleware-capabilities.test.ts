// AUTH-CAPABILITIES-01 — the Edge gate must not read a claim no writer stamps.
//
// The JWT callback in auth.ts stamps only `sub` and `provider`. A capability
// read in middleware.ts or lib/auth/middleware-policy.ts therefore can never
// fire: it is dead code that describes a check the system does not perform.
// This test fails if such a read reappears while no writer stamps the claim.
// Stamping the claim is AUTH-CAPABILITIES-02; until it lands the reader stays
// deleted. Comments are stripped before scanning so prose that explains the
// absence of a capability check cannot be mistaken for the check itself.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { decidePortalMiddleware } from '@/lib/auth/middleware-policy'

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..')

const EDGE_GATE_FILES = [
  'middleware.ts',
  'lib/auth/middleware-policy.ts',
] as const

const JWT_WRITER_FILE = 'auth.ts'

function stripComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/(^|[^:])\/\/[^\n]*/g, '$1')
}

function capabilityReads(source: string): string[] {
  return stripComments(source).match(/\.capabilities\b|capabilities\s*[:?]/g) ?? []
}

function hasCapabilityWriter(source: string): boolean {
  return /token\.capabilities\s*(?:=|\?\?=|\|\|=)/.test(stripComments(source))
}

test('AUTH-CAPABILITIES-01: no Edge gate file reads a capability claim', () => {
  for (const file of EDGE_GATE_FILES) {
    const source = readFileSync(resolve(REPO_ROOT, file), 'utf8')
    assert.deepEqual(
      capabilityReads(source),
      [],
      `${file} reads a capability claim that nothing stamps — delete the read, or land the writer (AUTH-CAPABILITIES-02) first`,
    )
  }
})

test('AUTH-CAPABILITIES-01: a capability read is refused while no writer stamps the claim', () => {
  const writerExists = hasCapabilityWriter(
    readFileSync(resolve(REPO_ROOT, JWT_WRITER_FILE), 'utf8'),
  )
  for (const file of EDGE_GATE_FILES) {
    const source = readFileSync(resolve(REPO_ROOT, file), 'utf8')
    if (capabilityReads(source).length > 0) {
      assert.equal(
        writerExists,
        true,
        `${file} reads a capability claim, but ${JWT_WRITER_FILE} never stamps token.capabilities`,
      )
    }
  }
})

test('AUTH-CAPABILITIES-01: the Edge gate decides on authentication alone', () => {
  assert.deepEqual(decidePortalMiddleware('/portal/dashboard', { authenticated: false }), {
    kind: 'redirect',
    to: '/login',
  })
  assert.deepEqual(decidePortalMiddleware('/portal/dashboard', { authenticated: true }), {
    kind: 'next',
  })
  assert.deepEqual(decidePortalMiddleware('/portal/settings', { authenticated: true }), {
    kind: 'next',
  })
  assert.deepEqual(decidePortalMiddleware('/login', { authenticated: false }), { kind: 'next' })
})
