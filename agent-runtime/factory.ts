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
import { CliAgentGatewayAdapter } from './gateway/cli-agent-adapter'
import { openClawProvider } from './gateway/openclaw-provider'
import {
  resolveForgeExecutionProviderForProfile,
  type ForgeExecutionProvider,
} from './gateway/provider'
import { warpProvider } from './gateway/warp-provider'
import { DEFAULT_FORGE_TEAM, type ForgePosition } from './team'

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

function adapterIdForProvider(provider: ForgeExecutionProvider): string {
  return provider === 'deepseek' ? 'deepseek-harness' : `gateway-${provider}`
}

const POSITION_CAPABILITIES: Record<ForgePosition, typeof READ_CAPABILITIES> = {
  scout: READ_CAPABILITIES,
  architect: READ_CAPABILITIES,
  smith: WRITE_CAPABILITIES,
  assay: ASSAY_CAPABILITIES,
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
  registry.registerAdapter({
    adapterId: 'gateway-warp',
    description: warpProvider.description,
    capabilities: warpProvider.capabilities,
    factory: (deps) => new CliAgentGatewayAdapter(deps, warpProvider),
  })
  registry.registerAdapter({
    adapterId: 'gateway-openclaw',
    description: openClawProvider.description,
    capabilities: openClawProvider.capabilities,
    factory: (deps) => new CliAgentGatewayAdapter(deps, openClawProvider),
  })

  // V4-05: the active team owns which logical profile fills each core position.
  // Provider routing remains below that boundary, so V3 lane semantics do not
  // learn vendor/model names. Architect is registered now even though A1 still
  // does not auto-queue the Architect lane.
  for (const position of ['scout', 'architect', 'smith', 'assay'] as ForgePosition[]) {
    const assignment = DEFAULT_FORGE_TEAM.assignments[position]
    const provider = resolveForgeExecutionProviderForProfile(assignment.profile)
    registry.registerProfile({
      profile: assignment.profile,
      adapterId: adapterIdForProvider(provider),
      capabilities: POSITION_CAPABILITIES[position],
    })
  }
  return registry
}
