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
import { resolveSecurityLevel } from '../services/security/level'
import type { QueryExecutor } from './query-executor'

let defaultReadExecutor: QueryExecutor | null = null

async function readExecutor(): Promise<QueryExecutor> {
  if (!defaultReadExecutor) {
    const client = await import('./client')
    defaultReadExecutor = client.sql
  }
  return defaultReadExecutor
}

/** Resolve the locally-applied broker's canonical email for completion CC. */
export async function getAppliedBrokerCompletionEmail(
  appliedSignatures: unknown,
  execute?: QueryExecutor,
): Promise<string | null> {
  if (!Array.isArray(appliedSignatures)) return null
  const appUserIds = [
    ...new Set(
      appliedSignatures
        .map((entry) =>
          entry && typeof entry === 'object' &&
          typeof (entry as Record<string, unknown>).signerAppUserId === 'string'
            ? String((entry as Record<string, unknown>).signerAppUserId).trim()
            : '',
        )
        .filter(Boolean),
    ),
  ]
  if (appUserIds.length !== 1) return null
  const q = execute ?? (await readExecutor())
  const rows = await q`
    select email
    from app_user
    where id = ${appUserIds[0]}
      and active = true
    limit 1
  `
  const email = rows[0]?.email
  return typeof email === 'string' && email.trim() ? email.trim() : null
}

// CulebraLuxe is currently a single-broker execution model. These are durable
// logical identities, not environment-specific UUIDs. PROD/DEV may have
// different app_user/media UUIDs; the resolver finds the correct local rows.
const DEFAULT_BROKER_SIGNER_NAME = 'Lisa Penfield'
const DEFAULT_BROKER_LICENSE_NUMBER = 'C-9931'
const DEFAULT_BROKER_SIGNATURE_PURPOSE = 'broker_signature:lisa_penfield'

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

const protectedAssetCache = new Map<string, ProtectedSignatureAsset>()

export type BrokerSignatureConfig = {
  enabled: boolean
  appUserId: string | null
  mediaId: string | null
  signerName: string
  licenseNumber: string
  configured: boolean
}

/**
 * Environment values remain supported as explicit overrides, but the normal
 * CulebraLuxe path no longer depends on manually remembered PROD UUIDs. The
 * canonical broker and protected signature are resolved from durable database
 * identity when appUserId/mediaId are absent.
 */
export function getBrokerSignatureConfig(
  env: NodeJS.ProcessEnv = process.env,
): BrokerSignatureConfig {
  const enabledValue = env.BROKER_SIGNATURE_ENABLED?.trim().toLowerCase()
  const enabled = enabledValue === undefined ? true : enabledValue !== 'false'
  const appUserId = env.BROKER_SIGNATURE_APP_USER_ID?.trim() || null
  const mediaId = env.BROKER_SIGNATURE_MEDIA_ID?.trim() || null
  const signerName =
    env.BROKER_SIGNATURE_SIGNER_NAME?.trim() || DEFAULT_BROKER_SIGNER_NAME
  const licenseNumber =
    env.BROKER_SIGNATURE_LICENSE_NUMBER?.trim() ||
    DEFAULT_BROKER_LICENSE_NUMBER
  return {
    enabled,
    appUserId,
    mediaId,
    signerName,
    licenseNumber,
    configured: Boolean(signerName && licenseNumber),
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

async function resolveBrokerRows(
  config: BrokerSignatureConfig,
  execute: QueryExecutor,
): Promise<
  | {
      appUserId: string
      mediaId: string
      displayName: string
      email: string
      personId: string | null
    }
  | string
> {
  const userRows = config.appUserId
    ? await execute`
        select id, display_name, email, person_id, active
        from app_user
        where id = ${config.appUserId}
        limit 2
      `
    : await execute`
        select id, display_name, email, person_id, active
        from app_user
        where active = true
          and lower(display_name) = lower(${config.signerName})
        order by id
        limit 2
      `
  if (userRows.length !== 1) {
    return 'must resolve exactly one active Lisa Penfield app user.'
  }
  const user = userRows[0] as Record<string, unknown>
  if (
    user.active !== true ||
    normalized(String(user.display_name ?? '')) !== normalized(config.signerName)
  ) {
    return 'resolved broker identity does not match the active Lisa Penfield principal.'
  }

  const mediaRows = config.mediaId
    ? await execute`
        select id
        from media
        where id = ${config.mediaId}
          and media_type = 'image'
          and mime_type in ('image/png', 'image/jpeg')
        limit 2
      `
    : await execute`
        select id
        from media
        where alt_text = ${DEFAULT_BROKER_SIGNATURE_PURPOSE}
          and media_type = 'image'
          and mime_type in ('image/png', 'image/jpeg')
        order by created_at desc
        limit 2
      `
  if (mediaRows.length !== 1) {
    return 'must resolve exactly one protected Lisa Penfield signature image.'
  }

  return {
    appUserId: String(user.id),
    mediaId: String(mediaRows[0]?.id ?? ''),
    displayName: String(user.display_name),
    email: String(user.email ?? ''),
    personId: user.person_id ? String(user.person_id) : null,
  }
}

/**
 * ROOT is the deliberate break-glass/delegation level in the four-level
 * CulebraLuxe security model. A ROOT actor may execute a broker-owned operation
 * on Lisa's behalf. The command/audit receipt still records the real actor;
 * the applied signature evidence still records Lisa as the signer identity.
 */
async function actorMayApplyBrokerSignature(
  actorAppUserId: string,
  brokerAppUserId: string,
  execute: QueryExecutor,
): Promise<boolean> {
  if (actorAppUserId === brokerAppUserId) return true

  const tableRows = await execute`
    select to_regclass('public.security_role')::text as security_role
  `
  const usesSecurityRole = Boolean(tableRows[0]?.security_role)
  const rows = usesSecurityRole
    ? await execute`
        select r.code
        from app_user u
        join app_user_role aur on aur.app_user_id = u.id
        join security_role r on r.id = aur.role_id and r.active = true
        where u.id = ${actorAppUserId}
          and u.active = true
        order by r.code
      `
    : await execute`
        select r.code
        from app_user u
        join app_user_role aur on aur.app_user_id = u.id
        join role r on r.id = aur.role_id and r.active = true
        where u.id = ${actorAppUserId}
          and u.active = true
        order by r.code
      `
  const roleCodes = rows
    .map((row) => String((row as { code?: unknown }).code ?? '').trim())
    .filter(Boolean)
  return resolveSecurityLevel(roleCodes) === 'ROOT'
}

async function loadProtectedSignatureAsset(
  mediaId: string,
  execute: QueryExecutor,
): Promise<ProtectedSignatureAsset | string> {
  const cached = protectedAssetCache.get(mediaId)
  if (cached) return cached
  const mediaRows = await execute`
    select file_data, mime_type, alt_text, caption
    from media
    where id = ${mediaId}
      and media_type = 'image'
    limit 1
  `
  const media = mediaRows[0] as
    | {
        file_data?: unknown
        mime_type?: unknown
        alt_text?: unknown
        caption?: unknown
      }
    | undefined
  if (!media) {
    return 'asset is missing from the protected media store or is not an image.'
  }
  const mimeType = String(media.mime_type ?? '').toLowerCase()
  if (mimeType !== 'image/png' && mimeType !== 'image/jpeg') {
    return 'asset must be an image/png or image/jpeg file.'
  }
  if (String(media.alt_text ?? '') !== DEFAULT_BROKER_SIGNATURE_PURPOSE) {
    return 'asset does not carry the canonical Lisa Penfield signature purpose.'
  }
  const bytes = Buffer.isBuffer(media.file_data)
    ? media.file_data
    : media.file_data instanceof Uint8Array
      ? Buffer.from(media.file_data)
      : Buffer.alloc(0)
  if (bytes.length === 0) return 'asset contains no image bytes.'
  const checksumSha256 = createHash('sha256').update(bytes).digest('hex')
  const recordedChecksum = String(media.caption ?? '').match(
    /^sha256:([0-9a-f]{64})$/i,
  )?.[1]
  if (recordedChecksum && recordedChecksum.toLowerCase() !== checksumSha256) {
    return 'asset checksum does not match the protected media record.'
  }
  const asset: ProtectedSignatureAsset = {
    bytes,
    mimeType: mimeType as AppliedSignatureImageMimeType,
    checksumSha256,
  }
  protectedAssetCache.set(mediaId, asset)
  return asset
}

/**
 * Resolve Lisa's standing local pre-signature. Draft preview may render without
 * an immutable execution slot; issuance must still require the slot so Lisa is
 * locally satisfied before the external BoldSign envelope is constructed.
 */
export async function resolveBrokerSignatureForIssuance(
  input: {
    template: TemplateDefinition
    values: TemplateFieldValues
    participants: readonly IssuedExecutionSlot[]
    actorAppUserId: string | null
    issuedAt: string | null
    requireExecutionSlot?: boolean
  },
  execute: QueryExecutor,
  config: BrokerSignatureConfig = getBrokerSignatureConfig(),
): Promise<BrokerSignatureResolution> {
  if (!config.enabled) return { ok: true, signatures: [] }
  if (!config.configured) {
    return invalidConfiguration('is incomplete; signer name and license are required.')
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
  if (!declaredSigner || normalized(declaredSigner) !== normalized(config.signerName)) {
    // Lisa is not occupying this document's broker role. Never substitute her
    // signature for another broker or an unassigned line.
    return { ok: true, signatures: [] }
  }
  if (!input.actorAppUserId) {
    return {
      ok: false,
      outcome: 'unauthorized',
      message:
        'document.issue failed: an authenticated application user is required to apply the broker pre-signature.',
    }
  }
  if (!input.issuedAt || Number.isNaN(new Date(input.issuedAt).getTime())) {
    return invalidConfiguration(
      'requires the command requestedAt timestamp as its deterministic issuance date.',
    )
  }

  const resolved = await resolveBrokerRows(config, execute)
  if (typeof resolved === 'string') return invalidConfiguration(resolved)
  if (
    !(await actorMayApplyBrokerSignature(
      input.actorAppUserId,
      resolved.appUserId,
      execute,
    ))
  ) {
    return {
      ok: false,
      outcome: 'unauthorized',
      message:
        'document.issue failed: the authenticated actor is neither the configured broker signature owner nor a ROOT delegate.',
    }
  }

  const roleSlots = input.participants.filter(
    (slot) =>
      slot.role === policy.role &&
      normalized(slot.name) === normalized(config.signerName),
  )
  if (roleSlots.length > 1) {
    return invalidConfiguration(
      `resolved more than one ${policy.role} participant for ${config.signerName}.`,
    )
  }
  const slot = roleSlots[0] ?? null
  if (slot?.personId && resolved.personId && slot.personId !== resolved.personId) {
    return {
      ok: false,
      outcome: 'unauthorized',
      message:
        'document.issue failed: the broker participant does not match the canonical signature owner.',
    }
  }
  const requireExecutionSlot = input.requireExecutionSlot !== false
  if (
    requireExecutionSlot &&
    (input.template.id === 'PR-PNS' || input.template.id === 'LISTING-01') &&
    !slot
  ) {
    return invalidConfiguration(
      `could not resolve Lisa to the required ${policy.role} execution slot for ${input.template.id}.`,
    )
  }

  const asset = await loadProtectedSignatureAsset(resolved.mediaId, execute)
  if (typeof asset === 'string') return invalidConfiguration(asset)

  return {
    ok: true,
    signatures: [
      {
        role: policy.role,
        slotId: slot?.slotId ?? null,
        signerName: config.signerName,
        credentialLine: `Real Estate Broker License #: ${config.licenseNumber}`,
        signerAppUserId: resolved.appUserId,
        imageBytes: asset.bytes,
        imageMimeType: asset.mimeType,
        assetMediaId: resolved.mediaId,
        assetChecksumSha256: asset.checksumSha256,
        appliedAt: new Date(input.issuedAt).toISOString(),
        consentBasis: BROKER_SIGNATURE_CONSENT_BASIS,
        dateSemantic: BROKER_SIGNATURE_DATE_SEMANTIC,
      },
    ],
  }
}
