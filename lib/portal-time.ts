import { PortalWriteError } from '@/lib/portal-write-error'

// Portal V1 operating timezone is Puerto Rico local time (UTC-04:00, no DST).
// User-entered HTML datetime-local values represent Puerto Rico wall-clock time
// and are converted to absolute ISO UTC strings at the server-action boundary
// BEFORE being passed to db/service functions. Server-generated timestamps
// (now(), new Date().toISOString()) are never routed through this helper.
export const PORTAL_TIME_ZONE_OFFSET = '-04:00'

// Matches a naive datetime-local value emitted by <input type="datetime-local">:
// YYYY-MM-DDTHH:mm or YYYY-MM-DDTHH:mm:ss (no timezone suffix).
const DATETIME_LOCAL_PATTERN =
  /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?$/

function isRealCalendarDate(year: number, month: number, day: number): boolean {
  // Date.UTC months are 0-based. Round-trip the components so invalid days
  // (e.g. Feb 30) cannot roll over into a usable timestamp.
  const probe = new Date(Date.UTC(year, month - 1, day))
  return (
    probe.getUTCFullYear() === year &&
    probe.getUTCMonth() === month - 1 &&
    probe.getUTCDate() === day
  )
}

/**
 * Converts a naive datetime-local value (Puerto Rico wall-clock time) into an
 * absolute ISO UTC string. Adds seconds when missing, appends the fixed -04:00
 * offset, and returns the UTC instant via Date.toISOString(). Malformed or
 * unparseable input throws a validation error instead of silently producing a
 * usable timestamp.
 */
export function toPortalInstant(datetimeLocal: string): string {
  if (typeof datetimeLocal !== 'string') {
    throw new PortalWriteError('validation', 'Invalid date/time value.')
  }
  const trimmed = datetimeLocal.trim()
  const match = DATETIME_LOCAL_PATTERN.exec(trimmed)
  if (!match) {
    throw new PortalWriteError('validation', 'Invalid date/time value.')
  }
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  const hour = Number(match[4])
  const minute = Number(match[5])
  const second = match[6] === undefined ? 0 : Number(match[6])
  if (
    month < 1 ||
    month > 12 ||
    hour < 0 ||
    hour > 23 ||
    minute < 0 ||
    minute > 59 ||
    second < 0 ||
    second > 59 ||
    !isRealCalendarDate(year, month, day)
  ) {
    throw new PortalWriteError('validation', 'Invalid date/time value.')
  }
  const withSeconds = match[6] === undefined ? `${trimmed}:00` : trimmed
  const instant = new Date(`${withSeconds}${PORTAL_TIME_ZONE_OFFSET}`)
  if (Number.isNaN(instant.getTime())) {
    throw new PortalWriteError('validation', 'Invalid date/time value.')
  }
  return instant.toISOString()
}
