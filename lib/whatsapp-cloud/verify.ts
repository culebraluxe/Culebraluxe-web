import { createHmac, timingSafeEqual } from 'node:crypto'

function safeEqual(left: string, right: string): boolean {
  const a = Buffer.from(left, 'utf8')
  const b = Buffer.from(right, 'utf8')
  return a.length === b.length && timingSafeEqual(a, b)
}

export function verifyMetaWhatsAppHandshake(input: {
  mode: string | null
  token: string | null
  challenge: string | null
  expectedToken: string
}): string | null {
  if (input.mode !== 'subscribe') return null
  if (!input.token || !input.challenge || !input.expectedToken) return null
  return safeEqual(input.token, input.expectedToken) ? input.challenge : null
}

/** Meta signs the exact raw request body as `sha256=<lowercase hex>`. */
export function verifyMetaWhatsAppSignature(
  rawBody: string,
  header: string | null,
  appSecret: string,
): boolean {
  if (!header?.startsWith('sha256=') || !appSecret) return false
  const supplied = header.slice('sha256='.length)
  if (!/^[a-f0-9]{64}$/i.test(supplied)) return false
  const expected = createHmac('sha256', appSecret)
    .update(rawBody, 'utf8')
    .digest('hex')
  return safeEqual(supplied.toLowerCase(), expected)
}
