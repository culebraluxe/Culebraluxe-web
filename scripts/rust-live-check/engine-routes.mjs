// Engine route auth matrix + one real call. Run with the API up: pnpm dev, or cargo run -p server --bin http
import { apiBase, internalKey, devIdentity } from './_env.mjs'

const key = internalKey()
const identity = await devIdentity()

const post = async (label, body, headers) => {
  const response = await fetch(`${apiBase}/v1/engine/reclaim`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...headers },
    body: JSON.stringify(body),
  })
  const text = await response.text()
  const code = (() => {
    try {
      return JSON.parse(text).error?.code ?? 'ok'
    } catch {
      return 'non-json'
    }
  })()
  console.log(`${String(response.status).padEnd(4)} ${code.padEnd(26)} ${label}`)
  return response.status
}

console.log('expected: 401, 401, 401, then 200\n')
let failures = 0
if ((await post('no internal key', { batch: 1 }, {})) !== 401) failures += 1
if ((await post('wrong internal key', { batch: 1 }, { 'x-culebra-internal-key': 'nope-nope-nope-nope' })) !== 401)
  failures += 1
if (
  (await post('one identity header only', { batch: 1 }, { 'x-culebra-internal-key': key, 'x-culebra-auth-provider': 'authjs' })) !==
  401
)
  failures += 1
if ((await post('background command (no identity)', { batch: 1 }, { 'x-culebra-internal-key': key })) !== 200) failures += 1
if (
  (await post(
    `attributed to ${identity?.provider ?? 'no mapped identity'}`,
    { batch: 1 },
    {
      'x-culebra-internal-key': key,
      'x-culebra-auth-provider': identity.provider,
      'x-culebra-auth-sub': identity.provider_subject,
    },
  )) !== 200
)
  failures += 1

console.log(failures ? `\n${failures} unexpected result(s)` : '\nall as expected')
process.exit(failures ? 1 : 0)
