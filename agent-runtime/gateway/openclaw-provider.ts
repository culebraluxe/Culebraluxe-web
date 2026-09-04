import { CORE_CAPABILITIES } from '../capabilities'
import type { ForgeProviderDescriptor } from './provider'

/** OpenClaw executes the exact provider/model frozen on the Forge work item. */
export const openClawProvider: ForgeProviderDescriptor = {
  id: 'openclaw',
  description: 'OpenClaw isolated headless agent provider with explicit model selection',
  capabilities: CORE_CAPABILITIES,
  buildCommand: ({ cwd, task, modelRef }) => ({
    bin: process.env.OPENCLAW_BIN ?? 'openclaw',
    // Legacy provider unit fixtures may omit modelRef. Production V6.1 runtime
    // always supplies it from the frozen work item before this seam.
    args: modelRef
      ? ['agent', 'exec', task, '--cwd', cwd, '--model', modelRef, '--json']
      : ['agent', 'exec', task, '--cwd', cwd, '--json'],
  }),
}
