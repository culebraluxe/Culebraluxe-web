import test from 'node:test'
import assert from 'node:assert/strict'

import { createAgentRuntimeRegistry, defaultDeepSeekConfig } from './factory'

// ---------------------------------------------------------------------------
// THE OPTIONS BAG IS NOT A HARNESS CONFIG.
//
// `createAgentRuntimeRegistry` accepts either a DeepSeekHarnessConfig or an options bag
// (`{ opencode, team, builderFlashOverride, deepseekWorkspace }`). The fallback for a missing
// `deepseek` key used to cast the OPTIONS OBJECT as the harness config, so `cliBin` became
// undefined and every DeepSeek profile (scout / architect / lead) reported:
//
//   adapter 'deepseek-harness:deepseek/deepseek-chat' is not ready:
//   DeepSeek Harness CLI entrypoint not found or not executable: undefined
//
// on a machine where the CLI was installed and executable. The live caller that hit it was
// `scripts/agent-work.ts`, which passes `{ builderFlashOverride }` and nothing else — so the
// worker could not start an architect lane at all (measured on PROD 2026-09-15).
//
// The invariant, asserted here without depending on whether this host has the CLI installed:
// an options bag WITHOUT `deepseek` must behave exactly like the default config.
// ---------------------------------------------------------------------------

test('the default DeepSeek config always names a CLI path', () => {
  const cliBin = defaultDeepSeekConfig().cliBin
  assert.equal(typeof cliBin, 'string')
  assert.ok(cliBin.trim().length > 0, 'cliBin must never be empty — an empty one reads as "not installed"')
})

test('an options bag without `deepseek` uses the default config, not the options object', () => {
  const fromOptionsBag = createAgentRuntimeRegistry({ builderFlashOverride: null })
  const fromDefaults = createAgentRuntimeRegistry()

  const adapterId = 'deepseek-harness:deepseek/deepseek-chat'
  const bag = fromOptionsBag.inspectAdapterReadiness(adapterId)
  const defaults = fromDefaults.inspectAdapterReadiness(adapterId)

  assert.deepEqual(
    { installed: bag.installed, reason: bag.reason },
    { installed: defaults.installed, reason: defaults.reason },
    'an options bag must resolve the DeepSeek adapter the same way the default config does',
  )
  // And the failure this test exists for is specific: the reason must never be built from `undefined`.
  assert.doesNotMatch(String(bag.reason ?? ''), /undefined/, 'a readiness reason must name a real path')
})
