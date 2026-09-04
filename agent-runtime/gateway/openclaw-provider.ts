import { CORE_CAPABILITIES } from '../capabilities'
import type { ForgeProviderDescriptor } from './provider'

/** OpenClaw executes the exact provider/model frozen on the Forge work item. */
export const openClawProvider: ForgeProviderDescriptor = {
  id: 'openclaw',
  description: 'OpenClaw isolated headless agent provider with explicit model selection',
  capabilities: CORE_CAPABILITIES,
  buildCommand: ({ cwd, task, modelRef }) => ({
    bin: process.env.OPENCLAW_BIN ?? 'openclaw',
    args: ['agent', 'exec', task, '--cwd', cwd, '--model', modelRef, '--json'],
  }),
}
