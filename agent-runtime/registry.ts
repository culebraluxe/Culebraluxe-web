import { AgentRuntimeAdapter } from './agent-runtime-adapter'
import type { AgentCapability } from './capabilities'
import type { AgentRuntimeAdapterDeps } from './agent-runtime-adapter'
import type { AdapterReadiness } from './readiness'
import { readyAdapterReadiness } from './readiness'

export interface AdapterDescriptor {
  adapterId: string
  description: string
  capabilities: AgentCapability[]
  readiness?: () => AdapterReadiness
  factory: (deps: AgentRuntimeAdapterDeps) => AgentRuntimeAdapter
}

export interface ModelProfileConfig {
  profile: string
  adapterId: string
  capabilities: AgentCapability[]
}

export class AgentRuntimeRegistry {
  private readonly adapters = new Map<string, AdapterDescriptor>()
  private readonly profiles = new Map<string, ModelProfileConfig>()

  registerAdapter(descriptor: AdapterDescriptor): void {
    if (this.adapters.has(descriptor.adapterId)) {
      throw new Error(`adapter already registered: ${descriptor.adapterId}`)
    }
    this.adapters.set(descriptor.adapterId, descriptor)
  }

  registerProfile(config: ModelProfileConfig): void {
    if (!this.adapters.has(config.adapterId)) {
      throw new Error(
        `profile '${config.profile}' references unknown adapter '${config.adapterId}'`,
      )
    }
    this.profiles.set(config.profile, config)
  }

  resolveProfile(profile: string): ModelProfileConfig {
    const config = this.profiles.get(profile)
    if (!config) throw new Error(`unknown model profile: '${profile}'`)
    return config
  }

  inspectProfileReadiness(profile: string): AdapterReadiness {
    return this.inspectAdapterReadiness(this.resolveProfile(profile).adapterId)
  }

  inspectAdapterReadiness(adapterId: string): AdapterReadiness {
    const descriptor = this.adapters.get(adapterId)
    if (!descriptor) {
      return {
        registered: false,
        installed: false,
        authentication: 'unknown',
        ready: false,
        reason: `adapter '${adapterId}' is not registered`,
      }
    }
    return descriptor.readiness?.() ?? readyAdapterReadiness('delegated', 'adapter readiness is delegated to the runtime')
  }

  private assertAdapterReady(adapterId: string): void {
    const readiness = this.inspectAdapterReadiness(adapterId)
    if (!readiness.ready) {
      throw new Error(`adapter '${adapterId}' is not ready: ${readiness.reason}`)
    }
  }

  resolveAdapter(profile: string, deps: AgentRuntimeAdapterDeps): AgentRuntimeAdapter {
    const config = this.resolveProfile(profile)
    return this.resolveAdapterById(config.adapterId, deps)
  }

  /** V6.1 execution path: frozen work-item harness identity outranks mutable team/profile maps. */
  resolveAdapterById(adapterId: string, deps: AgentRuntimeAdapterDeps): AgentRuntimeAdapter {
    const descriptor = this.adapters.get(adapterId)
    if (!descriptor) throw new Error(`unknown adapter: '${adapterId}'`)
    this.assertAdapterReady(adapterId)
    return descriptor.factory(deps)
  }

  adapterCapabilities(adapterId: string): AgentCapability[] {
    const descriptor = this.adapters.get(adapterId)
    if (!descriptor) throw new Error(`unknown adapter: '${adapterId}'`)
    return descriptor.capabilities
  }

  /** Low-level diagnostic/test access; intentionally does not imply readiness. */
  adapterForId(adapterId: string, deps: AgentRuntimeAdapterDeps): AgentRuntimeAdapter {
    const descriptor = this.adapters.get(adapterId)
    if (!descriptor) throw new Error(`unknown adapter: '${adapterId}'`)
    return descriptor.factory(deps)
  }

  hasProfile(profile: string): boolean {
    return this.profiles.has(profile)
  }

  listAdapters(): string[] {
    return [...this.adapters.keys()]
  }

  listProfiles(): string[] {
    return [...this.profiles.keys()]
  }
}
