// ---------------------------------------------------------------------------
// Provider-neutral signature geometry attached to an immutable issued PDF.
//
// Coordinates are PDF points (1/72 inch), measured from the PDF's bottom-left
// origin. Provider adapters convert this contract at their own boundary.
// Templates declare semantic signature groups; the renderer resolves their
// exact page rectangles after pagination and issuance snapshots those results.
// ---------------------------------------------------------------------------

export const PDF_SIGNATURE_COORDINATE_SPACE =
  'pdf-points-bottom-left' as const

export type PdfPointRectangle = {
  x: number
  y: number
  width: number
  height: number
}

export type FormSignatureAnchorKind = 'signature' | 'initial' | 'date'

export type FormSignatureAnchor = {
  role: string
  slotId: string | null
  kind: FormSignatureAnchorKind
  /** Zero-based canonical page index. Provider adapters usually add one. */
  pageIndex: number
  pageWidth: number
  pageHeight: number
  rect: PdfPointRectangle
  coordinateSpace: typeof PDF_SIGNATURE_COORDINATE_SPACE
}

function finitePositive(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

function finiteNonNegative(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
}

export function parseFormSignatureAnchors(
  value: unknown,
): FormSignatureAnchor[] {
  if (!Array.isArray(value)) return []
  const anchors: FormSignatureAnchor[] = []
  for (const entry of value) {
    if (!entry || typeof entry !== 'object') continue
    const raw = entry as Record<string, unknown>
    const rect = raw.rect as Record<string, unknown> | null
    if (
      typeof raw.role !== 'string' ||
      !['signature', 'initial', 'date'].includes(String(raw.kind)) ||
      typeof raw.pageIndex !== 'number' ||
      !Number.isInteger(raw.pageIndex) ||
      raw.pageIndex < 0 ||
      !finitePositive(raw.pageWidth) ||
      !finitePositive(raw.pageHeight) ||
      raw.coordinateSpace !== PDF_SIGNATURE_COORDINATE_SPACE ||
      !rect ||
      !finiteNonNegative(rect.x) ||
      !finiteNonNegative(rect.y) ||
      !finitePositive(rect.width) ||
      !finitePositive(rect.height) ||
      Number(rect.x) + Number(rect.width) > Number(raw.pageWidth) ||
      Number(rect.y) + Number(rect.height) > Number(raw.pageHeight)
    ) {
      continue
    }
    anchors.push({
      role: raw.role,
      slotId:
        typeof raw.slotId === 'string' && raw.slotId.trim()
          ? raw.slotId
          : null,
      kind: raw.kind as FormSignatureAnchorKind,
      pageIndex: Number(raw.pageIndex),
      pageWidth: Number(raw.pageWidth),
      pageHeight: Number(raw.pageHeight),
      rect: {
        x: Number(rect.x),
        y: Number(rect.y),
        width: Number(rect.width),
        height: Number(rect.height),
      },
      coordinateSpace: PDF_SIGNATURE_COORDINATE_SPACE,
    })
  }
  return anchors
}

/**
 * Resolve every field region belonging to one immutable signer slot. Slot id
 * is authoritative when supplied; role is the bounded fallback for generic
 * non-agreement forms that do not carry execution slots.
 */
export function resolveFormSignatureAnchors(
  anchors: readonly FormSignatureAnchor[],
  selection: { role?: string | null; slotId?: string | null },
): FormSignatureAnchor[] {
  if (selection.slotId) {
    const exact = anchors.filter((anchor) => anchor.slotId === selection.slotId)
    if (exact.length > 0) return exact
  }
  if (selection.role) {
    const byRole = anchors.filter((anchor) => anchor.role === selection.role)
    const slotIds = new Set(
      byRole.map((anchor) => anchor.slotId).filter(Boolean),
    )
    if (slotIds.size <= 1) return byRole
    return []
  }
  const signatureRoles = new Set(
    anchors
      .filter((anchor) => anchor.kind === 'signature')
      .map((anchor) => `${anchor.role}:${anchor.slotId ?? ''}`),
  )
  return signatureRoles.size === 1 ? [...anchors] : []
}
