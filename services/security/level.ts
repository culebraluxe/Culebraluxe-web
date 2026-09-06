export type SecurityLevel =
  | 'ROOT'
  | 'BUSINESS_POWER_USER'
  | 'USER'
  | 'GUEST'

const SECURITY_LEVEL_RANK: Record<SecurityLevel, number> = {
  GUEST: 0,
  USER: 1,
  BUSINESS_POWER_USER: 2,
  ROOT: 3,
}

const ROLE_LEVELS: Readonly<Record<string, SecurityLevel>> = {
  // Current security_role codes.
  root: 'ROOT',
  business_power: 'BUSINESS_POWER_USER',
  business_power_user: 'BUSINESS_POWER_USER',
  bus_power_user: 'BUSINESS_POWER_USER',
  user: 'USER',
  ops: 'USER',
  guest: 'GUEST',

  // AUTH-02 compatibility while older assignments are retired.
  owner: 'ROOT',
  agent: 'BUSINESS_POWER_USER',
  viewer: 'USER',
  client: 'GUEST',
}

/**
 * Resolve one broad CulebraLuxe security level from the canonical
 * security_role codes already projected onto ActingUser.
 *
 * Multiple assigned roles collapse to the highest level. Unknown/empty roles
 * fail closed to GUEST rather than manufacturing additional access.
 */
export function resolveSecurityLevel(roleCodes: readonly string[]): SecurityLevel {
  let resolved: SecurityLevel = 'GUEST'

  for (const roleCode of roleCodes) {
    const candidate = ROLE_LEVELS[roleCode.trim().toLowerCase()]
    if (
      candidate &&
      SECURITY_LEVEL_RANK[candidate] > SECURITY_LEVEL_RANK[resolved]
    ) {
      resolved = candidate
    }
  }

  return resolved
}

/** Hierarchical broad-level check for Portal visibility and future policy hooks. */
export function hasSecurityLevel(
  actual: SecurityLevel,
  required: SecurityLevel,
): boolean {
  return SECURITY_LEVEL_RANK[actual] >= SECURITY_LEVEL_RANK[required]
}
