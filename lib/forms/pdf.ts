// ---------------------------------------------------------------------------
// DOC-06 — PDF renderer (pdf-lib).
//
// TemplateDefinition + field/section values → real PDF 1.4 binary bytes.
// Preview, issuance, Save, and Share all use this function so the vault blob
// is the same document the user attaches in Mail/Messages.
// ---------------------------------------------------------------------------

import { PDFDocument, PDFFont, PDFPage, StandardFonts, rgb } from 'pdf-lib'

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
  ) {}

  static async create(): Promise<PdfWriter> {
    const doc = await PDFDocument.create()
    const regular = await doc.embedFont(StandardFonts.Helvetica)
    const bold = await doc.embedFont(StandardFonts.HelveticaBold)
    const page = doc.addPage([PAGE_WIDTH, PAGE_HEIGHT])
    return new PdfWriter(doc, page, regular, bold)
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

  paragraph(bold: boolean, size: number, value: string) {
    const font = bold ? this.bold : this.regular
    for (const line of wrapText(pdfSafe(value), size, MAX_LINE_WIDTH, font)) {
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
    layout.space(8)
  }
}

async function buildTemplatePdf(
  template: TemplateDefinition,
  values: TemplateFieldValues,
  sections: TemplateSectionValues,
  issuedVersion: number,
): Promise<Buffer> {
  const layout = await PdfWriter.create()
  layout.text(false, 9, template.rendering.issuer.toUpperCase())
  layout.text(true, 16, template.rendering.title)
  layout.text(false, 8, `Issued document v${issuedVersion}`)
  layout.rule()
  layout.space(10)

  for (const field of template.fields) {
    const raw = (values[field.name] ?? '').trim()
    if (!raw && !field.required) continue
    layout.text(true, 9, field.label.toUpperCase())
    layout.text(false, 11, formatFieldValue(field, raw))
    layout.space(4)
  }
  layout.space(8)

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
  layout.text(true, 13, 'SIGNATURES')
  layout.space(4)
  for (const group of template.signatureGroups) {
    layout.ensureSpace(52)
    const name = group.field ? (values[group.field] ?? '').trim() : ''
    layout.text(false, 10, `${group.label}${name ? ` — ${name}` : ''}`)
    layout.text(false, 10, 'By: ____________________________________')
    if (group.initials) layout.text(false, 10, 'Initials: ________')
    layout.space(12)
  }
}
