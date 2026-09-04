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
import { ASSAY_CAPABILITIES, WRITE_CAPABILITIES, DEFAULT_LANES } from './lanes'
import { revokeForbiddenCommit, writeBoundaryLines } from './write-policy'
import { CliAgentGatewayAdapter } from './gateway/cli-agent-adapter'
import { openClawProvider } from './gateway/openclaw-provider'
import { warpProvider } from './gateway/warp-provider'
import type { ForgeHarnessId } from './team'
import {
  allForgeAssignmentVariants,
  DEFAULT_FORGE_TEAM,
  FORGE_PLAYERS,
  type ForgeAssignmentVariant,
  type ForgeTeam,
} from './team'
import {
  blockedAdapterReadiness,
  commandIsInstalled,
  explicitAuthenticationReady,
  readyAdapterReadiness,
  type AdapterReadiness,
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

/**
 * Factory finding #1 — explicit builder-flash harness override.
 *
 * Previously read inside the registration loop from
 * `process.env.FORGE_PROVIDER_BUILDER_FLASH`, which made the same code behave
 * differently per host and could never be observed by team.ts selectors.
 * Overrides are now parsed once at the call boundary into this explicit
 * record and applied as a named team variant before registration.
 */
export type BuilderFlashOverride = 'deepseek' | 'forge-native' | 'opencode' | 'openclaw' | 'warp' | 'warp-agent'

export function parseBuilderFlashOverride(
  value: string | null | undefined,
): BuilderFlashOverride | null {
  const normalized = (value ?? '').trim().toLowerCase()
  if (!normalized) return null
  if (
    normalized === 'deepseek' ||
    normalized === 'forge-native' ||
    normalized === 'opencode' ||
    normalized === 'openclaw' ||
    normalized === 'warp' ||
    normalized === 'warp-agent'
  ) {
    return normalized
  }
  throw new Error(
    `unknown FORGE_PROVIDER_BUILDER_FLASH '${value}': expected deepseek|forge-native|opencode|openclaw|warp|warp-agent`,
  )
}

/**
 * Return a team copy with the builder-flash Smith default (plus night default,
 * which shares the profile) rerouted to the requested harness. Upgrade and
 * emergency grades keep their mapped harness unless they share the
 * overridden profile, so grade escalation never silently lands on a
 * different harness than the base grade.
 */
export function forgeTeamWithBuilderFlashOverride(
  team: ForgeTeam,
  override: BuilderFlashOverride | null | undefined,
): ForgeTeam {
  if (!override) return team
  const apply = (variant: ForgeAssignmentVariant): ForgeAssignmentVariant => {
    if (variant.profile !== 'builder-flash') return variant
    if (override === 'deepseek' || override === 'forge-native') {
      return { ...variant, harnessId: 'forge-native', playerId: 'deepseek-flash' }
    }
    if (override === 'opencode') {
      return { ...variant, harnessId: 'opencode', playerId: 'deepseek-flash' }
    }
    if (override === 'openclaw') {
      return { ...variant, harnessId: 'openclaw' }
    }
    return { ...variant, harnessId: 'warp-agent' }
  }
  const remap = (
    position: keyof ForgeTeam['assignments'],
  ): ForgeTeam['assignments'][typeof position] => {
    const base = team.assignments[position]
    return {
      ...base,
      ...apply(base),
      ...(base.upgrade ? { upgrade: apply(base.upgrade) } : {}),
      ...(base.emergency ? { emergency: apply(base.emergency) } : {}),
    }
  }
  return {
    ...team,
    assignments: {
      ...team.assignments,
      smith: remap('smith'),
      night: remap('night'),
    },
  }
}

/**
 * Factory finding #2 — single shared exact-candidate commit seam.
 *
 * Both policy harness adapters previously duplicated this commit + revoke
 * block with only the evidence noun differing. Drift here risks losing a
 * candidate or leaking a forbidden commit on one harness, so both subclasses
 * now call this helper. No behavior change.
 */
async function commitAndRevokePolicyEvidence(input: {
  evidence: AgentRunEvidence
  command: AgentWorkCommand
  context: AgentExecutionContext
  workspace: string
  sourceLabel: string
}): Promise<AgentRunEvidence> {
  const { evidence, command, context, workspace, sourceLabel } = input
  void command
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
        notes: `${evidence.notes}\n\nForge harness created candidate commit ${committed.commitHash} from ${sourceLabel} worker changes.`,
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

class PolicyDeepSeekHarnessAdapter extends DeepSeekHarnessAdapter {
  protected override async resultExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    const evidence = await super.resultExternal(command, context)
    if (!evidence) return evidence
    const workspace =
      context.executionWorkspace?.worktreePath ?? defaultDeepSeekConfig().workspace
    return commitAndRevokePolicyEvidence({
      evidence,
      command,
      context,
      workspace,
      sourceLabel: 'worker',
    })
  }
}

class PolicyOpenCodeHarnessAdapter extends OpenCodeHarnessAdapter {
  protected override async resultExternal(
    command: AgentWorkCommand,
    context: AgentExecutionContext,
  ): Promise<AgentRunEvidence | null> {
    const evidence = await super.resultExternal(command, context)
    if (!evidence) return evidence
    const workspace =
      context.executionWorkspace?.worktreePath ?? defaultOpenCodeConfig().workspace
    return commitAndRevokePolicyEvidence({
      evidence,
      command,
      context,
      workspace,
      sourceLabel: "OpenCode's",
    })
  }
}

/** Team mapping chooses the harness. Position/role code never does. */
function adapterIdForHarness(harnessId: ForgeHarnessId): string | null {
  switch (harnessId) {
    case 'forge-native':
      return 'deepseek-harness'
    case 'forge-assay':
      return 'forge-assay'
    case 'opencode':
      return 'opencode-harness'
    case 'openclaw':
      return 'gateway-openclaw'
    case 'warp-agent':
      return 'gateway-warp'
    // Factory finding #3: Pi is a reserved connection point with no runtime
    // adapter. Register a blocked placeholder instead of throwing at boot so
    // one unused mapping cannot kill scout/lead/assay hydration.
    case 'pi':
      return null
  }
}

function unionCapabilities(
  left: AgentCapability[],
  right: AgentCapability[],
): AgentCapability[] {
  return [...new Set([...left, ...right])]
}

/**
 * Factory finding #4 — uniform model-harness readiness contract.
 *
 * DeepSeek previously used existsSync (no exec-bit check) with unconditional
 * delegated auth and treated any startRun injection as installed. Every
 * model harness now uses the same probe: commandIsInstalled (X_OK aware).
 * An injected (non-native) startRun counts as hosted qualification in tests.
 */
function deepSeekHarnessReadiness(config: DeepSeekHarnessConfig): AdapterReadiness {
  if (isInjectedStartRun(config.startRun)) {
    return readyAdapterReadiness(
      'delegated',
      'DeepSeek harness start function is injected (test/hosted qualification); authentication is delegated to DSH',
    )
  }
  const installed = commandIsInstalled(config.cliBin)
  if (!installed) {
    return blockedAdapterReadiness({
      installed: false,
      authentication: 'delegated',
      reason: `DeepSeek Harness CLI entrypoint not found or not executable: ${config.cliBin}. Forge routing is explicit; no silent fallback to another harness.`,
    })
  }
  return readyAdapterReadiness(
    'delegated',
    'DeepSeek Harness CLI is installed and executable; authentication is delegated to DSH',
  )
}

function isInjectedStartRun(value: unknown): boolean {
  if (typeof value !== 'function') return false
  const source = Function.prototype.toString.call(value)
  return !/native code/.test(source)
}

/**
 * Factory finding #7 — explicit workspace injection.
 *
 * defaultDeepSeekConfig() bakes process.cwd() at call time, which flaps under
 * launchd vs interactive shell vs worktree CWD. Callers that know the isolated
 * worktree pass it explicitly; the homedir/CWD fallback stays for local CLI.
 */
export function resolveDeepSeekWorkspace(workspace?: string | null): string {
  const explicit = (workspace ?? '').trim()
  if (explicit) return explicit
  return defaultDeepSeekConfig().workspace
}

export type CreateAgentRuntimeRegistryOptions = {
  deepseek?: DeepSeekHarnessConfig
  opencode?: OpenCodeHarnessConfig
  /** Explicit team (defaults to DEFAULT_FORGE_TEAM). */
  team?: ForgeTeam
  /**
   * Explicit builder-flash harness override (parsed once at the call boundary
   * via parseBuilderFlashOverride). Applies to every variant sharing the
   * profile, including grade upgrades, so escalation never lands on a silent
   * different harness.
   */
  builderFlashOverride?: BuilderFlashOverride | null
  /** Explicit DeepSeek workspace (isolated worktree); falls back to CWD config. */
  deepseekWorkspace?: string | null
}

export function createAgentRuntimeRegistry(
  config: DeepSeekHarnessConfig | CreateAgentRuntimeRegistryOptions = defaultDeepSeekConfig(),
  openCodeConfig: OpenCodeHarnessConfig = defaultOpenCodeConfig(),
): AgentRuntimeRegistry {
  const opts: CreateAgentRuntimeRegistryOptions =
    config && typeof config === 'object' && ('deepseek' in config || 'opencode' in config || 'team' in config || 'builderFlashOverride' in config || 'deepseekWorkspace' in config)
      ? (config as CreateAgentRuntimeRegistryOptions)
      : { deepseek: config as DeepSeekHarnessConfig, opencode: openCodeConfig }
  const deepseekConfig = opts.deepseek ?? (config as DeepSeekHarnessConfig)
  const resolvedOpenCodeConfig = opts.opencode ?? openCodeConfig
  const team = forgeTeamWithBuilderFlashOverride(
    opts.team ?? DEFAULT_FORGE_TEAM,
    opts.builderFlashOverride ?? parseBuilderFlashOverride(process.env.FORGE_PROVIDER_BUILDER_FLASH ?? null),
  )
  // Factory finding #7: prefer the explicitly injected isolated workspace over
  // the CWD-baked default when the caller knows it.
  const effectiveDeepseekConfig: DeepSeekHarnessConfig = opts.deepseekWorkspace?.trim()
    ? { ...deepseekConfig, workspace: opts.deepseekWorkspace.trim() }
    : deepseekConfig
  // Spend vision: every forge-native profile gets its team-mapped exact model.
  // One shared deepseek-harness adapter cannot carry per-profile models, so
  // the factory registers one model-pinned adapter per distinct native model
  // and routes each profile to its own. Profiles sharing a model share the
  // adapter; a profile whose player model is missing fails closed here.
  const nativeModels = new Map<string, string>()
  for (const { variant } of allForgeAssignmentVariants(team)) {
    if (variant.harnessId !== 'forge-native') continue
    const player = FORGE_PLAYERS[variant.playerId]
    if (!player) throw new Error(`unknown Forge player '${variant.playerId}'`)
    const model = `${player.provider}/${player.model}`
    nativeModels.set(model, variant.profile)
  }
  const nativeAdapterIdForModel = (model: string): string =>
    `deepseek-harness:${model}`
  const registry = new AgentRuntimeRegistry()

  // Factory finding #3: Pi has no runtime adapter. Register a blocked
  // placeholder so an unused Pi mapping cannot crash hydration for live lanes.
  registry.registerAdapter({
    adapterId: 'pi-harness',
    description: 'Reserved Pi harness connection point (no runtime adapter configured)',
    capabilities: [],
    readiness: () =>
      blockedAdapterReadiness({
        installed: false,
        authentication: 'unknown',
        reason: 'Forge team maps a profile to Pi, but no Pi runtime adapter is configured',
      }),
    factory: () => {
      throw new Error('Forge team maps a profile to Pi, but no Pi runtime adapter is configured')
    },
  })

  for (const model of nativeModels.keys()) {
    const modelConfig: DeepSeekHarnessConfig = { ...effectiveDeepseekConfig, model }
    registry.registerAdapter({
      adapterId: nativeAdapterIdForModel(model),
      description: `DeepSeek Harness headless CLI adapter (model ${model})`,
      capabilities: WRITE_CAPABILITIES,
      readiness: () => deepSeekHarnessReadiness(modelConfig),
      factory: (deps) =>
        new PolicyDeepSeekHarnessAdapter(deps, modelConfig, (command, context) => {
          const base = buildTaskText(command, context)
          if (context.policy.allowCommit) return base
          return `${base}\n${writeBoundaryLines(context.policy).join('\n')}`
        }),
    })
  }

  // Legacy shared adapter id kept for direct callers that construct with an
  // explicit config (tests, dogfood drivers). It carries whatever model the
  // caller configured, or none (no enforcement).
  registry.registerAdapter({
    adapterId: 'deepseek-harness',
    description: 'DeepSeek Harness headless CLI adapter',
    capabilities: WRITE_CAPABILITIES,
    readiness: () => deepSeekHarnessReadiness(effectiveDeepseekConfig),
    factory: (deps) =>
      new PolicyDeepSeekHarnessAdapter(deps, effectiveDeepseekConfig, (command, context) => {
        const base = buildTaskText(command, context)
        if (context.policy.allowCommit) return base
        return `${base}\n${writeBoundaryLines(context.policy).join('\n')}`
      }),
  })

  registry.registerAdapter({
    adapterId: 'forge-assay',
    description: 'Forge deterministic exact-candidate Assay executor (model-free)',
    capabilities: ASSAY_CAPABILITIES,
    readiness: () =>
      readyAdapterReadiness(
        'delegated',
        'Forge deterministic Assay is local, model-free, and ready',
      ),
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
    capabilities: WRITE_CAPABILITIES,
    readiness: () => {
      // Uniform with DeepSeek: an injected (non-native) startRun qualifies the
      // harness in tests; otherwise the CLI must be installed and executable.
      const installed =
        isInjectedStartRun(resolvedOpenCodeConfig.startRun) ||
        commandIsInstalled(resolvedOpenCodeConfig.cliBin)
      if (!installed) {
        return blockedAdapterReadiness({
          installed: false,
          authentication: 'delegated',
          reason: `OpenCode CLI entrypoint not found: ${resolvedOpenCodeConfig.cliBin}. OpenCode routing is explicit; no silent fallback to another harness.`,
        })
      }
      const modelBlocker = openCodeModelBlocker(resolvedOpenCodeConfig.model)
      if (modelBlocker) {
        return blockedAdapterReadiness({
          installed: true,
          authentication: 'delegated',
          reason: modelBlocker,
        })
      }
      return readyAdapterReadiness(
        'delegated',
        `OpenCode CLI is installed; model pinned explicitly to ${OPENCODE_PINNED_MODEL}`,
      )
    },
    factory: (deps) =>
      new PolicyOpenCodeHarnessAdapter(deps, resolvedOpenCodeConfig, (command, context) => {
        const base = buildTaskText(command, context)
        if (context.policy.allowCommit) return base
        return `${base}\n${writeBoundaryLines(context.policy).join('\n')}`
      }),
  })

  // Build logical profile -> runtime adapter registrations from ONE team map.
  // A shared profile must map to the same player+harness everywhere it is used.
  // Factory finding #6: a profile reused across lanes with different required
  // capabilities fails closed instead of silently widening privilege.
  const mapped = new Map<
    string,
    { adapterId: string; playerId: string; capabilities: AgentCapability[] }
  >()

  for (const { position, variant } of allForgeAssignmentVariants(team)) {
    const player = FORGE_PLAYERS[variant.playerId]
    if (!player) throw new Error(`unknown Forge player '${variant.playerId}'`)

    // Spend vision: forge-native profiles route to their model-pinned adapter
    // so the team-mapped exact model is enforced per run. Every other harness
    // keeps the shared adapter id.
    let adapterId = adapterIdForHarness(variant.harnessId)
    if (variant.harnessId === 'forge-native') {
      adapterId = nativeAdapterIdForModel(`${player.provider}/${player.model}`)
    }
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
        throw new Error(
          `logical profile '${variant.profile}' has conflicting Forge team mappings`,
        )
      }
      // Same player+harness but different lane capabilities: the profile would
      // silently inherit the union (privilege widening). Fail closed unless the
      // required sets are identical.
      const sameCaps =
        existing.capabilities.length === unionCapabilities(existing.capabilities, required).length &&
        required.every((c) => existing.capabilities.includes(c))
      if (!sameCaps) {
        throw new Error(
          `logical profile '${variant.profile}' is shared across lanes with different required capabilities — give each lane its own profile instead of widening privilege`,
        )
      }
    } else {
      mapped.set(variant.profile, {
        adapterId: adapterId ?? 'pi-harness',
        playerId: variant.playerId,
        capabilities: [...required],
      })
    }
  }

  for (const [profile, configEntry] of mapped) {
    registry.registerProfile({
      profile,
      adapterId: configEntry.adapterId,
      capabilities: configEntry.capabilities,
    })
  }

  return registry
}

/**
 * Factory finding #8 — shared process registry.
 *
 * hydrateBareReadyItems previously constructed a fresh registry per wake (plus
 * another inside gateSmithEnvelope), repeating PATH scans and existsSync
 * probes. The shared registry is memoized per process; pass an explicit
 * registry in tests or when configuration changes.
 */
let sharedRegistry: AgentRuntimeRegistry | null = null

export function sharedAgentRuntimeRegistry(): AgentRuntimeRegistry {
  if (!sharedRegistry) sharedRegistry = createAgentRuntimeRegistry()
  return sharedRegistry
}

export function resetSharedAgentRuntimeRegistry(): void {
  sharedRegistry = null
}
