import type { Interaction } from './crm-types'
import type {
  IdentityMatch,
  NormalizedIdentityHint,
  NormalizedInboundEvent,
} from './crm-intake-types'

export type PersonRole = 'buyer' | 'seller' | 'both' | 'unclassified'

export interface PersonCreationPolicy {
  allowCreation: boolean
  role: PersonRole
}

export interface IdentityClaim {
  kind: NormalizedIdentityHint['kind']
  normalizedValue: string
  sourceSystem?: string
  isPrimary: boolean
}

export interface IdentityOwnership extends IdentityMatch {
  archived: boolean
}

export interface AtomicPersonClaimInput {
  personId: string
  displayName: string
  role: PersonRole
  identities: IdentityClaim[]
}

export interface PersonCreationRepositories {
  findInteractionBySourceIdentity(
    sourceSystem: string,
    sourceExternalId: string,
  ): Promise<Interaction | null>
  personExists(personId: string): Promise<boolean>
  findIdentityMatch(
    hint: NormalizedIdentityHint,
  ): Promise<IdentityMatch | null>
  findIdentityOwnership(
    hint: NormalizedIdentityHint,
  ): Promise<IdentityOwnership | null>
  createPersonWithIdentities(input: AtomicPersonClaimInput): Promise<void>
}

export type PersonCreationStatus =
  | 'created'
  | 'resolved_existing'
  | 'duplicate'
  | 'conflicting'
  | 'resolution_required'
  | 'rejected'

export interface PersonCreationResult {
  status: PersonCreationStatus
  normalizedEvent: NormalizedInboundEvent
  personId?: string
  existingInteractionId?: string
  displayName?: string
  displayNameSource?: 'hint' | 'email' | 'phone'
  claimedIdentities: IdentityClaim[]
  unclaimedIdentities: NormalizedIdentityHint[]
  reason?:
    | 'creation_not_allowed'
    | 'insufficient_identity_evidence'
    | 'explicit_person_not_found'
    | 'identity_conflict'
    | 'archived_identity_owner'
    | 'race_ownership_unresolved'
    | 'repository_failure'
}

