export type ForgeReleaseKind = 'deployment' | 'production_verification'

export type ForgeReleaseReceipt = {
  kind: ForgeReleaseKind
  artifactSha: string
  receiptId: string
  success: boolean
  provider: string | null
}

function normalizedSha(value: string | null | undefined): string | null {
  const sha = value?.trim().toLowerCase() ?? ''
  return /^[0-9a-f]{7,64}$/.test(sha) ? sha : null
}

/** Provider-neutral. Never store a vendor type in engine facts. */
export function parseForgeReleaseReceipt(raw: {
  kind?: string | null
  artifactSha?: string | null
  receiptId?: string | null
  success?: boolean | null
  provider?: string | null
}): ForgeReleaseReceipt | null {
  const kind = raw.kind === 'production_verification' ? 'production_verification' : raw.kind === 'deployment' ? 'deployment' : null
  const artifactSha = normalizedSha(raw.artifactSha)
  const receiptId = raw.receiptId?.trim() || null
  if (!kind || !artifactSha || !receiptId) return null
  return {
    kind,
    artifactSha,
    receiptId,
    success: raw.success === true,
    provider: raw.provider?.trim() || null,
  }
}
