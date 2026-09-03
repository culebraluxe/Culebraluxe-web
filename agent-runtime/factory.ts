import { existsSync } from 'node:fs'
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
import {
  blockedAdapterReadiness,
  commandIsInstalled,
  explicitAuthenticationReady,
  readyAdapterReadiness,
} from './readiness'
import { commitWorkerWorkspaceChanges } from '../lib/worker-workspace'

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

    let committedEvidence = evidence
    if (
      context.policy.allowCommit &&
      context.executionWorkspace &&
      !evidence.commitHash
    ) {
      const committed = await commitWorkerWorkspaceChanges(
        workspace,
        `${context.story.id}: ${context.story.title}`,
      )
      if (committed.commitHash) {
        committedEvidence = {
          ...evidence,
          commitHash: committed.commitHash,
          notes: `${evidence.notes}\n\nForge harness created candidate commit ${committed.commitHash} from Smith's worker changes.`,
        }
      }
    }

    const revoked = revokeForbiddenCommit({
      allowCommit: context.policy.allowCommit,
      workspace,
      commitHash: committedEvidence.commitHash ?? null,
      baseCommit: context.executionWorkspace?.baseCommit ?? null,
    })
    if (!revoked.violation) return committedEvidence
    return {
      ...committedEvidence,
      commitHash: null,
      notes: `${committedEvidence.notes}\n\n${revoked.violation}`,
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
    readiness: () => {
      const installed = Boolean(config.startRun) || existsSync(config.cliBin)
      if (!installed) {
        return blockedAdapterReadiness({
          installed: false,
          authentication: 'delegated',
          reason: `DeepSeek harness entrypoint not found: ${config.cliBin}`,
        })
      }
      return readyAdapterReadiness('delegated', 'DeepSeek harness is installed; authentication is delegated to DSH')
    },
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
    readiness: () => {
      const bin = process.env.WARP_HEADLESS_BIN
      const installed = commandIsInstalled(bin)
      const authentication = explicitAuthenticationReady(process.env.FORGE_WARP_AUTHENTICATED)
      if (!installed) {
        return blockedAdapterReadiness({
          installed: false,
          authentication,
          reason: 'Warp is registered but no executable WARP_HEADLESS_BIN is configured',
        })
      }
      if (authentication !== 'authenticated') {
        return blockedAdapterReadiness({
          installed: true,
          authentication,
          reason: 'Warp headless executor is installed but authentication has not been explicitly qualified',
        })
      }
      return readyAdapterReadiness('authenticated', 'Warp headless executor is installed and authentication is qualified')
    },
    factory: (deps) => new CliAgentGatewayAdapter(deps, warpProvider),
  })
  registry.registerAdapter({
    adapterId: 'gateway-openclaw',
    description: openClawProvider.description,
    capabilities: openClawProvider.capabilities,
    readiness: () => {
      const bin = process.env.OPENCLAW_BIN ?? 'openclaw'
      const installed = commandIsInstalled(bin)
      const authentication = explicitAuthenticationReady(process.env.FORGE_OPENCLAW_AUTHENTICATED)
      if (!installed) {
        return blockedAdapterReadiness({
          installed: false,
          authentication,
          reason: `OpenClaw executor is registered but '${bin}' is not installed on PATH`,
        })
      }
      if (authentication !== 'authenticated') {
        return blockedAdapterReadiness({
          installed: true,
          authentication,
          reason: 'OpenClaw is installed but authentication has not been explicitly qualified',
        })
      }
      return readyAdapterReadiness('authenticated', 'OpenClaw is installed and authentication is qualified')
    },
    factory: (deps) => new CliAgentGatewayAdapter(deps, openClawProvider),
  })

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
