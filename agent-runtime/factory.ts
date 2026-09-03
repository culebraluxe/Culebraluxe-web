import { homedir } from 'node:os'
import { join, resolve } from 'node:path'

import { AgentRuntimeRegistry } from './registry'
import {
  DeepSeekHarnessAdapter,
  buildTaskText,
  type DeepSeekHarnessConfig,
} from './deepseek/deepseek-harness-adapter'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import { CORE_CAPABILITIES } from './capabilities'
import { ASSAY_CAPABILITIES, READ_CAPABILITIES, WRITE_CAPABILITIES } from './lanes'
import { revokeForbiddenCommit, writeBoundaryLines } from './write-policy'

export function defaultDeepSeekConfig(): DeepSeekHarnessConfig {
  return {
    cliBin:
      process.env.DSH_CLI_BIN ??
      join(homedir(), '.dsh', 'profiles', 'node_modules', '@deepseek-ai', 'dsh', 'lib', 'bin.js'),
    workspace: resolve(process.cwd()),
  }
}

class PolicyDeepSeekHarnessAdapter extends DeepSeekHarnessAdapter {
  protected override async resultExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    const evidence = await super.resultExternal(command, context)
    if (!evidence) return evidence
    const workspace =
      context.executionWorkspace?.worktreePath ?? defaultDeepSeekConfig().workspace
    const revoked = revokeForbiddenCommit({
      allowCommit: context.policy.allowCommit,
      workspace,
      commitHash: evidence.commitHash ?? null,
      baseCommit: context.executionWorkspace?.baseCommit ?? null,
    })
    if (!revoked.violation) return evidence
    return {
      ...evidence,
      commitHash: null,
      notes: `${evidence.notes}\n\n${revoked.violation}`,
    }
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
    factory: (deps) =>
      new PolicyDeepSeekHarnessAdapter(deps, config, (command, context) => {
        const base = buildTaskText(command, context)
        if (context.policy.allowCommit) return base
        return `${base}\n${writeBoundaryLines(context.policy).join('\n')}`
      }),
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
