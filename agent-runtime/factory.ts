import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, resolve } from 'node:path'

import { AgentRuntimeRegistry } from './registry'
import {
  DeepSeekHarnessAdapter,
  buildTaskText,
  type DeepSeekHarnessConfig,
} from './deepseek/deepseek-harness-adapter'
import {
  OPENCODE_PINNED_MODEL,
  OpenCodeHarnessAdapter,
  defaultOpenCodeConfig,
  openCodeModelBlocker,
  type OpenCodeHarnessConfig,
} from './opencode/opencode-harness-adapter'
import { DeterministicAssayAdapter } from './deterministic-assay-adapter'
import type {
  AgentExecutionContext,
  AgentRunEvidence,
  AgentWorkCommand,
} from './types'
import type { AgentCapability } from './capabilities'
import { CORE_CAPABILITIES } from './capabilities'
import { ASSAY_CAPABILITIES, DEFAULT_LANES } from './lanes'
import { revokeForbiddenCommit, writeBoundaryLines } from './write-policy'
import { CliAgentGatewayAdapter } from './gateway/cli-agent-adapter'
import { openClawProvider } from './gateway/openclaw-provider'
import { warpProvider } from './gateway/warp-provider'
import {
  allForgeAssignmentVariants,
  configuredForgeTeam,
  FORGE_PLAYERS,
  type ForgeHarnessId,
  type ForgeTeam,
} from './team'
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
    const workspace = context.executionWorkspace?.worktreePath ?? defaultDeepSeekConfig().workspace

    let committedEvidence = evidence
    if (context.policy.allowCommit && context.executionWorkspace && !evidence.commitHash) {
      const committed = await commitWorkerWorkspaceChanges(
        workspace,
        `${context.story.id}: ${context.story.title}`,
      )
      if (committed.commitHash) {
        committedEvidence = {
          ...evidence,
          commitHash: committed.commitHash,
          notes: `${evidence.notes}\n\nForge harness created candidate commit ${committed.commitHash} from worker changes.`,
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

class PolicyOpenCodeHarnessAdapter extends OpenCodeHarnessAdapter {
  protected override async resultExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    const evidence = await super.resultExternal(command, context)
    if (!evidence) return evidence
    const workspace = context.executionWorkspace?.worktreePath ?? defaultOpenCodeConfig().workspace

    let committedEvidence = evidence
    if (context.policy.allowCommit && context.executionWorkspace && !evidence.commitHash) {
      const committed = await commitWorkerWorkspaceChanges(
        workspace,
        `${context.story.id}: ${context.story.title}`,
      )
      if (committed.commitHash) {
        committedEvidence = {
          ...evidence,
          commitHash: committed.commitHash,
          notes: `${evidence.notes}\n\nForge harness created candidate commit ${committed.commitHash} from OpenCode's worker changes.`,
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

/** Team mapping chooses the harness. Position/role code never does. */
export function adapterIdForHarness(harnessId: ForgeHarnessId): string {
  switch (harnessId) {
    case 'forge-native': return 'deepseek-harness'
    case 'forge-assay': return 'forge-assay'
    case 'opencode': return 'opencode-harness'
    case 'openclaw': return 'gateway-openclaw'
    case 'warp-agent': return 'gateway-warp'
    case 'pi':
      throw new Error('Forge team maps a profile to Pi, but no Pi runtime adapter is configured')
  }
}

function unionCapabilities(left: AgentCapability[], right: AgentCapability[]): AgentCapability[] {
  return [...new Set([...left, ...right])]
}

export function createAgentRuntimeRegistry(
  config: DeepSeekHarnessConfig = defaultDeepSeekConfig(),
  openCodeConfig: OpenCodeHarnessConfig = defaultOpenCodeConfig(),
  team: ForgeTeam = configuredForgeTeam(),
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
    adapterId: 'forge-assay',
    description: 'Forge deterministic exact-candidate Assay executor (model-free)',
    capabilities: ASSAY_CAPABILITIES,
    readiness: () => readyAdapterReadiness('delegated', 'Forge deterministic Assay is local, model-free, and ready'),
    factory: (deps) => new DeterministicAssayAdapter(deps),
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

  registry.registerAdapter({
    adapterId: 'opencode-harness',
    description: 'OpenCode CLI inner harness adapter (opencode run)',
    capabilities: CORE_CAPABILITIES,
    readiness: () => {
      const installed = Boolean(openCodeConfig.startRun) || commandIsInstalled(openCodeConfig.cliBin)
      if (!installed) {
        return blockedAdapterReadiness({
          installed: false,
          authentication: 'delegated',
          reason: `OpenCode CLI entrypoint not found: ${openCodeConfig.cliBin}. OpenCode routing is explicit; no silent fallback to another harness.`,
        })
      }
      const modelBlocker = openCodeModelBlocker(openCodeConfig.model)
      if (modelBlocker) {
        return blockedAdapterReadiness({ installed: true, authentication: 'delegated', reason: modelBlocker })
      }
      return readyAdapterReadiness('delegated', `OpenCode CLI is installed; model pinned explicitly to ${OPENCODE_PINNED_MODEL}`)
    },
    factory: (deps) =>
      new PolicyOpenCodeHarnessAdapter(deps, openCodeConfig, (command, context) => {
        const base = buildTaskText(command, context)
        if (context.policy.allowCommit) return base
        return `${base}\n${writeBoundaryLines(context.policy).join('\n')}`
      }),
  })

  // Profiles remain convenient quality labels, but the durable work item freezes
  // the actual harness/player/model. Registry execution can therefore use the
  // frozen harness id even if the host team map changes after enqueue.
  const mapped = new Map<string, { adapterId: string; playerId: string; capabilities: AgentCapability[] }>()
  for (const { position, variant } of allForgeAssignmentVariants(team)) {
    const player = FORGE_PLAYERS[variant.playerId]
    if (!player) throw new Error(`unknown Forge player '${variant.playerId}'`)
    const adapterId = adapterIdForHarness(variant.harnessId)
    if (variant.harnessId === 'opencode') {
      const mappedModel = `${player.provider}/${player.model}`
      if (mappedModel !== OPENCODE_PINNED_MODEL) {
        throw new Error(
          `Forge team maps profile '${variant.profile}' to OpenCode player '${mappedModel}', but OpenCode is pinned to '${OPENCODE_PINNED_MODEL}'`,
        )
      }
    }
    const required = DEFAULT_LANES[position].requiredCapabilities
    const existing = mapped.get(variant.profile)
    if (existing) {
      if (existing.adapterId !== adapterId || existing.playerId !== variant.playerId) {
        throw new Error(`logical profile '${variant.profile}' has conflicting Forge team mappings`)
      }
      existing.capabilities = unionCapabilities(existing.capabilities, required)
    } else {
      mapped.set(variant.profile, { adapterId, playerId: variant.playerId, capabilities: [...required] })
    }
  }

  for (const [profile, entry] of mapped) {
    registry.registerProfile({ profile, adapterId: entry.adapterId, capabilities: entry.capabilities })
  }
  return registry
}
