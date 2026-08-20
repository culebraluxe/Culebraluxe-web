// Generates a scrypt hash for AUTH_BREAK_GLASS_SECRET_HASH.
// Run locally with the secret (never commit the secret or this output):
//   node scripts/generate-break-glass-hash.mjs '<strong-secret>'
// Copy the printed value into server-side environment configuration.

import { randomBytes, scryptSync } from 'node:crypto'

const secret = process.argv[2]
if (!secret || secret.length < 16) {
  console.error('Provide a secret of at least 16 characters as the first argument.')
  process.exit(1)
}

const salt = randomBytes(16)
const key = scryptSync(secret, salt, 64)
console.log(`scrypt$${salt.toString('hex')}$${key.toString('hex')}`)
