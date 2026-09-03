import { CORE_CAPABILITIES } from '../capabilities'
import type { ForgeProviderDescriptor } from './provider'

/**
 * OpenClaw provider boundary. `agent exec` is the isolated headless path; Forge
 * still owns lane state, commit policy, follow rules, and Assay acceptance.
 */
export const openClawProvider: ForgeProviderDescriptor = {
  id: 'openclaw',
  description: 'OpenClaw isolated headless agent provider',
  capabilities: CORE_CAPABILITIES,
  buildCommand: ({ cwd, task }) => ({
    bin: process.env.OPENCLAW_BIN ?? 'openclaw',
    args: ['agent', 'exec', task, '--cwd', cwd, '--json'],
  }),
}
