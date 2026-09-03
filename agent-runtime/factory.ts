import { homedir } from 'node:os'
import { join, resolve } from 'node:path'

import { AgentRuntimeRegistry } from './registry'
import {
  DeepSeekHarnessAdapter,
  type DeepSeekHarnessConfig,
} from './deepseek/deepseek-harness-adapter'
import { CORE_CAPABILITIES } from './capabilities'
import { ASSAY_CAPABILITIES, READ_CAPABILITIES, WRITE_CAPABILITIES } from './lanes'

export function defaultDeepSeekConfig(): DeepSeekHarnessConfig {
  return {
    cliBin:
      process.env.DSH_CLI_BIN ??
      join(homedir(), '.dsh', 'profiles', 'node_modules', '@deepseek-ai', 'dsh', 'lib', 'bin.js'),
    workspace: resolve(process.cwd()),
  }
}

export function createAgentRuntimeRegistry(
  config: DeepSeekHarnessConfig = defaultDeepSeekConfig(),
): AgentRuntimeRegistry {
  const registry = new AgentRuntimeRegistry()
  registry.registerAdapter({
    adapterId: 'deepseek-harness',
    description: 'DeepSeek Harness headless CLI adapter',
    capabilities: CORE_CAPABILITIES,
    factory: (deps) => new DeepSeekHarnessAdapter(deps, config),
  })
  registry.registerProfile({
    profile: 'builder-flash',
    adapterId: 'deepseek-harness',
    capabilities: WRITE_CAPABILITIES,
  })
  registry.registerProfile({
    profile: 'scout-volume',
    adapterId: 'deepseek-harness',
    capabilities: READ_CAPABILITIES,
  })
  registry.registerProfile({
    profile: 'verifier-mini',
    adapterId: 'deepseek-harness',
    capabilities: ASSAY_CAPABILITIES,
  })
  return registry
}
