// ---------------------------------------------------------------------------
// Runtime registry factory (ENG-20B) — the ONE place the scheduler/poller and
// the debug driver build the adapter registry. No duplicate execution
// mechanism: both paths resolve the same deepseek-harness adapter for the
// builder-flash profile.
// ---------------------------------------------------------------------------

import { homedir } from 'node:os'
import { join, resolve } from 'node:path'

import { AgentRuntimeRegistry } from './registry'
import {
  DeepSeekHarnessAdapter,
  type DeepSeekHarnessConfig,
} from './deepseek/deepseek-harness-adapter'
import { CORE_CAPABILITIES } from './capabilities'

/** Resolve the DeepSeek Harness CLI bin + workspace (repo root). */
export function defaultDeepSeekConfig(): DeepSeekHarnessConfig {
  return {
    cliBin:
      process.env.DSH_CLI_BIN ??
      join(homedir(), '.dsh', 'profiles', 'node_modules', '@deepseek-ai', 'dsh', 'lib', 'bin.js'),
    workspace: resolve(process.cwd()),
  }
}

/** Build the default agent runtime registry (deepseek-harness / builder-flash). */
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
    capabilities: CORE_CAPABILITIES,
  })
  return registry
}
