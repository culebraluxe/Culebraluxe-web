// AUTH-02 break-glass secret verification.
//
// Uses Node's built-in crypto (scrypt + timingSafeEqual) — no external password
// library, no plaintext secret in source or database. Only the scrypt hash is
// ever held in server-side environment configuration.

import { randomBytes, scryptSync, timingSafeEqual } from 'node:crypto'

const KEY_LENGTH = 64

// Format: scrypt$<salt hex>$<key hex>
export function hashBreakGlassSecret(secret: string, salt?: Buffer): string {
  const saltBuf = salt ?? randomBytes(16)
  const key = scryptSync(secret, saltBuf, KEY_LENGTH)
  return `scrypt$${saltBuf.toString('hex')}$${key.toString('hex')}`
}

export function verifyBreakGlassSecret(
  secret: string,
  storedHash: string,
): boolean {
  try {
    const parts = storedHash.split('$')
    if (parts.length !== 3 || parts[0] !== 'scrypt') return false
    const salt = Buffer.from(parts[1], 'hex')
    const expected = Buffer.from(parts[2], 'hex')
    const actual = scryptSync(secret, salt, expected.length)
    return (
      actual.length === expected.length && timingSafeEqual(actual, expected)
    )
  } catch {
    return false
  }
}
