// ---------------------------------------------------------------------------
// AgentRuntimeRegistry — resolves LOGICAL model profiles to runtime adapters.
//
// The SDLC workflow only knows logical profiles (architect-pro, builder-flash,
// reviewer, local-builder). The registry owns the profile -> adapter mapping.
// Provider/model identifiers are confined to the concrete adapter layer and
// its configuration, never the command/Story Board/workflow code.
// ---------------------------------------------------------------------------

import { AgentRuntimeAdapter } from './agent-runtime-adapter'
import type { AgentCapability } from './capabilities'
import type { AgentRuntimeAdapterDeps } from './agent-runtime-adapter'

export interface AdapterDescriptor {
  adapterId: string
  description: string
  /** Capabilities this adapter can satisfy. */
  capabilities: AgentCapability[]
  factory: (deps: AgentRuntimeAdapterDeps) => AgentRuntimeAdapter
}

export interface ModelProfileConfig {
  profile: string
  /** Logical adapter id this profile resolves to. */
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
    if (!config) {
      throw new Error(`unknown model profile: '${profile}'`)
    }
    return config
  }

  resolveAdapter(profile: string, deps: AgentRuntimeAdapterDeps): AgentRuntimeAdapter {
    const config = this.resolveProfile(profile)
    const descriptor = this.adapters.get(config.adapterId)
    if (!descriptor) {
      throw new Error(`unknown adapter: '${config.adapterId}'`)
    }
    return descriptor.factory(deps)
  }

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
