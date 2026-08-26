import { createHash } from 'node:crypto'

import type { IssuedExecutionSlot } from '../lib/agreements/execution'
import {
  BROKER_SIGNATURE_CONSENT_BASIS,
  BROKER_SIGNATURE_DATE_SEMANTIC,
  type AppliedSignatureImageMimeType,
  type FormAppliedSignature,
} from '../lib/forms/applied-signature'
import type {
  TemplateDefinition,
  TemplateFieldValues,
} from '../lib/forms/template-types'
import type { QueryExecutor } from './query-executor'

// Only these template-owned brokerage fields may receive the configured owner
// signature. The field value must also identify the configured signer.
const BROKER_SIGNATURE_ALLOWLIST: Record<
  string,
  { role: string; signerField: string }
> = {
  'OFFER-01': { role: 'BUYER_BROKER', signerField: 'brokerName' },
  'LISTING-01': { role: 'SELLER_BROKER', signerField: 'brokerName' },
  'PR-PNS': { role: 'SELLER_BROKER', signerField: 'sellerBrokerName' },
  'PR-PNS-AMD': { role: 'SELLER_BROKER', signerField: 'sellerBrokerName' },
  'SHOW-INFO': { role: 'BUYER_BROKER', signerField: 'buyerBrokerName' },
  'SHOW-RPT': { role: 'BUYER_BROKER', signerField: 'agentName' },
}

type ProtectedSignatureAsset = {
  bytes: Buffer
  mimeType: AppliedSignatureImageMimeType
  checksumSha256: string
}

// `media` rows used for signatures are immutable; replacement means a new id.
// Cache by that id so debounced live preview does not repeatedly transfer the
// same protected image bytes from Postgres. A config/media-id change naturally
// invalidates the cache and a new server process starts empty.
const protectedAssetCache = new Map<string, ProtectedSignatureAsset>()

export type BrokerSignatureConfig = {
  enabled: boolean
  appUserId: string | null
  mediaId: string | null
  signerName: string | null
  licenseNumber: string | null
  configured: boolean
}

export function getBrokerSignatureConfig(
  env: NodeJS.ProcessEnv = process.env,
): BrokerSignatureConfig {
  const enabled = env.BROKER_SIGNATURE_ENABLED?.trim().toLowerCase() === 'true'
  const appUserId = env.BROKER_SIGNATURE_APP_USER_ID?.trim() || null
  const mediaId = env.BROKER_SIGNATURE_MEDIA_ID?.trim() || null
  const signerName = env.BROKER_SIGNATURE_SIGNER_NAME?.trim() || null
  const licenseNumber = env.BROKER_SIGNATURE_LICENSE_NUMBER?.trim() || null
  return {
    enabled,
    appUserId,
    mediaId,
    signerName,
    licenseNumber,
    configured: Boolean(appUserId && mediaId && signerName && licenseNumber),
  }
}

export type BrokerSignatureResolution =
  | { ok: true; signatures: FormAppliedSignature[] }
  | {
      ok: false
      outcome: 'validation_failure' | 'unauthorized'
      message: string
    }

function normalized(value: string): string {
  return value.trim().replace(/\s+/g, ' ').toLocaleLowerCase('en-US')
}

function invalidConfiguration(
  message: string,
): Extract<BrokerSignatureResolution, { ok: false }> {
  return {
    ok: false,
    outcome: 'validation_failure',
    message: `document.issue failed: broker signature configuration ${message}`,
  }
}

async function loadProtectedSignatureAsset(
  mediaId: string,
  execute: QueryExecutor,
): Promise<ProtectedSignatureAsset | string> {
  const cached = protectedAssetCache.get(mediaId)
  if (cached) return cached
  const mediaRows = await execute`
    select file_data, mime_type
    from media
    where id = ${mediaId}
      and media_type = 'image'
    limit 1
  `
  const media = mediaRows[0] as
    | { file_data?: unknown; mime_type?: unknown }
    | undefined
  if (!media) {
    return 'asset is missing from the protected media store or is not an image.'
  }
  const mimeType = String(media.mime_type ?? '').toLowerCase()
  if (mimeType !== 'image/png' && mimeType !== 'image/jpeg') {
    return 'asset must be an image/png or image/jpeg file.'
  }
  const bytes = Buffer.isBuffer(media.file_data)
    ? media.file_data
    : media.file_data instanceof Uint8Array
      ? Buffer.from(media.file_data)
      : Buffer.alloc(0)
  if (bytes.length === 0) return 'asset contains no image bytes.'
  const asset: ProtectedSignatureAsset = {
    bytes,
    mimeType: mimeType as AppliedSignatureImageMimeType,
    checksumSha256: createHash('sha256').update(bytes).digest('hex'),
  }
  protectedAssetCache.set(mediaId, asset)
  return asset
}

/**
 * Resolve Lisa/the configured owner signature at the human issuance boundary.
 * Disabled means staged rollout (unsigned behavior is preserved). Once enabled,
 * every identity/asset check fails closed with a non-secret operator message.
 */
export async function resolveBrokerSignatureForIssuance(
  input: {
    template: TemplateDefinition
    values: TemplateFieldValues
    participants: readonly IssuedExecutionSlot[]
    actorAppUserId: string | null
    issuedAt: string | null
  },
  execute: QueryExecutor,
  config: BrokerSignatureConfig = getBrokerSignatureConfig(),
): Promise<BrokerSignatureResolution> {
  if (!config.enabled) return { ok: true, signatures: [] }
  if (!config.configured) {
    return invalidConfiguration(
      'is incomplete; set the owner app-user, protected media asset, signer name, and broker license number.',
    )
  }

  const policy = BROKER_SIGNATURE_ALLOWLIST[input.template.id]
  if (!policy) return { ok: true, signatures: [] }
  if (
    !input.template.signatureGroups.some(
      (group) => group.role === policy.role && group.field === policy.signerField,
    )
  ) {
    return invalidConfiguration(
      `does not match template ${input.template.id}'s declared broker signature group.`,
    )
  }

  const declaredSigner = (input.values[policy.signerField] ?? '').trim()
  if (!declaredSigner || normalized(declaredSigner) !== normalized(config.signerName!)) {
    // The configured owner is not occupying this document's broker role. Never
    // substitute their signature for another broker or an unassigned line.
    return { ok: true, signatures: [] }
  }
  if (!input.actorAppUserId || input.actorAppUserId !== config.appUserId) {
    return {
      ok: false,
      outcome: 'unauthorized',
      message:
        'document.issue failed: the configured broker signature can only be applied during authenticated issuance by its owner.',
    }
  }
  if (!input.issuedAt || Number.isNaN(new Date(input.issuedAt).getTime())) {
    return invalidConfiguration(
      'requires the command requestedAt timestamp as its deterministic issuance date.',
    )
  }

  const userRows = await execute`
    select u.display_name, u.person_id, u.active,
      exists (
        select 1
        from app_user_role aur
        join role r on r.id = aur.role_id and r.active = true
        where aur.app_user_id = u.id and r.code = 'owner'
      ) as is_owner
    from app_user u
    where u.id = ${config.appUserId}
    limit 1
  `
  const user = userRows[0] as
    | {
        display_name?: unknown
        person_id?: unknown
        active?: unknown
        is_owner?: unknown
      }
    | undefined
  if (
    !user ||
    user.active !== true ||
    user.is_owner !== true ||
    normalized(String(user.display_name ?? '')) !== normalized(config.signerName!)
  ) {
    return {
      ok: false,
      outcome: 'unauthorized',
      message:
        'document.issue failed: the configured broker signature owner is not an active owner principal.',
    }
  }

  const roleSlots = input.participants.filter(
    (slot) =>
      slot.role === policy.role &&
      normalized(slot.name) === normalized(config.signerName!),
  )
  if (roleSlots.length > 1) {
    return invalidConfiguration(
      `resolved more than one ${policy.role} participant for ${config.signerName}.`,
    )
  }
  const slot = roleSlots[0] ?? null
  const ownerPersonId = user.person_id ? String(user.person_id) : null
  if (slot?.personId && ownerPersonId && slot.personId !== ownerPersonId) {
    return {
      ok: false,
      outcome: 'unauthorized',
      message:
        'document.issue failed: the broker participant does not match the configured signature owner.',
    }
  }
  if (input.template.id === 'PR-PNS' && !slot) {
    return invalidConfiguration(
      'could not resolve Lisa to the required SELLER_BROKER execution slot for PR-PNS.',
    )
  }

  const asset = await loadProtectedSignatureAsset(config.mediaId!, execute)
  if (typeof asset === 'string') return invalidConfiguration(asset)

  return {
    ok: true,
    signatures: [
      {
        role: policy.role,
        slotId: slot?.slotId ?? null,
        signerName: config.signerName!,
        credentialLine: `Real Estate Broker License #: ${config.licenseNumber!}`,
        signerAppUserId: config.appUserId!,
        imageBytes: asset.bytes,
        imageMimeType: asset.mimeType,
        assetMediaId: config.mediaId!,
        assetChecksumSha256: asset.checksumSha256,
        appliedAt: new Date(input.issuedAt).toISOString(),
        consentBasis: BROKER_SIGNATURE_CONSENT_BASIS,
        dateSemantic: BROKER_SIGNATURE_DATE_SEMANTIC,
      },
    ],
  }
}
