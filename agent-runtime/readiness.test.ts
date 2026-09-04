import assert from 'node:assert/strict'
import { chmodSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import test from 'node:test'

import { createAgentRuntimeRegistry } from './factory'
import { commandIsInstalled, explicitAuthenticationReady } from './readiness'

test('command readiness resolves an executable from PATH', () => {
  const dir = mkdtempSync(join(tmpdir(), 'forge-ready-'))
  const bin = join(dir, 'fake-agent')
  writeFileSync(bin, '#!/bin/sh\nexit 0\n')
  chmodSync(bin, 0o755)
  assert.equal(commandIsInstalled('fake-agent', { PATH: dir }), true)
  assert.equal(commandIsInstalled('missing-agent', { PATH: dir }), false)
})

test('authentication readiness requires an explicit qualification marker', () => {
  assert.equal(explicitAuthenticationReady(undefined), 'required')
  assert.equal(explicitAuthenticationReady('false'), 'required')
  assert.equal(explicitAuthenticationReady('authenticated'), 'authenticated')
})

test('qualified DeepSeek harness reports registered installed and ready on a DeepSeek-default profile', () => {
  const registry = createAgentRuntimeRegistry({
    cliBin: '/not-used-in-test',
    workspace: process.cwd(),
    startRun: (() => { throw new Error('not invoked') }) as never,
  })
  // ENG-FORGE-V5-03: scout-volume still defaults to the forge-native DeepSeek
  // harness (only builder-flash defaults to OpenCode), so a qualified DeepSeek
  // harness reports ready here.
  const readiness = registry.inspectProfileReadiness('scout-volume')
  assert.equal(readiness.registered, true)
  assert.equal(readiness.installed, true)
  assert.equal(readiness.authentication, 'delegated')
  assert.equal(readiness.ready, true)
})

test('explicit deepseek override keeps builder-flash ready on the qualified forge-native harness', () => {
  const previousProvider = process.env.FORGE_PROVIDER_BUILDER_FLASH
  try {
    process.env.FORGE_PROVIDER_BUILDER_FLASH = 'deepseek'
    const registry = createAgentRuntimeRegistry({
      cliBin: '/not-used-in-test',
      workspace: process.cwd(),
      startRun: (() => { throw new Error('not invoked') }) as never,
    })
    // ENG-FORGE-V5-03: builder-flash defaults to OpenCode, so this test
    // explicitly returns it to forge-native DeepSeek (explicit overrides
    // remain supported) and verifies the qualified harness is ready.
    assert.equal(registry.resolveProfile('builder-flash').adapterId, 'deepseek-harness')
    const readiness = registry.inspectProfileReadiness('builder-flash')
    assert.equal(readiness.registered, true)
    assert.equal(readiness.installed, true)
    assert.equal(readiness.authentication, 'delegated')
    assert.equal(readiness.ready, true)
  } finally {
    if (previousProvider === undefined) delete process.env.FORGE_PROVIDER_BUILDER_FLASH
    else process.env.FORGE_PROVIDER_BUILDER_FLASH = previousProvider
  }
})

test('OpenClaw can be registered for a profile while remaining not ready', () => {
  const previousProvider = process.env.FORGE_PROVIDER_BUILDER_FLASH
  const previousPath = process.env.PATH
  const previousAuth = process.env.FORGE_OPENCLAW_AUTHENTICATED
  try {
    process.env.FORGE_PROVIDER_BUILDER_FLASH = 'openclaw'
    process.env.PATH = ''
    delete process.env.FORGE_OPENCLAW_AUTHENTICATED
    const registry = createAgentRuntimeRegistry({
      cliBin: '/not-used-in-test',
      workspace: process.cwd(),
      startRun: (() => { throw new Error('not invoked') }) as never,
    })
    const readiness = registry.inspectProfileReadiness('builder-flash')
    assert.equal(readiness.registered, true)
    assert.equal(readiness.installed, false)
    assert.equal(readiness.authentication, 'required')
    assert.equal(readiness.ready, false)
  } finally {
    if (previousProvider === undefined) delete process.env.FORGE_PROVIDER_BUILDER_FLASH
    else process.env.FORGE_PROVIDER_BUILDER_FLASH = previousProvider
    if (previousPath === undefined) delete process.env.PATH
    else process.env.PATH = previousPath
    if (previousAuth === undefined) delete process.env.FORGE_OPENCLAW_AUTHENTICATED
    else process.env.FORGE_OPENCLAW_AUTHENTICATED = previousAuth
  }
})
