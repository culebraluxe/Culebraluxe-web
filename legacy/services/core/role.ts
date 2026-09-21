export type RoleScope =
  | 'person_person'
  | 'person_firm'
  | 'person_property'
  | 'firm_property'
  | 'contract_person'
  | 'contract_firm'
  | 'contract_property'

export type RoleDefinition = {
  id?: string
  scope: RoleScope
  code: string
  name: string
  aliases?: readonly string[]
}

export type ResolvedRole = {
  scope: RoleScope
  code: string
  name: string
}

export function normalizeRoleAlias(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, ' ')
}

/**
 * Small middle-tier primitive for canonical business-role vocabulary.
 *
 * Role is data. This catalog resolves boundary language to one scoped code; it
 * does not own Person/Firm/Property/Contract truth and it is deliberately not a
 * public RoleService capable of bypassing owning-service invariants.
 */
export class RoleCatalog {
  private readonly byScopeAndAlias = new Map<string, ResolvedRole>()

  constructor(definitions: readonly RoleDefinition[]) {
    for (const definition of definitions) {
      const role: ResolvedRole = {
        scope: definition.scope,
        code: definition.code.trim().toUpperCase(),
        name: definition.name,
      }
      const aliases = new Set([
        definition.code,
        definition.name,
        ...(definition.aliases ?? []),
      ])
      for (const alias of aliases) {
        const key = this.key(definition.scope, alias)
        const existing = this.byScopeAndAlias.get(key)
        if (existing && existing.code !== role.code) {
          throw new Error(
            `Ambiguous role alias "${alias}" in ${definition.scope}: ${existing.code} vs ${role.code}`,
          )
        }
        this.byScopeAndAlias.set(key, role)
      }
    }
  }

  resolve(scope: RoleScope, input: string): ResolvedRole | null {
    return this.byScopeAndAlias.get(this.key(scope, input)) ?? null
  }

  require(scope: RoleScope, input: string): ResolvedRole {
    const role = this.resolve(scope, input)
    if (!role) throw new Error(`Unknown role "${input}" for scope ${scope}`)
    return role
  }

  private key(scope: RoleScope, input: string): string {
    return `${scope}:${normalizeRoleAlias(input)}`
  }
}
