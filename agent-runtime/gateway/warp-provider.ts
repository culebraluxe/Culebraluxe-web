import { CORE_CAPABILITIES } from '../capabilities'
import type { ForgeProviderDescriptor } from './provider'

/**
 * Warp remains fail-closed until a headless wrapper is configured. V6.1 adds
 * the frozen provider/model to that wrapper contract so role mappings cannot
 * silently fall back to a wrapper default model.
 */
export const warpProvider: ForgeProviderDescriptor = {
  id: 'warp',
  description: 'Warp provider (headless wrapper required, explicit model)',
  capabilities: CORE_CAPABILITIES,
  buildCommand: ({ cwd, task, modelRef }) => {
    const headlessBin = process.env.WARP_HEADLESS_BIN?.trim()
    if (!headlessBin) {
      throw new Error(
        "Warp Agent CLI 'warp' is interactive-only for this integration; " +
          'set WARP_HEADLESS_BIN to a Forge-compatible headless wrapper',
      )
    }
    return {
      bin: headlessBin,
      args: modelRef
        ? ['--cwd', cwd, '--task', task, '--model', modelRef]
        : ['--cwd', cwd, '--task', task],
    }
  },
}
