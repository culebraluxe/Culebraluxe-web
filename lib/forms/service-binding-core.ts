import { randomUUID } from 'node:crypto'

export function compactFormValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : ''
}

export function nullableFormValue(value: unknown): string | null {
  const clean = compactFormValue(value)
  return clean || null
}

export function formServiceContext(actorId: string | null = null) {
  return {
    actor: { id: actorId, kind: actorId ? 'user' as const : 'system' as const },
    correlationId: randomUUID(),
  }
}

export async function serviceValue<T>(
  promise: Promise<{ ok: true; value: T } | { ok: false; error: { code: string; message: string } }>,
  label: string,
): Promise<T> {
  const result = await promise
  if (!result.ok) throw new Error(`${label}: ${result.error.code} ${result.error.message}`)
  return result.value
}
