export type RelationScope =
  | 'person_person'
  | 'person_firm'
  | 'person_property'
  | 'firm_property'
  | 'contract_person'
  | 'contract_firm'
  | 'contract_property'

export type RelationRoleDefinition = {
  id?: string
  scope: RelationScope
  code: string
  name: string
  aliases?: readonly string[]
}

export type ResolvedRelationRole = {
  scope: RelationScope
  code: string
  name: string
}

export function normalizeRelationRoleAlias(value: string): string {
  return value.trim().toLowerCase().replace(/\s+/g, ' ')
}

/**
 * Small middle-tier primitive for canonical relationship vocabulary.
 *
 * Role is data. This catalog resolves boundary language to one scoped code; it
 * does not own Person/Firm/Property/Contract truth and it is deliberately not a
 * public RelationService capable of bypassing owning-service invariants.
 */
export class RelationRoleCatalog {
  private readonly byScopeAndAlias = new Map<string, ResolvedRelationRole>()

  constructor(definitions: readonly RelationRoleDefinition[]) {
    for (const definition of definitions) {
      const role: ResolvedRelationRole = {
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
            `Ambiguous relation role alias "${alias}" in ${definition.scope}: ${existing.code} vs ${role.code}`,
          )
        }
        this.byScopeAndAlias.set(key, role)
      }
    }
  }

  resolve(scope: RelationScope, input: string): ResolvedRelationRole | null {
    return this.byScopeAndAlias.get(this.key(scope, input)) ?? null
  }

  require(scope: RelationScope, input: string): ResolvedRelationRole {
    const role = this.resolve(scope, input)
    if (!role) throw new Error(`Unknown relation role "${input}" for scope ${scope}`)
    return role
  }

  private key(scope: RelationScope, input: string): string {
    return `${scope}:${normalizeRelationRoleAlias(input)}`
  }
}
