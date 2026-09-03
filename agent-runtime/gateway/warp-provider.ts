import { CORE_CAPABILITIES } from '../capabilities'
import type { ForgeProviderDescriptor } from './provider'

/**
 * Warp/Oz provider boundary. Forge owns story state and acceptance; Warp only
 * receives one canonical task and returns process evidence through the runtime.
 */
export const warpProvider: ForgeProviderDescriptor = {
  id: 'warp',
  description: 'Warp Oz local agent provider',
  capabilities: CORE_CAPABILITIES,
  buildCommand: ({ cwd, task }) => ({
    bin: process.env.WARP_AGENT_BIN ?? 'oz',
    args: ['agent', 'run', '--cwd', cwd, task],
  }),
}
