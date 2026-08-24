import type { JsonObject } from '../crm-types'
import {
  lowerBatchItemToIntakeMessage,
  type IntakeBatchItemInput,
  type IntakeBatchManifest,
} from './batch'
import {
  assertValidIntakeMessage,
  type CanonicalIntakeMessage,
  type IntakeIdentity,
  type IntakeParticipant,
} from './contracts'

export const APPLE_CONTACTS_SOURCE_SYSTEM = 'apple_contacts'
export const APPLE_CONTACTS_EVENT_TYPE = 'contact.imported'
export const APPLE_CONTACTS_ADAPTER = 'apple-contacts.swift-json'
export const APPLE_CONTACTS_ADAPTER_VERSION = '1.0.0'

export type AppleLabeledTextValue = {
  sourceLabel: string | null
  value: string
}

export type ApplePostalAddress = {
  sourceLabel: string | null
  street: string
  city: string
  state: string
  postalCode: string
  country: string
  isoCountryCode: string
}

export type AppleContactExport = {
  sourceId: string
  namePrefix: string
  givenName: string
  middleName: string
  familyName: string
  nameSuffix: string
  nickname: string
  organization: string
  department: string
  jobTitle: string
  emails: AppleLabeledTextValue[]
  phones: AppleLabeledTextValue[]
  postalAddresses: ApplePostalAddress[]
}

export type AppleContactExportBatch = {
  schemaVersion: 1
  sourceSystem: 'apple_contacts'
  exportId: string
  exportedAt: string
  contacts: AppleContactExport[]
}

function object(value: unknown, path: string): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${path} must be an object.`)
  }
  return value as Record<string, unknown>
}

function string(value: unknown, path: string): string {
  if (typeof value !== 'string') throw new Error(`${path} must be a string.`)
  return value
}

function nullableString(value: unknown, path: string): string | null {
  if (value === null || value === undefined) return null
  return string(value, path)
}

function array(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`${path} must be an array.`)
  return value
}

function labeledText(value: unknown, path: string): AppleLabeledTextValue {
  const row = object(value, path)
  return {
    sourceLabel: nullableString(row.sourceLabel, `${path}.sourceLabel`),
    value: string(row.value, `${path}.value`),
  }
}

function postalAddress(value: unknown, path: string): ApplePostalAddress {
  const row = object(value, path)
  return {
    sourceLabel: nullableString(row.sourceLabel, `${path}.sourceLabel`),
    street: string(row.street, `${path}.street`),
    city: string(row.city, `${path}.city`),
    state: string(row.state, `${path}.state`),
    postalCode: string(row.postalCode, `${path}.postalCode`),
    country: string(row.country, `${path}.country`),
    isoCountryCode: string(row.isoCountryCode, `${path}.isoCountryCode`),
  }
}

function contact(value: unknown, index: number): AppleContactExport {
  const path = `contacts[${index}]`
  const row = object(value, path)
  const sourceId = string(row.sourceId, `${path}.sourceId`).trim()
  if (!sourceId) throw new Error(`${path}.sourceId is required.`)
  return {
    sourceId,
    namePrefix: string(row.namePrefix, `${path}.namePrefix`),
    givenName: string(row.givenName, `${path}.givenName`),
    middleName: string(row.middleName, `${path}.middleName`),
    familyName: string(row.familyName, `${path}.familyName`),
    nameSuffix: string(row.nameSuffix, `${path}.nameSuffix`),
    nickname: string(row.nickname, `${path}.nickname`),
    organization: string(row.organization, `${path}.organization`),
    department: string(row.department, `${path}.department`),
    jobTitle: string(row.jobTitle, `${path}.jobTitle`),
    emails: array(row.emails, `${path}.emails`).map((item, itemIndex) =>
      labeledText(item, `${path}.emails[${itemIndex}]`),
    ),
    phones: array(row.phones, `${path}.phones`).map((item, itemIndex) =>
      labeledText(item, `${path}.phones[${itemIndex}]`),
    ),
    postalAddresses: array(row.postalAddresses, `${path}.postalAddresses`).map(
      (item, itemIndex) =>
        postalAddress(item, `${path}.postalAddresses[${itemIndex}]`),
    ),
  }
}

/** Parse and validate the versioned JSON artifact emitted by contact-export. */
export function parseAppleContactExportBatch(
  value: unknown,
): AppleContactExportBatch {
  const row = object(value, 'batch')
  if (row.schemaVersion !== 1) {
    throw new Error('batch.schemaVersion must be 1.')
  }
  if (row.sourceSystem !== APPLE_CONTACTS_SOURCE_SYSTEM) {
    throw new Error(`batch.sourceSystem must be '${APPLE_CONTACTS_SOURCE_SYSTEM}'.`)
  }
  const exportId = string(row.exportId, 'batch.exportId').trim()
  if (!exportId) throw new Error('batch.exportId is required.')
  const exportedAt = string(row.exportedAt, 'batch.exportedAt')
  if (Number.isNaN(new Date(exportedAt).getTime())) {
    throw new Error('batch.exportedAt must be a valid ISO timestamp.')
  }
  const contacts = array(row.contacts, 'batch.contacts').map(contact)
  const seen = new Set<string>()
  for (const item of contacts) {
    if (seen.has(item.sourceId)) {
      throw new Error(`Duplicate Apple sourceId in one export: ${item.sourceId}`)
    }
    seen.add(item.sourceId)
  }
  return {
    schemaVersion: 1,
    sourceSystem: APPLE_CONTACTS_SOURCE_SYSTEM,
    exportId,
    exportedAt,
    contacts,
  }
}

export function appleContactDisplayName(contact: AppleContactExport): string {
  const personal = [
    contact.namePrefix,
    contact.givenName,
    contact.middleName,
    contact.familyName,
    contact.nameSuffix,
  ]
    .map((part) => part.trim())
    .filter(Boolean)
    .join(' ')
  return (
    personal ||
    contact.organization.trim() ||
    contact.nickname.trim() ||
    contact.sourceId
  )
}

function uniqueIdentities(values: IntakeIdentity[]): IntakeIdentity[] {
  const seen = new Set<string>()
  return values.filter((identity) => {
    const key = `${identity.kind}|${identity.value.trim().toLowerCase()}`
    if (!identity.value.trim() || seen.has(key)) return false
    seen.add(key)
    return true
  })
}

function contactIdentities(contact: AppleContactExport): IntakeIdentity[] {
  const displayName = appleContactDisplayName(contact)
  return uniqueIdentities([
    ...contact.emails.map((item) => ({
      kind: 'email' as const,
      value: item.value,
      displayName,
    })),
    ...contact.phones.map((item) => ({
      kind: 'phone' as const,
      value: item.value,
      displayName,
    })),
  ])
}

function contactSourceFacts(contact: AppleContactExport): JsonObject {
  // These are bounded, source-derived contact facts, not canonical Person
  // truth. The current inbox projection intentionally drops sourcePayload;
  // the persistence story must add an explicit neutral profile seam before
  // the full import is allowed to run.
  return JSON.parse(JSON.stringify({
    namePrefix: contact.namePrefix,
    givenName: contact.givenName,
    middleName: contact.middleName,
    familyName: contact.familyName,
    nameSuffix: contact.nameSuffix,
    nickname: contact.nickname,
    organization: contact.organization,
    department: contact.department,
    jobTitle: contact.jobTitle,
    emails: contact.emails,
    phones: contact.phones,
    postalAddresses: contact.postalAddresses,
  })) as JsonObject
}

export function appleContactToBatchItem(
  contact: AppleContactExport,
  rawReference: string,
  occurredAt: string,
): IntakeBatchItemInput {
  const displayName = appleContactDisplayName(contact)
  const identities = contactIdentities(contact)
  const participants: IntakeParticipant[] = identities.length
    ? identities.map((identity) => ({ ...identity, role: 'contact' }))
    : [
        {
          kind: 'contact',
          value: contact.sourceId,
          displayName,
          role: 'contact',
        },
      ]
  return {
    itemId: contact.sourceId,
    eventType: APPLE_CONTACTS_EVENT_TYPE,
    occurredAt,
    participants,
    contactCandidates: identities,
    content: {
      subject: displayName,
      summary: contact.organization.trim() || undefined,
    },
    rawReference: `${rawReference}#contact=${encodeURIComponent(contact.sourceId)}`,
    sourcePayload: contactSourceFacts(contact),
  }
}

/** Lower one complete Swift export into the existing canonical batch lane. */
export function lowerAppleContactExport(
  batch: AppleContactExportBatch,
  rawReference: string,
): CanonicalIntakeMessage[] {
  const manifest: IntakeBatchManifest = {
    importId: batch.exportId,
    sourceSystem: APPLE_CONTACTS_SOURCE_SYSTEM,
    adapter: APPLE_CONTACTS_ADAPTER,
    adapterVersion: APPLE_CONTACTS_ADAPTER_VERSION,
    importedAt: batch.exportedAt,
  }
  return batch.contacts.map((item) => {
    const message = lowerBatchItemToIntakeMessage(
      manifest,
      appleContactToBatchItem(item, rawReference, batch.exportedAt),
    )
    assertValidIntakeMessage(message)
    return message
  })
}
