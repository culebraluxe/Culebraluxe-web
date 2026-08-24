// ---------------------------------------------------------------------------
// DOC-06 — PDF renderer (pdf-lib).
//
// TemplateDefinition + field/section values → real PDF 1.4 binary bytes.
// Preview, issuance, Save, and Share all use this function so the vault blob
// is the same document the user attaches in Mail/Messages.
// ---------------------------------------------------------------------------

import { readFile } from 'node:fs/promises'
import { join } from 'node:path'

import { PDFDocument, PDFFont, PDFImage, PDFPage, StandardFonts, rgb } from 'pdf-lib'

import type {
  TemplateDefinition,
  TemplateFieldValues,
  TemplateSectionValues,
} from './template-types'
import {
  documentBodyText,
  formatDate,
  formatFieldValue,
  formatMoney,
  interpolateSectionText,
} from './format'

export {
  documentBodyText,
  formatDate,
  formatFieldValue,
  formatMoney,
  interpolateSectionText,
}

const PAGE_WIDTH = 612
const PAGE_HEIGHT = 792
const MARGIN = 56
const MAX_LINE_WIDTH = PAGE_WIDTH - MARGIN * 2
const TOP_MARGIN = 72
const BOTTOM_MARGIN = 72

// ---------------------------------------------------------------------------
// Brand logo — a transparent PNG wordmark drawn top-right of every generated
// form. Read from the public/ uploads path (replaceable without a rebuild) and
// gracefully skipped (null) when unavailable so a form never fails to render
// because of a missing logo. NOTE: on Vercel serverless the public/ folder is
// served as static assets and is NOT on the function filesystem — if the logo
// is required there, the bytes must be bundled instead (see summary).
// ---------------------------------------------------------------------------

const LOGO_PUBLIC_PATH = 'brand/CLLOGO.png'
/**
 * Logo height in PDF points, sized to match the scale of the document title
 * text (e.g. "OFFER LETTER") on the left. The wordmark is a wide 3:1, so height
 * 50 ≈ 150pt wide. The header title wraps into the space left of the logo so a
 * long document title never overlaps it.
 */
const LOGO_TARGET_HEIGHT_PT = 50

let logoBytesCache: Uint8Array | null | undefined

async function loadLogoBytes(): Promise<Uint8Array | null> {
  if (logoBytesCache !== undefined) return logoBytesCache
  try {
    const raw = await readFile(join(process.cwd(), 'public', LOGO_PUBLIC_PATH))
    logoBytesCache = new Uint8Array(raw)
  } catch {
    logoBytesCache = null
  }
  return logoBytesCache
}

async function embedLogo(doc: PDFDocument): Promise<PDFImage | null> {
  const bytes = await loadLogoBytes()
  if (!bytes) return null
  try {
    return await doc.embedPng(bytes)
  } catch {
    return null
  }
}

/** Greedy word-wrap using the embedded font's widths. */
export function wrapText(
  text: string,
  fontSize: number,
  maxWidth: number,
  font?: PDFFont,
): string[] {
  const normalized = text.replace(/\s+/g, ' ').trim()
  if (!normalized) return []
  const words = normalized.split(' ')
  const widthOf = (value: string) =>
    font ? font.widthOfTextAtSize(value, fontSize) : value.length * fontSize * 0.5
  const lines: string[] = []
  let current = ''
  for (const word of words) {
    const candidate = current ? `${current} ${word}` : word
    if (widthOf(candidate) <= maxWidth) {
      current = candidate
    } else {
      if (current) lines.push(current)
      current = word
    }
  }
  if (current) lines.push(current)
  return lines
}

function pdfSafe(value: string): string {
  return value.replace(/[^\x20-\x7e]/g, '?')
}

class PdfWriter {
  y = PAGE_HEIGHT - 90

  constructor(
    private readonly doc: PDFDocument,
    private page: PDFPage,
    private readonly regular: PDFFont,
    private readonly bold: PDFFont,
    private readonly logo: PDFImage | null,
  ) {}

  static async create(): Promise<PdfWriter> {
    const doc = await PDFDocument.create()
    const regular = await doc.embedFont(StandardFonts.Helvetica)
    const bold = await doc.embedFont(StandardFonts.HelveticaBold)
    const page = doc.addPage([PAGE_WIDTH, PAGE_HEIGHT])
    return new PdfWriter(doc, page, regular, bold, await embedLogo(doc))
  }

  /** Drawn logo width in points, preserving the source aspect ratio. */
  private logoWidth(): number {
    if (!this.logo) return 0
    const scale = LOGO_TARGET_HEIGHT_PT / this.logo.height
    return Math.round(this.logo.width * scale)
  }

  /** Text width available to the LEFT of the logo (whole line when no logo). */
  headerTextWidth(): number {
    if (!this.logo) return MAX_LINE_WIDTH
    return Math.max(120, PAGE_WIDTH - MARGIN - this.logoWidth() - MARGIN)
  }

  /** Draw the brand logo top-right, aligned with the top of the content area. */
  drawLogoTopRight() {
    if (!this.logo) return
    const height = LOGO_TARGET_HEIGHT_PT
    const width = this.logoWidth()
    this.page.drawImage(this.logo, {
      x: PAGE_WIDTH - MARGIN - width,
      y: this.y - height,
      width,
      height,
    })
  }

  ensureSpace(needed: number): boolean {
    if (this.y - needed >= BOTTOM_MARGIN) return false
    this.page = this.doc.addPage([PAGE_WIDTH, PAGE_HEIGHT])
    this.y = PAGE_HEIGHT - TOP_MARGIN
    return true
  }

  text(bold: boolean, size: number, value: string, x = MARGIN) {
    const safe = pdfSafe(value)
    if (!safe) return
    this.ensureSpace(size * 1.45)
    this.page.drawText(safe, {
      x,
      y: this.y,
      size,
      font: bold ? this.bold : this.regular,
      color: rgb(0, 0, 0),
    })
    this.y -= size * 1.45
  }

  paragraph(bold: boolean, size: number, value: string, maxWidth = MAX_LINE_WIDTH) {
    const font = bold ? this.bold : this.regular
    for (const line of wrapText(pdfSafe(value), size, maxWidth, font)) {
      this.ensureSpace(size * 1.45)
      this.page.drawText(line, {
        x: MARGIN,
        y: this.y,
        size,
        font,
        color: rgb(0, 0, 0),
      })
      this.y -= size * 1.45
    }
  }

  rule() {
    this.page.drawLine({
      start: { x: MARGIN, y: this.y },
      end: { x: MARGIN + MAX_LINE_WIDTH, y: this.y },
      thickness: 0.6,
      color: rgb(0, 0, 0),
    })
    this.y -= 14
  }

  /**
   * A drawn signature baseline: a horizontal rule the signer signs on, placed
   * just below the cursor. Cleaner than an underscore run and lets the signer's
   * name read above the line.
   */
  signatureLine(width: number) {
    this.ensureSpace(22)
    const baselineY = this.y - 4
    this.page.drawLine({
      start: { x: MARGIN, y: baselineY },
      end: { x: MARGIN + width, y: baselineY },
      thickness: 0.8,
      color: rgb(0, 0, 0),
    })
    this.y -= 20
  }

  space(points: number) {
    this.y -= points
  }

  async save(): Promise<Buffer> {
    const bytes = await this.doc.save({ useObjectStreams: false })
    return Buffer.from(bytes)
  }
}

function drawBody(layout: PdfWriter, body: string) {
  const blocks = body.split(/\n{2,}/)
  for (const block of blocks) {
    const trimmed = block.trim()
    if (!trimmed) continue
    const lines = trimmed.split('\n')
    const heading = lines[0]?.trim() ?? ''
    const rest = lines.slice(1).join(' ').trim()
    if (rest) {
      layout.ensureSpace(28)
      layout.text(true, 11, heading)
      layout.paragraph(false, 10, rest)
    } else {
      layout.paragraph(false, 10, heading)
    }
    layout.space(10)
  }
}

async function buildTemplatePdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Promise<Buffer> {
  const layout = await PdfWriter.create()
  layout.drawLogoTopRight()
  // Clean header: the document title sits at the top-left, balancing the logo
  // top-right, with a solid rule beneath. The issuer text is omitted for a
  // cleaner look; the version stamp moves below the rule so it stays subtle.
  layout.paragraph(true, 16, template.rendering.title, layout.headerTextWidth())
  layout.rule()
  layout.text(false, 8, `Issued document v${issuedVersion}`)
  layout.space(10)

  for (const field of template.fields) {
    const raw = (values[field.name] ?? '').trim()
    if (!raw && !field.required) continue
    layout.text(true, 8, field.label.toUpperCase())
    layout.text(false, 11, formatFieldValue(field, raw))
    layout.space(8)
  }
  layout.space(10)

  const body =
    (sections.body ?? '').trim() || documentBodyText(template, values)
  drawBody(layout, body)
  drawSignatures(layout, template, values)
  return layout.save()
}

export async function renderFormPdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Promise<Buffer> {
  return buildTemplatePdf(template, values, sections, issuedVersion)
}

export async function buildOfferLetterPdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Promise<Buffer> {
  return buildTemplatePdf(template, values, sections, issuedVersion)
}

export async function buildPurchaseSalePdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Promise<Buffer> {
  return buildTemplatePdf(template, values, sections, issuedVersion)
}

function drawSignatures(
  layout: PdfWriter,
  template: TemplateDefinition,
  values: TemplateFieldValues,
) {
  if (template.signatureGroups.length === 0) return
  // A bounded, distinct section closing the document: a separator rule, a clear
  // heading, and one drawn signing baseline per signature group. Spacing keeps
  // each signer's name, signature line, and initials grouped together.
  layout.ensureSpace(72)
  layout.rule()
  layout.space(6)
  layout.text(true, 12, 'SIGNATURES')
  layout.space(8)
  for (const group of template.signatureGroups) {
    layout.ensureSpace(56)
    const name = group.field ? (values[group.field] ?? '').trim() : ''
    layout.text(false, 10, `${group.label}${name ? ` — ${name}` : ''}`)
    layout.signatureLine(300)
    if (group.initials) {
      layout.space(4)
      layout.text(false, 9, 'Initials')
      layout.signatureLine(140)
    }
    layout.space(16)
  }
}
