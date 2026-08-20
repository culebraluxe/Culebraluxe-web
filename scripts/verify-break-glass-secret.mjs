// Runnable pure verification of the break-glass secret algorithm
// (mirrors lib/auth/break-glass-secret.ts). No DB, no writes.
//   node scripts/verify-break-glass-secret.mjs

import { randomBytes, scryptSync, timingSafeEqual } from 'node:crypto'

function hash(secret, salt) {
  const s = salt ?? randomBytes(16)
  const key = scryptSync(secret, s, 64)
  return `scrypt$${s.toString('hex')}$${key.toString('hex')}`
}

function verify(secret, storedHash) {
  try {
    const parts = storedHash.split('$')
    if (parts.length !== 3 || parts[0] !== 'scrypt') return false
    const salt = Buffer.from(parts[1], 'hex')
    const expected = Buffer.from(parts[2], 'hex')
    const actual = scryptSync(secret, salt, expected.length)
    return actual.length === expected.length && timingSafeEqual(actual, expected)
  } catch {
    return false
  }
}

const cases = [
  ['correct secret verifies', verify('correct horse battery staple', hash('correct horse battery staple')) === true],
  ['wrong secret rejected', verify('wrong', hash('correct horse battery staple')) === false],
  ['malformed hash rejected', verify('x', 'not-a-hash') === false],
  ['empty secret rejected', verify('', hash('correct horse battery staple')) === false],
  ['deterministic per salt', (() => {
    const salt = Buffer.alloc(16, 1)
    return hash('abc', salt) === hash('abc', salt)
  })()],
]

let failed = 0
for (const [label, pass] of cases) {
  if (!pass) failed += 1
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${label}`)
}
console.log(failed === 0 ? 'ALL PASSED' : `${failed} FAILED`)
process.exit(failed === 0 ? 0 : 1)
