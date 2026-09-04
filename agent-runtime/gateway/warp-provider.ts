import { WRITE_CAPABILITIES } from '../lanes'
import type { ForgeProviderDescriptor } from './provider'

/**
 * Warp provider boundary. The installed Warp Agent CLI (`warp`) is currently
 * an interactive conversation TUI; it does not expose a documented one-shot
 * prompt command suitable for Forge's headless process adapter.
 *
 * Forge therefore fails closed unless the operator supplies a compatible
 * headless wrapper through WARP_HEADLESS_BIN. The wrapper contract is:
 *
 *   <wrapper> --cwd <worker-worktree> --task <canonical-forge-task>
 *
 * This keeps the provider seam honest while leaving room for Warp Automation
 * Platform (or a future documented headless CLI contract) without changing
 * Forge story/lane semantics.
 */
export const warpProvider: ForgeProviderDescriptor = {
  id: 'warp',
  description: 'Warp provider (headless wrapper required)',
  capabilities: WRITE_CAPABILITIES,
  buildCommand: ({ cwd, task }) => {
    const headlessBin = process.env.WARP_HEADLESS_BIN?.trim()
    if (!headlessBin) {
      throw new Error(
        "Warp Agent CLI 'warp' is interactive-only for this integration; " +
          'set WARP_HEADLESS_BIN to a Forge-compatible headless wrapper',
      )
    }
    return {
      bin: headlessBin,
      args: ['--cwd', cwd, '--task', task],
    }
  },
}
